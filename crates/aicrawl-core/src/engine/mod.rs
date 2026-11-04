//! The engine: one implementation shared by the CLI, REST API, MCP server and dashboard.
//!
//! Every entry point builds a [`Request`] and calls [`AicrawlEngine::handle`], so
//! validation, security policy, extraction, storage and the response envelope
//! are identical across interfaces.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::config::{self, AppConfig, ExtractConfig};
use crate::crawl;
use crate::error::{AicrawlError, Result};
use crate::fetcher::{FetchOptions, FetchedPage, Fetcher};
use crate::map;
use crate::output::{count_items, ErrorBody, Issue, Meta, PageMeta, Response};
use crate::parser::{parse_html, parse_plain_text, Heading, ParsedPage};
use crate::pipeline::{self, PageOutput};
use crate::request::{Action, Request, SCHEMA_VERSION};
use crate::security::SecuritySummary;
use crate::storage::{
    job_status, CrawlJob, CrawlPage, ExtractedResult, JobKind, NewJob, NewPage, NewResult,
    SqliteStore, StoreStats,
};
use crate::util::sha256_hex;

/// Structural report produced by `aicrawl inspect`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectReport {
    pub url: String,
    pub final_url: String,
    pub status: u16,
    pub render: String,
    pub cached: bool,
    pub title: Option<String>,
    pub description: Option<String>,
    pub lang: Option<String>,
    pub word_count: usize,
    pub headings: Vec<Heading>,
    pub links: usize,
    pub images: usize,
    pub tables: usize,
    pub json_ld_blocks: usize,
    pub meta: BTreeMap<String, String>,
    pub prices: Vec<String>,
    pub truncated: bool,
    pub elapsed_ms: u64,
}

/// Extracted page text, used by agents that need content as well as data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageContent {
    pub url: String,
    pub final_url: String,
    pub status: u16,
    pub content_type: String,
    pub text: String,
    pub markdown: String,
    pub word_count: usize,
    pub truncated: bool,
    pub cached: bool,
    pub rendered: bool,
}

/// One keyword match inside a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub line: usize,
    pub snippet: String,
    pub score: usize,
}

/// Matches for one page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMatches {
    pub url: String,
    pub title: Option<String>,
    pub matches: Vec<SearchHit>,
}

/// Result of `search_pages`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchReport {
    pub query: String,
    pub pages_processed: u32,
    pub count: usize,
    pub pages: Vec<PageMatches>,
}

/// Summary fingerprint of a page, used by `compare_pages`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fingerprint {
    pub url: String,
    pub final_url: String,
    pub status: u16,
    pub title: Option<String>,
    pub content_hash: String,
    pub word_count: usize,
    pub text_chars: usize,
}

/// Result of `compare_pages`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareReport {
    pub a: Fingerprint,
    pub b: Fingerprint,
    pub same_content: bool,
    pub differences: Vec<String>,
}

/// Stored crawl with its pages and per-page payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlDetails {
    pub job: CrawlJob,
    pub pages: Vec<CrawlPage>,
    pub results: Vec<Value>,
}

struct Inner {
    config: AppConfig,
    data_dir: PathBuf,
    store: SqliteStore,
    fetcher: Arc<Fetcher>,
    cancels: Mutex<BTreeMap<String, CancellationToken>>,
    started: Instant,
}

/// Shared engine handle. Cloning is cheap; all state is behind an `Arc`.
#[derive(Clone)]
pub struct AicrawlEngine {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for AicrawlEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AicrawlEngine")
            .field("database", &self.inner.store.location())
            .field("data_dir", &self.inner.data_dir)
            .finish()
    }
}

impl AicrawlEngine {
    /// Opens the default database and builds the shared fetcher.
    pub async fn open(config: AppConfig) -> Result<Self> {
        let data_dir = config::ensure_data_dir(&config::data_dir())?;
        let path = config.database_path();
        let store = SqliteStore::connect(&path, config.storage.max_connections.max(1)).await?;
        Self::from_store(config, data_dir, store)
    }

    /// Opens an engine against an existing store (tests and embeddings).
    pub fn from_store(config: AppConfig, data_dir: PathBuf, store: SqliteStore) -> Result<Self> {
        let fetcher = Arc::new(Fetcher::new(
            config.crawler.clone(),
            config.cache.clone(),
            &config.security,
            store.clone(),
        )?);
        Ok(Self {
            inner: Arc::new(Inner {
                config,
                data_dir,
                store,
                fetcher,
                cancels: Mutex::new(BTreeMap::new()),
                started: Instant::now(),
            }),
        })
    }

    /// Opens an in-memory engine with permissive local settings (tests only).
    pub async fn in_memory(mut config: AppConfig) -> Result<Self> {
        config.security.allow_private_networks = true;
        let store = SqliteStore::connect_memory().await?;
        Self::from_store(config, PathBuf::from("."), store)
    }

    /// Active configuration.
    pub fn config(&self) -> &AppConfig {
        &self.inner.config
    }

    /// Extraction limits.
    pub fn extract_config(&self) -> &ExtractConfig {
        &self.inner.config.extract
    }

    /// Shared store.
    pub fn store(&self) -> &SqliteStore {
        &self.inner.store
    }

    /// Shared fetcher.
    pub fn fetcher(&self) -> &Arc<Fetcher> {
        &self.inner.fetcher
    }

    /// Directory holding the database and configuration.
    pub fn data_dir(&self) -> &std::path::Path {
        &self.inner.data_dir
    }

    /// Process uptime in milliseconds.
    pub fn uptime_ms(&self) -> u64 {
        self.inner.started.elapsed().as_millis() as u64
    }

    /// Validates a request and runs the requested action.
    pub async fn handle(&self, request: Request) -> Response {
        if let Err(err) = request.validate() {
            return Response::failure(&err);
        }
        let result = match request.action {
            Action::Extract if request.targets().len() > 1 => self.run_batch(&request).await,
            Action::Extract => self.run_extract(&request).await,
            Action::Batch => self.run_batch(&request).await,
            Action::Crawl => self.run_crawl(&request).await,
            Action::Map => self.run_map(&request).await,
        };
        match result {
            Ok(response) => response,
            Err(err) => Response::failure(&err),
        }
    }

    /// Validates a request without any network access.
    pub fn validate(&self, request: &Request) -> Result<()> {
        request.validate()
    }

    // ------------------------------------------------------------- extraction

    async fn run_extract(&self, request: &Request) -> Result<Response> {
        let started = Instant::now();
        let target = request
            .targets()
            .first()
            .cloned()
            .ok_or_else(|| AicrawlError::InvalidInput("request must contain 'url'".into()))?;
        let url = self.parse_target(&target)?;
        let job = self
            .open_job(JobKind::Extract, &url.to_string(), request)
            .await?;

        let pages = pipeline::run_request_pages(
            &self.inner.fetcher,
            &self.inner.config.extract,
            request,
            &url,
        )
        .await;
        let pages = match pages {
            Ok(pages) => pages,
            Err(err) => {
                self.fail_job(&job, &err).await;
                return Err(err);
            }
        };

        let (data, issues) = pipeline::merge_pages(&pages, request.dedupe_spec().as_ref());
        let elapsed = started.elapsed().as_millis() as u64;
        let meta = self.build_meta(&pages, &data, elapsed, Some(&job.id), None);

        if let Some(required) = required_issues(&issues) {
            let message = format!(
                "{} required field(s) were not found: {}",
                required.len(),
                required
                    .iter()
                    .map(|issue| issue.field.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            self.inner
                .store
                .finish_job(
                    &job.id,
                    job_status::FAILED,
                    pages.len() as u32,
                    Some(&message),
                )
                .await
                .ok();
            return Ok(Response::failure_with_issues(
                "FIELD_REQUIRED",
                message,
                false,
                issues,
            ));
        }

        let result = self
            .record_result(&job.id, &url.to_string(), &data, &pages, request)
            .await?;
        self.inner
            .store
            .finish_job(&job.id, job_status::SUCCEEDED, pages.len() as u32, None)
            .await
            .ok();
        let mut meta = meta;
        meta.result_id = Some(result.id.clone());

        if issues.is_empty() {
            Ok(Response::ok(data, meta))
        } else {
            Ok(Response::partial(data, meta, issues))
        }
    }

    async fn run_batch(&self, request: &Request) -> Result<Response> {
        let started = Instant::now();
        let targets = request.targets();
        let root = targets
            .first()
            .cloned()
            .ok_or_else(|| AicrawlError::InvalidInput("request must contain 'urls'".into()))?;
        let job = self.open_job(JobKind::Extract, &root, request).await?;

        let mut entries: Vec<Value> = Vec::new();
        let mut combined = Map::new();
        let mut issues: Vec<Issue> = Vec::new();
        let mut page_meta: Vec<PageMeta> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        let mut processed = 0u32;
        let mut items = 0usize;
        let mut first_error: Option<AicrawlError> = None;

        for target in &targets {
            let outcome = match self.parse_target(target) {
                Ok(url) => {
                    pipeline::run_request_pages(
                        &self.inner.fetcher,
                        &self.inner.config.extract,
                        request,
                        &url,
                    )
                    .await
                }
                Err(err) => Err(err),
            };
            match outcome {
                Ok(pages) => {
                    for page in &pages {
                        processed += 1;
                        items += page.items;
                        warnings.extend(page.warnings.clone());
                        page_meta.push(page.to_meta());
                        merge_into(&mut combined, &page.data);
                    }
                    let (data, page_issues) =
                        pipeline::merge_pages(&pages, request.dedupe_spec().as_ref());
                    issues.extend(page_issues);
                    entries.push(json!({
                        "url": target,
                        "success": true,
                        "pages_processed": pages.len(),
                        "items_extracted": count_items(&data),
                        "data": data,
                    }));
                }
                Err(err) => {
                    let body = ErrorBody::from(&err);
                    if first_error.is_none() {
                        first_error = Some(err);
                    }
                    entries.push(json!({
                        "url": target,
                        "success": false,
                        "data": Value::Null,
                        "error": body,
                    }));
                }
            }
        }

        if !entries.iter().any(|entry| entry["success"] == json!(true)) {
            let err = first_error.unwrap_or_else(|| AicrawlError::Network("batch failed".into()));
            self.fail_job(&job, &err).await;
            return Err(err);
        }

        if let Some(spec) = request.dedupe_spec() {
            for value in combined.values_mut() {
                if let Value::Array(list) = value {
                    crate::dedup::dedupe(list, &spec.keys);
                }
            }
        }

        let data = json!({
            "pages": entries,
            "combined": Value::Object(combined),
            "count": targets.len(),
        });
        let elapsed = started.elapsed().as_millis() as u64;
        let mut meta = Meta {
            url: Some(root),
            pages_processed: processed,
            items_extracted: items,
            duration_ms: elapsed,
            render: "http".into(),
            job_id: Some(job.id.clone()),
            warnings,
            pages: page_meta,
            ..Meta::default()
        };
        meta.result_id = Some(
            self.record_result(&job.id, &job.url, &data, &[], request)
                .await?
                .id,
        );
        self.inner
            .store
            .finish_job(&job.id, job_status::SUCCEEDED, processed, None)
            .await
            .ok();

        if issues.is_empty() {
            Ok(Response::ok(data, meta))
        } else {
            Ok(Response::partial(data, meta, issues))
        }
    }

    async fn run_crawl(&self, request: &Request) -> Result<Response> {
        let started = Instant::now();
        let target = request
            .targets()
            .first()
            .cloned()
            .ok_or_else(|| AicrawlError::InvalidInput("request must contain 'url'".into()))?;
        let url = self.parse_target(&target)?;
        let job = self
            .open_job(JobKind::Crawl, &url.to_string(), request)
            .await?;
        let token = CancellationToken::new();
        self.register(&job.id, token.clone());

        let outcome = crawl::crawl_site(
            self.inner.fetcher.clone(),
            &self.inner.config.extract,
            self.inner.config.crawler.max_crawl_secs,
            Some(&self.inner.store),
            request,
            &url,
            Some(token),
        )
        .await;
        self.unregister(&job.id);
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(err) => {
                self.fail_job(&job, &err).await;
                return Err(err);
            }
        };

        let mut entries: Vec<Value> = Vec::new();
        let mut issues: Vec<Issue> = Vec::new();
        let mut page_meta: Vec<PageMeta> = Vec::new();
        let mut items = 0usize;
        let mut warnings = outcome.summary.warnings.clone();
        let mut changed = 0u32;

        for page in &outcome.pages {
            items += page.items;
            warnings.extend(page.warnings.clone());
            issues.extend(page.issues.clone());
            page_meta.push(page.to_meta());
            if page.changed {
                changed += 1;
            }

            if page.status != 0 {
                self.inner
                    .store
                    .insert_page(NewPage {
                        job_id: job.id.clone(),
                        url: page.final_url.clone(),
                        depth: 0,
                        status_code: Some(page.status),
                        title: page.title.clone(),
                        content_type: Some(page.content_type.clone()),
                        text_length: page.data.to_string().len(),
                        content_hash: Some(page.content_hash.clone()),
                        word_count: page.word_count,
                        error: None,
                    })
                    .await
                    .ok();
                if !page.data.is_null() {
                    self.record_page_result(&job.id, page, request).await.ok();
                }
            }

            entries.push(json!({
                "url": page.final_url,
                "status": page.status,
                "render": page.render,
                "changed": page.changed,
                "items_extracted": page.items,
                "data": page.data,
                "warnings": page.warnings,
            }));
        }

        let summary = json!({
            "pages_fetched": outcome.summary.pages_fetched,
            "pages_succeeded": outcome.summary.pages_succeeded,
            "errors": outcome.summary.errors,
            "changed_pages": changed,
            "cancelled": outcome.summary.cancelled,
            "timed_out": outcome.summary.timed_out,
            "duration_ms": started.elapsed().as_millis() as u64,
        });
        let data = json!({
            "pages": entries,
            "urls": outcome.discovered,
            "summary": summary,
        });

        let status = if outcome.summary.cancelled {
            job_status::CANCELLED
        } else {
            job_status::SUCCEEDED
        };
        let elapsed = started.elapsed().as_millis() as u64;
        let mut meta = Meta {
            url: Some(url.to_string()),
            final_url: page_meta.first().and_then(|page| page.final_url.clone()),
            status: page_meta.first().and_then(|page| page.status),
            pages_processed: outcome.summary.pages_fetched,
            items_extracted: items,
            job_id: Some(job.id.clone()),
            duration_ms: elapsed,
            render: "http".into(),
            warnings,
            pages: page_meta,
            ..Meta::default()
        };
        meta.result_id = Some(
            self.record_result(&job.id, &url.to_string(), &data, &[], request)
                .await?
                .id,
        );
        self.inner
            .store
            .finish_job(&job.id, status, outcome.summary.pages_fetched, None)
            .await
            .ok();

        if issues.is_empty() {
            Ok(Response::ok(data, meta))
        } else {
            Ok(Response::partial(data, meta, issues))
        }
    }

    async fn run_map(&self, request: &Request) -> Result<Response> {
        let started = Instant::now();
        let target = request
            .targets()
            .first()
            .cloned()
            .ok_or_else(|| AicrawlError::InvalidInput("request must contain 'url'".into()))?;
        let url = self.parse_target(&target)?;
        let spec = request.crawl_spec();
        let (max_pages, max_depth) = spec.limits();
        let job = self
            .open_job(JobKind::Crawl, &url.to_string(), request)
            .await?;

        let outcome = map::map_site(
            &self.inner.fetcher,
            &url,
            max_pages,
            max_depth,
            spec.sitemap,
            self.inner.config.crawler.max_crawl_secs,
        )
        .await;
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(err) => {
                self.fail_job(&job, &err).await;
                return Err(err);
            }
        };

        let count = outcome.urls.len();
        let data = json!({
            "urls": outcome.urls,
            "count": count,
            "sitemap_urls": outcome.sitemap_urls,
            "pages_visited": outcome.pages_visited,
            "truncated": outcome.truncated,
        });
        self.inner
            .store
            .finish_job(&job.id, job_status::SUCCEEDED, outcome.pages_visited, None)
            .await
            .ok();

        let meta = Meta {
            url: Some(url.to_string()),
            pages_processed: outcome.pages_visited,
            items_extracted: data["count"].as_u64().unwrap_or(0) as usize,
            duration_ms: started.elapsed().as_millis() as u64,
            render: "http".into(),
            job_id: Some(job.id),
            warnings: outcome.warnings,
            ..Meta::default()
        };
        Ok(Response::ok(data, meta))
    }

    // -------------------------------------------------------------- utilities

    /// Fetches a page and describes its structure.
    pub async fn inspect(&self, input: &str, render: bool) -> Result<InspectReport> {
        let started = Instant::now();
        let url = self.parse_target(input)?;
        let page = self
            .inner
            .fetcher
            .fetch(
                &url,
                &FetchOptions {
                    render,
                    use_cache: true,
                    ..Default::default()
                },
            )
            .await?;
        let parsed = self.parse_document(&page);

        Ok(InspectReport {
            url: input.to_string(),
            final_url: page.url.clone(),
            status: page.status,
            render: if page.rendered { "playwright" } else { "http" }.into(),
            cached: page.from_cache,
            title: parsed.title.clone(),
            description: parsed.description.clone(),
            lang: parsed.lang.clone(),
            word_count: parsed.word_count,
            headings: parsed.headings.clone(),
            links: parsed.links.len(),
            images: parsed.images.len(),
            tables: parsed.tables.len(),
            json_ld_blocks: parsed.json_ld.len(),
            meta: parsed.meta.clone(),
            prices: parsed
                .prices
                .iter()
                .map(|price| price.text.clone())
                .collect(),
            truncated: parsed.truncated || page.truncated,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }

    /// Fetches a page and returns its readable text.
    pub async fn page_content(&self, input: &str) -> Result<PageContent> {
        let url = self.parse_target(input)?;
        let page = self
            .inner
            .fetcher
            .fetch(
                &url,
                &FetchOptions {
                    render: false,
                    use_cache: true,
                    ..Default::default()
                },
            )
            .await?;
        let parsed = self.parse_document(&page);
        Ok(PageContent {
            url: input.to_string(),
            final_url: page.url.clone(),
            status: page.status,
            content_type: page.content_type.clone(),
            text: parsed.text,
            markdown: parsed.markdown,
            word_count: parsed.word_count,
            truncated: parsed.truncated || page.truncated,
            cached: page.from_cache,
            rendered: page.rendered,
        })
    }

    /// Case-insensitive keyword search across one or more pages.
    pub async fn search(
        &self,
        urls: &[String],
        query: &str,
        max_hits: usize,
    ) -> Result<SearchReport> {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|term| term.to_ascii_lowercase())
            .filter(|term| !term.is_empty())
            .collect();
        if terms.is_empty() {
            return Err(AicrawlError::InvalidInput(
                "search query must contain at least one word".into(),
            ));
        }
        let max_hits = max_hits.clamp(1, 500);

        let mut pages: Vec<PageMatches> = Vec::new();
        let mut count = 0usize;
        for raw in urls.iter().take(50) {
            let Ok(url) = self.parse_target(raw) else {
                continue;
            };
            let page = match self
                .inner
                .fetcher
                .fetch(
                    &url,
                    &FetchOptions {
                        render: false,
                        use_cache: true,
                        ..Default::default()
                    },
                )
                .await
            {
                Ok(page) => page,
                Err(err) => {
                    pages.push(PageMatches {
                        url: raw.clone(),
                        title: Some(err.to_string()),
                        matches: Vec::new(),
                    });
                    continue;
                }
            };
            let parsed = self.parse_document(&page);
            let mut hits: Vec<SearchHit> = Vec::new();
            for (index, line) in parsed.text.lines().enumerate() {
                let lower = line.to_ascii_lowercase();
                let score = terms
                    .iter()
                    .filter(|term| lower.contains(term.as_str()))
                    .count();
                if score == 0 {
                    continue;
                }
                hits.push(SearchHit {
                    line: index + 1,
                    snippet: truncate(line.trim(), 240),
                    score,
                });
                if hits.len() >= max_hits {
                    break;
                }
            }
            count += hits.len();
            pages.push(PageMatches {
                url: raw.clone(),
                title: parsed.title,
                matches: hits,
            });
        }

        Ok(SearchReport {
            query: query.to_string(),
            pages_processed: pages.len() as u32,
            count,
            pages,
        })
    }

    /// Compares two pages by content fingerprint.
    pub async fn compare(&self, a: &str, b: &str) -> Result<CompareReport> {
        let first = self.fingerprint(a).await?;
        let second = self.fingerprint(b).await?;
        let mut differences: Vec<String> = Vec::new();
        if first.status != second.status {
            differences.push(format!("status: {} vs {}", first.status, second.status));
        }
        if first.title != second.title {
            differences.push(format!("title: {:?} vs {:?}", first.title, second.title));
        }
        if first.word_count != second.word_count {
            differences.push(format!(
                "word_count: {} vs {}",
                first.word_count, second.word_count
            ));
        }
        if first.content_hash != second.content_hash {
            differences.push("content hash differs".into());
        }
        Ok(CompareReport {
            same_content: first.content_hash == second.content_hash,
            a: first,
            b: second,
            differences,
        })
    }

    async fn fingerprint(&self, raw: &str) -> Result<Fingerprint> {
        let url = self.parse_target(raw)?;
        let page = self
            .inner
            .fetcher
            .fetch(&url, &FetchOptions::fresh())
            .await?;
        let parsed = self.parse_document(&page);
        Ok(Fingerprint {
            url: raw.to_string(),
            final_url: page.url.clone(),
            status: page.status,
            title: parsed.title,
            content_hash: sha256_hex(page.body.as_bytes()),
            word_count: parsed.word_count,
            text_chars: parsed.text.chars().count(),
        })
    }

    /// Loads a stored crawl job with its pages and payloads.
    pub async fn crawl_details(&self, job_id: &str) -> Result<CrawlDetails> {
        let job = self
            .inner
            .store
            .get_job(job_id)
            .await?
            .ok_or_else(|| AicrawlError::NotFound(format!("job '{job_id}'")))?;
        let pages = self
            .inner
            .store
            .list_pages(job_id)
            .await
            .unwrap_or_default();
        let results = self
            .inner
            .store
            .results_for_job(job_id)
            .await
            .unwrap_or_default();
        Ok(CrawlDetails {
            job,
            pages,
            results: results.into_iter().map(|result| result.data()).collect(),
        })
    }

    /// Serializes one job's stored results as JSON or JSON Lines.
    pub async fn export_dataset(&self, job_id: &str, format: &str) -> Result<String> {
        let format = format.trim().to_ascii_lowercase();
        if !matches!(format.as_str(), "json" | "jsonl" | "ndjson") {
            return Err(AicrawlError::InvalidInput(format!(
                "unsupported export format '{format}'; use json or jsonl"
            )));
        }
        let job = self
            .inner
            .store
            .get_job(job_id)
            .await?
            .ok_or_else(|| AicrawlError::NotFound(format!("job '{job_id}'")))?;
        let results: Vec<ExtractedResult> = self.inner.store.results_for_job(&job.id).await?;
        if results.is_empty() {
            return Err(AicrawlError::NotFound(format!(
                "job '{}' has no stored results",
                job.id
            )));
        }
        if format == "json" {
            let data: Vec<Value> = results.iter().map(ExtractedResult::data).collect();
            return Ok(serde_json::to_string_pretty(&json!({
                "job_id": job.id,
                "url": job.url,
                "status": job.status,
                "count": data.len(),
                "results": data,
            }))?);
        }
        Ok(results
            .iter()
            .map(|result| result.data().to_string())
            .collect::<Vec<_>>()
            .join("\n"))
    }

    /// Aggregate storage counters computed directly from SQLite.
    pub async fn stats(&self) -> Result<StoreStats> {
        let pool: sqlx::SqlitePool = self.inner.store.pool().clone();
        let count = |sql: &'static str| {
            let pool = pool.clone();
            async move {
                sqlx::query_scalar::<_, i64>(sql)
                    .fetch_one(&pool)
                    .await
                    .unwrap_or(0)
            }
        };
        Ok(StoreStats {
            jobs_total: count("SELECT COUNT(*) FROM crawl_jobs").await,
            jobs_running: count(
                "SELECT COUNT(*) FROM crawl_jobs WHERE status IN ('queued','running')",
            )
            .await,
            jobs_failed: count("SELECT COUNT(*) FROM crawl_jobs WHERE status = 'failed'").await,
            jobs_succeeded: count("SELECT COUNT(*) FROM crawl_jobs WHERE status = 'succeeded'")
                .await,
            pages_total: count("SELECT COUNT(*) FROM crawl_pages").await,
            results_total: count("SELECT COUNT(*) FROM extracted_results").await,
            websites_total: count("SELECT COUNT(*) FROM websites").await,
        })
    }

    /// Health payload shared by `/health`, `aicrawl doctor` and MCP.
    pub async fn health(&self) -> Value {
        let database = match self.inner.store.ping().await {
            Ok(()) => "ok".to_string(),
            Err(err) => err.to_string(),
        };
        let stats = self.stats().await.unwrap_or_default();
        let renderer = self.inner.fetcher.renderer().status().await;
        let render = serde_json::to_value(&renderer).unwrap_or_else(|_| json!({}));
        json!({
            "status": if database == "ok" { "ok" } else { "degraded" },
            "version": crate::VERSION,
            "schema_version": SCHEMA_VERSION,
            "uptime_ms": self.uptime_ms(),
            "database": { "status": database, "path": self.inner.store.location() },
            "data_dir": self.inner.data_dir.display().to_string(),
            "api": {
                "host": self.inner.config.server.api_host,
                "port": self.inner.config.server.api_port,
            },
            "mcp": {
                "host": self.inner.config.server.mcp_host,
                "port": self.inner.config.server.mcp_port,
                "endpoint": format!(
                    "http://{}:{}/mcp",
                    self.inner.config.server.mcp_host, self.inner.config.server.mcp_port
                ),
            },
            "stats": stats,
            "renderer": render,
            "security": self.security_summary(),
            "capabilities": {
                "actions": ["extract", "batch", "crawl", "map"],
                "output_formats": ["json", "jsonl"],
                "selectors": ["css", "xpath"],
                "structured_data": ["json_ld", "open_graph", "meta", "schema_org"],
                "schema_validation": true,
                "rendering": "optional (playwright)",
                "ai_required": false
            }
        })
    }

    /// Current security posture.
    pub fn security_summary(&self) -> SecuritySummary {
        SecuritySummary {
            allow_private_networks: self.inner.config.security.allow_private_networks,
            allowed_domains: self.inner.config.security.allowed_domains.clone(),
            blocked_domains: self.inner.config.security.blocked_domains.clone(),
            max_redirects: self.inner.config.security.max_redirects,
            rate_limit_per_minute: self.inner.config.security.rate_limit_per_minute,
            respect_robots_txt: self.inner.config.crawler.respect_robots_txt,
        }
    }

    /// Requests cancellation of a running crawl.
    pub async fn cancel_job(&self, job_id: &str) -> Result<bool> {
        let token = self
            .inner
            .cancels
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(job_id)
            .cloned();
        if let Some(token) = token {
            token.cancel();
            self.inner
                .store
                .set_job_status(job_id, job_status::CANCELLED, None)
                .await
                .ok();
            return Ok(true);
        }
        self.inner.store.cancel_job(job_id).await
    }

    // -------------------------------------------------------------- internals

    fn parse_target(&self, raw: &str) -> Result<Url> {
        self.inner.fetcher.policy().parse(raw)
    }

    fn parse_document(&self, page: &FetchedPage) -> ParsedPage {
        let url = Url::parse(&page.url)
            .or_else(|_| Url::parse("http://invalid.local/"))
            .expect("static fallback URL");
        let mime = page
            .content_type
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let limit = self.inner.config.extract.max_text_chars;
        if mime.contains("html") || mime.contains("xml") || mime.is_empty() {
            parse_html(&url, &page.body, limit)
        } else {
            parse_plain_text(&url, &page.body, limit)
        }
    }

    async fn open_job(&self, kind: JobKind, url: &str, request: &Request) -> Result<CrawlJob> {
        let (max_pages, max_depth) = request.crawl_spec().limits();
        let (max_pages, max_depth) = match kind {
            JobKind::Extract => (1, 0),
            JobKind::Crawl => (max_pages, max_depth),
        };
        let job = self
            .inner
            .store
            .create_job(NewJob {
                kind,
                url: url.to_string(),
                query: None,
                output_format: "json".into(),
                schema_json: request.schema.as_ref().map(|schema| schema.to_string()),
                max_pages: Some(max_pages),
                max_depth: Some(max_depth),
                meta_json: Some(
                    json!({
                        "schema_version": SCHEMA_VERSION,
                        "action": request.action.as_str(),
                        "targets": request.targets().len(),
                    })
                    .to_string(),
                ),
            })
            .await?;
        self.inner.store.start_job(&job.id).await?;
        Ok(job)
    }

    async fn fail_job(&self, job: &CrawlJob, err: &AicrawlError) {
        self.inner
            .store
            .finish_job(&job.id, job_status::FAILED, 0, Some(&err.to_string()))
            .await
            .ok();
    }

    async fn record_result(
        &self,
        job_id: &str,
        url: &str,
        data: &Value,
        pages: &[PageOutput],
        request: &Request,
    ) -> Result<ExtractedResult> {
        let mut warnings: Vec<String> = Vec::new();
        let mut duration = 0u64;
        for page in pages {
            duration += page.elapsed_ms;
            for warning in &page.warnings {
                if !warnings.contains(warning) {
                    warnings.push(warning.clone());
                }
            }
        }
        self.inner
            .store
            .insert_result(NewResult {
                job_id: job_id.to_string(),
                url: url.to_string(),
                query: None,
                mode: "deterministic".into(),
                format: "json".into(),
                data_json: data.to_string(),
                warnings_json: json!(warnings).to_string(),
                validated: request.schema.is_some(),
                duration_ms: duration as i64,
            })
            .await
    }

    async fn record_page_result(
        &self,
        job_id: &str,
        page: &PageOutput,
        request: &Request,
    ) -> Result<ExtractedResult> {
        self.inner
            .store
            .insert_result(NewResult {
                job_id: job_id.to_string(),
                url: page.final_url.clone(),
                query: None,
                mode: "deterministic".into(),
                format: "json".into(),
                data_json: page.data.to_string(),
                warnings_json: json!(page.warnings).to_string(),
                validated: request.schema.is_some(),
                duration_ms: page.elapsed_ms as i64,
            })
            .await
    }

    fn build_meta(
        &self,
        pages: &[PageOutput],
        data: &Value,
        elapsed: u64,
        job_id: Option<&str>,
        result_id: Option<&str>,
    ) -> Meta {
        let first = pages.first();
        let mut warnings: Vec<String> = Vec::new();
        for page in pages {
            for warning in &page.warnings {
                if !warnings.contains(warning) {
                    warnings.push(warning.clone());
                }
            }
        }
        Meta {
            url: first.map(|page| page.requested_url.clone()),
            final_url: first.map(|page| page.final_url.clone()),
            status: first.map(|page| page.status),
            pages_processed: pages.len() as u32,
            items_extracted: count_items(data),
            job_id: job_id.map(str::to_string),
            result_id: result_id.map(str::to_string),
            duration_ms: elapsed,
            render: first
                .map(|page| page.render.clone())
                .unwrap_or_else(|| "http".into()),
            cached: pages.iter().all(|page| page.cached),
            truncated: pages.iter().any(|page| page.truncated),
            projection: first
                .map(|page| page.projection.clone())
                .unwrap_or_default(),
            warnings,
            pages: pages.iter().map(PageOutput::to_meta).collect(),
        }
    }

    fn register(&self, job_id: &str, token: CancellationToken) {
        self.inner
            .cancels
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(job_id.to_string(), token);
    }

    fn unregister(&self, job_id: &str) {
        self.inner
            .cancels
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(job_id);
    }
}

fn required_issues(issues: &[Issue]) -> Option<Vec<Issue>> {
    let required: Vec<Issue> = issues
        .iter()
        .filter(|issue| issue.required)
        .cloned()
        .collect();
    (!required.is_empty()).then_some(required)
}

fn merge_into(target: &mut Map<String, Value>, data: &Value) {
    let Some(map) = data.as_object() else {
        return;
    };
    for (key, value) in map {
        match target.get_mut(key) {
            None => {
                target.insert(key.clone(), value.clone());
            }
            Some(Value::Array(existing)) => {
                if let Value::Array(additional) = value {
                    existing.extend(additional.iter().cloned());
                }
            }
            Some(_) => {}
        }
    }
}

fn truncate(value: &str, limit: usize) -> String {
    let mut out: String = value.chars().take(limit).collect();
    if value.chars().count() > limit {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_arrays_and_keeps_first_scalars() {
        let mut target = Map::new();
        merge_into(&mut target, &json!({"items": [{"a": 1}], "title": "one"}));
        merge_into(&mut target, &json!({"items": [{"a": 2}], "title": "two"}));
        assert_eq!(target["items"].as_array().unwrap().len(), 2);
        assert_eq!(target["title"], "one");
    }

    #[test]
    fn required_issue_detection() {
        let issues = vec![
            Issue::not_found("optional", false),
            Issue::not_found("needed", true),
        ];
        let required = required_issues(&issues).unwrap();
        assert_eq!(required.len(), 1);
        assert!(required_issues(&[]).is_none());
    }
}

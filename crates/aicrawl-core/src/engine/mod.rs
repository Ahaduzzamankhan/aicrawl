//! Shared engine facade.
//!
//! The CLI, REST API, MCP server and dashboard all call these methods, so
//! crawling, extraction, storage and security behave identically no matter how
//! a human or an AI agent reaches AICrawl.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::ai::{AiRegistry, AiStatus};
use crate::config::{ensure_data_dir, AppConfig};
use crate::crawler::{CrawlOptions, CrawlSummary, Crawler, ProgressSink};
use crate::error::{AicrawlError, Result};
use crate::extractor::{
    parse_document, ExtractOutcome, ExtractRequest, ExtractionEngine, PageSummary, QueryIntent,
};
use crate::fetcher::{FetchOptions, Fetcher};
use crate::parser::{Heading, PageLink, ParsedPage};
use crate::security::{SecretStore, SecuritySummary};
use crate::storage::{
    job_status, CrawlJob, CrawlPage, ExtractedResult, JobKind, NewJob, NewResult, SqliteStore,
    StoreStats,
};

/// One extraction including its persisted job and result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractRun {
    pub job: CrawlJob,
    pub result: ExtractedResult,
    pub outcome: ExtractOutcome,
}

/// Inspection report for a single URL (no AI involved).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectReport {
    pub requested_url: String,
    pub final_url: String,
    pub status: u16,
    pub content_type: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub lang: Option<String>,
    pub headings: Vec<Heading>,
    pub word_count: usize,
    pub text_chars: usize,
    pub truncated: bool,
    pub links: usize,
    pub link_samples: Vec<String>,
    pub images: usize,
    pub tables: usize,
    pub json_ld_types: Vec<String>,
    pub prices: usize,
    pub from_cache: bool,
    pub rendered: bool,
    pub elapsed_ms: u64,
    pub warnings: Vec<String>,
}

/// Extracted article content for one page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageContent {
    pub url: String,
    pub final_url: String,
    pub status: u16,
    pub content_type: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub text: String,
    pub markdown: String,
    pub word_count: usize,
    pub truncated: bool,
    pub links: Vec<PageLink>,
}

/// A keyword search hit inside one page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub snippet: String,
    pub score: usize,
}

/// Result of `search_website`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchReport {
    pub url: String,
    pub query: String,
    pub count: usize,
    pub matches: Vec<SearchHit>,
}

/// Job, pages and results for the MCP `get_crawl_result` tool and the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlDetails {
    pub job: CrawlJob,
    pub pages: Vec<CrawlPage>,
    pub results: Vec<Value>,
}

struct EngineInner {
    config: AppConfig,
    data_dir: PathBuf,
    store: SqliteStore,
    secrets: SecretStore,
    fetcher: Arc<Fetcher>,
    ai: AiRegistry,
    extractor: Arc<ExtractionEngine>,
    crawler: Arc<Crawler>,
    cancels: Mutex<HashMap<String, CancellationToken>>,
    started_at: Instant,
}

/// Process-wide AICrawl engine.
#[derive(Clone)]
pub struct AicrawlEngine {
    inner: Arc<EngineInner>,
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
    /// Opens the database, secret store, fetcher, AI registry and crawler.
    pub async fn open(config: AppConfig) -> Result<Self> {
        let database_path = config.database_path();
        let data_dir = database_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(crate::config::data_dir);
        ensure_data_dir(&data_dir)?;

        let store =
            SqliteStore::connect(&database_path, config.storage.max_connections.max(5)).await?;
        let secrets = SecretStore::open(&data_dir)?;
        let fetcher = Arc::new(Fetcher::new(
            config.crawler.clone(),
            config.cache.clone(),
            &config.security,
            store.clone(),
        )?);
        let ai = AiRegistry::new(store.clone(), secrets.clone(), config.ai.clone());
        let extractor = Arc::new(ExtractionEngine::new(
            fetcher.clone(),
            ai.clone(),
            config.crawler.max_text_chars,
            config.ai.max_input_chars,
        ));
        let crawler = Arc::new(Crawler::new(
            fetcher.clone(),
            store.clone(),
            extractor.clone(),
            config.crawler.clone(),
        ));

        Ok(Self {
            inner: Arc::new(EngineInner {
                config,
                data_dir,
                store,
                secrets,
                fetcher,
                ai,
                extractor,
                crawler,
                cancels: Mutex::new(HashMap::new()),
                started_at: Instant::now(),
            }),
        })
    }

    /// Effective configuration for this process.
    pub fn config(&self) -> &AppConfig {
        &self.inner.config
    }

    /// SQLite store.
    pub fn store(&self) -> &SqliteStore {
        &self.inner.store
    }

    /// Secret store (encrypted provider keys).
    pub fn secrets(&self) -> &SecretStore {
        &self.inner.secrets
    }

    /// Shared fetcher.
    pub fn fetcher(&self) -> &Arc<Fetcher> {
        &self.inner.fetcher
    }

    /// External AI provider registry.
    pub fn ai(&self) -> &AiRegistry {
        &self.inner.ai
    }

    /// Extraction engine.
    pub fn extractor(&self) -> &Arc<ExtractionEngine> {
        &self.inner.extractor
    }

    /// Directory holding the database, config and key material.
    pub fn data_dir(&self) -> &Path {
        &self.inner.data_dir
    }

    /// Runs, records and returns a single-page extraction.
    pub async fn extract(&self, request: ExtractRequest) -> Result<ExtractRun> {
        let url = self.inner.fetcher.policy().parse(&request.url)?;
        if request.query.trim().is_empty() {
            return Err(AicrawlError::InvalidInput(
                "query must not be empty; describe what to extract".into(),
            ));
        }
        let job = self
            .inner
            .store
            .create_job(NewJob {
                kind: JobKind::Extract,
                url: url.to_string(),
                query: Some(request.query.clone()),
                output_format: request.output_format.clone(),
                schema_json: request.schema.as_ref().map(|schema| schema.to_string()),
                max_pages: None,
                max_depth: None,
                meta_json: None,
            })
            .await?;
        self.inner.store.start_job(&job.id).await?;

        match self.inner.extractor.extract(&request).await {
            Ok(outcome) => {
                let result = self
                    .inner
                    .store
                    .complete_extract_job(
                        &job.id,
                        job_status::SUCCEEDED,
                        1,
                        None,
                        NewResult {
                            job_id: job.id.clone(),
                            url: url.to_string(),
                            query: Some(request.query.clone()),
                            mode: outcome.mode.clone(),
                            format: outcome.format.clone(),
                            data_json: serde_json::to_string(&outcome.data)?,
                            warnings_json: serde_json::to_string(&outcome.warnings)?,
                            provider: outcome.provider.clone(),
                            validated: outcome.validated,
                            duration_ms: outcome.duration_ms as i64,
                        },
                    )
                    .await?;
                let job = self.inner.store.get_job(&job.id).await?.unwrap_or(job);
                Ok(ExtractRun {
                    job,
                    result,
                    outcome,
                })
            }
            Err(err) => {
                self.inner
                    .store
                    .finish_job(&job.id, job_status::FAILED, 0, Some(&err.to_string()))
                    .await
                    .ok();
                Err(err)
            }
        }
    }

    /// Validates crawl options and creates a queued crawl job.
    pub async fn create_crawl_job(&self, options: &CrawlOptions) -> Result<CrawlJob> {
        let (url, max_pages, max_depth) = options.validate()?;
        self.inner.fetcher.policy().validate_syntax(&url)?;
        self.inner
            .store
            .create_job(NewJob {
                kind: JobKind::Crawl,
                url: url.to_string(),
                query: options.query.clone(),
                output_format: "json".into(),
                schema_json: None,
                max_pages: Some(max_pages),
                max_depth: Some(max_depth),
                meta_json: None,
            })
            .await
    }

    /// Creates a crawl job and starts it in the background.
    pub async fn start_crawl(&self, options: CrawlOptions) -> Result<CrawlJob> {
        let job = self.create_crawl_job(&options).await?;
        let engine = self.clone();
        let job_id = job.id.clone();
        let spawned_options = options.clone();
        tokio::spawn(async move {
            if let Err(error) = engine.run_crawl_job(&job_id, spawned_options, None).await {
                tracing::error!(%error, job_id = %job_id, "crawl job failed");
            }
        });
        Ok(job)
    }

    /// Runs a crawl job to completion. Used by the API background task and CLI.
    pub async fn run_crawl_job(
        &self,
        job_id: &str,
        options: CrawlOptions,
        sink: Option<ProgressSink>,
    ) -> Result<CrawlSummary> {
        self.inner.store.start_job(job_id).await?;
        let token = CancellationToken::new();
        {
            let mut cancels = self
                .inner
                .cancels
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            cancels.insert(job_id.to_string(), token.clone());
        }

        let result = self.inner.crawler.run(job_id, &options, token, sink).await;

        {
            let mut cancels = self
                .inner
                .cancels
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            cancels.remove(job_id);
        }

        match result {
            Ok(summary) => {
                let status = if summary.cancelled {
                    job_status::CANCELLED
                } else {
                    job_status::SUCCEEDED
                };
                self.inner
                    .store
                    .finish_job(job_id, status, summary.pages_fetched, None)
                    .await?;
                Ok(summary)
            }
            Err(err) => {
                let pages = self.inner.store.count_pages(job_id).await.unwrap_or(0) as u32;
                self.inner
                    .store
                    .finish_job(job_id, job_status::FAILED, pages, Some(&err.to_string()))
                    .await
                    .ok();
                Err(err)
            }
        }
    }

    /// Cancels a queued or running job. Returns true when it was active.
    pub async fn cancel_job(&self, job_id: &str) -> Result<bool> {
        let token = {
            let cancels = self
                .inner
                .cancels
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            cancels.get(job_id).cloned()
        };
        if let Some(token) = token {
            token.cancel();
            let _ = self.inner.store.cancel_job(job_id).await;
            return Ok(true);
        }
        self.inner.store.cancel_job(job_id).await
    }

    /// Metadata/structure inspection for one URL.
    pub async fn inspect(&self, input: &str, render: bool) -> Result<InspectReport> {
        let url = self.inner.fetcher.policy().parse(input)?;
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
        let parsed = parse_document(
            &page.content_type,
            &page.body,
            &Url::parse(&page.url).unwrap_or(url),
            self.inner.config.crawler.max_text_chars,
        );
        let json_ld_types = json_ld_types(&parsed);
        Ok(InspectReport {
            requested_url: input.to_string(),
            final_url: page.url.clone(),
            status: page.status,
            content_type: page.content_type.clone(),
            title: parsed.title.clone(),
            description: parsed.description.clone(),
            lang: parsed.lang.clone(),
            headings: parsed.headings.clone(),
            word_count: parsed.word_count,
            text_chars: parsed.text.chars().count(),
            truncated: parsed.truncated,
            links: parsed.links.len(),
            link_samples: parsed
                .links
                .iter()
                .take(25)
                .map(|link| link.url.clone())
                .collect(),
            images: parsed.images.len(),
            tables: parsed.tables.len(),
            json_ld_types,
            prices: parsed.prices.len(),
            from_cache: page.from_cache,
            rendered: page.rendered,
            elapsed_ms: page.elapsed_ms,
            warnings: page.warnings,
        })
    }

    /// Cleaned page text and structure, without AI.
    pub async fn page_content(&self, input: &str) -> Result<PageContent> {
        let url = self.inner.fetcher.policy().parse(input)?;
        let page = self
            .inner
            .fetcher
            .fetch(&url, &FetchOptions::standard())
            .await?;
        let parsed = parse_document(
            &page.content_type,
            &page.body,
            &Url::parse(&page.url).unwrap_or(url),
            self.inner.config.crawler.max_text_chars,
        );
        Ok(PageContent {
            url: input.to_string(),
            final_url: page.url.clone(),
            status: page.status,
            content_type: page.content_type.clone(),
            title: parsed.title.clone(),
            description: parsed.description.clone(),
            text: parsed.text.clone(),
            markdown: parsed.markdown.clone(),
            word_count: parsed.word_count,
            truncated: parsed.truncated,
            links: parsed.links.iter().take(100).cloned().collect(),
        })
    }

    /// Deterministic keyword search within a page.
    pub async fn search(&self, input: &str, query: &str) -> Result<SearchReport> {
        if query.trim().is_empty() {
            return Err(AicrawlError::InvalidInput(
                "search query must not be empty".into(),
            ));
        }
        let content = self.page_content(input).await?;
        let intent = QueryIntent::parse(query);
        let keywords = if intent.keywords.is_empty() {
            vec![query.trim().to_ascii_lowercase()]
        } else {
            intent.keywords
        };

        let mut matches: Vec<SearchHit> = content
            .text
            .lines()
            .filter(|line| line.len() >= 8)
            .filter_map(|line| {
                let lower = line.to_ascii_lowercase();
                let score = keywords
                    .iter()
                    .filter(|keyword| lower.contains(keyword.as_str()))
                    .count();
                (score > 0).then(|| SearchHit {
                    snippet: line.trim().chars().take(400).collect(),
                    score,
                })
            })
            .collect();
        matches.sort_by(|a, b| b.score.cmp(&a.score));
        matches.truncate(20);
        let count = matches.len();
        Ok(SearchReport {
            url: content.final_url,
            query: query.to_string(),
            count,
            matches,
        })
    }

    /// Job, pages and results for a crawl.
    pub async fn crawl_details(&self, job_id: &str) -> Result<CrawlDetails> {
        let job = self
            .inner
            .store
            .get_job(job_id)
            .await?
            .ok_or_else(|| AicrawlError::NotFound(format!("job {job_id}")))?;
        let pages = self.inner.store.list_pages(job_id).await?;
        let results = self
            .inner
            .store
            .results_for_job(job_id)
            .await?
            .iter()
            .map(crate::api::result_json)
            .collect();
        Ok(CrawlDetails {
            job,
            pages,
            results,
        })
    }

    /// Store statistics.
    pub async fn stats(&self) -> Result<StoreStats> {
        self.inner.store.stats().await
    }

    /// AI provider status.
    pub async fn ai_status(&self) -> AiStatus {
        self.inner.ai.status().await
    }

    /// Health snapshot used by the API, CLI and MCP.
    pub async fn health(&self) -> Value {
        let database = match self.inner.store.ping().await {
            Ok(()) => json!({"ok": true, "path": self.inner.store.location(), "error": null}),
            Err(err) => {
                json!({"ok": false, "path": self.inner.store.location(), "error": err.to_string()})
            }
        };
        let db_ok = database.get("ok").and_then(Value::as_bool).unwrap_or(false);
        let stats = self.inner.store.stats().await.unwrap_or_default();
        let cache_entries = self.inner.store.cache_entries().await.unwrap_or(0);
        let ai = self.inner.ai.status().await;
        let security = SecuritySummary {
            allow_private_networks: self.inner.config.security.allow_private_networks,
            allowed_domains: self.inner.config.security.allowed_domains.clone(),
            blocked_domains: self.inner.config.security.blocked_domains.clone(),
            max_redirects: self.inner.config.security.max_redirects,
            rate_limit_per_minute: self.inner.config.security.rate_limit_per_minute,
            respect_robots_txt: self.inner.config.crawler.respect_robots_txt,
        };

        json!({
            "status": if db_ok { "ok" } else { "degraded" },
            "version": crate::VERSION,
            "uptime_secs": self.inner.started_at.elapsed().as_secs(),
            "database": database,
            "stats": stats,
            "cache_entries": cache_entries,
            "ai": ai,
            "security": security,
            "api": {
                "host": self.inner.config.server.api_host,
                "port": self.inner.config.server.api_port
            },
            "mcp": {
                "host": self.inner.config.server.mcp_host,
                "port": self.inner.config.server.mcp_port,
                "endpoint": format!("http://{}:{}/mcp", self.inner.config.server.mcp_host, self.inner.config.server.mcp_port)
            },
            "data_dir": self.inner.data_dir.display().to_string()
        })
    }
}

fn json_ld_types(parsed: &ParsedPage) -> Vec<String> {
    fn walk(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            Value::Object(map) => {
                if let Some(types) = map.get("@type") {
                    match types {
                        Value::String(name) => {
                            if !out.contains(name) {
                                out.push(name.clone());
                            }
                        }
                        Value::Array(names) => {
                            for name in names.iter().filter_map(Value::as_str) {
                                if !out.contains(&name.to_string()) {
                                    out.push(name.to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }
                map.values().for_each(|child| walk(child, out));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for value in &parsed.json_ld {
        walk(value, &mut out);
    }
    out
}

/// Builds a summary page object for callers that already parsed a page.
pub fn page_summary(
    final_url: &str,
    status: u16,
    content_type: &str,
    parsed: &ParsedPage,
    bytes: usize,
    truncated: bool,
) -> PageSummary {
    PageSummary {
        final_url: final_url.to_string(),
        status,
        content_type: content_type.to_string(),
        title: parsed.title.clone(),
        description: parsed.description.clone(),
        word_count: parsed.word_count,
        bytes,
        truncated,
        from_cache: false,
        rendered: false,
    }
}

//! Bounded, breadth-first crawler.
//!
//! Every fetch goes through the shared [`Fetcher`], so SSRF protection,
//! robots.txt, rate limiting, redirect validation and caching apply identically
//! to CLI, API and MCP crawls. Depth, page count and wall-clock budget are
//! enforced, and cancellation is checked between batches.

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::stream;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::config::CrawlerConfig;
use crate::error::{AicrawlError, Result};
use crate::extractor::{is_crawlable, parse_document, ExtractRequest, ExtractionEngine};
use crate::fetcher::{FetchOptions, Fetcher};
use crate::storage::{job_status, NewPage, NewResult, SqliteStore};
use crate::util::{sha256_hex, truncate_chars};

/// Crawl parameters supplied by a caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlOptions {
    pub url: String,
    #[serde(default = "default_max_pages")]
    pub max_pages: u32,
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    /// Optional natural-language extraction applied to every crawled page.
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub render: bool,
    /// Restrict discovered links to the starting host.
    #[serde(default = "crate::crawler::default_true")]
    pub same_domain: bool,
}

fn default_max_pages() -> u32 {
    10
}

fn default_max_depth() -> u32 {
    2
}

pub(crate) fn default_true() -> bool {
    true
}

impl CrawlOptions {
    /// Options for a new crawl of `url`.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            max_pages: default_max_pages(),
            max_depth: default_max_depth(),
            query: None,
            render: false,
            same_domain: true,
        }
    }

    /// Validates and clamps the limits. Hard caps protect both the target and this process.
    pub fn validate(&self) -> Result<(Url, u32, u32)> {
        let url = Url::parse(self.url.trim())
            .map_err(|e| AicrawlError::InvalidInput(format!("invalid crawl URL: {e}")))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AicrawlError::Blocked(
                "crawl URLs must use http or https".into(),
            ));
        }
        if self.max_pages == 0 {
            return Err(AicrawlError::InvalidInput(
                "max_pages must be at least 1".into(),
            ));
        }
        let max_pages = self.max_pages.min(500);
        let max_depth = self.max_depth.min(5);
        Ok((url, max_pages, max_depth))
    }
}

/// Progress event emitted after each page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlProgress {
    pub pages_fetched: u32,
    pub max_pages: u32,
    pub queued: usize,
    pub url: String,
    pub depth: u32,
    pub ok: bool,
}

/// Callback used by the CLI to render live progress.
pub type ProgressSink = Arc<dyn Fn(CrawlProgress) + Send + Sync>;

/// Outcome of a crawl.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CrawlSummary {
    pub pages_fetched: u32,
    pub pages_succeeded: u32,
    pub errors: u32,
    pub extraction_errors: u32,
    pub results_saved: u32,
    pub cancelled: bool,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub warnings: Vec<String>,
}

/// Runs bounded crawls using the shared fetcher and store.
pub struct Crawler {
    fetcher: Arc<Fetcher>,
    store: SqliteStore,
    extractor: Arc<ExtractionEngine>,
    config: CrawlerConfig,
}

impl Crawler {
    /// Creates a crawler.
    pub fn new(
        fetcher: Arc<Fetcher>,
        store: SqliteStore,
        extractor: Arc<ExtractionEngine>,
        config: CrawlerConfig,
    ) -> Self {
        Self {
            fetcher,
            store,
            extractor,
            config,
        }
    }

    /// Crawls the target, recording pages and (optionally) per-page extractions.
    pub async fn run(
        &self,
        job_id: &str,
        options: &CrawlOptions,
        cancel: CancellationToken,
        sink: Option<ProgressSink>,
    ) -> Result<CrawlSummary> {
        let (start, max_pages, max_depth) = options.validate()?;
        self.fetcher.validate(&start).await?;
        let started = Instant::now();

        let deadline = Instant::now() + Duration::from_secs(self.config.max_crawl_secs);
        let start_host = host_key(&start);
        let mut frontier: VecDeque<(Url, u32)> = VecDeque::new();
        frontier.push_back((start, 0));
        let mut visited: HashSet<String> = HashSet::new();
        if let Some((url, _)) = frontier.front() {
            visited.insert(normalize(url));
        }

        let mut summary = CrawlSummary::default();
        let concurrency = self.config.concurrency.clamp(1, 16);

        while !frontier.is_empty() && summary.pages_fetched < max_pages {
            if cancel.is_cancelled() {
                summary.cancelled = true;
                break;
            }
            if Instant::now() >= deadline {
                summary.timed_out = true;
                summary.warnings.push(format!(
                    "crawl stopped after the {}s wall-clock budget",
                    self.config.max_crawl_secs
                ));
                break;
            }

            let remaining = (max_pages - summary.pages_fetched) as usize;
            let batch_size = concurrency.min(remaining).min(frontier.len());
            let mut batch: Vec<(Url, u32)> = Vec::with_capacity(batch_size);
            while batch.len() < batch_size {
                match frontier.pop_front() {
                    Some(item) => batch.push(item),
                    None => break,
                }
            }

            let fetcher = self.fetcher.clone();
            let render = options.render;
            let futures = batch.into_iter().map(|(url, depth)| {
                let fetcher = fetcher.clone();
                async move {
                    let fetched = fetcher
                        .fetch(
                            &url,
                            &FetchOptions {
                                render,
                                use_cache: true,
                                ..Default::default()
                            },
                        )
                        .await;
                    (url, depth, fetched)
                }
            });
            let results: Vec<_> = stream::iter(futures)
                .buffer_unordered(concurrency)
                .collect()
                .await;

            for (requested, depth, fetched) in results {
                summary.pages_fetched += 1;
                let _ = self.store.increment_job_pages(job_id, 1).await;

                match fetched {
                    Ok(page) => {
                        summary.pages_succeeded += 1;
                        let final_url = Url::parse(&page.url).unwrap_or_else(|_| requested.clone());
                        let parsed = parse_document(
                            &page.content_type,
                            &page.body,
                            &final_url,
                            self.config.max_text_chars,
                        );
                        let _ = self
                            .store
                            .insert_page(NewPage {
                                job_id: job_id.to_string(),
                                url: final_url.to_string(),
                                depth,
                                status_code: Some(page.status),
                                title: parsed.title.clone(),
                                content_type: Some(page.content_type.clone()),
                                text_length: parsed.text.len(),
                                content_hash: Some(sha256_hex(page.body.as_bytes())),
                                word_count: parsed.word_count,
                                error: None,
                            })
                            .await;
                        let _ = self
                            .store
                            .touch_website(
                                &host_key(&final_url),
                                final_url.scheme(),
                                Some(page.status),
                                Some(true),
                            )
                            .await;

                        if let Some(query) = options.query.as_deref() {
                            if !query.trim().is_empty() {
                                let request = ExtractRequest {
                                    url: final_url.to_string(),
                                    query: query.to_string(),
                                    schema: None,
                                    output_format: "json".into(),
                                    use_ai: true,
                                    render: false,
                                    use_cache: true,
                                };
                                let summary_page = crate::extractor::PageSummary {
                                    final_url: final_url.to_string(),
                                    status: page.status,
                                    content_type: page.content_type.clone(),
                                    title: parsed.title.clone(),
                                    description: parsed.description.clone(),
                                    word_count: parsed.word_count,
                                    bytes: page.bytes,
                                    truncated: page.truncated,
                                    from_cache: page.from_cache,
                                    rendered: page.rendered,
                                };
                                let warnings = page.warnings.clone();
                                match self
                                    .extractor
                                    .extract_document(
                                        &final_url,
                                        &page.content_type,
                                        &page.body,
                                        &request,
                                        summary_page,
                                        warnings,
                                    )
                                    .await
                                {
                                    Ok(outcome) => {
                                        let result = NewResult {
                                            job_id: job_id.to_string(),
                                            url: final_url.to_string(),
                                            query: Some(query.to_string()),
                                            mode: outcome.mode.clone(),
                                            format: "json".into(),
                                            data_json: serde_json::to_string(&outcome.data)
                                                .unwrap_or_else(|_| "null".into()),
                                            warnings_json: serde_json::to_string(&outcome.warnings)
                                                .unwrap_or_else(|_| "[]".into()),
                                            provider: outcome.provider.clone(),
                                            validated: outcome.validated,
                                            duration_ms: outcome.duration_ms as i64,
                                        };
                                        if self.store.insert_result(result).await.is_ok() {
                                            summary.results_saved += 1;
                                        }
                                    }
                                    Err(err) => {
                                        summary.extraction_errors += 1;
                                        summary.warnings.push(format!(
                                            "extraction failed for {}: {err}",
                                            final_url
                                        ));
                                    }
                                }
                            }
                        }

                        if depth < max_depth {
                            for link in parsed.links.iter().filter(|link| is_crawlable(link)) {
                                let Ok(next) = Url::parse(&link.url) else {
                                    continue;
                                };
                                if options.same_domain
                                    && !same_host(&start_host, &host_key(&next))
                                {
                                    continue;
                                }
                                let normalized = normalize(&next);
                                if visited.insert(normalized) {
                                    frontier.push_back((next, depth + 1));
                                }
                            }
                        }

                        if let Some(sink) = sink.as_ref() {
                            sink(CrawlProgress {
                                pages_fetched: summary.pages_fetched,
                                max_pages,
                                queued: frontier.len(),
                                url: final_url.to_string(),
                                depth,
                                ok: true,
                            });
                        }
                    }
                    Err(err) => {
                        summary.errors += 1;
                        let message = err.to_string();
                        let _ = self
                            .store
                            .insert_page(NewPage {
                                job_id: job_id.to_string(),
                                url: requested.to_string(),
                                depth,
                                status_code: None,
                                title: None,
                                content_type: None,
                                text_length: 0,
                                content_hash: None,
                                word_count: 0,
                                error: Some(truncate_chars(&message, 500).0),
                            })
                            .await;
                        if let Some(sink) = sink.as_ref() {
                            sink(CrawlProgress {
                                pages_fetched: summary.pages_fetched,
                                max_pages,
                                queued: frontier.len(),
                                url: requested.to_string(),
                                depth,
                                ok: false,
                            });
                        }
                    }
                }
            }
        }

        summary.duration_ms = started.elapsed().as_millis() as u64;
        Ok(summary)
    }
}

fn host_key(url: &Url) -> String {
    url.host_str()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_ascii_lowercase()
}

fn same_host(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn normalize(url: &Url) -> String {
    let mut copy = url.clone();
    copy.set_fragment(None);
    // `index.html` is equivalent to the directory URL for crawl de-duplication,
    // and a trailing slash is not significant.
    let mut path = copy.path().to_string();
    if let Some(prefix) = path.strip_suffix("index.html") {
        path = prefix.to_string();
    }
    if path.len() > 1 {
        path = path.trim_end_matches('/').to_string();
    }
    if path.is_empty() {
        path.push('/');
    }
    copy.set_path(&path);
    copy.to_string()
}

/// Status string for a crawl summary.
pub fn summary_status(summary: &CrawlSummary) -> &'static str {
    if summary.cancelled {
        job_status::CANCELLED
    } else {
        job_status::SUCCEEDED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_clamp_limits() {
        let mut options = CrawlOptions::new("https://example.com/");
        options.max_pages = 10_000;
        options.max_depth = 99;
        let (_, pages, depth) = options.validate().unwrap();
        assert_eq!(pages, 500);
        assert_eq!(depth, 5);
    }

    #[test]
    fn rejects_non_http_and_empty_pages() {
        assert!(CrawlOptions::new("ftp://example.com").validate().is_err());
        let mut options = CrawlOptions::new("https://example.com");
        options.max_pages = 0;
        assert!(options.validate().is_err());
        assert!(CrawlOptions::new("not a url").validate().is_err());
    }

    #[test]
    fn urls_normalize_for_dedup() {
        let a = Url::parse("https://example.com/a/index.html#top").unwrap();
        let b = Url::parse("https://example.com/a").unwrap();
        assert_eq!(normalize(&a), normalize(&b));
    }

    #[test]
    fn www_is_ignored_for_same_domain() {
        assert_eq!(host_key(&Url::parse("https://www.Example.com/").unwrap()), "example.com");
    }
}

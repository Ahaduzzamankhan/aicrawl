//! Bounded breadth-first crawl.
//!
//! Every fetch goes through the shared [`Fetcher`], so SSRF protection,
//! robots.txt, rate limits, redirect validation and caching apply identically
//! to CLI, REST and MCP crawls. Depth, page count and wall-clock budget are
//! enforced, and cancellation is checked between pages.

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::config::ExtractConfig;
use crate::error::{AicrawlError, Result};
use crate::fetcher::Fetcher;
use crate::map;
use crate::parser::PageLink;
use crate::pipeline::{self, PageOutput};
use crate::request::Request;
use crate::storage::SqliteStore;

/// Aggregate result of a crawl.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CrawlSummary {
    pub pages_fetched: u32,
    pub pages_succeeded: u32,
    pub errors: u32,
    pub changed_pages: u32,
    pub results_saved: u32,
    pub cancelled: bool,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub warnings: Vec<String>,
}

/// Crawl result: per-page projections plus the discovered frontier.
#[derive(Debug, Clone)]
pub struct CrawlOutcome {
    pub pages: Vec<PageOutput>,
    pub discovered: Vec<String>,
    pub summary: CrawlSummary,
}

/// Runs a bounded crawl from `start`, projecting every page through `request`.
pub async fn crawl_site(
    fetcher: Arc<Fetcher>,
    config: &ExtractConfig,
    max_crawl_secs: u64,
    store: Option<&SqliteStore>,
    request: &Request,
    start: &Url,
    cancel: Option<CancellationToken>,
) -> Result<CrawlOutcome> {
    let spec = request.crawl_spec();
    let (max_pages, max_depth) = spec.limits();
    let started = Instant::now();
    let deadline = started + Duration::from_secs(max_crawl_secs.max(1));
    let start_host = host_key(start);

    let mut summary = CrawlSummary::default();
    let mut frontier: VecDeque<(Url, u32)> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut discovered: Vec<String> = Vec::new();
    let mut pages: Vec<PageOutput> = Vec::new();

    frontier.push_back((start.clone(), 0));
    visited.insert(normalize(start));

    if spec.sitemap {
        match map::sitemap_urls(&fetcher, start, max_pages as usize * 4).await {
            Ok(urls) => {
                for raw in urls {
                    let Ok(url) = Url::parse(&raw) else { continue };
                    if spec.same_domain && host_key(&url) != start_host {
                        continue;
                    }
                    if visited.insert(normalize(&url)) {
                        frontier.push_back((url, 1));
                    }
                }
            }
            Err(err) => summary
                .warnings
                .push(format!("sitemap discovery failed: {err}")),
        }
    }

    while !frontier.is_empty() && pages.len() < max_pages as usize {
        if cancel
            .as_ref()
            .map(|token| token.is_cancelled())
            .unwrap_or(false)
        {
            summary.cancelled = true;
            break;
        }
        if Instant::now() >= deadline {
            summary.timed_out = true;
            summary.warnings.push(format!(
                "crawl stopped after the {max_crawl_secs}s wall-clock budget"
            ));
            break;
        }

        let (url, depth) = frontier.pop_front().expect("frontier is not empty");
        discovered.push(url.to_string());

        let previous_hash = match store {
            Some(store) if spec.incremental => store
                .previous_content_hash(&url.to_string())
                .await
                .unwrap_or(None),
            _ => None,
        };

        let (page, _) =
            match pipeline::run_page(&fetcher, config, request, &url, previous_hash).await {
                Ok(result) => result,
                Err(err) => {
                    summary.errors += 1;
                    summary.warnings.push(format!("{url}: {err}"));
                    pages.push(error_page(&url, &err));
                    continue;
                }
            };

        summary.pages_fetched += 1;
        summary.pages_succeeded += 1;
        if page.changed {
            summary.changed_pages += 1;
        }
        if depth < max_depth {
            for link in page.links.iter().filter(|link| is_crawlable(link)) {
                let Ok(next) = Url::parse(&link.url) else {
                    continue;
                };
                if spec.same_domain && host_key(&next) != start_host {
                    continue;
                }
                if visited.insert(normalize(&next)) {
                    frontier.push_back((next, depth + 1));
                }
            }
        }

        pages.push(page);
    }

    summary.duration_ms = started.elapsed().as_millis() as u64;
    if summary.errors > 0 && summary.pages_succeeded == 0 && !summary.cancelled {
        return Err(AicrawlError::Network(format!(
            "crawl failed for every page; see warnings: {}",
            summary.warnings.join("; ")
        )));
    }
    Ok(CrawlOutcome {
        pages,
        discovered,
        summary,
    })
}

fn error_page(url: &Url, err: &AicrawlError) -> PageOutput {
    PageOutput {
        requested_url: url.to_string(),
        final_url: url.to_string(),
        status: 0,
        content_type: String::new(),
        render: "http".into(),
        cached: false,
        truncated: false,
        data: serde_json::Value::Null,
        issues: Vec::new(),
        warnings: vec![err.to_string()],
        content_hash: String::new(),
        changed: false,
        items: 0,
        title: None,
        word_count: 0,
        elapsed_ms: 0,
        projection: "rules".into(),
        links: Vec::new(),
    }
}

/// Link filter shared by crawl and map: skips binaries and non-HTTP schemes.
pub fn is_crawlable(link: &PageLink) -> bool {
    const SKIP_EXTENSIONS: &[&str] = &[
        ".pdf", ".zip", ".gz", ".tar", ".rar", ".7z", ".png", ".jpg", ".jpeg", ".gif", ".webp",
        ".svg", ".ico", ".mp3", ".mp4", ".avi", ".mov", ".wav", ".webm", ".woff", ".woff2", ".ttf",
        ".otf", ".eot", ".css", ".js", ".json", ".xml", ".rss", ".doc", ".docx", ".xls", ".xlsx",
        ".ppt", ".pptx", ".exe", ".dmg", ".iso",
    ];
    let Ok(url) = Url::parse(&link.url) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    let path = url.path().to_ascii_lowercase();
    !SKIP_EXTENSIONS
        .iter()
        .any(|extension| path.ends_with(extension))
}

/// Host key used for same-domain checks (www is not significant).
pub fn host_key(url: &Url) -> String {
    url.host_str()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_ascii_lowercase()
}

/// Canonical form used for crawl de-duplication.
pub fn normalize(url: &Url) -> String {
    let mut copy = url.clone();
    copy.set_fragment(None);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn link(url: &str) -> PageLink {
        PageLink {
            url: url.into(),
            text: String::new(),
            rel: None,
        }
    }

    #[test]
    fn skips_asset_links() {
        assert!(is_crawlable(&link("https://example.com/about")));
        assert!(!is_crawlable(&link("https://example.com/manual.pdf")));
        assert!(!is_crawlable(&link("https://example.com/logo.png")));
        assert!(!is_crawlable(&link("mailto:a@example.com")));
    }

    #[test]
    fn normalizes_for_dedup() {
        let a = Url::parse("https://example.com/a/index.html#top").unwrap();
        let b = Url::parse("https://example.com/a").unwrap();
        assert_eq!(normalize(&a), normalize(&b));
    }

    #[test]
    fn host_keys_ignore_www() {
        assert_eq!(
            host_key(&Url::parse("https://www.Example.com/").unwrap()),
            "example.com"
        );
    }
}

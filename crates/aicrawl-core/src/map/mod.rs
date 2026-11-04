//! Site mapping: internal links, pagination-agnostic discovery and sitemap.xml.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use regex::Regex;
use url::Url;

use crate::crawl::{host_key, is_crawlable, normalize};
use crate::error::{AicrawlError, Result};
use crate::fetcher::{FetchOptions, Fetcher};
use crate::parser::{parse_html, parse_plain_text};

/// Result of `aicrawl map`.
#[derive(Debug, Clone, Default)]
pub struct MapOutcome {
    pub urls: Vec<String>,
    pub pages_visited: u32,
    pub sitemap_urls: usize,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

/// Discovers URLs reachable from `start` without extracting data.
pub async fn map_site(
    fetcher: &Arc<Fetcher>,
    start: &Url,
    max_pages: u32,
    max_depth: u32,
    include_sitemap: bool,
    max_secs: u64,
) -> Result<MapOutcome> {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(max_secs.max(1));
    let max_pages = max_pages.clamp(1, 500);
    let max_depth = max_depth.min(5);
    let start_host = host_key(start);

    let mut outcome = MapOutcome::default();
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: Vec<(Url, u32)> = vec![(start.clone(), 0)];
    seen.insert(normalize(start));
    outcome.urls.push(start.to_string());

    if include_sitemap {
        match sitemap_urls(fetcher, start, (max_pages as usize) * 8).await {
            Ok(urls) => {
                outcome.sitemap_urls = urls.len();
                for raw in urls {
                    let Ok(url) = Url::parse(&raw) else { continue };
                    if host_key(&url) != start_host {
                        continue;
                    }
                    if seen.insert(normalize(&url)) {
                        outcome.urls.push(url.to_string());
                        queue.push((url, 1));
                    }
                }
            }
            Err(err) => outcome
                .warnings
                .push(format!("sitemap discovery failed: {err}")),
        }
    }

    while let Some((url, depth)) = queue.pop() {
        if outcome.pages_visited >= max_pages {
            outcome.truncated = true;
            break;
        }
        if Instant::now() >= deadline {
            outcome.truncated = true;
            outcome
                .warnings
                .push(format!("map stopped after the {max_secs}s budget"));
            break;
        }
        let fetched = match fetcher
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
                outcome.warnings.push(format!("{url}: {err}"));
                continue;
            }
        };
        outcome.pages_visited += 1;
        let final_url = Url::parse(&fetched.url).unwrap_or_else(|_| url.clone());
        let parsed =
            if fetched.content_type.contains("html") || fetched.content_type.contains("xml") {
                parse_html(&final_url, &fetched.body, 60_000)
            } else {
                parse_plain_text(&final_url, &fetched.body, 60_000)
            };

        if depth >= max_depth {
            continue;
        }
        for link in parsed.links.iter().filter(|link| is_crawlable(link)) {
            let Ok(next) = Url::parse(&link.url) else {
                continue;
            };
            if host_key(&next) != start_host {
                continue;
            }
            if seen.insert(normalize(&next)) {
                outcome.urls.push(next.to_string());
                queue.push((next, depth + 1));
                if outcome.urls.len() >= max_pages as usize * 20 {
                    outcome.truncated = true;
                    break;
                }
            }
        }
    }

    outcome.urls.sort();
    outcome.urls.dedup();
    Ok(outcome)
}

/// Reads `/sitemap.xml` (and one level of sitemap indexes) for `start`'s origin.
pub async fn sitemap_urls(
    fetcher: &Arc<Fetcher>,
    start: &Url,
    limit: usize,
) -> Result<Vec<String>> {
    let origin = start.origin().ascii_serialization();
    let mut candidates: Vec<String> = Vec::new();
    for path in ["/sitemap.xml", "/sitemap_index.xml", "/sitemap-index.xml"] {
        let Ok(url) = Url::parse(&format!("{origin}{path}")) else {
            continue;
        };
        match fetch_text(fetcher, &url).await {
            Ok(body) => {
                let (locs, nested) = parse_sitemap(&body);
                candidates.extend(locs);
                if candidates.len() >= limit {
                    break;
                }
                for child in nested.into_iter().take(5) {
                    if let Ok(child_url) = Url::parse(&child) {
                        if let Ok(body) = fetch_text(fetcher, &child_url).await {
                            candidates.extend(parse_sitemap(&body).0);
                        }
                    }
                }
            }
            Err(_) => continue,
        }
        if !candidates.is_empty() {
            break;
        }
    }

    let host = host_key(start);
    let mut urls: Vec<String> = candidates
        .into_iter()
        .filter(|raw| {
            Url::parse(raw)
                .map(|url| host_key(&url) == host)
                .unwrap_or(false)
        })
        .collect();
    urls.sort();
    urls.dedup();
    urls.truncate(limit.max(1));
    if urls.is_empty() {
        return Err(AicrawlError::NotFound(format!(
            "no sitemap.xml found at {origin}"
        )));
    }
    Ok(urls)
}

async fn fetch_text(fetcher: &Arc<Fetcher>, url: &Url) -> Result<String> {
    let page = fetcher
        .fetch(
            url,
            &FetchOptions {
                render: false,
                use_cache: true,
                max_body_bytes: Some(4 * 1024 * 1024),
                ..Default::default()
            },
        )
        .await?;
    Ok(page.body)
}

/// Extracts `<loc>` values and nested sitemap references from a sitemap body.
pub fn parse_sitemap(body: &str) -> (Vec<String>, Vec<String>) {
    use std::sync::OnceLock;
    static LOC: OnceLock<Regex> = OnceLock::new();
    let loc = LOC.get_or_init(|| Regex::new(r"(?is)<loc>\s*([^<\s]+)\s*</loc>").expect("regex"));
    let mut urls: Vec<String> = Vec::new();
    let mut nested: Vec<String> = Vec::new();
    for captures in loc.captures_iter(body) {
        let Some(value) = captures.get(1) else {
            continue;
        };
        let raw = value.as_str().trim().replace("&amp;", "&");
        if Url::parse(&raw).is_err() {
            continue;
        }
        let lower = raw.to_ascii_lowercase();
        if lower.ends_with(".xml") && !lower.contains("/sitemap.xml?") {
            if !nested.contains(&raw) {
                nested.push(raw);
            }
        } else if !urls.contains(&raw) {
            urls.push(raw);
        }
    }
    (urls, nested)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sitemaps_and_indexes() {
        let body = r#"<?xml version="1.0"?>
          <urlset>
            <url><loc>https://example.com/a</loc></url>
            <url><loc>https://example.com/b?x=1&amp;y=2</loc></url>
          </urlset>"#;
        let (urls, nested) = parse_sitemap(body);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[1], "https://example.com/b?x=1&y=2");
        assert!(nested.is_empty());

        let index = r#"<sitemapindex><sitemap><loc>https://example.com/sitemap-1.xml</loc></sitemap></sitemapindex>"#;
        let (urls, nested) = parse_sitemap(index);
        assert!(urls.is_empty());
        assert_eq!(nested, vec!["https://example.com/sitemap-1.xml"]);
    }
}

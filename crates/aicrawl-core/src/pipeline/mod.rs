//! Fetch → parse → select → extract → validate pipeline.
//!
//! One [`PageOutput`] is produced per fetched page. Pagination chains several
//! pages into a single payload; the engine adds storage, jobs and envelopes.

use std::time::Instant;

use scraper::ElementRef;
use serde_json::{json, Map, Value};
use url::Url;

use crate::config::ExtractConfig;
use crate::dedup;
use crate::error::{AicrawlError, Result};
use crate::extraction::{self, RuleContext};
use crate::fetcher::{FetchOptions, FetchedPage, Fetcher};
use crate::output::{Issue, PageMeta};
use crate::parser::{parse_html, parse_plain_text, PageLink, ParsedPage};
use crate::request::{Dedupe, RenderMode, Request};
use crate::safety;
use crate::util::sha256_hex;

/// Result of projecting one page through the request rules.
#[derive(Debug, Clone)]
pub struct PageOutput {
    pub requested_url: String,
    pub final_url: String,
    pub status: u16,
    pub content_type: String,
    pub render: String,
    pub cached: bool,
    pub truncated: bool,
    pub data: Value,
    pub issues: Vec<Issue>,
    pub warnings: Vec<String>,
    pub content_hash: String,
    pub changed: bool,
    pub items: usize,
    pub title: Option<String>,
    pub word_count: usize,
    pub elapsed_ms: u64,
    pub projection: String,
    /// Links discovered on the page (used by crawl and map).
    pub links: Vec<PageLink>,
}

impl PageOutput {
    /// Metadata for the response envelope.
    pub fn to_meta(&self) -> PageMeta {
        PageMeta {
            url: self.requested_url.clone(),
            final_url: Some(self.final_url.clone()),
            status: Some(self.status),
            items_extracted: self.items,
            changed: self.changed,
            error: None,
        }
    }
}

/// Fetches and projects a single page. The optional second value is the next
/// page URL when pagination is enabled and a next link exists.
pub async fn run_page(
    fetcher: &Fetcher,
    config: &ExtractConfig,
    request: &Request,
    url: &Url,
    previous_hash: Option<String>,
) -> Result<(PageOutput, Option<Url>)> {
    let started = Instant::now();
    let mut warnings: Vec<String> = Vec::new();

    let mut page = fetcher
        .fetch(
            url,
            &FetchOptions {
                render: request.render == RenderMode::On,
                use_cache: request.use_cache,
                ..Default::default()
            },
        )
        .await?;
    let mut rendered = page.rendered;

    if request.render == RenderMode::Auto && !rendered {
        let probe = parse_page(&page, config);
        if looks_javascript_driven(&page.body, &probe) {
            let status = fetcher.renderer().status().await;
            if status.available {
                match fetcher
                    .fetch(
                        url,
                        &FetchOptions {
                            render: true,
                            use_cache: false,
                            ..Default::default()
                        },
                    )
                    .await
                {
                    Ok(rendered_page) => {
                        warnings.push(
                            "JavaScript-rendered content detected; the page was rendered with Playwright"
                                .into(),
                        );
                        rendered = true;
                        page = rendered_page;
                    }
                    Err(err) => warnings.push(format!(
                        "automatic rendering failed ({err}); returned the raw HTTP response"
                    )),
                }
            } else {
                warnings.push(
                    "page appears to require JavaScript and the Playwright bridge is unavailable; returned the raw HTTP response"
                        .into(),
                );
            }
        }
    }

    let parsed = parse_page(&page, config);
    for warning in safety::scan_injection(&parsed.text) {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }
    for warning in &page.warnings {
        if !warnings.contains(warning) {
            warnings.push(warning.clone());
        }
    }

    let base = Url::parse(&page.url).unwrap_or_else(|_| url.clone());
    let (data, issues, projection, items) = project(&page, &parsed, request, &base, config)?;

    if let Some(schema) = request.schema.as_ref() {
        if let Some(errors) = extraction::validate_schema(schema, &data) {
            return Err(AicrawlError::Schema(format!(
                "extracted data does not match the requested schema: {}",
                errors.join("; ")
            )));
        }
    }

    let next_url = request
        .clamped_pagination()
        .filter(|spec| spec.enabled)
        .and_then(|spec| next_page_url(&page, &base, &spec.next_selector));

    let content_hash = sha256_hex(page.body.as_bytes());
    let changed = previous_hash
        .map(|previous| previous != content_hash)
        .unwrap_or(true);

    let output = PageOutput {
        requested_url: url.to_string(),
        final_url: page.url.clone(),
        status: page.status,
        content_type: page.content_type.clone(),
        render: if rendered { "playwright" } else { "http" }.to_string(),
        cached: page.from_cache,
        truncated: page.truncated,
        data,
        issues,
        warnings,
        content_hash,
        changed,
        items,
        title: parsed.title.clone(),
        word_count: parsed.word_count,
        elapsed_ms: started.elapsed().as_millis() as u64,
        projection,
        links: parsed.links.clone(),
    };
    Ok((output, next_url))
}

fn project(
    page: &FetchedPage,
    parsed: &ParsedPage,
    request: &Request,
    base: &Url,
    config: &ExtractConfig,
) -> Result<(Value, Vec<Issue>, String, usize)> {
    let mime = page
        .content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();

    let document_mode = request.selectors.is_empty();
    let mut data = Map::new();
    let mut issues = Vec::new();
    let projection;

    if mime.contains("html") || mime.contains("xml") {
        let document = crate::selectors::parse_html_document(&page.body);
        let root = document.root_element();
        if document_mode {
            data.insert("document".into(), extraction::document_summary(parsed));
            projection = "document";
        } else {
            let ctx = RuleContext {
                base,
                document: root,
                max_items: config.max_items,
            };
            let (mut projected, mut rule_issues) =
                extraction::apply_rules(&root, &request.selectors, &ctx);
            issues.append(&mut rule_issues);
            data.append(&mut projected);
            projection = "rules";
        }
    } else if mime.contains("json") {
        let payload: Value = serde_json::from_str(&page.body).unwrap_or(Value::Null);
        if document_mode {
            data.insert(
                "document".into(),
                json!({
                    "url": page.url,
                    "json": payload,
                    "content_type": page.content_type,
                }),
            );
            projection = "document";
        } else {
            data.insert("json".into(), payload);
            projection = "rules";
        }
    } else if document_mode {
        data.insert("document".into(), extraction::document_summary(parsed));
        projection = "document";
    } else {
        data.insert("text".into(), Value::String(parsed.text.clone()));
        projection = "rules";
    }

    if request.include_document && projection != "document" {
        data.insert("_document".into(), extraction::document_summary(parsed));
    }

    let value = Value::Object(data);
    let items = crate::output::count_items(&value);
    Ok((value, issues, projection.to_string(), items))
}

/// Runs pagination and merges every page into one payload.
pub async fn run_request_pages(
    fetcher: &Fetcher,
    config: &ExtractConfig,
    request: &Request,
    start: &Url,
) -> Result<Vec<PageOutput>> {
    let (first, next) = run_page(fetcher, config, request, start, None).await?;
    let mut pages = vec![first];
    let mut cursor = next;

    let pagination = request.clamped_pagination();
    let Some(spec) = pagination.filter(|spec| spec.enabled) else {
        return Ok(pages);
    };

    let mut visited: Vec<String> = vec![start.to_string()];
    while (pages.len() as u32) < spec.max_pages {
        let Some(next) = cursor.clone() else { break };
        if visited.iter().any(|url| url == &next.to_string()) {
            if let Some(last) = pages.last_mut() {
                last.warnings
                    .push("pagination stopped: the next link revisited an earlier page".into());
            }
            break;
        }
        if spec.same_origin && next.origin() != start.origin() {
            if let Some(last) = pages.last_mut() {
                last.warnings.push(format!(
                    "pagination stopped: next link points to a different origin ({})",
                    next.origin().ascii_serialization()
                ));
            }
            break;
        }
        match run_page(fetcher, config, request, &next, None).await {
            Ok((page, following)) => {
                visited.push(next.to_string());
                pages.push(page);
                cursor = following;
            }
            Err(err) => {
                if let Some(last) = pages.last_mut() {
                    last.warnings
                        .push(format!("pagination stopped at {next}: {err}"));
                }
                break;
            }
        }
    }

    Ok(pages)
}

fn next_page_url(page: &FetchedPage, base: &Url, selector: &str) -> Option<Url> {
    let selector = selector.trim();
    if selector.is_empty() {
        return None;
    }
    let document = crate::selectors::parse_html_document(&page.body);
    let root: ElementRef<'_> = document.root_element();
    let href = if selector.starts_with('/') || selector.starts_with('.') {
        crate::selectors::select_xpath(&root, selector)
            .ok()?
            .into_iter()
            .find_map(|node| match node.element() {
                Some(element) => crate::selectors::attribute_of(&element, "href"),
                None => node.literal().map(str::to_string),
            })
    } else {
        crate::selectors::select_css(&root, selector)
            .ok()?
            .into_iter()
            .find_map(|node| {
                node.element()
                    .and_then(|element| crate::selectors::attribute_of(&element, "href"))
            })
    }?;
    let absolute = crate::transform::absolutize(&href, base);
    Url::parse(&absolute).ok()
}

/// Merges paginated outputs, applying de-duplication when configured.
pub fn merge_pages(pages: &[PageOutput], dedupe: Option<&Dedupe>) -> (Value, Vec<Issue>) {
    let mut merged = Map::new();
    let mut issues: Vec<Issue> = Vec::new();

    for (index, page) in pages.iter().enumerate() {
        issues.append(&mut page.issues.clone());
        let Value::Object(map) = &page.data else {
            continue;
        };
        for (key, value) in map {
            if key == "_document" && index > 0 {
                continue;
            }
            match merged.get_mut(key) {
                None => {
                    merged.insert(key.clone(), value.clone());
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

    if let Some(spec) = dedupe {
        for value in merged.values_mut() {
            if let Value::Array(items) = value {
                dedup::dedupe(items, &spec.keys);
            }
        }
    }
    (Value::Object(merged), issues)
}

fn parse_page(page: &FetchedPage, config: &ExtractConfig) -> ParsedPage {
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
    if mime.contains("html") || mime.contains("xml") || mime.is_empty() {
        parse_html(&url, &page.body, config.max_text_chars)
    } else {
        parse_plain_text(&url, &page.body, config.max_text_chars)
    }
}

/// Heuristics for pages whose meaningful content is produced by JavaScript.
pub fn looks_javascript_driven(body: &str, parsed: &ParsedPage) -> bool {
    if parsed.word_count >= 60 {
        return false;
    }
    let lower = body.to_ascii_lowercase();
    let has_app_root = lower.contains("id=\"root\"")
        || lower.contains("id='root'")
        || lower.contains("id=\"app\"")
        || lower.contains("id='app'")
        || lower.contains("__next_data__")
        || lower.contains("window.__nuxt__")
        || lower.contains("ng-version")
        || lower.contains("data-reactroot");
    let asks_for_js = lower.contains("enable javascript")
        || lower.contains("javascript is required")
        || lower.contains("javascript to continue");
    let script_count = lower.matches("<script").count();
    (has_app_root && script_count >= 1) || asks_for_js
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_html;

    #[test]
    fn detects_javascript_shells() {
        let url = Url::parse("https://example.com").unwrap();
        let parsed = parse_html(&url, "<html><body></body></html>", 1000);
        assert!(looks_javascript_driven(
            "<div id=\"root\"></div><script src=\"app.js\"></script>",
            &parsed
        ));
        assert!(!looks_javascript_driven(
            "<p>plenty of server rendered text that does not need javascript</p>",
            &parsed
        ));
    }

    #[test]
    fn merges_and_deduplicates_pages() {
        let mut first = sample(
            "https://example.com/1",
            json!({"items": [{"name": "a"}, {"name": "b"}]}),
        );
        let second = sample(
            "https://example.com/2",
            json!({"items": [{"name": "b"}, {"name": "c"}]}),
        );
        let pages = vec![first.clone(), second.clone()];
        let (merged, _) = merge_pages(&pages, None);
        assert_eq!(merged["items"].as_array().unwrap().len(), 4);

        let spec = Dedupe {
            enabled: true,
            keys: vec!["name".into()],
        };
        let (deduped, _) = merge_pages(&pages, Some(&spec));
        assert_eq!(deduped["items"].as_array().unwrap().len(), 3);

        first.data = json!({"title": "one", "items": []});
        second.data = json!({"title": "two", "items": []});
        let (merged, _) = merge_pages(&[first, second], None);
        assert_eq!(merged["title"], "one");
    }

    fn sample(url: &str, data: Value) -> PageOutput {
        PageOutput {
            requested_url: url.into(),
            final_url: url.into(),
            status: 200,
            content_type: "text/html".into(),
            render: "http".into(),
            cached: false,
            truncated: false,
            data,
            issues: Vec::new(),
            warnings: Vec::new(),
            content_hash: "h".into(),
            changed: true,
            items: 0,
            title: None,
            word_count: 0,
            elapsed_ms: 1,
            projection: "rules".into(),
            links: Vec::new(),
        }
    }
}

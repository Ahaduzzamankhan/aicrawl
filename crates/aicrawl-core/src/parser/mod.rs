//! HTML → structured page data.
//!
//! Parsing is deterministic and never executes JavaScript. Boilerplate removal
//! is heuristic: script/style/iframe-like elements and common chrome elements
//! (`nav`, `header`, `footer`, `aside`, `form`) are dropped before readable text
//! is assembled from block-level elements.

use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::util::{collapse_whitespace, truncate_chars};

/// A heading with its level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

/// A resolved hyperlink.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageLink {
    pub url: String,
    pub text: String,
    pub rel: Option<String>,
}

/// An image reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRef {
    pub src: String,
    pub alt: Option<String>,
}

/// A parsed HTML table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Table {
    pub caption: Option<String>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// A currency-looking price found in visible text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceMention {
    pub text: String,
    pub amount: f64,
    pub currency: Option<String>,
}

/// Everything deterministic parsing produces for one page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPage {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical: Option<String>,
    pub lang: Option<String>,
    pub headings: Vec<Heading>,
    pub text: String,
    pub markdown: String,
    pub word_count: usize,
    pub truncated: bool,
    pub links: Vec<PageLink>,
    pub images: Vec<ImageRef>,
    pub json_ld: Vec<serde_json::Value>,
    pub tables: Vec<Table>,
    pub meta: BTreeMap<String, String>,
    pub prices: Vec<PriceMention>,
}

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("static CSS selector must be valid")
}

fn element_text(element: &ElementRef<'_>) -> String {
    collapse_whitespace(&element.text().collect::<Vec<_>>().join(" "))
}

fn noise_regexes() -> &'static Vec<Regex> {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        [
            "script", "style", "noscript", "template", "svg", "canvas", "iframe", "object",
            "embed", "form", "nav", "footer", "aside", "header",
        ]
        .iter()
        .map(|tag| {
            Regex::new(&format!(
                r"(?is)<{tag}\b[^>]*>.*?</{tag}\s*>|<{tag}\b[^>]*/?>"
            ))
            .expect("static noise regex must compile")
        })
        .collect()
    })
}

fn price_regexes() -> &'static (Regex, Regex) {
    static REGEXES: OnceLock<(Regex, Regex)> = OnceLock::new();
    REGEXES.get_or_init(|| {
        (
            // $12.99 / € 1 299,00 (no thousands separator support beyond comma)
            Regex::new(r"(?i)([$€£¥₹])\s?(\d[\d.,]*)").unwrap(),
            // 12.99 USD / 12,99 EUR
            Regex::new(r"(?i)\b(\d[\d.,]*)\s?(USD|EUR|GBP|JPY|INR|CAD|AUD|CHF|SEK|BRL)\b").unwrap(),
        )
    })
}

/// Extracts metadata, readable text, links, tables and structured data.
pub fn parse_html(url: &Url, html: &str, max_chars: usize) -> ParsedPage {
    let document = Html::parse_document(html);
    let base = canonical_url(url)
        .and_then(|c| Url::parse(&c).ok())
        .unwrap_or_else(|| url.clone());

    let title = document
        .select(&selector("title"))
        .next()
        .map(|el| element_text(&el))
        .filter(|t| !t.is_empty())
        .or_else(|| {
            document
                .select(&selector("h1"))
                .next()
                .map(|el| element_text(&el))
                .filter(|t| !t.is_empty())
        });

    let description = meta_content(&document, "description")
        .or_else(|| meta_content(&document, "og:description"))
        .or_else(|| meta_content(&document, "twitter:description"));

    let canonical = canonical_url(url);
    let lang = document
        .select(&selector("html"))
        .next()
        .and_then(|el| el.value().attr("lang").map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty());

    let mut meta = BTreeMap::new();
    for element in document.select(&selector("meta")) {
        let value = element.value();
        let key = value.attr("name").or_else(|| value.attr("property"));
        let content = value.attr("content");
        if let (Some(key), Some(content)) = (key, content) {
            let key = key.trim().to_ascii_lowercase();
            if !key.is_empty() && !content.trim().is_empty() {
                meta.entry(key)
                    .or_insert_with(|| collapse_whitespace(content));
            }
        }
    }

    let mut headings = Vec::new();
    for element in document.select(&selector("h1, h2, h3, h4, h5, h6")) {
        let level = element
            .value()
            .name()
            .as_bytes()
            .get(1)
            .copied()
            .unwrap_or(b'0')
            - b'0';
        let text = element_text(&element);
        if !text.is_empty() {
            headings.push(Heading {
                level: level.clamp(1, 6),
                text,
            });
        }
    }

    let links = extract_links(&document, &base);
    let images = extract_images(&document, &base);
    let json_ld = extract_json_ld(&document);
    let tables = extract_tables(&document);
    let (text, markdown, word_count, truncated) = readable_text(&document, max_chars, &headings);
    let prices = extract_prices(&text);

    ParsedPage {
        url: url.to_string(),
        title,
        description,
        canonical,
        lang,
        headings,
        text,
        markdown,
        word_count,
        truncated,
        links,
        images,
        json_ld,
        tables,
        meta,
        prices,
    }
}

/// Parses a `text/plain` document into the same shape.
pub fn parse_plain_text(url: &Url, body: &str, max_chars: usize) -> ParsedPage {
    let cleaned = collapse_whitespace(body);
    let word_count = cleaned.split_whitespace().count();
    let (text, truncated) = truncate_chars(&cleaned, max_chars);
    ParsedPage {
        url: url.to_string(),
        title: None,
        description: None,
        canonical: canonical_url(url),
        lang: None,
        headings: Vec::new(),
        markdown: text.clone(),
        text,
        word_count,
        truncated,
        links: Vec::new(),
        images: Vec::new(),
        json_ld: Vec::new(),
        tables: Vec::new(),
        meta: BTreeMap::new(),
        prices: Vec::new(),
    }
}

fn meta_content(document: &Html, name: &str) -> Option<String> {
    let selector = selector("meta");
    document.select(&selector).find_map(|element| {
        let value = element.value();
        let key = value.attr("name").or_else(|| value.attr("property"))?;
        if key.eq_ignore_ascii_case(name) {
            value
                .attr("content")
                .map(collapse_whitespace)
                .filter(|c| !c.is_empty())
        } else {
            None
        }
    })
}

fn canonical_url(url: &Url) -> Option<String> {
    let mut copy = url.clone();
    copy.set_fragment(None);
    Some(copy.to_string())
}

/// Resolves a link against the page URL and drops non-HTTP and fragment links.
pub fn resolve_link(base: &Url, href: &str) -> Option<Url> {
    let href = href.trim();
    if href.is_empty()
        || href.starts_with('#')
        || href.starts_with("mailto:")
        || href.starts_with("tel:")
        || href.starts_with("javascript:")
        || href.starts_with("data:")
    {
        return None;
    }
    let mut resolved = base.join(href).ok()?;
    match resolved.scheme() {
        "http" | "https" => {}
        _ => return None,
    }
    if resolved.host_str().is_none() {
        return None;
    }
    resolved.set_fragment(None);
    Some(resolved)
}

fn extract_links(document: &Html, base: &Url) -> Vec<PageLink> {
    let mut seen = HashSet::new();
    let mut links = Vec::new();
    for element in document.select(&selector("a[href]")) {
        let Some(href) = element.value().attr("href") else {
            continue;
        };
        let Some(resolved) = resolve_link(base, href) else {
            continue;
        };
        let key = resolved.to_string();
        if !seen.insert(key.clone()) {
            continue;
        }
        let rel = element.value().attr("rel").map(|r| r.trim().to_string());
        links.push(PageLink {
            url: key,
            text: collapse_whitespace(&element_text(&element))
                .chars()
                .take(200)
                .collect(),
            rel,
        });
        if links.len() >= 500 {
            break;
        }
    }
    links
}

fn extract_images(document: &Html, base: &Url) -> Vec<ImageRef> {
    let mut seen = HashSet::new();
    let mut images = Vec::new();
    for element in document.select(&selector("img[src]")) {
        let Some(src) = element.value().attr("src") else {
            continue;
        };
        let Some(resolved) = resolve_link(base, src) else {
            continue;
        };
        let key = resolved.to_string();
        if !seen.insert(key.clone()) {
            continue;
        }
        images.push(ImageRef {
            src: key,
            alt: element
                .value()
                .attr("alt")
                .map(collapse_whitespace)
                .filter(|a| !a.is_empty()),
        });
        if images.len() >= 100 {
            break;
        }
    }
    images
}

fn extract_json_ld(document: &Html) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    for element in document.select(&selector("script[type='application/ld+json']")) {
        let raw: String = element.text().collect::<Vec<_>>().join("\n");
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(raw) {
            Ok(serde_json::Value::Array(items)) => values.extend(items),
            Ok(value) => values.push(value),
            Err(_) => continue,
        }
        if values.len() >= 50 {
            break;
        }
    }
    values
}

fn extract_tables(document: &Html) -> Vec<Table> {
    let mut tables = Vec::new();
    for element in document.select(&selector("table")) {
        let caption = element
            .select(&selector("caption"))
            .next()
            .map(|c| element_text(&c))
            .filter(|c| !c.is_empty());

        let mut headers: Vec<String> = Vec::new();
        if let Some(head_row) = element.select(&selector("thead tr")).next() {
            headers = header_cells(&head_row);
        }

        let rows: Vec<Vec<String>> = element
            .select(&selector("tr"))
            .map(|row| {
                row.select(&selector("td, th"))
                    .map(|cell| element_text(&cell))
                    .collect()
            })
            .filter(|row: &Vec<String>| !row.is_empty())
            .take(200)
            .collect();

        if headers.is_empty() {
            if let Some(first) = element.select(&selector("tr")).next() {
                let cells = header_cells(&first);
                let looks_like_header = first.select(&selector("th")).next().is_some();
                if looks_like_header {
                    headers = cells;
                }
            }
        }

        if !headers.is_empty() || !rows.is_empty() {
            tables.push(Table {
                caption,
                headers,
                rows,
            });
        }
        if tables.len() >= 20 {
            break;
        }
    }
    tables
}

fn header_cells(row: &ElementRef<'_>) -> Vec<String> {
    row.select(&selector("th, td"))
        .map(|cell| element_text(&cell))
        .filter(|cell| !cell.is_empty())
        .collect()
}

fn readable_text(
    document: &Html,
    max_chars: usize,
    headings: &[Heading],
) -> (String, String, usize, bool) {
    let scope = ["article", "main", "[role='main']", "body"]
        .iter()
        .find_map(|css| document.select(&selector(css)).next())
        .unwrap_or_else(|| document.root_element());

    let mut html = scope.inner_html();
    for regex in noise_regexes() {
        html = regex.replace_all(&html, " ").into_owned();
    }

    let cleaned = Html::parse_fragment(&html);
    let blocks = selector(
        "h1, h2, h3, h4, h5, h6, p, li, blockquote, pre, dt, dd, figcaption, caption, td, th",
    );

    let mut lines: Vec<String> = Vec::new();
    let mut markdown_lines: Vec<String> = Vec::new();
    let mut last = String::new();
    for element in cleaned.select(&blocks) {
        let text = element_text(&element);
        if text.is_empty() || text == last {
            continue;
        }
        last = text.clone();
        let tag = element.value().name();
        let md = match tag {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = tag.as_bytes()[1] as usize - b'0' as usize;
                format!("{} {text}", "#".repeat(level.clamp(1, 6)))
            }
            "li" => format!("- {text}"),
            "blockquote" => format!("> {text}"),
            _ => text.clone(),
        };
        lines.push(text);
        markdown_lines.push(md);
    }

    if lines.is_empty() {
        // No block elements: fall back to the plain text of the scope.
        let fallback = element_text(&scope);
        if !fallback.is_empty() {
            lines.push(fallback.clone());
            markdown_lines.push(fallback);
        }
    }

    let full_text = collapse_whitespace(&lines.join("\n"));
    let markdown = markdown_lines.join("\n\n");
    let word_count = full_text.split_whitespace().count();
    let (text, truncated) = truncate_chars(&full_text, max_chars);
    let markdown = truncate_chars(&markdown, max_chars).0;

    // Headings that never made it into the readable scope are still useful.
    if headings.is_empty() {
        let _ = headings;
    }

    (text, markdown, word_count, truncated)
}

fn extract_prices(text: &str) -> Vec<PriceMention> {
    let (symbol, code) = price_regexes();
    let mut prices = Vec::new();
    let mut seen = HashSet::new();
    for captures in symbol.captures_iter(text) {
        let symbol_text = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        let number = captures.get(2).map(|m| m.as_str()).unwrap_or("");
        let Some(amount) = parse_amount(number) else {
            continue;
        };
        let key = format!("{symbol_text}{amount}");
        if !seen.insert(key) {
            continue;
        }
        prices.push(PriceMention {
            text: captures
                .get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default(),
            amount,
            currency: Some(symbol_text.to_string()),
        });
        if prices.len() >= 50 {
            return prices;
        }
    }
    for captures in code.captures_iter(text) {
        let number = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        let Some(amount) = parse_amount(number) else {
            continue;
        };
        let currency = captures.get(2).map(|m| m.as_str().to_uppercase());
        let key = format!("{amount}{currency:?}");
        if !seen.insert(key) {
            continue;
        }
        prices.push(PriceMention {
            text: captures
                .get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default(),
            amount,
            currency,
        });
        if prices.len() >= 50 {
            break;
        }
    }
    prices
}

fn parse_amount(raw: &str) -> Option<f64> {
    let cleaned = raw.trim().trim_end_matches(|c| c == '.' || c == ',');
    if cleaned.is_empty() {
        return None;
    }
    // "1,299.00" and "1.299,00" both appear in the wild.
    let candidate = if cleaned.contains(',') && cleaned.contains('.') {
        if cleaned.rfind(',') > cleaned.rfind('.') {
            cleaned.replace('.', "").replace(',', ".")
        } else {
            cleaned.replace(',', "")
        }
    } else if cleaned.contains(',') {
        let decimals = cleaned.split(',').last().unwrap_or("");
        if decimals.len() == 2 {
            cleaned.replace(',', ".")
        } else {
            cleaned.replace(',', "")
        }
    } else {
        cleaned.to_string()
    };
    candidate
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = r#"
    <!doctype html>
    <html lang="en">
      <head>
        <title>Example Store</title>
        <meta name="description" content="Buy things" />
        <script type="application/ld+json">
          {"@context":"https://schema.org","@type":"Product","name":"Widget","offers":{"@type":"Offer","price":"19.99","priceCurrency":"USD"}}
        </script>
        <style>.x{color:red}</style>
      </head>
      <body>
        <nav><a href="/menu">Menu</a></nav>
        <header><h1>Our Products</h1></header>
        <main>
          <p>Welcome to the Example Store.</p>
          <h2>Widget</h2>
          <p>A useful widget for testing.</p>
          <p>Price: <strong>$19.99</strong></p>
          <table>
            <caption>Specs</caption>
            <thead><tr><th>Name</th><th>Value</th></tr></thead>
            <tbody><tr><td>Weight</td><td>1kg</td></tr></tbody>
          </table>
          <a href="/products/2">Second product</a>
          <img src="/img/widget.png" alt="Widget" />
        </main>
        <footer>Copyright</footer>
        <script>console.log('ignore me')</script>
      </body>
    </html>
    "#;

    #[test]
    fn parses_metadata_and_structure() {
        let url = Url::parse("https://example.com/products").unwrap();
        let page = parse_html(&url, HTML, 10_000);
        assert_eq!(page.title.as_deref(), Some("Example Store"));
        assert_eq!(page.description.as_deref(), Some("Buy things"));
        assert_eq!(page.lang.as_deref(), Some("en"));
        assert!(page
            .headings
            .iter()
            .any(|h| h.text == "Widget" && h.level == 2));
        assert_eq!(page.links[0].url, "https://example.com/menu");
        assert!(page.links.iter().any(|l| l.url.ends_with("/products/2")));
        assert_eq!(page.images[0].alt.as_deref(), Some("Widget"));
        assert_eq!(page.json_ld.len(), 1);
        assert_eq!(page.tables.len(), 1);
        assert_eq!(page.tables[0].headers, vec!["Name", "Value"]);
        assert!(page.text.contains("Welcome to the Example Store."));
        assert!(!page.text.contains("console.log"));
        assert!(!page.text.contains("color:red"));
        assert!(!page.text.contains("Copyright"));
        assert!(page
            .prices
            .iter()
            .any(|p| (p.amount - 19.99).abs() < f64::EPSILON));
    }

    #[test]
    fn plain_text_is_supported() {
        let url = Url::parse("https://example.com/readme.txt").unwrap();
        let page = parse_plain_text(&url, "hello\n\nworld", 100);
        assert_eq!(page.text, "hello world");
        assert_eq!(page.word_count, 2);
    }

    #[test]
    fn truncation_sets_flag() {
        let url = Url::parse("https://example.com/").unwrap();
        let page = parse_html(&url, "<html><body><p>abcdefghij</p></body></html>", 5);
        assert!(page.truncated);
        assert!(page.text.chars().count() <= 6);
    }

    #[test]
    fn amounts_parse_common_formats() {
        assert_eq!(parse_amount("1,299.00"), Some(1299.0));
        assert_eq!(parse_amount("1.299,00"), Some(1299.0));
        assert_eq!(parse_amount("12,50"), Some(12.5));
        assert_eq!(parse_amount("19."), Some(19.0));
        assert_eq!(parse_amount("abc"), None);
    }
}

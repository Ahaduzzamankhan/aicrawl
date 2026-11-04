//! Extraction engine.
//!
//! Deterministic extraction always runs and never requires AI. When the user
//! has configured an **external** AI provider (OpenAI-compatible endpoint or
//! Ollama) and `use_ai` is enabled, the cleaned page is sent to that provider
//! for semantic extraction. AICrawl ships no model and no credentials.
//!
//! Results requested with a JSON Schema are validated with the `jsonschema`
//! crate; one repair round is attempted when a model returns invalid JSON.

use std::sync::Arc;
use std::time::Instant;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use url::Url;

use crate::ai::{scan_injection, wrap_untrusted, AiRegistry, EXTRACTION_SYSTEM_PROMPT};
use crate::error::{AicrawlError, Result};
use crate::fetcher::{FetchOptions, Fetcher};
use crate::parser::{parse_html, parse_plain_text, Heading, PageLink, ParsedPage};
use crate::util::truncate_chars;

/// Parameters for one extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractRequest {
    pub url: String,
    pub query: String,
    #[serde(default)]
    pub schema: Option<Value>,
    #[serde(default = "default_output_format")]
    pub output_format: String,
    /// Use the configured external AI provider when semantic extraction is needed.
    #[serde(default = "default_true")]
    pub use_ai: bool,
    /// Render JavaScript through the optional Playwright bridge before parsing.
    #[serde(default)]
    pub render: bool,
    #[serde(default = "default_true")]
    pub use_cache: bool,
}

fn default_output_format() -> String {
    "json".to_string()
}

fn default_true() -> bool {
    true
}

impl ExtractRequest {
    /// Minimal request with default options.
    pub fn new(url: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            query: query.into(),
            schema: None,
            output_format: default_output_format(),
            use_ai: true,
            render: false,
            use_cache: true,
        }
    }

    fn normalized_format(&self) -> Result<&str> {
        match self.output_format.trim().to_ascii_lowercase().as_str() {
            "json" => Ok("json"),
            "markdown" | "md" => Ok("markdown"),
            "text" | "txt" => Ok("text"),
            other => Err(AicrawlError::InvalidInput(format!(
                "unsupported output_format '{other}'; use json, markdown or text"
            ))),
        }
    }
}

/// Summary of the fetched page attached to every extraction.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageSummary {
    pub final_url: String,
    pub status: u16,
    pub content_type: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub word_count: usize,
    pub bytes: usize,
    pub truncated: bool,
    pub from_cache: bool,
    pub rendered: bool,
}

/// Result of an extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractOutcome {
    pub url: String,
    pub query: String,
    /// `deterministic` or `ai`.
    pub mode: String,
    pub format: String,
    pub data: Value,
    /// Human-readable output for `markdown`/`text` formats.
    pub output: String,
    pub warnings: Vec<String>,
    pub provider: Option<String>,
    /// True only when a JSON Schema was supplied and the payload validated.
    pub validated: bool,
    pub duration_ms: u64,
    pub fetch_ms: u64,
    pub page: PageSummary,
}

/// Wraps the fetcher and the external AI provider registry.
#[derive(Clone)]
pub struct ExtractionEngine {
    fetcher: Arc<Fetcher>,
    ai: AiRegistry,
    max_text_chars: usize,
    max_input_chars: usize,
}

impl ExtractionEngine {
    /// Creates an engine sharing the process-wide fetcher.
    pub fn new(
        fetcher: Arc<Fetcher>,
        ai: AiRegistry,
        max_text_chars: usize,
        max_input_chars: usize,
    ) -> Self {
        Self {
            fetcher,
            ai,
            max_text_chars,
            max_input_chars,
        }
    }

    /// Fetches a URL and extracts data from it.
    pub async fn extract(&self, request: &ExtractRequest) -> Result<ExtractOutcome> {
        let format = request.normalized_format()?.to_string();
        if request.query.trim().is_empty() {
            return Err(AicrawlError::InvalidInput(
                "query must not be empty; describe what to extract".into(),
            ));
        }
        let url = self.fetcher.policy().parse(&request.url)?;
        let started = Instant::now();

        let fetched = self
            .fetcher
            .fetch(
                &url,
                &FetchOptions {
                    render: request.render,
                    use_cache: request.use_cache,
                    ..Default::default()
                },
            )
            .await?;

        let summary = PageSummary {
            final_url: fetched.url.clone(),
            status: fetched.status,
            content_type: fetched.content_type.clone(),
            title: None,
            description: None,
            word_count: 0,
            bytes: fetched.bytes,
            truncated: fetched.truncated,
            from_cache: fetched.from_cache,
            rendered: fetched.rendered,
        };
        let fetch_ms = fetched.elapsed_ms;

        let final_url = Url::parse(&fetched.url).unwrap_or(url);
        let mut outcome = self
            .extract_document(
                &final_url,
                &fetched.content_type,
                &fetched.body,
                request,
                summary,
                fetched.warnings,
            )
            .await?;
        outcome.format = format;
        outcome.output = render_output(&outcome.format, &outcome.data, &mut outcome.page);
        outcome.fetch_ms = fetch_ms;
        outcome.duration_ms = started.elapsed().as_millis() as u64;
        Ok(outcome)
    }

    /// Extracts from an already fetched document (used by the crawler to avoid refetching).
    pub async fn extract_document(
        &self,
        url: &Url,
        content_type: &str,
        body: &str,
        request: &ExtractRequest,
        mut summary: PageSummary,
        mut warnings: Vec<String>,
    ) -> Result<ExtractOutcome> {
        let started = Instant::now();
        let format = request.normalized_format()?.to_string();
        let parsed = parse_document(content_type, body, url, self.max_text_chars);
        let json_payload = json_payload(content_type, body);

        summary.title = parsed.title.clone();
        summary.description = parsed.description.clone();
        summary.word_count = parsed.word_count;
        summary.content_type = content_type.to_string();

        for warning in scan_injection(&parsed.text) {
            if !warnings.contains(&warning) {
                warnings.push(warning);
            }
        }

        let deterministic = deterministic_data(&parsed, &request.query, json_payload.as_ref());
        let mut data = deterministic.clone();
        let mut mode = "deterministic".to_string();
        let mut provider = None;

        let client = if request.use_ai {
            self.ai.active().await?
        } else {
            None
        };

        if let Some(client) = client {
            let prompt = self.build_prompt(&parsed, request);
            match client
                .complete_json(EXTRACTION_SYSTEM_PROMPT, &prompt)
                .await
            {
                Ok(value) => {
                    let mut candidate = value;
                    let mut schema_errors = request
                        .schema
                        .as_ref()
                        .and_then(|schema| validate_schema(schema, &candidate));

                    if let (Some(schema), Some(errors)) = (&request.schema, schema_errors.as_ref())
                    {
                        let repair_prompt = self.build_repair_prompt(schema, &candidate, errors);
                        match client
                            .complete_json(EXTRACTION_SYSTEM_PROMPT, &repair_prompt)
                            .await
                        {
                            Ok(repaired) => {
                                let retry_errors = validate_schema(schema, &repaired);
                                if retry_errors.is_none() {
                                    candidate = repaired;
                                    schema_errors = None;
                                } else {
                                    schema_errors = retry_errors;
                                }
                            }
                            Err(err) => {
                                warnings.push(format!(
                                    "AI repair attempt failed: {err}; returning first response"
                                ));
                            }
                        }
                    }

                    if let Some(errors) = schema_errors {
                        return Err(AicrawlError::Schema(format!(
                            "AI response did not match the requested schema after one repair attempt: {}",
                            errors.join("; ")
                        )));
                    }

                    data = candidate;
                    mode = "ai".to_string();
                    provider = Some(client.describe());
                }
                Err(err) => {
                    return Err(AicrawlError::Ai(format!(
                        "extraction through external provider failed: {err}"
                    )));
                }
            }
        } else if request.use_ai {
            warnings.push(
                "no external AI provider is configured; returned deterministic extraction only"
                    .into(),
            );
        }

        if let Some(schema) = request.schema.as_ref() {
            if let Some(errors) = validate_schema(schema, &data) {
                return Err(AicrawlError::Schema(format!(
                    "extraction did not satisfy the requested schema: {}. {}",
                    errors.join("; "),
                    if mode == "ai" {
                        "Adjust the query or schema and try again."
                    } else {
                        "Configure an external AI provider for schema-constrained extraction, or request a schema that matches the deterministic payload."
                    }
                )));
            }
        }

        let mut outcome = ExtractOutcome {
            url: url.to_string(),
            query: request.query.clone(),
            mode,
            format,
            output: String::new(),
            data,
            warnings,
            provider,
            validated: request.schema.is_some(),
            duration_ms: started.elapsed().as_millis() as u64,
            fetch_ms: 0,
            page: summary,
        };
        outcome.output = render_output(&outcome.format, &outcome.data, &mut outcome.page);
        Ok(outcome)
    }

    fn build_prompt(&self, parsed: &ParsedPage, request: &ExtractRequest) -> String {
        let mut prompt = String::new();
        prompt.push_str("Extract information from the page below.\n\n");
        prompt.push_str(&format!("Query: {}\n", request.query.trim()));
        prompt.push_str(&format!("URL: {}\n", parsed.url));
        if let Some(title) = parsed.title.as_deref() {
            prompt.push_str(&format!("Title: {title}\n"));
        }
        if let Some(schema) = request.schema.as_ref() {
            prompt.push_str(
                "\nThe JSON response MUST validate against this JSON Schema (no extra restrictions beyond it):\n",
            );
            prompt.push_str(&pretty_limited(schema, 6_000).as_str());
            prompt.push('\n');
        } else {
            prompt.push_str(
                "\nReturn a JSON object with clearly named fields that answer the query.\n",
            );
        }

        if !parsed.headings.is_empty() {
            let headings: Vec<&str> = parsed
                .headings
                .iter()
                .take(25)
                .map(|h| h.text.as_str())
                .collect();
            prompt.push_str(&format!("\nHeadings: {}\n", headings.join(" | ")));
        }
        if !parsed.tables.is_empty() {
            prompt.push_str("\nTables (JSON):\n");
            prompt.push_str(&pretty_limited(&json!(parsed.tables), 4_000));
            prompt.push('\n');
        }
        if !parsed.json_ld.is_empty() {
            prompt.push_str("\nStructured data found on the page (JSON-LD):\n");
            prompt.push_str(&pretty_limited(&json!(parsed.json_ld), 4_000));
            prompt.push('\n');
        }

        let (content, cut) = truncate_chars(&parsed.text, self.max_input_chars);
        if cut {
            prompt.push_str("\n[Content truncated to fit the model context]\n");
        }
        prompt.push_str("\nPage content:\n");
        prompt.push_str(&wrap_untrusted(&content));
        prompt.push_str("\n\nReturn only the JSON document.");
        prompt
    }

    fn build_repair_prompt(&self, schema: &Value, candidate: &Value, errors: &[String]) -> String {
        format!(
            "The previous JSON response did not validate against the requested schema.\n\nSchema:\n{}\n\nPrevious response:\n{}\n\nValidation errors:\n{}\n\nReturn a corrected JSON document that fixes every error and validates against the schema. Return JSON only.",
            pretty_limited(schema, 6_000),
            pretty_limited(candidate, 8_000),
            errors.join("\n")
        )
    }
}

/// Parses a document based on its content type.
pub fn parse_document(content_type: &str, body: &str, url: &Url, max_chars: usize) -> ParsedPage {
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime == "text/html" || mime == "application/xhtml+xml" || mime.contains("xml") {
        parse_html(url, body, max_chars)
    } else {
        parse_plain_text(url, body, max_chars)
    }
}

fn json_payload(content_type: &str, body: &str) -> Option<Value> {
    let mime = content_type.split(';').next().unwrap_or("").trim();
    if !mime.contains("json") {
        return None;
    }
    serde_json::from_str(body).ok()
}

fn render_output(format: &str, data: &Value, page: &mut PageSummary) -> String {
    match format {
        "markdown" => {
            if let Some(markdown) = data.get("markdown").and_then(Value::as_str) {
                return markdown.to_string();
            }
            let mut out = String::new();
            if let Some(title) = page.title.as_deref() {
                out.push_str(&format!("# {title}\n\n"));
            }
            if let Some(summary) = data
                .get("summary")
                .or_else(|| data.get("answer"))
                .and_then(Value::as_str)
            {
                out.push_str(summary);
                out.push_str("\n\n");
            }
            out.push_str("```json\n");
            out.push_str(&pretty_limited(data, 200_000));
            out.push_str("\n```\n");
            out
        }
        "text" => {
            if let Some(text) = data.get("text").and_then(Value::as_str) {
                return text.to_string();
            }
            if let Some(content) = data.pointer("/content/text").and_then(Value::as_str) {
                return content.to_string();
            }
            pretty_limited(data, 200_000)
        }
        _ => serde_json::to_string_pretty(data).unwrap_or_else(|_| "null".into()),
    }
}

fn pretty_limited(value: &Value, max_chars: usize) -> String {
    let raw = serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".into());
    truncate_chars(&raw, max_chars).0
}

/// Validates `data` against `schema`; returns human-readable errors when invalid.
pub fn validate_schema(schema: &Value, data: &Value) -> Option<Vec<String>> {
    let validator = match jsonschema::validator_for(schema) {
        Ok(validator) => validator,
        Err(err) => return Some(vec![format!("schema is invalid: {err}")]),
    };
    if validator.is_valid(data) {
        return None;
    }
    let mut errors: Vec<String> = validator
        .iter_errors(data)
        .take(20)
        .map(|err| format!("{}: {}", err.instance_path(), err))
        .collect();
    if errors.is_empty() {
        errors.push("payload does not match schema".into());
    }
    Some(errors)
}

/// Deterministic, AI-free extraction payload.
pub fn deterministic_data(parsed: &ParsedPage, query: &str, json_payload: Option<&Value>) -> Value {
    let intent = QueryIntent::parse(query);
    let products = extract_products(parsed);
    let prices = &parsed.prices;

    let mut data = json!({
        "url": parsed.url,
        "title": parsed.title,
        "description": parsed.description,
        "query": query,
        "mode": "deterministic",
        "intent": {
            "products": intent.products,
            "tables": intent.tables,
            "links": intent.links,
            "contact": intent.contact,
        },
        "content": {
            "text": parsed.text,
            "word_count": parsed.word_count,
            "truncated": parsed.truncated,
        },
        "headings": parsed.headings,
        "matched_sections": matched_sections(parsed, &intent.keywords),
        "tables": parsed.tables.iter().take(10).collect::<Vec<_>>(),
        "structured_data": parsed.json_ld.iter().take(10).collect::<Vec<_>>(),
        "prices": prices.iter().take(30).collect::<Vec<_>>(),
        "links": parsed.links.iter().take(50).collect::<Vec<_>>(),
        "images": parsed.images.iter().take(20).collect::<Vec<_>>(),
        "emails": find_emails(&parsed.text),
        "phones": find_phones(&parsed.text),
        "metadata": parsed.meta,
        "truncated": parsed.truncated,
    });

    if let Some(payload) = json_payload {
        data["json"] = payload.clone();
    }
    if !products.is_empty() {
        data["products"] = Value::Array(products);
    }
    data
}

/// Keywords and coarse intent inferred from the natural-language query.
#[derive(Debug, Clone)]
pub struct QueryIntent {
    pub keywords: Vec<String>,
    pub products: bool,
    pub tables: bool,
    pub links: bool,
    pub contact: bool,
}

impl QueryIntent {
    /// Tokenises the query and detects common intents.
    pub fn parse(query: &str) -> Self {
        const STOPWORDS: &[&str] = &[
            "the",
            "and",
            "for",
            "with",
            "from",
            "that",
            "this",
            "into",
            "all",
            "any",
            "are",
            "extract",
            "find",
            "get",
            "list",
            "give",
            "show",
            "page",
            "website",
            "site",
            "please",
            "what",
            "which",
            "where",
            "how",
            "does",
            "their",
            "there",
            "about",
            "including",
        ];
        let lower = query.to_ascii_lowercase();
        let mut keywords: Vec<String> = Vec::new();
        for token in lower.split(|c: char| !c.is_alphanumeric()) {
            let token = token.trim();
            if token.len() < 3 || STOPWORDS.contains(&token) {
                continue;
            }
            let owned = token.to_string();
            if !keywords.contains(&owned) {
                keywords.push(owned);
            }
        }
        Self {
            products: [
                "product", "price", "cost", "shop", "buy", "offer", "item", "pricing",
            ]
            .iter()
            .any(|needle| lower.contains(needle)),
            tables: lower.contains("table") || lower.contains("column") || lower.contains("row"),
            links: lower.contains("link") || lower.contains("url") || lower.contains("navigation"),
            contact: lower.contains("contact")
                || lower.contains("email")
                || lower.contains("phone"),
            keywords,
        }
    }
}

fn matched_sections(parsed: &ParsedPage, keywords: &[String]) -> Vec<String> {
    if keywords.is_empty() {
        return Vec::new();
    }
    let mut matches: Vec<(usize, String)> = parsed
        .text
        .lines()
        .filter(|line| line.len() >= 12)
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            let score = keywords
                .iter()
                .filter(|keyword| lower.contains(keyword.as_str()))
                .count();
            (score, truncate_chars(line.trim(), 400).0)
        })
        .filter(|(score, _)| *score > 0)
        .collect();
    matches.sort_by(|a, b| b.0.cmp(&a.0));
    matches.into_iter().take(12).map(|(_, line)| line).collect()
}

fn extract_products(parsed: &ParsedPage) -> Vec<Value> {
    let mut products: Vec<Value> = Vec::new();

    // 1. JSON-LD Product/Offer entries (highest confidence).
    for value in &parsed.json_ld {
        collect_json_ld_products(value, &mut products);
    }

    // 2. Tables with a name-like and a price-like column.
    for table in &parsed.tables {
        let headers: Vec<String> = table
            .headers
            .iter()
            .map(|h| h.to_ascii_lowercase())
            .collect();
        let name_index = headers.iter().position(|h| {
            h.contains("name") || h.contains("product") || h.contains("title") || h.contains("item")
        });
        let price_index = headers
            .iter()
            .position(|h| h.contains("price") || h.contains("cost") || h.contains("amount"));
        let (Some(name_index), Some(price_index)) = (name_index, price_index) else {
            continue;
        };
        for row in table
            .rows
            .iter()
            .skip(usize::from(!table.headers.is_empty()))
        {
            let name = row.get(name_index).map(|s| s.trim()).unwrap_or("");
            let price = row.get(price_index).map(|s| s.trim()).unwrap_or("");
            if name.is_empty() || price.is_empty() {
                continue;
            }
            products.push(json!({
                "name": name,
                "price": price,
                "source": "table",
                "table": table.caption,
            }));
        }
    }

    // 3. Price lines with a nearby preceding label.
    let lines: Vec<&str> = parsed
        .text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    for (index, line) in lines.iter().enumerate() {
        let has_price = parsed
            .prices
            .iter()
            .any(|price| line.contains(price.text.as_str()));
        if !has_price || line.len() > 240 {
            continue;
        }
        let label = if line.len() <= 120 {
            Some((*line).to_string())
        } else if index > 0 && lines[index - 1].len() <= 120 {
            Some(lines[index - 1].to_string())
        } else {
            None
        };
        let Some(label) = label else { continue };
        if products
            .iter()
            .any(|product| product.get("name").and_then(Value::as_str) == Some(label.as_str()))
        {
            continue;
        }
        products.push(json!({
            "name": label,
            "price": line,
            "source": "text",
        }));
        if products.len() >= 50 {
            break;
        }
    }

    products.truncate(50);
    products
}

fn collect_json_ld_products(value: &Value, out: &mut Vec<Value>) {
    match value {
        Value::Array(items) => items
            .iter()
            .for_each(|item| collect_json_ld_products(item, out)),
        Value::Object(map) => {
            let types: Vec<String> = match map.get("@type") {
                Some(Value::String(name)) => vec![name.clone()],
                Some(Value::Array(names)) => names
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
                _ => Vec::new(),
            };
            if types.iter().any(|t| t == "Product" || t == "ProductGroup") {
                let name = map.get("name").and_then(Value::as_str);
                let description = map.get("description").and_then(Value::as_str);
                let (price, currency) = offer_price(map);
                if name.is_some() || price.is_some() {
                    out.push(json!({
                        "name": name,
                        "description": description,
                        "price": price,
                        "currency": currency,
                        "source": "json-ld",
                    }));
                }
            }
            for child in map.values() {
                if child.is_object() || child.is_array() {
                    collect_json_ld_products(child, out);
                }
            }
        }
        _ => {}
    }
}

fn offer_price(map: &serde_json::Map<String, Value>) -> (Option<String>, Option<String>) {
    let Some(offers) = map.get("offers") else {
        return (None, None);
    };
    let first = match offers {
        Value::Array(items) => items.first(),
        other => Some(other),
    };
    let Some(offer) = first else {
        return (None, None);
    };
    let price = offer
        .get("price")
        .or_else(|| offer.get("lowPrice"))
        .map(|value| match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        });
    let currency = offer
        .get("priceCurrency")
        .and_then(Value::as_str)
        .map(str::to_string);
    (price, currency)
}

fn find_emails(text: &str) -> Vec<String> {
    static EMAIL: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let regex = EMAIL.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").expect("static regex")
    });
    let mut seen: Vec<String> = Vec::new();
    for matched in regex.find_iter(text) {
        let value = matched.as_str().to_string();
        if !seen.contains(&value) {
            seen.push(value);
        }
        if seen.len() >= 20 {
            break;
        }
    }
    seen
}

fn find_phones(text: &str) -> Vec<String> {
    static PHONE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let regex = PHONE.get_or_init(|| {
        Regex::new(r"(?:\+\d{1,3}[\s().-]?)?(?:\(\d{2,4}\)[\s.-]?)?\d{3,5}[\s.-]?\d{3,4}(?:[\s.-]?\d{2,4})?")
            .expect("static regex")
    });
    let mut seen: Vec<String> = Vec::new();
    for matched in regex.find_iter(text) {
        let raw = matched.as_str().trim();
        if raw.chars().filter(|c| c.is_ascii_digit()).count() < 7 {
            continue;
        }
        if !seen.iter().any(|existing| existing == raw) {
            seen.push(raw.to_string());
        }
        if seen.len() >= 10 {
            break;
        }
    }
    seen
}

/// Links worth enqueueing for a crawl (skips binaries and non-HTTP schemes).
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
    !SKIP_EXTENSIONS.iter().any(|ext| path.ends_with(ext))
}

/// Headings summary helper shared with the API.
pub fn heading_outline(headings: &[Heading]) -> Vec<Value> {
    headings
        .iter()
        .map(|heading| json!({"level": heading.level, "text": heading.text}))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parsed() -> ParsedPage {
        let url = Url::parse("https://example.com/products").unwrap();
        let html = r#"
        <html><head><title>Store</title>
        <script type="application/ld+json">
        {"@type":"Product","name":"Widget","offers":{"@type":"Offer","price":"19.99","priceCurrency":"USD"}}
        </script></head>
        <body>
          <h1>Products</h1>
          <p>Contact us at sales@example.com or +1 (555) 123-4567.</p>
          <table><thead><tr><th>Product</th><th>Price</th></tr></thead>
          <tbody><tr><td>Gadget</td><td>$29.00</td></tr></tbody></table>
          <p>Widget - $19.99</p>
        </body></html>
        "#;
        parse_html(&url, html, 10_000)
    }

    #[test]
    fn deterministic_payload_contains_expected_blocks() {
        let page = parsed();
        let data = deterministic_data(&page, "Extract product names and prices", None);
        assert_eq!(data["mode"], "deterministic");
        assert_eq!(data["intent"]["products"], true);
        assert!(data["products"].as_array().unwrap().len() >= 2);
        assert!(data["emails"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "sales@example.com"));
        assert!(data["tables"].as_array().unwrap().len() == 1);
        assert!(data["matched_sections"].as_array().is_some());
    }

    #[test]
    fn schema_validation_reports_errors() {
        let schema = json!({
            "type": "object",
            "required": ["items"],
            "properties": {"items": {"type": "array"}}
        });
        assert!(validate_schema(&schema, &json!({"items": []})).is_none());
        let errors = validate_schema(&schema, &json!({"other": 1})).unwrap();
        assert!(!errors.is_empty());
        let invalid_schema = json!({"type": "not-a-type"});
        assert!(validate_schema(&invalid_schema, &json!({})).is_some());
    }

    #[test]
    fn crawlable_links_skip_assets() {
        let link = |url: &str| PageLink {
            url: url.into(),
            text: String::new(),
            rel: None,
        };
        assert!(is_crawlable(&link("https://example.com/about")));
        assert!(!is_crawlable(&link("https://example.com/manual.pdf")));
        assert!(!is_crawlable(&link("https://example.com/logo.png")));
        assert!(!is_crawlable(&link("mailto:a@example.com")));
    }

    #[test]
    fn request_format_is_validated() {
        let mut request = ExtractRequest::new("https://example.com", "q");
        request.output_format = "yaml".into();
        assert!(request.normalized_format().is_err());
        request.output_format = "Markdown".into();
        assert_eq!(request.normalized_format().unwrap(), "markdown");
    }
}

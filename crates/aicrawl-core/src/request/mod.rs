//! Versioned JSON request contract.
//!
//! AICrawl is JSON-first: callers describe **what** to extract with structured
//! selectors and receive structured JSON back. There is no natural-language
//! prompt and no model in the request path.
//!
//! The schema is versioned; `version` defaults to [`SCHEMA_VERSION`] and must
//! match it when present. Requests are validated *before* any network access so
//! invalid selectors, URLs and limits fail fast with a structured error.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AicrawlError, Result};
use crate::selectors::{validate_css, validate_xpath};

/// Version of the request contract implemented by this build.
pub const SCHEMA_VERSION: &str = "1";

/// What the engine should do with the described target(s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Fetch one page and apply the selector rules.
    #[default]
    Extract,
    /// Crawl a site, applying the selector rules to every visited page.
    Crawl,
    /// Discover URLs only (links, pagination and sitemap.xml).
    Map,
    /// Apply the same rules to several pages.
    Batch,
}

impl Action {
    /// Stable string form used in logs and responses.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Extract => "extract",
            Self::Crawl => "crawl",
            Self::Map => "map",
            Self::Batch => "batch",
        }
    }
}

/// When to use the optional Playwright rendering bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RenderMode {
    /// Render only when the fetched HTML looks JavaScript-dependent.
    #[default]
    Auto,
    /// Never render; raw HTTP only.
    #[serde(alias = "never", alias = "false", alias = "none")]
    Off,
    /// Always render through the bridge.
    #[serde(alias = "always", alias = "true", alias = "playwright")]
    On,
}

impl RenderMode {
    /// Whether the caller asked for rendering unconditionally.
    pub fn forced(&self) -> bool {
        matches!(self, Self::On)
    }

    /// Stable string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Off => "off",
            Self::On => "on",
        }
    }
}

/// Crawl limits for `action = "crawl"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CrawlSpec {
    /// Maximum pages to fetch (1-500).
    pub max_pages: u32,
    /// Maximum link depth (0-5).
    pub max_depth: u32,
    /// Restrict the crawl to the starting host.
    pub same_domain: bool,
    /// Skip pages whose content hash did not change since the last crawl.
    pub incremental: bool,
    /// Also read `/sitemap.xml` and `Sitemap:` entries in robots.txt.
    pub sitemap: bool,
}

impl Default for CrawlSpec {
    fn default() -> Self {
        Self {
            max_pages: 10,
            max_depth: 2,
            same_domain: true,
            incremental: false,
            sitemap: true,
        }
    }
}

impl CrawlSpec {
    /// Clamped limits used by the engine.
    pub fn limits(&self) -> (u32, u32) {
        (self.max_pages.clamp(1, 500), self.max_depth.min(5))
    }
}

/// Follow a "next page" link while extracting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Pagination {
    pub enabled: bool,
    /// CSS selector (or XPath when prefixed with `/` or `//`) for the next link.
    pub next_selector: String,
    /// Hard cap on pages followed, including the first page.
    pub max_pages: u32,
    /// Refuse to follow the next link to a different origin.
    pub same_origin: bool,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            enabled: false,
            next_selector: String::new(),
            max_pages: 10,
            same_origin: true,
        }
    }
}

/// Remove duplicate extracted items.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Dedupe {
    pub enabled: bool,
    /// Field names inside list items that identify an item.
    pub keys: Vec<String>,
}

impl Default for Dedupe {
    fn default() -> Self {
        Self {
            enabled: false,
            keys: Vec::new(),
        }
    }
}

/// Serialization options for the response payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// A single JSON document (default).
    #[default]
    Json,
    /// One JSON object per line, convenient for datasets.
    #[serde(alias = "ndjson")]
    Jsonl,
}

/// Output options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputOptions {
    pub format: OutputFormat,
    /// Pretty-print the JSON body in CLI/API output.
    pub pretty: bool,
}

impl Default for OutputOptions {
    fn default() -> Self {
        Self {
            format: OutputFormat::Json,
            pretty: true,
        }
    }
}

/// Kind of value a rule produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// Visible text of the first match (default).
    #[default]
    Text,
    /// Sanitized inner HTML of the first match.
    Html,
    /// Attribute value of the first match.
    #[serde(alias = "attr")]
    Attribute,
    /// Text (or attribute) resolved to an absolute URL.
    #[serde(alias = "href", alias = "link")]
    Url,
    /// Text converted to a JSON number.
    Number,
    /// Text converted to an integer.
    Integer,
    /// Text converted to a boolean.
    Boolean,
    /// Repeated elements, optionally projected through `fields`.
    #[serde(alias = "array")]
    List,
    /// A nested object built from `fields`.
    #[serde(alias = "nested")]
    Object,
    /// Schema.org JSON-LD blocks found in the page (or inside the match).
    #[serde(alias = "jsonld", alias = "json-ld")]
    JsonLd,
    /// A `<meta name=...>`/`<meta property=...>` value looked up by `name`.
    Meta,
}

impl RuleKind {
    /// Stable string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Html => "html",
            Self::Attribute => "attribute",
            Self::Url => "url",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::List => "list",
            Self::Object => "object",
            Self::JsonLd => "json_ld",
            Self::Meta => "meta",
        }
    }
}

/// A single deterministic transform applied to a string value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transform {
    /// Trim leading and trailing whitespace.
    Trim,
    /// Collapse internal whitespace runs to single spaces.
    #[serde(alias = "collapse")]
    CollapseWhitespace,
    /// Lowercase the value.
    Lowercase,
    /// Uppercase the value.
    Uppercase,
    /// Remove tags and decode basic HTML entities.
    StripHtml,
    /// Parse a number (`$1,299.00` becomes `1299.0`).
    Number,
    /// Parse an integer.
    Integer,
    /// Parse a boolean (`yes`, `true`, `1`, ...).
    Boolean,
    /// Resolve the value against the page URL.
    #[serde(alias = "url")]
    AbsoluteUrl,
}

impl Transform {
    /// Stable string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trim => "trim",
            Self::CollapseWhitespace => "collapse_whitespace",
            Self::Lowercase => "lowercase",
            Self::Uppercase => "uppercase",
            Self::StripHtml => "strip_html",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::AbsoluteUrl => "absolute_url",
        }
    }
}

/// A single transform or a list of transforms.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TransformSpec {
    /// One transform.
    One(Transform),
    /// Applied in order.
    Many(Vec<Transform>),
}

impl TransformSpec {
    /// Flattened transform list.
    pub fn list(&self) -> Vec<Transform> {
        match self {
            Self::One(transform) => vec![*transform],
            Self::Many(list) => list.clone(),
        }
    }
}

/// One extraction rule.
///
/// A rule selects nodes with `selector` (CSS) or `xpath` and converts them with
/// `type`. `fields` turns a rule into an object projection (or a list of
/// objects when `type = "list"`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Rule {
    #[serde(rename = "type", default)]
    pub kind: RuleKind,
    /// CSS selector evaluated inside the parent scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// XPath expression evaluated inside the parent scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xpath: Option<String>,
    /// Attribute name for `attribute`/`url` rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute: Option<String>,
    /// Meta name or property for `meta` rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Nested field rules.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Rule>,
    /// Fail the whole extraction when this field is missing or empty.
    #[serde(default)]
    pub required: bool,
    /// Value used when an optional field is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    /// Collect every match instead of the first one.
    #[serde(default)]
    pub multiple: bool,
    /// Post-processing transforms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<TransformSpec>,
    /// Override the whitespace handling for text values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim: Option<bool>,
    /// Resolve this value (or every value) as an absolute URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absolute_url: Option<bool>,
    /// Cap on items produced by a list rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
}

impl Rule {
    /// Whether this rule projects nested fields.
    pub fn has_fields(&self) -> bool {
        !self.fields.is_empty()
    }

    /// Whether the rule produces a collection.
    pub fn is_collection(&self) -> bool {
        self.kind == RuleKind::List || self.multiple
    }

    /// Transforms declared on the rule.
    pub fn transforms(&self) -> Vec<Transform> {
        self.transform
            .as_ref()
            .map(TransformSpec::list)
            .unwrap_or_default()
    }
}

/// A validated, versioned extraction request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Contract version; defaults to [`SCHEMA_VERSION`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Single target URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Multiple target URLs (`action = "batch"`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
    #[serde(default)]
    pub action: Action,
    #[serde(default)]
    pub render: RenderMode,
    /// Named selector rules.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub selectors: BTreeMap<String, Rule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Pagination>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deduplicate: Option<Dedupe>,
    #[serde(default)]
    pub output: OutputOptions,
    /// Optional JSON Schema the extracted data must validate against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crawl: Option<CrawlSpec>,
    /// Reuse the SQLite page cache.
    #[serde(default = "default_true")]
    pub use_cache: bool,
    /// Add a `_document` summary (metadata, JSON-LD, headings, text) to `data`.
    #[serde(default)]
    pub include_document: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Request {
    fn default() -> Self {
        Self {
            version: None,
            url: None,
            urls: Vec::new(),
            action: Action::Extract,
            render: RenderMode::Auto,
            selectors: BTreeMap::new(),
            pagination: None,
            deduplicate: None,
            output: OutputOptions::default(),
            schema: None,
            crawl: None,
            use_cache: true,
            include_document: false,
        }
    }
}

impl Request {
    /// A minimal extraction request for one URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: Some(url.into()),
            ..Self::default()
        }
    }

    /// Parses a JSON request body.
    pub fn from_json(raw: &str) -> Result<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(AicrawlError::InvalidInput(
                "request body is empty; send a JSON request object".into(),
            ));
        }
        serde_json::from_str(trimmed).map_err(|e| {
            AicrawlError::InvalidInput(format!(
                "request is not valid JSON for schema v{SCHEMA_VERSION}: {e}"
            ))
        })
    }

    /// Effective crawl spec (`action = "crawl"`).
    pub fn crawl_spec(&self) -> CrawlSpec {
        self.crawl.clone().unwrap_or_default()
    }

    /// Effective rendering decision: force, disable, or decide after fetching.
    pub fn render_mode(&self) -> RenderMode {
        self.render
    }

    /// Targets in request order, without duplicates.
    pub fn targets(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if let Some(url) = self.url.as_deref() {
            push_unique(&mut out, url);
        }
        for url in &self.urls {
            push_unique(&mut out, url);
        }
        out
    }

    /// Clamps limits against configuration and engine hard caps.
    pub fn clamped_pagination(&self) -> Option<Pagination> {
        self.pagination.as_ref().map(|spec| Pagination {
            enabled: spec.enabled,
            next_selector: spec.next_selector.trim().to_string(),
            max_pages: spec.max_pages.clamp(1, 100),
            same_origin: spec.same_origin,
        })
    }

    /// Dedupe settings normalised for the engine.
    pub fn dedupe_spec(&self) -> Option<Dedupe> {
        self.deduplicate.as_ref().and_then(|spec| {
            let keys: Vec<String> = spec
                .keys
                .iter()
                .map(|key| key.trim().to_string())
                .filter(|key| !key.is_empty())
                .collect();
            (spec.enabled && !keys.is_empty()).then_some(Dedupe {
                enabled: true,
                keys,
            })
        })
    }

    /// Validates the request without touching the network.
    pub fn validate(&self) -> Result<()> {
        if let Some(version) = self.version.as_deref() {
            let version = version.trim();
            if !version.is_empty() && version != SCHEMA_VERSION {
                return Err(AicrawlError::InvalidInput(format!(
                    "unsupported request version '{version}'; this build implements version {SCHEMA_VERSION}"
                )));
            }
        }

        let targets = self.targets();
        if targets.is_empty() {
            return Err(AicrawlError::InvalidInput(
                "request must contain 'url' or 'urls'".into(),
            ));
        }
        if self.action != Action::Batch && self.url.is_none() {
            return Err(AicrawlError::InvalidInput(
                "request must contain 'url' for this action".into(),
            ));
        }
        if self.action == Action::Batch && self.urls.is_empty() {
            return Err(AicrawlError::InvalidInput(
                "action 'batch' requires a non-empty 'urls' array".into(),
            ));
        }
        if targets.len() > 200 {
            return Err(AicrawlError::InvalidInput(
                "at most 200 target URLs are accepted per request".into(),
            ));
        }
        for target in &targets {
            validate_url(target)?;
        }

        if let Some(spec) = &self.pagination {
            if spec.enabled {
                if spec.next_selector.trim().is_empty() {
                    return Err(AicrawlError::InvalidInput(
                        "pagination.next_selector must not be empty when pagination is enabled"
                            .into(),
                    ));
                }
                validate_selector_or_xpath(&spec.next_selector, "pagination.next_selector")?;
                if spec.max_pages == 0 {
                    return Err(AicrawlError::InvalidInput(
                        "pagination.max_pages must be at least 1".into(),
                    ));
                }
            }
        }

        if let Some(spec) = &self.deduplicate {
            if spec.enabled && spec.keys.iter().all(|key| key.trim().is_empty()) {
                return Err(AicrawlError::InvalidInput(
                    "deduplicate.keys must contain at least one field name".into(),
                ));
            }
        }

        if let Some(crawl) = &self.crawl {
            if crawl.max_pages == 0 {
                return Err(AicrawlError::InvalidInput(
                    "crawl.max_pages must be at least 1".into(),
                ));
            }
        }

        if let Some(schema) = &self.schema {
            if !schema.is_object() && !schema.is_boolean() {
                return Err(AicrawlError::InvalidInput(
                    "schema must be a JSON Schema object".into(),
                ));
            }
            crate::extraction::validate_schema_definition(schema)?;
        }

        validate_rules(&self.selectors, "selectors")?;
        Ok(())
    }
}

fn push_unique(list: &mut Vec<String>, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    if !list.iter().any(|existing| existing == value) {
        list.push(value.to_string());
    }
}

fn validate_url(raw: &str) -> Result<()> {
    let url = url::Url::parse(raw.trim())
        .map_err(|e| AicrawlError::InvalidInput(format!("invalid URL '{raw}': {e}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AicrawlError::InvalidInput(format!(
            "unsupported URL scheme '{}'; only http and https are accepted",
            url.scheme()
        )));
    }
    if url.host_str().unwrap_or("").is_empty() {
        return Err(AicrawlError::InvalidInput(format!(
            "URL '{raw}' has no host"
        )));
    }
    Ok(())
}

fn validate_selector_or_xpath(value: &str, path: &str) -> Result<()> {
    if value.trim_start().starts_with('/') {
        validate_xpath(value).map_err(|e| {
            AicrawlError::InvalidInput(format!("{path} is not a valid XPath expression: {e}"))
        })
    } else {
        validate_css(value).map_err(|e| {
            AicrawlError::InvalidInput(format!("{path} is not a valid CSS selector: {e}"))
        })
    }
}

fn validate_rules(rules: &BTreeMap<String, Rule>, path: &str) -> Result<()> {
    for (name, rule) in rules {
        if name.trim().is_empty() {
            return Err(AicrawlError::InvalidInput(format!(
                "{path} contains an empty field name"
            )));
        }
        let rule_path = format!("{path}.{name}");
        let has_locator = rule.selector.is_some() || rule.xpath.is_some();
        match rule.kind {
            RuleKind::Meta => {
                if rule.name.as_deref().unwrap_or("").trim().is_empty() && !has_locator {
                    return Err(AicrawlError::InvalidInput(format!(
                        "{rule_path}: meta rules require 'name' (or a selector)"
                    )));
                }
            }
            RuleKind::JsonLd => {}
            _ => {
                if !has_locator && !rule.has_fields() && rule.kind != RuleKind::Object {
                    return Err(AicrawlError::InvalidInput(format!(
                        "{rule_path}: rule requires 'selector' or 'xpath'"
                    )));
                }
            }
        }
        if rule.selector.is_some() && rule.xpath.is_some() {
            return Err(AicrawlError::InvalidInput(format!(
                "{rule_path}: use either 'selector' or 'xpath', not both"
            )));
        }
        if matches!(rule.kind, RuleKind::Attribute)
            && rule.attribute.as_deref().unwrap_or("").trim().is_empty()
        {
            return Err(AicrawlError::InvalidInput(format!(
                "{rule_path}: attribute rules require 'attribute'"
            )));
        }
        if let Some(selector) = rule.selector.as_deref() {
            validate_css(selector).map_err(|e| {
                AicrawlError::InvalidInput(format!(
                    "{rule_path}: invalid CSS selector '{selector}': {e}"
                ))
            })?;
        }
        if let Some(xpath) = rule.xpath.as_deref() {
            validate_xpath(xpath).map_err(|e| {
                AicrawlError::InvalidInput(format!("{rule_path}: invalid XPath '{xpath}': {e}"))
            })?;
        }
        if rule.multiple && rule.kind == RuleKind::Object {
            return Err(AicrawlError::InvalidInput(format!(
                "{rule_path}: object rules cannot be 'multiple'; use type 'list'"
            )));
        }
        validate_rules(&rule.fields, &format!("{rule_path}.fields"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Request {
        Request::from_json(raw).unwrap()
    }

    #[test]
    fn parses_the_documented_contract() {
        let request = parse(
            r#"{
              "url": "https://example.com/products",
              "action": "extract",
              "render": "auto",
              "selectors": {
                "products": {
                  "type": "list",
                  "selector": ".product-card",
                  "fields": {
                    "name": {"selector": ".product-title", "type": "text"},
                    "price": {"selector": ".price", "type": "text"},
                    "image": {"selector": "img", "attribute": "src"},
                    "link": {"selector": "a", "attribute": "href"}
                  }
                }
              },
              "pagination": {
                "enabled": true,
                "next_selector": "a.next",
                "max_pages": 10
              },
              "deduplicate": {"enabled": true, "keys": ["name"]},
              "output": {"format": "json", "pretty": true}
            }"#,
        );
        request.validate().unwrap();
        assert_eq!(request.action, Action::Extract);
        assert_eq!(request.render, RenderMode::Auto);
        let products = &request.selectors["products"];
        assert_eq!(products.kind, RuleKind::List);
        assert_eq!(products.fields.len(), 4);
        assert!(request.dedupe_spec().is_some());
        assert_eq!(request.targets(), vec!["https://example.com/products"]);
    }

    #[test]
    fn rejects_bad_requests() {
        assert!(Request::from_json("{").is_err());
        assert!(Request::from_json("{}").unwrap().validate().is_err());

        let missing_url = Request {
            urls: vec!["https://example.com".into()],
            ..Default::default()
        };
        assert!(missing_url.validate().is_err());

        let batch_without_urls = Request {
            action: Action::Batch,
            url: Some("https://example.com".into()),
            ..Default::default()
        };
        assert!(batch_without_urls.validate().is_err());

        let bad_version = Request {
            version: Some("2".into()),
            url: Some("https://example.com".into()),
            ..Default::default()
        };
        assert!(bad_version.validate().is_err());

        let bad_selector =
            parse(r#"{"url":"https://example.com","selectors":{"a":{"selector":"div["}}}"#);
        assert!(bad_selector.validate().is_err());

        let bad_xpath =
            parse(r#"{"url":"https://example.com","selectors":{"a":{"xpath":"//div["}}}"#);
        assert!(bad_xpath.validate().is_err());

        let missing_attribute = parse(
            r#"{"url":"https://example.com","selectors":{"a":{"selector":"a","type":"attribute"}}}"#,
        );
        assert!(missing_attribute.validate().is_err());

        let both = parse(
            r#"{"url":"https://example.com","selectors":{"a":{"selector":"a","xpath":"//a"}}}"#,
        );
        assert!(both.validate().is_err());
    }

    #[test]
    fn render_aliases_cover_common_spellings() {
        for (raw, expected) in [
            ("\"auto\"", RenderMode::Auto),
            ("\"off\"", RenderMode::Off),
            ("\"never\"", RenderMode::Off),
            ("\"always\"", RenderMode::On),
            ("true", RenderMode::On),
            ("false", RenderMode::Off),
        ] {
            let parsed: RenderMode = serde_json::from_str(raw).unwrap();
            assert_eq!(parsed, expected, "{raw}");
        }
    }

    #[test]
    fn transform_spec_accepts_one_or_many() {
        let one: TransformSpec = serde_json::from_str("\"collapse\"").unwrap();
        assert_eq!(one.list(), vec![Transform::CollapseWhitespace]);
        let many: TransformSpec = serde_json::from_str("[\"trim\",\"number\"]").unwrap();
        assert_eq!(many.list(), vec![Transform::Trim, Transform::Number]);
    }

    #[test]
    fn targets_are_unique_and_ordered() {
        let request = parse(
            r#"{"action":"batch","urls":["https://a.example","https://b.example","https://a.example"]}"#,
        );
        assert_eq!(
            request.targets(),
            vec!["https://a.example", "https://b.example"]
        );
    }
}

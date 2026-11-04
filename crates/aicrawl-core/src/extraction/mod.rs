//! Deterministic extraction: selector rules to JSON.
//!
//! Extraction never fabricates values. A missing optional field becomes `null`
//! (or its declared `default`) and a missing **required** field produces a
//! `SELECTOR_NOT_FOUND` issue that fails the request.

use std::collections::BTreeMap;

use scraper::{ElementRef, Selector};
use serde_json::{json, Map, Value};
use url::Url;

use crate::error::{AicrawlError, Result};
use crate::output::Issue;
use crate::parser::ParsedPage;
use crate::request::{Rule, RuleKind};
use crate::selectors::{self, Node};
use crate::transform;

/// Shared context for one page projection.
pub struct RuleContext<'a> {
    /// Effective document URL, used to resolve relative links.
    pub base: &'a Url,
    /// Document root element, used for document-wide lookups.
    pub document: ElementRef<'a>,
    /// Hard cap on items per list rule.
    pub max_items: usize,
}

/// Applies every named rule to the document.
pub fn apply_rules(
    root: &ElementRef<'_>,
    rules: &BTreeMap<String, Rule>,
    ctx: &RuleContext<'_>,
) -> (Map<String, Value>, Vec<Issue>) {
    let mut data = Map::new();
    let mut issues = Vec::new();
    for (name, rule) in rules {
        let (value, mut rule_issues) = extract_rule(*root, rule, name, ctx);
        issues.append(&mut rule_issues);
        data.insert(name.clone(), value);
    }
    (data, issues)
}

/// Applies one rule inside `scope`.
pub fn extract_rule(
    scope: ElementRef<'_>,
    rule: &Rule,
    path: &str,
    ctx: &RuleContext<'_>,
) -> (Value, Vec<Issue>) {
    let mut issues: Vec<Issue> = Vec::new();

    if rule.kind == RuleKind::Meta {
        let value = meta_value(ctx.document, rule.name.as_deref().unwrap_or(""));
        return finish_scalar(value, rule, path, ctx);
    }

    if rule.kind == RuleKind::JsonLd {
        let mut value = json_ld_value(scope, rule, ctx);
        apply_value_transforms(&mut value, rule, ctx, path, &mut issues);
        return (value, issues);
    }

    let matches = match selectors::select(&scope, rule.selector.as_deref(), rule.xpath.as_deref()) {
        Ok(nodes) => nodes,
        Err(err) => {
            issues.push(Issue::new(
                "INVALID_SELECTOR",
                path,
                err.to_string(),
                rule.required,
            ));
            return (default_value(rule), issues);
        }
    };

    if rule.kind == RuleKind::List || (rule.multiple && rule.has_fields()) {
        let limit = rule.max_items.unwrap_or(ctx.max_items).max(1);
        let mut items: Vec<Value> = Vec::new();
        for node in matches.iter().take(limit) {
            let item_scope = match node.element() {
                Some(element) => element,
                None => continue,
            };
            if rule.has_fields() {
                let (object, mut field_issues) = apply_rules(
                    &item_scope,
                    &rule.fields,
                    &RuleContext {
                        base: ctx.base,
                        document: ctx.document,
                        max_items: ctx.max_items,
                    },
                );
                // Field paths include the collection name for actionable errors.
                for issue in field_issues.iter_mut() {
                    issue.field = format!("{path}.{}.{}", items.len(), issue.field);
                }
                issues.append(&mut field_issues);
                items.push(Value::Object(object));
            } else {
                let (value, mut issues_here) = node_value(node, rule, ctx, path, false);
                issues.append(&mut issues_here);
                items.push(value);
            }
        }
        if items.is_empty() && rule.required {
            issues.push(Issue::not_found(path, true));
        }
        let value = if items.is_empty() {
            default_value(rule)
        } else {
            Value::Array(items)
        };
        return (value, issues);
    }

    if rule.kind == RuleKind::Object || (rule.has_fields() && !rule.multiple) {
        let item_scope = matches
            .first()
            .and_then(|node| node.element())
            .unwrap_or(scope);
        let (object, mut field_issues) = apply_rules(&item_scope, &rule.fields, ctx);
        for issue in field_issues.iter_mut() {
            issue.field = format!("{path}.{}", issue.field);
        }
        issues.append(&mut field_issues);
        if matches.is_empty() && rule.required {
            issues.push(Issue::not_found(path, true));
        }
        return (Value::Object(object), issues);
    }

    let wanted = if rule.multiple {
        matches.len()
    } else {
        usize::from(!matches.is_empty())
    };
    if wanted == 0 {
        return finish_scalar(None, rule, path, ctx);
    }

    if rule.multiple {
        let limit = rule.max_items.unwrap_or(ctx.max_items).max(1);
        let mut values = Vec::new();
        for node in matches.iter().take(limit) {
            let (value, mut node_issues) = node_value(node, rule, ctx, path, true);
            issues.append(&mut node_issues);
            values.push(value);
        }
        return (Value::Array(values), issues);
    }

    let (value, mut node_issues) = node_value(&matches[0], rule, ctx, path, true);
    issues.append(&mut node_issues);
    (value, issues)
}

fn finish_scalar(
    raw: Option<String>,
    rule: &Rule,
    path: &str,
    ctx: &RuleContext<'_>,
) -> (Value, Vec<Issue>) {
    let mut issues = Vec::new();
    let Some(raw) = raw else {
        if rule.required {
            issues.push(Issue::not_found(path, true));
        }
        return (default_value(rule), issues);
    };
    let (value, mut conversion_issues) = coerce(raw, rule, path, ctx);
    issues.append(&mut conversion_issues);
    if value.is_null() && rule.required {
        issues.push(Issue::not_found(path, true));
    }
    (value, issues)
}

fn node_value(
    node: &Node<'_>,
    rule: &Rule,
    ctx: &RuleContext<'_>,
    path: &str,
    report: bool,
) -> (Value, Vec<Issue>) {
    let mut issues = Vec::new();
    let raw = match rule.kind {
        RuleKind::Html => match node.element() {
            Some(element) => Some(selectors::inner_html(&element)),
            None => node.literal().map(str::to_string),
        },
        RuleKind::Attribute => {
            let name = rule.attribute.as_deref().unwrap_or("");
            match node.element() {
                Some(element) => selectors::attribute_of(&element, name),
                None => node.literal().map(str::to_string),
            }
        }
        RuleKind::Url => {
            let raw = match node.element() {
                Some(element) => match rule.attribute.as_deref() {
                    Some(name) => selectors::attribute_of(&element, name),
                    None => Some(selectors::text_of(&element)),
                },
                None => node.literal().map(str::to_string),
            };
            raw.map(|value| transform::absolutize(&value, ctx.base))
        }
        _ => match node.element() {
            Some(element) => Some(if rule.trim == Some(false) {
                selectors::raw_text_of(&element)
            } else {
                selectors::text_of(&element)
            }),
            None => node.literal().map(str::to_string),
        },
    };

    let Some(raw) = raw else {
        if rule.required && report {
            issues.push(Issue::not_found(path, true));
        }
        return (default_value(rule), issues);
    };
    let (value, mut conversion_issues) = coerce(raw, rule, path, ctx);
    if report {
        issues.append(&mut conversion_issues);
    }
    if value.is_null() && rule.required && report {
        issues.push(Issue::not_found(path, true));
    }
    (value, issues)
}

/// Applies transforms, absolute-URL resolution and type coercion.
fn coerce(raw: String, rule: &Rule, path: &str, ctx: &RuleContext<'_>) -> (Value, Vec<Issue>) {
    let mut issues = Vec::new();
    let mut transforms = rule.transforms();
    if rule.trim == Some(false) {
        transforms.retain(|transform| *transform != crate::request::Transform::Trim);
    }
    let base = Some(ctx.base);
    let mut text = transform::apply_all(&raw, &transforms, base);

    let wants_absolute = rule.absolute_url.unwrap_or(false)
        || rule.kind == RuleKind::Url
        || transforms
            .iter()
            .any(|transform| *transform == crate::request::Transform::AbsoluteUrl);
    if wants_absolute {
        text = transform::absolutize(&text, ctx.base);
    }

    let value = match rule.kind {
        RuleKind::Number | RuleKind::Integer => {
            let parsed = if rule.kind == RuleKind::Integer {
                transform::parse_integer(&text).map(|integer| json!(integer))
            } else {
                transform::parse_number(&text).map(|number| json!(number))
            };
            match parsed {
                Some(value) => value,
                None => {
                    issues.push(Issue::new(
                        "TYPE_CONVERSION_FAILED",
                        path,
                        format!(
                            "'{}' is not a valid {}",
                            truncate(&text, 120),
                            rule.kind.as_str()
                        ),
                        rule.required,
                    ));
                    Value::Null
                }
            }
        }
        RuleKind::Boolean => match transform::parse_bool(&text) {
            Some(value) => json!(value),
            None => {
                issues.push(Issue::new(
                    "TYPE_CONVERSION_FAILED",
                    path,
                    format!("'{}' is not a valid boolean", truncate(&text, 120)),
                    rule.required,
                ));
                Value::Null
            }
        },
        RuleKind::Html => Value::String(selectors::sanitize_html(&text)),
        _ => {
            if text.is_empty() {
                return (default_value(rule), issues);
            }
            Value::String(text)
        }
    };
    (value, issues)
}

fn apply_value_transforms(
    value: &mut Value,
    rule: &Rule,
    ctx: &RuleContext<'_>,
    path: &str,
    issues: &mut Vec<Issue>,
) {
    if rule.kind != RuleKind::JsonLd {
        return;
    }
    let transforms = rule.transforms();
    if transforms.is_empty() {
        return;
    }
    if let Some(array) = value.as_array_mut() {
        for item in array.iter_mut() {
            if let Some(text) = item.as_str().map(str::to_string) {
                *item = Value::String(transform::apply_all(&text, &transforms, Some(ctx.base)));
            }
        }
    } else if let Some(text) = value.as_str().map(str::to_string) {
        *value = Value::String(transform::apply_all(&text, &transforms, Some(ctx.base)));
    }
    let _ = path;
    let _ = issues;
}

fn default_value(rule: &Rule) -> Value {
    rule.default.clone().unwrap_or(Value::Null)
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        value.chars().take(max).collect::<String>() + "…"
    }
}

fn meta_value(document: ElementRef<'_>, name: &str) -> Option<String> {
    if name.trim().is_empty() {
        return None;
    }
    let selector = Selector::parse("meta").ok()?;
    for element in document.select(&selector) {
        let value = element.value();
        let matches = value.attr("name") == Some(name) || value.attr("property") == Some(name);
        if matches {
            if let Some(content) = value.attr("content") {
                let content = content.trim();
                if !content.is_empty() {
                    return Some(content.to_string());
                }
            }
        }
    }
    None
}

fn json_ld_value(scope: ElementRef<'_>, rule: &Rule, ctx: &RuleContext<'_>) -> Value {
    let mut blocks: Vec<String> = Vec::new();
    if rule.selector.is_some() || rule.xpath.is_some() {
        if let Ok(nodes) =
            selectors::select(&scope, rule.selector.as_deref(), rule.xpath.as_deref())
        {
            for node in nodes {
                match node.element() {
                    Some(element) => blocks.push(selectors::raw_text_of(&element)),
                    None => {
                        if let Some(text) = node.literal() {
                            blocks.push(text.to_string());
                        }
                    }
                }
            }
        }
    } else {
        let selector = match Selector::parse("script[type='application/ld+json']") {
            Ok(selector) => selector,
            Err(_) => return Value::Null,
        };
        for element in ctx.document.select(&selector) {
            blocks.push(selectors::raw_text_of(&element));
        }
    }

    let mut parsed: Vec<Value> = Vec::new();
    for block in blocks {
        let trimmed = block.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            match value {
                Value::Array(items) => parsed.extend(items),
                other => parsed.push(other),
            }
        }
    }
    match parsed.len() {
        0 => Value::Null,
        1 => parsed.remove(0),
        _ => Value::Array(parsed),
    }
}

/// Summary of a parsed document, used when no selector rules are supplied.
pub fn document_summary(parsed: &ParsedPage) -> Value {
    let metadata = serde_json::to_value(&parsed.meta).unwrap_or(Value::Null);
    let open_graph = open_graph(parsed);
    json!({
        "url": parsed.url,
        "title": parsed.title,
        "description": parsed.description,
        "lang": parsed.lang,
        "word_count": parsed.word_count,
        "truncated": parsed.truncated,
        "headings": parsed.headings,
        "links": parsed.links.iter().take(200).collect::<Vec<_>>(),
        "images": parsed.images.iter().take(50).collect::<Vec<_>>(),
        "tables": parsed.tables.iter().take(20).collect::<Vec<_>>(),
        "prices": parsed.prices.iter().take(50).collect::<Vec<_>>(),
        "json_ld": parsed.json_ld.iter().take(20).collect::<Vec<_>>(),
        "open_graph": open_graph,
        "metadata": metadata,
        "text": parsed.text,
        "markdown": parsed.markdown,
    })
}

fn open_graph(parsed: &ParsedPage) -> Map<String, Value> {
    let mut out = Map::new();
    let Ok(value) = serde_json::to_value(&parsed.meta) else {
        return out;
    };
    let Some(map) = value.as_object() else {
        return out;
    };
    for (key, value) in map {
        let lower = key.to_ascii_lowercase();
        if lower.starts_with("og:") || lower.starts_with("twitter:") {
            out.insert(lower, value.clone());
        }
    }
    out
}

/// Validates a JSON Schema document itself.
pub fn validate_schema_definition(schema: &Value) -> Result<()> {
    jsonschema::validator_for(schema)
        .map(|_| ())
        .map_err(|e| AicrawlError::InvalidInput(format!("invalid JSON Schema: {e}")))
}

/// Validates extracted data against a schema; returns readable errors.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::Request;
    use crate::selectors::parse_html_document;

    const SHOP: &str = r#"<!doctype html><html><head>
        <title>Shop</title>
        <meta name="description" content="A test shop">
        <meta property="og:title" content="Shop OG">
        <script type="application/ld+json">{"@type":"Product","name":"Widget","offers":{"price":"19.99","priceCurrency":"USD"}}</script>
      </head><body>
        <div class="product-card">
          <h2 class="title">Widget</h2>
          <span class="price">$1,299.00</span>
          <a href="/p/1">Details</a>
          <img src="/img/1.png">
          <p class="desc"> A <b>great</b> widget &amp; more </p>
        </div>
        <div class="product-card">
          <h2 class="title">Gadget</h2>
          <span class="price">$29.00</span>
          <a href="/p/2">Details</a>
        </div>
        <a class="next" href="/page/2">Next</a>
      </body></html>"#;

    fn with_document<T>(body: &str, f: impl FnOnce(ElementRef<'_>) -> T) -> T {
        let document = parse_html_document(body);
        f(document.root_element())
    }

    fn request(json: &str) -> Request {
        Request::from_json(json).unwrap()
    }

    #[test]
    fn list_of_objects_with_attributes_and_urls() {
        let request = request(
            r#"{
              "url": "https://example.com/products",
              "selectors": {
                "products": {
                  "type": "list",
                  "selector": ".product-card",
                  "fields": {
                    "name": {"selector": ".title"},
                    "price_text": {"selector": ".price"},
                    "price": {"selector": ".price", "type": "number", "transform": ["trim"]},
                    "image": {"selector": "img", "attribute": "src"},
                    "link": {"selector": "a", "type": "url"}
                  }
                }
              }
            }"#,
        );
        with_document(SHOP, |root| {
            let base = Url::parse("https://example.com/products").unwrap();
            let ctx = RuleContext {
                base: &base,
                document: root,
                max_items: 100,
            };
            let (data, issues) = apply_rules(&root, &request.selectors, &ctx);
            assert!(issues.is_empty(), "{issues:?}");
            let products = data["products"].as_array().unwrap();
            assert_eq!(products.len(), 2);
            assert_eq!(products[0]["name"], "Widget");
            assert_eq!(products[0]["price_text"], "$1,299.00");
            assert_eq!(products[0]["price"], 1299.0);
            assert_eq!(products[0]["image"], "/img/1.png");
            assert_eq!(products[0]["link"], "https://example.com/p/1");
            assert_eq!(products[1]["name"], "Gadget");
        });
    }

    #[test]
    fn nested_xpath_and_meta_and_json_ld() {
        let request = request(
            r#"{
              "url": "https://example.com/products",
              "selectors": {
                "first": {
                  "type": "object",
                  "xpath": "//div[@class='product-card'][1]",
                  "fields": {
                    "title": {"xpath": ".//h2/text()"},
                    "href": {"xpath": ".//a/@href", "absolute_url": true}
                  }
                },
                "description": {"type": "meta", "name": "description"},
                "og_title": {"type": "meta", "name": "og:title"},
                "product": {"type": "json_ld"}
              }
            }"#,
        );
        with_document(SHOP, |root| {
            let base = Url::parse("https://example.com/products").unwrap();
            let ctx = RuleContext {
                base: &base,
                document: root,
                max_items: 100,
            };
            let (data, issues) = apply_rules(&root, &request.selectors, &ctx);
            assert!(issues.is_empty(), "{issues:?}");
            assert_eq!(data["description"], "A test shop");
            assert_eq!(data["og_title"], "Shop OG");
            assert_eq!(data["product"]["name"], "Widget");
            assert_eq!(data["first"]["title"], "Widget");
            assert_eq!(data["first"]["href"], "https://example.com/p/1");
        });
    }

    #[test]
    fn required_missing_field_is_reported() {
        let request = request(
            r#"{
              "url": "https://example.com",
              "selectors": {
                "name": {"selector": ".does-not-exist", "required": true},
                "optional": {"selector": ".also-missing"},
                "fallback": {"selector": ".nope", "default": "unknown"}
              }
            }"#,
        );
        with_document(SHOP, |root| {
            let base = Url::parse("https://example.com").unwrap();
            let ctx = RuleContext {
                base: &base,
                document: root,
                max_items: 10,
            };
            let (data, issues) = apply_rules(&root, &request.selectors, &ctx);
            assert_eq!(data["name"], Value::Null);
            assert_eq!(data["optional"], Value::Null);
            assert_eq!(data["fallback"], "unknown");
            assert_eq!(issues.len(), 1);
            assert_eq!(issues[0].code, "SELECTOR_NOT_FOUND");
            assert_eq!(issues[0].field, "name");
            assert!(issues[0].required);
        });
    }

    #[test]
    fn html_rule_is_sanitized_and_whitespace_is_normalised() {
        let request = request(
            r#"{
              "url": "https://example.com",
              "selectors": {
                "html": {"selector": ".desc", "type": "html"},
                "text": {"selector": ".desc"}
              }
            }"#,
        );
        with_document(SHOP, |root| {
            let base = Url::parse("https://example.com").unwrap();
            let ctx = RuleContext {
                base: &base,
                document: root,
                max_items: 10,
            };
            let (data, _) = apply_rules(&root, &request.selectors, &ctx);
            assert_eq!(data["text"], "A great widget & more");
            let html = data["html"].as_str().unwrap();
            assert!(html.contains("<b>great</b>"));
            assert!(!html.contains("<script"));
        });
    }

    #[test]
    fn scalar_list_and_transforms() {
        let request = request(
            r#"{
              "url": "https://example.com",
              "selectors": {
                "names": {"type": "list", "selector": ".title"},
                "lower": {"selector": ".title", "transform": ["lowercase"]},
                "bad_number": {"selector": ".title", "type": "number"}
              }
            }"#,
        );
        with_document(SHOP, |root| {
            let base = Url::parse("https://example.com").unwrap();
            let ctx = RuleContext {
                base: &base,
                document: root,
                max_items: 10,
            };
            let (data, issues) = apply_rules(&root, &request.selectors, &ctx);
            assert_eq!(data["names"], json!(["Widget", "Gadget"]));
            assert_eq!(data["lower"], "widget");
            assert_eq!(data["bad_number"], Value::Null);
            assert_eq!(issues[0].code, "TYPE_CONVERSION_FAILED");
        });
    }
}

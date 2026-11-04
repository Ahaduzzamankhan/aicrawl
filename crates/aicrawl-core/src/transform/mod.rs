//! Value transforms: whitespace, HTML stripping, numbers, booleans and URLs.
//!
//! Transforms never invent data: a value that cannot be converted becomes
//! `null` and the caller records a `TYPE_CONVERSION_FAILED` issue.

use serde_json::Value;
use url::Url;

use crate::request::Transform;
use crate::selectors::collapse;

/// Applies every transform in order to a raw string.
pub fn apply_all(value: &str, transforms: &[Transform], base: Option<&Url>) -> String {
    let mut current = value.to_string();
    for transform in transforms {
        current = apply(&current, *transform, base);
    }
    current
}

/// Applies a single transform.
pub fn apply(value: &str, transform: Transform, base: Option<&Url>) -> String {
    match transform {
        Transform::Trim => value.trim().to_string(),
        Transform::CollapseWhitespace => collapse(value),
        Transform::Lowercase => value.to_lowercase(),
        Transform::Uppercase => value.to_uppercase(),
        Transform::StripHtml => strip_html(value),
        Transform::AbsoluteUrl => match base {
            Some(base) => absolutize(value, base),
            None => value.to_string(),
        },
        // Numeric/boolean transforms are applied during type conversion; keep
        // the raw text intact so no digits are lost.
        Transform::Number | Transform::Integer | Transform::Boolean => value.to_string(),
    }
}

/// Resolves `raw` against `base`, keeping already-absolute URLs unchanged.
pub fn absolutize(raw: &str, base: &Url) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match base.join(trimmed) {
        Ok(url) => url.to_string(),
        Err(_) => trimmed.to_string(),
    }
}

/// Removes tags and decodes the entity subset that appears in extracted text.
pub fn strip_html(input: &str) -> String {
    use std::sync::OnceLock;
    static TAGS: OnceLock<regex::Regex> = OnceLock::new();
    let tags = TAGS.get_or_init(|| regex::Regex::new(r"(?s)<[^>]*>").expect("static regex"));
    decode_entities(&tags.replace_all(input, " "))
}

/// Decodes the common named and numeric HTML entities.
pub fn decode_entities(input: &str) -> String {
    use std::sync::OnceLock;
    static NUMERIC: OnceLock<regex::Regex> = OnceLock::new();
    let numeric =
        NUMERIC.get_or_init(|| regex::Regex::new(r"&#(x?[0-9A-Fa-f]+);").expect("static regex"));

    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(index) = rest.find('&') {
        out.push_str(&rest[..index]);
        rest = &rest[index..];
        let Some(end) = rest.find(';') else {
            out.push_str(rest);
            return out;
        };
        let entity = &rest[1..end];
        let replacement = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            "ndash" => Some('-'),
            "mdash" => Some('-'),
            "hellip" => Some('.'),
            "euro" => Some('E'),
            _ => None,
        };
        if let Some(ch) = replacement {
            out.push(ch);
            rest = &rest[end + 1..];
            continue;
        }
        if let Some(captures) = numeric.captures(rest) {
            if captures.get(0).map(|m| m.end()) == Some(end + 1) {
                let digits = captures.get(1).map(|m| m.as_str()).unwrap_or("");
                let code = if let Some(hex) = digits
                    .strip_prefix('x')
                    .or_else(|| digits.strip_prefix('X'))
                {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    digits.parse::<u32>().ok()
                };
                if let Some(ch) = code.and_then(char::from_u32) {
                    out.push(ch);
                    rest = &rest[end + 1..];
                    continue;
                }
            }
        }
        out.push('&');
        rest = &rest[1..];
    }
    out.push_str(rest);
    collapse(&out)
}

/// Parses a human-formatted number: `$1,299.00`, `1.299,00 €`, `42`.
pub fn parse_number(raw: &str) -> Option<f64> {
    let mut cleaned = String::with_capacity(raw.len());
    let mut last_digit = false;
    for ch in raw.chars() {
        match ch {
            '0'..='9' => {
                cleaned.push(ch);
                last_digit = true;
            }
            '.' | ',' => {
                cleaned.push(ch);
                last_digit = false;
            }
            '-' if cleaned.is_empty() => cleaned.push('-'),
            _ => {
                if last_digit {
                    last_digit = false;
                }
            }
        }
    }
    if cleaned.is_empty() || cleaned == "-" {
        return None;
    }
    // Decide which separator is the decimal one: the last of them when it has
    // one or two trailing digits, otherwise treat it as a thousands separator.
    let last_dot = cleaned.rfind('.');
    let last_comma = cleaned.rfind(',');
    let decimal = match (last_dot, last_comma) {
        (Some(dot), Some(comma)) => Some(dot.max(comma)),
        (Some(dot), None) => Some(dot),
        (None, Some(comma)) => Some(comma),
        (None, None) => None,
    };
    let normalized = match decimal {
        Some(position) => {
            let trailing = cleaned.len() - position - 1;
            let integer: String = cleaned[..position]
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == '-')
                .collect();
            if trailing == 0 {
                integer
            } else if trailing <= 2 {
                let fraction = &cleaned[position + 1..];
                format!(
                    "{integer}.{}",
                    fraction
                        .chars()
                        .filter(char::is_ascii_digit)
                        .collect::<String>()
                )
            } else {
                cleaned.chars().filter(char::is_ascii_digit).collect()
            }
        }
        None => cleaned.clone(),
    };
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

/// Parses an integer, truncating any fractional part.
pub fn parse_integer(raw: &str) -> Option<i64> {
    parse_number(raw).map(|value| value.trunc() as i64)
}

/// Parses common truthy/falsy spellings.
pub fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "y" | "1" | "on" | "in stock" | "available" => Some(true),
        "false" | "no" | "n" | "0" | "off" | "out of stock" | "unavailable" => Some(false),
        _ => None,
    }
}

/// Symbolic currency found in a string, if any.
pub fn detect_currency(raw: &str) -> Option<&'static str> {
    for (symbol, code) in [
        ("$", "USD"),
        ("€", "EUR"),
        ("£", "GBP"),
        ("¥", "JPY"),
        ("₹", "INR"),
    ] {
        if raw.contains(symbol) {
            return Some(code);
        }
    }
    let upper = raw.to_ascii_uppercase();
    for code in ["USD", "EUR", "GBP", "JPY", "INR", "CAD", "AUD"] {
        if upper.contains(code) {
            return Some(code);
        }
    }
    None
}

/// Converts a JSON scalar to a string for field-key based de-duplication.
pub fn value_key(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://example.com/shop/index.html").unwrap()
    }

    #[test]
    fn parses_numbers_in_common_formats() {
        assert_eq!(parse_number("$1,299.00"), Some(1299.0));
        assert_eq!(parse_number("1.299,50 €"), Some(1299.5));
        assert_eq!(parse_number("42"), Some(42.0));
        assert_eq!(parse_number("-7.5%"), Some(-7.5));
        assert_eq!(parse_number("free"), None);
        assert_eq!(parse_integer("19.99"), Some(19));
    }

    #[test]
    fn parses_booleans() {
        assert_eq!(parse_bool(" Yes "), Some(true));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("maybe"), None);
    }

    #[test]
    fn resolves_urls() {
        assert_eq!(absolutize("/p/1", &base()), "https://example.com/p/1");
        assert_eq!(absolutize("../about", &base()), "https://example.com/about");
        assert_eq!(
            absolutize("https://other.example/x", &base()),
            "https://other.example/x"
        );
    }

    #[test]
    fn strips_tags_and_decodes_entities() {
        assert_eq!(strip_html("<b>Tom</b> &amp; Jerry&nbsp;"), "Tom & Jerry");
        assert_eq!(decode_entities("5 &lt; 6 &#65;"), "5 < 6 A");
    }

    #[test]
    fn applies_transforms_in_order() {
        let value = apply_all(
            "  $1,299.00 ",
            &[
                Transform::Trim,
                Transform::CollapseWhitespace,
                Transform::Number,
            ],
            Some(&base()),
        );
        assert_eq!(value, "$1,299.00");
        assert_eq!(parse_number(&value), Some(1299.0));
        assert_eq!(detect_currency(&value), Some("USD"));
    }
}

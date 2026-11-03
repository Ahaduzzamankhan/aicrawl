//! Small shared helpers.

use chrono::{SecondsFormat, Utc};
use sha2::{Digest, Sha256};

/// Current UTC timestamp in RFC 3339 with milliseconds.
pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// RFC 3339 timestamp `secs` in the future (negative values go to the past).
pub fn rfc3339_offset(secs: i64) -> String {
    let target = Utc::now() + chrono::Duration::seconds(secs);
    target.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Whether an RFC 3339 timestamp lies in the past.
pub fn is_expired(timestamp: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|t| t.with_timezone(&Utc) <= Utc::now())
        .unwrap_or(true)
}

/// New random identifier with a readable prefix.
pub fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

/// Hex-encoded SHA-256 of a byte slice.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Collapses runs of whitespace and trims the result.
pub fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_space && !out.is_empty() {
                out.push(' ');
            }
            last_space = true;
        } else if !ch.is_control() {
            out.push(ch);
            last_space = false;
        }
    }
    out.trim().to_string()
}

/// Truncates text on a char boundary and appends an ellipsis marker.
pub fn truncate_chars(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() <= max_chars {
        return (text.to_string(), false);
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push('…');
    (out, true)
}

/// Formats a byte count for terminal/UI display.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_is_boundary_safe() {
        let text = "héllo wörld";
        let (out, truncated) = truncate_chars(text, 5);
        assert!(truncated);
        assert!(out.starts_with("héllo"));
    }

    #[test]
    fn whitespace_collapse_keeps_words() {
        assert_eq!(collapse_whitespace(" a \n\t b   c "), "a b c");
    }

    #[test]
    fn hashes_are_stable() {
        assert_eq!(sha256_hex(b"x"), sha256_hex(b"x"));
        assert_eq!(sha256_hex(b"x").len(), 64);
    }
}

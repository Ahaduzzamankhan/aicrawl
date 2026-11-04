//! Response contract shared by the CLI, REST API and MCP tools.
//!
//! Success:
//! ```json
//! {"success": true, "data": {}, "meta": {...}}
//! ```
//! Failure:
//! ```json
//! {"success": false, "data": null, "error": {"code": "...", "message": "...", "retryable": false}}
//! ```
//! Field-level problems are reported in `errors` so callers can distinguish a
//! missing optional value from a required-field failure.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AicrawlError;

/// One field-level issue produced while extracting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Issue {
    /// Stable machine-readable code, e.g. `SELECTOR_NOT_FOUND`.
    pub code: String,
    /// Field path, e.g. `products.price`.
    pub field: String,
    /// Human-readable explanation.
    pub message: String,
    /// Whether the value was required.
    #[serde(default)]
    pub required: bool,
}

impl Issue {
    /// Builds an issue.
    pub fn new(
        code: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
        required: bool,
    ) -> Self {
        Self {
            code: code.into(),
            field: field.into(),
            message: message.into(),
            required,
        }
    }

    /// Missing selector/value for a field.
    pub fn not_found(field: &str, required: bool) -> Self {
        Self::new(
            "SELECTOR_NOT_FOUND",
            field,
            format!("no value matched for field '{field}'"),
            required,
        )
    }
}

/// Fatal error body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl From<&AicrawlError> for ErrorBody {
    fn from(err: &AicrawlError) -> Self {
        Self {
            code: err.code().to_string(),
            message: err.to_string(),
            retryable: err.retryable(),
        }
    }
}

/// Response metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    /// Requested URL (single-page actions) or the batch root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Final URL after redirects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_url: Option<String>,
    /// HTTP status of the primary page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default)]
    pub pages_processed: u32,
    #[serde(default)]
    pub items_extracted: usize,
    /// Stored job identifier, when the request was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    /// Stored result identifier, when a result row was written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    #[serde(default)]
    pub duration_ms: u64,
    /// `http` or `playwright`.
    #[serde(default)]
    pub render: String,
    #[serde(default)]
    pub cached: bool,
    #[serde(default)]
    pub truncated: bool,
    /// Projection mode: `rules` or `document`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub projection: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<PageMeta>,
}

/// Per-page metadata for crawl and batch responses.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageMeta {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default)]
    pub items_extracted: usize,
    #[serde(default)]
    pub changed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

/// The single response type every interface returns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub success: bool,
    pub data: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<Issue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl Response {
    /// Successful response with metadata.
    pub fn ok(data: Value, meta: Meta) -> Self {
        Self {
            success: true,
            data,
            meta: Some(meta),
            errors: Vec::new(),
            error: None,
        }
    }

    /// Successful response that also carries field-level issues.
    pub fn partial(data: Value, meta: Meta, errors: Vec<Issue>) -> Self {
        Self {
            success: true,
            data,
            meta: Some(meta),
            errors,
            error: None,
        }
    }

    /// Fatal failure with no data.
    pub fn failure(err: &AicrawlError) -> Self {
        Self {
            success: false,
            data: Value::Null,
            meta: None,
            errors: Vec::new(),
            error: Some(ErrorBody::from(err)),
        }
    }

    /// Failure with field-level detail (for example a required-selector miss).
    pub fn failure_with_issues(
        code: &str,
        message: impl Into<String>,
        retryable: bool,
        issues: Vec<Issue>,
    ) -> Self {
        Self {
            success: false,
            data: Value::Null,
            meta: None,
            errors: issues,
            error: Some(ErrorBody {
                code: code.to_string(),
                message: message.into(),
                retryable,
            }),
        }
    }

    /// Serializes the response as pretty or compact JSON.
    pub fn to_json(&self, pretty: bool) -> String {
        if pretty {
            serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
        } else {
            serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
        }
    }
}

/// Counts extracted items: every element of top-level arrays, otherwise the
/// number of non-null scalar/object values.
pub fn count_items(data: &Value) -> usize {
    let Some(map) = data.as_object() else {
        return usize::from(!data.is_null());
    };
    let mut total = 0;
    let mut scalars = 0;
    for (key, value) in map {
        if key.starts_with('_') {
            continue;
        }
        match value {
            Value::Array(items) => total += items.len(),
            Value::Null => {}
            _ => scalars += 1,
        }
    }
    if total == 0 {
        scalars
    } else {
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn success_shape_omits_error() {
        let response = Response::ok(json!({"a": 1}), Meta::default());
        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(value["success"], true);
        assert!(value.get("error").is_none());
        assert!(value.get("errors").is_none());
        assert_eq!(value["data"]["a"], 1);
    }

    #[test]
    fn failure_shape_has_null_data() {
        let err = AicrawlError::Blocked("loopback".into());
        let response = Response::failure(&err);
        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(value["success"], false);
        assert_eq!(value["data"], Value::Null);
        assert_eq!(value["error"]["code"], "BLOCKED_BY_POLICY");
        assert_eq!(value["error"]["retryable"], false);
    }

    #[test]
    fn required_failure_carries_issues() {
        let response = Response::failure_with_issues(
            "FIELD_REQUIRED",
            "required field missing",
            false,
            vec![Issue::not_found("products.name", true)],
        );
        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(value["error"]["code"], "FIELD_REQUIRED");
        assert_eq!(value["errors"][0]["code"], "SELECTOR_NOT_FOUND");
        assert_eq!(value["errors"][0]["required"], true);
    }

    #[test]
    fn item_counting_uses_lists() {
        assert_eq!(count_items(&json!({"a": [1, 2, 3], "b": "x"})), 3);
        assert_eq!(count_items(&json!({"b": "x", "c": null})), 1);
        assert_eq!(count_items(&json!({"a": [], "b": "x"})), 1);
    }
}

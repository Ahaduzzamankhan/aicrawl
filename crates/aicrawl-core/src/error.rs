//! Error type shared by the CLI, REST API, MCP server and core engine.
//!
//! AICrawl has never had a model in the request path, and this release removes
//! the optional provider integration entirely: every failure is either a
//! request, network, policy, extraction or storage problem.

use thiserror::Error;

/// Result alias used across AICrawl.
pub type Result<T> = std::result::Result<T, AicrawlError>;

/// All failures produced by the AICrawl engine.
#[derive(Debug, Error)]
pub enum AicrawlError {
    /// The caller supplied an invalid argument (URL, selector, schema, limit, ...).
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// A request was rejected by the SSRF / domain / robots policy.
    #[error("blocked by security policy: {0}")]
    Blocked(String),

    /// Transport or DNS failure.
    #[error("network error: {0}")]
    Network(String),

    /// Remote server answered with an unsuccessful status code.
    #[error("remote server returned HTTP {status} for {url}")]
    Http { status: u16, url: String },

    /// The request exceeded its deadline.
    #[error("request timed out after {0} ms")]
    Timeout(u64),

    /// robots.txt does not allow the path.
    #[error("robots.txt disallows fetching {0}")]
    RobotsDisallowed(String),

    /// The response body exceeded the configured maximum size.
    #[error("response body exceeded the {limit} byte limit")]
    BodyTooLarge { limit: usize },

    /// The response could not be interpreted as a document AICrawl understands.
    #[error("unsupported content type: {0}")]
    UnsupportedContent(String),

    /// Deterministic extraction failed.
    #[error("extraction failed: {0}")]
    Extraction(String),

    /// JSON Schema validation failed.
    #[error("schema validation failed: {0}")]
    Schema(String),

    /// A job, result, page or key could not be found.
    #[error("not found: {0}")]
    NotFound(String),

    /// SQLite failure.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Filesystem failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid or unreadable configuration.
    #[error("configuration error: {0}")]
    Config(String),

    /// Playwright rendering was requested but the optional bridge is unavailable.
    #[error("browser rendering unavailable: {0}")]
    BrowserUnavailable(String),

    /// Authentication is required and missing or invalid.
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// Rate limit exceeded.
    #[error("rate limit exceeded: {0}")]
    RateLimited(String),

    /// Any other failure.
    #[error("{0}")]
    Other(String),
}

impl AicrawlError {
    /// Stable machine-readable error code used by the REST API and MCP server.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "INVALID_REQUEST",
            Self::Blocked(_) => "BLOCKED_BY_POLICY",
            Self::Network(_) => "NETWORK_ERROR",
            Self::Http { .. } => "HTTP_ERROR",
            Self::Timeout(_) => "TIMEOUT",
            Self::RobotsDisallowed(_) => "ROBOTS_DISALLOWED",
            Self::BodyTooLarge { .. } => "BODY_TOO_LARGE",
            Self::UnsupportedContent(_) => "UNSUPPORTED_CONTENT",
            Self::Extraction(_) => "EXTRACTION_FAILED",
            Self::Schema(_) => "SCHEMA_VALIDATION_FAILED",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Database(_) => "DATABASE_ERROR",
            Self::Io(_) => "IO_ERROR",
            Self::Json(_) => "JSON_ERROR",
            Self::Config(_) => "CONFIG_ERROR",
            Self::BrowserUnavailable(_) => "RENDER_UNAVAILABLE",
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::RateLimited(_) => "RATE_LIMITED",
            Self::Other(_) => "INTERNAL_ERROR",
        }
    }

    /// HTTP status code that best represents this error.
    pub fn http_status(&self) -> u16 {
        match self {
            Self::InvalidInput(_) | Self::Json(_) => 400,
            Self::Unauthorized(_) => 401,
            Self::Blocked(_) | Self::RobotsDisallowed(_) => 403,
            Self::NotFound(_) => 404,
            Self::Http { status, .. } => {
                if (400..500).contains(status) {
                    *status
                } else {
                    502
                }
            }
            Self::Timeout(_) => 504,
            Self::RateLimited(_) => 429,
            Self::Schema(_) => 422,
            Self::BrowserUnavailable(_) => 503,
            Self::BodyTooLarge { .. } => 413,
            Self::Network(_) | Self::UnsupportedContent(_) => 502,
            _ => 500,
        }
    }

    /// Whether the failure is transient and worth retrying.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::Timeout(_) | Self::RateLimited(_)
        ) || matches!(self, Self::Http { status, .. } if *status >= 500)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_statuses_are_stable() {
        let cases: Vec<(AicrawlError, &str, u16)> = vec![
            (
                AicrawlError::InvalidInput("bad selector".into()),
                "INVALID_REQUEST",
                400,
            ),
            (
                AicrawlError::Blocked("loopback".into()),
                "BLOCKED_BY_POLICY",
                403,
            ),
            (
                AicrawlError::RobotsDisallowed("https://x/".into()),
                "ROBOTS_DISALLOWED",
                403,
            ),
            (
                AicrawlError::Schema("missing field".into()),
                "SCHEMA_VALIDATION_FAILED",
                422,
            ),
            (
                AicrawlError::BrowserUnavailable("no bridge".into()),
                "RENDER_UNAVAILABLE",
                503,
            ),
            (AicrawlError::NotFound("job".into()), "NOT_FOUND", 404),
        ];
        for (error, code, status) in cases {
            assert_eq!(error.code(), code);
            assert_eq!(error.http_status(), status);
        }
    }

    #[test]
    fn retryable_is_transport_only() {
        assert!(AicrawlError::Timeout(1000).retryable());
        assert!(AicrawlError::Http {
            status: 503,
            url: "https://x/".into()
        }
        .retryable());
        assert!(!AicrawlError::InvalidInput("x".into()).retryable());
        assert!(!AicrawlError::Http {
            status: 404,
            url: "https://x/".into()
        }
        .retryable());
    }
}

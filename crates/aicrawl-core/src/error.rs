//! Error type shared by the CLI, REST API, MCP server and core engine.

use thiserror::Error;

/// Result alias used across AICrawl.
pub type Result<T> = std::result::Result<T, AicrawlError>;

/// All failures produced by the AICrawl engine.
#[derive(Debug, Error)]
pub enum AicrawlError {
    /// The caller supplied an invalid argument (URL, schema, port, ...).
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

    /// Deterministic or AI extraction failed.
    #[error("extraction failed: {0}")]
    Extraction(String),

    /// JSON Schema validation failed.
    #[error("schema validation failed: {0}")]
    Schema(String),

    /// The configured AI provider returned an error.
    #[error("ai provider error: {0}")]
    Ai(String),

    /// No AI provider is configured, so semantic extraction is unavailable.
    #[error("no AI provider is configured; configure one with `aicrawl config` or the dashboard")]
    AiNotConfigured,

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
            Self::InvalidInput(_) => "invalid_input",
            Self::Blocked(_) => "blocked_by_policy",
            Self::Network(_) => "network_error",
            Self::Http { .. } => "http_error",
            Self::Timeout(_) => "timeout",
            Self::RobotsDisallowed(_) => "robots_disallowed",
            Self::BodyTooLarge { .. } => "body_too_large",
            Self::UnsupportedContent(_) => "unsupported_content",
            Self::Extraction(_) => "extraction_failed",
            Self::Schema(_) => "schema_validation_failed",
            Self::Ai(_) => "ai_provider_error",
            Self::AiNotConfigured => "ai_not_configured",
            Self::NotFound(_) => "not_found",
            Self::Database(_) => "database_error",
            Self::Io(_) => "io_error",
            Self::Json(_) => "json_error",
            Self::Config(_) => "config_error",
            Self::BrowserUnavailable(_) => "browser_unavailable",
            Self::Unauthorized(_) => "unauthorized",
            Self::RateLimited(_) => "rate_limited",
            Self::Other(_) => "internal_error",
        }
    }

    /// HTTP status code that best represents this error.
    pub fn http_status(&self) -> u16 {
        match self {
            Self::InvalidInput(_) | Self::Json(_) | Self::Schema(_) => 400,
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
            Self::AiNotConfigured | Self::BrowserUnavailable(_) => 503,
            Self::BodyTooLarge { .. } => 413,
            Self::Ai(_) | Self::Network(_) | Self::UnsupportedContent(_) => 502,
            _ => 500,
        }
    }

    /// Whether the failure is transient and worth retrying.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::Timeout(_) | Self::RateLimited(_) | Self::Ai(_)
        ) || matches!(self, Self::Http { status, .. } if *status >= 500)
    }
}

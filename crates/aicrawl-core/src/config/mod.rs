//! Configuration loading, defaults and environment overrides.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AicrawlError, Result};

/// The user's home directory (`USERPROFILE` on Windows, `HOME` elsewhere).
pub fn home_dir() -> PathBuf {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Directory that holds the database, key material and config (`~/.aicrawl`).
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = env::var("AICRAWL_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = env::var("AICRAWL_HOME") {
        return PathBuf::from(dir);
    }
    home_dir().join(".aicrawl")
}

/// Canonical location of the config file for a data directory.
pub fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("config.toml")
}

/// Creates the data directory when missing and returns it.
pub fn ensure_data_dir(data_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(data_dir)
        .map_err(|e| AicrawlError::Config(format!("cannot create {}: {e}", data_dir.display())))?;
    Ok(data_dir.to_path_buf())
}

/// Top-level configuration. Every section has defaults and may be omitted in TOML.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub crawler: CrawlerConfig,
    pub security: SecurityConfig,
    pub extract: ExtractConfig,
    pub storage: StorageConfig,
    pub cache: CacheConfig,
    pub api: ApiConfig,
}

/// Listener settings for the REST API/dashboard and the MCP HTTP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub api_host: String,
    pub api_port: u16,
    pub mcp_host: String,
    pub mcp_port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            api_host: "127.0.0.1".into(),
            api_port: 8080,
            mcp_host: "127.0.0.1".into(),
            mcp_port: 8787,
        }
    }
}

/// Crawl limits and HTTP client behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CrawlerConfig {
    pub max_pages: u32,
    pub max_depth: u32,
    pub concurrency: usize,
    pub timeout_ms: u64,
    /// Timeout for the optional Playwright bridge.
    pub render_timeout_ms: u64,
    pub max_body_bytes: usize,
    pub max_text_chars: usize,
    pub max_crawl_secs: u64,
    pub user_agent: String,
    pub respect_robots_txt: bool,
    pub same_domain: bool,
}

impl Default for CrawlerConfig {
    fn default() -> Self {
        Self {
            max_pages: 10,
            max_depth: 2,
            concurrency: 4,
            timeout_ms: 15_000,
            render_timeout_ms: 30_000,
            max_body_bytes: 3 * 1024 * 1024,
            max_text_chars: 60_000,
            max_crawl_secs: 180,
            user_agent: format!(
                "AICrawl/{} (+https://github.com/Ahaduzzamankhan/aicrawl; AI extraction bot)",
                crate::VERSION
            ),
            respect_robots_txt: true,
            same_domain: true,
        }
    }
}

/// SSRF, domain and rate-limit policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    /// Allow loopback/private network targets. Off by default; enable only for trusted self-hosting.
    pub allow_private_networks: bool,
    /// If non-empty, only these domains (and their subdomains) may be crawled.
    pub allowed_domains: Vec<String>,
    /// Domains (and subdomains) that are always refused.
    pub blocked_domains: Vec<String>,
    pub max_redirects: u32,
    pub rate_limit_per_minute: u32,
    pub allow_http: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            allow_private_networks: false,
            allowed_domains: Vec::new(),
            blocked_domains: Vec::new(),
            max_redirects: 5,
            rate_limit_per_minute: 60,
            allow_http: true,
        }
    }
}

/// Extraction limits applied to every request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtractConfig {
    /// Hard cap on items produced by one list rule.
    pub max_items: usize,
    /// Maximum characters of page text kept in memory.
    pub max_text_chars: usize,
}

impl Default for ExtractConfig {
    fn default() -> Self {
        Self {
            max_items: 1000,
            max_text_chars: 60_000,
        }
    }
}

/// Storage locations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StorageConfig {
    /// Overrides the default `<data_dir>/aicrawl.db` location.
    pub database_path: Option<PathBuf>,
    pub max_connections: u32,
}

/// Page cache settings (metadata and bodies live in SQLite).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    pub enabled: bool,
    pub ttl_secs: u64,
    pub max_entries: u32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_secs: 3600,
            max_entries: 500,
        }
    }
}

/// REST API and dashboard serving options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiConfig {
    pub require_api_key: bool,
    pub cors_origins: Vec<String>,
    pub dashboard_dist: Option<PathBuf>,
    pub request_timeout_secs: u64,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            require_api_key: false,
            cors_origins: vec![
                "http://localhost:5173".into(),
                "http://127.0.0.1:5173".into(),
            ],
            dashboard_dist: None,
            request_timeout_secs: 120,
        }
    }
}

impl AppConfig {
    /// Loads a config file, returning defaults when it does not exist.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)
            .map_err(|e| AicrawlError::Config(format!("cannot read {}: {e}", path.display())))?;
        let mut config: Self = toml::from_str(&raw).map_err(|e| {
            AicrawlError::Config(format!("invalid TOML in {}: {e}", path.display()))
        })?;
        config.apply_env();
        Ok(config)
    }

    /// Loads from an explicit path or the default data directory, then applies env overrides.
    pub fn load(explicit: Option<&Path>) -> Result<Self> {
        let path = match explicit {
            Some(path) => path.to_path_buf(),
            None => config_path(&data_dir()),
        };
        Self::load_or_default(&path)
    }

    /// Writes the config as TOML, creating parent directories.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(self)
            .map_err(|e| AicrawlError::Config(format!("cannot serialize config: {e}")))?;
        fs::write(path, raw)?;
        Ok(())
    }

    /// Applies `AICRAWL_*` environment overrides.
    pub fn apply_env(&mut self) {
        fn var(name: &str) -> Option<String> {
            env::var(name).ok().filter(|value| !value.trim().is_empty())
        }
        fn parse<T: std::str::FromStr>(name: &str) -> Option<T> {
            var(name).and_then(|value| value.parse::<T>().ok())
        }

        if let Some(value) = var("AICRAWL_API_HOST") {
            self.server.api_host = value;
        }
        if let Some(value) = parse("AICRAWL_API_PORT") {
            self.server.api_port = value;
        }
        if let Some(value) = var("AICRAWL_MCP_HOST") {
            self.server.mcp_host = value;
        }
        if let Some(value) = parse("AICRAWL_MCP_PORT") {
            self.server.mcp_port = value;
        }
        if let Some(value) = parse("AICRAWL_ALLOW_PRIVATE_NETWORKS") {
            self.security.allow_private_networks = value;
        }
        if let Some(value) = parse("AICRAWL_RATE_LIMIT_PER_MINUTE") {
            self.security.rate_limit_per_minute = value;
        }
        if let Some(value) = var("AICRAWL_ALLOWED_DOMAINS") {
            self.security.allowed_domains = split_list(&value);
        }
        if let Some(value) = var("AICRAWL_BLOCKED_DOMAINS") {
            self.security.blocked_domains = split_list(&value);
        }
        if let Some(value) = var("AICRAWL_DATABASE_PATH") {
            self.storage.database_path = Some(PathBuf::from(value));
        }
        if let Some(value) = parse("AICRAWL_MAX_ITEMS") {
            self.extract.max_items = value;
        }
        if let Some(value) = parse("AICRAWL_REQUIRE_API_KEY") {
            self.api.require_api_key = value;
        }
        if let Some(value) = var("AICRAWL_DASHBOARD_DIST") {
            self.api.dashboard_dist = Some(PathBuf::from(value));
        }
    }

    /// Resolved SQLite path for this configuration.
    pub fn database_path(&self) -> PathBuf {
        self.storage
            .database_path
            .clone()
            .unwrap_or_else(|| data_dir().join("aicrawl.db"))
    }

    /// JSON-safe view of the configuration.
    pub fn redacted(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe() {
        let config = AppConfig::default();
        assert!(!config.security.allow_private_networks);
        assert!(config.crawler.respect_robots_txt);
        assert!(config.security.max_redirects <= 10);
        assert!(config.extract.max_items >= 100);
        assert!(config.crawler.render_timeout_ms >= config.crawler.timeout_ms);
    }

    #[test]
    fn toml_round_trip() {
        let mut config = AppConfig::default();
        config.crawler.max_pages = 42;
        config.extract.max_items = 7;
        let raw = toml::to_string_pretty(&config).unwrap();
        let parsed: AppConfig = toml::from_str(&raw).unwrap();
        assert_eq!(parsed.crawler.max_pages, 42);
        assert_eq!(parsed.extract.max_items, 7);
    }

    #[test]
    fn legacy_ai_section_is_ignored() {
        let raw = "[ai]\nbase_url = \"https://api.openai.com/v1\"\n";
        let parsed: AppConfig = toml::from_str(raw).unwrap();
        assert_eq!(parsed.crawler.max_pages, 10);
    }
}

//! HTTP fetching with SSRF protection, robots.txt, manual redirect validation,
//! per-host rate limiting, size caps and a SQLite page cache.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use url::Url;

use crate::browser::PlaywrightRenderer;
use crate::config::{CacheConfig, CrawlerConfig, SecurityConfig};
use crate::error::{AicrawlError, Result};
use crate::security::{HostRateLimiter, RobotsRules, UrlPolicy};
use crate::storage::{NewCacheEntry, SqliteStore};
use crate::util::{now_rfc3339, sha256_hex};

const ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,text/plain;q=0.8,application/json;q=0.7,*/*;q=0.5";
const ROBOTS_MAX_BYTES: usize = 512 * 1024;
const ROBOTS_TTL_SECS: i64 = 3600;

/// Options for a single fetch.
#[derive(Debug, Clone, Default)]
pub struct FetchOptions {
    /// Render JavaScript through Playwright instead of reading raw HTML.
    pub render: bool,
    /// Consult and populate the SQLite page cache.
    pub use_cache: bool,
    /// Bypass robots.txt (only used by tests and explicit operator overrides).
    pub ignore_robots: bool,
    /// Override the configured body size cap.
    pub max_body_bytes: Option<usize>,
}

impl FetchOptions {
    /// Cached, robots-respecting fetch.
    pub fn standard() -> Self {
        Self {
            use_cache: true,
            ..Default::default()
        }
    }

    /// Fetch that bypasses the cache (still respects robots.txt).
    pub fn fresh() -> Self {
        Self::default()
    }
}

/// A successfully fetched document.
#[derive(Debug, Clone)]
pub struct FetchedPage {
    pub requested_url: String,
    pub url: String,
    pub status: u16,
    pub content_type: String,
    pub body: String,
    pub truncated: bool,
    pub from_cache: bool,
    pub rendered: bool,
    pub fetched_at: String,
    pub elapsed_ms: u64,
    pub bytes: usize,
    pub warnings: Vec<String>,
}

/// Shared HTTP fetcher.
pub struct Fetcher {
    client: reqwest::Client,
    policy: UrlPolicy,
    limiter: Arc<HostRateLimiter>,
    store: SqliteStore,
    crawler: CrawlerConfig,
    cache: CacheConfig,
    renderer: PlaywrightRenderer,
    robots_memory: Mutex<HashMap<String, (RobotsRules, u16, Instant)>>,
}

impl std::fmt::Debug for Fetcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fetcher")
            .field("policy", &self.policy)
            .field("user_agent", &self.crawler.user_agent)
            .finish()
    }
}

impl Fetcher {
    /// Builds a fetcher from configuration and storage.
    pub fn new(
        crawler: CrawlerConfig,
        cache: CacheConfig,
        security: &SecurityConfig,
        store: SqliteStore,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(&crawler.user_agent)
            .timeout(Duration::from_millis(crawler.timeout_ms))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .gzip(true)
            .brotli(true)
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| AicrawlError::Config(format!("cannot build HTTP client: {e}")))?;
        Ok(Self {
            client,
            policy: UrlPolicy::from_config(security),
            limiter: Arc::new(HostRateLimiter::new(security.rate_limit_per_minute)),
            store,
            renderer: PlaywrightRenderer::from_env(crawler.timeout_ms * 2),
            crawler,
            cache,
            robots_memory: Mutex::new(HashMap::new()),
        })
    }

    /// Active URL policy.
    pub fn policy(&self) -> &UrlPolicy {
        &self.policy
    }

    /// Optional renderer handle.
    pub fn renderer(&self) -> &PlaywrightRenderer {
        &self.renderer
    }

    /// Validates a URL against the policy (syntax + DNS).
    pub async fn validate(&self, url: &Url) -> Result<()> {
        self.policy.validate_resolution(url).await.map(|_| ())
    }

    /// Fetches a URL, following validated redirects.
    pub async fn fetch(&self, url: &Url, options: &FetchOptions) -> Result<FetchedPage> {
        let started = Instant::now();
        self.policy.validate_resolution(url).await?;

        if self.crawler.respect_robots_txt && !options.ignore_robots {
            let (rules, _) = self.robots_for(url).await?;
            if !rules.is_allowed_for(&self.crawler.user_agent, &path_and_query(url)) {
                return Err(AicrawlError::RobotsDisallowed(url.to_string()));
            }
        }

        if options.use_cache && self.cache.enabled {
            if let Some(entry) = self.store.get_cached_page(url.as_str()).await? {
                return Ok(FetchedPage {
                    requested_url: url.to_string(),
                    url: entry.final_url,
                    status: entry.status_code as u16,
                    content_type: entry.content_type,
                    body: entry.body,
                    truncated: false,
                    from_cache: true,
                    rendered: false,
                    fetched_at: entry.fetched_at,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    bytes: entry.body_bytes as usize,
                    warnings: vec![],
                });
            }
        }

        self.limiter.acquire(host_key(url))?;

        if options.render {
            let rendered = self.renderer.render(url).await?;
            let final_url = Url::parse(&rendered.final_url).unwrap_or_else(|_| url.clone());
            self.policy.validate_redirect(url, &final_url).await?;
            let bytes = rendered.html.len();
            let page = FetchedPage {
                requested_url: url.to_string(),
                url: final_url.to_string(),
                status: rendered.status,
                content_type: "text/html".into(),
                body: rendered.html,
                truncated: false,
                from_cache: false,
                rendered: true,
                fetched_at: now_rfc3339(),
                elapsed_ms: started.elapsed().as_millis() as u64,
                bytes,
                warnings: vec!["rendered with JavaScript through Playwright".into()],
            };
            self.cache_page(url, &page).await?;
            return Ok(page);
        }

        let mut current = url.clone();
        let mut warnings = Vec::new();
        let max_redirects = self.policy.max_redirects();

        for hop in 0..=max_redirects {
            self.policy.validate_resolution(&current).await?;
            if hop > 0 {
                self.limiter.acquire(host_key(&current))?;
            }

            let response = self
                .client
                .get(current.as_str())
                .header(reqwest::header::ACCEPT, ACCEPT)
                .send()
                .await
                .map_err(|e| self.map_error(&current, e))?;

            let status = response.status();
            if status.is_redirection() {
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| {
                        AicrawlError::Http {
                            status: status.as_u16(),
                            url: current.to_string(),
                        }
                    })?;
                let next = current.join(location).map_err(|e| {
                    AicrawlError::InvalidInput(format!(
                        "redirect from {current} has an invalid Location '{location}': {e}"
                    ))
                })?;
                if hop == max_redirects {
                    return Err(AicrawlError::Blocked(format!(
                        "too many redirects (limit {max_redirects}) starting at {url}"
                    )));
                }
                self.policy.validate_redirect(&current, &next).await?;
                warnings.push(format!("followed redirect {current} -> {next}"));
                current = next;
                continue;
            }

            if !status.is_success() {
                return Err(AicrawlError::Http {
                    status: status.as_u16(),
                    url: current.to_string(),
                });
            }

            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();
            let mime = content_type
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            if !is_supported(&mime) {
                return Err(AicrawlError::UnsupportedContent(format!(
                    "{mime} at {current}"
                )));
            }

            let limit = options
                .max_body_bytes
                .unwrap_or(self.crawler.max_body_bytes)
                .max(1024);
            let (body, bytes, truncated) = read_body(response, limit, &content_type).await?;
            if truncated {
                warnings.push(format!(
                    "response body truncated at {limit} bytes (content continues)"
                ));
            }

            let page = FetchedPage {
                requested_url: url.to_string(),
                url: current.to_string(),
                status: status.as_u16(),
                content_type,
                body,
                truncated,
                from_cache: false,
                rendered: false,
                fetched_at: now_rfc3339(),
                elapsed_ms: started.elapsed().as_millis() as u64,
                bytes,
                warnings,
            };
            self.cache_page(url, &page).await?;
            return Ok(page);
        }

        Err(AicrawlError::Other(format!(
            "request loop ended unexpectedly for {url}"
        )))
    }

    /// Fetches and caches `/robots.txt` for the URL's origin.
    async fn robots_for(&self, url: &Url) -> Result<(RobotsRules, u16)> {
        let origin = origin_of(url);
        {
            let memory = self
                .robots_memory
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some((rules, status, fetched)) = memory.get(&origin) {
                if fetched.elapsed() < Duration::from_secs(ROBOTS_TTL_SECS as u64) {
                    return Ok((rules.clone(), *status));
                }
            }
        }

        if let Some(row) = self.store.get_robots(&origin).await? {
            let rules = RobotsRules::parse(&row.body);
            let status = row.status_code as u16;
            self.remember_robots(&origin, &rules, status);
            return Ok((rules, status));
        }

        let robots_url = Url::parse(&format!("{origin}/robots.txt"))
            .map_err(|e| AicrawlError::Other(format!("invalid robots URL: {e}")))?;
        // The origin was already validated; still re-check before connecting.
        self.policy.validate_syntax(&robots_url)?;
        self.limiter.acquire(host_key(&robots_url))?;

        let (rules, status, body) = match self
            .client
            .get(robots_url.as_str())
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                let (body, _, _) = read_body(response, ROBOTS_MAX_BYTES, "text/plain")
                    .await
                    .unwrap_or((String::new(), 0, false));
                let rules = if status == 401 || status == 403 {
                    RobotsRules::parse("User-agent: *\nDisallow: /\n")
                } else {
                    RobotsRules::parse(&body)
                };
                (rules, status, body)
            }
            Err(error) => {
                // A missing or unreachable robots.txt is treated as "no restrictions"
                // (status 0 is recorded so `doctor` can surface the condition).
                tracing::warn!(%error, %origin, "robots.txt fetch failed; continuing without restrictions");
                (RobotsRules::default(), 0, String::new())
            }
        };

        self.store
            .put_robots(&origin, &body, status, ROBOTS_TTL_SECS)
            .await?;
        self.remember_robots(&origin, &rules, status);
        Ok((rules, status))
    }

    fn remember_robots(&self, origin: &str, rules: &RobotsRules, status: u16) {
        let mut memory = self
            .robots_memory
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        memory.insert(origin.to_string(), (rules.clone(), status, Instant::now()));
    }

    async fn cache_page(&self, key: &Url, page: &FetchedPage) -> Result<()> {
        if !self.cache.enabled || page.status < 200 || page.status >= 300 {
            return Ok(());
        }
        let entry = NewCacheEntry {
            url: key.to_string(),
            final_url: page.url.clone(),
            status_code: page.status,
            content_type: page.content_type.clone(),
            body: page.body.clone(),
            content_hash: sha256_hex(page.body.as_bytes()),
            body_bytes: page.bytes,
            ttl_secs: self.cache.ttl_secs as i64,
        };
        self.store.put_cached_page(entry).await?;
        self.store
            .prune_cache(self.cache.max_entries as i64)
            .await?;
        Ok(())
    }

    fn map_error(&self, url: &Url, error: reqwest::Error) -> AicrawlError {
        if error.is_timeout() {
            AicrawlError::Timeout(self.crawler.timeout_ms)
        } else if error.is_builder() {
            AicrawlError::InvalidInput(format!("cannot build request for {url}: {error}"))
        } else {
            AicrawlError::Network(format!("{url}: {error}"))
        }
    }
}

fn is_supported(mime: &str) -> bool {
    matches!(
        mime,
        "text/html"
            | "application/xhtml+xml"
            | "text/plain"
            | "application/json"
            | "application/ld+json"
            | "application/xml"
            | "text/xml"
            | "text/markdown"
    )
}

fn origin_of(url: &Url) -> String {
    let scheme = url.scheme();
    match url.host_str() {
        Some(host) => match url.port() {
            Some(port) => format!("{scheme}://{host}:{port}"),
            None => format!("{scheme}://{host}"),
        },
        None => format!("{scheme}://"),
    }
}

fn host_key(url: &Url) -> &str {
    url.host_str().unwrap_or("")
}

fn path_and_query(url: &Url) -> String {
    match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_string(),
    }
}

/// Reads a response body while enforcing a byte cap.
async fn read_body(
    response: reqwest::Response,
    limit: usize,
    content_type: &str,
) -> Result<(String, usize, bool)> {
    let charset = content_type
        .split(';')
        .find_map(|part| part.trim().strip_prefix("charset="))
        .map(|value| value.trim_matches('"').to_ascii_lowercase());

    let mut buffer: Vec<u8> = Vec::new();
    let mut truncated = false;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AicrawlError::Network(format!("body read failed: {e}")))?;
        if buffer.len() + chunk.len() > limit {
            let take = limit.saturating_sub(buffer.len());
            buffer.extend_from_slice(&chunk[..take]);
            truncated = true;
            break;
        }
        buffer.extend_from_slice(&chunk);
    }

    let bytes = buffer.len();
    let text = decode_bytes(&buffer, charset.as_deref());
    Ok((text, bytes, truncated))
}

fn decode_bytes(bytes: &[u8], charset: Option<&str>) -> String {
    match charset {
        Some("iso-8859-1") | Some("latin1") | Some("windows-1252") => {
            let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
            decoded.into_owned()
        }
        Some("utf-8") | None => String::from_utf8_lossy(bytes).into_owned(),
        Some(other) => {
            if let Some(encoding) = encoding_rs::Encoding::for_label(other.as_bytes()) {
                let (decoded, _, _) = encoding.decode(bytes);
                decoded.into_owned()
            } else {
                String::from_utf8_lossy(bytes).into_owned()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_and_paths() {
        let url = Url::parse("https://example.com:8443/a/b?x=1").unwrap();
        assert_eq!(origin_of(&url), "https://example.com:8443");
        assert_eq!(path_and_query(&url), "/a/b?x=1");
    }

    #[test]
    fn content_types_are_restricted() {
        assert!(is_supported("text/html"));
        assert!(is_supported("text/plain"));
        assert!(!is_supported("application/pdf"));
        assert!(!is_supported("image/png"));
    }

    #[test]
    fn decodes_latin1() {
        let bytes = [0x63, 0x61, 0x66, 0xe9];
        assert_eq!(decode_bytes(&bytes, Some("iso-8859-1")), "café");
    }
}

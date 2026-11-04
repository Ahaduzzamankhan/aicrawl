//! Security primitives shared by every interface: SSRF protection, domain
//! policy, host rate limiting, robots.txt evaluation and secret storage.

pub mod robots;
pub mod secrets;

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::config::SecurityConfig;
use crate::error::{AicrawlError, Result};

pub use robots::{RobotsRules, RobotsSnapshot};
pub use secrets::{hash_api_key, key_prefix, SecretStore};

/// Validates target URLs and resolved IP addresses.
#[derive(Debug, Clone)]
pub struct UrlPolicy {
    allow_private_networks: bool,
    allowed_domains: Vec<String>,
    blocked_domains: Vec<String>,
    allow_http: bool,
    max_redirects: u32,
}

impl UrlPolicy {
    /// Builds a policy from the `[security]` config section.
    pub fn from_config(config: &SecurityConfig) -> Self {
        Self {
            allow_private_networks: config.allow_private_networks,
            allowed_domains: normalize_domains(&config.allowed_domains),
            blocked_domains: normalize_domains(&config.blocked_domains),
            allow_http: config.allow_http,
            max_redirects: config.max_redirects,
        }
    }

    /// Maximum number of redirects that may be followed.
    pub fn max_redirects(&self) -> u32 {
        self.max_redirects
    }

    /// Whether private/loopback targets are permitted (tests and local dev only).
    pub fn allows_private_networks(&self) -> bool {
        self.allow_private_networks
    }

    /// Parses a URL string and applies the policy's syntax checks.
    pub fn parse(&self, input: &str) -> Result<Url> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(AicrawlError::InvalidInput("url must not be empty".into()));
        }
        let url = Url::parse(trimmed)
            .map_err(|e| AicrawlError::InvalidInput(format!("invalid URL '{trimmed}': {e}")))?;
        self.validate_syntax(&url)?;
        Ok(url)
    }

    /// Scheme, credential and domain checks. Does not resolve DNS.
    pub fn validate_syntax(&self, url: &Url) -> Result<()> {
        match url.scheme() {
            "https" => {}
            "http" if self.allow_http => {}
            "http" => {
                return Err(AicrawlError::Blocked(
                    "plain HTTP targets are disabled (security.allow_http = false)".into(),
                ))
            }
            other => {
                return Err(AicrawlError::Blocked(format!(
                    "unsupported URL scheme '{other}'; only http and https are allowed"
                )))
            }
        }

        if !url.username().is_empty() || url.password().is_some() {
            return Err(AicrawlError::Blocked(
                "URLs containing credentials are refused".into(),
            ));
        }

        let host = url
            .host_str()
            .ok_or_else(|| AicrawlError::InvalidInput("URL has no host".into()))?;
        let host = host.trim_end_matches('.').to_ascii_lowercase();

        if host.is_empty() || (host == "localhost" && !self.allow_private_networks) {
            return Err(AicrawlError::Blocked(
                "loopback hosts are blocked by the SSRF policy".into(),
            ));
        }
        // Reject literal private/loopback IPs without waiting for DNS.
        if !self.allow_private_networks {
            let bare = host.trim_start_matches('[').trim_end_matches(']');
            if let Ok(ip) = bare.parse::<std::net::IpAddr>() {
                if is_forbidden_ip(ip) {
                    return Err(AicrawlError::Blocked(format!(
                        "target uses non-public address {ip}"
                    )));
                }
            }
        }
        if is_blocked_hostname(&host) {
            return Err(AicrawlError::Blocked(format!(
                "host '{host}' is reserved for internal metadata services"
            )));
        }
        if self
            .blocked_domains
            .iter()
            .any(|domain| domain_matches(&host, domain))
        {
            return Err(AicrawlError::Blocked(format!(
                "domain '{host}' is on the blocked list"
            )));
        }
        if !self.allowed_domains.is_empty()
            && !self
                .allowed_domains
                .iter()
                .any(|domain| domain_matches(&host, domain))
        {
            return Err(AicrawlError::Blocked(format!(
                "domain '{host}' is not on the allowed list"
            )));
        }

        if let Some(port) = url.port() {
            if port == 0 {
                return Err(AicrawlError::InvalidInput("port 0 is not usable".into()));
            }
        }

        Ok(())
    }

    /// Resolves the host and refuses targets that resolve to non-public addresses.
    pub async fn validate_resolution(&self, url: &Url) -> Result<Vec<IpAddr>> {
        self.validate_syntax(url)?;
        let host = url
            .host()
            .ok_or_else(|| AicrawlError::InvalidInput("URL has no host".into()))?;
        let port = url.port_or_known_default().unwrap_or(443);

        match host {
            Host::Ipv4(ip) => {
                self.check_ip(IpAddr::V4(ip))?;
                Ok(vec![IpAddr::V4(ip)])
            }
            Host::Ipv6(ip) => {
                self.check_ip(IpAddr::V6(ip))?;
                Ok(vec![IpAddr::V6(ip)])
            }
            Host::Domain(domain) => {
                let lookup = tokio::time::timeout(
                    Duration::from_secs(10),
                    tokio::net::lookup_host((domain, port)),
                )
                .await
                .map_err(|_| AicrawlError::Timeout(10_000))?
                .map_err(|e| {
                    AicrawlError::Network(format!("DNS resolution failed for '{domain}': {e}"))
                })?;
                let ips: Vec<IpAddr> = lookup.map(|addr| addr.ip()).collect();
                if ips.is_empty() {
                    return Err(AicrawlError::Network(format!(
                        "DNS resolution returned no addresses for '{domain}'"
                    )));
                }
                // Refuse when *any* answer is internal: prevents DNS-rebinding style
                // round-robin answers from smuggling a private target through.
                for ip in &ips {
                    self.check_ip(*ip)?;
                }
                Ok(ips)
            }
        }
    }

    /// Validates a redirect target before it is followed.
    pub async fn validate_redirect(&self, from: &Url, to: &Url) -> Result<()> {
        self.validate_resolution(to)
            .await
            .map_err(|err| match err {
                AicrawlError::Blocked(message) => AicrawlError::Blocked(format!(
                    "redirect from {from} to {to} refused: {message}"
                )),
                other => other,
            })?;
        Ok(())
    }

    fn check_ip(&self, ip: IpAddr) -> Result<()> {
        if self.allow_private_networks {
            return Ok(());
        }
        if is_forbidden_ip(ip) {
            return Err(AicrawlError::Blocked(format!(
                "target resolves to non-public address {ip}"
            )));
        }
        Ok(())
    }
}

fn normalize_domains(domains: &[String]) -> Vec<String> {
    domains
        .iter()
        .map(|d| {
            d.trim()
                .trim_start_matches("*.")
                .trim_end_matches('.')
                .to_ascii_lowercase()
        })
        .filter(|d| !d.is_empty())
        .collect()
}

/// True when `host` equals `domain` or is a subdomain of it.
pub fn domain_matches(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// Hostnames that always denote local/metadata services.
pub fn is_blocked_hostname(host: &str) -> bool {
    matches!(
        host,
        "metadata.google.internal"
            | "metadata.goog"
            | "instance-data"
            | "instance-data.ec2.internal"
            | "metadata"
    ) || host.ends_with(".internal")
        || host.ends_with(".local")
}

/// Whether an IP address must not be fetched by default.
///
/// Covers loopback, private RFC1918, link-local (including cloud metadata
/// 169.254.169.254), CGNAT, documentation, benchmark, multicast, broadcast,
/// unique-local IPv6 and IPv4-mapped variants.
pub fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_forbidden_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_forbidden_v4(mapped);
            }
            if let Some(v4) = ipv4_compatible(v6) {
                return is_forbidden_v4(v4);
            }
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.segments()[0] & 0xffc0 == 0xfe80
        }
    }
}

fn ipv4_compatible(v6: Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = v6.segments();
    if segments[..6].iter().all(|s| *s == 0) {
        let o = v6.octets();
        Some(Ipv4Addr::new(o[12], o[13], o[14], o[15]))
    } else {
        None
    }
}

fn is_forbidden_v4(v4: Ipv4Addr) -> bool {
    let o = v4.octets();
    v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_broadcast()
        || v4.is_documentation()
        || v4.is_unspecified()
        || v4.is_multicast()
        || o[0] == 0
        || (o[0] == 100 && (64..128).contains(&o[1]))
        || (o[0] == 192 && o[1] == 0 && o[2] == 0)
        || (o[0] == 198 && (o[1] == 18 || o[1] == 19))
        || o[0] >= 240
}

/// Sliding-window, per-host request limiter.
#[derive(Debug)]
pub struct HostRateLimiter {
    per_minute: u32,
    window: Duration,
    hits: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl HostRateLimiter {
    /// Creates a limiter allowing `per_minute` requests per host.
    pub fn new(per_minute: u32) -> Self {
        Self {
            per_minute,
            window: Duration::from_secs(60),
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// Records a request for `host`, or returns an error when the limit is reached.
    pub fn acquire(&self, host: &str) -> Result<()> {
        if self.per_minute == 0 {
            return Ok(());
        }
        let now = Instant::now();
        let mut hits = self
            .hits
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let entry = hits.entry(host.to_ascii_lowercase()).or_default();
        while let Some(front) = entry.front() {
            if now.duration_since(*front) > self.window {
                entry.pop_front();
            } else {
                break;
            }
        }
        if entry.len() as u32 >= self.per_minute {
            return Err(AicrawlError::RateLimited(format!(
                "{} requests in the last 60s for host '{host}'",
                entry.len()
            )));
        }
        entry.push_back(now);
        Ok(())
    }

    /// Current number of recorded requests for a host.
    pub fn usage(&self, host: &str) -> usize {
        self.hits
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&host.to_ascii_lowercase())
            .map(|v| v.len())
            .unwrap_or(0)
    }
}

/// DTO describing the active security posture (used by `doctor` and `/api/v1/settings`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySummary {
    pub allow_private_networks: bool,
    pub allowed_domains: Vec<String>,
    pub blocked_domains: Vec<String>,
    pub max_redirects: u32,
    pub rate_limit_per_minute: u32,
    pub respect_robots_txt: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> UrlPolicy {
        UrlPolicy::from_config(&SecurityConfig::default())
    }

    #[test]
    fn blocks_loopback_literals() {
        for raw in [
            "http://127.0.0.1/",
            "http://127.1.2.3/",
            "http://[::1]/",
            "http://169.254.169.254/latest/meta-data",
            "http://10.0.0.5/",
            "http://192.168.1.10/",
            "http://172.16.0.1/",
            "http://0.0.0.0/",
            "http://[::ffff:127.0.0.1]/",
        ] {
            let err = policy().validate_syntax(&Url::parse(raw).unwrap());
            assert!(err.is_err(), "{raw} should be refused");
        }
    }

    #[test]
    fn accepts_public_hosts() {
        assert!(policy()
            .validate_syntax(&Url::parse("https://example.com/products").unwrap())
            .is_ok());
        assert!(policy()
            .validate_syntax(&Url::parse("https://93.184.216.34/").unwrap())
            .is_ok());
    }

    #[test]
    fn rejects_bad_schemes_and_credentials() {
        assert!(policy()
            .validate_syntax(&Url::parse("file:///etc/passwd").unwrap())
            .is_err());
        assert!(policy()
            .validate_syntax(&Url::parse("ftp://example.com/").unwrap())
            .is_err());
        assert!(policy()
            .validate_syntax(&Url::parse("https://user:pass@example.com/").unwrap())
            .is_err());
    }

    #[test]
    fn domain_allow_and_deny_lists() {
        let mut config = SecurityConfig::default();
        config.allowed_domains = vec!["example.com".into()];
        let policy = UrlPolicy::from_config(&config);
        assert!(policy
            .validate_syntax(&Url::parse("https://sub.example.com/").unwrap())
            .is_ok());
        assert!(policy
            .validate_syntax(&Url::parse("https://example.org/").unwrap())
            .is_err());
        assert!(policy
            .validate_syntax(&Url::parse("https://notexample.com/").unwrap())
            .is_err());

        let mut config = SecurityConfig::default();
        config.blocked_domains = vec!["tracker.example".into()];
        let policy = UrlPolicy::from_config(&config);
        assert!(policy
            .validate_syntax(&Url::parse("https://a.tracker.example/").unwrap())
            .is_err());
    }

    #[test]
    fn ip_classification() {
        assert!(is_forbidden_ip("127.0.0.1".parse().unwrap()));
        assert!(is_forbidden_ip("169.254.169.254".parse().unwrap()));
        assert!(is_forbidden_ip("100.64.0.1".parse().unwrap()));
        assert!(is_forbidden_ip("fc00::1".parse().unwrap()));
        assert!(is_forbidden_ip("fe80::1".parse().unwrap()));
        assert!(is_forbidden_ip("::ffff:192.168.0.1".parse().unwrap()));
        assert!(!is_forbidden_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_forbidden_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn rate_limiter_blocks_then_allows() {
        let limiter = HostRateLimiter::new(2);
        assert!(limiter.acquire("example.com").is_ok());
        assert!(limiter.acquire("example.com").is_ok());
        assert!(limiter.acquire("example.com").is_err());
        assert!(limiter.acquire("other.com").is_ok());
        assert_eq!(limiter.usage("example.com"), 2);
    }
}

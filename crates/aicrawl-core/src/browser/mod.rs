//! Optional JavaScript rendering through Playwright.
//!
//! AICrawl's fetch pipeline is HTML-first: no scripts run unless rendering is
//! explicitly requested. Rendering is delegated to the Playwright bridge in
//! `browser/playwright_bridge.mjs`, which must be installed separately
//! (`cd browser && npm install && npx playwright install chromium`). When the
//! bridge or Playwright is missing, callers receive a clear
//! [`AicrawlError::BrowserUnavailable`] instead of silently degraded output.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use url::Url;

use crate::error::{AicrawlError, Result};

/// Availability of the optional rendering stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserStatus {
    pub available: bool,
    pub node: Option<String>,
    pub bridge_path: String,
    pub playwright_installed: bool,
    pub detail: String,
}

/// Client for the Playwright bridge process.
#[derive(Debug, Clone)]
pub struct PlaywrightRenderer {
    bridge_path: PathBuf,
    node_binary: String,
    working_dir: PathBuf,
    timeout: Duration,
}

/// Result of a rendered page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedPage {
    pub html: String,
    pub final_url: String,
    pub status: u16,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BridgeResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    html: Option<String>,
    #[serde(default)]
    final_url: Option<String>,
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    title: Option<String>,
}

impl PlaywrightRenderer {
    /// Locates the bridge using `AICRAWL_PLAYWRIGHT_BRIDGE` or the repository layout.
    pub fn from_env(timeout_ms: u64) -> Self {
        let node_binary = std::env::var("AICRAWL_NODE").unwrap_or_else(|_| "node".to_string());
        let bridge_path = std::env::var("AICRAWL_PLAYWRIGHT_BRIDGE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_bridge_path());
        let working_dir = bridge_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            bridge_path,
            node_binary,
            working_dir,
            timeout: Duration::from_millis(timeout_ms.max(5_000)),
        }
    }

    /// Bridge script location.
    pub fn bridge_path(&self) -> &Path {
        &self.bridge_path
    }

    /// Probes Node and Playwright without launching a browser.
    pub async fn status(&self) -> BrowserStatus {
        let node_version = Command::new(&self.node_binary)
            .arg("--version")
            .output()
            .await
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

        let bridge_exists = self.bridge_path.exists();
        let mut playwright_installed = false;
        if node_version.is_some() && bridge_exists {
            let probe = Command::new(&self.node_binary)
                .arg("-e")
                .arg("import('playwright').then(()=>process.exit(0)).catch(()=>process.exit(1))")
                .current_dir(&self.working_dir)
                .output()
                .await;
            playwright_installed = probe.map(|o| o.status.success()).unwrap_or(false);
        }

        let detail = if node_version.is_none() {
            format!("Node.js not found (looked for '{}')", self.node_binary)
        } else if !bridge_exists {
            format!("bridge script missing at {}", self.bridge_path.display())
        } else if !playwright_installed {
            "Playwright package not installed in browser/ (run: cd browser && npm install)"
                .to_string()
        } else {
            "Playwright bridge ready".to_string()
        };

        BrowserStatus {
            available: node_version.is_some() && bridge_exists && playwright_installed,
            node: node_version,
            bridge_path: self.bridge_path.display().to_string(),
            playwright_installed,
            detail,
        }
    }

    /// Renders `url` and returns the post-JavaScript HTML.
    pub async fn render(&self, url: &Url) -> Result<RenderedPage> {
        if !self.bridge_path.exists() {
            return Err(AicrawlError::BrowserUnavailable(format!(
                "bridge script not found at {}",
                self.bridge_path.display()
            )));
        }
        let timeout_ms = self.timeout.as_millis().to_string();
        let child = Command::new(&self.node_binary)
            .arg(&self.bridge_path)
            .arg("--url")
            .arg(url.as_str())
            .arg("--timeout")
            .arg(&timeout_ms)
            .current_dir(&self.working_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                AicrawlError::BrowserUnavailable(format!(
                    "cannot start Node.js ('{}'): {e}",
                    self.node_binary
                ))
            })?;

        let output = match tokio::time::timeout(self.timeout + Duration::from_secs(5), child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Err(AicrawlError::BrowserUnavailable(format!(
                    "bridge process failed: {e}"
                )))
            }
            Err(_) => {
                return Err(AicrawlError::Timeout(
                    self.timeout.as_millis() as u64 + 5_000,
                ))
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let line = stdout
            .lines()
            .rev()
            .find(|line| line.trim_start().starts_with('{'))
            .ok_or_else(|| {
                AicrawlError::BrowserUnavailable(format!(
                    "bridge produced no JSON response (exit {:?}): {}",
                    output.status.code(),
                    stderr.trim()
                ))
            })?;

        let response: BridgeResponse = serde_json::from_str(line).map_err(|e| {
            AicrawlError::BrowserUnavailable(format!("invalid bridge response: {e}"))
        })?;
        if !response.ok {
            return Err(AicrawlError::BrowserUnavailable(
                response
                    .error
                    .unwrap_or_else(|| "rendering failed without a message".into()),
            ));
        }
        let html = response
            .html
            .ok_or_else(|| AicrawlError::BrowserUnavailable("bridge returned no HTML".into()))?;
        if html.trim().is_empty() {
            return Err(AicrawlError::BrowserUnavailable(
                "bridge returned an empty document".into(),
            ));
        }

        Ok(RenderedPage {
            html,
            final_url: response.final_url.unwrap_or_else(|| url.to_string()),
            status: response.status.unwrap_or(200),
            title: response.title,
        })
    }
}

fn default_bridge_path() -> PathBuf {
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("browser/playwright_bridge.mjs"));
        if let Some(parent) = cwd.parent() {
            candidates.push(parent.join("browser/playwright_bridge.mjs"));
        }
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../browser/playwright_bridge.mjs")
            .canonicalize()
            .unwrap_or_else(|_| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../browser/playwright_bridge.mjs")
            }),
    );
    candidates
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../browser/playwright_bridge.mjs")
        })
}

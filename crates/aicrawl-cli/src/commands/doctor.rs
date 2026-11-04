//! `aicrawl doctor`

use std::time::Duration;

use clap::Args;
use serde_json::json;

use aicrawl_core::error::{AicrawlError, Result};

use super::Ctx;
use crate::terminal;

/// Arguments for `aicrawl doctor`.
#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Skip the outbound network test.
    #[arg(long)]
    pub offline: bool,

    /// Print JSON instead of a report.
    #[arg(long)]
    pub json: bool,
}

struct Check {
    name: &'static str,
    ok: bool,
    critical: bool,
    detail: String,
}

/// Runs local diagnostics and prints a coloured report.
pub async fn run(ctx: &Ctx, args: DoctorArgs) -> Result<()> {
    let theme = ctx.theme;
    let mut checks: Vec<Check> = Vec::new();

    // Data directory writability.
    let probe = ctx.data_dir.join(".aicrawl-write-test");
    let writable = std::fs::write(&probe, b"ok").is_ok() && std::fs::remove_file(&probe).is_ok();
    checks.push(Check {
        name: "data directory writable",
        ok: writable,
        critical: true,
        detail: ctx.data_dir.display().to_string(),
    });

    // Config file.
    let config_exists = ctx.config_path.exists();
    checks.push(Check {
        name: "config file",
        ok: true,
        critical: false,
        detail: if config_exists {
            ctx.config_path.display().to_string()
        } else {
            format!("{} (defaults in use)", ctx.config_path.display())
        },
    });

    // Engine + database.
    let engine = match ctx.engine().await {
        Ok(engine) => {
            checks.push(Check {
                name: "SQLite database",
                ok: true,
                critical: true,
                detail: engine.store().location().to_string(),
            });
            Some(engine)
        }
        Err(err) => {
            checks.push(Check {
                name: "SQLite database",
                ok: false,
                critical: true,
                detail: err.to_string(),
            });
            None
        }
    };

    if let Some(engine) = engine.as_ref() {
        checks.push(Check {
            name: "secret store",
            ok: engine.secrets().key_path().exists() || std::env::var("AICRAWL_SECRET_KEY").is_ok(),
            critical: false,
            detail: engine.secrets().key_path().display().to_string(),
        });

        let ai = engine.ai_status().await;
        checks.push(Check {
            name: "external AI provider",
            ok: ai.configured,
            critical: false,
            detail: ai.detail,
        });

        let browser = engine.fetcher().renderer().status().await;
        checks.push(Check {
            name: "Playwright rendering",
            ok: browser.available,
            critical: false,
            detail: browser.detail,
        });

        let security = &engine.config().security;
        checks.push(Check {
            name: "SSRF protection",
            ok: !security.allow_private_networks,
            critical: false,
            detail: if security.allow_private_networks {
                "private networks are ALLOWED (development setting)".into()
            } else {
                "private and loopback targets are blocked".into()
            },
        });
        checks.push(Check {
            name: "robots.txt",
            ok: engine.config().crawler.respect_robots_txt,
            critical: false,
            detail: if engine.config().crawler.respect_robots_txt {
                "respected".into()
            } else {
                "DISABLED (operator override)".into()
            },
        });

        if !args.offline {
            let started = std::time::Instant::now();
            let result = tokio::time::timeout(
                Duration::from_secs(20),
                engine.inspect("https://example.com", false),
            )
            .await;
            let (ok, detail) = match result {
                Ok(Ok(report)) => (
                    true,
                    format!(
                        "example.com reachable ({}, {}ms)",
                        report.status, report.elapsed_ms
                    ),
                ),
                Ok(Err(err)) => (false, format!("outbound fetch failed: {err}")),
                Err(_) => (false, "outbound fetch timed out".into()),
            };
            checks.push(Check {
                name: "outbound network",
                ok,
                critical: false,
                detail: format!("{detail} ({}ms)", started.elapsed().as_millis()),
            });
        }
    }

    // Listener availability.
    checks.push(Check {
        name: "API port",
        ok: port_free_or_running(&ctx.config.server.api_host, ctx.config.server.api_port).await,
        critical: false,
        detail: format!(
            "{}:{}",
            ctx.config.server.api_host, ctx.config.server.api_port
        ),
    });
    checks.push(Check {
        name: "MCP port",
        ok: port_free_or_running(&ctx.config.server.mcp_host, ctx.config.server.mcp_port).await,
        critical: false,
        detail: format!(
            "{}:{}",
            ctx.config.server.mcp_host, ctx.config.server.mcp_port
        ),
    });

    let critical_failures = checks.iter().filter(|c| !c.ok && c.critical).count();
    let warnings = checks.iter().filter(|c| !c.ok && !c.critical).count();

    if args.json {
        let items: Vec<serde_json::Value> = checks
            .iter()
            .map(|check| {
                json!({
                    "check": check.name,
                    "ok": check.ok,
                    "critical": check.critical,
                    "detail": check.detail
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": critical_failures == 0,
                "critical_failures": critical_failures,
                "warnings": warnings,
                "checks": items
            }))?
        );
    } else {
        terminal::print_banner(&theme);
        terminal::section(&theme, "Diagnostics");
        for check in &checks {
            let mark = if check.ok {
                theme.ok_mark()
            } else if check.critical {
                theme.err_mark()
            } else {
                theme.warn_mark()
            };
            println!(
                "  {} {:<26} {}",
                mark,
                if check.ok {
                    check.name.to_string()
                } else {
                    theme.bold(check.name)
                },
                theme.dim(&check.detail)
            );
        }
        terminal::section(&theme, "Summary");
        if critical_failures == 0 {
            println!(
                "  {} {}",
                theme.ok_mark(),
                theme.success(format!(
                    "Ready to crawl. {warnings} warning(s), {critical_failures} blocking issue(s)."
                ))
            );
        } else {
            println!(
                "  {} {}",
                theme.err_mark(),
                theme.error(format!(
                    "{critical_failures} blocking issue(s), {warnings} warning(s)."
                ))
            );
        }
        println!(
            "  {} {}",
            theme.arrow(),
            theme.dim(
                "AI extraction is optional: deterministic extraction works without a provider."
            )
        );
    }

    if critical_failures > 0 {
        return Err(AicrawlError::Other(format!(
            "doctor found {critical_failures} blocking issue(s)"
        )));
    }
    Ok(())
}

async fn port_free_or_running(host: &str, port: u16) -> bool {
    let host = match host {
        "0.0.0.0" | "::" => "127.0.0.1",
        other => other,
    };
    let address = format!("{host}:{port}");
    if tokio::time::timeout(
        Duration::from_millis(400),
        tokio::net::TcpStream::connect(&address),
    )
    .await
    .map(|result| result.is_ok())
    .unwrap_or(false)
    {
        return true;
    }
    tokio::net::TcpListener::bind(&address).await.is_ok()
}

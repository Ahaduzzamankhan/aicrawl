//! `aicrawl doctor` — local installation checks.

use aicrawl_core::error::Result;
use clap::Args;
use serde_json::json;

use crate::commands::{self, write_json, Ctx};
use crate::terminal;

/// Arguments for `aicrawl doctor`.
#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Print machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

/// Runs `aicrawl doctor`.
pub async fn run(ctx: &Ctx, args: DoctorArgs) -> Result<i32> {
    let engine = ctx.engine().await?;
    let health = engine.health().await;

    let database_ok = health["database"]["status"] == json!("ok");
    let database_path = health["database"]["path"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let data_dir_ok = ctx.data_dir.exists();
    let config_present = ctx.config_path.exists();
    let dashboard_ok = aicrawl_core::dashboard::available();
    let render_visible = health["renderer"]["available"].as_bool().unwrap_or(false);
    let render_detail = health["renderer"]["detail"]
        .as_str()
        .unwrap_or("browser rendering not detected")
        .to_string();
    let stats = &health["stats"];

    if args.json {
        write_json(
            &json!({
                "success": database_ok && data_dir_ok,
                "data": {
                    "version": aicrawl_core::VERSION,
                    "schema_version": aicrawl_core::SCHEMA_VERSION,
                    "database": { "ok": database_ok, "path": database_path },
                    "data_dir": { "ok": data_dir_ok, "path": ctx.data_dir.display().to_string() },
                    "config": { "present": config_present, "path": ctx.config_path.display().to_string() },
                    "dashboard": { "embedded": dashboard_ok },
                    "renderer": { "available": render_visible, "detail": render_detail },
                    "security": health["security"].clone(),
                    "stats": stats.clone(),
                    "ai_required": false,
                }
            }),
            None,
            true,
        )?;
        return Ok(if database_ok { 0 } else { 1 });
    }

    let theme = ctx.theme;
    eprintln!("{}", theme.bold("AICrawl doctor"));
    terminal::kv(&theme, "version", aicrawl_core::VERSION);
    terminal::kv(
        &theme,
        "schema",
        format!("v{}", aicrawl_core::SCHEMA_VERSION),
    );
    terminal::kv(&theme, "data dir", ctx.data_dir.display());
    terminal::kv(
        &theme,
        "config",
        format!(
            "{} ({})",
            ctx.config_path.display(),
            if config_present {
                "present"
            } else {
                "defaults"
            }
        ),
    );
    terminal::kv(
        &theme,
        "database",
        format!(
            "{} ({})",
            database_path,
            if database_ok { "ok" } else { "unavailable" }
        ),
    );
    terminal::kv(
        &theme,
        "dashboard",
        if dashboard_ok {
            "embedded".to_string()
        } else {
            "not built (run: cd frontend && npm run build)".to_string()
        },
    );
    terminal::kv(
        &theme,
        "playwright",
        if render_visible {
            "available (optional renderer)".to_string()
        } else {
            format!("{render_detail} (optional; HTTP-only extraction works)")
        },
    );
    terminal::kv(
        &theme,
        "robots.txt",
        if health["security"]["respect_robots_txt"] == json!(true) {
            "enforced".to_string()
        } else {
            "disabled".to_string()
        },
    );
    terminal::kv(
        &theme,
        "stats",
        format!(
            "{} jobs, {} pages, {} results",
            stats["jobs_total"], stats["pages_total"], stats["results_total"]
        ),
    );
    terminal::kv(&theme, "ai", "not required");

    if database_ok && data_dir_ok {
        eprintln!("{} ready", theme.ok_mark());
        Ok(0)
    } else {
        eprintln!(
            "{} fix the database or data directory first",
            theme.error_mark()
        );
        Ok(1)
    }
}

#[allow(dead_code)]
fn duration(ms: u64) -> String {
    commands::human_duration(ms)
}

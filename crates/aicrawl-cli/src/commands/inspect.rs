//! `aicrawl inspect` — structural report for one page.

use std::path::PathBuf;

use aicrawl_core::error::Result;
use clap::Args;

use crate::commands::{self, write_json, Ctx};

/// Arguments for `aicrawl inspect`.
#[derive(Debug, Args)]
pub struct InspectArgs {
    /// URL to inspect.
    pub url: String,
    /// Render with Playwright before inspecting.
    #[arg(long)]
    pub render: bool,
    /// Write JSON to this file instead of stdout.
    #[arg(long, short, value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// Print compact JSON.
    #[arg(long)]
    pub compact: bool,
}

/// Runs `aicrawl inspect`.
pub async fn run(ctx: &Ctx, args: InspectArgs) -> Result<i32> {
    let engine = ctx.engine().await?;
    let report = engine.inspect(&args.url, args.render).await?;
    let value = serde_json::to_value(&report)?;
    write_json(&value, args.output.as_deref(), !args.compact)?;
    if args.output.is_some() && !args.compact {
        eprintln!(
            "{} inspected {} ({}, {} words)",
            ctx.theme.ok_mark(),
            report.final_url,
            commands::human_duration(report.elapsed_ms),
            report.word_count
        );
    }
    Ok(0)
}

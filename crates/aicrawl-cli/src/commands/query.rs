//! `aicrawl jobs` and `aicrawl results` — inspect stored work.

use std::path::PathBuf;

use aicrawl_core::error::Result;
use clap::Args;
use serde_json::json;

use crate::commands::{write_json, Ctx};

/// Arguments for `aicrawl jobs`.
#[derive(Debug, Args)]
pub struct JobsArgs {
    /// Filter by status (queued, running, succeeded, failed, cancelled).
    #[arg(long)]
    pub status: Option<String>,
    /// Maximum rows to return.
    #[arg(long, default_value_t = 20)]
    pub limit: i64,
    /// Write JSON to this file instead of stdout.
    #[arg(long, short, value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// Print compact JSON.
    #[arg(long)]
    pub compact: bool,
}

/// Arguments for `aicrawl results`.
#[derive(Debug, Args)]
pub struct ResultsArgs {
    /// Job identifier.
    pub job_id: String,
    /// Export format: json (default) or jsonl.
    #[arg(long, default_value = "json")]
    pub format: String,
    /// Write the dataset to this file instead of stdout.
    #[arg(long, short, value_name = "PATH")]
    pub output: Option<PathBuf>,
}

/// Runs `aicrawl jobs`.
pub async fn jobs(ctx: &Ctx, args: JobsArgs) -> Result<i32> {
    let engine = ctx.engine().await?;
    let jobs = engine
        .store()
        .list_jobs(args.status.as_deref(), args.limit, 0)
        .await?;
    let value = json!({ "success": true, "data": { "jobs": jobs } });
    write_json(&value, args.output.as_deref(), !args.compact)?;
    Ok(0)
}

/// Runs `aicrawl results`.
pub async fn results(ctx: &Ctx, args: ResultsArgs) -> Result<i32> {
    let engine = ctx.engine().await?;
    let dataset = engine.export_dataset(&args.job_id, &args.format).await?;
    match args.output {
        Some(path) => {
            std::fs::write(&path, format!("{dataset}\n"))?;
            eprintln!("{} wrote {}", ctx.theme.ok_mark(), path.display());
        }
        None => println!("{dataset}"),
    }
    Ok(0)
}

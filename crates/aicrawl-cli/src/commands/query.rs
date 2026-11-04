//! `aicrawl jobs` and `aicrawl results`

use clap::Args;
use serde_json::json;

use aicrawl_core::error::{AicrawlError, Result};
use aicrawl_core::storage::job_status;

use super::{human_duration, Ctx};
use crate::terminal;

/// Arguments for `aicrawl jobs`.
#[derive(Debug, Args)]
pub struct JobsArgs {
    /// Filter by status: queued, running, succeeded, failed, cancelled.
    #[arg(long)]
    pub status: Option<String>,

    /// Maximum jobs to show.
    #[arg(long, default_value_t = 20)]
    pub limit: i64,

    /// Print JSON.
    #[arg(long)]
    pub json: bool,
}

/// Lists stored jobs.
pub async fn jobs(ctx: &Ctx, args: JobsArgs) -> Result<()> {
    let theme = ctx.theme;
    if let Some(status) = args.status.as_deref() {
        if !matches!(
            status,
            job_status::QUEUED
                | job_status::RUNNING
                | job_status::SUCCEEDED
                | job_status::FAILED
                | job_status::CANCELLED
        ) {
            return Err(AicrawlError::InvalidInput(format!(
                "unknown status '{status}'"
            )));
        }
    }
    let engine = ctx.engine().await?;
    let jobs = engine
        .store()
        .list_jobs(args.status.as_deref(), args.limit, 0)
        .await?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&jobs)?);
        return Ok(());
    }

    terminal::print_banner(&theme);
    terminal::section(&theme, &format!("Jobs ({} shown)", jobs.len()));

    let rows: Vec<Vec<String>> = jobs
        .iter()
        .map(|job| {
            vec![
                job.id.clone(),
                job.status.clone(),
                job.kind.clone(),
                job.created_at.chars().take(19).collect(),
                job.pages_crawled.to_string(),
                job.url.clone(),
            ]
        })
        .collect();
    terminal::table(
        &theme,
        &["id", "status", "kind", "created", "pages", "url"],
        &rows,
    );
    terminal::section(&theme, "Hints");
    println!(
        "  {} {}",
        theme.arrow(),
        theme.dim("aicrawl jobs --status succeeded")
    );
    println!(
        "  {} {}",
        theme.arrow(),
        theme.dim("aicrawl results --job <job-id>")
    );
    Ok(())
}

/// Arguments for `aicrawl results`.
#[derive(Debug, Args)]
pub struct ResultsArgs {
    /// Result id to print in full.
    pub id: Option<String>,

    /// Filter by job id.
    #[arg(long)]
    pub job: Option<String>,

    /// Maximum results to list.
    #[arg(long, default_value_t = 20)]
    pub limit: i64,

    /// Print JSON.
    #[arg(long)]
    pub json: bool,

    /// Write the selected result's data to a file.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<std::path::PathBuf>,
}

/// Lists or prints stored results.
pub async fn results(ctx: &Ctx, args: ResultsArgs) -> Result<()> {
    let theme = ctx.theme;
    let engine = ctx.engine().await?;

    if let Some(id) = args.id.as_deref() {
        let result = engine
            .store()
            .get_result(id)
            .await?
            .ok_or_else(|| AicrawlError::NotFound(format!("result {id}")))?;
        let payload = if args.json {
            serde_json::to_string_pretty(&json!({
                "id": result.id,
                "job_id": result.job_id,
                "url": result.url,
                "query": result.query,
                "mode": result.mode,
                "format": result.format,
                "validated": result.is_validated(),
                "provider": result.provider,
                "warnings": result.warnings(),
                "duration_ms": result.duration_ms,
                "created_at": result.created_at,
                "data": result.data(),
            }))?
        } else {
            serde_json::to_string_pretty(&result.data())?
        };

        match args.output.as_ref() {
            Some(path) => {
                std::fs::write(path, format!("{payload}\n"))?;
                eprintln!(
                    "{} Wrote result to {}",
                    theme.ok_mark(),
                    theme.bold(path.display())
                );
            }
            None => println!("{payload}"),
        }
        if !args.json {
            eprintln!();
            eprintln!(
                "{} {} · {} · {}",
                theme.ok_mark(),
                theme.success("Result"),
                theme.info(format!("mode {}", result.mode)),
                theme.dim(format!(
                    "{} · validated {} · {}",
                    human_duration(result.duration_ms as u64),
                    result.is_validated(),
                    result.created_at
                ))
            );
            if let Some(query) = result.query.as_deref() {
                eprintln!("  {} {}", theme.dim("query"), query);
            }
            eprintln!("  {} {}", theme.dim("url"), result.url);
            eprintln!("  {} job {}", theme.dim("job"), result.job_id);
            for warning in result.warnings() {
                eprintln!("  {} {}", theme.warn_mark(), theme.warn(warning));
            }
        }
        return Ok(());
    }

    let results = match args.job.as_deref() {
        Some(job) => engine.store().results_for_job(job).await?,
        None => engine.store().list_results(args.limit, 0).await?,
    };

    if args.json {
        let items: Vec<serde_json::Value> = results
            .iter()
            .map(|result| {
                json!({
                    "id": result.id,
                    "job_id": result.job_id,
                    "url": result.url,
                    "query": result.query,
                    "mode": result.mode,
                    "validated": result.is_validated(),
                    "created_at": result.created_at,
                    "data": result.data(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    terminal::print_banner(&theme);
    terminal::section(&theme, &format!("Results ({} shown)", results.len()));
    let rows: Vec<Vec<String>> = results
        .iter()
        .map(|result| {
            vec![
                result.id.clone(),
                result.mode.clone(),
                result.validated.to_string(),
                result.created_at.chars().take(19).collect(),
                result
                    .query
                    .clone()
                    .unwrap_or_else(|| "—".into())
                    .chars()
                    .take(40)
                    .collect(),
                result.url.clone(),
            ]
        })
        .collect();
    terminal::table(
        &theme,
        &["id", "mode", "schema", "created", "query", "url"],
        &rows,
    );
    terminal::section(&theme, "Hint");
    println!(
        "  {} {}",
        theme.arrow(),
        theme.dim("aicrawl results <result-id>")
    );
    Ok(())
}

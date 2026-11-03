//! `aicrawl crawl <url>`

use clap::Args;

use aicrawl_core::crawler::{CrawlOptions, ProgressSink};
use aicrawl_core::error::Result;

use super::{human_duration, Ctx};
use crate::terminal::{self, Spinner};

/// Arguments for `aicrawl crawl`.
#[derive(Debug, Args)]
pub struct CrawlArgs {
    /// Starting URL.
    pub url: String,

    /// Maximum pages to fetch (1-500).
    #[arg(long, default_value_t = 10)]
    pub max_pages: u32,

    /// Maximum link depth (0-5).
    #[arg(long, default_value_t = 2)]
    pub max_depth: u32,

    /// Optionally extract from every crawled page with this query.
    #[arg(short, long)]
    pub query: Option<String>,

    /// Render JavaScript with the optional Playwright bridge.
    #[arg(long)]
    pub render: bool,

    /// Follow links to other domains as well.
    #[arg(long)]
    pub allow_external: bool,

    /// Print the stored job id only.
    #[arg(long)]
    pub quiet: bool,
}

/// Runs a crawl in the foreground with a live spinner.
pub async fn run(ctx: &Ctx, args: CrawlArgs) -> Result<()> {
    let theme = ctx.theme;
    let mut options = CrawlOptions::new(args.url.clone());
    options.max_pages = args.max_pages;
    options.max_depth = args.max_depth;
    options.query = args.query.clone();
    options.render = args.render;
    options.same_domain = !args.allow_external;
    let (_, pages, depth) = options.validate()?;

    let engine = ctx.engine().await?;
    let job = engine.create_crawl_job(&options).await?;

    if args.quiet {
        println!("{}", job.id);
        return Ok(());
    }

    let spinner = Spinner::start(
        &theme,
        format!(
            "Crawling {} (max {pages} pages, depth {depth})",
            job.url
        ),
    );
    let bar = spinner.bar();
    let sink: ProgressSink = std::sync::Arc::new(move |progress| {
        bar.set_message(format!(
            "{} {}/{} · {} queued · {}",
            if progress.ok { "fetched" } else { "failed " },
            progress.pages_fetched,
            progress.max_pages,
            progress.queued,
            progress.url
        ));
    });

    let summary = match engine
        .run_crawl_job(&job.id, options, Some(sink))
        .await
    {
        Ok(summary) => summary,
        Err(err) => {
            spinner.fail();
            return Err(err);
        }
    };
    spinner.finish(
        &theme,
        format!(
            "Crawl finished in {}",
            human_duration(summary.duration_ms)
        ),
    );

    terminal::section(&theme, "Crawl summary");
    terminal::kv(&theme, "job", theme.accent(&job.id));
    terminal::kv(&theme, "pages fetched", summary.pages_fetched);
    terminal::kv(&theme, "pages stored", summary.pages_succeeded);
    terminal::kv(&theme, "errors", summary.errors);
    terminal::kv(&theme, "results saved", summary.results_saved);
    if summary.cancelled {
        terminal::kv(&theme, "state", theme.warn("cancelled"));
    }
    if summary.timed_out {
        terminal::kv(&theme, "state", theme.warn("timed out (budget reached)"));
    }
    if !summary.warnings.is_empty() {
        terminal::section(&theme, "Warnings");
        for warning in summary.warnings.iter().take(10) {
            println!("  {} {}", theme.warn_mark(), theme.warn(warning));
        }
    }

    let pages = engine.store().list_pages(&job.id).await.unwrap_or_default();
    if !pages.is_empty() {
        terminal::section(&theme, "Pages");
        let rows: Vec<Vec<String>> = pages
            .iter()
            .take(30)
            .map(|page| {
                vec![
                    page.depth.to_string(),
                    page.title.clone().unwrap_or_else(|| "(untitled)".into()),
                    page.status_code
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "—".into()),
                    page.url.clone(),
                ]
            })
            .collect();
        terminal::table(&theme, &["d", "title", "status", "url"], &rows);
    }

    eprintln!();
    eprintln!(
        "{} {}",
        theme.arrow(),
        theme.dim(format!("Inspect with: aicrawl jobs --status succeeded · aicrawl results --job {}", job.id))
    );
    Ok(())
}

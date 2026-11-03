//! `aicrawl inspect <url>`

use clap::Args;

use aicrawl_core::error::Result;

use super::Ctx;
use crate::terminal::{self, Spinner};

/// Arguments for `aicrawl inspect`.
#[derive(Debug, Args)]
pub struct InspectArgs {
    /// Page to inspect.
    pub url: String,

    /// Render JavaScript with the optional Playwright bridge.
    #[arg(long)]
    pub render: bool,

    /// Print the report as JSON.
    #[arg(long)]
    pub json: bool,
}

/// Fetches and describes a page without using AI.
pub async fn run(ctx: &Ctx, args: InspectArgs) -> Result<()> {
    let theme = ctx.theme;
    let engine = ctx.engine().await?;

    let spinner = Spinner::start(&theme, format!("Inspecting {}", args.url));
    let report = match engine.inspect(&args.url, args.render).await {
        Ok(report) => report,
        Err(err) => {
            spinner.fail();
            return Err(err);
        }
    };
    spinner.clear();

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    terminal::print_banner(&theme);
    terminal::section(&theme, "Page");
    terminal::kv(&theme, "url", theme.accent(&report.final_url));
    terminal::kv(
        &theme,
        "status",
        if report.status < 400 {
            theme.success(report.status)
        } else {
            theme.error(report.status)
        },
    );
    terminal::kv(&theme, "content", &report.content_type);
    terminal::kv(&theme, "title", report.title.as_deref().unwrap_or("—"));
    if let Some(description) = report.description.as_deref() {
        terminal::kv(&theme, "description", description);
    }
    if let Some(lang) = report.lang.as_deref() {
        terminal::kv(&theme, "language", lang);
    }
    terminal::kv(&theme, "words", report.word_count);
    terminal::kv(&theme, "text chars", report.text_chars);
    if report.from_cache {
        terminal::kv(&theme, "cache", theme.info("hit"));
    }
    if report.rendered {
        terminal::kv(&theme, "renderer", theme.info("Playwright"));
    }

    terminal::section(&theme, "Structure");
    terminal::kv(&theme, "headings", report.headings.len());
    terminal::kv(&theme, "links", format!("{} ({} sampled)", report.links, report.link_samples.len()));
    terminal::kv(&theme, "images", report.images);
    terminal::kv(&theme, "tables", report.tables);
    terminal::kv(&theme, "JSON-LD types", if report.json_ld_types.is_empty() {
        "none".to_string()
    } else {
        report.json_ld_types.join(", ")
    });
    terminal::kv(&theme, "price mentions", report.prices);

    if !report.headings.is_empty() {
        terminal::section(&theme, "Outline");
        for heading in report.headings.iter().take(25) {
            let indent = "  ".repeat(heading.level.saturating_sub(1) as usize);
            println!(
                "  {}{}",
                indent,
                if heading.level <= 2 {
                    theme.bold(&heading.text)
                } else {
                    heading.text.clone()
                }
            );
        }
    }

    if !report.link_samples.is_empty() {
        terminal::section(&theme, "Links");
        for link in report.link_samples.iter().take(15) {
            println!("  {} {}", theme.arrow(), theme.dim(link));
        }
    }

    for warning in &report.warnings {
        eprintln!("  {} {}", theme.warn_mark(), theme.warn(warning));
    }
    eprintln!();
    eprintln!("{} {}", theme.ok_mark(), theme.dim(format!("completed in {}ms", report.elapsed_ms)));
    Ok(())
}

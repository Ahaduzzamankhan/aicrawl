//! `aicrawl extract <url> --query ...`

use std::io::Write;

use clap::Args;

use aicrawl_core::error::{AicrawlError, Result};
use aicrawl_core::extractor::ExtractRequest;

use super::{human_duration, Ctx};
use crate::terminal::Spinner;

/// Arguments for `aicrawl extract`.
#[derive(Debug, Args)]
pub struct ExtractArgs {
    /// Page to extract from.
    pub url: String,

    /// Natural-language description of the data to extract.
    #[arg(
        short,
        long,
        default_value = "Extract the main content, title and key details of this page"
    )]
    pub query: String,

    /// JSON Schema to validate against: inline JSON or a file path.
    #[arg(long, value_name = "FILE|JSON")]
    pub schema: Option<String>,

    /// Output format: json, markdown or text.
    #[arg(short, long, default_value = "json")]
    pub format: String,

    /// Write the result to a file instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<std::path::PathBuf>,

    /// Render JavaScript with the optional Playwright bridge.
    #[arg(long)]
    pub render: bool,

    /// Skip external AI even when a provider is configured.
    #[arg(long)]
    pub no_ai: bool,

    /// Bypass the SQLite page cache.
    #[arg(long)]
    pub no_cache: bool,
}

/// Runs the extraction and prints or writes the result.
pub async fn run(ctx: &Ctx, args: ExtractArgs) -> Result<()> {
    let theme = ctx.theme;
    let schema = ctx.schema(args.schema.as_deref())?;
    let engine = ctx.engine().await?;

    let request = ExtractRequest {
        url: args.url.clone(),
        query: args.query.clone(),
        schema,
        output_format: args.format.clone(),
        use_ai: !args.no_ai,
        render: args.render,
        use_cache: !args.no_cache,
    };

    let spinner = Spinner::start(
        &theme,
        format!(
            "{} {}",
            if args.render { "Rendering" } else { "Fetching" },
            args.url
        ),
    );
    let outcome = match engine.extract(request).await {
        Ok(run) => run.outcome,
        Err(err) => {
            spinner.fail();
            return Err(err);
        }
    };
    spinner.clear();

    let rendered = if outcome.format == "json" {
        serde_json::to_string_pretty(&outcome.data)?
    } else {
        outcome.output.clone()
    };

    match args.output.as_ref() {
        Some(path) => {
            let mut file = std::fs::File::create(path).map_err(|e| {
                AicrawlError::Io(std::io::Error::new(
                    e.kind(),
                    format!("cannot write {}: {e}", path.display()),
                ))
            })?;
            writeln!(file, "{rendered}")?;
            eprintln!(
                "{} Wrote {} to {}",
                theme.ok_mark(),
                theme.dim(format!("{} bytes", rendered.len())),
                theme.bold(path.display())
            );
        }
        None => println!("{rendered}"),
    }

    eprintln!();
    eprintln!(
        "{} {} · {} · {}",
        theme.ok_mark(),
        theme.success("Extraction complete"),
        theme.info(outcome.mode.clone()),
        theme.dim(format!(
            "{} total, {} fetch",
            human_duration(outcome.duration_ms),
            human_duration(outcome.fetch_ms)
        ))
    );
    if let Some(provider) = outcome.provider.as_deref() {
        eprintln!("  {} {}", theme.dim("provider"), theme.accent(provider));
    }
    if outcome.validated {
        eprintln!("  {} {}", theme.dim("schema"), theme.success("validated"));
    }
    if let Some(title) = outcome.page.title.as_deref() {
        eprintln!("  {} {}", theme.dim("title"), title);
    }
    eprintln!(
        "  {} {} words · {}",
        theme.dim("page"),
        outcome.page.word_count,
        outcome.page.final_url
    );
    for warning in &outcome.warnings {
        eprintln!("  {} {}", theme.warn_mark(), theme.warn(warning));
    }
    if outcome.page.truncated {
        eprintln!(
            "  {} {}",
            theme.warn_mark(),
            theme.warn("page content was truncated to the configured size limit")
        );
    }
    Ok(())
}

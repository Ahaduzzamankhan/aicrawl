//! AICrawl command line interface.
//!
//! Every extraction command reads a JSON request (schema v1) and prints the
//! shared JSON response, so the CLI composes with `jq`, scripts and agents.

mod commands;
mod terminal;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// AICrawl — deterministic, JSON-first web data extraction.
#[derive(Debug, Parser)]
#[command(
    name = "aicrawl",
    version = aicrawl_core::VERSION,
    about = "Deterministic, JSON-first web data extraction",
    long_about = "Extract structured data from permitted websites with CSS/XPath rules.\n\n\
No AI model or API key is required. Requests and responses are JSON (schema v1).\n\n\
Examples:\n  aicrawl extract --input request.json\n  aicrawl extract --stdin < request.json\n  aicrawl extract --url https://example.com --output result.json\n  aicrawl crawl --input crawl.json\n  aicrawl batch --input urls.json\n  aicrawl map https://example.com\n  aicrawl inspect https://example.com\n  aicrawl api\n  aicrawl mcp",
    arg_required_else_help = true
)]
pub struct Cli {
    /// Disable colored diagnostics (also honours NO_COLOR).
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Configuration file (defaults to <data-dir>/config.toml).
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Data directory for the SQLite database and configuration.
    #[arg(long, global = true, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Extract structured data from one page using a JSON request.
    Extract(commands::structured::ExtractArgs),
    /// Crawl a site and project every page through the request rules.
    Crawl(commands::structured::CrawlArgs),
    /// Apply one request to several URLs.
    Batch(commands::structured::BatchArgs),
    /// Discover URLs reachable from a page.
    Map(commands::structured::MapArgs),
    /// Validate a request without any network access.
    Validate(commands::structured::ValidateArgs),
    /// Describe a page's structure (metadata, JSON-LD, headings, links).
    Inspect(commands::inspect::InspectArgs),
    /// Serve the REST API and dashboard.
    Api(commands::server::ApiArgs),
    /// Serve MCP over stdio (default) or streamable HTTP.
    Mcp(commands::server::McpArgs),
    /// List stored jobs.
    Jobs(commands::query::JobsArgs),
    /// Export a job's stored results.
    Results(commands::query::ResultsArgs),
    /// Show or create the configuration file.
    Config {
        #[command(subcommand)]
        action: commands::config_cmd::ConfigCommand,
    },
    /// Check the installation, database and optional renderer.
    Doctor(commands::doctor::DoctorArgs),
    /// Print version information.
    Version,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let theme = terminal::Theme::detect(cli.no_color);

    match commands::run(cli, theme).await {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{} {}", theme.error_mark(), theme.error(&error.to_string()));
            std::process::exit(1);
        }
    }
}

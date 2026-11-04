//! AICrawl command line interface.
//!
//! Humans and scripts use the same engine as the REST API and MCP server.

mod commands;
mod terminal;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// AICrawl — AI-powered web extraction engine.
#[derive(Debug, Parser)]
#[command(
    name = "aicrawl",
    version = aicrawl_core::VERSION,
    about = "AI-powered web extraction engine",
    long_about = "Crawl permitted websites and extract structured data for humans, scripts and AI agents.\n\n\
Examples:\n  aicrawl extract https://example.com --query \"Extract product names and prices\"\n  aicrawl crawl https://example.com --max-pages 20\n  aicrawl inspect https://example.com\n  aicrawl start\n  aicrawl mcp stdio",
    arg_required_else_help = true
)]
pub struct Cli {
    /// Disable colored output (also honours NO_COLOR).
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Configuration file to use (defaults to <data-dir>/config.toml).
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Data directory for the SQLite database, key and config.
    #[arg(long, global = true, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the REST API, dashboard and MCP server.
    Start(commands::server::StartArgs),
    /// Stop a daemonized AICrawl started with `start --daemon`.
    Stop,
    /// Show server status and storage statistics.
    Status,
    /// Extract structured data from one web page.
    Extract(commands::extract::ExtractArgs),
    /// Crawl a website with depth and page limits.
    Crawl(commands::crawl::CrawlArgs),
    /// Inspect a page's structure without using AI.
    Inspect(commands::inspect::InspectArgs),
    /// Run or connect to the MCP server.
    Mcp {
        #[command(subcommand)]
        action: commands::server::McpCommand,
    },
    /// Run the REST API.
    Api {
        #[command(subcommand)]
        action: commands::server::ApiCommand,
    },
    /// List crawl and extraction jobs.
    Jobs(commands::query::JobsArgs),
    /// Show stored extraction results.
    Results(commands::query::ResultsArgs),
    /// Show or edit configuration.
    Config {
        #[command(subcommand)]
        action: commands::config_cmd::ConfigCommand,
    },
    /// Check the local installation and external integrations.
    Doctor(commands::doctor::DoctorArgs),
    /// Print version information.
    Version,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let theme = terminal::Theme::detect(cli.no_color);

    if let Err(error) = commands::run(cli, theme).await {
        eprintln!("{} {}", theme.err_mark(), theme.error(error));
        std::process::exit(1);
    }
}

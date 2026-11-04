//! CLI command implementations.

pub mod config_cmd;
pub mod crawl;
pub mod doctor;
pub mod extract;
pub mod inspect;
pub mod query;
pub mod server;

use std::path::PathBuf;

use aicrawl_core::config::{self, AppConfig};
use aicrawl_core::engine::AicrawlEngine;
use aicrawl_core::error::{AicrawlError, Result};
use serde_json::Value;

use crate::terminal::Theme;
use crate::{Cli, Command};

/// Shared CLI context: resolved config, data directory and theme.
pub struct Ctx {
    pub config: AppConfig,
    pub data_dir: PathBuf,
    pub config_path: PathBuf,
    pub theme: Theme,
}

impl Ctx {
    /// Resolves the config file and data directory for this invocation.
    pub fn from_cli(cli: &Cli, theme: Theme) -> Result<Self> {
        let data_dir = cli
            .data_dir
            .clone()
            .unwrap_or_else(aicrawl_core::config::data_dir);
        config::ensure_data_dir(&data_dir)?;
        let config_path = match &cli.config {
            Some(path) => path.clone(),
            None => config::config_path(&data_dir),
        };
        let mut config = AppConfig::load_or_default(&config_path)?;
        if cli.data_dir.is_some() && config.storage.database_path.is_none() {
            config.storage.database_path = Some(data_dir.join("aicrawl.db"));
        }
        Ok(Self {
            config,
            data_dir,
            config_path,
            theme,
        })
    }

    /// Opens the shared engine.
    pub async fn engine(&self) -> Result<AicrawlEngine> {
        AicrawlEngine::open(self.config.clone()).await
    }

    /// Path of the JSON schema argument, either inline or a filename.
    pub fn schema(&self, input: Option<&str>) -> Result<Option<Value>> {
        read_schema(input)
    }

    /// PID file used by `start`/`stop`/`status`.
    pub fn pid_path(&self) -> PathBuf {
        self.data_dir.join("aicrawl.pid")
    }

    /// Log file used when daemonizing.
    pub fn log_path(&self) -> PathBuf {
        self.data_dir.join("aicrawl.log")
    }
}

/// Reads a schema passed inline (`{...}`) or from a file.
pub fn read_schema(input: Option<&str>) -> Result<Option<Value>> {
    let Some(raw) = input else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        let value = serde_json::from_str(trimmed).map_err(|e| {
            AicrawlError::InvalidInput(format!("inline schema is not valid JSON: {e}"))
        })?;
        return Ok(Some(value));
    }
    let contents = std::fs::read_to_string(trimmed)
        .map_err(|e| AicrawlError::InvalidInput(format!("cannot read schema '{trimmed}': {e}")))?;
    let value = serde_json::from_str(&contents).map_err(|e| {
        AicrawlError::InvalidInput(format!("schema '{trimmed}' is not valid JSON: {e}"))
    })?;
    Ok(Some(value))
}

/// Formats milliseconds for humans.
pub fn human_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

/// Dispatches the parsed command.
pub async fn run(cli: Cli, theme: Theme) -> Result<()> {
    let ctx = Ctx::from_cli(&cli, theme)?;
    match cli.command {
        Command::Start(args) => server::start(&ctx, args).await,
        Command::Stop => server::stop(&ctx).await,
        Command::Status => server::status(&ctx).await,
        Command::Extract(args) => extract::run(&ctx, args).await,
        Command::Crawl(args) => crawl::run(&ctx, args).await,
        Command::Inspect(args) => inspect::run(&ctx, args).await,
        Command::Mcp { action } => server::mcp(&ctx, action).await,
        Command::Api { action } => server::api(&ctx, action).await,
        Command::Jobs(args) => query::jobs(&ctx, args).await,
        Command::Results(args) => query::results(&ctx, args).await,
        Command::Config { action } => config_cmd::run(&ctx, action).await,
        Command::Doctor(args) => doctor::run(&ctx, args).await,
        Command::Version => {
            crate::terminal::print_banner(&theme);
            println!("{}", theme.bold("Version"));
            crate::terminal::kv(&theme, "aicrawl", aicrawl_core::VERSION);
            crate::terminal::kv(&theme, "rust", rustc_version());
            crate::terminal::kv(&theme, "database", "SQLite (sqlx)");
            crate::terminal::kv(&theme, "data dir", ctx.data_dir.display());
            crate::terminal::kv(&theme, "config", ctx.config_path.display());
            Ok(())
        }
    }
}

fn rustc_version() -> &'static str {
    option_env!("AICRAWL_RUSTC_VERSION").unwrap_or("unknown")
}

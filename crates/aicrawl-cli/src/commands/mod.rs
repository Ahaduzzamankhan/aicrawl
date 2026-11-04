//! CLI command implementations.

pub mod config_cmd;
pub mod doctor;
pub mod inspect;
pub mod query;
pub mod server;
pub mod structured;

use std::io::Read;
use std::path::{Path, PathBuf};

use aicrawl_core::config::{self, AppConfig};
use aicrawl_core::engine::AicrawlEngine;
use aicrawl_core::error::{AicrawlError, Result};
use aicrawl_core::output::Response;
use serde_json::Value;

use crate::terminal::Theme;
use crate::{Cli, Command};

/// Shared CLI context: resolved configuration, paths and colour theme.
pub struct Ctx {
    pub config: AppConfig,
    pub data_dir: PathBuf,
    pub config_path: PathBuf,
    pub theme: Theme,
}

impl Ctx {
    /// Resolves the configuration for this invocation.
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
}

/// Reads JSON from `--input <path>` or stdin.
pub fn read_json(path: Option<&Path>, stdin: bool) -> Result<Value> {
    let raw = match (path, stdin) {
        (Some(path), _) => std::fs::read_to_string(path).map_err(|e| {
            AicrawlError::InvalidInput(format!("cannot read {}: {e}", path.display()))
        })?,
        (None, true) => {
            let mut buffer = String::new();
            std::io::stdin()
                .read_to_string(&mut buffer)
                .map_err(|e| AicrawlError::InvalidInput(format!("cannot read stdin: {e}")))?;
            buffer
        }
        (None, false) => return Ok(Value::Null),
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AicrawlError::InvalidInput(
            "input is empty; send a JSON request object".into(),
        ));
    }
    serde_json::from_str(trimmed)
        .map_err(|e| AicrawlError::InvalidInput(format!("input is not valid JSON: {e}")))
}

/// Writes JSON to stdout or to `--output`.
pub fn write_json(value: &Value, output: Option<&Path>, pretty: bool) -> Result<()> {
    let text = if pretty {
        serde_json::to_string_pretty(value)?
    } else {
        serde_json::to_string(value)?
    };
    match output {
        Some(path) => {
            std::fs::write(path, format!("{text}\n")).map_err(|e| {
                AicrawlError::Io(std::io::Error::new(
                    e.kind(),
                    format!("cannot write {}: {e}", path.display()),
                ))
            })?;
        }
        None => println!("{text}"),
    }
    Ok(())
}

/// Process exit code for a response: 0 success, 2 invalid request, 1 failure.
pub fn exit_code(response: &Response) -> i32 {
    if response.success {
        return 0;
    }
    match response
        .error
        .as_ref()
        .map(|error| error.code.as_str())
        .unwrap_or("INTERNAL_ERROR")
    {
        "INVALID_REQUEST" | "FIELD_REQUIRED" | "SCHEMA_VALIDATION_FAILED" => 2,
        _ => 1,
    }
}

/// Prints a response as JSON and reports field issues on stderr.
pub fn emit(response: &Response, output: Option<&Path>, pretty: bool) -> Result<i32> {
    if !response.success {
        if let Some(error) = &response.error {
            eprintln!("error [{}]: {}", error.code, error.message);
        }
        for issue in &response.errors {
            eprintln!("  - {}: {}", issue.field, issue.message);
        }
    } else if !response.errors.is_empty() {
        for issue in &response.errors {
            eprintln!("  warning: {}: {}", issue.field, issue.message);
        }
    }
    let value = serde_json::to_value(response)?;
    write_json(&value, output, pretty)?;
    Ok(exit_code(response))
}

/// Formats milliseconds for humans.
pub fn human_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

/// Dispatches the parsed command and returns its exit code.
pub async fn run(cli: Cli, theme: Theme) -> Result<i32> {
    let ctx = Ctx::from_cli(&cli, theme)?;
    match cli.command {
        Command::Extract(args) => structured::extract(&ctx, args).await,
        Command::Crawl(args) => structured::crawl(&ctx, args).await,
        Command::Batch(args) => structured::batch(&ctx, args).await,
        Command::Map(args) => structured::map(&ctx, args).await,
        Command::Validate(args) => structured::validate(&ctx, args).await,
        Command::Inspect(args) => inspect::run(&ctx, args).await,
        Command::Api(args) => server::api(&ctx, args).await,
        Command::Mcp(args) => server::mcp(&ctx, args).await,
        Command::Jobs(args) => query::jobs(&ctx, args).await,
        Command::Results(args) => query::results(&ctx, args).await,
        Command::Config { action } => config_cmd::run(&ctx, action).await,
        Command::Doctor(args) => doctor::run(&ctx, args).await,
        Command::Version => {
            println!("aicrawl {}", aicrawl_core::VERSION);
            println!("request schema v{}", aicrawl_core::SCHEMA_VERSION);
            println!("database  SQLite (sqlx)");
            println!("data dir  {}", ctx.data_dir.display());
            println!("config    {}", ctx.config_path.display());
            println!("ai        not required (deterministic extraction)");
            Ok(0)
        }
    }
}

//! `aicrawl config`

use clap::Subcommand;

use aicrawl_core::config::AppConfig;
use aicrawl_core::error::{AicrawlError, Result};

use super::Ctx;
use crate::terminal;

/// Configuration operations.
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the effective configuration.
    Show,
    /// Read a dotted key (for example `crawler.max_pages`).
    Get { key: String },
    /// Set a dotted key (for example `crawler.max_pages 20`).
    Set { key: String, value: String },
    /// Print the configuration file path.
    Path,
    /// Write a default configuration file if none exists.
    Init,
}

/// Handles `aicrawl config`.
pub async fn run(ctx: &Ctx, action: ConfigCommand) -> Result<()> {
    let theme = ctx.theme;
    let path = ctx.config_path.clone();

    match action {
        ConfigCommand::Show => {
            let config = AppConfig::load_or_default(&path)?;
            let raw = toml::to_string_pretty(&config)
                .map_err(|e| AicrawlError::Config(format!("cannot render config: {e}")))?;
            terminal::print_banner(&theme);
            terminal::kv(&theme, "file", theme.dim(path.display()));
            println!();
            println!("{raw}");
            println!("{}", theme.dim("Secrets are never stored in this file; provider keys are encrypted in SQLite."));
            Ok(())
        }
        ConfigCommand::Path => {
            println!("{}", path.display());
            Ok(())
        }
        ConfigCommand::Init => {
            if path.exists() {
                println!(
                    "{} {}",
                    theme.warn_mark(),
                    theme.warn(format!("{} already exists; left unchanged", path.display()))
                );
                return Ok(());
            }
            AppConfig::default().save(&path)?;
            println!(
                "{} Wrote default configuration to {}",
                theme.ok_mark(),
                theme.bold(path.display())
            );
            Ok(())
        }
        ConfigCommand::Get { key } => {
            let config = AppConfig::load_or_default(&path)?;
            let value = serde_json::to_value(&config)
                .map_err(|e| AicrawlError::Config(format!("cannot read config: {e}")))?;
            let found = value.pointer(&format!("/{}", key.replace('.', "/")));
            match found {
                Some(value) => {
                    println!("{}", serde_json::to_string_pretty(value)?);
                    Ok(())
                }
                None => Err(AicrawlError::NotFound(format!("config key '{key}'"))),
            }
        }
        ConfigCommand::Set { key, value } => {
            if key.starts_with("ai.api_key") || key.contains("secret") {
                return Err(AicrawlError::InvalidInput(
                    "secrets are never stored in the config file; add provider keys with the dashboard or API"
                        .into(),
                ));
            }
            let config = AppConfig::load_or_default(&path)?;
            let mut doc = toml::Value::try_from(&config)
                .map_err(|e| AicrawlError::Config(format!("cannot read config: {e}")))?;
            let parsed = toml::from_str::<toml::Value>(&format!("value = {value}"))
                .or_else(|_| toml::from_str::<toml::Value>(&format!("value = {value:?}")))
                .map_err(|e| {
                    AicrawlError::InvalidInput(format!(
                        "cannot parse value '{value}' as TOML: {e}"
                    ))
                })?;
            let parsed = parsed
                .get("value")
                .cloned()
                .ok_or_else(|| AicrawlError::InvalidInput("empty value".into()))?;

            set_path(&mut doc, &key, parsed)?;

            let updated: AppConfig = serde::Deserialize::deserialize(doc.clone()).map_err(|e| {
                AicrawlError::InvalidInput(format!("'{key}' is not a valid configuration key: {e}"))
            })?;
            updated.save(&path)?;
            println!(
                "{} Set {} = {}",
                theme.ok_mark(),
                theme.accent(&key),
                theme.dim(value)
            );
            Ok(())
        }
    }
}

fn set_path(doc: &mut toml::Value, key: &str, value: toml::Value) -> Result<()> {
    let parts: Vec<&str> = key.split('.').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return Err(AicrawlError::InvalidInput("key must not be empty".into()));
    }
    let mut current = doc;
    for (index, part) in parts.iter().enumerate() {
        let is_last = index + 1 == parts.len();
        let table = current.as_table_mut().ok_or_else(|| {
            AicrawlError::InvalidInput(format!("'{part}' is not a configuration table"))
        })?;
        if is_last {
            table.insert((*part).to_string(), value.clone());
            return Ok(());
        }
        current = table
            .entry((*part).to_string())
            .or_insert_with(|| toml::Value::Table(Default::default()));
    }
    Ok(())
}

//! `aicrawl config` — inspect and create the configuration file.

use aicrawl_core::error::{AicrawlError, Result};
use clap::Subcommand;

use crate::commands::Ctx;

/// Config subcommands.
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the effective configuration.
    Show {
        /// Print JSON instead of TOML.
        #[arg(long)]
        json: bool,
    },
    /// Print the configuration file path.
    Path,
    /// Write a default configuration file.
    Init {
        /// Overwrite an existing file.
        #[arg(long)]
        force: bool,
    },
}

/// Runs `aicrawl config`.
pub async fn run(ctx: &Ctx, action: ConfigCommand) -> Result<i32> {
    match action {
        ConfigCommand::Show { json } => {
            if json {
                println!("{}", serde_json::to_string_pretty(&ctx.config.redacted())?);
            } else {
                let toml = toml::to_string_pretty(&ctx.config)
                    .map_err(|e| AicrawlError::Config(format!("cannot serialize config: {e}")))?;
                println!("{toml}");
            }
        }
        ConfigCommand::Path => println!("{}", ctx.config_path.display()),
        ConfigCommand::Init { force } => {
            if ctx.config_path.exists() && !force {
                return Err(AicrawlError::Config(format!(
                    "{} already exists; pass --force to overwrite",
                    ctx.config_path.display()
                )));
            }
            ctx.config.save(&ctx.config_path)?;
            eprintln!(
                "{} wrote {}",
                ctx.theme.ok_mark(),
                ctx.config_path.display()
            );
        }
    }
    Ok(0)
}

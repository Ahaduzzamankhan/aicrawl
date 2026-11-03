//! AICrawl core engine.
//!
//! The CLI, REST API, MCP server and React dashboard all drive the same
//! [`engine::AicrawlEngine`]; there is a single crawling, extraction and
//! storage implementation.

pub mod ai;
pub mod api;
pub mod browser;
pub mod config;
pub mod crawler;
pub mod dashboard;
pub mod engine;
pub mod error;
pub mod extractor;
pub mod fetcher;
pub mod mcp;
pub mod parser;
pub mod security;
pub mod storage;
pub mod util;

pub use crawler::CrawlOptions;
pub use engine::AicrawlEngine;
pub use error::{AicrawlError, Result};
pub use extractor::{ExtractOutcome, ExtractRequest};

/// Crate version, surfaced by the CLI and MCP server.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

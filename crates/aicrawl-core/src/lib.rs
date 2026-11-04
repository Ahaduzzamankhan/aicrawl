//! AICrawl core engine.
//!
//! The CLI, REST API, MCP server and React dashboard all drive the same
//! [`engine::AicrawlEngine`]; there is a single fetch, selection, extraction and
//! storage implementation. Requests and responses are versioned JSON.

pub mod api;
pub mod browser;
pub mod config;
pub mod crawl;
pub mod dashboard;
pub mod dedup;
pub mod engine;
pub mod error;
pub mod extraction;
pub mod fetcher;
pub mod map;
pub mod mcp;
pub mod output;
pub mod parser;
pub mod pipeline;
pub mod request;
pub mod safety;
pub mod security;
pub mod selectors;
pub mod storage;
pub mod transform;
pub mod util;

pub use error::{AicrawlError, Result};
pub use output::Response;
pub use request::{Request, SCHEMA_VERSION};

/// Release version from `version.json`, surfaced by the CLI and MCP server.
pub const VERSION: &str = env!("AICRAWL_VERSION");

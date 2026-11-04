//! JSON-driven extraction commands: `extract`, `crawl`, `batch`, `map`, `validate`.

use std::path::PathBuf;

use aicrawl_core::error::{AicrawlError, Result};
use aicrawl_core::output::{Meta, Response};
use aicrawl_core::request::{Action, CrawlSpec, RenderMode, Request};
use clap::Args;
use serde_json::Value;

use crate::commands::{self, write_json, Ctx};

/// Where a request comes from: a file, stdin or a bare URL.
#[derive(Debug, Args)]
pub struct RequestSource {
    /// Request JSON file.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["stdin", "url"])]
    pub input: Option<PathBuf>,
    /// Read the request JSON from stdin.
    #[arg(long, conflicts_with = "url")]
    pub stdin: bool,
    /// Build a minimal request for this URL.
    #[arg(long, value_name = "URL")]
    pub url: Option<String>,
    /// Write the response JSON to this file instead of stdout.
    #[arg(long, short, value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// Print compact JSON instead of pretty JSON.
    #[arg(long)]
    pub compact: bool,
}

impl RequestSource {
    fn load(&self) -> Result<Option<Value>> {
        if self.input.is_none() && !self.stdin {
            return Ok(None);
        }
        let value = commands::read_json(self.input.as_deref(), self.stdin)?;
        if value.is_null() {
            return Err(AicrawlError::InvalidInput("request JSON is empty".into()));
        }
        Ok(Some(value))
    }

    fn pretty(&self) -> bool {
        !self.compact
    }
}

/// `aicrawl extract`.
#[derive(Debug, Args)]
pub struct ExtractArgs {
    #[command(flatten)]
    pub source: RequestSource,
    /// Render with Playwright even when the page looks static.
    #[arg(long)]
    pub render: bool,
    /// Bypass the page cache.
    #[arg(long = "no-cache")]
    pub no_cache: bool,
    /// Optional JSON Schema file the extracted data must satisfy.
    #[arg(long, value_name = "PATH")]
    pub schema: Option<PathBuf>,
}

/// `aicrawl crawl`.
#[derive(Debug, Args)]
pub struct CrawlArgs {
    #[command(flatten)]
    pub source: RequestSource,
    /// Maximum pages to fetch (1-500).
    #[arg(long)]
    pub max_pages: Option<u32>,
    /// Maximum link depth (0-5).
    #[arg(long)]
    pub max_depth: Option<u32>,
    /// Skip sitemap.xml discovery.
    #[arg(long = "no-sitemap")]
    pub no_sitemap: bool,
    /// Follow links to other domains.
    #[arg(long = "other-domains")]
    pub other_domains: bool,
    /// Only report pages whose content changed since the previous crawl.
    #[arg(long)]
    pub incremental: bool,
}

impl CrawlArgs {
    fn build(&self) -> Result<Request> {
        let mut request = request_from(&self.source, Action::Crawl)?;
        let mut spec = request.crawl_spec();
        if let Some(max_pages) = self.max_pages {
            spec.max_pages = max_pages;
        }
        if let Some(max_depth) = self.max_depth {
            spec.max_depth = max_depth;
        }
        if self.no_sitemap {
            spec.sitemap = false;
        }
        if self.other_domains {
            spec.same_domain = false;
        }
        if self.incremental {
            spec.incremental = true;
        }
        request.crawl = Some(spec);
        Ok(request)
    }
}

/// `aicrawl batch`.
#[derive(Debug, Args)]
pub struct BatchArgs {
    #[command(flatten)]
    pub source: RequestSource,
    /// Comma-separated URLs (alternative to --input).
    #[arg(long, value_name = "URLS", conflicts_with_all = ["input", "stdin"])]
    pub urls: Option<String>,
}

/// `aicrawl map`.
#[derive(Debug, Args)]
pub struct MapArgs {
    #[command(flatten)]
    pub source: RequestSource,
    /// Maximum pages to visit (1-500).
    #[arg(long)]
    pub max_pages: Option<u32>,
    /// Maximum link depth (0-5).
    #[arg(long)]
    pub max_depth: Option<u32>,
    /// Skip sitemap.xml discovery.
    #[arg(long = "no-sitemap")]
    pub no_sitemap: bool,
}

/// `aicrawl validate`.
#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[command(flatten)]
    pub source: RequestSource,
}

fn request_from(source: &RequestSource, action: Action) -> Result<Request> {
    let mut request = match source.load()? {
        Some(value) => Request::from_json(&value.to_string())?,
        None => match &source.url {
            Some(url) => Request::new(url.clone()),
            None => {
                return Err(AicrawlError::InvalidInput(
                    "provide --input <file>, --stdin or --url".into(),
                ))
            }
        },
    };
    request.action = action;
    Ok(request)
}

/// Runs `aicrawl extract`.
pub async fn extract(ctx: &Ctx, args: ExtractArgs) -> Result<i32> {
    let mut request = request_from(&args.source, Action::Extract)?;
    if args.render {
        request.render = RenderMode::On;
    }
    if args.no_cache {
        request.use_cache = false;
    }
    if let Some(path) = &args.schema {
        request.schema = Some(commands::read_json(Some(path), false)?);
    }
    let engine = ctx.engine().await?;
    let response = engine.handle(request).await;
    commands::emit(
        &response,
        args.source.output.as_deref(),
        args.source.pretty(),
    )
}

/// Runs `aicrawl crawl`.
pub async fn crawl(ctx: &Ctx, args: CrawlArgs) -> Result<i32> {
    let request = args.build()?;
    let engine = ctx.engine().await?;
    let response = engine.handle(request).await;
    commands::emit(
        &response,
        args.source.output.as_deref(),
        args.source.pretty(),
    )
}

/// Runs `aicrawl batch`.
pub async fn batch(ctx: &Ctx, args: BatchArgs) -> Result<i32> {
    let mut request = match &args.urls {
        Some(list) => {
            let urls: Vec<String> = list
                .split(',')
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty())
                .collect();
            Request {
                urls,
                action: Action::Batch,
                ..Request::default()
            }
        }
        None => match args.source.load()? {
            Some(Value::Array(items)) => {
                let urls: Vec<String> = items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect();
                if urls.is_empty() {
                    return Err(AicrawlError::InvalidInput(
                        "URL array must contain at least one string".into(),
                    ));
                }
                Request {
                    urls,
                    action: Action::Batch,
                    ..Request::default()
                }
            }
            Some(value) => Request::from_json(&value.to_string())?,
            None => {
                return Err(AicrawlError::InvalidInput(
                    "provide --input <file>, --stdin or --urls <a,b>".into(),
                ))
            }
        },
    };
    request.action = Action::Batch;
    if request.urls.is_empty() {
        if let Some(url) = request.url.take() {
            request.urls = vec![url];
        }
    }
    let engine = ctx.engine().await?;
    let response = engine.handle(request).await;
    commands::emit(
        &response,
        args.source.output.as_deref(),
        args.source.pretty(),
    )
}

/// Runs `aicrawl map`.
pub async fn map(ctx: &Ctx, args: MapArgs) -> Result<i32> {
    let mut request = request_from(&args.source, Action::Map)?;
    let mut spec: CrawlSpec = request.crawl_spec();
    if let Some(max_pages) = args.max_pages {
        spec.max_pages = max_pages;
    }
    if let Some(max_depth) = args.max_depth {
        spec.max_depth = max_depth;
    }
    if args.no_sitemap {
        spec.sitemap = false;
    }
    request.crawl = Some(spec);
    let engine = ctx.engine().await?;
    let response = engine.handle(request).await;
    commands::emit(
        &response,
        args.source.output.as_deref(),
        args.source.pretty(),
    )
}

/// Runs `aicrawl validate`: parses and validates without network access.
pub async fn validate(ctx: &Ctx, args: ValidateArgs) -> Result<i32> {
    let request = request_from(&args.source, Action::Extract)?;
    let engine = ctx.engine().await?;
    let response = match engine.validate(&request) {
        Ok(()) => Response::ok(
            serde_json::json!({
                "valid": true,
                "version": aicrawl_core::SCHEMA_VERSION,
                "action": request.action.as_str(),
                "targets": request.targets(),
                "fields": request.selectors.keys().cloned().collect::<Vec<_>>(),
            }),
            Meta::default(),
        ),
        Err(err) => Response::failure(&err),
    };
    let output = args.source.output.as_deref();
    commands::emit(&response, output, args.source.pretty())
}

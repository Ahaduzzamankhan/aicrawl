//! `aicrawl api` and `aicrawl mcp` — long-running servers.

use std::net::SocketAddr;
use std::sync::Arc;

use aicrawl_core::error::{AicrawlError, Result};
use clap::Args;

use crate::commands::Ctx;

/// Arguments for `aicrawl api`.
#[derive(Debug, Args)]
pub struct ApiArgs {
    /// Interface to bind (default from config, 127.0.0.1).
    #[arg(long)]
    pub host: Option<String>,
    /// Port to bind (default 8080).
    #[arg(long)]
    pub port: Option<u16>,
}

/// Arguments for `aicrawl mcp`.
#[derive(Debug, Args)]
pub struct McpArgs {
    /// Serve streamable HTTP instead of stdio.
    #[arg(long)]
    pub http: bool,
    /// Interface to bind for HTTP mode (default 127.0.0.1).
    #[arg(long)]
    pub host: Option<String>,
    /// Port to bind for HTTP mode (default 8787).
    #[arg(long)]
    pub port: Option<u16>,
}

/// Runs `aicrawl api` until interrupted.
pub async fn api(ctx: &Ctx, args: ApiArgs) -> Result<i32> {
    let engine = ctx.engine().await?;
    let host = args
        .host
        .unwrap_or_else(|| engine.config().server.api_host.clone());
    let port = args.port.unwrap_or(engine.config().server.api_port);
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| AicrawlError::Config(format!("cannot bind {addr}: {e}")))?;
    let local = listener
        .local_addr()
        .map(|addr| addr.to_string())
        .unwrap_or(addr);

    eprintln!("{} AICrawl API on http://{local}", ctx.theme.ok_mark());
    eprintln!("  dashboard   http://{local}/");
    eprintln!("  extract     POST http://{local}/api/v1/extract");
    eprintln!("  health      GET  http://{local}/api/v1/health");
    eprintln!("  MCP stdio   aicrawl mcp");
    eprintln!("{}", ctx.theme.dim("press Ctrl+C to stop"));

    axum::serve(listener, aicrawl_core::api::router(engine))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| AicrawlError::Other(format!("API server error: {e}")))?;
    Ok(0)
}

/// Runs `aicrawl mcp` over stdio or streamable HTTP.
pub async fn mcp(ctx: &Ctx, args: McpArgs) -> Result<i32> {
    let engine = Arc::new(ctx.engine().await?);
    if args.http {
        let host = args
            .host
            .unwrap_or_else(|| engine.config().server.mcp_host.clone());
        let port = args.port.unwrap_or(engine.config().server.mcp_port);
        let addr: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|e| AicrawlError::Config(format!("invalid MCP address {host}:{port}: {e}")))?;
        eprintln!(
            "{} MCP streamable HTTP on http://{addr}/mcp",
            ctx.theme.ok_mark()
        );
        aicrawl_core::mcp::serve_http(engine, addr, shutdown_signal()).await?;
        return Ok(0);
    }

    eprintln!(
        "{} MCP stdio ready ({} tools)",
        ctx.theme.ok_mark(),
        aicrawl_core::mcp::tool_catalog().len()
    );
    aicrawl_core::mcp::serve_stdio(engine).await?;
    Ok(0)
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

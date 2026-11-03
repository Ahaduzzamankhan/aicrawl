//! Server lifecycle commands: `start`, `stop`, `status`, `api`, `mcp`.

use std::net::SocketAddr;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use clap::{Args, Subcommand};
use tokio::sync::Notify;

use aicrawl_core::engine::AicrawlEngine;
use aicrawl_core::error::{AicrawlError, Result};

use super::Ctx;
use crate::terminal;

/// Arguments for `aicrawl start`.
#[derive(Debug, Args)]
pub struct StartArgs {
    /// REST API/dashboard port (default 8080).
    #[arg(long)]
    pub api_port: Option<u16>,

    /// MCP HTTP port (default 8787).
    #[arg(long)]
    pub mcp_port: Option<u16>,

    /// Run in the background and write a log file.
    #[arg(long)]
    pub daemon: bool,
}

/// MCP transport commands.
#[derive(Debug, Subcommand)]
pub enum McpCommand {
    /// Serve MCP over streamable HTTP.
    Start {
        /// Port to listen on (default 8787).
        #[arg(long)]
        port: Option<u16>,
    },
    /// Serve MCP over stdio for local agent integrations.
    Stdio,
}

/// API commands.
#[derive(Debug, Subcommand)]
pub enum ApiCommand {
    /// Serve the REST API and dashboard.
    Start {
        /// Port to listen on (default 8080).
        #[arg(long)]
        port: Option<u16>,
    },
}

/// Starts the full stack (API + dashboard + MCP), optionally daemonized.
pub async fn start(ctx: &Ctx, args: StartArgs) -> Result<()> {
    let api_port = args.api_port.unwrap_or(ctx.config.server.api_port);
    let mcp_port = args.mcp_port.unwrap_or(ctx.config.server.mcp_port);

    if args.daemon {
        return daemonize(ctx, api_port, mcp_port);
    }

    let mut config = ctx.config.clone();
    config.server.api_port = api_port;
    config.server.mcp_port = mcp_port;
    let engine = Arc::new(AicrawlEngine::open(config).await?);
    let theme = ctx.theme;

    terminal::print_banner(&theme);
    println!("{} {}", theme.ok_mark(), theme.success("Core engine initialized"));
    let database = if engine.store().ping().await.is_ok() {
        theme.success("SQLite database connected")
    } else {
        theme.error("SQLite database unreachable")
    };
    println!("{} {}", theme.ok_mark(), database);
    println!(
        "{} {}",
        theme.ok_mark(),
        match engine.ai_status().await {
            status if status.configured => theme.success(status.detail),
            status => theme.warn(status.detail),
        }
    );
    println!("{} {}", theme.ok_mark(), theme.success("MCP server initialized"));

    let host = engine.config().server.api_host.clone();
    let mcp_host = engine.config().server.mcp_host.clone();
    let api_address = parse_address(&host, engine.config().server.api_port)?;
    let mcp_address = parse_address(&mcp_host, engine.config().server.mcp_port)?;

    // Bind both listeners before announcing readiness, so a busy port is a
    // startup error instead of a server that looks healthy but is unreachable.
    let api_listener = aicrawl_core::api::bind(api_address).await?;
    let mcp_listener = aicrawl_core::mcp::bind(mcp_address).await?;

    write_pid(ctx)?;
    let notify = Arc::new(Notify::new());

    let api_engine = engine.clone();
    let api_notify = notify.clone();
    let api_task = tokio::spawn(async move {
        aicrawl_core::api::serve_listener(api_engine, api_listener, async move {
            api_notify.notified().await
        })
        .await
    });

    let mcp_engine = engine.clone();
    let mcp_notify = notify.clone();
    let mcp_task = tokio::spawn(async move {
        aicrawl_core::mcp::serve_http_listener(mcp_engine, mcp_listener, async move {
            mcp_notify.notified().await
        })
        .await
    });

    println!();
    println!("{} {}", theme.ok_mark(), theme.success("AICrawl is ready!"));
    println!();
    println!("{} {}", theme.dim("Dashboard:"), theme.accent(format!("http://{host}:{api_port}")));
    println!("{} {}", theme.dim("REST API: "), theme.accent(format!("http://{host}:{api_port}/api/v1")));
    println!("{} {}", theme.dim("MCP:      "), theme.accent(format!("http://{mcp_host}:{mcp_port}/mcp")));
    println!("{}", theme.dim("Press Ctrl+C to stop."));

    tokio::signal::ctrl_c().await.ok();
    eprintln!(
        "{} {}",
        theme.warn_mark(),
        theme.warn("shutting down…")
    );
    notify.notify_waiters();
    let api_result = api_task.await.unwrap_or_else(|e| {
        Err(AicrawlError::Other(format!("API task failed: {e}")))
    });
    let mcp_result = mcp_task.await.unwrap_or_else(|e| {
        Err(AicrawlError::Other(format!("MCP task failed: {e}")))
    });
    remove_pid(ctx);

    api_result?;
    mcp_result?;
    eprintln!("{} {}", theme.ok_mark(), theme.success("stopped"));
    Ok(())
}

/// Starts the REST API and dashboard only.
pub async fn api(ctx: &Ctx, action: ApiCommand) -> Result<()> {
    let port = match action {
        ApiCommand::Start { port } => port.unwrap_or(ctx.config.server.api_port),
    };
    let mut config = ctx.config.clone();
    config.server.api_port = port;
    let host = config.server.api_host.clone();
    let engine = Arc::new(AicrawlEngine::open(config).await?);
    let address = parse_address(&host, port)?;
    let theme = ctx.theme;

    terminal::print_banner(&theme);
    println!("{} {}", theme.ok_mark(), theme.success("API initialized"));
    if cfg!(debug_assertions) {
        println!(
            "  {} {}",
            theme.warn_mark(),
            theme.warn("debug build: use `cargo build --release` for production")
        );
    }
    println!(
        "{} {}",
        theme.ok_mark(),
        theme.success(format!("Listening: http://{host}:{port}"))
    );

    write_pid(ctx)?;
    let result = tokio::select! {
        result = aicrawl_core::api::serve(engine, address, std::future::pending()) => result,
        _ = tokio::signal::ctrl_c() => Ok(()),
    };
    remove_pid(ctx);
    result
}

/// Starts or connects MCP.
pub async fn mcp(ctx: &Ctx, action: McpCommand) -> Result<()> {
    match action {
        McpCommand::Stdio => {
            // stdio is a machine channel: logs go to stderr, never stdout.
            let _ = tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
                )
                .with_writer(std::io::stderr)
                .try_init();
            let engine = Arc::new(ctx.engine().await?);
            aicrawl_core::mcp::serve_stdio(engine).await
        }
        McpCommand::Start { port } => {
            let port = port.unwrap_or(ctx.config.server.mcp_port);
            let mut config = ctx.config.clone();
            config.server.mcp_port = port;
            let host = config.server.mcp_host.clone();
            let engine = Arc::new(AicrawlEngine::open(config).await?);
            let address = parse_address(&host, port)?;
            let theme = ctx.theme;

            terminal::print_banner(&theme);
            println!(
                "{} {}",
                theme.ok_mark(),
                theme.success(format!("MCP server listening: http://{host}:{port}/mcp"))
            );
            println!(
                "  {} {}",
                theme.dim("tools"),
                theme.dim("extract_website_data, crawl_website, get_page_content, inspect_website, search_website, get_crawl_result, get_crawl_status")
            );
            let result = tokio::select! {
                result = aicrawl_core::mcp::serve_http(engine, address, std::future::pending()) => result,
                _ = tokio::signal::ctrl_c() => Ok(()),
            };
            result
        }
    }
}

/// Stops a daemonized instance.
pub async fn stop(ctx: &Ctx) -> Result<()> {
    let theme = ctx.theme;
    let pid = read_pid(ctx).ok_or_else(|| {
        AicrawlError::NotFound(format!(
            "no PID file at {} (start one with `aicrawl start --daemon`)",
            ctx.pid_path().display()
        ))
    })?;

    #[cfg(windows)]
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
    #[cfg(not(windows))]
    let status = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();

    match status {
        Ok(status) if status.success() => {
            remove_pid(ctx);
            println!(
                "{} {}",
                theme.ok_mark(),
                theme.success(format!("stopped AICrawl (pid {pid})"))
            );
            Ok(())
        }
        Ok(status) => Err(AicrawlError::Other(format!(
            "could not stop pid {pid}: exit {status}"
        ))),
        Err(err) => Err(AicrawlError::Other(format!(
            "could not run the process manager: {err}"
        ))),
    }
}

/// Prints status, storage statistics and reachable endpoints.
pub async fn status(ctx: &Ctx) -> Result<()> {
    let theme = ctx.theme;
    terminal::print_banner(&theme);

    let pid = read_pid(ctx);
    let api_up = probe(&ctx.config.server.api_host, ctx.config.server.api_port).await;
    let mcp_up = probe(&ctx.config.server.mcp_host, ctx.config.server.mcp_port).await;

    terminal::section(&theme, "Server");
    terminal::kv(
        &theme,
        "process",
        match pid {
            Some(pid) => theme.success(format!("running (pid {pid})")),
            None => theme.warn("no PID file (foreground or stopped)"),
        },
    );
    terminal::kv(
        &theme,
        "API",
        if api_up {
            theme.success(format!("http://{}:{}", ctx.config.server.api_host, ctx.config.server.api_port))
        } else {
            theme.dim("not listening")
        },
    );
    terminal::kv(
        &theme,
        "MCP",
        if mcp_up {
            theme.success(format!("http://{}:{}/mcp", ctx.config.server.mcp_host, ctx.config.server.mcp_port))
        } else {
            theme.dim("not listening")
        },
    );

    let engine = ctx.engine().await?;
    let stats = engine.stats().await?;
    let ai = engine.ai_status().await;

    terminal::section(&theme, "Storage");
    terminal::kv(&theme, "database", engine.store().location());
    terminal::kv(&theme, "jobs", stats.jobs_total);
    terminal::kv(&theme, "active jobs", stats.jobs_running);
    terminal::kv(&theme, "pages", stats.pages_total);
    terminal::kv(&theme, "results", stats.results_total);
    terminal::kv(&theme, "websites", stats.websites_total);

    terminal::section(&theme, "Configuration");
    terminal::kv(&theme, "data dir", ctx.data_dir.display());
    terminal::kv(&theme, "config", ctx.config_path.display());
    terminal::kv(
        &theme,
        "AI provider",
        if ai.configured {
            theme.success(ai.detail)
        } else {
            theme.warn(ai.detail)
        },
    );
    terminal::kv(
        &theme,
        "SSRF",
        if ctx.config.security.allow_private_networks {
            theme.warn("private networks allowed")
        } else {
            theme.success("private networks blocked")
        },
    );
    terminal::kv(
        &theme,
        "robots.txt",
        if ctx.config.crawler.respect_robots_txt {
            theme.success("respected")
        } else {
            theme.warn("disabled")
        },
    );
    Ok(())
}

fn daemonize(ctx: &Ctx, api_port: u16, mcp_port: u16) -> Result<()> {
    let theme = ctx.theme;
    if read_pid(ctx).is_some() {
        return Err(AicrawlError::Other(format!(
            "a PID file already exists at {}; run `aicrawl stop` first",
            ctx.pid_path().display()
        )));
    }
    let exe = std::env::current_exe()?;
    let log_path = ctx.log_path();
    let log = std::fs::File::create(&log_path)?;
    let log_err = log.try_clone()?;

    let mut command = std::process::Command::new(exe);
    command
        .arg("start")
        .arg("--api-port")
        .arg(api_port.to_string())
        .arg("--mcp-port")
        .arg(mcp_port.to_string())
        .arg("--data-dir")
        .arg(ctx.data_dir.display().to_string())
        .arg("--config")
        .arg(ctx.config_path.display().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
        command.creation_flags(0x0000_0008 | 0x0000_0200);
    }

    let child = command
        .spawn()
        .map_err(|e| AicrawlError::Other(format!("cannot start background process: {e}")))?;
    println!(
        "{} {}",
        theme.ok_mark(),
        theme.success(format!(
            "AICrawl starting in the background (pid {})",
            child.id()
        ))
    );
    println!(
        "  {} {}",
        theme.dim("dashboard"),
        theme.accent(format!("http://{}:{}", ctx.config.server.api_host, api_port))
    );
    println!(
        "  {} {}",
        theme.dim("look"),
        theme.dim(format!("aicrawl status · log: {}", log_path.display()))
    );
    Ok(())
}

fn parse_address(host: &str, port: u16) -> Result<SocketAddr> {
    // Accept hostnames such as "localhost" in addition to IP literals.
    let normalized = match host {
        "localhost" => "127.0.0.1",
        other => other,
    };
    let address = format!("{normalized}:{port}");
    address
        .parse()
        .map_err(|e| AicrawlError::Config(format!("invalid bind address '{address}': {e}")))
}

fn write_pid(ctx: &Ctx) -> Result<()> {
    std::fs::write(ctx.pid_path(), std::process::id().to_string())?;
    Ok(())
}

fn remove_pid(ctx: &Ctx) {
    let _ = std::fs::remove_file(ctx.pid_path());
}

fn read_pid(ctx: &Ctx) -> Option<u32> {
    std::fs::read_to_string(ctx.pid_path())
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok())
}

async fn probe(host: &str, port: u16) -> bool {
    let host = match host {
        "0.0.0.0" | "::" => "127.0.0.1",
        other => other,
    };
    let address = format!("{host}:{port}");
    tokio::time::timeout(
        Duration::from_millis(400),
        tokio::net::TcpStream::connect(&address),
    )
    .await
    .map(|result| result.is_ok())
    .unwrap_or(false)
}


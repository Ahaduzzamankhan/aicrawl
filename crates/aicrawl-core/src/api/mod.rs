//! REST API and dashboard host.
//!
//! Responses use a consistent envelope:
//! `{"success": true, "data": ...}` or
//! `{"success": false, "error": {"code": "...", "message": "..."}}`.
//! API-key authentication is optional for loopback use and enforced when
//! `api.require_api_key` is enabled.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::crawler::CrawlOptions;
use crate::engine::AicrawlEngine;
use crate::error::{AicrawlError, Result};
use crate::extractor::ExtractRequest;
use crate::mcp::{tool_catalog, ToolInfo};
use crate::security::{hash_api_key, key_prefix};
use crate::storage::{ApiKeyRecord, ExtractedResult, NewProvider};

/// Builds the complete application router (API + dashboard assets).
pub fn router(engine: Arc<AicrawlEngine>) -> Router {
    let api: Router = Router::new()
        .route("/health", get(health))
        .route("/extract", post(extract))
        .route("/crawl", post(start_crawl))
        .route("/jobs", get(list_jobs))
        .route("/jobs/{id}", get(get_job))
        .route("/jobs/{id}/cancel", post(cancel_job))
        .route("/results", get(list_results))
        .route("/results/{id}", get(get_result))
        .route("/stats", get(stats))
        .route("/websites", get(list_websites))
        .route("/settings", get(get_settings).put(put_settings))
        .route("/providers", get(list_providers).post(create_provider))
        .route("/providers/{id}", put(update_provider).delete(delete_provider))
        .route("/providers/{id}/default", post(make_default_provider))
        .route("/providers/{id}/test", post(test_provider))
        .route("/api-keys", get(list_api_keys).post(create_api_key))
        .route("/api-keys/{id}", delete(revoke_api_key))
        .route("/cache", delete(clear_cache))
        .route("/mcp", get(mcp_config))
        .with_state(engine.clone())
        .layer(middleware::from_fn_with_state(
            engine.clone(),
            auth_middleware,
        ));

    // The dashboard is embedded at compile time. An explicit directory is only
    // used for dashboard development, where Vite serves the UI anyway.
    let app: Router = Router::new().nest("/api/v1", api);
    let app: Router = match dashboard_dir(&engine) {
        Some(dist) => {
            let index = dist.join("index.html");
            let assets = ServeDir::new(&dist).not_found_service(ServeFile::new(&index));
            app.fallback_service(assets)
        }
        None if crate::dashboard::available() => app.fallback(dashboard_handler),
        None => app.route("/", get(root_info).with_state(engine.clone())),
    };

    app.layer(TraceLayer::new_for_http())
        .layer(cors_layer(&engine))
}

fn dashboard_dir(engine: &AicrawlEngine) -> Option<PathBuf> {
    if let Some(configured) = engine.config().api.dashboard_dist.clone() {
        if configured.join("index.html").exists() {
            return Some(configured);
        }
    }
    if let Ok(configured) = std::env::var("AICRAWL_DASHBOARD_DIR") {
        let configured = PathBuf::from(configured);
        if configured.join("index.html").exists() {
            return Some(configured);
        }
    }
    None
}

async fn dashboard_handler(uri: Uri) -> Response {
    crate::dashboard::handler(uri).await
}

fn cors_layer(engine: &AicrawlEngine) -> CorsLayer {
    let layer = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .max_age(Duration::from_secs(600));
    let configured = &engine.config().api.cors_origins;
    if configured.is_empty() || configured.iter().any(|origin| origin == "*") {
        return layer.allow_origin(Any);
    }
    let mut origins = Vec::new();
    for origin in configured {
        if let Ok(value) = HeaderValue::from_str(origin) {
            origins.push(value);
        }
    }
    if origins.is_empty() {
        layer.allow_origin(Any)
    } else {
        layer.allow_origin(AllowOrigin::list(origins))
    }
}

/// Binds the API listener so callers can fail before announcing readiness.
pub async fn bind(addr: SocketAddr) -> Result<tokio::net::TcpListener> {
    tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| AicrawlError::Config(format!("cannot bind API listener on {addr}: {e}")))
}

/// Serves the API/dashboard until `shutdown` resolves.
pub async fn serve(
    engine: Arc<AicrawlEngine>,
    addr: SocketAddr,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<()> {
    let listener = bind(addr).await?;
    serve_listener(engine, listener, shutdown).await
}

/// Serves the API/dashboard on an already-bound listener.
pub async fn serve_listener(
    engine: Arc<AicrawlEngine>,
    listener: tokio::net::TcpListener,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<()> {
    axum::serve(listener, router(engine))
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(|e| AicrawlError::Other(format!("API server error: {e}")))?;
    Ok(())
}

// ---------------------------------------------------------------- helpers

fn ok(data: impl serde::Serialize) -> Json<Value> {
    Json(json!({"success": true, "data": data}))
}

fn error_response(err: AicrawlError) -> Response {
    let status =
        StatusCode::from_u16(err.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(json!({
            "success": false,
            "error": {
                "code": err.code(),
                "message": err.to_string(),
                "retryable": err.retryable()
            }
        })),
    )
        .into_response()
}

/// Compact JSON form of a stored extraction result.
pub fn result_json(result: &ExtractedResult) -> Value {
    json!({
        "id": result.id,
        "job_id": result.job_id,
        "url": result.url,
        "query": result.query,
        "mode": result.mode,
        "format": result.format,
        "data": result.data(),
        "warnings": result.warnings(),
        "provider": result.provider,
        "validated": result.is_validated(),
        "duration_ms": result.duration_ms,
        "created_at": result.created_at,
    })
}

fn api_key_json(record: &ApiKeyRecord) -> Value {
    record.redacted()
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        for prefix in ["Bearer ", "bearer "] {
            if let Some(token) = value.strip_prefix(prefix) {
                let token = token.trim();
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
}

async fn auth_middleware(
    State(engine): State<Arc<AicrawlEngine>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    if path.ends_with("/health") || request.method() == axum::http::Method::OPTIONS {
        return next.run(request).await;
    }
    if !engine.config().api.require_api_key {
        return next.run(request).await;
    }

    let Some(token) = bearer_token(request.headers()) else {
        return error_response(AicrawlError::Unauthorized(
            "provide an API key via `Authorization: Bearer <key>` or `X-API-Key`".into(),
        ));
    };
    match engine
        .store()
        .find_active_api_key(&hash_api_key(&token))
        .await
    {
        Ok(Some(record)) => {
            let _ = engine.store().touch_api_key(&record.id).await;
            next.run(request).await
        }
        Ok(None) => error_response(AicrawlError::Unauthorized(
            "the supplied API key is invalid or revoked".into(),
        )),
        Err(err) => error_response(err.into()),
    }
}

// --------------------------------------------------------------- handlers

async fn root_info(State(engine): State<Arc<AicrawlEngine>>) -> Json<Value> {
    ok(json!({
        "name": "AICrawl",
        "version": crate::VERSION,
        "description": "AI-powered web extraction engine",
        "api": "/api/v1",
        "dashboard": "not embedded in this build (build the frontend, then rebuild the binary)",
        "mcp": format!("http://{}:{}/mcp", engine.config().server.mcp_host, engine.config().server.mcp_port),
        "docs": "https://github.com/aicrawl/aicrawl#readme",
        "auth_required": engine.config().api.require_api_key,
    }))
}

async fn health(State(engine): State<Arc<AicrawlEngine>>) -> Json<Value> {
    ok(engine.health().await)
}

async fn extract(
    State(engine): State<Arc<AicrawlEngine>>,
    Json(request): Json<ExtractRequest>,
) -> Response {
    let timeout = Duration::from_secs(engine.config().api.request_timeout_secs.max(5));
    match tokio::time::timeout(timeout, engine.extract(request)).await {
        Ok(Ok(run)) => ok(json!({
            "job": run.job,
            "result": result_json(&run.result),
            "outcome": run.outcome,
        }))
        .into_response(),
        Ok(Err(err)) => error_response(err),
        Err(_) => error_response(AicrawlError::Timeout(timeout.as_millis() as u64)),
    }
}

async fn start_crawl(
    State(engine): State<Arc<AicrawlEngine>>,
    Json(options): Json<CrawlOptions>,
) -> Response {
    match engine.start_crawl(options).await {
        Ok(job) => (
            StatusCode::ACCEPTED,
            ok(json!({"job": job})),
        )
            .into_response(),
        Err(err) => error_response(err),
    }
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_jobs(
    State(engine): State<Arc<AicrawlEngine>>,
    Query(query): Query<ListQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    match engine
        .store()
        .list_jobs(query.status.as_deref(), limit, offset)
        .await
    {
        Ok(jobs) => ok(json!({"jobs": jobs, "limit": limit, "offset": offset})).into_response(),
        Err(err) => error_response(err.into()),
    }
}

async fn get_job(
    State(engine): State<Arc<AicrawlEngine>>,
    Path(id): Path<String>,
) -> Response {
    match engine.store().get_job(&id).await {
        Ok(Some(job)) => {
            let pages = engine.store().list_pages(&id).await.unwrap_or_default();
            let results = engine
                .store()
                .results_for_job(&id)
                .await
                .unwrap_or_default()
                .iter()
                .map(result_json)
                .collect::<Vec<_>>();
            ok(json!({"job": job, "pages": pages, "results": results})).into_response()
        }
        Ok(None) => error_response(AicrawlError::NotFound(format!("job {id}"))),
        Err(err) => error_response(err.into()),
    }
}

async fn cancel_job(
    State(engine): State<Arc<AicrawlEngine>>,
    Path(id): Path<String>,
) -> Response {
    match engine.store().get_job(&id).await {
        Ok(Some(_)) => match engine.cancel_job(&id).await {
            Ok(true) => ok(json!({"job_id": id, "cancelled": true})).into_response(),
            Ok(false) => (
                StatusCode::CONFLICT,
                Json(json!({
                    "success": false,
                    "error": {"code": "job_not_active", "message": "job is not queued or running"}
                })),
            )
                .into_response(),
            Err(err) => error_response(err),
        },
        Ok(None) => error_response(AicrawlError::NotFound(format!("job {id}"))),
        Err(err) => error_response(err.into()),
    }
}

async fn list_results(
    State(engine): State<Arc<AicrawlEngine>>,
    Query(query): Query<ListQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    match engine.store().list_results(limit, offset).await {
        Ok(results) => {
            let items: Vec<Value> = results.iter().map(result_json).collect();
            ok(json!({"results": items, "limit": limit, "offset": offset})).into_response()
        }
        Err(err) => error_response(err.into()),
    }
}

async fn get_result(
    State(engine): State<Arc<AicrawlEngine>>,
    Path(id): Path<String>,
) -> Response {
    match engine.store().get_result(&id).await {
        Ok(Some(result)) => ok(result_json(&result)).into_response(),
        Ok(None) => error_response(AicrawlError::NotFound(format!("result {id}"))),
        Err(err) => error_response(err.into()),
    }
}

async fn stats(State(engine): State<Arc<AicrawlEngine>>) -> Response {
    match engine.stats().await {
        Ok(stats) => {
            let recent_jobs = engine.store().recent_jobs(8).await.unwrap_or_default();
            let recent_results = engine
                .store()
                .list_results(5, 0)
                .await
                .unwrap_or_default()
                .iter()
                .map(result_json)
                .collect::<Vec<_>>();
            let ai = engine.ai_status().await;
            let cache_entries = engine.store().cache_entries().await.unwrap_or(0);
            ok(json!({
                "stats": stats,
                "recent_jobs": recent_jobs,
                "recent_results": recent_results,
                "ai": ai,
                "cache_entries": cache_entries,
            }))
            .into_response()
        }
        Err(err) => error_response(err.into()),
    }
}

async fn list_websites(State(engine): State<Arc<AicrawlEngine>>) -> Response {
    match engine.store().list_websites(100).await {
        Ok(sites) => ok(json!({"websites": sites})).into_response(),
        Err(err) => error_response(err.into()),
    }
}

async fn get_settings(State(engine): State<Arc<AicrawlEngine>>) -> Response {
    let preferences = engine.store().all_settings().await.unwrap_or_default();
    let stats = engine.stats().await.unwrap_or_default();
    let cache_entries = engine.store().cache_entries().await.unwrap_or(0);
    ok(json!({
        "effective": engine.config().redacted(),
        "preferences": preferences,
        "security": {
            "allow_private_networks": engine.config().security.allow_private_networks,
            "allowed_domains": engine.config().security.allowed_domains,
            "blocked_domains": engine.config().security.blocked_domains,
            "max_redirects": engine.config().security.max_redirects,
            "rate_limit_per_minute": engine.config().security.rate_limit_per_minute,
            "respect_robots_txt": engine.config().crawler.respect_robots_txt,
        },
        "storage": {
            "path": engine.store().location(),
            "data_dir": engine.data_dir().display().to_string(),
            "stats": stats,
            "cache_entries": cache_entries,
        },
        "runtime": {
            "version": crate::VERSION,
            "api_requires_key": engine.config().api.require_api_key,
            "mcp_endpoint": format!("http://{}:{}/mcp", engine.config().server.mcp_host, engine.config().server.mcp_port),
        }
    }))
    .into_response()
}

#[derive(Debug, Deserialize)]
struct SettingsPayload {
    #[serde(default)]
    preferences: BTreeMap<String, Value>,
}

async fn put_settings(
    State(engine): State<Arc<AicrawlEngine>>,
    Json(payload): Json<SettingsPayload>,
) -> Response {
    for (key, value) in &payload.preferences {
        let key = key.trim();
        if key.is_empty() || key.len() > 64 {
            return error_response(AicrawlError::InvalidInput(
                "preference keys must be 1-64 characters".into(),
            ));
        }
        let text = match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        if text.len() > 4096 {
            return error_response(AicrawlError::InvalidInput(format!(
                "preference '{key}' exceeds 4096 characters"
            )));
        }
        if let Err(err) = engine.store().set_setting(key, &text).await {
            return error_response(err.into());
        }
    }
    get_settings(State(engine)).await
}

// -------------------------------------------------------------- providers

const PROVIDER_KINDS: [&str; 3] = ["openai", "openai_compatible", "ollama"];

#[derive(Debug, Deserialize)]
struct ProviderPayload {
    name: String,
    kind: String,
    base_url: String,
    model: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    is_default: Option<bool>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    temperature: Option<f32>,
}

async fn list_providers(State(engine): State<Arc<AicrawlEngine>>) -> Response {
    match engine.store().list_providers().await {
        Ok(providers) => {
            let items: Vec<Value> = providers.iter().map(|p| p.redacted()).collect();
            ok(json!({"providers": items, "ai": engine.ai_status().await})).into_response()
        }
        Err(err) => error_response(err.into()),
    }
}

async fn create_provider(
    State(engine): State<Arc<AicrawlEngine>>,
    Json(payload): Json<ProviderPayload>,
) -> Response {
    let kind = payload.kind.trim().to_ascii_lowercase();
    if !PROVIDER_KINDS.contains(&kind.as_str()) {
        return error_response(AicrawlError::InvalidInput(format!(
            "kind must be one of {PROVIDER_KINDS:?}"
        )));
    }
    if payload.name.trim().is_empty() || payload.model.trim().is_empty() {
        return error_response(AicrawlError::InvalidInput(
            "name and model must not be empty".into(),
        ));
    }
    if url::Url::parse(payload.base_url.trim()).is_err() {
        return error_response(AicrawlError::InvalidInput(
            "base_url must be an absolute URL".into(),
        ));
    }
    let encrypted = match payload.api_key.as_deref().map(str::trim) {
        Some(key) if !key.is_empty() => match engine.secrets().encrypt(key) {
            Ok(value) => Some(value),
            Err(err) => return error_response(err),
        },
        _ => None,
    };
    let is_default = payload.is_default.unwrap_or(false);
    match engine
        .store()
        .upsert_provider(NewProvider {
            id: None,
            name: payload.name.trim().to_string(),
            kind,
            base_url: payload.base_url.trim().to_string(),
            model: payload.model.trim().to_string(),
            api_key_enc: encrypted,
            is_default,
            max_tokens: payload.max_tokens.unwrap_or(1024).clamp(64, 8192) as i64,
            temperature: payload.temperature.unwrap_or(0.0).clamp(0.0, 2.0) as f64,
        })
        .await
    {
        Ok(provider) => (
            StatusCode::CREATED,
            ok(json!({"provider": provider.redacted()})),
        )
            .into_response(),
        Err(err) => error_response(err.into()),
    }
}

async fn update_provider(
    State(engine): State<Arc<AicrawlEngine>>,
    Path(id): Path<String>,
    Json(payload): Json<ProviderPayload>,
) -> Response {
    match engine.store().get_provider(&id).await {
        Ok(None) => return error_response(AicrawlError::NotFound(format!("provider {id}"))),
        Ok(Some(_)) => {}
        Err(err) => return error_response(err.into()),
    }
    let kind = payload.kind.trim().to_ascii_lowercase();
    if !PROVIDER_KINDS.contains(&kind.as_str()) {
        return error_response(AicrawlError::InvalidInput(format!(
            "kind must be one of {PROVIDER_KINDS:?}"
        )));
    }
    let encrypted = match payload.api_key.as_deref().map(str::trim) {
        Some(key) if !key.is_empty() => match engine.secrets().encrypt(key) {
            Ok(value) => Some(value),
            Err(err) => return error_response(err),
        },
        _ => None,
    };
    match engine
        .store()
        .upsert_provider(NewProvider {
            id: Some(id.clone()),
            name: payload.name.trim().to_string(),
            kind,
            base_url: payload.base_url.trim().to_string(),
            model: payload.model.trim().to_string(),
            api_key_enc: encrypted,
            is_default: payload.is_default.unwrap_or(false),
            max_tokens: payload.max_tokens.unwrap_or(1024).clamp(64, 8192) as i64,
            temperature: payload.temperature.unwrap_or(0.0).clamp(0.0, 2.0) as f64,
        })
        .await
    {
        Ok(provider) => ok(json!({"provider": provider.redacted()})).into_response(),
        Err(err) => error_response(err.into()),
    }
}

async fn delete_provider(
    State(engine): State<Arc<AicrawlEngine>>,
    Path(id): Path<String>,
) -> Response {
    match engine.store().delete_provider(&id).await {
        Ok(true) => ok(json!({"deleted": id})).into_response(),
        Ok(false) => error_response(AicrawlError::NotFound(format!("provider {id}"))),
        Err(err) => error_response(err.into()),
    }
}

async fn make_default_provider(
    State(engine): State<Arc<AicrawlEngine>>,
    Path(id): Path<String>,
) -> Response {
    match engine.store().set_default_provider(&id).await {
        Ok(true) => ok(json!({"default": id})).into_response(),
        Ok(false) => error_response(AicrawlError::NotFound(format!("provider {id}"))),
        Err(err) => error_response(err.into()),
    }
}

async fn test_provider(
    State(engine): State<Arc<AicrawlEngine>>,
    Path(id): Path<String>,
) -> Response {
    let client = match engine.ai().client_for_id(&id).await {
        Ok(client) => client,
        Err(err) => return error_response(err),
    };
    let started = std::time::Instant::now();
    match client
        .complete_json(
            "You are a connectivity test. Respond with JSON only.",
            "Return {\"ok\":true} and nothing else.",
        )
        .await
    {
        Ok(reply) => ok(json!({
            "provider": client.describe(),
            "ok": true,
            "reply": reply,
            "latency_ms": started.elapsed().as_millis() as u64
        }))
        .into_response(),
        Err(err) => error_response(err),
    }
}

// --------------------------------------------------------------- api keys

#[derive(Debug, Deserialize)]
struct CreateKeyPayload {
    #[serde(default = "default_key_name")]
    name: String,
    #[serde(default)]
    scopes: Option<String>,
}

fn default_key_name() -> String {
    "dashboard".into()
}

async fn list_api_keys(State(engine): State<Arc<AicrawlEngine>>) -> Response {
    match engine.store().list_api_keys().await {
        Ok(keys) => {
            let items: Vec<Value> = keys.iter().map(api_key_json).collect();
            ok(json!({"api_keys": items})).into_response()
        }
        Err(err) => error_response(err.into()),
    }
}

async fn create_api_key(
    State(engine): State<Arc<AicrawlEngine>>,
    Json(payload): Json<CreateKeyPayload>,
) -> Response {
    let name = payload.name.trim();
    if name.is_empty() || name.len() > 80 {
        return error_response(AicrawlError::InvalidInput(
            "name must be 1-80 characters".into(),
        ));
    }
    let key = match engine.secrets().generate_api_key() {
        Ok(key) => key,
        Err(err) => return error_response(err),
    };
    let scopes = payload.scopes.unwrap_or_else(|| "read,write".into());
    match engine
        .store()
        .create_api_key(name, &hash_api_key(&key), &key_prefix(&key), &scopes)
        .await
    {
        Ok(record) => (
            StatusCode::CREATED,
            ok(json!({
                "api_key": record.redacted(),
                "key": key,
                "note": "This is the only time the key is shown. Store it now."
            })),
        )
            .into_response(),
        Err(err) => error_response(err.into()),
    }
}

async fn revoke_api_key(
    State(engine): State<Arc<AicrawlEngine>>,
    Path(id): Path<String>,
) -> Response {
    match engine.store().revoke_api_key(&id).await {
        Ok(true) => ok(json!({"revoked": id})).into_response(),
        Ok(false) => error_response(AicrawlError::NotFound(format!("api key {id}"))),
        Err(err) => error_response(err.into()),
    }
}

async fn clear_cache(State(engine): State<Arc<AicrawlEngine>>) -> Response {
    match engine.store().clear_cache().await {
        Ok(()) => ok(json!({"cache": "cleared"})).into_response(),
        Err(err) => error_response(err.into()),
    }
}

// ------------------------------------------------------------------- mcp

async fn mcp_config(State(engine): State<Arc<AicrawlEngine>>) -> Json<Value> {
    let host = &engine.config().server.mcp_host;
    let port = engine.config().server.mcp_port;
    let endpoint = format!("http://{host}:{port}/mcp");
    let tools: Vec<ToolInfo> = tool_catalog();
    ok(json!({
        "http": {
            "endpoint": endpoint,
            "transport": "streamable-http",
            "client_config": {
                "mcpServers": {
                    "aicrawl": {"url": endpoint}
                }
            }
        },
        "stdio": {
            "command": "aicrawl",
            "args": ["mcp", "stdio"],
            "client_config": {
                "mcpServers": {
                    "aicrawl": {"command": "aicrawl", "args": ["mcp", "stdio"]}
                }
            }
        },
        "tools": tools,
        "notice": "Website content is treated as untrusted data, never as agent instructions."
    }))
    .into()
}

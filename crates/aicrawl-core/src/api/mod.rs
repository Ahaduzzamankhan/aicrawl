//! Versioned REST API. Responses use the same envelope as the CLI and MCP tools.
//!
//! All `/api/v1` routes accept JSON request objects (schema v1). The dashboard is
//! served from the same origin as a fallback, so `aicrawl api` gives a complete
//! local application.

use axum::extract::{DefaultBodyLimit, Path, Query, Request as AxumRequest, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response as AxumResponse};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::dashboard;
use crate::engine::AicrawlEngine;
use crate::error::AicrawlError;
use crate::output::Response;
use crate::request::{Action, Request, SCHEMA_VERSION};
use crate::security::{generate_api_key, hash_api_key, key_prefix};
use crate::storage::{ApiKeyRecord, ExtractedResult};

const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

/// Builds the router for the REST API and dashboard.
pub fn router(engine: AicrawlEngine) -> Router {
    let cors = cors_layer(&engine);
    let api = Router::new()
        .route("/extract", post(extract))
        .route("/crawl", post(crawl))
        .route("/batch", post(batch))
        .route("/map", post(map))
        .route("/validate", post(validate))
        .route("/jobs", get(list_jobs))
        .route("/jobs/{id}", get(get_job))
        .route("/jobs/{id}/results", get(job_results))
        .route("/jobs/{id}/cancel", post(cancel_job))
        .route("/results", get(list_results))
        .route("/results/{id}", get(get_result))
        .route("/mcp", get(mcp_discovery))
        .route("/export", post(export))
        .route("/stats", get(stats))
        .route("/settings", get(settings))
        .route("/api-keys", get(list_api_keys).post(create_api_key))
        .route("/api-keys/{id}", delete(revoke_api_key))
        .route("/health", get(health))
        .route("/schema", get(schema))
        .with_state(engine.clone())
        .route_layer(middleware::from_fn_with_state(
            engine.clone(),
            require_api_key,
        ));

    Router::new()
        .nest("/api/v1", api)
        .fallback(dashboard::handler)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(cors)
}

fn cors_layer(engine: &AicrawlEngine) -> CorsLayer {
    let layer = CorsLayer::new()
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);
    let origins = &engine.config().api.cors_origins;
    if origins.is_empty() {
        return layer.allow_origin(tower_http::cors::Any);
    }
    let values: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|origin| origin.parse::<HeaderValue>().ok())
        .collect();
    if values.is_empty() {
        return layer.allow_origin(tower_http::cors::Any);
    }
    layer.allow_origin(AllowOrigin::list(values))
}

/// Error body that mirrors the shared response envelope.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    payload: Value,
}

impl ApiError {
    /// Builds an envelope-shaped error.
    pub fn new(status: u16, code: &str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            payload: json!({
                "success": false,
                "data": Value::Null,
                "error": { "code": code, "message": message.into(), "retryable": false }
            }),
        }
    }

    fn from_error(err: &AicrawlError) -> Self {
        Self {
            status: StatusCode::from_u16(err.http_status())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            payload: json!({
                "success": false,
                "data": Value::Null,
                "error": {
                    "code": err.code(),
                    "message": err.to_string(),
                    "retryable": err.retryable(),
                }
            }),
        }
    }
}

impl From<AicrawlError> for ApiError {
    fn from(err: AicrawlError) -> Self {
        Self::from_error(&err)
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        Self::from_error(&AicrawlError::from(err))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> AxumResponse {
        (self.status, Json(self.payload)).into_response()
    }
}

fn envelope(response: Response) -> AxumResponse {
    let status = if response.success {
        StatusCode::OK
    } else {
        let code = response
            .error
            .as_ref()
            .map(|error| error.code.as_str())
            .unwrap_or("INTERNAL_ERROR");
        status_for(code)
    };
    (status, Json(response)).into_response()
}

fn status_for(code: &str) -> StatusCode {
    match code {
        "INVALID_REQUEST" | "JSON_ERROR" | "FIELD_REQUIRED" => StatusCode::BAD_REQUEST,
        "UNAUTHORIZED" => StatusCode::UNAUTHORIZED,
        "BLOCKED_BY_POLICY" | "ROBOTS_DISALLOWED" => StatusCode::FORBIDDEN,
        "NOT_FOUND" => StatusCode::NOT_FOUND,
        "BODY_TOO_LARGE" => StatusCode::PAYLOAD_TOO_LARGE,
        "SCHEMA_VALIDATION_FAILED" => StatusCode::UNPROCESSABLE_ENTITY,
        "RATE_LIMITED" => StatusCode::TOO_MANY_REQUESTS,
        "TIMEOUT" => StatusCode::GATEWAY_TIMEOUT,
        "RENDER_UNAVAILABLE" => StatusCode::SERVICE_UNAVAILABLE,
        "NETWORK_ERROR" | "HTTP_ERROR" | "UNSUPPORTED_CONTENT" => StatusCode::BAD_GATEWAY,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn require_api_key(
    State(engine): State<AicrawlEngine>,
    headers: HeaderMap,
    request: AxumRequest,
    next: Next,
) -> Result<AxumResponse, ApiError> {
    if !engine.config().api.require_api_key {
        return Ok(next.run(request).await);
    }
    let key = bearer(&headers).ok_or_else(|| {
        ApiError::new(
            401,
            "UNAUTHORIZED",
            "missing API key; send 'Authorization: Bearer <key>' or 'X-API-Key'",
        )
    })?;
    let hash = hash_api_key(&key);
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM api_keys WHERE key_hash = ? AND revoked_at IS NULL")
            .bind(&hash)
            .fetch_optional(engine.store().pool())
            .await
            .map_err(|_| ApiError::new(401, "UNAUTHORIZED", "API key could not be verified"))?;
    let Some((id,)) = row else {
        return Err(ApiError::new(
            401,
            "UNAUTHORIZED",
            "invalid or revoked API key",
        ));
    };
    let _ = sqlx::query("UPDATE api_keys SET last_used_at = ? WHERE id = ?")
        .bind(crate::util::now_rfc3339())
        .bind(&id)
        .execute(engine.store().pool())
        .await;
    Ok(next.run(request).await)
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = value
            .strip_prefix("Bearer ")
            .or_else(|| value.strip_prefix("bearer "))
        {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

// ------------------------------------------------------------------- actions

async fn extract(
    State(engine): State<AicrawlEngine>,
    body: Option<Json<Value>>,
) -> Result<AxumResponse, ApiError> {
    run_action(engine, body, Action::Extract).await
}

async fn crawl(
    State(engine): State<AicrawlEngine>,
    body: Option<Json<Value>>,
) -> Result<AxumResponse, ApiError> {
    run_action(engine, body, Action::Crawl).await
}

async fn batch(
    State(engine): State<AicrawlEngine>,
    body: Option<Json<Value>>,
) -> Result<AxumResponse, ApiError> {
    run_action(engine, body, Action::Batch).await
}

async fn map(
    State(engine): State<AicrawlEngine>,
    body: Option<Json<Value>>,
) -> Result<AxumResponse, ApiError> {
    run_action(engine, body, Action::Map).await
}

async fn run_action(
    engine: AicrawlEngine,
    body: Option<Json<Value>>,
    action: Action,
) -> Result<AxumResponse, ApiError> {
    let body = body
        .map(|Json(value)| value)
        .ok_or_else(|| ApiError::new(400, "INVALID_REQUEST", "request body is required"))?;
    let mut request = Request::from_json(&body.to_string())?;
    request.action = action;
    if action == Action::Batch && request.urls.is_empty() {
        if let Some(url) = request.url.clone() {
            request.urls = vec![url];
            request.url = None;
        }
    }
    Ok(envelope(engine.handle(request).await))
}

async fn validate(
    State(engine): State<AicrawlEngine>,
    body: Option<Json<Value>>,
) -> Result<AxumResponse, ApiError> {
    let body = body
        .map(|Json(value)| value)
        .ok_or_else(|| ApiError::new(400, "INVALID_REQUEST", "request body is required"))?;
    let request = Request::from_json(&body.to_string())?;
    match engine.validate(&request) {
        Ok(()) => Ok(envelope(Response::ok(
            json!({
                "valid": true,
                "version": SCHEMA_VERSION,
                "action": request.action.as_str(),
                "targets": request.targets(),
                "fields": request.selectors.keys().cloned().collect::<Vec<_>>(),
            }),
            crate::output::Meta::default(),
        ))),
        Err(err) => Ok(envelope(Response::failure(&err))),
    }
}

// --------------------------------------------------------------------- jobs

#[derive(Debug, Deserialize)]
struct JobsQuery {
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_jobs(
    State(engine): State<AicrawlEngine>,
    Query(query): Query<JobsQuery>,
) -> Result<AxumResponse, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let jobs = engine
        .store()
        .list_jobs(query.status.as_deref(), limit, offset)
        .await?;
    Ok(
        Json(
            json!({ "success": true, "data": { "jobs": jobs, "limit": limit, "offset": offset } }),
        )
        .into_response(),
    )
}

async fn get_job(
    State(engine): State<AicrawlEngine>,
    Path(id): Path<String>,
) -> Result<AxumResponse, ApiError> {
    let details = engine.crawl_details(&id).await?;
    let results: Vec<Value> = engine
        .store()
        .results_for_job(&id)
        .await?
        .iter()
        .map(result_json)
        .collect();
    Ok(Json(json!({
        "success": true,
        "data": { "job": details.job, "pages": details.pages, "results": results }
    }))
    .into_response())
}

fn result_json(result: &ExtractedResult) -> Value {
    json!({
        "id": result.id,
        "job_id": result.job_id,
        "url": result.url,
        "mode": result.mode,
        "format": result.format,
        "data": result.data(),
        "warnings": result.warnings(),
        "validated": result.is_validated(),
        "duration_ms": result.duration_ms,
        "created_at": result.created_at,
    })
}

#[derive(Debug, Deserialize)]
struct ResultsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_results(
    State(engine): State<AicrawlEngine>,
    Query(query): Query<ResultsQuery>,
) -> Result<AxumResponse, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let results = engine.store().list_results(limit, offset).await?;
    let data: Vec<Value> = results.iter().map(result_json).collect();
    Ok(Json(json!({ "success": true, "data": { "results": data } })).into_response())
}

async fn mcp_discovery(State(engine): State<AicrawlEngine>) -> AxumResponse {
    let config = &engine.config().server;
    let endpoint = format!("http://{}:{}/mcp", config.mcp_host, config.mcp_port);
    let command = std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "aicrawl".to_string());
    Json(json!({
        "success": true,
        "data": {
            "http": {
                "endpoint": endpoint,
                "transport": "streamable-http",
                "client_config": {
                    "mcpServers": { "aicrawl": { "type": "http", "url": endpoint } }
                }
            },
            "stdio": {
                "command": command,
                "args": ["mcp"],
                "client_config": {
                    "mcpServers": { "aicrawl": { "command": command, "args": ["mcp"] } }
                }
            },
            "tools": crate::mcp::tool_catalog(),
            "notice": "Website content is untrusted data. AICrawl flags prompt-injection patterns in extracted text and never follows page instructions."
        }
    }))
    .into_response()
}

async fn job_results(
    State(engine): State<AicrawlEngine>,
    Path(id): Path<String>,
) -> Result<AxumResponse, ApiError> {
    let results = engine.store().results_for_job(&id).await?;
    let data: Vec<Value> = results
        .iter()
        .map(|result| {
            json!({
                "id": result.id,
                "url": result.url,
                "validated": result.is_validated(),
                "duration_ms": result.duration_ms,
                "created_at": result.created_at,
                "warnings": result.warnings(),
                "data": result.data(),
            })
        })
        .collect();
    Ok(Json(json!({ "success": true, "data": { "job_id": id, "results": data } })).into_response())
}

async fn cancel_job(
    State(engine): State<AicrawlEngine>,
    Path(id): Path<String>,
) -> Result<AxumResponse, ApiError> {
    let cancelled = engine.cancel_job(&id).await?;
    Ok(
        Json(json!({ "success": true, "data": { "job_id": id, "cancelled": cancelled } }))
            .into_response(),
    )
}

async fn get_result(
    State(engine): State<AicrawlEngine>,
    Path(id): Path<String>,
) -> Result<AxumResponse, ApiError> {
    let result = engine
        .store()
        .get_result(&id)
        .await?
        .ok_or_else(|| AicrawlError::NotFound(format!("result '{id}'")))?;
    Ok(Json(json!({
        "success": true,
        "data": {
            "id": result.id,
            "job_id": result.job_id,
            "url": result.url,
            "format": result.format,
            "validated": result.is_validated(),
            "duration_ms": result.duration_ms,
            "created_at": result.created_at,
            "warnings": result.warnings(),
            "data": result.data(),
        }
    }))
    .into_response())
}

#[derive(Debug, Deserialize)]
struct ExportArgs {
    job_id: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "json".into()
}

async fn export(
    State(engine): State<AicrawlEngine>,
    Json(args): Json<ExportArgs>,
) -> Result<AxumResponse, ApiError> {
    let dataset = engine.export_dataset(&args.job_id, &args.format).await?;
    Ok(Json(json!({
        "success": true,
        "data": { "job_id": args.job_id, "format": args.format, "dataset": dataset }
    }))
    .into_response())
}

async fn stats(State(engine): State<AicrawlEngine>) -> Result<AxumResponse, ApiError> {
    let stats = engine.stats().await?;
    let recent_jobs = engine.store().recent_jobs(10).await.unwrap_or_default();
    let recent_results = engine.store().list_results(10, 0).await.unwrap_or_default();
    Ok(Json(json!({
        "success": true,
        "data": { "stats": stats, "recent_jobs": recent_jobs, "recent_results": recent_results }
    }))
    .into_response())
}

async fn settings(State(engine): State<AicrawlEngine>) -> Result<AxumResponse, ApiError> {
    let config = engine.config();
    Ok(Json(json!({
        "success": true,
        "data": {
            "version": crate::VERSION,
            "schema_version": SCHEMA_VERSION,
            "data_dir": engine.data_dir().display().to_string(),
            "database": engine.store().location(),
            "server": config.server,
            "crawler": config.crawler,
            "security": engine.security_summary(),
            "extract": config.extract,
            "cache": config.cache,
            "api": { "require_api_key": config.api.require_api_key, "cors_origins": config.api.cors_origins },
            "ai_required": false
        }
    }))
    .into_response())
}

async fn health(State(engine): State<AicrawlEngine>) -> AxumResponse {
    Json(json!({ "success": true, "data": engine.health().await })).into_response()
}

async fn schema(State(engine): State<AicrawlEngine>) -> AxumResponse {
    Json(json!({
        "success": true,
        "data": {
            "version": SCHEMA_VERSION,
            "actions": ["extract", "batch", "crawl", "map"],
            "endpoints": [
                "POST /api/v1/extract",
                "POST /api/v1/crawl",
                "POST /api/v1/batch",
                "POST /api/v1/map",
                "POST /api/v1/validate",
                "GET /api/v1/jobs",
                "GET /api/v1/jobs/{id}",
                "GET /api/v1/results/{id}",
                "POST /api/v1/export",
            ],
            "request_example": {
                "url": "https://example.com/products",
                "action": "extract",
                "selectors": {
                    "products": {
                        "type": "list",
                        "selector": ".product-card",
                        "fields": {
                            "name": { "selector": ".product-title", "type": "text" },
                            "price": { "selector": ".price", "type": "number" }
                        }
                    }
                }
            },
            "renderer": engine.fetcher().renderer().status().await.available,
        }
    }))
    .into_response()
}

// ----------------------------------------------------------------- API keys

async fn list_api_keys(State(engine): State<AicrawlEngine>) -> Result<AxumResponse, ApiError> {
    let keys: Vec<ApiKeyRecord> = sqlx::query_as(
        "SELECT id, name, key_hash, prefix, scopes, created_at, last_used_at, revoked_at FROM api_keys ORDER BY created_at DESC",
    )
    .fetch_all(engine.store().pool())
    .await?;
    let data: Vec<Value> = keys.iter().map(ApiKeyRecord::redacted).collect();
    Ok(Json(json!({ "success": true, "data": { "api_keys": data } })).into_response())
}

#[derive(Debug, Deserialize)]
struct NewKey {
    name: String,
    #[serde(default)]
    scopes: Option<String>,
}

async fn create_api_key(
    State(engine): State<AicrawlEngine>,
    Json(args): Json<NewKey>,
) -> Result<AxumResponse, ApiError> {
    let name = args.name.trim();
    if name.is_empty() {
        return Err(ApiError::new(
            400,
            "INVALID_REQUEST",
            "name must not be empty",
        ));
    }
    let key = generate_api_key()?;
    let id = crate::util::new_id("key");
    let hash = hash_api_key(&key);
    let prefix = key_prefix(&key);
    let scopes = args.scopes.unwrap_or_else(|| "read,write".into());
    sqlx::query(
        "INSERT INTO api_keys (id, name, key_hash, prefix, scopes, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(&hash)
    .bind(&prefix)
    .bind(&scopes)
    .bind(crate::util::now_rfc3339())
    .execute(engine.store().pool())
    .await?;
    Ok(Json(json!({
        "success": true,
        "data": {
            "api_key": {
                "id": id,
                "name": name,
                "prefix": prefix,
                "scopes": scopes,
                "created_at": crate::util::now_rfc3339(),
            },
            "key": key,
            "note": "Store this key now; AICrawl keeps only its hash and cannot show it again."
        }
    }))
    .into_response())
}

async fn revoke_api_key(
    State(engine): State<AicrawlEngine>,
    Path(id): Path<String>,
) -> Result<AxumResponse, ApiError> {
    let result =
        sqlx::query("UPDATE api_keys SET revoked_at = ? WHERE id = ? AND revoked_at IS NULL")
            .bind(crate::util::now_rfc3339())
            .bind(&id)
            .execute(engine.store().pool())
            .await?;
    if result.rows_affected() == 0 {
        return Err(AicrawlError::NotFound(format!("API key '{id}'")).into());
    }
    Ok(Json(json!({ "success": true, "data": { "revoked": id } })).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mapping_is_stable() {
        assert_eq!(status_for("INVALID_REQUEST"), StatusCode::BAD_REQUEST);
        assert_eq!(status_for("NOT_FOUND"), StatusCode::NOT_FOUND);
        assert_eq!(
            status_for("SCHEMA_VALIDATION_FAILED"),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(status_for("whatever"), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn bearer_header_parsing() {
        let mut headers = HeaderMap::new();
        assert!(bearer(&headers).is_none());
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer ac_live_x"),
        );
        assert_eq!(bearer(&headers).as_deref(), Some("ac_live_x"));
        headers.insert("x-api-key", HeaderValue::from_static(" ac_live_y "));
        assert_eq!(bearer(&headers).as_deref(), Some("ac_live_y"));
    }
}

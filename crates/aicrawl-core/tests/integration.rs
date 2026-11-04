//! End-to-end tests against a local HTTP server.
//!
//! These exercise the real fetch → parse → extract → store pipeline, the REST
//! router and MCP discovery. No external network access is used.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

use aicrawl_core::config::AppConfig;
use aicrawl_core::crawler::CrawlOptions;
use aicrawl_core::engine::AicrawlEngine;
use aicrawl_core::error::AicrawlError;
use aicrawl_core::extractor::ExtractRequest;
use aicrawl_core::storage::job_status;

const HOME: &str = r#"<!doctype html>
<html lang="en"><head>
<title>Test Shop</title>
<meta name="description" content="A store used by tests">
<script type="application/ld+json">
{"@context":"https://schema.org","@type":"Product","name":"Widget","offers":{"@type":"Offer","price":"19.99","priceCurrency":"USD"}}
</script>
</head><body>
<h1>Test Shop</h1>
<p>Widget costs $19.99 and ships worldwide.</p>
<p><a href="/second">Second page</a> · <a href="/private">Private</a></p>
</body></html>"#;

const SECOND: &str = r#"<html><head><title>Second</title></head>
<body><h1>Second page</h1><p>Gadget costs $29.00.</p></body></html>"#;

async fn spawn_site() -> SocketAddr {
    let app = Router::new()
        .route(
            "/",
            get(|| async { ([("content-type", "text/html")], HOME) }),
        )
        .route(
            "/second",
            get(|| async { ([("content-type", "text/html")], SECOND) }),
        )
        .route(
            "/private",
            get(|| async { ([("content-type", "text/html")], HOME) }),
        )
        .route(
            "/robots.txt",
            get(|| async {
                (
                    [("content-type", "text/plain")],
                    "User-agent: *\nDisallow: /private\n",
                )
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

async fn test_engine(dir: &tempfile::TempDir, allow_private: bool) -> AicrawlEngine {
    let mut config = AppConfig::default();
    config.storage.database_path = Some(dir.path().join("aicrawl-test.db"));
    config.security.allow_private_networks = allow_private;
    config.security.rate_limit_per_minute = 10_000;
    config.crawler.respect_robots_txt = true;
    config.crawler.max_crawl_secs = 30;
    config.cache.enabled = true;
    AicrawlEngine::open(config).await.unwrap()
}

#[tokio::test]
async fn extraction_pipeline_records_job_and_result() {
    let addr = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let request = ExtractRequest {
        url: format!("http://{addr}/"),
        query: "Extract product names and prices".into(),
        schema: None,
        output_format: "json".into(),
        use_ai: false,
        render: false,
        use_cache: false,
    };
    let run = engine.extract(request).await.unwrap();
    assert_eq!(run.job.status, job_status::SUCCEEDED);
    assert_eq!(run.outcome.mode, "deterministic");
    assert_eq!(run.outcome.page.status, 200);
    assert_eq!(run.outcome.provider, None);

    let data = &run.outcome.data;
    assert_eq!(data["title"], "Test Shop");
    assert!(data["content"]["text"]
        .as_str()
        .unwrap()
        .contains("Widget costs"));

    let stored = engine.store().get_result(&run.result.id).await.unwrap();
    assert!(stored.is_some());
    assert!(!run.result.is_validated());
}

#[tokio::test]
async fn robots_txt_is_enforced() {
    let addr = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let result = engine
        .inspect(&format!("http://{addr}/private"), false)
        .await;
    assert!(matches!(result, Err(AicrawlError::RobotsDisallowed(_))));
}

#[tokio::test]
async fn ssrf_policy_blocks_loopback_when_private_networks_disabled() {
    let addr = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, false).await;

    let result = engine.inspect(&format!("http://{addr}/"), false).await;
    assert!(matches!(result, Err(AicrawlError::Blocked(_))));
}

#[tokio::test]
async fn crawl_records_multiple_pages_and_respects_depth() {
    let addr = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let options = CrawlOptions {
        url: format!("http://{addr}/"),
        max_pages: 5,
        max_depth: 1,
        query: Some("Extract product prices".into()),
        render: false,
        same_domain: true,
    };
    let job = engine.create_crawl_job(&options).await.unwrap();
    let summary = engine.run_crawl_job(&job.id, options, None).await.unwrap();
    assert!(summary.pages_succeeded >= 2, "{summary:?}");
    assert!(summary.results_saved >= 2, "{summary:?}");

    let pages = engine.store().list_pages(&job.id).await.unwrap();
    assert!(pages.len() >= 2);
    let job = engine.store().get_job(&job.id).await.unwrap().unwrap();
    assert_eq!(job.status, job_status::SUCCEEDED);
}

#[tokio::test]
async fn api_health_and_extract_use_the_same_engine() {
    let addr = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = Arc::new(test_engine(&dir, true).await);
    let app = aicrawl_core::api::router(engine.clone());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["status"], "ok");

    let request_body = serde_json::json!({
        "url": format!("http://{addr}/"),
        "query": "Extract product names and prices",
        "use_ai": false,
        "use_cache": false
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/extract")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["outcome"]["mode"], "deterministic");
    assert!(json["data"]["result"]["id"].is_string());
}

#[tokio::test]
async fn embedded_dashboard_is_served_with_spa_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Arc::new(test_engine(&dir, false).await);
    let app = aicrawl_core::api::router(engine);

    let response = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(content_type.contains("text/html"), "{content_type}");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/jobs/some-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn mcp_discovery_lists_all_tools() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Arc::new(test_engine(&dir, false).await);
    let app = aicrawl_core::mcp::router(engine);

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["name"], "aicrawl");
    assert_eq!(json["tools"].as_array().unwrap().len(), 7);
    assert_eq!(json["transport"], "streamable-http");
}

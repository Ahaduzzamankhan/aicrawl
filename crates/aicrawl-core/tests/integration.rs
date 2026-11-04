//! End-to-end tests against a local fixture site.
//!
//! Every test uses 127.0.0.1 and a temporary SQLite database; no external
//! network access is required.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request as HttpRequest, StatusCode};
use axum::routing::get;
use axum::Router;
use serde_json::{json, Value};
use tower::ServiceExt;

use aicrawl_core::config::AppConfig;
use aicrawl_core::engine::AicrawlEngine;
use aicrawl_core::error::AicrawlError;
use aicrawl_core::request::{Request, SCHEMA_VERSION};
use aicrawl_core::storage::{job_status, SqliteStore};

const PAGE_ONE: &str = r#"<!doctype html>
<html lang="en"><head>
<title>Test Shop</title>
<meta name="description" content="A store used by tests">
<meta property="og:title" content="Test Shop OG">
<script type="application/ld+json">
{"@context":"https://schema.org","@type":"Product","name":"Alpha","offers":{"@type":"Offer","price":"19.99","priceCurrency":"USD"}}
</script>
</head><body>
<h1>Test Shop</h1>
<div class="product-card">
  <h2 class="product-title">Alpha</h2>
  <span class="price">$19.99</span>
  <a href="/p/alpha">Details</a>
</div>
<div class="product-card">
  <h2 class="product-title">Beta</h2>
  <span class="price">$29.00</span>
  <a href="/p/beta">Details</a>
</div>
<a class="next" href="/page2">Next</a>
</body></html>"#;

const PAGE_TWO: &str = r#"<html><head><title>Test Shop page 2</title></head><body>
<h1>Page two</h1>
<div class="product-card">
  <h2 class="product-title">Beta</h2>
  <span class="price">$29.00</span>
  <a href="/p/beta">Details</a>
</div>
<div class="product-card">
  <h2 class="product-title">Gamma</h2>
  <span class="price">$39.50</span>
  <a href="/p/gamma">Details</a>
</div>
<a href="/">Home</a>
</body></html>"#;

const SECRET: &str =
    r#"<html><head><title>Private</title></head><body><p>secret</p></body></html>"#;

async fn spawn_site() -> String {
    let app = Router::new()
        .route(
            "/",
            get(|| async { ([("content-type", "text/html")], PAGE_ONE) }),
        )
        .route(
            "/page2",
            get(|| async { ([("content-type", "text/html")], PAGE_TWO) }),
        )
        .route(
            "/private",
            get(|| async { ([("content-type", "text/html")], SECRET) }),
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
    format!("http://{addr}")
}

async fn test_engine(dir: &tempfile::TempDir, allow_private: bool) -> AicrawlEngine {
    let mut config = AppConfig::default();
    config.security.allow_private_networks = allow_private;
    config.security.rate_limit_per_minute = 10_000;
    config.crawler.respect_robots_txt = true;
    config.crawler.max_crawl_secs = 30;
    config.crawler.render_timeout_ms = 5_000;
    let store = SqliteStore::connect(&dir.path().join("aicrawl-test.db"), 1)
        .await
        .unwrap();
    AicrawlEngine::from_store(config, dir.path().to_path_buf(), store).unwrap()
}

fn products_request(url: &str) -> Request {
    Request::from_json(&format!(
        r#"{{
          "version": "1",
          "url": "{url}",
          "render": "off",
          "use_cache": false,
          "selectors": {{
            "products": {{
              "type": "list",
              "selector": ".product-card",
              "fields": {{
                "name": {{"selector": ".product-title"}},
                "price": {{"selector": ".price", "type": "number"}},
                "link": {{"selector": "a", "type": "url"}}
              }}
            }}
          }}
        }}"#
    ))
    .unwrap()
}

#[tokio::test]
async fn extract_projects_css_rules_into_typed_json() {
    let base = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let response = engine.handle(products_request(&format!("{base}/"))).await;
    assert!(response.success, "{:?}", response.error);
    let items = response.data["products"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["name"], "Alpha");
    assert_eq!(items[0]["price"], 19.99);
    assert_eq!(items[0]["link"], format!("{base}/p/alpha"));
    assert_eq!(response.meta.as_ref().unwrap().items_extracted, 2);
    assert_eq!(SCHEMA_VERSION, "1");
}

#[tokio::test]
async fn document_projection_returns_structured_summary() {
    let base = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let mut request = Request::new(format!("{base}/"));
    request.render = aicrawl_core::request::RenderMode::Off;
    request.use_cache = false;
    let response = engine.handle(request).await;
    assert!(response.success);
    assert_eq!(response.data["document"]["title"], "Test Shop");
    assert_eq!(
        response.data["document"]["open_graph"]["og:title"],
        "Test Shop OG"
    );
    assert!(
        response.data["document"]["json_ld"]
            .as_array()
            .unwrap()
            .len()
            >= 1
    );
    assert_eq!(response.data["document"]["headings"][0]["level"], 1);
}

#[tokio::test]
async fn required_field_misses_fail_with_field_required() {
    let base = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let request = Request::from_json(&format!(
        r#"{{"url":"{base}/","render":"off","selectors":{{"sku":{{"selector":".sku","required":true}}}}}}"#
    ))
    .unwrap();
    let response = engine.handle(request).await;
    assert!(!response.success);
    let error = response.error.as_ref().unwrap();
    assert_eq!(error.code, "FIELD_REQUIRED");
    assert!(!error.retryable);
    assert_eq!(response.errors[0].code, "SELECTOR_NOT_FOUND");
    assert!(response.errors[0].required);
}

#[tokio::test]
async fn optional_field_misses_are_null_and_non_fatal() {
    let base = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let request = Request::from_json(&format!(
        r#"{{"url":"{base}/","render":"off","selectors":{{"sku":{{"selector":".sku","default":"unknown"}}}}}}"#
    ))
    .unwrap();
    let response = engine.handle(request).await;
    assert!(response.success);
    assert_eq!(response.data["sku"], "unknown");
    assert_eq!(response.errors.len(), 1);
    assert!(!response.errors[0].required);
}

#[tokio::test]
async fn xpath_rules_select_the_same_dom() {
    let base = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let request = Request::from_json(&format!(
        r#"{{"url":"{base}/","render":"off","selectors":{{"first":{{"xpath":"//div[@class='product-card'][1]//h2/text()"}}}}}}"#
    ))
    .unwrap();
    let response = engine.handle(request).await;
    assert!(response.success, "{:?}", response.error);
    assert_eq!(response.data["first"], "Alpha");
}

#[tokio::test]
async fn json_schema_violations_are_reported() {
    let base = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let request = Request::from_json(&format!(
        r#"{{"url":"{base}/","render":"off",
            "selectors":{{"products":{{"type":"list","selector":".product-card"}}}},
            "schema":{{"type":"object","properties":{{"products":{{"type":"array","items":{{"type":"object","required":["sku"]}}}}}},"required":["products"]}}}}"#
    ))
    .unwrap();
    let response = engine.handle(request).await;
    assert!(!response.success);
    assert_eq!(
        response.error.as_ref().unwrap().code,
        "SCHEMA_VALIDATION_FAILED"
    );
}

#[tokio::test]
async fn pagination_and_deduplication_merge_pages() {
    let base = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let request = Request::from_json(&format!(
        r#"{{"url":"{base}/","render":"off",
            "selectors":{{"products":{{"type":"list","selector":".product-card","fields":{{"name":{{"selector":".product-title"}}}}}}}},
            "pagination":{{"enabled":true,"next_selector":"a.next","max_pages":3}},
            "deduplicate":{{"enabled":true,"keys":["name"]}}}}"#
    ))
    .unwrap();
    let response = engine.handle(request).await;
    assert!(response.success, "{:?}", response.error);
    let items = response.data["products"].as_array().unwrap();
    assert_eq!(items.len(), 3, "{items:?}");
    assert_eq!(response.meta.as_ref().unwrap().pages_processed, 2);
}

#[tokio::test]
async fn robots_txt_is_enforced() {
    let base = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let result = engine.inspect(&format!("{base}/private"), false).await;
    assert!(matches!(result, Err(AicrawlError::RobotsDisallowed(_))));
}

#[tokio::test]
async fn ssrf_policy_blocks_loopback_by_default() {
    let base = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, false).await;

    let result = engine.inspect(&format!("{base}/"), false).await;
    assert!(matches!(result, Err(AicrawlError::Blocked(_))));
}

#[tokio::test]
async fn crawl_stores_pages_and_results() {
    let base = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let request = Request::from_json(&format!(
        r#"{{"url":"{base}/","action":"crawl","render":"off",
            "crawl":{{"max_pages":5,"max_depth":1,"sitemap":false}},
            "selectors":{{"title":{{"selector":"h1"}}}}}}"#
    ))
    .unwrap();
    let response = engine.handle(request).await;
    assert!(response.success, "{:?}", response.error);
    assert!(response.data["pages"].as_array().unwrap().len() >= 2);
    let job_id = response.meta.as_ref().unwrap().job_id.clone().unwrap();
    let job = engine.store().get_job(&job_id).await.unwrap().unwrap();
    assert_eq!(job.status, job_status::SUCCEEDED);
    let pages = engine.store().list_pages(&job_id).await.unwrap();
    assert!(pages.len() >= 2);
    let dataset = engine.export_dataset(&job_id, "jsonl").await.unwrap();
    assert!(dataset.lines().count() >= 2);
}

#[tokio::test]
async fn api_uses_the_shared_envelope() {
    let base = spawn_site().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;
    let app = aicrawl_core::api::router(engine);

    let health = app
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    let body = axum::body::to_bytes(health.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["schema_version"], "1");
    assert_eq!(json["data"]["capabilities"]["ai_required"], false);

    let payload = json!({
        "url": format!("{base}/"),
        "render": "off",
        "selectors": {"name": {"selector": ".product-title"}}
    });
    let response = app
        .oneshot(
            HttpRequest::builder()
                .method("POST")
                .uri("/api/v1/extract")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["name"], "Alpha");
    assert_eq!(json["meta"]["projection"], "rules");
}

#[tokio::test]
async fn invalid_requests_fail_before_any_fetch() {
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;

    let request = Request::from_json(
        r#"{"url":"https://example.com","selectors":{"bad":{"selector":"div["}}}"#,
    )
    .unwrap();
    let response = engine.handle(request).await;
    assert!(!response.success);
    assert_eq!(response.error.as_ref().unwrap().code, "INVALID_REQUEST");
    assert!(response.data.is_null());
}

#[tokio::test]
async fn api_rejects_invalid_requests_with_400() {
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, true).await;
    let app = aicrawl_core::api::router(engine);

    let response = app
        .oneshot(
            HttpRequest::builder()
                .method("POST")
                .uri("/api/v1/extract")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"selectors":{}}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn mcp_discovery_lists_ten_tools() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Arc::new(test_engine(&dir, false).await);
    let app = aicrawl_core::mcp::router(engine);

    let response = app
        .oneshot(HttpRequest::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["name"], "aicrawl");
    assert_eq!(json["schema_version"], "1");
    assert_eq!(json["tools"].as_array().unwrap().len(), 10);
}

#[tokio::test]
async fn embedded_dashboard_is_served_with_spa_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let engine = test_engine(&dir, false).await;
    let app = aicrawl_core::api::router(engine);

    let response = app
        .clone()
        .oneshot(HttpRequest::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(content_type.contains("text/html"), "{content_type}");

    let response = app
        .oneshot(
            HttpRequest::builder()
                .uri("/jobs/some-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

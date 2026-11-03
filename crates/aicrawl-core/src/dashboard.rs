//! Embedded dashboard assets.
//!
//! The production dashboard is compiled into the `aicrawl` executable, so a
//! single binary serves the CLI, REST API, MCP endpoint and React dashboard.
//! Set `AICRAWL_DASHBOARD_DIR` (or `[api].dashboard_dist`) to serve an
//! un-embedded build from disk during dashboard development.

use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

/// Files produced by `npm run build` in `frontend/`.
#[derive(RustEmbed)]
#[folder = "../../frontend/dist/"]
struct Assets;

/// Whether the dashboard was embedded at compile time.
pub fn available() -> bool {
    Assets::get("index.html").is_some()
}

/// Serves an embedded asset, falling back to `index.html` for client routes.
pub async fn handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let candidate = if path.is_empty() { "index.html" } else { path };
    if let Some(response) = asset(candidate) {
        return response;
    }
    // Client-side routes such as /jobs/<id> have no extension: hand them to the
    // SPA. Missing files with an extension stay a 404.
    let last_segment = candidate.rsplit('/').next().unwrap_or_default();
    if !last_segment.contains('.') {
        if let Some(response) = asset("index.html") {
            return response;
        }
    }
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

fn asset(path: &str) -> Option<Response> {
    let file = Assets::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache = if path.starts_with("assets/") {
        // Vite emits content-hashed asset names.
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    Some(
        (
            [
                (header::CONTENT_TYPE, mime.as_ref()),
                (header::CACHE_CONTROL, cache),
            ],
            Body::from(file.data.into_owned()),
        )
            .into_response(),
    )
}

/// Number of embedded files (used by `aicrawl doctor`/status output).
pub fn file_count() -> usize {
    Assets::iter().count()
}

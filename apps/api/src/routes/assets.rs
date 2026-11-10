//! The built web client and the not-found answer (SE-5): hashed assets are immutable, the entry
//! page is never cached, directories are never listed, and unknown `/api` paths are JSON 404s
//! rather than the client page.

use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

const IMMUTABLE: &str = "public, max-age=31536000, immutable";
const NO_CACHE: &str = "no-cache";

/// Serve a client file, the client entry page for client routes, or a 404.
pub async fn fallback(State(state): State<AppState>, request: Request) -> Response {
    let path = request.uri().path().to_owned();
    if path == "/api" || path.starts_with("/api/") {
        return ApiError::not_found().into_response();
    }
    let Some(directory) = state.web_dir.as_ref() else {
        return ApiError::not_found().into_response();
    };
    let service = ServeDir::new(directory)
        .append_index_html_on_directories(false)
        .fallback(ServeFile::new(directory.join("index.html")));
    let Ok(response) = service.oneshot(request).await;
    let mut response = response.into_response();
    let cache = if path.starts_with("/assets/") {
        IMMUTABLE
    } else {
        NO_CACHE
    };
    if response.status().is_success() {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    }
    response
}

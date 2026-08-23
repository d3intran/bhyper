use axum::{
    http::header,
    response::{Html, IntoResponse, Response},
};

pub const INDEX_HTML: &str = include_str!("../../static/index.html");

/// Serves the embedded single-file high-performance Web / TG Mini App UI
pub async fn serve_index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

/// Serves favicon
pub async fn serve_favicon() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "image/x-icon")
        .body(axum::body::Body::empty())
        .unwrap()
}

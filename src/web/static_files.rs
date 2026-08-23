use axum::{
    http::header,
    response::{Html, IntoResponse, Response},
};

pub const INDEX_HTML: &str = include_str!("../../static/index.html");
pub const THEME_CSS: &str = include_str!("../../static/css/theme.css");
pub const APP_JS: &str = include_str!("../../static/js/app.js");
pub const API_JS: &str = include_str!("../../static/js/api.js");
pub const FORMAT_JS: &str = include_str!("../../static/js/utils/format.js");
pub const OVERVIEW_JS: &str = include_str!("../../static/js/components/overview.js");
pub const RADAR_JS: &str = include_str!("../../static/js/components/radar.js");
pub const POSITIONS_JS: &str = include_str!("../../static/js/components/positions.js");
pub const CONFIG_JS: &str = include_str!("../../static/js/components/config.js");
pub const JOURNAL_JS: &str = include_str!("../../static/js/components/journal.js");

/// Serves the modular index.html
pub async fn serve_index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

/// Serves theme.css
pub async fn serve_theme_css() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/css; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(THEME_CSS.to_string())
        .unwrap()
}

/// Serves app.js
pub async fn serve_app_js() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(APP_JS.to_string())
        .unwrap()
}

/// Serves api.js
pub async fn serve_api_js() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(API_JS.to_string())
        .unwrap()
}

/// Serves format.js
pub async fn serve_format_js() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(FORMAT_JS.to_string())
        .unwrap()
}

/// Serves overview.js
pub async fn serve_overview_js() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(OVERVIEW_JS.to_string())
        .unwrap()
}

/// Serves radar.js
pub async fn serve_radar_js() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(RADAR_JS.to_string())
        .unwrap()
}

/// Serves positions.js
pub async fn serve_positions_js() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(POSITIONS_JS.to_string())
        .unwrap()
}

/// Serves config.js
pub async fn serve_config_js() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(CONFIG_JS.to_string())
        .unwrap()
}

/// Serves journal.js
pub async fn serve_journal_js() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(JOURNAL_JS.to_string())
        .unwrap()
}

/// Serves favicon
pub async fn serve_favicon() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "image/x-icon")
        .body(axum::body::Body::empty())
        .unwrap()
}

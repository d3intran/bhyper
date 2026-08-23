use axum::{
    body::Body,
    http::header,
    response::{IntoResponse, Response},
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

#[inline]
fn static_response(content: &'static str, content_type: &'static str) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::PRAGMA, "no-cache")
        .header(header::EXPIRES, "0")
        .body(Body::from(content))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

/// Serves the modular index.html with no-cache headers
pub async fn serve_index() -> impl IntoResponse {
    static_response(INDEX_HTML, "text/html; charset=utf-8")
}

/// Serves theme.css
pub async fn serve_theme_css() -> impl IntoResponse {
    static_response(THEME_CSS, "text/css; charset=utf-8")
}

/// Serves app.js
pub async fn serve_app_js() -> impl IntoResponse {
    static_response(APP_JS, "application/javascript; charset=utf-8")
}

/// Serves api.js
pub async fn serve_api_js() -> impl IntoResponse {
    static_response(API_JS, "application/javascript; charset=utf-8")
}

/// Serves format.js
pub async fn serve_format_js() -> impl IntoResponse {
    static_response(FORMAT_JS, "application/javascript; charset=utf-8")
}

/// Serves overview.js
pub async fn serve_overview_js() -> impl IntoResponse {
    static_response(OVERVIEW_JS, "application/javascript; charset=utf-8")
}

/// Serves radar.js
pub async fn serve_radar_js() -> impl IntoResponse {
    static_response(RADAR_JS, "application/javascript; charset=utf-8")
}

/// Serves positions.js
pub async fn serve_positions_js() -> impl IntoResponse {
    static_response(POSITIONS_JS, "application/javascript; charset=utf-8")
}

/// Serves config.js
pub async fn serve_config_js() -> impl IntoResponse {
    static_response(CONFIG_JS, "application/javascript; charset=utf-8")
}

/// Serves journal.js
pub async fn serve_journal_js() -> impl IntoResponse {
    static_response(JOURNAL_JS, "application/javascript; charset=utf-8")
}

/// Serves favicon
pub async fn serve_favicon() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "image/x-icon")
        .body(Body::empty())
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

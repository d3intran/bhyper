pub mod auth;
pub mod handlers;
pub mod state;
pub mod static_files;
pub mod ws;

pub use auth::AuthenticatedIdentity;
pub use state::AppState;

use crate::config::Config;
use crate::paper::PaperTradingStore;
use crate::state::StateStore;
use crate::ws::MarketDataCache;
use anyhow::{Context, Result};
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use parking_lot::Mutex;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

/// Constructs the complete Axum Router with all endpoints, middlewares, and embedded static UI
pub fn build_router(state: Arc<AppState>) -> Router {
    let api_routes = Router::new()
        .route("/status", get(handlers::get_status))
        .route("/health", get(handlers::get_health))
        .route("/positions", get(handlers::get_positions))
        .route("/scan", get(handlers::get_scan))
        .route(
            "/config",
            get(handlers::get_config).post(handlers::update_config),
        )
        .route("/action/unwind", post(handlers::action_unwind))
        .route("/action/paper_trade", post(handlers::action_paper_trade))
        .route("/journal", get(handlers::get_journal))
        .route("/report", get(handlers::get_report))
        .route("/ws", get(ws::ws_handler));

    Router::new()
        .route("/", get(static_files::serve_index))
        .route("/index.html", get(static_files::serve_index))
        .route("/favicon.ico", get(static_files::serve_favicon))
        .route("/css/theme.css", get(static_files::serve_theme_css))
        .route("/js/app.js", get(static_files::serve_app_js))
        .route("/js/api.js", get(static_files::serve_api_js))
        .route("/js/utils/format.js", get(static_files::serve_format_js))
        .route(
            "/js/components/overview.js",
            get(static_files::serve_overview_js),
        )
        .route("/js/components/radar.js", get(static_files::serve_radar_js))
        .route(
            "/js/components/positions.js",
            get(static_files::serve_positions_js),
        )
        .route(
            "/js/components/config.js",
            get(static_files::serve_config_js),
        )
        .route(
            "/js/components/journal.js",
            get(static_files::serve_journal_js),
        )
        .route("/js/components/about.js", get(static_files::serve_about_js))
        .nest("/api", api_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Spawns and runs the Axum Web & WebSocket API server
pub async fn start_web_server(
    config: Config,
    config_path: PathBuf,
    state_store: Arc<Mutex<StateStore>>,
    paper_store_opt: Option<PaperTradingStore>,
    market_cache: MarketDataCache,
    shutdown_rx: Option<tokio::sync::oneshot::Receiver<()>>,
) -> Result<()> {
    let host = config.web.host.clone();
    let port = config.web.port;
    let addr_str = format!("{}:{}", host, port);
    let addr: SocketAddr = addr_str
        .parse()
        .with_context(|| format!("Invalid host:port binding: {}", addr_str))?;

    let state = Arc::new(AppState::new(
        config,
        config_path,
        state_store,
        paper_store_opt,
        market_cache,
    ));

    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Failed to bind Web Server to {}", addr))?;

    info!("🌐 ========================================================");
    info!("🚀 BHyper Web Control & Mini App Server Running!");
    info!("🔗 Local Access:   http://{}", addr);
    info!("🚇 Cloudflare:     Ready for cloudflared tunnel forwarding");
    info!("📱 TG Mini App:    Ready for Telegram WebApp HMAC verification");
    info!("🌐 ========================================================");

    if let Some(rx) = shutdown_rx {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
                info!("🌐 Web server shutting down gracefully...");
            })
            .await
            .context("Web server encountered runtime error")?;
    } else {
        axum::serve(listener, app)
            .await
            .context("Web server encountered runtime error")?;
    }

    Ok(())
}

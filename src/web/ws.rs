use crate::web::auth::validate_auth;
use crate::web::state::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

#[derive(Debug, Deserialize)]
pub struct WsParams {
    pub token: Option<String>,
}

/// GET /api/ws - Real-time WebSocket connection for live telemetry & opportunity matrix
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    Query(params): Query<WsParams>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    let cfg = state.config.load();
    let query_token = params.token.as_ref().map(|t| format!("token={}", t));
    validate_auth(&headers, query_token.as_deref(), &cfg)?;

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state)))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx_broadcast = state.ws_broadcast.subscribe();

    info!("🔌 [WS] New Web client connected to real-time feed");

    // 1. Send initial handshake packet
    let cfg = state.config.load();
    let cost_bps = if cfg.strategy.maker_taker_mode { 7.0 } else { 14.0 };
    let opps = state
        .market_cache
        .compute_opportunities(cost_bps);

    let initial_packet = json!({
        "type": "INIT",
        "server_time": Utc::now(),
        "version": env!("CARGO_PKG_VERSION"),
        "opportunities": opps,
    });

    if let Ok(msg_str) = serde_json::to_string(&initial_packet) {
        if sender.send(Message::Text(msg_str)).await.is_err() {
            return;
        }
    }

    // 2. Spawn a periodic ticker (every 1s) to push live status, opportunities & active positions
    let state_clone = state.clone();
    let mut ticker = tokio::time::interval(Duration::from_millis(1000));

    let mut send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let current_cfg = state_clone.config.load();
                    let cost_bps = if current_cfg.strategy.maker_taker_mode { 7.0 } else { 14.0 };
                    let current_opps = state_clone.market_cache.compute_opportunities(cost_bps);

                    // Periodically reload stores to ensure 100% real-time synchronization with background paper/live daemon
                    if let Ok(p_store) = crate::paper::PaperTradingStore::load_or_create(None, 500.0) {
                        let mut p_lock = state_clone.paper_store.lock();
                        *p_lock = Some(p_store);
                    }
                    if let Ok(s_store) = crate::state::StateStore::load_or_create(None) {
                        let mut s_lock = state_clone.state_store.lock();
                        *s_lock = s_store;
                    }

                    let mut live_pos = {
                        let store = state_clone.state_store.lock();
                        store.get_active_positions()
                    };

                    for pos in &mut live_pos {
                        if let Some((bn_p, hl_p)) = state_clone.market_cache.get_latest_prices(&pos.symbol) {
                            let bn_rate = state_clone.market_cache.get_binance_rate(&pos.symbol).unwrap_or(0.0);
                            let hl_rate = state_clone.market_cache.get_hyperliquid_rate(&pos.symbol).unwrap_or(0.0);
                            let bn_apr = bn_rate * 1095.0 * 100.0;
                            let hl_apr = hl_rate * 8760.0 * 100.0;
                            pos.current_spread_apr = (hl_apr - bn_apr).abs();

                            let bn_pnl = match pos.binance_side {
                                crate::types::PositionSide::Long => (bn_p - pos.binance_entry_price) * pos.binance_qty,
                                crate::types::PositionSide::Short => (pos.binance_entry_price - bn_p) * pos.binance_qty,
                            };
                            let hl_pnl = match pos.hyperliquid_side {
                                crate::types::PositionSide::Long => (hl_p - pos.hyperliquid_entry_price) * pos.hyperliquid_qty,
                                crate::types::PositionSide::Short => (pos.hyperliquid_entry_price - hl_p) * pos.hyperliquid_qty,
                            };
                            pos.realized_pnl_usd = Some(bn_pnl + hl_pnl);
                        }
                    }

                    let mut paper_pos = {
                        let p_lock = state_clone.paper_store.lock();
                        p_lock.as_ref().map(|s| s.state.active_positions.values().cloned().collect::<Vec<_>>()).unwrap_or_default()
                    };

                    let mut total_bn_unrealized = 0.0;
                    let mut total_hl_unrealized = 0.0;

                    for pos in &mut paper_pos {
                        if let Some((bn_p, hl_p)) = state_clone.market_cache.get_latest_prices(&pos.symbol) {
                            let bn_rate = state_clone.market_cache.get_binance_rate(&pos.symbol).unwrap_or(0.0);
                            let hl_rate = state_clone.market_cache.get_hyperliquid_rate(&pos.symbol).unwrap_or(0.0);
                            let bn_apr = bn_rate * 1095.0 * 100.0;
                            let hl_apr = hl_rate * 8760.0 * 100.0;
                            pos.current_spread_apr = (hl_apr - bn_apr).abs();

                            let bn_pnl = match pos.binance_side {
                                crate::types::PositionSide::Long => (bn_p - pos.binance_entry_price) * pos.binance_qty,
                                crate::types::PositionSide::Short => (pos.binance_entry_price - bn_p) * pos.binance_qty,
                            };
                            let hl_pnl = match pos.hyperliquid_side {
                                crate::types::PositionSide::Long => (hl_p - pos.hyperliquid_entry_price) * pos.hyperliquid_qty,
                                crate::types::PositionSide::Short => (pos.hyperliquid_entry_price - hl_p) * pos.hyperliquid_qty,
                            };
                            pos.realized_pnl_usd = Some(bn_pnl + hl_pnl);
                            total_bn_unrealized += bn_pnl;
                            total_hl_unrealized += hl_pnl;
                        }
                    }

                    let mut paper_wallet = {
                        let p_lock = state_clone.paper_store.lock();
                        p_lock.as_ref().map(|s| s.state.wallet.clone())
                    };

                    if let Some(ref mut wallet) = paper_wallet {
                        wallet.binance.unrealized_pnl_usd = total_bn_unrealized;
                        wallet.hyperliquid.unrealized_pnl_usd = total_hl_unrealized;
                    }

                    // Deterministically sort positions to prevent UI bouncing
                    live_pos.sort_by(|a, b| a.symbol.cmp(&b.symbol));
                    paper_pos.sort_by(|a, b| a.symbol.cmp(&b.symbol));

                    let packet = json!({
                        "type": "TICK",
                        "timestamp": Utc::now(),
                        "opportunities": current_opps,
                        "live_positions": live_pos,
                        "paper_positions": paper_pos,
                        "paper_wallet": paper_wallet,
                        "cache_size": state_clone.market_cache.len(),
                    });

                    if let Ok(str_val) = serde_json::to_string(&packet) {
                        if sender.send(Message::Text(str_val)).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(broadcast_msg) = rx_broadcast.recv() => {
                    if sender.send(Message::Text(broadcast_msg)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // 3. Receive client control messages (e.g. ping/pong)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(t) => {
                    debug!("Received WS message from client: {}", t);
                }
                Message::Ping(_) => {
                    // Handled automatically by axum
                }
                Message::Close(_) => {
                    break;
                }
                _ => {}
            }
        }
    });

    // If either task finishes, abort the other
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    info!("🔌 [WS] Client disconnected from real-time feed");
}

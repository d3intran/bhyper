use crate::binance::BinanceFuturesClient;
use crate::config::Config;
use crate::hyperliquid::HyperliquidClient;
use crate::journal::{JournalFilter, PerformanceAnalytics, TradeJournal};
use crate::paper::engine::PaperPosition;
use crate::paper::wallet::PaperDualWallet;
use crate::paper::{PaperExecutionEngine, PaperTradingStore};
use crate::strategy::precision::LotPrecisionMatcher;
use crate::strategy::trigger::ProfitTriggerEngine;
use crate::types::{
    ActiveArbitragePosition, ArbitrageOpportunity, ExecutionMode,
};
use crate::web::state::AppState;
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug, Serialize)]
pub struct SystemStatusResponse {
    pub version: &'static str,
    pub uptime_secs: i64,
    pub server_time: chrono::DateTime<Utc>,
    pub live_positions_count: usize,
    pub paper_positions_count: usize,
    pub market_cache_symbols_count: usize,
    pub total_realized_pnl_usd: f64,
    pub total_accumulated_funding_usd: f64,
    pub total_closed_trades: usize,
    pub win_rate_pct: f64,
}

#[derive(Debug, Serialize)]
pub struct PositionsResponse {
    pub live_positions: Vec<ActiveArbitragePosition>,
    pub paper_positions: Vec<PaperPosition>,
    pub paper_wallet: Option<PaperDualWallet>,
}

#[derive(Debug, Deserialize)]
pub struct JournalQuery {
    pub symbol: Option<String>,
    pub event_type: Option<String>,
    pub limit: Option<usize>,
    pub paper_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UnwindRequest {
    pub symbol: String, // "BTC" or "ALL"
}

#[derive(Debug, Deserialize)]
pub struct PaperTradeRequest {
    pub symbol: String,
    pub margin_usd: Option<f64>,
    pub action: String, // "open" or "close"
}

#[derive(Debug, Deserialize)]
pub struct ReportQuery {
    pub initial_capital: Option<f64>,
}

/// Helper function to infer adaptive precision for any token price tier
fn infer_precision_info(symbol: &str, mark_price: f64) -> crate::types::SymbolPrecisionInfo {
    let (sz_decimals, step_size, min_qty) = if mark_price >= 1000.0 {
        (4, 0.0001, 0.0001)
    } else if mark_price >= 100.0 {
        (3, 0.001, 0.001)
    } else if mark_price >= 1.0 {
        (2, 0.01, 0.01)
    } else if mark_price >= 0.01 {
        (1, 0.1, 0.1)
    } else {
        (0, 1.0, 1.0)
    };

    crate::types::SymbolPrecisionInfo {
        symbol: symbol.to_string(),
        binance_step_size: step_size,
        binance_tick_size: 0.0001,
        binance_min_qty: min_qty,
        binance_min_notional: 5.0,
        hyperliquid_sz_decimals: sz_decimals,
        hyperliquid_asset_index: 0,
        hyperliquid_min_notional: 10.0,
    }
}

/// GET /api/status - Basic health & runtime telemetry
pub async fn get_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let now = Utc::now();
    let uptime = (now - state.start_time).num_seconds();

    // Reload paper store and state store from disk to guarantee 100% sync with active daemons
    if let Ok(p_store) = PaperTradingStore::load_or_create(None, 500.0) {
        let mut p_lock = state.paper_store.lock();
        *p_lock = Some(p_store);
    }
    if let Ok(s_store) = crate::state::StateStore::load_or_create(None) {
        let mut s_lock = state.state_store.lock();
        *s_lock = s_store;
    }

    let (live_count, total_realized_pnl, total_funding) = {
        let store = state.state_store.lock();
        let p_lock = state.paper_store.lock();
        
        let live_pnl = store.data.total_realized_pnl_usd;
        let live_fund = store.data.total_accumulated_funding_usd;
        
        if let Some(paper) = p_lock.as_ref() {
            let paper_fund = paper.state.wallet.binance.total_funding_usd + paper.state.wallet.hyperliquid.total_funding_usd;
            let paper_pnl = paper.state.wallet.binance.realized_pnl_usd + paper.state.wallet.hyperliquid.realized_pnl_usd;
            (
                store.get_active_positions().len(),
                if live_pnl != 0.0 { live_pnl } else { paper_pnl },
                if live_fund != 0.0 { live_fund } else { paper_fund },
            )
        } else {
            (
                store.get_active_positions().len(),
                live_pnl,
                live_fund,
            )
        }
    };

    let paper_count = {
        let p_lock = state.paper_store.lock();
        p_lock
            .as_ref()
            .map(|s| s.state.active_positions.len())
            .unwrap_or(0)
    };

    let market_symbols = state.market_cache.len();

    // Compute win rate & closed trades count from trade journal
    let journal = TradeJournal::open_default();
    let (total_closed, win_rate) = match journal.read_all() {
        Ok(entries) => {
            let analytics = PerformanceAnalytics::compute_from_entries(&entries, 500.0);
            (analytics.total_trades, analytics.win_rate_pct)
        }
        Err(_) => (0, 100.0),
    };

    Json(SystemStatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        uptime_secs: uptime,
        server_time: now,
        live_positions_count: live_count,
        paper_positions_count: paper_count,
        market_cache_symbols_count: market_symbols,
        total_realized_pnl_usd: total_realized_pnl,
        total_accumulated_funding_usd: total_funding,
        total_closed_trades: total_closed,
        win_rate_pct: win_rate,
    })
}

/// GET /api/health - Cross-exchange margin health and rebalancing assessment
pub async fn get_health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cfg = state.config.load();

    // Reload paper store from disk
    if let Ok(p_store) = PaperTradingStore::load_or_create(None, 500.0) {
        let mut p_lock = state.paper_store.lock();
        *p_lock = Some(p_store);
    }

    // If exchange API credentials are configured, query live exchange health
    if !cfg.binance.api_key.is_empty() && !cfg.hyperliquid.wallet_address.is_empty() {
        let bn_client = BinanceFuturesClient::new(
            cfg.binance.api_key.clone(),
            cfg.binance.api_secret.clone(),
            cfg.binance.base_url.clone(),
        );
        let hl_client = HyperliquidClient::new(
            cfg.hyperliquid.private_key.clone(),
            cfg.hyperliquid.wallet_address.clone(),
            cfg.hyperliquid.base_url.clone(),
        );

        let (bn_health_res, hl_health_res) =
            tokio::join!(bn_client.fetch_margin_health(), hl_client.fetch_margin_health());

        if let (Ok(bn_health), Ok(hl_health)) = (bn_health_res, hl_health_res) {
            let assessment = crate::state::StateStore::compute_rebalance_advisory(
                &bn_health,
                &hl_health,
                cfg.risk.rebalance_threshold_imbalance_pct,
            );
            return Json(json!({ "status": "ok", "assessment": assessment }));
        }
    }

    // Fallback: Use Paper Trading wallet simulation health if available
    let p_lock = state.paper_store.lock();
    if let Some(paper) = p_lock.as_ref() {
        let bn_h = crate::types::ExchangeMarginHealth {
            exchange: crate::types::Exchange::Binance,
            account_value_usd: paper.state.wallet.binance.total_equity_usd(),
            total_margin_used_usd: paper.state.wallet.binance.allocated_margin_usd,
            free_margin_usd: paper.state.wallet.binance.free_margin_usd(),
            margin_utilization_pct: paper.state.wallet.binance.utilization_pct(),
            min_liquidation_distance_pct: 50.0,
            is_healthy: true,
        };
        let hl_h = crate::types::ExchangeMarginHealth {
            exchange: crate::types::Exchange::Hyperliquid,
            account_value_usd: paper.state.wallet.hyperliquid.total_equity_usd(),
            total_margin_used_usd: paper.state.wallet.hyperliquid.allocated_margin_usd,
            free_margin_usd: paper.state.wallet.hyperliquid.free_margin_usd(),
            margin_utilization_pct: paper.state.wallet.hyperliquid.utilization_pct(),
            min_liquidation_distance_pct: 50.0,
            is_healthy: true,
        };
        let assessment = crate::state::StateStore::compute_rebalance_advisory(
            &bn_h,
            &hl_h,
            cfg.risk.rebalance_threshold_imbalance_pct,
        );
        return Json(json!({ "status": "paper_simulated", "assessment": assessment }));
    }

    Json(json!({
        "status": "unconfigured",
        "message": "Exchange API keys not fully configured and no paper wallet initialized"
    }))
}

/// GET /api/positions - Real-time active arbitrage positions
pub async fn get_positions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Reload stores from disk to ensure real-time daemon synchronization
    if let Ok(p_store) = PaperTradingStore::load_or_create(None, 500.0) {
        let mut p_lock = state.paper_store.lock();
        *p_lock = Some(p_store);
    }
    if let Ok(s_store) = crate::state::StateStore::load_or_create(None) {
        let mut s_lock = state.state_store.lock();
        *s_lock = s_store;
    }

    let mut live_positions = {
        let store = state.state_store.lock();
        store.get_active_positions()
    };

    // Update real-time mark prices and floating PnL on live positions
    for pos in &mut live_positions {
        if let Some((bn_p, hl_p)) = state.market_cache.get_latest_prices(&pos.symbol) {
            let bn_rate = state.market_cache.get_binance_rate(&pos.symbol).unwrap_or(0.0);
            let hl_rate = state.market_cache.get_hyperliquid_rate(&pos.symbol).unwrap_or(0.0);
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

    let (mut paper_positions, mut paper_wallet) = {
        let p_lock = state.paper_store.lock();
        if let Some(paper) = p_lock.as_ref() {
            (
                paper.state.active_positions.values().cloned().collect::<Vec<PaperPosition>>(),
                Some(paper.state.wallet.clone()),
            )
        } else {
            (Vec::new(), None)
        }
    };

    // Update real-time mark prices and current spread APR on paper positions,
    // and aggregate floating unrealized PnL into the virtual wallet
    let mut total_bn_unrealized = 0.0;
    let mut total_hl_unrealized = 0.0;

    for pos in &mut paper_positions {
        if let Some((bn_p, hl_p)) = state.market_cache.get_latest_prices(&pos.symbol) {
            let bn_rate = state.market_cache.get_binance_rate(&pos.symbol).unwrap_or(0.0);
            let hl_rate = state.market_cache.get_hyperliquid_rate(&pos.symbol).unwrap_or(0.0);
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

    if let Some(ref mut wallet) = paper_wallet {
        wallet.binance.unrealized_pnl_usd = total_bn_unrealized;
        wallet.hyperliquid.unrealized_pnl_usd = total_hl_unrealized;
    }

    // Deterministically sort positions to prevent UI flickering/jumping
    live_positions.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    paper_positions.sort_by(|a, b| a.symbol.cmp(&b.symbol));

    Json(PositionsResponse {
        live_positions,
        paper_positions,
        paper_wallet,
    })
}

/// GET /api/scan - Instant arbitrage opportunity matrix computed from zero-delay MarketDataCache
pub async fn get_scan(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cfg = state.config.load();
    let cost_bps = if cfg.strategy.maker_taker_mode { 7.0 } else { 14.0 };
    let mut opps = state
        .market_cache
        .compute_opportunities(cost_bps);

    // Fallback if cache is warming up
    if opps.is_empty() {
        let bn_client = BinanceFuturesClient::new(
            cfg.binance.api_key.clone(),
            cfg.binance.api_secret.clone(),
            cfg.binance.base_url.clone(),
        );
        let hl_client = HyperliquidClient::new(
            cfg.hyperliquid.private_key.clone(),
            cfg.hyperliquid.wallet_address.clone(),
            cfg.hyperliquid.base_url.clone(),
        );
        let scanner = crate::strategy::ArbitrageScanner::new(bn_client, hl_client, cfg.strategy.maker_taker_mode);
        if let Ok(scanned) = scanner.scan_opportunities().await {
            opps = scanned;
        }
    }

    // Apply whitelist/blacklist and sorting
    let mut filtered_opps: Vec<ArbitrageOpportunity> = opps
        .into_iter()
        .filter(|o| {
            if !cfg.strategy.symbol_whitelist.is_empty()
                && !cfg.strategy.symbol_whitelist.iter().any(|s| s.eq_ignore_ascii_case(&o.symbol))
            {
                return false;
            }
            if cfg.strategy.symbol_blacklist.iter().any(|s| s.eq_ignore_ascii_case(&o.symbol)) {
                return false;
            }
            true
        })
        .collect();

    filtered_opps.sort_by(|a, b| {
        b.net_spread_apr_pct
            .partial_cmp(&a.net_spread_apr_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
        });

    Json(json!({
        "count": filtered_opps.len(),
        "opportunities": filtered_opps,
        "updated_at": Utc::now()
    }))
}

/// GET /api/config - Retrieve current strategy & risk configuration (sanitized secrets)
pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<Config> {
    let cfg = state.config.load();
    let mut sanitized: Config = (**cfg).clone();
    // Mask sensitive keys for frontend display
    if !sanitized.binance.api_secret.is_empty() {
        sanitized.binance.api_secret = "********".to_string();
    }
    if !sanitized.hyperliquid.private_key.is_empty() {
        sanitized.hyperliquid.private_key = "********".to_string();
    }
    if let Some(ref mut t) = sanitized.web.auth_token {
        if !t.is_empty() {
            *t = "********".to_string();
        }
    }
    if let Some(ref mut t) = sanitized.telegram.bot_token {
        if !t.is_empty() {
            *t = "********".to_string();
        }
    }

    Json(sanitized)
}

/// POST /api/config - Hot update and immediately persist strategy & risk configuration
pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(update_payload): Json<Config>,
) -> impl IntoResponse {
    let current_cfg = state.config.load();
    let mut new_cfg = update_payload;

    // Preserve secret keys if payload masked them
    if new_cfg.binance.api_secret == "********" || new_cfg.binance.api_secret.is_empty() {
        new_cfg.binance.api_secret = current_cfg.binance.api_secret.clone();
    }
    if new_cfg.binance.api_key.is_empty() {
        new_cfg.binance.api_key = current_cfg.binance.api_key.clone();
    }
    if new_cfg.hyperliquid.private_key == "********" || new_cfg.hyperliquid.private_key.is_empty() {
        new_cfg.hyperliquid.private_key = current_cfg.hyperliquid.private_key.clone();
    }
    if new_cfg.hyperliquid.wallet_address.is_empty() {
        new_cfg.hyperliquid.wallet_address = current_cfg.hyperliquid.wallet_address.clone();
    }
    if new_cfg.telegram.bot_token.as_deref() == Some("********") || new_cfg.telegram.bot_token.is_none() {
        new_cfg.telegram.bot_token = current_cfg.telegram.bot_token.clone();
    }
    if new_cfg.web.auth_token.as_deref() == Some("********") || new_cfg.web.auth_token.is_none() {
        new_cfg.web.auth_token = current_cfg.web.auth_token.clone();
    }

    // Synchronize risk section parameters with strategy adjustments
    new_cfg.risk.stop_loss_basis_bps = new_cfg.strategy.stop_loss_basis_bps;
    new_cfg.risk.take_profit_basis_bps = new_cfg.strategy.take_profit_basis_bps;
    new_cfg.risk.max_holding_hours = new_cfg.strategy.max_holding_hours;
    new_cfg.risk.min_exit_apr_pct = new_cfg.strategy.min_exit_apr_pct;
    new_cfg.risk.fee_amortization_lock = new_cfg.strategy.fee_amortization_lock;

    match state.update_config(new_cfg.clone()) {
        Ok(_) => {
            info!("⚙️ [WEB API] Configuration successfully hot-reloaded and persisted to disk");
            Json(json!({
                "status": "ok",
                "message": "Configuration updated and hot-reloaded successfully"
            }))
        }
        Err(e) => {
            warn!("Failed to persist config update: {:?}", e);
            Json(json!({
                "status": "error",
                "message": format!("Failed to update config: {:?}", e)
            }))
        }
    }
}

/// POST /api/action/unwind - Emergency close active arbitrage positions
pub async fn action_unwind(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UnwindRequest>,
) -> impl IntoResponse {
    let sym_upper = payload.symbol.trim().to_ascii_uppercase();
    info!("🚨 [WEB API] Unwind triggered for symbol: {}", sym_upper);

    // 1. Unwind paper positions if present
    let paper_res = {
        let mut p_lock = state.paper_store.lock();
        if let Some(ref mut paper) = *p_lock {
            let mut engine = PaperExecutionEngine::new(paper.clone());
            let symbols_to_close: Vec<String> = if sym_upper == "ALL" {
                engine.store.state.active_positions.keys().cloned().collect()
            } else {
                vec![sym_upper.clone()]
            };

            let mut closed_count = 0;
            for sym in symbols_to_close {
                let (bn_px, hl_px) = state.market_cache.get_latest_prices(&sym).unwrap_or((0.0, 0.0));
                if let Ok(Some(_)) = engine.simulate_close(&sym, bn_px, hl_px, "Web API Unwind") {
                    closed_count += 1;
                }
            }
            *paper = engine.store;
            Some(closed_count)
        } else {
            None
        }
    };

    // 2. Unwind live positions in state store
    let live_closed_count = {
        let mut store = state.state_store.lock();
        let symbols: Vec<String> = if sym_upper == "ALL" {
            store.get_active_positions().into_iter().map(|p| p.symbol).collect()
        } else {
            vec![sym_upper.clone()]
        };

        let mut count = 0;
        for sym in symbols {
            let (bn_px, hl_px) = state.market_cache.get_latest_prices(&sym).unwrap_or((0.0, 0.0));
            if let Ok(Some(_)) = store.close_position(&sym, bn_px, hl_px, 0.0, "Web API Unwind") {
                count += 1;
            }
        }
        count
    };

    Json(json!({
        "status": "ok",
        "symbol": sym_upper,
        "live_positions_unwound": live_closed_count,
        "paper_positions_unwound": paper_res.unwrap_or(0)
    }))
}

/// POST /api/action/paper_trade - Execute single-shot manual paper trade (open or close)
pub async fn action_paper_trade(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PaperTradeRequest>,
) -> impl IntoResponse {
    let sym_upper = payload.symbol.trim().to_ascii_uppercase();
    let margin = payload.margin_usd.unwrap_or(50.0);
    let cfg = state.config.load();

    let mut p_lock = state.paper_store.lock();
    let paper_store = match *p_lock {
        Some(ref s) => s.clone(),
        None => {
            let new_store = match crate::paper::PaperTradingStore::load_or_create(None, 500.0) {
                Ok(s) => s,
                Err(e) => return Json(json!({ "status": "error", "message": format!("{:?}", e) })),
            };
            *p_lock = Some(new_store.clone());
            new_store
        }
    };

    let mut engine = PaperExecutionEngine::new(paper_store);

    match payload.action.to_ascii_lowercase().as_str() {
        "open" => {
            let cost_bps = if cfg.strategy.maker_taker_mode { 7.0 } else { 14.0 };
            let opps = state.market_cache.compute_opportunities(cost_bps);
            let opp = match opps.iter().find(|o| o.symbol == sym_upper) {
                Some(o) => o.clone(),
                None => {
                    return Json(json!({
                        "status": "error",
                        "message": format!("Symbol {} not found in market cache", sym_upper)
                    }));
                }
            };

            let notional_target = (margin * cfg.strategy.leverage).max(12.0);
            let prec = infer_precision_info(&opp.symbol, opp.hyperliquid_mark_price);

            let trigger_engine = ProfitTriggerEngine::new(0.0, notional_target, cfg.strategy.maker_taker_mode);
            let mut decision = trigger_engine.evaluate_opportunity(&opp, notional_target, true, Some(&prec));

            if decision.aligned_quantity.is_none() {
                let aligned = LotPrecisionMatcher::calculate_aligned_quantity(
                    &opp.symbol,
                    opp.hyperliquid_mark_price,
                    notional_target,
                    &prec,
                );
                if aligned.is_aligned {
                    decision.aligned_quantity = Some(aligned);
                    decision.should_open = true;
                }
            }

            match engine.simulate_open(&opp, &decision, &prec, ExecutionMode::MakerTaker) {
                Ok(pos) => {
                    *p_lock = Some(engine.store);
                    Json(json!({
                        "status": "ok",
                        "action": "open",
                        "position": pos
                    }))
                }
                Err(e) => Json(json!({ "status": "error", "message": format!("{:?}", e) })),
            }
        }
        "close" => {
            let (bn_px, hl_px) = state.market_cache.get_latest_prices(&sym_upper).unwrap_or((0.0, 0.0));
            match engine.simulate_close(&sym_upper, bn_px, hl_px, "Web API Paper Trade Close") {
                Ok(Some(close_ev)) => {
                    *p_lock = Some(engine.store);
                    Json(json!({
                        "status": "ok",
                        "action": "close",
                        "close_event": close_ev
                    }))
                }
                Ok(None) => Json(json!({
                    "status": "error",
                    "message": format!("No active paper position found for {}", sym_upper)
                })),
                Err(e) => Json(json!({ "status": "error", "message": format!("{:?}", e) })),
            }
        }
        _ => Json(json!({ "status": "error", "message": "Invalid action. Must be 'open' or 'close'" })),
    }
}

/// GET /api/journal - Query journal entries
pub async fn get_journal(Query(q): Query<JournalQuery>) -> impl IntoResponse {
    let journal = TradeJournal::open_default();

    let filter = JournalFilter {
        symbol: q.symbol,
        event_type: q.event_type,
        is_paper: q.paper_only,
        limit: q.limit.or(Some(50)),
        ..Default::default()
    };

    match journal.query(&filter) {
        Ok(entries) => Json(json!({ "status": "ok", "entries": entries, "count": entries.len() })),
        Err(e) => Json(json!({ "status": "error", "message": format!("{:?}", e) })),
    }
}

/// GET /api/report - Get comprehensive strategy analytics and performance attribution
pub async fn get_report(Query(q): Query<ReportQuery>) -> impl IntoResponse {
    let journal = TradeJournal::open_default();

    let capital = q.initial_capital.unwrap_or(500.0);
    match journal.read_all() {
        Ok(entries) => {
            let analytics = PerformanceAnalytics::compute_from_entries(&entries, capital);
            Json(json!({ "status": "ok", "analytics": analytics }))
        }
        Err(e) => Json(json!({ "status": "error", "message": format!("{:?}", e) })),
    }
}

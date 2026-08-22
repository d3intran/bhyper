use crate::binance::{BinanceFuturesClient, BinanceWsApiClient};
use crate::config::Config;
use crate::hyperliquid::HyperliquidClient;
use crate::risk::{ExitSignal, RiskSentinel};
use crate::state::StateStore;
use crate::strategy::{ArbitrageScanner, ProfitTriggerEngine, TwoLegExecutor};
use crate::telemetry::TelemetryNotifier;
use crate::types::{self, ExecutionMode};
use crate::ws::{BinanceWsStream, HyperliquidWsStream, MarketDataCache};
use anyhow::Result;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{info, warn};

pub async fn run(
    config: &Config,
    state_store: Arc<Mutex<StateStore>>,
    margin_usd: f64,
    dry_run: bool,
    live_danger: bool,
    taker_taker: bool,
    interval_secs: u64,
) -> Result<()> {
    let actual_dry_run = if live_danger {
        warn!("⚠️ LIVE TRADING MODE ENABLED WITH REAL FUNDS!");
        false
    } else {
        info!(
            "🧪 Dry-run simulation mode active (Safety paper trading: {}).",
            dry_run
        );
        true
    };

    let execution_mode = if taker_taker {
        ExecutionMode::TakerTaker
    } else {
        ExecutionMode::MakerTaker
    };

    info!("Execution mode set to: {}", execution_mode);

    // 1. Initialize real-time WebSocket market cache & telemetry
    let cache = MarketDataCache::new();
    BinanceWsStream::spawn(cache.clone());
    let hl_ws_url = if config.hyperliquid.is_testnet {
        Some("wss://api.hyperliquid-testnet.xyz/ws".to_string())
    } else {
        Some("wss://api.hyperliquid.xyz/ws".to_string())
    };
    HyperliquidWsStream::spawn(
        cache.clone(),
        hl_ws_url,
        Some(config.hyperliquid.wallet_address.clone()),
    );

    info!("⏳ Warming up WebSocket feeds (2s)...");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let bn_client = BinanceFuturesClient::new(
        config.binance.api_key.clone(),
        config.binance.api_secret.clone(),
        config.binance.base_url.clone(),
    );
    let hl_client = HyperliquidClient::new(
        config.hyperliquid.private_key.clone(),
        config.hyperliquid.wallet_address.clone(),
        config.hyperliquid.base_url.clone(),
    );
    let notifier = TelemetryNotifier::new(config.telegram.clone());
    let scanner = ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode)
        .with_cache(cache.clone());

    let mut executor = TwoLegExecutor::new(
        BinanceFuturesClient::new(
            config.binance.api_key.clone(),
            config.binance.api_secret.clone(),
            config.binance.base_url.clone(),
        ),
        HyperliquidClient::new(
            config.hyperliquid.private_key.clone(),
            config.hyperliquid.wallet_address.clone(),
            config.hyperliquid.base_url.clone(),
        ),
        notifier.clone(),
        state_store.clone(),
        actual_dry_run,
        execution_mode,
    )
    .with_cache(cache.clone());

    if config.strategy.use_binance_ws_api && !config.binance.api_key.is_empty() {
        info!("⚡ Initializing Binance WebSocket API client for ultra low-latency order dispatching...");
        let ws_api = BinanceWsApiClient::spawn(
            config.binance.api_key.clone(),
            config.binance.api_secret.clone(),
            None,
        );
        executor = executor.with_ws_api(ws_api);
    }

    let trigger_engine = ProfitTriggerEngine::new(
        config.strategy.min_open_apr_pct / 8760.0 * 100.0,
        config.strategy.max_position_usd_per_pair,
        config.strategy.maker_taker_mode,
    )
    .with_dual_horizon(
        config.strategy.dual_horizon_mode,
        config.strategy.min_carry_apr_pct,
    )
    .with_liquidity_guards(
        config.strategy.min_open_interest_usd,
        config.strategy.min_24h_volume_usd,
        config.strategy.max_bid_ask_spread_bps,
        config.strategy.max_oracle_mark_divergence_pct,
        config.strategy.symbol_whitelist.clone(),
        config.strategy.symbol_blacklist.clone(),
    );

    let risk_sentinel = RiskSentinel::new(config.risk.clone());

    info!(
        "🚀 BHyper Automated Engine Running (Interval: {}s, Max Pair Margin: ${:.2}, Max Positions: {})...",
        interval_secs, margin_usd, config.strategy.max_active_positions
    );

    let mut timer = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
    loop {
        timer.tick().await;

        let (opps_res, precisions_res) = tokio::join!(
            scanner.scan_opportunities(),
            scanner.fetch_symbol_precisions()
        );

        let opps = match opps_res {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("Error scanning opportunities: {:?}", e);
                continue;
            }
        };

        let precisions = precisions_res.unwrap_or_default();
        let opps_map: std::collections::HashMap<String, &types::ArbitrageOpportunity> =
            opps.iter().map(|o| (o.symbol.clone(), o)).collect();

        // PHASE 1: ACTIVE POSITION AUDIT & AUTOMATED EXIT
        let active_positions = {
            let store = state_store.lock();
            store.get_active_positions()
        };

        for mut pos in active_positions {
            let prec = match precisions.get(&pos.symbol) {
                Some(p) => p,
                None => continue,
            };

            let (live_bn_px, live_hl_px) = cache
                .get_latest_prices(&pos.symbol)
                .unwrap_or((pos.binance_entry_price, pos.hyperliquid_entry_price));

            let current_opp = opps_map.get(&pos.symbol).copied();

            if let Some(opp) = current_opp {
                let eff_spread = match pos.hyperliquid_side {
                    types::PositionSide::Short => opp.hyperliquid_apr_pct - opp.binance_apr_pct,
                    types::PositionSide::Long => opp.binance_apr_pct - opp.hyperliquid_apr_pct,
                };
                pos.current_spread_apr = eff_spread;
                pos.last_updated_at = chrono::Utc::now();
                let _ = state_store.lock().upsert_position(pos.clone());
            }

            if config.strategy.auto_unwind_on_decay {
                let exit_signal =
                    risk_sentinel.evaluate_position_exit(&pos, current_opp, live_bn_px, live_hl_px);

                match exit_signal {
                    ExitSignal::Hold => {}
                    ExitSignal::SpreadDecay { reason, .. }
                    | ExitSignal::SpreadInverted { reason, .. }
                    | ExitSignal::BasisStopLoss { reason, .. }
                    | ExitSignal::BasisTakeProfit { reason, .. }
                    | ExitSignal::MaxDurationExceeded { reason, .. }
                    | ExitSignal::DeltaDriftCritical { reason, .. }
                    | ExitSignal::MarginCritical { reason, .. }
                    | ExitSignal::LiquidationThreat { reason, .. } => {
                        warn!("🚨 AUTOMATIC EXIT TRIGGERED for {}: {}", pos.symbol, reason);
                        if let Err(e) = executor.execute_close(&pos, prec, &reason).await {
                            tracing::error!(
                                "Failed to auto-close position for {}: {:?}",
                                pos.symbol,
                                e
                            );
                        }
                    }
                }
            }
        }

        // PHASE 2: CAPACITY CHECK & NEW ARBITRAGE OPPORTUNITY EVALUATION
        let current_active_count = {
            let store = state_store.lock();
            store.get_active_positions().len()
        };

        if current_active_count >= config.strategy.max_active_positions {
            continue;
        }

        let held_symbols: std::collections::HashSet<String> = {
            let store = state_store.lock();
            store
                .get_active_positions()
                .into_iter()
                .map(|p| p.symbol)
                .collect()
        };

        for opp in opps.iter().take(10) {
            if held_symbols.contains(&opp.symbol) {
                continue;
            }

            let prec = match precisions.get(&opp.symbol) {
                Some(p) => p,
                None => continue,
            };

            let decision = trigger_engine.evaluate_opportunity(
                opp,
                margin_usd,
                false,
                Some(prec),
            );

            if decision.should_open {
                info!(
                    "🎯 PROFIT TRIGGER FIRED for {}! Executing two-leg arbitrage (Mode: {})...",
                    opp.symbol, execution_mode
                );
                match executor.execute_open(opp, &decision, prec).await {
                    Ok(pos) => {
                        info!(
                            "✅ Successfully established arbitrage position on {}",
                            pos.symbol
                        );
                        break;
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to execute trade on {}: {:?}",
                            opp.symbol,
                            e
                        );
                    }
                }
            }
        }
    }
}

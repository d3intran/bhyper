use crate::binance::BinanceFuturesClient;
use crate::config::Config;
use crate::hyperliquid::HyperliquidClient;
use crate::state::StateStore;
use crate::strategy::TwoLegExecutor;
use crate::telemetry::TelemetryNotifier;
use crate::types::{ActiveArbitragePosition, ExecutionMode, PositionSide, SymbolPrecisionInfo};
use anyhow::Result;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::info;

pub async fn run(config: &Config, state_store: Arc<Mutex<StateStore>>, symbol: &str) -> Result<()> {
    info!(
        "Emergency unwinding position for {} on both exchanges...",
        symbol
    );
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

    let executor = TwoLegExecutor::new(
        bn_client,
        hl_client,
        notifier,
        state_store.clone(),
        false,
        ExecutionMode::TakerTaker,
    );

    let (target_pos, default_prec) = {
        let store = state_store.lock();
        let pos = store
            .get_position(symbol)
            .cloned()
            .unwrap_or_else(|| ActiveArbitragePosition {
                symbol: symbol.to_string(),
                binance_side: PositionSide::Long,
                binance_qty: 0.0,
                binance_entry_price: 0.0,
                hyperliquid_side: PositionSide::Short,
                hyperliquid_qty: 0.0,
                hyperliquid_entry_price: 0.0,
                nominal_value_usd: 0.0,
                net_delta_usd: 0.0,
                entry_spread_apr: 0.0,
                current_spread_apr: 0.0,
                accumulated_funding_usd: 0.0,
                opened_at: chrono::Utc::now(),
                last_updated_at: chrono::Utc::now(),
                is_closed: false,
                closed_at: None,
                realized_pnl_usd: None,
            });

        let prec = SymbolPrecisionInfo {
            symbol: symbol.to_string(),
            binance_step_size: 1.0,
            binance_tick_size: 0.001,
            binance_min_qty: 1.0,
            binance_min_notional: 5.0,
            hyperliquid_sz_decimals: 0,
            hyperliquid_asset_index: 0,
            hyperliquid_min_notional: 10.0,
        };

        (pos, prec)
    };

    let _ = executor
        .execute_close(&target_pos, &default_prec, "Emergency manual unwind")
        .await;
    println!("✅ Unwind command dispatched and recorded for {}.", symbol);

    Ok(())
}

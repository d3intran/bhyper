use crate::binance::BinanceFuturesClient;
use crate::config::Config;
use crate::hyperliquid::HyperliquidClient;
use crate::state::StateStore;
use crate::telemetry::TelemetryNotifier;
use anyhow::Result;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::warn;

pub async fn run(config: &Config, state_store: Arc<Mutex<StateStore>>) -> Result<()> {
    println!(
        "🔍 Fetching live exchange positions from Binance and Hyperliquid for reconciliation..."
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

    let (bn_pos_res, hl_pos_res, bn_health_res, hl_health_res) = tokio::join!(
        bn_client.fetch_positions(),
        hl_client.fetch_clearinghouse_state(),
        bn_client.fetch_margin_health(),
        hl_client.fetch_margin_health()
    );

    let bn_pos = match bn_pos_res {
        Ok(p) => p,
        Err(e) => {
            warn!("Could not fetch Binance positions: {:?}", e);
            Vec::new()
        }
    };

    let hl_pos = match hl_pos_res {
        Ok(s) => s.asset_positions,
        Err(e) => {
            warn!("Could not fetch Hyperliquid positions: {:?}", e);
            Vec::new()
        }
    };

    let mut report = {
        let mut store = state_store.lock();
        store.reconcile(&bn_pos, &hl_pos)
    };

    if let (Ok(bn_h), Ok(hl_h)) = (bn_health_res, hl_health_res) {
        let assessment = StateStore::compute_rebalance_advisory(
            &bn_h,
            &hl_h,
            config.risk.rebalance_threshold_imbalance_pct,
        );
        report.margin_assessment = Some(assessment);
    }

    TelemetryNotifier::render_reconciliation_report(&report);

    Ok(())
}

use crate::binance::BinanceFuturesClient;
use crate::config::Config;
use crate::hyperliquid::HyperliquidClient;
use crate::state::StateStore;
use crate::telemetry::TelemetryNotifier;
use crate::types::Exchange;
use anyhow::Result;
use tracing::warn;

pub async fn run(config: &Config) -> Result<()> {
    println!("🔍 Fetching live cross-exchange margin health and account balances...");
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

    let (bn_health_res, hl_health_res) = tokio::join!(
        bn_client.fetch_margin_health(),
        hl_client.fetch_margin_health()
    );

    let bn_health = match bn_health_res {
        Ok(h) => h,
        Err(e) => {
            warn!(
                "Could not fetch Binance margin health (Check API keys): {:?}",
                e
            );
            crate::types::ExchangeMarginHealth {
                exchange: Exchange::Binance,
                account_value_usd: 0.0,
                total_margin_used_usd: 0.0,
                free_margin_usd: 0.0,
                margin_utilization_pct: 0.0,
                min_liquidation_distance_pct: 100.0,
                is_healthy: true,
            }
        }
    };

    let hl_health = match hl_health_res {
        Ok(h) => h,
        Err(e) => {
            warn!(
                "Could not fetch Hyperliquid margin health (Check Wallet address): {:?}",
                e
            );
            crate::types::ExchangeMarginHealth {
                exchange: Exchange::Hyperliquid,
                account_value_usd: 0.0,
                total_margin_used_usd: 0.0,
                free_margin_usd: 0.0,
                margin_utilization_pct: 0.0,
                min_liquidation_distance_pct: 100.0,
                is_healthy: true,
            }
        }
    };

    let assessment = StateStore::compute_rebalance_advisory(
        &bn_health,
        &hl_health,
        config.risk.rebalance_threshold_imbalance_pct,
    );

    TelemetryNotifier::render_margin_assessment(&assessment);

    Ok(())
}

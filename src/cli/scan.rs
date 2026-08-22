use crate::binance::BinanceFuturesClient;
use crate::config::Config;
use crate::hyperliquid::HyperliquidClient;
use crate::strategy::ArbitrageScanner;
use crate::telemetry::TelemetryNotifier;
use anyhow::Result;

pub async fn run(config: &Config, limit: usize) -> Result<()> {
    println!("🔍 Connecting to Binance FAPI and Hyperliquid L1...");
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

    let scanner = ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode);

    let start = std::time::Instant::now();
    let opps = scanner.scan_opportunities().await?;
    let elapsed = start.elapsed();

    TelemetryNotifier::render_console_table(&opps, limit);
    println!(
        "✅ Scanned {} pairs in {:.2}ms. Config min APR threshold: {:.1}%\n",
        opps.len(),
        elapsed.as_secs_f64() * 1000.0,
        config.strategy.min_open_apr_pct
    );

    Ok(())
}

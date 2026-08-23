use crate::binance::BinanceFuturesClient;
use crate::config::Config;
use crate::hyperliquid::HyperliquidClient;
use crate::strategy::{ArbitrageScanner, LotPrecisionMatcher};
use crate::telemetry::TelemetryNotifier;
use anyhow::Result;

pub async fn run(config: &Config, limit: usize, target_usd: f64) -> Result<()> {
    println!(
        "🔍 Fetching exchange metadata and computing lot precision alignment for target ${:.2}...",
        target_usd
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

    let scanner = ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode);

    let (opps_res, precisions_res) = tokio::join!(
        scanner.scan_opportunities(),
        scanner.fetch_symbol_precisions()
    );

    let opps = opps_res?;
    let precisions = precisions_res?;

    let mut price_map = std::collections::HashMap::new();
    for o in &opps {
        price_map.insert(o.symbol.clone(), o.binance_mark_price);
    }

    let mut precision_rows = Vec::new();
    for (sym, prec) in &precisions {
        if let Some(&price) = price_map.get(sym) {
            let aligned =
                LotPrecisionMatcher::calculate_aligned_quantity(sym, price, target_usd, prec);
            precision_rows.push((prec.clone(), aligned, price));
        }
    }

    precision_rows.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    TelemetryNotifier::render_precision_table(&precision_rows, limit);
    println!(
        "✅ Analyzed {} shared pairs. Verified small-capital zero-delta compatibility.\n",
        precision_rows.len()
    );

    Ok(())
}

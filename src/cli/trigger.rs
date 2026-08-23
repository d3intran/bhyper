use crate::binance::BinanceFuturesClient;
use crate::config::Config;
use crate::hyperliquid::HyperliquidClient;
use crate::strategy::{ArbitrageScanner, ProfitTriggerEngine};
use anyhow::Result;

pub async fn run(config: &Config, margin_usd: f64, ignore_window: bool) -> Result<()> {
    println!(
        "🎯 Running Deterministic Profit Trigger Evaluation (Margin: ${:.2}, Ignore Window: {})...",
        margin_usd, ignore_window
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
    let precisions = precisions_res.unwrap_or_default();
    let trigger_engine = ProfitTriggerEngine::default().with_liquidity_guards(
        config.strategy.min_open_interest_usd,
        config.strategy.min_24h_volume_usd,
        config.strategy.max_bid_ask_spread_bps,
        config.strategy.max_oracle_mark_divergence_pct,
        config.strategy.symbol_whitelist.clone(),
        config.strategy.symbol_blacklist.clone(),
    );

    println!("\n{}", "=".repeat(130));
    println!("🎯 BHyper Deterministic Profit Trigger Analysis (With Exact Lot Precision Math)");
    println!("{}", "=".repeat(130));
    println!(
        "{:<8} {:<8} {:<12} {:<12} {:<14} {:<14} {:<15} {:<28}",
        "Symbol",
        "Trigger",
        "1h Inc(bps)",
        "Friction",
        "4h Net(bps)",
        "4h Net USD",
        "Aligned Qty",
        "Status / Reason"
    );
    println!("{}", "-".repeat(130));

    let mut passed_count = 0;
    for opp in opps.iter().take(20) {
        let prec_info = precisions.get(&opp.symbol);
        let decision =
            trigger_engine.evaluate_opportunity(opp, margin_usd, ignore_window, prec_info);

        let trigger_badge = if decision.should_open {
            passed_count += 1;
            "✅ YES"
        } else {
            "❌ NO"
        };

        let aligned_str = match &decision.aligned_quantity {
            Some(a) => format!("{} (0-Delta)", a.binance_formatted_qty),
            None => "N/A".to_string(),
        };

        let reason_str = decision
            .reject_reason
            .unwrap_or_else(|| "🎯 ALL PROFIT LOCKS PASSED!".to_string());

        println!(
            "{:<8} {:<8} {:>10.2} bps {:>10.2} bps {:>12.2} bps ${:<13.4} {:<15} {:<28}",
            decision.symbol,
            trigger_badge,
            decision.single_cycle_income_bps,
            decision.total_friction_cost_bps,
            decision.projected_4h_net_bps,
            decision.net_expected_profit_usd,
            aligned_str,
            reason_str
        );
    }
    println!("{}\n", "=".repeat(130));
    let secs_left = ProfitTriggerEngine::seconds_until_next_hour();
    let is_bn = ProfitTriggerEngine::is_binance_settlement_hour();
    println!(
        "⏱️  Seconds to next hourly settlement: {}s | Next hour is Binance 8h settlement: {}",
        secs_left,
        if is_bn {
            "✅ YES"
        } else {
            "❌ NO (HL 1h only)"
        }
    );
    println!("📊 Actionable Triggers: {} / 20 evaluated.\n", passed_count);

    Ok(())
}

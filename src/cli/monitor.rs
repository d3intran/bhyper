use crate::binance::BinanceFuturesClient;
use crate::config::Config;
use crate::hyperliquid::HyperliquidClient;
use crate::strategy::ArbitrageScanner;
use crate::telemetry::TelemetryNotifier;
use anyhow::Result;
use tracing::info;

pub async fn run(config: &Config, interval_secs: u64) -> Result<()> {
    info!(
        "Starting BHyper live monitoring loop (interval: {}s)...",
        interval_secs
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
    let scanner = ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode);

    let mut timer = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
    loop {
        timer.tick().await;
        match scanner.scan_opportunities().await {
            Ok(opps) => {
                let top_opportunities: Vec<_> = opps
                    .into_iter()
                    .filter(|o| o.net_spread_apr_pct >= config.strategy.min_open_apr_pct)
                    .collect();

                if !top_opportunities.is_empty() {
                    info!(
                        "Found {} actionable arbitrage opportunities > {:.1}% APR!",
                        top_opportunities.len(),
                        config.strategy.min_open_apr_pct
                    );
                    TelemetryNotifier::render_console_table(&top_opportunities, 5);

                    if let Some(best) = top_opportunities.first() {
                        let alert_msg = format!(
                            "🚨 <b>BHyper 套利机会发现!</b>\n\n\
                            • <b>标的:</b> <code>{}</code>\n\
                            • <b>净利差 APR:</b> <code>{:.2}%</code>\n\
                            • <b>Binance APR:</b> <code>{:.2}%</code>\n\
                            • <b>Hyperliquid APR:</b> <code>{:.2}%</code>\n\
                            • <b>推荐操作:</b> <code>Hyperliquid {} | Binance {}</code>\n\
                            • <b>预计时收益:</b> <code>{:.2} bps/h</code> (回本时间: <code>{:.1}h</code>)\n\
                            • <b>4h净利预估:</b> <code>{:.2} bps</code>",
                            best.symbol,
                            best.net_spread_apr_pct,
                            best.binance_apr_pct,
                            best.hyperliquid_apr_pct,
                            best.hyperliquid_side,
                            best.binance_side,
                            best.est_hourly_return_bps,
                            best.est_break_even_hours,
                            best.projected_4h_net_bps
                        );
                        let _ = notifier.send_alert(&alert_msg).await;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Error scanning opportunities: {:?}", e);
            }
        }
    }
}

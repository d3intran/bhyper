use crate::config::TelegramConfig;
use crate::types::{AlignedQuantity, ArbitrageOpportunity, SymbolPrecisionInfo};
use anyhow::Result;
use serde_json::json;

#[derive(Clone)]
pub struct TelemetryNotifier {
    config: TelegramConfig,
    client: reqwest::Client,
}

impl TelemetryNotifier {
    pub fn new(config: TelegramConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Sends a Markdown/HTML formatted message to the user's Telegram chat
    pub async fn send_alert(&self, message: &str) -> Result<()> {
        if !self.config.alerts_enabled {
            return Ok(());
        }
        let token = match &self.config.bot_token {
            Some(t) if !t.is_empty() => t,
            _ => return Ok(()),
        };
        let chat_id = match self.config.chat_id {
            Some(id) => id,
            None => return Ok(()),
        };

        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
        let payload = json!({
            "chat_id": chat_id,
            "text": message,
            "parse_mode": "HTML"
        });

        let _ = self.client.post(&url).json(&payload).send().await;
        Ok(())
    }

    /// Prints a formatted ASCII table of top arbitrage opportunities
    pub fn render_console_table(opportunities: &[ArbitrageOpportunity], top_n: usize) {
        println!("\n{}", "=".repeat(110));
        println!("🚀 BHyper Multi-Asset Cross-Exchange Funding Arbitrage Matrix (Top Ranked)");
        println!("{}", "=".repeat(110));
        println!(
            "{:<8} {:<10} {:<10} {:<12} {:<12} {:<12} {:<15} {:<12} {:<10}",
            "Symbol",
            "BN Price",
            "HL Price",
            "BN APR(8h)",
            "HL APR(1h)",
            "Net Spread",
            "Action",
            "1h PnL (bps)",
            "Break-Even"
        );
        println!("{}", "-".repeat(110));

        for opp in opportunities.iter().take(top_n) {
            let action_str = format!("HL:{} | BN:{}", opp.hyperliquid_side, opp.binance_side);
            let be_str = if opp.est_break_even_hours > 500.0 {
                ">500h".to_string()
            } else {
                format!("{:.1}h", opp.est_break_even_hours)
            };

            println!(
                "{:<8} ${:<9.3} ${:<9.3} {:>10.2}% {:>10.2}% {:>10.2}% {:<15} {:>10.2} bps {:>10}",
                opp.symbol,
                opp.binance_mark_price,
                opp.hyperliquid_mark_price,
                opp.binance_apr_pct,
                opp.hyperliquid_apr_pct,
                opp.net_spread_apr_pct,
                action_str,
                opp.est_hourly_return_bps,
                be_str
            );
        }
        println!("{}\n", "=".repeat(110));
    }

    /// Prints a formatted ASCII table for precision matching analysis
    pub fn render_precision_table(
        precisions: &[(SymbolPrecisionInfo, AlignedQuantity, f64)],
        top_n: usize,
    ) {
        println!("\n{}", "=".repeat(115));
        println!(
            "📐 BHyper Small-Capital Lot Precision & Alignment Matrix ($50 Target Allocation)"
        );
        println!("{}", "=".repeat(115));
        println!(
            "{:<8} {:<10} {:<12} {:<10} {:<12} {:<12} {:<10} {:<30}",
            "Symbol",
            "Price",
            "BN StepSize",
            "HL Decs",
            "BN Formatted",
            "HL Formatted",
            "Notional",
            "Status / Notes"
        );
        println!("{}", "-".repeat(115));

        for (prec, aligned, price) in precisions.iter().take(top_n) {
            let status = if aligned.is_aligned {
                "✅ PERFECT MATCH (0 Delta)".to_string()
            } else {
                aligned
                    .reject_reason
                    .clone()
                    .unwrap_or_else(|| "❌ MISMATCH".to_string())
            };

            println!(
                "{:<8} ${:<9.3} {:<12} {:<10} {:<12} {:<12} ${:<9.2} {:<30}",
                prec.symbol,
                price,
                prec.binance_step_size,
                prec.hyperliquid_sz_decimals,
                aligned.binance_formatted_qty,
                aligned.hyperliquid_formatted_qty,
                aligned.notional_usd,
                status
            );
        }
        println!("{}\n", "=".repeat(115));
    }
}

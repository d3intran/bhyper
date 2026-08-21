use crate::config::TelegramConfig;
use crate::types::{
    ActiveArbitragePosition, AlignedQuantity, ArbitrageOpportunity, ReconciliationReport,
    SymbolPrecisionInfo,
};
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
            _ => return Ok(()),
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
        println!("\n{}", "=".repeat(128));
        println!("🚀 BHyper Multi-Asset Cross-Exchange Funding Arbitrage Matrix (Multi-Horizon Projections)");
        println!("{}", "=".repeat(128));
        println!(
            "{:<8} {:<9} {:<9} {:<11} {:<11} {:<11} {:<14} {:<9} {:<10} {:<10} {:<10}",
            "Symbol",
            "BN Price",
            "HL Price",
            "BN APR(8h)",
            "HL APR(1h)",
            "Net Spread",
            "Action",
            "BreakEven",
            "1h Net(bps)",
            "4h Net(bps)",
            "8h Net(bps)"
        );
        println!("{}", "-".repeat(128));

        for opp in opportunities.iter().take(top_n) {
            let action_str = format!("HL:{} | BN:{}", opp.hyperliquid_side, opp.binance_side);
            let be_str = if opp.est_break_even_hours > 500.0 {
                ">500h".to_string()
            } else {
                format!("{:.1}h", opp.est_break_even_hours)
            };

            println!(
                "{:<8} ${:<8.3} ${:<8.3} {:>9.2}% {:>9.2}% {:>9.2}% {:<14} {:>9} {:>10.2} {:>10.2} {:>10.2}",
                opp.symbol,
                opp.binance_mark_price,
                opp.hyperliquid_mark_price,
                opp.binance_apr_pct,
                opp.hyperliquid_apr_pct,
                opp.net_spread_apr_pct,
                action_str,
                be_str,
                opp.projected_1h_net_bps,
                opp.projected_4h_net_bps,
                opp.projected_8h_net_bps
            );
        }
        println!("{}\n", "=".repeat(128));
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

    /// Prints active positions table
    pub fn render_positions_table(positions: &[ActiveArbitragePosition]) {
        println!("\n{}", "=".repeat(110));
        println!("💼 BHyper Active Managed Arbitrage Positions");
        println!("{}", "=".repeat(110));
        if positions.is_empty() {
            println!("  (No active arbitrage positions found in local store)");
        } else {
            println!(
                "{:<8} {:<12} {:<12} {:<12} {:<12} {:<12} {:<20}",
                "Symbol", "Notional", "HL Side/Qty", "BN Side/Qty", "Entry APR", "Current APR", "Opened At"
            );
            println!("{}", "-".repeat(110));
            for p in positions {
                let hl_str = format!("{} {:.4}", p.hyperliquid_side, p.hyperliquid_qty);
                let bn_str = format!("{} {:.4}", p.binance_side, p.binance_qty);
                println!(
                    "{:<8} ${:<11.2} {:<12} {:<12} {:>10.2}% {:>10.2}% {:<20}",
                    p.symbol,
                    p.nominal_value_usd,
                    hl_str,
                    bn_str,
                    p.entry_spread_apr,
                    p.current_spread_apr,
                    p.opened_at.format("%Y-%m-%d %H:%M:%S")
                );
            }
        }
        println!("{}\n", "=".repeat(110));
    }

    /// Prints reconciliation report
    pub fn render_reconciliation_report(report: &ReconciliationReport) {
        println!("\n{}", "=".repeat(90));
        println!("🔍 BHyper Cross-Exchange Reconciliation & Health Audit");
        println!("{}", "=".repeat(90));
        println!("• Consistent State: {}", if report.is_consistent { "✅ YES" } else { "⚠️ DISCREPANCY DETECTED" });
        println!("• Active Matched Pairs: {}", report.active_pairs_count);
        if !report.orphaned_binance_positions.is_empty() {
            println!("• ⚠️ Orphaned Binance Positions: {:?}", report.orphaned_binance_positions);
        }
        if !report.orphaned_hyperliquid_positions.is_empty() {
            println!("• ⚠️ Orphaned Hyperliquid Positions: {:?}", report.orphaned_hyperliquid_positions);
        }
        if !report.delta_discrepancies.is_empty() {
            println!("• ⚠️ Delta Discrepancies: {:?}", report.delta_discrepancies);
        }
        for w in &report.warnings {
            println!("  - {}", w);
        }
        println!("{}\n", "=".repeat(90));
    }
}

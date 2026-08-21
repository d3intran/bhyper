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

    /// Prints a formatted ASCII table of top arbitrage opportunities with full liquidity telemetry
    pub fn render_console_table(opportunities: &[ArbitrageOpportunity], top_n: usize) {
        println!("\n{}", "=".repeat(145));
        println!("🚀 BHyper Multi-Asset Cross-Exchange Funding Arbitrage Matrix (Multi-Horizon Projections & Liquidity Tiering)");
        println!("{}", "=".repeat(145));
        println!(
            "{:<8} {:<9} {:<9} {:<11} {:<11} {:<11} {:<14} {:<9} {:<10} {:<10} {:<12} {:<14} {:<12}",
            "Symbol",
            "BN Price",
            "HL Price",
            "BN APR(8h)",
            "HL APR(1h)",
            "Net Spread",
            "Action",
            "BreakEven",
            "1h Net",
            "4h Net",
            "Total OI",
            "24h Vol(BN)",
            "Liquidity"
        );
        println!("{}", "-".repeat(145));

        for opp in opportunities.iter().take(top_n) {
            let action_str = format!("HL:{} | BN:{}", opp.hyperliquid_side, opp.binance_side);
            let be_str = if opp.est_break_even_hours > 500.0 {
                ">500h".to_string()
            } else {
                format!("{:.1}h", opp.est_break_even_hours)
            };

            let oi_str = if opp.total_open_interest_usd >= 1_000_000.0 {
                format!("${:.1}M", opp.total_open_interest_usd / 1_000_000.0)
            } else if opp.total_open_interest_usd > 0.0 {
                format!("${:.0}k", opp.total_open_interest_usd / 1000.0)
            } else {
                "N/A".to_string()
            };

            let vol_str = if opp.binance_volume_24h_usd >= 1_000_000.0 {
                format!("${:.1}M", opp.binance_volume_24h_usd / 1_000_000.0)
            } else if opp.binance_volume_24h_usd > 0.0 {
                format!("${:.0}k", opp.binance_volume_24h_usd / 1000.0)
            } else {
                "N/A".to_string()
            };

            let tier_badge = match opp.liquidity_tier.as_str() {
                "TIER_1_PRIME" => "🟢 PRIME",
                "TIER_2_LIQUID" => "🟢 LIQUID",
                "TIER_3_MID" => "🟡 MID",
                "STREAM_WS" => "⚡ WS-LIVE",
                _ => "🔴 ILLIQUID",
            };

            println!(
                "{:<8} ${:<8.3} ${:<8.3} {:>9.2}% {:>9.2}% {:>9.2}% {:<14} {:>9} {:>8.2}bps {:>8.2}bps {:<12} {:<14} {:<12}",
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
                oi_str,
                vol_str,
                tier_badge
            );
        }
        println!("{}\n", "=".repeat(145));
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
                "Symbol",
                "Notional",
                "HL Side/Qty",
                "BN Side/Qty",
                "Entry APR",
                "Current APR",
                "Opened At"
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

    /// Prints cross-exchange margin assessment and rebalancing table
    pub fn render_margin_assessment(assessment: &crate::types::CrossExchangeMarginAssessment) {
        println!("\n{}", "=".repeat(95));
        println!("⚖️  BHyper Cross-Exchange Margin Health & Capital Balance Assessment");
        println!("{}", "=".repeat(95));
        println!(
            "{:<16} {:<16} {:<16} {:<16} {:<16} {:<10}",
            "Exchange", "Total Equity", "Margin Used", "Free Margin", "Utilization", "Liq Buffer"
        );
        println!("{}", "-".repeat(95));

        println!(
            "{:<16} ${:<15.2} ${:<15.2} ${:<15.2} {:>11.1}% {:>13.1}%",
            "Binance",
            assessment.binance.account_value_usd,
            assessment.binance.total_margin_used_usd,
            assessment.binance.free_margin_usd,
            assessment.binance.margin_utilization_pct,
            assessment.binance.min_liquidation_distance_pct
        );

        println!(
            "{:<16} ${:<15.2} ${:<15.2} ${:<15.2} {:>11.1}% {:>13.1}%",
            "Hyperliquid",
            assessment.hyperliquid.account_value_usd,
            assessment.hyperliquid.total_margin_used_usd,
            assessment.hyperliquid.free_margin_usd,
            assessment.hyperliquid.margin_utilization_pct,
            assessment.hyperliquid.min_liquidation_distance_pct
        );

        println!("{}", "-".repeat(95));
        println!(
            "• Total Cross-Exchange Equity: ${:.2}",
            assessment.total_equity_usd
        );
        println!(
            "• Capital Asymmetry / Imbalance: ${:.2}",
            assessment.imbalance_usd.abs()
        );
        println!("• Status: {}", assessment.risk_status);
        if assessment.rebalance_required {
            println!("• 💡 Suggested Transfer: {}", assessment.transfer_direction);
        }
        println!("{}\n", "=".repeat(95));
    }

    /// Prints reconciliation report
    pub fn render_reconciliation_report(report: &ReconciliationReport) {
        println!("\n{}", "=".repeat(90));
        println!("🔍 BHyper Cross-Exchange Reconciliation & Health Audit");
        println!("{}", "=".repeat(90));
        println!(
            "• Consistent State: {}",
            if report.is_consistent {
                "✅ YES"
            } else {
                "⚠️ DISCREPANCY DETECTED"
            }
        );
        println!("• Active Matched Pairs: {}", report.active_pairs_count);
        if !report.orphaned_binance_positions.is_empty() {
            println!(
                "• ⚠️ Orphaned Binance Positions: {:?}",
                report.orphaned_binance_positions
            );
        }
        if !report.orphaned_hyperliquid_positions.is_empty() {
            println!(
                "• ⚠️ Orphaned Hyperliquid Positions: {:?}",
                report.orphaned_hyperliquid_positions
            );
        }
        if !report.delta_discrepancies.is_empty() {
            println!("• ⚠️ Delta Discrepancies: {:?}", report.delta_discrepancies);
        }
        for w in &report.warnings {
            println!("  - {}", w);
        }
        if let Some(ref assessment) = report.margin_assessment {
            Self::render_margin_assessment(assessment);
        }
        println!("{}\n", "=".repeat(90));
    }
}

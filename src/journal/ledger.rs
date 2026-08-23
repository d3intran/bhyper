use crate::types::{Exchange, PositionSide};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum JournalEntry {
    Intent(TradeIntentEvent),
    OpenFill(TradeOpenFillEvent),
    Funding(FundingSettlementEvent),
    RiskAlert(RiskAuditEvent),
    CloseFill(TradeCloseFillEvent),
}

impl JournalEntry {
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            JournalEntry::Intent(e) => e.timestamp,
            JournalEntry::OpenFill(e) => e.timestamp,
            JournalEntry::Funding(e) => e.timestamp,
            JournalEntry::RiskAlert(e) => e.timestamp,
            JournalEntry::CloseFill(e) => e.timestamp,
        }
    }

    pub fn symbol(&self) -> &str {
        match self {
            JournalEntry::Intent(e) => &e.symbol,
            JournalEntry::OpenFill(e) => &e.symbol,
            JournalEntry::Funding(e) => &e.symbol,
            JournalEntry::RiskAlert(e) => &e.symbol,
            JournalEntry::CloseFill(e) => &e.symbol,
        }
    }

    pub fn is_paper(&self) -> bool {
        match self {
            JournalEntry::Intent(e) => e.is_paper,
            JournalEntry::OpenFill(e) => e.is_paper,
            JournalEntry::Funding(e) => e.is_paper,
            JournalEntry::RiskAlert(e) => e.is_paper,
            JournalEntry::CloseFill(e) => e.is_paper,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeIntentEvent {
    pub id: String,
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub is_paper: bool,
    pub hyperliquid_side: PositionSide,
    pub binance_side: PositionSide,
    pub hyperliquid_apr_pct: f64,
    pub binance_apr_pct: f64,
    pub net_spread_apr_pct: f64,
    pub projected_1h_net_bps: f64,
    pub projected_4h_net_bps: f64,
    pub target_notional_usd: f64,
    pub aligned_qty: f64,
    pub friction_cost_bps: f64,
    pub est_hourly_return_bps: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeOpenFillEvent {
    pub id: String,
    pub intent_id: String,
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub is_paper: bool,
    pub hyperliquid_side: PositionSide,
    pub hyperliquid_qty: f64,
    pub hyperliquid_price: f64,
    pub hyperliquid_fee_usd: f64,
    pub hyperliquid_mode: String,
    pub binance_side: PositionSide,
    pub binance_qty: f64,
    pub binance_price: f64,
    pub binance_fee_usd: f64,
    pub binance_mode: String,
    pub total_notional_usd: f64,
    pub entry_price_spread_bps: f64,
    pub total_open_fees_usd: f64,
    pub execution_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingSettlementEvent {
    pub id: String,
    pub position_id: String,
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub is_paper: bool,
    pub exchange: Exchange,
    pub side: PositionSide,
    pub rate_bps: f64,
    pub annualized_apr_pct: f64,
    pub mark_price: f64,
    pub position_qty: f64,
    pub notional_usd: f64,
    pub funding_payment_usd: f64,
    pub cumulative_funding_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAuditEvent {
    pub id: String,
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub is_paper: bool,
    pub event_type: String,
    pub details: String,
    pub action_taken: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeCloseFillEvent {
    pub id: String,
    pub open_trade_id: String,
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub is_paper: bool,
    pub holding_duration_secs: u64,
    pub exit_reason: String,
    pub hyperliquid_exit_price: f64,
    pub hyperliquid_exit_fee_usd: f64,
    pub binance_exit_price: f64,
    pub binance_exit_fee_usd: f64,
    pub total_exit_fees_usd: f64,
    pub total_roundtrip_fees_usd: f64,
    pub gross_basis_pnl_usd: f64,
    pub gross_funding_earned_usd: f64,
    pub net_realized_pnl_usd: f64,
    pub net_return_bps: f64,
    pub return_on_capital_pct: f64,
}

#[derive(Debug, Clone, Default)]
pub struct JournalFilter {
    pub symbol: Option<String>,
    pub is_paper: Option<bool>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub event_type: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct TradeJournal {
    path: PathBuf,
}

impl TradeJournal {
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/bhyper/trade_journal.jsonl")
    }

    pub fn new(path_opt: Option<PathBuf>) -> Self {
        let path = path_opt.unwrap_or_else(Self::default_path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        Self { path }
    }

    pub fn open_default() -> Self {
        Self::new(None)
    }

    pub fn append(&self, entry: &JournalEntry) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("Failed to open trade journal at {}", self.path.display()))?;

        let json_line =
            serde_json::to_string(entry).context("Failed to serialize journal entry to JSON")?;
        writeln!(file, "{}", json_line)
            .with_context(|| format!("Failed to write entry to {}", self.path.display()))?;
        Ok(())
    }

    pub fn read_all(&self) -> Result<Vec<JournalEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path)
            .with_context(|| format!("Failed to open journal file {}", self.path.display()))?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for line_res in reader.lines() {
            let line = line_res?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<JournalEntry>(trimmed) {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    pub fn query(&self, filter: &JournalFilter) -> Result<Vec<JournalEntry>> {
        let all = self.read_all()?;
        let filtered = all.into_iter().filter(|e| {
            if let Some(ref sym) = filter.symbol {
                if !e.symbol().eq_ignore_ascii_case(sym) {
                    return false;
                }
            }
            if let Some(is_p) = filter.is_paper {
                if e.is_paper() != is_p {
                    return false;
                }
            }
            if let Some(st) = filter.start_time {
                if e.timestamp() < st {
                    return false;
                }
            }
            if let Some(et) = filter.end_time {
                if e.timestamp() > et {
                    return false;
                }
            }
            if let Some(ref etype) = filter.event_type {
                let matches = match e {
                    JournalEntry::Intent(_) => etype.eq_ignore_ascii_case("INTENT"),
                    JournalEntry::OpenFill(_) => {
                        etype.eq_ignore_ascii_case("OPEN") || etype.eq_ignore_ascii_case("OPENFILL")
                    }
                    JournalEntry::Funding(_) => etype.eq_ignore_ascii_case("FUNDING"),
                    JournalEntry::RiskAlert(_) => {
                        etype.eq_ignore_ascii_case("RISK")
                            || etype.eq_ignore_ascii_case("RISKALERT")
                    }
                    JournalEntry::CloseFill(_) => {
                        etype.eq_ignore_ascii_case("CLOSE")
                            || etype.eq_ignore_ascii_case("CLOSEFILL")
                    }
                };
                if !matches {
                    return false;
                }
            }
            true
        });

        let mut results: Vec<JournalEntry> = filtered.collect();
        // Return latest entries first (newest on top)
        results.reverse();
        if let Some(limit) = filter.limit {
            if results.len() > limit {
                results.truncate(limit);
            }
        }
        Ok(results)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SymbolPerformance {
    pub symbol: String,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub total_volume_usd: f64,
    pub gross_funding_usd: f64,
    pub gross_basis_pnl_usd: f64,
    pub total_fees_usd: f64,
    pub net_pnl_usd: f64,
    pub total_duration_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceSummary {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate_pct: f64,
    pub total_turnover_usd: f64,
    pub total_gross_funding_usd: f64,
    pub total_basis_pnl_usd: f64,
    pub total_fees_paid_usd: f64,
    pub net_realized_pnl_usd: f64,
    pub net_return_on_capital_pct: f64,
    pub profit_factor: f64,
    pub max_drawdown_usd: f64,
    pub max_drawdown_pct: f64,
    pub avg_trade_duration_secs: u64,
    pub avg_net_profit_per_trade_usd: f64,
    pub avg_net_profit_bps: f64,
    pub total_funding_settlement_events: usize,
    pub symbol_breakdown: Vec<SymbolPerformance>,
}

pub struct PerformanceAnalytics;

impl PerformanceAnalytics {
    pub fn compute_from_entries(
        entries: &[JournalEntry],
        initial_capital_usd: f64,
    ) -> PerformanceSummary {
        let mut summary = PerformanceSummary::default();
        let mut symbol_map: HashMap<String, SymbolPerformance> = HashMap::new();
        let mut total_wins_usd = 0.0;
        let mut total_losses_usd = 0.0;
        let mut current_equity = initial_capital_usd;
        let mut peak_equity = initial_capital_usd;
        let mut max_dd_usd: f64 = 0.0;
        let mut max_dd_pct: f64 = 0.0;

        for entry in entries {
            match entry {
                JournalEntry::Funding(_f) => {
                    summary.total_funding_settlement_events += 1;
                }
                JournalEntry::CloseFill(c) => {
                    summary.total_trades += 1;
                    summary.total_turnover_usd += c.binance_exit_price * 2.0; // approximation
                    summary.total_gross_funding_usd += c.gross_funding_earned_usd;
                    summary.total_basis_pnl_usd += c.gross_basis_pnl_usd;
                    summary.total_fees_paid_usd += c.total_roundtrip_fees_usd;
                    summary.net_realized_pnl_usd += c.net_realized_pnl_usd;

                    if c.net_realized_pnl_usd >= 0.0 {
                        summary.winning_trades += 1;
                        total_wins_usd += c.net_realized_pnl_usd;
                    } else {
                        summary.losing_trades += 1;
                        total_losses_usd += c.net_realized_pnl_usd.abs();
                    }

                    current_equity += c.net_realized_pnl_usd;
                    if current_equity > peak_equity {
                        peak_equity = current_equity;
                    }
                    let dd_usd = peak_equity - current_equity;
                    let dd_pct = if peak_equity > 0.0 {
                        (dd_usd / peak_equity) * 100.0
                    } else {
                        0.0
                    };
                    if dd_usd > max_dd_usd {
                        max_dd_usd = dd_usd;
                    }
                    if dd_pct > max_dd_pct {
                        max_dd_pct = dd_pct;
                    }

                    let sym_perf =
                        symbol_map
                            .entry(c.symbol.clone())
                            .or_insert_with(|| SymbolPerformance {
                                symbol: c.symbol.clone(),
                                ..Default::default()
                            });
                    sym_perf.total_trades += 1;
                    if c.net_realized_pnl_usd >= 0.0 {
                        sym_perf.winning_trades += 1;
                    } else {
                        sym_perf.losing_trades += 1;
                    }
                    sym_perf.gross_funding_usd += c.gross_funding_earned_usd;
                    sym_perf.gross_basis_pnl_usd += c.gross_basis_pnl_usd;
                    sym_perf.total_fees_usd += c.total_roundtrip_fees_usd;
                    sym_perf.net_pnl_usd += c.net_realized_pnl_usd;
                    sym_perf.total_duration_secs += c.holding_duration_secs;
                }
                _ => {}
            }
        }

        if summary.total_trades > 0 {
            summary.win_rate_pct =
                (summary.winning_trades as f64 / summary.total_trades as f64) * 100.0;
            summary.avg_net_profit_per_trade_usd =
                summary.net_realized_pnl_usd / summary.total_trades as f64;
            summary.profit_factor = if total_losses_usd > 0.0 {
                total_wins_usd / total_losses_usd
            } else if total_wins_usd > 0.0 {
                999.0
            } else {
                1.0
            };
            if initial_capital_usd > 0.0 {
                summary.net_return_on_capital_pct =
                    (summary.net_realized_pnl_usd / initial_capital_usd) * 100.0;
            }
            summary.max_drawdown_usd = max_dd_usd;
            summary.max_drawdown_pct = max_dd_pct;
        }

        let mut sym_list: Vec<SymbolPerformance> = symbol_map.into_values().collect();
        sym_list.sort_unstable_by(|a, b| b.net_pnl_usd.total_cmp(&a.net_pnl_usd));
        summary.symbol_breakdown = sym_list;

        summary
    }

    pub fn render_console_summary(summary: &PerformanceSummary) {
        println!("\n{}", "=".repeat(105));
        println!("📊 BHyper Quantitative Performance & Trade Review Audit Report");
        println!("{}", "=".repeat(105));
        println!(
            "{:<32} {:<20} {:<32} {:<20}",
            "• Total Closed Trades:",
            format!("{}", summary.total_trades),
            "• Win Rate:",
            format!(
                "{:.1}% ({}/{} wins)",
                summary.win_rate_pct, summary.winning_trades, summary.total_trades
            )
        );
        println!(
            "{:<32} {:<20} {:<32} {:<20}",
            "• Net Realized PnL:",
            format!("${:.4}", summary.net_realized_pnl_usd),
            "• Profit Factor:",
            format!("{:.2}", summary.profit_factor)
        );
        println!(
            "{:<32} {:<20} {:<32} {:<20}",
            "• Gross Funding Income:",
            format!("${:.4}", summary.total_gross_funding_usd),
            "• Gross Basis PnL:",
            format!("${:.4}", summary.total_basis_pnl_usd)
        );
        println!(
            "{:<32} {:<20} {:<32} {:<20}",
            "• Total Trading Fees Paid:",
            format!("${:.4}", summary.total_fees_paid_usd),
            "• Return on Capital (ROC):",
            format!("{:.2}%", summary.net_return_on_capital_pct)
        );
        println!(
            "{:<32} {:<20} {:<32} {:<20}",
            "• Max Drawdown ($):",
            format!("${:.4}", summary.max_drawdown_usd),
            "• Max Drawdown (%):",
            format!("{:.2}%", summary.max_drawdown_pct)
        );
        println!(
            "{:<32} {:<20} {:<32} {:<20}",
            "• Avg Return / Trade:",
            format!("${:.4}", summary.avg_net_profit_per_trade_usd),
            "• Funding Settlement Ticks:",
            format!("{}", summary.total_funding_settlement_events)
        );

        if !summary.symbol_breakdown.is_empty() {
            println!("\n{}", "-".repeat(105));
            println!("📈 Symbol-by-Symbol PnL Attribution Breakdown");
            println!("{}", "-".repeat(105));
            println!(
                "{:<10} {:<8} {:<10} {:<15} {:<15} {:<15} {:<15}",
                "Symbol",
                "Trades",
                "Win Rate",
                "Gross Funding",
                "Basis PnL",
                "Total Fees",
                "Net PnL"
            );
            println!("{}", "-".repeat(105));

            for s in &summary.symbol_breakdown {
                let win_rate = if s.total_trades > 0 {
                    (s.winning_trades as f64 / s.total_trades as f64) * 100.0
                } else {
                    0.0
                };
                println!(
                    "{:<10} {:<8} {:>8.1}% ${:>13.4} ${:>13.4} ${:>13.4} ${:>13.4}",
                    s.symbol,
                    s.total_trades,
                    win_rate,
                    s.gross_funding_usd,
                    s.gross_basis_pnl_usd,
                    s.total_fees_usd,
                    s.net_pnl_usd
                );
            }
        }
        println!("{}\n", "=".repeat(105));
    }

    pub fn render_markdown_report(summary: &PerformanceSummary) -> String {
        let mut md = String::with_capacity(2048);
        md.push_str("# 📊 BHyper Quantitative Performance & Trade Journal Review\n\n");
        md.push_str(&format!(
            "*Generated at: `{}` UTC*\n\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S")
        ));
        md.push_str("## 1. Executive Summary\n\n");
        md.push_str("| Metric | Value | Metric | Value |\n");
        md.push_str("| :--- | :--- | :--- | :--- |\n");
        md.push_str(&format!(
            "| **Total Trades** | `{}` | **Win Rate** | `{:.1}%` ({}/{} wins) |\n",
            summary.total_trades,
            summary.win_rate_pct,
            summary.winning_trades,
            summary.total_trades
        ));
        md.push_str(&format!(
            "| **Net Realized PnL** | **`${:.4}`** | **Profit Factor** | `{:.2}` |\n",
            summary.net_realized_pnl_usd, summary.profit_factor
        ));
        md.push_str(&format!(
            "| **Gross Funding Income** | `${:.4}` | **Gross Basis PnL** | `${:.4}` |\n",
            summary.total_gross_funding_usd, summary.total_basis_pnl_usd
        ));
        md.push_str(&format!(
            "| **Total Trading Fees** | `${:.4}` | **Return on Capital (ROC)** | `{:.2}%` |\n",
            summary.total_fees_paid_usd, summary.net_return_on_capital_pct
        ));
        md.push_str(&format!(
            "| **Max Drawdown ($)** | `${:.4}` | **Max Drawdown (%)** | `{:.2}%` |\n",
            summary.max_drawdown_usd, summary.max_drawdown_pct
        ));
        md.push_str(&format!(
            "| **Avg Profit / Trade** | `${:.4}` | **Funding Settlements** | `{}` |\n\n",
            summary.avg_net_profit_per_trade_usd, summary.total_funding_settlement_events
        ));

        if !summary.symbol_breakdown.is_empty() {
            md.push_str("## 2. Symbol PnL Attribution Breakdown\n\n");
            md.push_str("| Symbol | Trades | Win Rate | Gross Funding ($) | Basis PnL ($) | Total Fees ($) | Net Realized PnL ($) |\n");
            md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");
            for s in &summary.symbol_breakdown {
                let win_rate = if s.total_trades > 0 {
                    (s.winning_trades as f64 / s.total_trades as f64) * 100.0
                } else {
                    0.0
                };
                md.push_str(&format!(
                    "| `{}` | `{}` | `{:.1}%` | `${:.4}` | `${:.4}` | `${:.4}` | **`${:.4}`** |\n",
                    s.symbol,
                    s.total_trades,
                    win_rate,
                    s.gross_funding_usd,
                    s.gross_basis_pnl_usd,
                    s.total_fees_usd,
                    s.net_pnl_usd
                ));
            }
            md.push('\n');
        }

        md
    }
}

use crate::journal::{JournalEntry, JournalFilter, TradeJournal};
use anyhow::Result;

pub fn run(
    symbol: Option<String>,
    event_type: Option<String>,
    limit: usize,
    paper_only: bool,
) -> Result<()> {
    let journal = TradeJournal::new(None);
    let filter = JournalFilter {
        symbol,
        event_type,
        limit: Some(limit),
        is_paper: if paper_only { Some(true) } else { None },
        ..Default::default()
    };

    let entries = journal.query(&filter)?;
    println!("\n{}", "=".repeat(125));
    println!("📖 BHyper Detailed Trade Execution Journal (Chronological Ledger)");
    println!("{}", "=".repeat(125));
    println!(
        "{:<20} {:<10} {:<8} {:<10} {:<45} {:<18}",
        "Timestamp (UTC)", "Event", "Symbol", "Mode", "Details / Execution", "PnL / Value"
    );
    println!("{}", "-".repeat(125));

    if entries.is_empty() {
        println!("  (No journal entries matching query filter found)");
    } else {
        for e in entries {
            let mode_tag = if e.is_paper() {
                "🧪 PAPER"
            } else {
                "⚡ LIVE"
            };
            let t_str = e.timestamp().format("%Y-%m-%d %H:%M:%S").to_string();

            match e {
                JournalEntry::Intent(i) => {
                    println!(
                        "{:<20} {:<10} {:<8} {:<10} {:<45} ${:<17.2}",
                        t_str,
                        "INTENT",
                        i.symbol,
                        mode_tag,
                        format!(
                            "Spread: {:.1}% | HL:{} BN:{}",
                            i.net_spread_apr_pct, i.hyperliquid_side, i.binance_side
                        ),
                        i.target_notional_usd
                    );
                }
                JournalEntry::OpenFill(o) => {
                    println!(
                        "{:<20} {:<10} {:<8} {:<10} {:<45} ${:<17.2}",
                        t_str,
                        "OPEN_FILL",
                        o.symbol,
                        mode_tag,
                        format!(
                            "HL: ${:.4} | BN: ${:.4} (Fee: ${:.4})",
                            o.hyperliquid_price, o.binance_price, o.total_open_fees_usd
                        ),
                        o.total_notional_usd
                    );
                }
                JournalEntry::Funding(f) => {
                    println!(
                        "{:<20} {:<10} {:<8} {:<10} {:<45} ${:<17.4}",
                        t_str,
                        "FUNDING",
                        f.symbol,
                        mode_tag,
                        format!(
                            "{}: {:.2} bps | Cum: ${:.4}",
                            f.exchange, f.rate_bps, f.cumulative_funding_usd
                        ),
                        f.funding_payment_usd
                    );
                }
                JournalEntry::RiskAlert(r) => {
                    println!(
                        "{:<20} {:<10} {:<8} {:<10} {:<45} {:<18}",
                        t_str,
                        "RISK",
                        r.symbol,
                        mode_tag,
                        format!("{}: {}", r.event_type, r.details),
                        r.action_taken
                    );
                }
                JournalEntry::CloseFill(c) => {
                    println!(
                        "{:<20} {:<10} {:<8} {:<10} {:<45} ${:<17.4}",
                        t_str,
                        "CLOSE_FILL",
                        c.symbol,
                        mode_tag,
                        format!(
                            "Held: {:.1}h | Funding: ${:.4} | Fees: ${:.4}",
                            c.holding_duration_secs as f64 / 3600.0,
                            c.gross_funding_earned_usd,
                            c.total_roundtrip_fees_usd
                        ),
                        c.net_realized_pnl_usd
                    );
                }
            }
        }
    }
    println!("{}\n", "=".repeat(125));
    Ok(())
}

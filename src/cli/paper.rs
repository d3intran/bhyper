use crate::binance::BinanceFuturesClient;
use crate::config::Config;
use crate::hyperliquid::HyperliquidClient;
use crate::paper::{self, PaperExecutionEngine, PaperTradingStore};
use crate::risk::{ExitSignal, RiskSentinel};
use crate::strategy::precision::LotPrecisionMatcher;
use crate::strategy::trigger::ProfitTriggerEngine;
use crate::strategy::ArbitrageScanner;
use crate::types::ExecutionMode;
use crate::ws::{BinanceWsStream, HyperliquidWsStream, MarketDataCache};
use anyhow::Result;
use tracing::{info, warn};

pub async fn run_daemon(
    config: &Config,
    initial_capital: f64,
    margin_usd: f64,
    taker_taker: bool,
    interval_secs: u64,
) -> Result<()> {
    let execution_mode = if taker_taker {
        ExecutionMode::TakerTaker
    } else {
        ExecutionMode::MakerTaker
    };

    let paper_store = PaperTradingStore::load_or_create(None, initial_capital)?;
    let mut engine = PaperExecutionEngine::new(paper_store);
    let risk_sentinel = RiskSentinel::new(config.risk.clone());

    let trigger_engine = ProfitTriggerEngine::new(
        config.strategy.min_open_apr_pct / 8760.0 * 100.0,
        config.strategy.max_position_usd_per_pair.min(margin_usd),
        config.strategy.maker_taker_mode,
    )
    .with_dual_horizon(
        config.strategy.dual_horizon_mode,
        config.strategy.min_carry_apr_pct,
    )
    .with_liquidity_guards(
        config.strategy.min_open_interest_usd,
        config.strategy.min_24h_volume_usd,
        config.strategy.max_bid_ask_spread_bps,
        config.strategy.max_oracle_mark_divergence_pct,
        config.strategy.symbol_whitelist.clone(),
        config.strategy.symbol_blacklist.clone(),
    );

    let cache = MarketDataCache::new();
    BinanceWsStream::spawn(cache.clone());
    let hl_ws_url = if config.hyperliquid.is_testnet {
        Some("wss://api.hyperliquid-testnet.xyz/ws".to_string())
    } else {
        Some("wss://api.hyperliquid.xyz/ws".to_string())
    };
    HyperliquidWsStream::spawn(
        cache.clone(),
        hl_ws_url,
        Some(config.hyperliquid.wallet_address.clone()),
    );

    info!("⏳ Warming up WebSocket feeds for Paper Trading (2s)...");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

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
    let scanner = ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode)
        .with_cache(cache.clone());

    info!(
        "🧪 [PAPER TRADING DAEMON] Running with ${:.2} Virtual Capital (${:.2} BN | ${:.2} HL)",
        engine.store.state.wallet.total_equity_usd(),
        engine.store.state.wallet.binance.total_equity_usd(),
        engine.store.state.wallet.hyperliquid.total_equity_usd()
    );

    let mut timer = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
    loop {
        timer.tick().await;

        let (opps_res, precisions_res) = tokio::join!(
            scanner.scan_opportunities(),
            scanner.fetch_symbol_precisions()
        );

        let opps = match opps_res {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("Error scanning opportunities in paper mode: {:?}", e);
                continue;
            }
        };

        let precisions = precisions_res.unwrap_or_default();
        let opps_map: std::collections::HashMap<String, &crate::types::ArbitrageOpportunity> =
            opps.iter().map(|o| (o.symbol.clone(), o)).collect();

        // 1. Funding Fee Accrual Clock Tick
        let _ = engine.accrue_funding_payments(&opps);

        // 2. Active Positions Exit Audit
        let active_syms: Vec<String> = engine
            .store
            .state
            .active_positions
            .keys()
            .cloned()
            .collect();
        for sym in active_syms {
            let pos = match engine.store.state.active_positions.get(&sym) {
                Some(p) => p.clone(),
                None => continue,
            };

            let (live_bn_px, live_hl_px) = cache
                .get_latest_prices(&pos.symbol)
                .unwrap_or((pos.binance_entry_price, pos.hyperliquid_entry_price));

            let current_opp = opps_map.get(&pos.symbol).copied();
            let active_pos_struct = pos.to_active_position();

            let exit_signal = risk_sentinel.evaluate_position_exit(
                &active_pos_struct,
                current_opp,
                live_bn_px,
                live_hl_px,
            );

            match exit_signal {
                ExitSignal::Hold => {}
                ExitSignal::SpreadDecay { reason, .. }
                | ExitSignal::SpreadInverted { reason, .. }
                | ExitSignal::BasisStopLoss { reason, .. }
                | ExitSignal::BasisTakeProfit { reason, .. }
                | ExitSignal::MaxDurationExceeded { reason, .. }
                | ExitSignal::DeltaDriftCritical { reason, .. }
                | ExitSignal::MarginCritical { reason, .. }
                | ExitSignal::LiquidationThreat { reason, .. } => {
                    warn!(
                        "🚨 [PAPER AUTO EXIT] Closing position on {}: {}",
                        sym, reason
                    );
                    let _ = engine.simulate_close(&sym, live_bn_px, live_hl_px, &reason);
                }
            }
        }

        // 3. New Arbitrage Opportunity Evaluation
        let current_active_count = engine.store.state.active_positions.len();
        if current_active_count < config.strategy.max_active_positions {
            let held_symbols: std::collections::HashSet<String> = engine
                .store
                .state
                .active_positions
                .keys()
                .cloned()
                .collect();

            let mut sorted_opps = opps.clone();
            sorted_opps.sort_by(|a, b| {
                let score_a = (a.net_spread_apr_pct / a.bid_ask_spread_bps.max(1.0))
                    * ((a.binance_volume_24h_usd + 1.0).ln());
                let score_b = (b.net_spread_apr_pct / b.bid_ask_spread_bps.max(1.0))
                    * ((b.binance_volume_24h_usd + 1.0).ln());
                score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
            });

            for opp in &sorted_opps {
                if held_symbols.contains(&opp.symbol) {
                    continue;
                }

                let prec = match precisions.get(&opp.symbol) {
                    Some(p) => p,
                    None => continue,
                };

                let decision = trigger_engine.evaluate_opportunity(
                    opp,
                    margin_usd,
                    false,
                    Some(prec),
                );

                if decision.should_open {
                    info!(
                        "🎯 [PAPER TRIGGER HIT] Validated arbitrage opportunity on {}: Net APR {:.2}%, Est Profit: ${:.4}",
                        opp.symbol, opp.net_spread_apr_pct, decision.net_expected_profit_usd
                    );

                    if let Err(e) =
                        engine.simulate_open(opp, &decision, prec, execution_mode)
                    {
                        warn!("Failed to simulate open on {}: {:?}", opp.symbol, e);
                    }
                    break;
                }
            }
        }

        // 4. Print Status Dashboard
        let total_eq = engine.store.state.wallet.total_equity_usd();
        let realized_pnl = engine.store.state.wallet.total_realized_pnl_usd();
        let funding_inc = engine.store.state.wallet.total_funding_income_usd();
        let fees_paid = engine.store.state.wallet.total_fees_paid_usd();
        let active_count = engine.store.state.active_positions.len();

        println!(
            "🧪 [PAPER TRADING] Equity: ${:.2} (BN: ${:.2} | HL: ${:.2}) | Positions: {} | Realized PnL: ${:.4} | Funding: +${:.4} | Fees: -${:.4}",
            total_eq,
            engine.store.state.wallet.binance.total_equity_usd(),
            engine.store.state.wallet.hyperliquid.total_equity_usd(),
            active_count,
            realized_pnl,
            funding_inc,
            fees_paid
        );
    }
}

pub fn run_reset(initial_capital: f64) -> Result<()> {
    let mut store = paper::PaperTradingStore::load_or_create(None, initial_capital)?;
    store.reset(initial_capital)?;
    println!(
        "✅ Reset paper trading state successfully. Virtual capital set to ${:.2}.",
        initial_capital
    );
    Ok(())
}

pub async fn run_trade(
    config: &Config,
    symbol: &str,
    margin_usd: f64,
    action: &str,
) -> Result<()> {
    let sym_upper = symbol.to_ascii_uppercase();
    let paper_store = paper::PaperTradingStore::load_or_create(None, 100.0)?;
    let mut engine = paper::PaperExecutionEngine::new(paper_store);

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

    match action.to_ascii_lowercase().as_str() {
        "open" => {
            println!(
                "🔍 Fetching live market quote & precision for {}...",
                sym_upper
            );
            let (opps, precisions) = tokio::join!(
                scanner.scan_opportunities(),
                scanner.fetch_symbol_precisions()
            );
            let opps = opps?;
            let precisions = precisions?;

            let opp = match opps.iter().find(|o| o.symbol == sym_upper) {
                Some(o) => o,
                None => {
                    eprintln!(
                        "❌ Symbol {} not found in active arbitrage opportunities scan.",
                        sym_upper
                    );
                    return Ok(());
                }
            };

            let prec = match precisions.get(&sym_upper) {
                Some(p) => p,
                None => {
                    eprintln!("❌ Precision rules for {} not found.", sym_upper);
                    return Ok(());
                }
            };

            let trigger_engine = ProfitTriggerEngine::new(
                0.0,
                margin_usd,
                config.strategy.maker_taker_mode,
            )
            .with_liquidity_guards(
                0.0,
                0.0,
                100.0,
                100.0,
                vec![],
                vec![],
            );

            let mut decision = trigger_engine.evaluate_opportunity(
                opp,
                margin_usd,
                true,
                Some(prec),
            );

            if decision.aligned_quantity.is_none() {
                let aligned = LotPrecisionMatcher::calculate_aligned_quantity(
                    &opp.symbol,
                    opp.hyperliquid_mark_price,
                    margin_usd,
                    prec,
                );
                if aligned.is_aligned {
                    decision.aligned_quantity = Some(aligned);
                    decision.should_open = true;
                }
            }

            println!("\n{}", "=".repeat(100));
            println!(
                "🧪 [MANUAL PAPER TRADE] Executing Simulated Open on {}",
                sym_upper
            );
            println!("{}", "=".repeat(100));
            println!("• Target Margin:        ${:.2}", margin_usd);
            println!("• Spread APR:           {:.2}%", opp.net_spread_apr_pct);
            println!("• Hyperliquid Side:     {}", opp.hyperliquid_side);
            println!("• Binance Side:         {}", opp.binance_side);

            match engine.simulate_open(opp, &decision, prec, ExecutionMode::MakerTaker) {
                Ok(pos) => {
                    println!("\n✅ [SIMULATION SUCCESSFUL] Position Established:");
                    println!("• Aligned Qty:          {}", pos.hyperliquid_qty);
                    println!("• Nominal Value:        ${:.2}", pos.nominal_value_usd);
                    println!(
                        "• HL Entry Price:       ${:.4} (Fee: ${:.4})",
                        pos.hyperliquid_entry_price, pos.hyperliquid_entry_fee_usd
                    );
                    println!(
                        "• BN Entry Price:       ${:.4} (Fee: ${:.4})",
                        pos.binance_entry_price, pos.binance_entry_fee_usd
                    );
                    println!(
                        "• Virtual Equity:       ${:.2} (Free: ${:.2})",
                        engine.store.state.wallet.total_equity_usd(),
                        engine.store.state.wallet.total_equity_usd()
                            - engine.store.state.wallet.binance.allocated_margin_usd
                            - engine.store.state.wallet.hyperliquid.allocated_margin_usd
                    );
                    println!("\n💡 You can run `bhyper journal` to view the recorded TradeIntent & OpenFill ledger entries!");
                }
                Err(e) => {
                    eprintln!("❌ Failed to simulate open: {:?}", e);
                }
            }
            println!("{}\n", "=".repeat(100));
        }
        "close" => {
            if !engine.store.state.active_positions.contains_key(&sym_upper) {
                eprintln!("❌ No active paper position found for {}.", sym_upper);
                return Ok(());
            }

            println!("🔍 Fetching live exit market price for {}...", sym_upper);
            let opps = scanner.scan_opportunities().await?;
            let (live_bn_px, live_hl_px) = if let Some(opp) =
                opps.iter().find(|o| o.symbol == sym_upper)
            {
                (opp.binance_mark_price, opp.hyperliquid_mark_price)
            } else if let Some(pos) = engine.store.state.active_positions.get(&sym_upper) {
                (pos.binance_entry_price, pos.hyperliquid_entry_price)
            } else {
                (0.0, 0.0)
            };

            println!("\n{}", "=".repeat(100));
            println!(
                "🧪 [MANUAL PAPER TRADE] Executing Simulated Close on {}",
                sym_upper
            );
            println!("{}", "=".repeat(100));

            match engine.simulate_close(
                &sym_upper,
                live_bn_px,
                live_hl_px,
                "Manual paper close test",
            ) {
                Ok(Some(close_event)) => {
                    println!("\n✅ [SIMULATION SUCCESSFUL] Position Closed & Settled:");
                    println!("• Symbol:               {}", close_event.symbol);
                    println!(
                        "• Gross Basis PnL:      ${:.4}",
                        close_event.gross_basis_pnl_usd
                    );
                    println!(
                        "• Gross Funding Earned: ${:.4}",
                        close_event.gross_funding_earned_usd
                    );
                    println!(
                        "• Total Roundtrip Fees: ${:.4}",
                        close_event.total_roundtrip_fees_usd
                    );
                    println!(
                        "• Net Realized PnL:     ${:.4} ({:.2} bps)",
                        close_event.net_realized_pnl_usd, close_event.net_return_bps
                    );
                    println!(
                        "• Updated Total Equity: ${:.2}",
                        engine.store.state.wallet.total_equity_usd()
                    );
                    println!("\n💡 Run `bhyper report` to view updated comprehensive performance & win-rate metrics!");
                }
                Ok(None) => {
                    println!("⚠️ Position was already closed.");
                }
                Err(e) => {
                    eprintln!("❌ Failed to simulate close: {:?}", e);
                }
            }
            println!("{}\n", "=".repeat(100));
        }
        other => {
            eprintln!(
                "❌ Invalid action '{}'. Use --action open or --action close.",
                other
            );
        }
    }
    Ok(())
}

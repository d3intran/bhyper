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

    let allocator = crate::strategy::CapitalAllocator::new(
        config.strategy.dynamic_sizing_enabled,
        config.strategy.liquidation_safety_buffer_pct,
        config.strategy.leverage,
        config.strategy.max_single_position_cap_usd,
        config.strategy.max_active_positions,
    );

    let trigger_engine = ProfitTriggerEngine::new(
        config.strategy.min_open_apr_pct / 8760.0 * 100.0,
        config.strategy.max_single_position_cap_usd.max(250.0),
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

    let rotator = crate::strategy::OpportunityRotator::new(
        config.strategy.auto_rotation_enabled,
        config.strategy.min_swap_apr_delta_pct,
        config.strategy.min_swap_profit_usd,
        config.strategy.min_holding_mins_before_swap,
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

    let hl_cfg = config.hyperliquid.clone();
    let cache_seed = cache.clone();

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

    info!("🌱 Performing instant initial market cache seed for 200+ symbols...");
    if let Ok((meta, ctxs)) = hl_client.fetch_meta_and_contexts().await {
        let mut hl_rates = Vec::with_capacity(meta.universe.len());
        let mut ois = std::collections::HashMap::with_capacity(meta.universe.len());
        for (u, ctx) in meta.universe.iter().zip(ctxs.iter()) {
            let sym = u.name.to_ascii_uppercase();
            let mark_p = ctx.mark_px.parse::<f64>().unwrap_or(0.0);
            let oracle_p = ctx
                .oracle_px
                .as_deref()
                .and_then(|p| p.parse::<f64>().ok())
                .unwrap_or(mark_p);
            let rate_1h = ctx.funding.parse::<f64>().unwrap_or(0.0);
            let apr = rate_1h * 8760.0 * 100.0;
            let oi = ctx.open_interest.parse::<f64>().unwrap_or(0.0) * mark_p;
            ois.insert(sym.clone(), oi);
            hl_rates.push(crate::types::FundingRateInfo {
                symbol: sym,
                exchange: crate::types::Exchange::Hyperliquid,
                mark_price: mark_p,
                index_price: oracle_p,
                funding_rate: rate_1h,
                funding_interval_hours: 1.0,
                annualized_apr_pct: apr,
                next_funding_time: Some(chrono::Utc::now()),
            });
        }
        cache_seed.update_hyperliquid_rates(hl_rates);
        cache_seed.update_metadata(
            std::collections::HashMap::new(),
            ois,
            std::collections::HashMap::new(),
        );
        info!(
            "✅ Initial Hyperliquid cache seeded with {} universe assets",
            meta.universe.len()
        );
    }

    if let Ok(vols) = bn_client.fetch_24h_volumes().await {
        cache_seed.update_metadata(
            vols,
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        );
    }

    // Spawn periodic background refresher
    tokio::spawn(async move {
        let hl_c =
            HyperliquidClient::new(hl_cfg.private_key, hl_cfg.wallet_address, hl_cfg.base_url);
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            interval.tick().await;
            if let Ok((meta, ctxs)) = hl_c.fetch_meta_and_contexts().await {
                let mut hl_rates = Vec::with_capacity(meta.universe.len());
                let mut ois = std::collections::HashMap::with_capacity(meta.universe.len());
                for (u, ctx) in meta.universe.iter().zip(ctxs.iter()) {
                    let sym = u.name.to_ascii_uppercase();
                    let mark_p = ctx.mark_px.parse::<f64>().unwrap_or(0.0);
                    let oracle_p = ctx
                        .oracle_px
                        .as_deref()
                        .and_then(|p| p.parse::<f64>().ok())
                        .unwrap_or(mark_p);
                    let rate_1h = ctx.funding.parse::<f64>().unwrap_or(0.0);
                    let apr = rate_1h * 8760.0 * 100.0;
                    let oi = ctx.open_interest.parse::<f64>().unwrap_or(0.0) * mark_p;
                    ois.insert(sym.clone(), oi);
                    hl_rates.push(crate::types::FundingRateInfo {
                        symbol: sym,
                        exchange: crate::types::Exchange::Hyperliquid,
                        mark_price: mark_p,
                        index_price: oracle_p,
                        funding_rate: rate_1h,
                        funding_interval_hours: 1.0,
                        annualized_apr_pct: apr,
                        next_funding_time: Some(chrono::Utc::now()),
                    });
                }
                cache_seed.update_hyperliquid_rates(hl_rates);
                cache_seed.update_metadata(
                    std::collections::HashMap::new(),
                    ois,
                    std::collections::HashMap::new(),
                );
            }
        }
    });

    info!("⏳ Warming up WebSocket feeds for Paper Trading (2s)...");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let scanner = ArbitrageScanner::new(
        bn_client.clone(),
        hl_client.clone(),
        config.strategy.maker_taker_mode,
    )
    .with_cache(cache.clone());

    info!("📊 Ingesting exchange precision rules (StepSize, TickSize, AssetIndex)...");
    let precisions_map = scanner.fetch_symbol_precisions().await.unwrap_or_default();
    info!(
        "✅ Cached precision rules for {} shared cross-exchange symbols",
        precisions_map.len()
    );
    let precisions_arc = std::sync::Arc::new(parking_lot::RwLock::new(precisions_map));

    let precisions_bg = precisions_arc.clone();
    let scanner_bg = scanner.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            if let Ok(updated_prec) = scanner_bg.fetch_symbol_precisions().await {
                *precisions_bg.write() = updated_prec;
            }
        }
    });

    info!(
        "🧪 [PAPER TRADING DAEMON] Running with ${:.2} Virtual Capital (${:.2} BN | ${:.2} HL)",
        engine.store.state.wallet.total_equity_usd(),
        engine.store.state.wallet.binance.total_equity_usd(),
        engine.store.state.wallet.hyperliquid.total_equity_usd()
    );

    let mut timer = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
    loop {
        timer.tick().await;

        let opps = match scanner.scan_opportunities().await {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("Error scanning opportunities in paper mode: {:?}", e);
                continue;
            }
        };

        let precisions = precisions_arc.read();
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

        // 3. 全局机会成本动态换仓 (Dynamic Opportunity Cost Swap)
        if config.strategy.auto_rotation_enabled {
            let active_vec: Vec<crate::types::ActiveArbitragePosition> = engine
                .store
                .state
                .active_positions
                .values()
                .map(|p| p.to_active_position())
                .collect();

            if let Some(swap) = rotator.evaluate_swaps(&active_vec, &opps, &opps_map) {
                if let Some(prec) = precisions.get(&swap.candidate_symbol) {
                    let bn_eq = engine.store.state.wallet.binance.total_equity_usd();
                    let hl_eq = engine.store.state.wallet.hyperliquid.total_equity_usd();
                    let active_notional: f64 = engine
                        .store
                        .state
                        .active_positions
                        .values()
                        .map(|p| p.nominal_value_usd)
                        .sum();
                    let active_cnt = engine.store.state.active_positions.len();
                    let unwind_notional = engine
                        .store
                        .state
                        .active_positions
                        .get(&swap.unwind_symbol)
                        .map(|p| p.nominal_value_usd)
                        .unwrap_or(0.0);

                    let alloc_d = allocator.calculate_slot_allocation(
                        bn_eq,
                        hl_eq,
                        (active_notional - unwind_notional).max(0.0),
                        active_cnt.saturating_sub(1),
                        Some(&swap.candidate_opp),
                    );
                    let target_notional = if alloc_d.is_safe {
                        alloc_d.target_notional_usd
                    } else {
                        margin_usd
                    };

                    let decision = trigger_engine.evaluate_opportunity(
                        &swap.candidate_opp,
                        target_notional,
                        true,
                        Some(prec),
                    );

                    if decision.should_open {
                        info!(
                            "🔄 [OPPORTUNITY SWAP] Rotating capital: Close {} ({:.1}% APR) -> Open {} ({:.1}% APR), Projected Net Gain: +${:.4}",
                            swap.unwind_symbol, swap.unwind_current_apr, swap.candidate_symbol, swap.candidate_opp.net_spread_apr_pct, swap.est_switching_gain_usd
                        );

                        let (live_bn_px, live_hl_px) = cache
                            .get_latest_prices(&swap.unwind_symbol)
                            .unwrap_or((0.0, 0.0));

                        let _ = engine.simulate_close(
                            &swap.unwind_symbol,
                            live_bn_px,
                            live_hl_px,
                            &swap.rationale,
                        );

                        if let Err(e) = engine.simulate_open(
                            &swap.candidate_opp,
                            &decision,
                            prec,
                            execution_mode,
                        ) {
                            warn!(
                                "Failed to simulate swap open on {}: {:?}",
                                swap.candidate_symbol, e
                            );
                        }
                    } else if let Some(ref reason) = decision.reject_reason {
                        tracing::debug!(
                            "Swap candidate {} rejected by trigger: {}",
                            swap.candidate_symbol,
                            reason
                        );
                    }
                }
            }
        }

        // 3.5 新套利标的开仓评估 (New Opportunity Evaluation)
        let current_active_count = engine.store.state.active_positions.len();
        if current_active_count < config.strategy.max_active_positions {
            let held_symbols: std::collections::HashSet<String> = engine
                .store
                .state
                .active_positions
                .keys()
                .cloned()
                .collect();

            let bn_eq = engine.store.state.wallet.binance.total_equity_usd();
            let hl_eq = engine.store.state.wallet.hyperliquid.total_equity_usd();
            let active_notional: f64 = engine
                .store
                .state
                .active_positions
                .values()
                .map(|p| p.nominal_value_usd)
                .sum();

            let mut sorted_opps = opps.clone();
            sorted_opps.sort_by(|a, b| {
                let score_a = (a.net_spread_apr_pct / a.bid_ask_spread_bps.max(1.0))
                    * ((a.binance_volume_24h_usd + 1.0).ln());
                let score_b = (b.net_spread_apr_pct / b.bid_ask_spread_bps.max(1.0))
                    * ((b.binance_volume_24h_usd + 1.0).ln());
                score_b
                    .partial_cmp(&score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            for opp in &sorted_opps {
                if held_symbols.contains(&opp.symbol) {
                    continue;
                }

                let prec = match precisions.get(&opp.symbol) {
                    Some(p) => p,
                    None => continue,
                };

                let alloc_d = allocator.calculate_slot_allocation(
                    bn_eq,
                    hl_eq,
                    active_notional,
                    current_active_count,
                    Some(opp),
                );

                if !alloc_d.is_safe {
                    continue;
                }

                let target_notional = alloc_d.target_notional_usd;
                let decision =
                    trigger_engine.evaluate_opportunity(opp, target_notional, false, Some(prec));

                if decision.should_open {
                    info!(
                        "🎯 [PAPER TRIGGER HIT] Validated arbitrage on {}: Net APR {:.2}%, Dynamic Target: ${:.2} (Lev {:.2}x), Est Profit: ${:.4}",
                        opp.symbol, opp.net_spread_apr_pct, target_notional, alloc_d.effective_leverage, decision.net_expected_profit_usd
                    );

                    if let Err(e) = engine.simulate_open(opp, &decision, prec, execution_mode) {
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
        let active_notional: f64 = engine
            .store
            .state
            .active_positions
            .values()
            .map(|p| p.nominal_value_usd)
            .sum();
        let effective_lev = if total_eq > 0.0 {
            active_notional / total_eq
        } else {
            0.0
        };
        let cap_util_pct = if total_eq > 0.0 {
            (active_notional / total_eq) * 100.0
        } else {
            0.0
        };

        println!(
            "🧪 [PAPER TRADING] Equity: ${:.2} (BN: ${:.2} | HL: ${:.2}) | Positions: {} (Notional: ${:.1}, Lev: {:.2}x, Util: {:.1}%) | Realized PnL: ${:.4} | Funding: +${:.4} | Fees: -${:.4}",
            total_eq,
            engine.store.state.wallet.binance.total_equity_usd(),
            engine.store.state.wallet.hyperliquid.total_equity_usd(),
            active_count,
            active_notional,
            effective_lev,
            cap_util_pct,
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

pub async fn run_trade(config: &Config, symbol: &str, margin_usd: f64, action: &str) -> Result<()> {
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

            let trigger_engine =
                ProfitTriggerEngine::new(0.0, margin_usd, config.strategy.maker_taker_mode)
                    .with_liquidity_guards(0.0, 0.0, 100.0, 100.0, vec![], vec![]);

            let mut decision =
                trigger_engine.evaluate_opportunity(opp, margin_usd, true, Some(prec));

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
            let (live_bn_px, live_hl_px) =
                if let Some(opp) = opps.iter().find(|o| o.symbol == sym_upper) {
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

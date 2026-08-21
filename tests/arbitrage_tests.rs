use bhyper::hyperliquid::signing::{
    ExchangeAction, HyperliquidSigner, LimitWire, OrderTypeWire, OrderWire,
};
use bhyper::state::StateStore;
use bhyper::strategy::{LotPrecisionMatcher, ProfitTriggerEngine};
use bhyper::types::{
    ActiveArbitragePosition, ArbitrageOpportunity, Exchange, FundingRateInfo, PositionSide,
    SymbolPrecisionInfo,
};
use bhyper::ws::MarketDataCache;
use chrono::Utc;

#[test]
fn test_apr_normalization_math() {
    // 8h rate 0.01% -> APR = 0.0001 * 1095 * 100 = 10.95%
    let bn_rate_8h: f64 = 0.0001;
    let bn_apr: f64 = bn_rate_8h * 1095.0 * 100.0;
    assert!((bn_apr - 10.95_f64).abs() < 1e-6);

    // 1h rate 0.005% -> APR = 0.00005 * 8760 * 100 = 43.8%
    let hl_rate_1h: f64 = 0.00005;
    let hl_apr: f64 = hl_rate_1h * 8760.0 * 100.0;
    assert!((hl_apr - 43.8_f64).abs() < 1e-6);
}

#[test]
fn test_precision_matcher_perfect_alignment() {
    let prec = SymbolPrecisionInfo {
        symbol: "SUI".to_string(),
        binance_step_size: 0.1,
        binance_tick_size: 0.0001,
        binance_min_qty: 0.1,
        binance_min_notional: 5.0,
        hyperliquid_sz_decimals: 1,
        hyperliquid_asset_index: 10,
        hyperliquid_min_notional: 10.0,
    };

    let mark_price = 3.25;
    let target_usd = 45.0;

    let aligned =
        LotPrecisionMatcher::calculate_aligned_quantity("SUI", mark_price, target_usd, &prec);

    assert!(aligned.is_aligned);
    assert_eq!(aligned.delta_imbalance_usd, 0.0);
    assert_eq!(aligned.delta_imbalance_pct, 0.0);
    assert!(aligned.notional_usd >= 12.0 && aligned.notional_usd <= 45.0);
    assert_eq!(
        aligned.binance_formatted_qty,
        aligned.hyperliquid_formatted_qty
    );
}

#[test]
fn test_precision_matcher_rejection_for_insufficient_notional() {
    let prec = SymbolPrecisionInfo {
        symbol: "SOL".to_string(),
        binance_step_size: 0.01,
        binance_tick_size: 0.01,
        binance_min_qty: 0.01,
        binance_min_notional: 5.0,
        hyperliquid_sz_decimals: 2,
        hyperliquid_asset_index: 2,
        hyperliquid_min_notional: 10.0,
    };

    let mark_price = 180.0;
    let target_usd = 5.0; // below minimum notional ($12)

    let aligned =
        LotPrecisionMatcher::calculate_aligned_quantity("SOL", mark_price, target_usd, &prec);

    assert!(!aligned.is_aligned);
    assert!(aligned
        .reject_reason
        .unwrap()
        .contains("低于两所最小名义面值"));
}

#[test]
fn test_trigger_engine_with_precision_lock() {
    let engine = ProfitTriggerEngine::default();
    let opp = ArbitrageOpportunity {
        symbol: "SAGA".into(),
        binance_mark_price: 0.016,
        hyperliquid_mark_price: 0.016,
        price_spread_pct: 0.0,
        binance_rate_8h_pct: 0.005,
        hyperliquid_rate_1h_pct: 0.095, // ~830% APR
        binance_apr_pct: 5.475,
        hyperliquid_apr_pct: 832.2,
        net_spread_apr_pct: 826.725,
        hyperliquid_side: PositionSide::Short,
        binance_side: PositionSide::Long,
        est_hourly_return_bps: 9.44,
        est_break_even_hours: 1.1,
        is_binance_settlement_next: false,
        projected_1h_net_bps: 8.3,
        projected_4h_net_bps: 26.0,
        projected_8h_net_bps: 63.8,
        binance_volume_24h_usd: 2_000_000.0,
        binance_open_interest_usd: 500_000.0,
        hyperliquid_open_interest_usd: 500_000.0,
        total_open_interest_usd: 1_000_000.0,
        bid_ask_spread_bps: 5.0,
        oracle_mark_divergence_pct: 0.05,
        is_liquid: true,
        liquidity_tier: "TIER_2_LIQUID".to_string(),
    };

    let prec = SymbolPrecisionInfo {
        symbol: "SAGA".to_string(),
        binance_step_size: 1.0,
        binance_tick_size: 0.00001,
        binance_min_qty: 1.0,
        binance_min_notional: 5.0,
        hyperliquid_sz_decimals: 0,
        hyperliquid_asset_index: 15,
        hyperliquid_min_notional: 10.0,
    };

    let decision = engine.evaluate_opportunity(&opp, 50.0, true, Some(&prec));
    assert!(decision.should_open);
    assert!(decision.net_expected_profit_bps > 0.0);
    assert!(decision.net_expected_profit_usd > 0.0);
    assert!(decision.aligned_quantity.is_some());
    assert!(decision.target_notional_usd >= 12.0 && decision.target_notional_usd <= 50.0);
}

#[test]
fn test_state_store_lifecycle_and_persistence() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "bhyper_test_{}",
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let state_file = tmp_dir.join("state.json");

    let mut store = StateStore::load_or_create(Some(state_file.clone())).unwrap();

    let pos = ActiveArbitragePosition {
        symbol: "BTC".to_string(),
        binance_side: PositionSide::Short,
        binance_qty: 0.001,
        binance_entry_price: 60000.0,
        hyperliquid_side: PositionSide::Long,
        hyperliquid_qty: 0.001,
        hyperliquid_entry_price: 60000.0,
        nominal_value_usd: 60.0,
        net_delta_usd: 0.0,
        entry_spread_apr: 45.0,
        current_spread_apr: 45.0,
        accumulated_funding_usd: 0.05,
        opened_at: Utc::now(),
        last_updated_at: Utc::now(),
        is_closed: false,
        closed_at: None,
        realized_pnl_usd: None,
    };

    store.upsert_position(pos).unwrap();
    assert_eq!(store.get_active_positions().len(), 1);

    // Reopen store from file
    let mut reopened_store = StateStore::load_or_create(Some(state_file.clone())).unwrap();
    assert_eq!(reopened_store.get_active_positions().len(), 1);

    // Close position
    let closed = reopened_store
        .close_position("BTC", 60100.0, 60100.0, 0.45, "Test close")
        .unwrap();
    assert!(closed.is_some());
    assert_eq!(reopened_store.get_active_positions().len(), 0);

    let _ = std::fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_market_cache_opportunity_computation() {
    let cache = MarketDataCache::new();

    cache.update_binance_rates(vec![FundingRateInfo {
        symbol: "ETH".to_string(),
        exchange: Exchange::Binance,
        mark_price: 3000.0,
        index_price: 3000.0,
        funding_rate: 0.0001,
        funding_interval_hours: 8.0,
        annualized_apr_pct: 10.95,
        next_funding_time: Some(Utc::now()),
    }]);

    cache.update_hyperliquid_rates(vec![FundingRateInfo {
        symbol: "ETH".to_string(),
        exchange: Exchange::Hyperliquid,
        mark_price: 3000.0,
        index_price: 3000.0,
        funding_rate: 0.0001, // 1h rate = 0.01% -> 87.6% APR
        funding_interval_hours: 1.0,
        annualized_apr_pct: 87.6,
        next_funding_time: Some(Utc::now()),
    }]);

    let opps = cache.compute_opportunities(12.0);
    assert_eq!(opps.len(), 1);
    assert_eq!(opps[0].symbol, "ETH");
    assert!(opps[0].net_spread_apr_pct > 70.0);
    assert_eq!(opps[0].hyperliquid_side, PositionSide::Short);
    assert_eq!(opps[0].binance_side, PositionSide::Long);
}

#[test]
fn test_hyperliquid_l1_order_signature() {
    let action = ExchangeAction::Order {
        orders: vec![OrderWire {
            a: 5,
            b: true,
            p: "2.45".to_string(),
            s: "10.0".to_string(),
            r: false,
            t: OrderTypeWire {
                limit: LimitWire {
                    tif: "Alo".to_string(),
                },
            },
        }],
        grouping: "na".to_string(),
    };

    let test_pk = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let nonce = 1710000000000;
    let sig = HyperliquidSigner::sign_l1_action(&action, nonce, test_pk, true).unwrap();

    assert_eq!(sig.r.len(), 66); // "0x" + 64 hex chars
    assert_eq!(sig.s.len(), 66);
    assert!(sig.v == 27 || sig.v == 28);
}

#[tokio::test]
async fn test_ws_fill_event_broadcast_instant() {
    let cache = MarketDataCache::new();
    let mut fill_rx = cache.subscribe_fills();

    let fill_event = bhyper::ws::market_cache::UserFillEvent {
        coin: "SUI".to_string(),
        px: 3.45,
        sz: 10.0,
        side: "B".to_string(),
        time: 1710000000000,
        fee: 0.001,
        oid: 998877,
        tid: 112233,
    };

    // Record fill on another thread / task
    let fill_clone = fill_event.clone();
    let cache_clone = cache.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        cache_clone.record_user_fill(fill_clone);
    });

    // Receive fill instantly
    let received = tokio::time::timeout(std::time::Duration::from_millis(500), fill_rx.recv())
        .await
        .expect("Timeout waiting for fill")
        .expect("Failed to receive fill");

    assert_eq!(received.coin, "SUI");
    assert_eq!(received.oid, 998877);
    assert_eq!(received.sz, 10.0);
    assert_eq!(received.px, 3.45);
}

#[test]
fn test_risk_sentinel_exit_signals_comprehensive() {
    use bhyper::config::RiskConfig;
    use bhyper::risk::{ExitSignal, RiskSentinel};

    let risk_config = RiskConfig {
        max_delta_drift_pct: 5.0,
        min_margin_ratio_pct: 20.0,
        max_total_notional_usd: 5000.0,
        auto_rebalance_delta: true,
        stop_loss_basis_bps: 40.0,
        max_holding_hours: 8.0,
        min_exit_apr_pct: 5.0,
        max_margin_utilization_pct: 75.0,
        min_liquidation_distance_pct: 20.0,
        rebalance_threshold_imbalance_pct: 30.0,
    };
    let sentinel = RiskSentinel::new(risk_config);

    let mut pos = ActiveArbitragePosition {
        symbol: "SUI".to_string(),
        binance_side: PositionSide::Long,
        binance_qty: 10.0,
        binance_entry_price: 3.0,
        hyperliquid_side: PositionSide::Short,
        hyperliquid_qty: 10.0,
        hyperliquid_entry_price: 3.0,
        nominal_value_usd: 30.0,
        net_delta_usd: 0.0,
        entry_spread_apr: 45.0,
        current_spread_apr: 45.0,
        accumulated_funding_usd: 0.02,
        opened_at: Utc::now(),
        last_updated_at: Utc::now(),
        is_closed: false,
        closed_at: None,
        realized_pnl_usd: None,
    };

    // Normal condition: should hold
    let signal = sentinel.evaluate_position_exit(&pos, None, 3.0, 3.0);
    assert_eq!(signal, ExitSignal::Hold);

    // Spread Inversion condition
    let inverted_opp = ArbitrageOpportunity {
        symbol: "SUI".to_string(),
        binance_mark_price: 3.0,
        hyperliquid_mark_price: 3.0,
        price_spread_pct: 0.0,
        binance_rate_8h_pct: 0.01,
        hyperliquid_rate_1h_pct: 0.0001,
        binance_apr_pct: 35.0,
        hyperliquid_apr_pct: 5.0,
        net_spread_apr_pct: 30.0,
        hyperliquid_side: PositionSide::Long,
        binance_side: PositionSide::Short,
        est_hourly_return_bps: 1.0,
        est_break_even_hours: 1.0,
        is_binance_settlement_next: false,
        projected_1h_net_bps: 1.0,
        projected_4h_net_bps: 1.0,
        projected_8h_net_bps: 1.0,
        binance_volume_24h_usd: 5_000_000.0,
        binance_open_interest_usd: 1_000_000.0,
        hyperliquid_open_interest_usd: 1_000_000.0,
        total_open_interest_usd: 2_000_000.0,
        bid_ask_spread_bps: 2.0,
        oracle_mark_divergence_pct: 0.01,
        is_liquid: true,
        liquidity_tier: "TIER_1_PRIME".to_string(),
    };
    // Position is HL Short (effective spread = HL APR - BN APR = 5 - 35 = -30%)
    let signal_inv = sentinel.evaluate_position_exit(&pos, Some(&inverted_opp), 3.0, 3.0);
    match signal_inv {
        ExitSignal::SpreadInverted { current_apr, .. } => {
            assert!(current_apr < 0.0);
        }
        _ => panic!("Expected SpreadInverted signal"),
    }

    // Basis stop loss condition: HL price pumped to $3.5 while BN price stayed at $3.0 (loss of $5 on $30 notional = -1666 bps)
    let signal_loss = sentinel.evaluate_position_exit(&pos, None, 3.0, 3.5);
    match signal_loss {
        ExitSignal::BasisStopLoss { basis_pnl_bps, .. } => {
            assert!(basis_pnl_bps < -40.0);
        }
        _ => panic!("Expected BasisStopLoss signal"),
    }

    // Max duration exceeded condition
    pos.opened_at = Utc::now() - chrono::Duration::hours(9);
    let signal_dur = sentinel.evaluate_position_exit(&pos, None, 3.0, 3.0);
    match signal_dur {
        ExitSignal::MaxDurationExceeded { holding_hours, .. } => {
            assert!(holding_hours >= 8.0);
        }
        _ => panic!("Expected MaxDurationExceeded signal"),
    }
}

#[test]
fn test_market_cache_mids_preserves_funding_rates() {
    let cache = MarketDataCache::new();

    // Seed full funding rate info
    cache.update_hyperliquid_rates(vec![FundingRateInfo {
        symbol: "SOL".to_string(),
        exchange: Exchange::Hyperliquid,
        mark_price: 150.0,
        index_price: 150.0,
        funding_rate: 0.0002, // 175.2% APR
        funding_interval_hours: 1.0,
        annualized_apr_pct: 175.2,
        next_funding_time: Some(Utc::now()),
    }]);

    // Update mids via WebSocket allMids
    let mut mids = std::collections::HashMap::new();
    mids.insert("SOL".to_string(), 152.5);
    cache.update_hyperliquid_mids(mids);

    assert!(cache.get_latest_prices("SOL").is_none()); // BN price not set yet

    cache.update_binance_rates(vec![FundingRateInfo {
        symbol: "SOL".to_string(),
        exchange: Exchange::Binance,
        mark_price: 152.0,
        index_price: 152.0,
        funding_rate: 0.0001,
        funding_interval_hours: 8.0,
        annualized_apr_pct: 10.95,
        next_funding_time: Some(Utc::now()),
    }]);

    let prices = cache.get_latest_prices("SOL").unwrap();
    assert_eq!(prices.0, 152.0); // BN price
    assert_eq!(prices.1, 152.5); // HL updated mid price

    // Verify funding rate was preserved (not zeroed out)
    let opp = cache.get_latest_opportunity("SOL", 12.0).unwrap();
    assert!((opp.hyperliquid_apr_pct - 175.2).abs() < 1e-3);
}

#[test]
fn test_liquidity_and_oi_filter_rejection() {
    let engine = ProfitTriggerEngine::default().with_liquidity_guards(
        1_000_000.0, // min $1M OI
        2_000_000.0, // min $2M 24h vol
        15.0,
        0.5,
        Vec::new(),
        Vec::new(),
    );

    let illiquid_opp = ArbitrageOpportunity {
        symbol: "LOWCAP".into(),
        binance_mark_price: 1.0,
        hyperliquid_mark_price: 1.0,
        price_spread_pct: 0.0,
        binance_rate_8h_pct: 0.001,
        hyperliquid_rate_1h_pct: 0.05, // High APR
        binance_apr_pct: 1.095,
        hyperliquid_apr_pct: 438.0,
        net_spread_apr_pct: 436.9,
        hyperliquid_side: PositionSide::Short,
        binance_side: PositionSide::Long,
        est_hourly_return_bps: 5.0,
        est_break_even_hours: 2.0,
        is_binance_settlement_next: false,
        projected_1h_net_bps: 4.0,
        projected_4h_net_bps: 18.0,
        projected_8h_net_bps: 38.0,
        binance_volume_24h_usd: 100_000.0, // Low volume!
        binance_open_interest_usd: 50_000.0,
        hyperliquid_open_interest_usd: 50_000.0,
        total_open_interest_usd: 100_000.0, // Low OI ($100k < $1M)
        bid_ask_spread_bps: 5.0,
        oracle_mark_divergence_pct: 0.02,
        is_liquid: false,
        liquidity_tier: "ILLIQUID_RISK".to_string(),
    };

    let prec = SymbolPrecisionInfo {
        symbol: "LOWCAP".to_string(),
        binance_step_size: 1.0,
        binance_tick_size: 0.001,
        binance_min_qty: 1.0,
        binance_min_notional: 5.0,
        hyperliquid_sz_decimals: 0,
        hyperliquid_asset_index: 20,
        hyperliquid_min_notional: 10.0,
    };

    let decision = engine.evaluate_opportunity(&illiquid_opp, 50.0, true, Some(&prec));
    assert!(!decision.should_open);
    assert!(decision.reject_reason.unwrap().contains("持仓量 OI"));
}

#[test]
fn test_rate_manipulation_divergence_lock() {
    let engine = ProfitTriggerEngine::default().with_liquidity_guards(
        100_000.0,
        100_000.0,
        15.0,
        0.5, // max 0.5% mark-oracle divergence
        Vec::new(),
        Vec::new(),
    );

    let manipulated_opp = ArbitrageOpportunity {
        symbol: "PUMP".into(),
        binance_mark_price: 10.0,
        hyperliquid_mark_price: 10.0,
        price_spread_pct: 0.0,
        binance_rate_8h_pct: 0.001,
        hyperliquid_rate_1h_pct: 0.05,
        binance_apr_pct: 1.095,
        hyperliquid_apr_pct: 438.0,
        net_spread_apr_pct: 436.9,
        hyperliquid_side: PositionSide::Short,
        binance_side: PositionSide::Long,
        est_hourly_return_bps: 5.0,
        est_break_even_hours: 2.0,
        is_binance_settlement_next: false,
        projected_1h_net_bps: 4.0,
        projected_4h_net_bps: 18.0,
        projected_8h_net_bps: 38.0,
        binance_volume_24h_usd: 5_000_000.0,
        binance_open_interest_usd: 1_000_000.0,
        hyperliquid_open_interest_usd: 1_000_000.0,
        total_open_interest_usd: 2_000_000.0,
        bid_ask_spread_bps: 5.0,
        oracle_mark_divergence_pct: 1.25, // 1.25% divergence (Manipulation warning!)
        is_liquid: true,
        liquidity_tier: "TIER_1_PRIME".to_string(),
    };

    let prec = SymbolPrecisionInfo {
        symbol: "PUMP".to_string(),
        binance_step_size: 0.1,
        binance_tick_size: 0.01,
        binance_min_qty: 0.1,
        binance_min_notional: 5.0,
        hyperliquid_sz_decimals: 1,
        hyperliquid_asset_index: 25,
        hyperliquid_min_notional: 10.0,
    };

    let decision = engine.evaluate_opportunity(&manipulated_opp, 50.0, true, Some(&prec));
    assert!(!decision.should_open);
    assert!(decision.reject_reason.unwrap().contains("标记价与预言机偏离"));
}

#[test]
fn test_cross_exchange_margin_health_and_rebalance_advisory() {
    let bn_health = bhyper::types::ExchangeMarginHealth {
        exchange: Exchange::Binance,
        account_value_usd: 90.0,
        total_margin_used_usd: 30.0,
        free_margin_usd: 60.0,
        margin_utilization_pct: 33.3,
        min_liquidation_distance_pct: 45.0,
        is_healthy: true,
    };

    let hl_health = bhyper::types::ExchangeMarginHealth {
        exchange: Exchange::Hyperliquid,
        account_value_usd: 10.0, // Depleted margin!
        total_margin_used_usd: 8.5,
        free_margin_usd: 1.5,
        margin_utilization_pct: 85.0, // High utilization!
        min_liquidation_distance_pct: 12.0, // Close to liq!
        is_healthy: false,
    };

    let assessment = StateStore::compute_rebalance_advisory(&bn_health, &hl_health, 30.0);
    assert!(assessment.rebalance_required);
    assert_eq!(assessment.total_equity_usd, 100.0);
    assert_eq!(assessment.suggested_transfer_usd, 40.0);
    assert!(assessment.transfer_direction.contains("Binance -> Hyperliquid"));

    // Check risk sentinel recognizes critical margin and liquidation threat
    let sentinel = bhyper::risk::RiskSentinel::new(bhyper::config::RiskConfig {
        max_margin_utilization_pct: 75.0,
        min_liquidation_distance_pct: 20.0,
        ..Default::default()
    });

    let exit_signal = sentinel.evaluate_margin_health(&assessment);
    match exit_signal {
        bhyper::risk::ExitSignal::MarginCritical { exchange, utilization_pct, .. } => {
            assert_eq!(exchange, "Hyperliquid");
            assert_eq!(utilization_pct, 85.0);
        }
        _ => panic!("Expected MarginCritical signal"),
    }
}

#[test]
fn test_paper_trading_virtual_wallet_margin_and_fees() {
    use bhyper::paper::wallet::PaperDualWallet;

    let mut wallet = PaperDualWallet::new(100.0);
    assert_eq!(wallet.total_equity_usd(), 100.0);
    assert_eq!(wallet.binance.free_margin_usd(), 50.0);
    assert_eq!(wallet.hyperliquid.free_margin_usd(), 50.0);

    // Lock margin for $30 trade ($15 on each exchange)
    assert!(wallet.can_allocate(15.0, 15.0).is_ok());
    wallet.binance.lock_margin(15.0).unwrap();
    wallet.hyperliquid.lock_margin(15.0).unwrap();
    wallet.binance.debit_fee(0.012); // Taker fee
    wallet.hyperliquid.debit_fee(0.000); // Maker fee

    assert_eq!(wallet.binance.free_margin_usd(), 34.988);
    assert_eq!(wallet.hyperliquid.free_margin_usd(), 35.0);

    // Apply funding payment (HL earned $0.05, BN paid $0.01)
    wallet.hyperliquid.apply_funding(0.05);
    wallet.binance.apply_funding(-0.01);
    assert_eq!(wallet.total_funding_income_usd(), 0.04);

    // Release margin on close with $0.10 profit on HL and -$0.02 loss on BN
    wallet.hyperliquid.release_margin(15.0, 0.10);
    wallet.binance.release_margin(15.0, -0.02);

    assert_eq!(wallet.binance.allocated_margin_usd, 0.0);
    assert_eq!(wallet.hyperliquid.allocated_margin_usd, 0.0);
    assert!(wallet.total_equity_usd() > 100.0);
}

#[test]
fn test_paper_execution_engine_open_accrual_and_close() {
    use bhyper::paper::engine::{PaperExecutionEngine, PaperTradingStore};
    use bhyper::strategy::trigger::ProfitTriggerEngine;

    let tmp_dir = std::env::temp_dir().join(format!(
        "bhyper_paper_test_{}",
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let state_file = tmp_dir.join("paper_state.json");
    let store = PaperTradingStore::load_or_create(Some(state_file), 100.0).unwrap();
    let mut engine = PaperExecutionEngine::new(store);

    let opp = ArbitrageOpportunity {
        symbol: "SUI".into(),
        binance_mark_price: 3.0,
        hyperliquid_mark_price: 3.0,
        price_spread_pct: 0.0,
        binance_rate_8h_pct: 0.01,
        hyperliquid_rate_1h_pct: 0.05, // 438% APR
        binance_apr_pct: 10.95,
        hyperliquid_apr_pct: 438.0,
        net_spread_apr_pct: 427.05,
        hyperliquid_side: PositionSide::Short,
        binance_side: PositionSide::Long,
        est_hourly_return_bps: 4.88,
        est_break_even_hours: 2.45,
        is_binance_settlement_next: false,
        projected_1h_net_bps: 4.0,
        projected_4h_net_bps: 18.0,
        projected_8h_net_bps: 38.0,
        binance_volume_24h_usd: 10_000_000.0,
        binance_open_interest_usd: 2_000_000.0,
        hyperliquid_open_interest_usd: 2_000_000.0,
        total_open_interest_usd: 4_000_000.0,
        bid_ask_spread_bps: 1.0,
        oracle_mark_divergence_pct: 0.02,
        is_liquid: true,
        liquidity_tier: "TIER_1_PRIME".to_string(),
    };

    let prec = SymbolPrecisionInfo {
        symbol: "SUI".to_string(),
        binance_step_size: 0.1,
        binance_tick_size: 0.001,
        binance_min_qty: 0.1,
        binance_min_notional: 5.0,
        hyperliquid_sz_decimals: 1,
        hyperliquid_asset_index: 5,
        hyperliquid_min_notional: 10.0,
    };

    let trigger = ProfitTriggerEngine::default();
    let decision = trigger.evaluate_opportunity(&opp, 50.0, true, Some(&prec));
    assert!(decision.should_open);

    // 1. Simulate Open
    let pos = engine
        .simulate_open(&opp, &decision, &prec, bhyper::types::ExecutionMode::MakerTaker)
        .unwrap();
    assert_eq!(pos.symbol, "SUI");
    assert_eq!(engine.store.state.active_positions.len(), 1);

    // 2. Simulate Funding Accrual
    // Artificially age the last HL funding time by 65 minutes
    if let Some(p) = engine.store.state.active_positions.get_mut("SUI") {
        p.last_hl_funding_time = Utc::now() - chrono::Duration::minutes(65);
    }
    let funding_events = engine.accrue_funding_payments(&[opp.clone()]).unwrap();
    assert_eq!(funding_events.len(), 1);
    assert!(funding_events[0].funding_payment_usd > 0.0);

    // 3. Simulate Close
    let close_event = engine
        .simulate_close("SUI", 3.01, 3.00, "Spread decay profit take")
        .unwrap()
        .unwrap();
    assert_eq!(close_event.symbol, "SUI");
    assert_eq!(engine.store.state.active_positions.len(), 0);
    assert!(close_event.gross_funding_earned_usd > 0.0);

    let _ = std::fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_trade_journal_persistence_and_filtering() {
    use bhyper::journal::{
        JournalEntry, JournalFilter, TradeCloseFillEvent, TradeIntentEvent, TradeJournal,
        TradeOpenFillEvent,
    };

    let tmp_dir = std::env::temp_dir().join(format!(
        "bhyper_journal_test_{}",
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let journal_file = tmp_dir.join("test_journal.jsonl");
    let journal = TradeJournal::new(Some(journal_file));

    let intent = TradeIntentEvent {
        id: "intent-1".into(),
        symbol: "BTC".into(),
        timestamp: Utc::now(),
        is_paper: true,
        hyperliquid_side: PositionSide::Short,
        binance_side: PositionSide::Long,
        hyperliquid_apr_pct: 120.0,
        binance_apr_pct: 10.0,
        net_spread_apr_pct: 110.0,
        projected_1h_net_bps: 5.0,
        projected_4h_net_bps: 20.0,
        target_notional_usd: 50.0,
        aligned_qty: 0.001,
        friction_cost_bps: 12.0,
        est_hourly_return_bps: 1.25,
        reason: "Test intent".into(),
    };
    journal.append(&JournalEntry::Intent(intent)).unwrap();

    let open = TradeOpenFillEvent {
        id: "fill-1".into(),
        intent_id: "intent-1".into(),
        symbol: "BTC".into(),
        timestamp: Utc::now(),
        is_paper: true,
        hyperliquid_side: PositionSide::Short,
        hyperliquid_qty: 0.001,
        hyperliquid_price: 60000.0,
        hyperliquid_fee_usd: 0.0,
        hyperliquid_mode: "MAKER".into(),
        binance_side: PositionSide::Long,
        binance_qty: 0.001,
        binance_price: 60000.0,
        binance_fee_usd: 0.024,
        binance_mode: "TAKER".into(),
        total_notional_usd: 60.0,
        entry_price_spread_bps: 0.0,
        total_open_fees_usd: 0.024,
        execution_latency_ms: 8,
    };
    journal.append(&JournalEntry::OpenFill(open)).unwrap();

    let close = TradeCloseFillEvent {
        id: "close-1".into(),
        open_trade_id: "fill-1".into(),
        symbol: "BTC".into(),
        timestamp: Utc::now(),
        is_paper: true,
        holding_duration_secs: 7200,
        exit_reason: "Target profit".into(),
        hyperliquid_exit_price: 60100.0,
        hyperliquid_exit_fee_usd: 0.021,
        binance_exit_price: 60100.0,
        binance_exit_fee_usd: 0.024,
        total_exit_fees_usd: 0.045,
        total_roundtrip_fees_usd: 0.069,
        gross_basis_pnl_usd: 0.0,
        gross_funding_earned_usd: 0.15,
        net_realized_pnl_usd: 0.081,
        net_return_bps: 13.5,
        return_on_capital_pct: 0.135,
    };
    journal.append(&JournalEntry::CloseFill(close)).unwrap();

    let all_entries = journal.read_all().unwrap();
    assert_eq!(all_entries.len(), 3);

    // Query filter by symbol
    let btc_entries = journal
        .query(&JournalFilter {
            symbol: Some("BTC".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(btc_entries.len(), 3);

    // Query filter by event type
    let close_entries = journal
        .query(&JournalFilter {
            event_type: Some("CLOSE".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(close_entries.len(), 1);

    let _ = std::fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_performance_analytics_pnl_attribution_and_summary() {
    use bhyper::journal::{PerformanceAnalytics, TradeCloseFillEvent, JournalEntry};

    let entries = vec![
        JournalEntry::CloseFill(TradeCloseFillEvent {
            id: "1".into(),
            open_trade_id: "1".into(),
            symbol: "ETH".into(),
            timestamp: Utc::now(),
            is_paper: true,
            holding_duration_secs: 3600,
            exit_reason: "Normal".into(),
            hyperliquid_exit_price: 3000.0,
            hyperliquid_exit_fee_usd: 0.01,
            binance_exit_price: 3000.0,
            binance_exit_fee_usd: 0.01,
            total_exit_fees_usd: 0.02,
            total_roundtrip_fees_usd: 0.04,
            gross_basis_pnl_usd: 0.0,
            gross_funding_earned_usd: 0.10,
            net_realized_pnl_usd: 0.06, // Win
            net_return_bps: 12.0,
            return_on_capital_pct: 0.12,
        }),
        JournalEntry::CloseFill(TradeCloseFillEvent {
            id: "2".into(),
            open_trade_id: "2".into(),
            symbol: "SOL".into(),
            timestamp: Utc::now(),
            is_paper: true,
            holding_duration_secs: 3600,
            exit_reason: "Basis loss".into(),
            hyperliquid_exit_price: 150.0,
            hyperliquid_exit_fee_usd: 0.01,
            binance_exit_price: 150.0,
            binance_exit_fee_usd: 0.01,
            total_exit_fees_usd: 0.02,
            total_roundtrip_fees_usd: 0.04,
            gross_basis_pnl_usd: -0.05,
            gross_funding_earned_usd: 0.02,
            net_realized_pnl_usd: -0.07, // Loss
            net_return_bps: -14.0,
            return_on_capital_pct: -0.14,
        }),
    ];

    let summary = PerformanceAnalytics::compute_from_entries(&entries, 100.0);
    assert_eq!(summary.total_trades, 2);
    assert_eq!(summary.winning_trades, 1);
    assert_eq!(summary.losing_trades, 1);
    assert_eq!(summary.win_rate_pct, 50.0);
    assert!((summary.net_realized_pnl_usd - (-0.01)).abs() < 1e-6);
    assert!((summary.total_gross_funding_usd - 0.12).abs() < 1e-6);
    assert_eq!(summary.symbol_breakdown.len(), 2);

    let md_report = PerformanceAnalytics::render_markdown_report(&summary);
    assert!(md_report.contains("Executive Summary"));
    assert!(md_report.contains("Win Rate"));
}


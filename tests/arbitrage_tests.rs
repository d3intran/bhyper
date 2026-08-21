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

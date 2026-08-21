use bhyper::strategy::ProfitTriggerEngine;
use bhyper::types::{ArbitrageOpportunity, PositionSide};

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
fn test_trigger_engine_high_price_rejection() {
    let engine = ProfitTriggerEngine::default();
    let opp = ArbitrageOpportunity {
        symbol: "BTC".into(),
        binance_mark_price: 95000.0, // High price coin
        hyperliquid_mark_price: 95050.0,
        price_spread_pct: 0.05,
        binance_rate_8h_pct: 0.01,
        hyperliquid_rate_1h_pct: 0.05,
        binance_apr_pct: 10.95,
        hyperliquid_apr_pct: 438.0,
        net_spread_apr_pct: 427.05,
        hyperliquid_side: PositionSide::Short,
        binance_side: PositionSide::Long,
        est_hourly_return_bps: 4.87,
        est_break_even_hours: 2.0,
    };

    let decision = engine.evaluate_opportunity(&opp, 50.0, true);
    // BTC mark price > $500 should be rejected for small capital safety
    assert!(!decision.should_open);
    assert!(decision.reject_reason.unwrap().contains("单价过高"));
}

#[test]
fn test_trigger_engine_lucrative_altcoin_acceptance() {
    let engine = ProfitTriggerEngine::default();
    let opp = ArbitrageOpportunity {
        symbol: "SAGA".into(),
        binance_mark_price: 0.016, // Low price altcoin
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
    };

    let decision = engine.evaluate_opportunity(&opp, 50.0, true);
    assert!(decision.should_open);
    assert!(decision.net_expected_profit_bps > 0.0);
    assert!(decision.target_notional_usd >= 12.0 && decision.target_notional_usd <= 50.0);
}

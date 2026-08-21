use bhyper::hyperliquid::signing::{
    ExchangeAction, HyperliquidSigner, LimitWire, OrderTypeWire, OrderWire,
};
use bhyper::strategy::{LotPrecisionMatcher, ProfitTriggerEngine};
use bhyper::types::{ArbitrageOpportunity, PositionSide, SymbolPrecisionInfo};

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
    assert_eq!(aligned.binance_formatted_qty, aligned.hyperliquid_formatted_qty);
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
    assert!(aligned.reject_reason.unwrap().contains("低于两所最小名义面值"));
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
    assert!(decision.aligned_quantity.is_some());
    assert!(decision.target_notional_usd >= 12.0 && decision.target_notional_usd <= 50.0);
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

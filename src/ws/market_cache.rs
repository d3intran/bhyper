use crate::types::{ArbitrageOpportunity, FundingRateInfo, PositionSide};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFillEvent {
    pub coin: String,
    pub px: f64,
    pub sz: f64,
    pub side: String,
    pub time: i64,
    pub fee: f64,
    pub oid: u64,
    pub tid: u64,
}

#[derive(Default)]
pub struct MarketDataCacheInner {
    pub binance_rates: HashMap<String, FundingRateInfo>,
    pub hyperliquid_rates: HashMap<String, FundingRateInfo>,
    pub last_binance_update: Option<DateTime<Utc>>,
    pub last_hyperliquid_update: Option<DateTime<Utc>>,
    pub user_fills: Vec<UserFillEvent>,
}

#[derive(Clone, Default)]
pub struct MarketDataCache {
    inner: Arc<RwLock<MarketDataCacheInner>>,
}

impl MarketDataCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(MarketDataCacheInner::default())),
        }
    }

    pub fn update_binance_rates(&self, rates: Vec<FundingRateInfo>) {
        let mut inner = self.inner.write();
        inner.last_binance_update = Some(Utc::now());
        for rate in rates {
            inner.binance_rates.insert(rate.symbol.to_uppercase(), rate);
        }
    }

    pub fn update_hyperliquid_rates(&self, rates: Vec<FundingRateInfo>) {
        let mut inner = self.inner.write();
        inner.last_hyperliquid_update = Some(Utc::now());
        for rate in rates {
            inner
                .hyperliquid_rates
                .insert(rate.symbol.to_uppercase(), rate);
        }
    }

    pub fn record_user_fill(&self, fill: UserFillEvent) {
        let mut inner = self.inner.write();
        inner.user_fills.push(fill);
        if inner.user_fills.len() > 1000 {
            inner.user_fills.drain(0..500);
        }
    }

    pub fn is_healthy(&self) -> bool {
        let inner = self.inner.read();
        let now = Utc::now();
        let bn_ok = inner
            .last_binance_update
            .map(|t| (now - t).num_seconds() < 10)
            .unwrap_or(false);
        let hl_ok = inner
            .last_hyperliquid_update
            .map(|t| (now - t).num_seconds() < 10)
            .unwrap_or(false);
        bn_ok && hl_ok
    }

    /// Compute ranked opportunities directly from memory
    pub fn compute_opportunities(
        &self,
        roundtrip_cost_bps: f64,
    ) -> Vec<ArbitrageOpportunity> {
        let inner = self.inner.read();
        let mut opps = Vec::with_capacity(inner.hyperliquid_rates.len());

        let now = Utc::now();
        let minute = chrono::Timelike::minute(&now);
        let hour = chrono::Timelike::hour(&now);
        let is_bn_settlement_next = (minute >= 50 && (hour == 7 || hour == 15 || hour == 23))
            || (minute <= 10 && (hour == 8 || hour == 16 || hour == 0));

        for (sym, hl_item) in &inner.hyperliquid_rates {
            if let Some(bn_item) = inner.binance_rates.get(sym) {
                if bn_item.mark_price <= 0.0 || hl_item.mark_price <= 0.0 {
                    continue;
                }

                let price_spread =
                    (hl_item.mark_price - bn_item.mark_price) / bn_item.mark_price * 100.0;
                let raw_spread = hl_item.annualized_apr_pct - bn_item.annualized_apr_pct;

                let (hl_side, bn_side, net_spread) = if raw_spread >= 0.0 {
                    (PositionSide::Short, PositionSide::Long, raw_spread)
                } else {
                    (PositionSide::Long, PositionSide::Short, -raw_spread)
                };

                let hourly_spread_bps = (net_spread / 8760.0) * 100.0;
                let break_even_hours = if hourly_spread_bps > 0.0001 {
                    roundtrip_cost_bps / hourly_spread_bps
                } else {
                    9999.0
                };

                // Projected multi-horizon cashflows
                let hl_1h_rate_bps = hl_item.funding_rate * 10_000.0;
                let bn_8h_rate_bps = bn_item.funding_rate * 10_000.0;

                let hl_1h_cashflow = match hl_side {
                    PositionSide::Short => hl_1h_rate_bps,
                    PositionSide::Long => -hl_1h_rate_bps,
                };
                let bn_8h_cashflow = match bn_side {
                    PositionSide::Short => bn_8h_rate_bps,
                    PositionSide::Long => -bn_8h_rate_bps,
                };

                let proj_1h = if is_bn_settlement_next {
                    hl_1h_cashflow + bn_8h_cashflow - roundtrip_cost_bps
                } else {
                    hl_1h_cashflow - roundtrip_cost_bps
                };

                let proj_4h = (hl_1h_cashflow * 4.0)
                    + (if is_bn_settlement_next {
                        bn_8h_cashflow
                    } else {
                        0.0
                    })
                    - roundtrip_cost_bps;
                let proj_8h = (hl_1h_cashflow * 8.0) + bn_8h_cashflow - roundtrip_cost_bps;

                opps.push(ArbitrageOpportunity {
                    symbol: sym.clone(),
                    binance_mark_price: bn_item.mark_price,
                    hyperliquid_mark_price: hl_item.mark_price,
                    price_spread_pct: price_spread,
                    binance_rate_8h_pct: bn_item.funding_rate * 100.0,
                    hyperliquid_rate_1h_pct: hl_item.funding_rate * 100.0,
                    binance_apr_pct: bn_item.annualized_apr_pct,
                    hyperliquid_apr_pct: hl_item.annualized_apr_pct,
                    net_spread_apr_pct: net_spread,
                    hyperliquid_side: hl_side,
                    binance_side: bn_side,
                    est_hourly_return_bps: hourly_spread_bps,
                    est_break_even_hours: break_even_hours,
                    is_binance_settlement_next: is_bn_settlement_next,
                    projected_1h_net_bps: proj_1h,
                    projected_4h_net_bps: proj_4h,
                    projected_8h_net_bps: proj_8h,
                });
            }
        }

        opps.sort_by(|a, b| {
            b.net_spread_apr_pct
                .partial_cmp(&a.net_spread_apr_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        opps
    }
}

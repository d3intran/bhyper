use crate::types::{ArbitrageOpportunity, FundingRateInfo, PositionSide};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use tokio::sync::broadcast;

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
    pub binance_rates: FxHashMap<String, FundingRateInfo>,
    pub hyperliquid_rates: FxHashMap<String, FundingRateInfo>,
    pub binance_volumes_24h: FxHashMap<String, f64>,
    pub total_open_interests: FxHashMap<String, f64>,
    pub book_spreads_bps: FxHashMap<String, f64>,
    pub last_binance_update: Option<DateTime<Utc>>,
    pub last_hyperliquid_update: Option<DateTime<Utc>>,
    pub user_fills: Vec<UserFillEvent>,
}

#[derive(Clone)]
pub struct MarketDataCache {
    inner: Arc<RwLock<MarketDataCacheInner>>,
    fill_tx: broadcast::Sender<UserFillEvent>,
}

impl Default for MarketDataCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MarketDataCache {
    pub fn new() -> Self {
        let (fill_tx, _) = broadcast::channel(2048);
        Self {
            inner: Arc::new(RwLock::new(MarketDataCacheInner::default())),
            fill_tx,
        }
    }

    /// Subscribe to live user fill events stream
    pub fn subscribe_fills(&self) -> broadcast::Receiver<UserFillEvent> {
        self.fill_tx.subscribe()
    }

    pub fn update_metadata(
        &self,
        volumes: std::collections::HashMap<String, f64>,
        ois: std::collections::HashMap<String, f64>,
        spreads: std::collections::HashMap<String, f64>,
    ) {
        let mut inner = self.inner.write();
        for (k, v) in volumes {
            inner.binance_volumes_24h.insert(k.to_ascii_uppercase(), v);
        }
        for (k, v) in ois {
            inner.total_open_interests.insert(k.to_ascii_uppercase(), v);
        }
        for (k, v) in spreads {
            inner.book_spreads_bps.insert(k.to_ascii_uppercase(), v);
        }
    }

    pub fn update_binance_rates(&self, rates: Vec<FundingRateInfo>) {
        let mut inner = self.inner.write();
        inner.last_binance_update = Some(Utc::now());
        for rate in rates {
            inner
                .binance_rates
                .insert(rate.symbol.to_ascii_uppercase(), rate);
        }
    }

    pub fn update_hyperliquid_rates(&self, rates: Vec<FundingRateInfo>) {
        let mut inner = self.inner.write();
        inner.last_hyperliquid_update = Some(Utc::now());
        for rate in rates {
            inner
                .hyperliquid_rates
                .insert(rate.symbol.to_ascii_uppercase(), rate);
        }
    }

    /// Update Hyperliquid prices while preserving existing funding rates
    pub fn update_hyperliquid_mids(&self, mids: std::collections::HashMap<String, f64>) {
        let mut inner = self.inner.write();
        inner.last_hyperliquid_update = Some(Utc::now());
        for (sym, price) in mids {
            if price <= 0.0 {
                continue;
            }
            let sym_upper = sym.to_ascii_uppercase();
            if let Some(existing) = inner.hyperliquid_rates.get_mut(&sym_upper) {
                existing.mark_price = price;
                existing.index_price = price;
            } else {
                inner.hyperliquid_rates.insert(
                    sym_upper.clone(),
                    FundingRateInfo {
                        symbol: sym_upper,
                        exchange: crate::types::Exchange::Hyperliquid,
                        mark_price: price,
                        index_price: price,
                        funding_rate: 0.0,
                        funding_interval_hours: 1.0,
                        annualized_apr_pct: 0.0,
                        next_funding_time: Some(Utc::now()),
                    },
                );
            }
        }
    }

    pub fn record_user_fill(&self, fill: UserFillEvent) {
        // 1. Broadcast fill instantaneously to all waiting workers
        let _ = self.fill_tx.send(fill.clone());

        // 2. Persist in ring buffer
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

    /// Get latest mark prices for a symbol (Binance, Hyperliquid)
    pub fn get_latest_prices(&self, symbol: &str) -> Option<(f64, f64)> {
        let sym_upper = symbol.to_ascii_uppercase();
        let inner = self.inner.read();
        let bn_p = inner.binance_rates.get(&sym_upper).map(|r| r.mark_price)?;
        let hl_p = inner
            .hyperliquid_rates
            .get(&sym_upper)
            .map(|r| r.mark_price)?;
        if bn_p > 0.0 && hl_p > 0.0 {
            Some((bn_p, hl_p))
        } else {
            None
        }
    }

    /// Get latest opportunity for a specific symbol directly from in-memory cache
    #[allow(dead_code)]
    pub fn get_latest_opportunity(
        &self,
        symbol: &str,
        roundtrip_cost_bps: f64,
    ) -> Option<ArbitrageOpportunity> {
        let sym_upper = symbol.to_ascii_uppercase();
        let inner = self.inner.read();
        let hl_item = inner.hyperliquid_rates.get(&sym_upper)?;
        let bn_item = inner.binance_rates.get(&sym_upper)?;

        if bn_item.mark_price <= 0.0 || hl_item.mark_price <= 0.0 {
            return None;
        }

        let now = Utc::now();
        let minute = chrono::Timelike::minute(&now);
        let hour = chrono::Timelike::hour(&now);
        let is_bn_settlement_next = (minute >= 50 && (hour == 7 || hour == 15 || hour == 23))
            || (minute <= 10 && (hour == 8 || hour == 16 || hour == 0));

        let price_spread = (hl_item.mark_price - bn_item.mark_price) / bn_item.mark_price * 100.0;
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

        let divergence = if hl_item.index_price > 0.0 {
            ((hl_item.mark_price - hl_item.index_price).abs() / hl_item.index_price) * 100.0
        } else {
            0.0
        };

        let bn_vol_24h = inner
            .binance_volumes_24h
            .get(&sym_upper)
            .copied()
            .unwrap_or(2_000_000.0);
        let total_oi = inner
            .total_open_interests
            .get(&sym_upper)
            .copied()
            .unwrap_or(1_000_000.0);
        let book_spread = inner
            .book_spreads_bps
            .get(&sym_upper)
            .copied()
            .unwrap_or(3.0);

        Some(ArbitrageOpportunity {
            symbol: sym_upper,
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
            binance_volume_24h_usd: bn_vol_24h,
            binance_open_interest_usd: total_oi * 0.6,
            hyperliquid_open_interest_usd: total_oi * 0.4,
            total_open_interest_usd: total_oi,
            bid_ask_spread_bps: book_spread,
            oracle_mark_divergence_pct: divergence,
            is_liquid: total_oi >= 300_000.0 && bn_vol_24h >= 500_000.0,
            liquidity_tier: if total_oi >= 3_000_000.0 {
                "TIER_1_PRIME".to_string()
            } else {
                "TIER_2_LIQUID".to_string()
            },
        })
    }

    /// Compute ranked opportunities directly from memory
    pub fn compute_opportunities(&self, roundtrip_cost_bps: f64) -> Vec<ArbitrageOpportunity> {
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

                let divergence = if hl_item.index_price > 0.0 {
                    ((hl_item.mark_price - hl_item.index_price).abs() / hl_item.index_price) * 100.0
                } else {
                    0.0
                };

                let bn_vol_24h = inner
                    .binance_volumes_24h
                    .get(sym)
                    .copied()
                    .unwrap_or(2_000_000.0);
                let total_oi = inner
                    .total_open_interests
                    .get(sym)
                    .copied()
                    .unwrap_or(1_000_000.0);
                let book_spread = inner.book_spreads_bps.get(sym).copied().unwrap_or(3.0);

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
                    binance_volume_24h_usd: bn_vol_24h,
                    binance_open_interest_usd: total_oi * 0.6,
                    hyperliquid_open_interest_usd: total_oi * 0.4,
                    total_open_interest_usd: total_oi,
                    bid_ask_spread_bps: book_spread,
                    oracle_mark_divergence_pct: divergence,
                    is_liquid: total_oi >= 300_000.0 && bn_vol_24h >= 500_000.0,
                    liquidity_tier: if total_oi >= 3_000_000.0 {
                        "TIER_1_PRIME".to_string()
                    } else {
                        "TIER_2_LIQUID".to_string()
                    },
                });
            }
        }

        opps.sort_unstable_by(|a, b| b.net_spread_apr_pct.total_cmp(&a.net_spread_apr_pct));

        opps
    }
}

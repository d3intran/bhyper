use crate::binance::BinanceFuturesClient;
use crate::hyperliquid::HyperliquidClient;
use crate::types::{ArbitrageOpportunity, PositionSide, SymbolPrecisionInfo};
use crate::ws::MarketDataCache;
use anyhow::Result;
use chrono::{Timelike, Utc};
use rustc_hash::FxHashMap;
use std::collections::HashMap;

pub struct ArbitrageScanner {
    binance: BinanceFuturesClient,
    hyperliquid: HyperliquidClient,
    roundtrip_cost_bps: f64,
    cache: Option<MarketDataCache>,
}

impl ArbitrageScanner {
    pub fn new(
        binance: BinanceFuturesClient,
        hyperliquid: HyperliquidClient,
        maker_taker_mode: bool,
    ) -> Self {
        // Maker-Taker: HL Maker (0.00%) + BN Taker (0.04%) + Slippage (0.02%) = 0.06% per leg = 12 bps roundtrip
        // Taker-Taker: HL Taker (0.035%) + BN Taker (0.04%) + Slippage (0.04%) = 0.115% per leg = 23 bps roundtrip
        let cost_bps = if maker_taker_mode { 12.0 } else { 23.0 };
        Self {
            binance,
            hyperliquid,
            roundtrip_cost_bps: cost_bps,
            cache: None,
        }
    }

    pub fn with_cache(mut self, cache: MarketDataCache) -> Self {
        self.cache = Some(cache);
        self
    }

    /// 获取两所所有共同支持交易对的完整精度与元数据信息 (StepSize, SzDecimals, AssetIndex, MinNotional)
    pub async fn fetch_symbol_precisions(&self) -> Result<HashMap<String, SymbolPrecisionInfo>> {
        let (bn_prec_res, hl_meta_res) = tokio::join!(
            self.binance.fetch_precision_info(),
            self.hyperliquid.fetch_meta()
        );

        let mut bn_precisions = bn_prec_res?;
        let hl_meta = hl_meta_res?;

        let mut shared_precisions = HashMap::with_capacity(hl_meta.universe.len());

        for (idx, u) in hl_meta.universe.into_iter().enumerate() {
            let symbol = u.name.to_ascii_uppercase();
            if let Some(bn_info) = bn_precisions.get_mut(&symbol) {
                bn_info.hyperliquid_sz_decimals = u.sz_decimals;
                bn_info.hyperliquid_asset_index = idx as u32;
                bn_info.hyperliquid_min_notional = 10.0;
                shared_precisions.insert(symbol, bn_info.clone());
            }
        }

        Ok(shared_precisions)
    }

    /// 扫描套利机会：优先从高频 WebSocket 内存缓存计算，降级从 REST 并发拉取
    pub async fn scan_opportunities(&self) -> Result<Vec<ArbitrageOpportunity>> {
        if let Some(ref cache) = self.cache {
            if cache.is_healthy() {
                let cached_opps = cache.compute_opportunities(self.roundtrip_cost_bps);
                if !cached_opps.is_empty() {
                    return Ok(cached_opps);
                }
            }
        }

        // Fallback to REST API
        let (bn_res, hl_res) = tokio::join!(
            self.binance.fetch_all_funding_rates(),
            self.hyperliquid.fetch_all_funding_rates()
        );

        let bn_rates = bn_res?;
        let hl_rates = hl_res?;

        let mut bn_map = FxHashMap::with_capacity_and_hasher(bn_rates.len(), Default::default());
        for item in bn_rates {
            bn_map.insert(item.symbol.to_ascii_uppercase(), item);
        }

        let now = Utc::now();
        let minute = now.minute();
        let hour = now.hour();
        let is_bn_settlement_next = (minute >= 50 && (hour == 7 || hour == 15 || hour == 23))
            || (minute <= 10 && (hour == 8 || hour == 16 || hour == 0));

        let mut opportunities = Vec::with_capacity(hl_rates.len());

        for hl_item in hl_rates {
            let symbol = hl_item.symbol.to_ascii_uppercase();
            if let Some(bn_item) = bn_map.get(&symbol) {
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

                let hourly_spread_bps = (net_spread / 8760.0) * 100.0; // 1% = 100 bps
                let break_even_hours = if hourly_spread_bps > 0.0001 {
                    self.roundtrip_cost_bps / hourly_spread_bps
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
                    hl_1h_cashflow + bn_8h_cashflow - self.roundtrip_cost_bps
                } else {
                    hl_1h_cashflow - self.roundtrip_cost_bps
                };

                let proj_4h = (hl_1h_cashflow * 4.0)
                    + (if is_bn_settlement_next {
                        bn_8h_cashflow
                    } else {
                        0.0
                    })
                    - self.roundtrip_cost_bps;
                let proj_8h = (hl_1h_cashflow * 8.0) + bn_8h_cashflow - self.roundtrip_cost_bps;

                opportunities.push(ArbitrageOpportunity {
                    symbol,
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

        // Sort descending by net spread APR using total_cmp (zero allocation)
        opportunities
            .sort_unstable_by(|a, b| b.net_spread_apr_pct.total_cmp(&a.net_spread_apr_pct));

        Ok(opportunities)
    }
}

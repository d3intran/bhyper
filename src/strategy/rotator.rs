use crate::types::{ActiveArbitragePosition, ArbitrageOpportunity, PositionSide};
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// 动态换仓决策推荐 (Swap Recommendation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapRecommendation {
    pub unwind_symbol: String,
    pub unwind_current_apr: f64,
    pub unwind_holding_hours: f64,
    pub candidate_symbol: String,
    pub candidate_opp: ArbitrageOpportunity,
    pub apr_delta_pct: f64,
    pub est_switching_gain_usd: f64,
    pub rationale: String,
}

/// 全局动态机会成本换仓引擎 (Opportunity Cost Swapper Engine)
#[derive(Debug, Clone)]
pub struct OpportunityRotator {
    pub auto_rotation_enabled: bool,
    pub min_swap_apr_delta_pct: f64,
    pub min_swap_profit_usd: f64,
    pub min_holding_mins_before_swap: f64,
    pub default_horizon_hours: f64,
}

impl Default for OpportunityRotator {
    fn default() -> Self {
        Self {
            auto_rotation_enabled: true,
            min_swap_apr_delta_pct: 30.0,
            min_swap_profit_usd: 0.04,
            min_holding_mins_before_swap: 15.0,
            default_horizon_hours: 6.0,
        }
    }
}

impl OpportunityRotator {
    pub fn new(
        auto_rotation_enabled: bool,
        min_swap_apr_delta_pct: f64,
        min_swap_profit_usd: f64,
        min_holding_mins_before_swap: f64,
    ) -> Self {
        Self {
            auto_rotation_enabled,
            min_swap_apr_delta_pct,
            min_swap_profit_usd,
            min_holding_mins_before_swap,
            default_horizon_hours: 6.0,
        }
    }

    /// 评估当前活跃持仓与全市场候选机会，寻找最高边际收益换仓决策
    pub fn evaluate_swaps(
        &self,
        active_positions: &[ActiveArbitragePosition],
        candidate_opportunities: &[ArbitrageOpportunity],
        opps_map: &std::collections::HashMap<String, &ArbitrageOpportunity>,
    ) -> Option<SwapRecommendation> {
        if !self.auto_rotation_enabled
            || active_positions.is_empty()
            || candidate_opportunities.is_empty()
        {
            return None;
        }

        let now = Utc::now();
        let held_symbols: std::collections::HashSet<&str> =
            active_positions.iter().map(|p| p.symbol.as_str()).collect();

        // 1. 过滤可开仓的候选标的 (排除已持仓标的，且流动性与利差必须达标，偏离度必须在安全范围内)
        let eligible_candidates: Vec<&ArbitrageOpportunity> = candidate_opportunities
            .iter()
            .filter(|o| {
                !held_symbols.contains(o.symbol.as_str())
                    && o.net_spread_apr_pct >= 25.0
                    && o.oracle_mark_divergence_pct <= 0.60
            })
            .collect();

        if eligible_candidates.is_empty() {
            return None;
        }

        let mut best_swap: Option<SwapRecommendation> = None;
        let mut max_net_gain_usd = self.min_swap_profit_usd;

        // 2. 遍历所有当前持仓标的，测算当前实际有效利差
        for pos in active_positions {
            let holding_mins = (now - pos.opened_at).num_seconds() as f64 / 60.0;
            let holding_hours = holding_mins / 60.0;

            // 保护期：避免微秒级频繁换仓导致过度损耗手续费
            if holding_mins < self.min_holding_mins_before_swap {
                continue;
            }

            // 计算该持仓当下的实时净利差
            let current_effective_apr = if let Some(current_opp) = opps_map.get(&pos.symbol) {
                match pos.hyperliquid_side {
                    PositionSide::Short => {
                        current_opp.hyperliquid_apr_pct - current_opp.binance_apr_pct
                    }
                    PositionSide::Long => {
                        current_opp.binance_apr_pct - current_opp.hyperliquid_apr_pct
                    }
                }
            } else {
                pos.current_spread_apr
            };

            let notional = if pos.nominal_value_usd > 0.0 {
                pos.nominal_value_usd
            } else {
                100.0
            };

            // 3. 对比每一个候选标的
            for cand in &eligible_candidates {
                let apr_delta = cand.net_spread_apr_pct - current_effective_apr;
                if apr_delta < self.min_swap_apr_delta_pct {
                    continue;
                }

                // 测算预期持有周期 (以候选标的回本周期的 2.5 倍或预设 6 小时为基准)
                let horizon_hours =
                    if cand.est_break_even_hours > 0.0 && cand.est_break_even_hours < 24.0 {
                        (cand.est_break_even_hours * 2.5).clamp(2.0, 12.0)
                    } else {
                        self.default_horizon_hours
                    };

                // 换仓总交易摩擦: 平掉旧仓 (双边 7.5 bps) + 开新仓 (Maker-Taker ~4.0 bps 或双边 7.5 bps)
                let total_friction_bps = 11.5;
                let friction_usd = notional * (total_friction_bps / 10_000.0);

                // 超额资金费收益
                let gross_gain_usd = notional * (apr_delta / 100.0) * (horizon_hours / 8760.0);
                let net_gain_usd = gross_gain_usd - friction_usd;

                if net_gain_usd > max_net_gain_usd {
                    max_net_gain_usd = net_gain_usd;
                    best_swap = Some(SwapRecommendation {
                        unwind_symbol: pos.symbol.clone(),
                        unwind_current_apr: current_effective_apr,
                        unwind_holding_hours: holding_hours,
                        candidate_symbol: cand.symbol.clone(),
                        candidate_opp: (*cand).clone(),
                        apr_delta_pct: apr_delta,
                        est_switching_gain_usd: net_gain_usd,
                        rationale: format!(
                            "当前持仓 {} 利差衰减至 {:.1}% APR, 候选标的 {} 净利差高达 {:.1}% APR (提升 +{:.1}% APR). 扣除换仓手续费后预计 {}h 净增益 +${:.4}",
                            pos.symbol, current_effective_apr, cand.symbol, cand.net_spread_apr_pct, apr_delta, horizon_hours as u32, net_gain_usd
                        ),
                    });
                }
            }
        }

        best_swap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opportunity_rotator_swap_selection() {
        let rotator = OpportunityRotator::default();

        let pos = ActiveArbitragePosition {
            symbol: "SAGA".into(),
            binance_side: PositionSide::Long,
            binance_qty: 6451.9,
            binance_entry_price: 0.014,
            hyperliquid_side: PositionSide::Short,
            hyperliquid_qty: 6451.9,
            hyperliquid_entry_price: 0.014,
            nominal_value_usd: 100.0,
            net_delta_usd: 0.0,
            entry_spread_apr: 765.0,
            current_spread_apr: 5.0,
            accumulated_funding_usd: 0.20,
            opened_at: Utc::now() - chrono::Duration::hours(4),
            last_updated_at: Utc::now(),
            is_closed: false,
            closed_at: None,
            realized_pnl_usd: None,
        };

        let cand = ArbitrageOpportunity {
            symbol: "PUMP".into(),
            binance_mark_price: 0.005,
            hyperliquid_mark_price: 0.005,
            price_spread_pct: 0.0,
            binance_rate_8h_pct: 0.005,
            hyperliquid_rate_1h_pct: 0.018,
            binance_apr_pct: 5.47,
            hyperliquid_apr_pct: 160.0,
            net_spread_apr_pct: 154.53,
            hyperliquid_side: PositionSide::Short,
            binance_side: PositionSide::Long,
            est_hourly_return_bps: 1.5,
            est_break_even_hours: 4.0,
            is_binance_settlement_next: false,
            projected_1h_net_bps: 10.0,
            projected_4h_net_bps: 40.0,
            projected_8h_net_bps: 80.0,
            binance_volume_24h_usd: 10_000_000.0,
            binance_open_interest_usd: 50_000_000.0,
            hyperliquid_open_interest_usd: 50_000_000.0,
            total_open_interest_usd: 100_000_000.0,
            bid_ask_spread_bps: 5.0,
            oracle_mark_divergence_pct: 0.05,
            is_liquid: true,
            liquidity_tier: "TIER_1_PRIME".into(),
        };

        let mut opps_map = std::collections::HashMap::new();
        let saga_opp = ArbitrageOpportunity {
            symbol: "SAGA".into(),
            binance_mark_price: 0.014,
            hyperliquid_mark_price: 0.014,
            price_spread_pct: 0.0,
            binance_rate_8h_pct: 0.005,
            hyperliquid_rate_1h_pct: 0.00125,
            binance_apr_pct: 5.47,
            hyperliquid_apr_pct: 10.95,
            net_spread_apr_pct: 5.48,
            hyperliquid_side: PositionSide::Short,
            binance_side: PositionSide::Long,
            est_hourly_return_bps: 0.05,
            est_break_even_hours: 100.0,
            is_binance_settlement_next: false,
            projected_1h_net_bps: -10.0,
            projected_4h_net_bps: -10.0,
            projected_8h_net_bps: -10.0,
            binance_volume_24h_usd: 1_000_000.0,
            binance_open_interest_usd: 1_000_000.0,
            hyperliquid_open_interest_usd: 1_000_000.0,
            total_open_interest_usd: 2_000_000.0,
            bid_ask_spread_bps: 5.0,
            oracle_mark_divergence_pct: 0.05,
            is_liquid: true,
            liquidity_tier: "TIER_2_LIQUID".into(),
        };
        opps_map.insert("SAGA".to_string(), &saga_opp);

        let swap = rotator.evaluate_swaps(&[pos], std::slice::from_ref(&cand), &opps_map);
        assert!(swap.is_some());
        let s = swap.unwrap();
        assert_eq!(s.unwind_symbol, "SAGA");
        assert_eq!(s.candidate_symbol, "PUMP");
        assert!(s.est_switching_gain_usd > 0.04);
    }
}

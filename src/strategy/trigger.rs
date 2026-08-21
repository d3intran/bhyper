use crate::types::{ArbitrageOpportunity, PositionSide};
use chrono::{Timelike, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDecision {
    pub symbol: String,
    pub should_open: bool,
    pub hl_side: PositionSide,
    pub bn_side: PositionSide,
    pub target_notional_usd: f64,
    pub single_cycle_income_bps: f64,
    pub total_friction_cost_bps: f64,
    pub net_expected_profit_bps: f64,
    pub seconds_to_settlement: u32,
    pub reject_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProfitTriggerEngine {
    pub min_net_profit_bps: f64, // 单次必须确保的最小净利润 (例如 3.5 bps = 0.035%)
    pub max_basis_spread_bps: f64, // 允许的最大基差倒挂 (例如 25 bps)
    pub min_notional_usd: f64,   // 单笔最小名义价值 (例如 $12 满足两所限制)
    pub max_notional_usd: f64,   // 单笔最大名义价值 (例如 $50 用于小资金)
    pub sniper_window_secs: (u32, u32), // 狙击窗口: (最小秒数 10s, 最大秒数 60s)
}

impl Default for ProfitTriggerEngine {
    fn default() -> Self {
        Self {
            min_net_profit_bps: 3.5,      // 必须保证单次至少净赚 0.035%
            max_basis_spread_bps: 20.0,   // 基差倒挂不超过 0.20%
            min_notional_usd: 12.0,       // 超过 HL $10 和 BN $5 的硬门槛
            max_notional_usd: 50.0,       // 针对 $100 初始小本金
            sniper_window_secs: (10, 60), // 整点前 10s ~ 60s 触发
        }
    }
}

impl ProfitTriggerEngine {
    #[allow(dead_code)]
    pub fn new(min_net_profit_bps: f64, max_notional_usd: f64) -> Self {
        Self {
            min_net_profit_bps,
            max_notional_usd,
            ..Default::default()
        }
    }

    /// 计算当前距离下一个整点结算还有多少秒
    pub fn seconds_until_next_hour() -> u32 {
        let now = Utc::now();
        let minute = now.minute();
        let second = now.second();
        let elapsed_in_hour = minute * 60 + second;
        3600 - elapsed_in_hour
    }

    /// 核心判定: 严格评估当前机会是否符合单次确定性盈利触发条件
    pub fn evaluate_opportunity(
        &self,
        opp: &ArbitrageOpportunity,
        available_margin_usd: f64,
        ignore_window: bool,
    ) -> TriggerDecision {
        let secs_left = Self::seconds_until_next_hour();

        // 1. 时间窗口锁 (Timing Sniper Lock)
        let in_window = ignore_window
            || (secs_left >= self.sniper_window_secs.0 && secs_left <= self.sniper_window_secs.1);
        if !in_window {
            return TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: false,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: 0.0,
                single_cycle_income_bps: 0.0,
                total_friction_cost_bps: 0.0,
                net_expected_profit_bps: 0.0,
                seconds_to_settlement: secs_left,
                reject_reason: Some(format!(
                    "不在整点狙击窗口内 (剩余 {}s, 需在 {}-{}s 之间)",
                    secs_left, self.sniper_window_secs.0, self.sniper_window_secs.1
                )),
            };
        }

        // 2. 标的单价硬筛选 (避开 BTC/ETH 等单价过高无法精细对齐的币种)
        if opp.binance_mark_price > 500.0 {
            return TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: false,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: 0.0,
                single_cycle_income_bps: 0.0,
                total_friction_cost_bps: 0.0,
                net_expected_profit_bps: 0.0,
                seconds_to_settlement: secs_left,
                reject_reason: Some(format!(
                    "单价过高 (${:.2}), 小资金无法消除步长截断风险",
                    opp.binance_mark_price
                )),
            };
        }

        // 3. 基差安全垫判断 (Basis Cushion Guard)
        let entry_basis_bps = opp.price_spread_pct * 100.0;
        if opp.hyperliquid_side == PositionSide::Short
            && entry_basis_bps < -self.max_basis_spread_bps
        {
            return TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: false,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: 0.0,
                single_cycle_income_bps: 0.0,
                total_friction_cost_bps: 0.0,
                net_expected_profit_bps: 0.0,
                seconds_to_settlement: secs_left,
                reject_reason: Some(format!(
                    "基差倒挂严重 ({:.2} bps < -{:.2} bps 限制)",
                    entry_basis_bps, self.max_basis_spread_bps
                )),
            };
        }

        // 4. 计算单期与目标持仓周期的利差收入
        let hl_1h_rate_bps = (opp.hyperliquid_rate_1h_pct / 100.0) * 10_000.0;
        let bn_1h_equiv_bps = ((opp.binance_rate_8h_pct / 8.0) / 100.0) * 10_000.0;

        let single_cycle_income_bps = match opp.hyperliquid_side {
            PositionSide::Short => hl_1h_rate_bps - bn_1h_equiv_bps,
            PositionSide::Long => -hl_1h_rate_bps + bn_1h_equiv_bps,
        };

        // 5. 计算确定性摩擦成本 (Maker-Taker 模式)
        // HL Maker 0.00% + BN Taker 0.04% + 双边滑点 0.02% + 平仓成本预留 0.04% = 10 bps (0.10%)
        let total_friction_cost_bps = 10.0;

        // 6. 预期净利润 (基于持仓 4 小时或回本周期评估)
        let single_cycle_net_bps = single_cycle_income_bps - total_friction_cost_bps;
        let target_holding_hours = 4.0;
        let multi_cycle_net_bps =
            (single_cycle_income_bps * target_holding_hours) - total_friction_cost_bps;

        // 触发条件: 回本时间 <= 2.0h 且 4h 预期收益 >= min_net_profit_bps
        let is_profitable =
            opp.est_break_even_hours <= 2.0 && multi_cycle_net_bps >= self.min_net_profit_bps;

        if !is_profitable {
            return TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: false,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: 0.0,
                single_cycle_income_bps,
                total_friction_cost_bps,
                net_expected_profit_bps: single_cycle_net_bps,
                seconds_to_settlement: secs_left,
                reject_reason: Some(format!(
                    "回本耗时 {:.1}h > 2.0h 门槛 (1h收益: {:.2} bps, 4h预期净利: {:.2} bps)",
                    opp.est_break_even_hours, single_cycle_income_bps, multi_cycle_net_bps
                )),
            };
        }

        // 7. 计算适合小资金的名义仓位 (受限于可用资金与最大单笔上限)
        let safe_notional =
            (available_margin_usd * 0.9).clamp(self.min_notional_usd, self.max_notional_usd);

        TriggerDecision {
            symbol: opp.symbol.clone(),
            should_open: true,
            hl_side: opp.hyperliquid_side,
            bn_side: opp.binance_side,
            target_notional_usd: safe_notional,
            single_cycle_income_bps,
            total_friction_cost_bps,
            net_expected_profit_bps: multi_cycle_net_bps,
            seconds_to_settlement: secs_left,
            reject_reason: None,
        }
    }
}

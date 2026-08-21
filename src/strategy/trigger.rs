use crate::strategy::precision::LotPrecisionMatcher;
use crate::types::{AlignedQuantity, ArbitrageOpportunity, PositionSide, SymbolPrecisionInfo};
use chrono::{Timelike, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDecision {
    pub symbol: String,
    pub should_open: bool,
    pub hl_side: PositionSide,
    pub bn_side: PositionSide,
    pub target_notional_usd: f64,
    pub aligned_quantity: Option<AlignedQuantity>,
    pub single_cycle_income_bps: f64,
    pub total_friction_cost_bps: f64,
    pub net_expected_profit_bps: f64,
    pub net_expected_profit_usd: f64,
    pub seconds_to_settlement: u32,
    pub is_binance_settlement_next: bool,
    pub projected_4h_net_bps: f64,
    pub reject_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProfitTriggerEngine {
    pub min_net_profit_bps: f64, // 单次必须确保的最小净利润 (例如 3.5 bps = 0.035%)
    pub max_basis_spread_bps: f64, // 允许的最大基差倒挂 (例如 20 bps)
    pub min_notional_usd: f64,   // 单笔最小名义价值 (例如 $12 满足两所限制)
    pub max_notional_usd: f64,   // 单笔最大名义价值 (例如 $50 用于小资金)
    pub sniper_window_secs: (u32, u32), // 狙击窗口: (最小秒数 10s, 最大秒数 60s)
    pub maker_taker_mode: bool,
    pub min_open_interest_usd: f64,
    pub min_24h_volume_usd: f64,
    pub max_bid_ask_spread_bps: f64,
    pub max_oracle_mark_divergence_pct: f64,
    pub symbol_whitelist: Vec<String>,
    pub symbol_blacklist: Vec<String>,
}

impl Default for ProfitTriggerEngine {
    fn default() -> Self {
        Self {
            min_net_profit_bps: 3.5,      // 必须保证单次至少净赚 0.035%
            max_basis_spread_bps: 20.0,   // 基差倒挂不超过 0.20%
            min_notional_usd: 12.0,       // 超过 HL $10 和 BN $5 的硬门槛
            max_notional_usd: 50.0,       // 针对 $100 初始小本金
            sniper_window_secs: (10, 60), // 整点前 10s ~ 60s 触发
            maker_taker_mode: true,
            min_open_interest_usd: 300_000.0,
            min_24h_volume_usd: 500_000.0,
            max_bid_ask_spread_bps: 25.0,
            max_oracle_mark_divergence_pct: 0.6,
            symbol_whitelist: Vec::new(),
            symbol_blacklist: vec!["USTC".into(), "LUNC".into()],
        }
    }
}

impl ProfitTriggerEngine {
    pub fn new(
        min_net_profit_bps: f64,
        max_notional_usd: f64,
        maker_taker_mode: bool,
    ) -> Self {
        Self {
            min_net_profit_bps,
            max_notional_usd,
            maker_taker_mode,
            ..Default::default()
        }
    }

    pub fn with_liquidity_guards(
        mut self,
        min_open_interest_usd: f64,
        min_24h_volume_usd: f64,
        max_bid_ask_spread_bps: f64,
        max_oracle_mark_divergence_pct: f64,
        whitelist: Vec<String>,
        blacklist: Vec<String>,
    ) -> Self {
        self.min_open_interest_usd = min_open_interest_usd;
        self.min_24h_volume_usd = min_24h_volume_usd;
        self.max_bid_ask_spread_bps = max_bid_ask_spread_bps;
        self.max_oracle_mark_divergence_pct = max_oracle_mark_divergence_pct;
        self.symbol_whitelist = whitelist;
        self.symbol_blacklist = blacklist;
        self
    }

    /// 计算当前距离下一个整点结算还有多少秒
    #[inline]
    pub fn seconds_until_next_hour_at(now: &chrono::DateTime<Utc>) -> u32 {
        let minute = now.minute();
        let second = now.second();
        let elapsed_in_hour = minute * 60 + second;
        3600 - elapsed_in_hour
    }

    /// 计算当前距离下一个整点结算还有多少秒
    #[inline]
    pub fn seconds_until_next_hour() -> u32 {
        Self::seconds_until_next_hour_at(&Utc::now())
    }

    /// 判断下一个整点是否是币安 8 小时结算周期 (00:00, 08:00, 16:00 UTC)
    #[inline]
    pub fn is_binance_settlement_hour_at(now: &chrono::DateTime<Utc>) -> bool {
        let minute = now.minute();
        let hour = now.hour();
        (minute >= 50 && (hour == 7 || hour == 15 || hour == 23))
            || (minute <= 10 && (hour == 8 || hour == 16 || hour == 0))
    }

    /// 判断下一个整点是否是币安 8 小时结算周期 (00:00, 08:00, 16:00 UTC)
    #[inline]
    pub fn is_binance_settlement_hour() -> bool {
        Self::is_binance_settlement_hour_at(&Utc::now())
    }

    /// 核心判定: 严格评估当前机会是否符合单次确定性盈利触发条件 (支持精确步长对齐与时序结算排期)
    pub fn evaluate_opportunity(
        &self,
        opp: &ArbitrageOpportunity,
        available_margin_usd: f64,
        ignore_window: bool,
        precision_info: Option<&SymbolPrecisionInfo>,
    ) -> TriggerDecision {
        let now = Utc::now();
        let secs_left = Self::seconds_until_next_hour_at(&now);
        let is_bn_settlement = Self::is_binance_settlement_hour_at(&now);

        // 0. 黑白名单锁 (Whitelist / Blacklist Guard)
        if self.symbol_blacklist.iter().any(|b| b.eq_ignore_ascii_case(&opp.symbol)) {
            return TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: false,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: 0.0,
                aligned_quantity: None,
                single_cycle_income_bps: 0.0,
                total_friction_cost_bps: 0.0,
                net_expected_profit_bps: 0.0,
                net_expected_profit_usd: 0.0,
                seconds_to_settlement: secs_left,
                is_binance_settlement_next: is_bn_settlement,
                projected_4h_net_bps: 0.0,
                reject_reason: Some(format!("标的 {} 在系统黑名单中", opp.symbol)),
            };
        }

        if !self.symbol_whitelist.is_empty()
            && !self.symbol_whitelist.iter().any(|w| w.eq_ignore_ascii_case(&opp.symbol))
        {
            return TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: false,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: 0.0,
                aligned_quantity: None,
                single_cycle_income_bps: 0.0,
                total_friction_cost_bps: 0.0,
                net_expected_profit_bps: 0.0,
                net_expected_profit_usd: 0.0,
                seconds_to_settlement: secs_left,
                is_binance_settlement_next: is_bn_settlement,
                projected_4h_net_bps: 0.0,
                reject_reason: Some(format!("标的 {} 不在策略白名单中", opp.symbol)),
            };
        }

        // 1. 流动性与持仓量锁 (Liquidity & Open Interest Guard)
        if opp.total_open_interest_usd > 0.0 && opp.total_open_interest_usd < self.min_open_interest_usd {
            return TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: false,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: 0.0,
                aligned_quantity: None,
                single_cycle_income_bps: 0.0,
                total_friction_cost_bps: 0.0,
                net_expected_profit_bps: 0.0,
                net_expected_profit_usd: 0.0,
                seconds_to_settlement: secs_left,
                is_binance_settlement_next: is_bn_settlement,
                projected_4h_net_bps: 0.0,
                reject_reason: Some(format!(
                    "持仓量 OI ${:.0} 低于门槛 ${:.0} (小币种操纵风险)",
                    opp.total_open_interest_usd, self.min_open_interest_usd
                )),
            };
        }

        if opp.binance_volume_24h_usd > 0.0 && opp.binance_volume_24h_usd < self.min_24h_volume_usd {
            return TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: false,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: 0.0,
                aligned_quantity: None,
                single_cycle_income_bps: 0.0,
                total_friction_cost_bps: 0.0,
                net_expected_profit_bps: 0.0,
                net_expected_profit_usd: 0.0,
                seconds_to_settlement: secs_left,
                is_binance_settlement_next: is_bn_settlement,
                projected_4h_net_bps: 0.0,
                reject_reason: Some(format!(
                    "24h 成交额 ${:.0} 低于门槛 ${:.0} (流动性不足)",
                    opp.binance_volume_24h_usd, self.min_24h_volume_usd
                )),
            };
        }

        if opp.bid_ask_spread_bps > self.max_bid_ask_spread_bps {
            return TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: false,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: 0.0,
                aligned_quantity: None,
                single_cycle_income_bps: 0.0,
                total_friction_cost_bps: 0.0,
                net_expected_profit_bps: 0.0,
                net_expected_profit_usd: 0.0,
                seconds_to_settlement: secs_left,
                is_binance_settlement_next: is_bn_settlement,
                projected_4h_net_bps: 0.0,
                reject_reason: Some(format!(
                    "买卖价差 {:.1} bps 超过限制 {:.1} bps",
                    opp.bid_ask_spread_bps, self.max_bid_ask_spread_bps
                )),
            };
        }

        if opp.oracle_mark_divergence_pct > self.max_oracle_mark_divergence_pct {
            return TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: false,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: 0.0,
                aligned_quantity: None,
                single_cycle_income_bps: 0.0,
                total_friction_cost_bps: 0.0,
                net_expected_profit_bps: 0.0,
                net_expected_profit_usd: 0.0,
                seconds_to_settlement: secs_left,
                is_binance_settlement_next: is_bn_settlement,
                projected_4h_net_bps: 0.0,
                reject_reason: Some(format!(
                    "标记价与预言机偏离 {:.2}% 超过阈值 {:.2}% (费率突变风险)",
                    opp.oracle_mark_divergence_pct, self.max_oracle_mark_divergence_pct
                )),
            };
        }

        // 2. 时间窗口锁 (Timing Sniper Lock)
        let in_window = ignore_window
            || (secs_left >= self.sniper_window_secs.0 && secs_left <= self.sniper_window_secs.1);
        if !in_window {
            return TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: false,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: 0.0,
                aligned_quantity: None,
                single_cycle_income_bps: 0.0,
                total_friction_cost_bps: 0.0,
                net_expected_profit_bps: 0.0,
                net_expected_profit_usd: 0.0,
                seconds_to_settlement: secs_left,
                is_binance_settlement_next: is_bn_settlement,
                projected_4h_net_bps: 0.0,
                reject_reason: Some(format!(
                    "不在整点狙击窗口内 (剩余 {}s, 需在 {}-{}s 之间)",
                    secs_left, self.sniper_window_secs.0, self.sniper_window_secs.1
                )),
            };
        }

        // 2. 基差安全垫判断 (Basis Cushion Guard)
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
                aligned_quantity: None,
                single_cycle_income_bps: 0.0,
                total_friction_cost_bps: 0.0,
                net_expected_profit_bps: 0.0,
                net_expected_profit_usd: 0.0,
                seconds_to_settlement: secs_left,
                is_binance_settlement_next: is_bn_settlement,
                projected_4h_net_bps: 0.0,
                reject_reason: Some(format!(
                    "基差倒挂严重 ({:.2} bps < -{:.2} bps 限制)",
                    entry_basis_bps, self.max_basis_spread_bps
                )),
            };
        }

        // 3. 计算单期真实的费率现金流 (考虑 1h 与 8h 结算排期差异)
        let hl_1h_rate_bps = (opp.hyperliquid_rate_1h_pct / 100.0) * 10_000.0;
        let bn_8h_rate_bps = (opp.binance_rate_8h_pct / 100.0) * 10_000.0;

        let hl_1h_cashflow = match opp.hyperliquid_side {
            PositionSide::Short => hl_1h_rate_bps,
            PositionSide::Long => -hl_1h_rate_bps,
        };
        let bn_8h_cashflow = match opp.binance_side {
            PositionSide::Short => bn_8h_rate_bps,
            PositionSide::Long => -bn_8h_rate_bps,
        };

        // 单期 (1h) 实际到账现金流
        let single_cycle_income_bps = if is_bn_settlement {
            hl_1h_cashflow + bn_8h_cashflow
        } else {
            hl_1h_cashflow // 币安在非 8h 节点结算不发放/扣除资金费
        };

        // 4. 摩擦成本 (Maker-Taker vs Taker-Taker)
        // Maker-Taker: HL Maker 0.00% + BN Taker 0.04% + 滑点 0.02% + 平仓摩擦 0.06% = 12 bps
        // Taker-Taker: HL Taker 0.035% + BN Taker 0.04% + 滑点 0.04% + 平仓摩擦 0.115% = 23 bps
        let total_friction_cost_bps = if self.maker_taker_mode { 12.0 } else { 23.0 };

        // 5. 预期持仓净利润 (单期 1h 与 4h 预期)
        let single_cycle_net_bps = single_cycle_income_bps - total_friction_cost_bps;
        let projected_4h_net_bps = (hl_1h_cashflow * 4.0)
            + (if is_bn_settlement {
                bn_8h_cashflow
            } else {
                0.0
            })
            - total_friction_cost_bps;

        // 触发条件: 回本时间 <= 2.5h 且 (4h 预期净利 >= min_net_profit_bps 或 1h 立即净赚)
        let is_profitable = (opp.est_break_even_hours <= 2.5
            && projected_4h_net_bps >= self.min_net_profit_bps)
            || single_cycle_net_bps >= self.min_net_profit_bps;

        if !is_profitable {
            return TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: false,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: 0.0,
                aligned_quantity: None,
                single_cycle_income_bps,
                total_friction_cost_bps,
                net_expected_profit_bps: single_cycle_net_bps,
                net_expected_profit_usd: 0.0,
                seconds_to_settlement: secs_left,
                is_binance_settlement_next: is_bn_settlement,
                projected_4h_net_bps,
                reject_reason: Some(format!(
                    "回本耗时 {:.1}h > 2.5h 或 4h净利 {:.2} bps 低于门槛 {:.2} bps",
                    opp.est_break_even_hours, projected_4h_net_bps, self.min_net_profit_bps
                )),
            };
        }

        // 6. 精确步长对齐与小资金名义价值校验 (Precision & StepSize Lock)
        let target_usd =
            (available_margin_usd * 0.9).clamp(self.min_notional_usd, self.max_notional_usd);

        if let Some(prec) = precision_info {
            let aligned = LotPrecisionMatcher::calculate_aligned_quantity(
                &opp.symbol,
                opp.binance_mark_price,
                target_usd,
                prec,
            );

            if !aligned.is_aligned {
                return TriggerDecision {
                    symbol: opp.symbol.clone(),
                    should_open: false,
                    hl_side: opp.hyperliquid_side,
                    bn_side: opp.binance_side,
                    target_notional_usd: 0.0,
                    aligned_quantity: Some(aligned.clone()),
                    single_cycle_income_bps,
                    total_friction_cost_bps,
                    net_expected_profit_bps: projected_4h_net_bps,
                    net_expected_profit_usd: 0.0,
                    seconds_to_settlement: secs_left,
                    is_binance_settlement_next: is_bn_settlement,
                    projected_4h_net_bps,
                    reject_reason: Some(
                        aligned
                            .reject_reason
                            .unwrap_or_else(|| "两所下单步长精度无法对齐".to_string()),
                    ),
                };
            }

            let net_usd = aligned.notional_usd * (projected_4h_net_bps / 10_000.0);

            TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: true,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: aligned.notional_usd,
                aligned_quantity: Some(aligned),
                single_cycle_income_bps,
                total_friction_cost_bps,
                net_expected_profit_bps: projected_4h_net_bps,
                net_expected_profit_usd: net_usd,
                seconds_to_settlement: secs_left,
                is_binance_settlement_next: is_bn_settlement,
                projected_4h_net_bps,
                reject_reason: None,
            }
        } else {
            // Fallback
            let net_usd = target_usd * (projected_4h_net_bps / 10_000.0);
            TriggerDecision {
                symbol: opp.symbol.clone(),
                should_open: true,
                hl_side: opp.hyperliquid_side,
                bn_side: opp.binance_side,
                target_notional_usd: target_usd,
                aligned_quantity: None,
                single_cycle_income_bps,
                total_friction_cost_bps,
                net_expected_profit_bps: projected_4h_net_bps,
                net_expected_profit_usd: net_usd,
                seconds_to_settlement: secs_left,
                is_binance_settlement_next: is_bn_settlement,
                projected_4h_net_bps,
                reject_reason: None,
            }
        }
    }
}

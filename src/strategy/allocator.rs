use crate::types::ArbitrageOpportunity;

/// 机构级动态资金配置与跨所防爆仓风控器
#[derive(Debug, Clone)]
pub struct CapitalAllocator {
    /// 是否开启自适应动态资金配置 (关闭时回退到固定单仓配置)
    pub enabled: bool,
    /// 绝对保留的清算安全垫比例 (例如 15.0 代表 15% 净值不可动用以抗单边 35% 价格脉冲)
    pub liquidation_safety_buffer_pct: f64,
    /// 目标保守杠杆 (例如 2.0x)
    pub target_leverage: f64,
    /// 单个标的硬顶名义价值上限 (USD, 避免小币种流动性冲击)
    pub max_single_position_cap_usd: f64,
    /// 最大允许同时持仓槽位数
    pub max_active_positions: usize,
    /// 跨所权益偏离度报警阈值 (例如 35.0 代表两所资金差距超过 35% 时降级开仓)
    pub max_skew_threshold_pct: f64,
    /// 超高 Alpha 标的倾斜扩容开关
    pub alpha_concentration_boost: bool,
}

/// 动态开仓资金量化决策结果
#[derive(Debug, Clone)]
pub struct DynamicAllocationDecision {
    /// 建议的目标开仓名义价值 (USD)
    pub target_notional_usd: f64,
    /// 单所单边所需锁定的保证金 (USD)
    pub margin_required_per_leg_usd: f64,
    /// 当前全组合总资金利用率 (%)
    pub portfolio_utilization_pct: f64,
    /// 组合实际有效名义杠杆 (x)
    pub effective_leverage: f64,
    /// 跨所资金偏离度 (%)
    pub cross_exchange_skew_pct: f64,
    /// 是否具备安全开仓条件
    pub is_safe: bool,
    /// 拒绝原因 (若不安全)
    pub reject_reason: Option<String>,
}

impl Default for CapitalAllocator {
    fn default() -> Self {
        Self {
            enabled: true,
            liquidation_safety_buffer_pct: 15.0,
            target_leverage: 2.0,
            max_single_position_cap_usd: 220.0,
            max_active_positions: 3,
            max_skew_threshold_pct: 35.0,
            alpha_concentration_boost: true,
        }
    }
}

impl CapitalAllocator {
    pub fn new(
        enabled: bool,
        liquidation_safety_buffer_pct: f64,
        target_leverage: f64,
        max_single_position_cap_usd: f64,
        max_active_positions: usize,
    ) -> Self {
        Self {
            enabled,
            liquidation_safety_buffer_pct: liquidation_safety_buffer_pct.clamp(5.0, 40.0),
            target_leverage: target_leverage.clamp(1.0, 5.0),
            max_single_position_cap_usd: max_single_position_cap_usd.max(10.0),
            max_active_positions: max_active_positions.max(1),
            max_skew_threshold_pct: 35.0,
            alpha_concentration_boost: true,
        }
    }

    /// 核心计算: 基于双所可用资金、已开名义价值与剩余槽位，动态计算当前最优开仓规模
    pub fn calculate_slot_allocation(
        &self,
        bn_equity_usd: f64,
        hl_equity_usd: f64,
        current_active_notional_usd: f64,
        current_active_count: usize,
        candidate_opp: Option<&ArbitrageOpportunity>,
    ) -> DynamicAllocationDecision {
        let total_equity = bn_equity_usd + hl_equity_usd;
        if total_equity <= 0.0 {
            return DynamicAllocationDecision {
                target_notional_usd: 0.0,
                margin_required_per_leg_usd: 0.0,
                portfolio_utilization_pct: 0.0,
                effective_leverage: 0.0,
                cross_exchange_skew_pct: 0.0,
                is_safe: false,
                reject_reason: Some("双所账户总净值归零或异常".to_string()),
            };
        }

        // 1. 跨所资金偏离度 (Skew Metric)
        let skew_pct = ((bn_equity_usd - hl_equity_usd).abs() / total_equity) * 100.0;

        // 2. 双所短板可用基础权益 (以两所中较少的一边作为对冲基准)
        let min_single_equity = bn_equity_usd.min(hl_equity_usd);
        let buffer_factor = (100.0 - self.liquidation_safety_buffer_pct) / 100.0;
        let safe_single_equity = min_single_equity * buffer_factor;

        // 3. 组合总安全名义容量 (Total Safe Notional Capacity)
        // 例如 min_equity = $250, buffer = 15% -> $212.50. 2x 杠杆 -> $425.00 总名义容量
        let total_safe_notional = safe_single_equity * self.target_leverage;

        // 4. 剩余未分配名义容量
        let remaining_notional = (total_safe_notional - current_active_notional_usd).max(0.0);
        let remaining_slots = self
            .max_active_positions
            .saturating_sub(current_active_count)
            .max(1);

        if remaining_notional < 10.0 {
            return DynamicAllocationDecision {
                target_notional_usd: 0.0,
                margin_required_per_leg_usd: 0.0,
                portfolio_utilization_pct: (current_active_notional_usd / total_equity) * 100.0,
                effective_leverage: current_active_notional_usd / total_equity,
                cross_exchange_skew_pct: skew_pct,
                is_safe: false,
                reject_reason: Some(format!(
                    "已达到安全名义容量上限 (${:.2} / ${:.2}), 触发防强平安全锁",
                    current_active_notional_usd, total_safe_notional
                )),
            };
        }

        // 5. 基准单槽分配金额
        let base_slot_notional = remaining_notional / (remaining_slots as f64);

        // 6. 极端高息倾斜 (Alpha Tilt Boost)
        let mut final_notional = base_slot_notional;
        if self.alpha_concentration_boost {
            if let Some(opp) = candidate_opp {
                if opp.net_spread_apr_pct >= 300.0 && remaining_slots == 1 {
                    // 若是最后一个槽位且年化超 300%, 允许吸收剩余未分配额度 (最高 1.35x)
                    final_notional = (base_slot_notional * 1.35).min(remaining_notional);
                } else if opp.net_spread_apr_pct >= 200.0 {
                    final_notional = (base_slot_notional * 1.15).min(remaining_notional);
                }
            }
        }

        // 7. 施加单币硬顶与最低名义价值约束 (Min $10 on HL, Hard cap)
        final_notional = final_notional
            .clamp(10.0, self.max_single_position_cap_usd)
            .min(remaining_notional);

        // 8. 计算单边所需保证金 (2x 杠杆 = 50% 保证金)
        let margin_req_per_leg = final_notional / self.target_leverage;

        // 9. 验证两所可用现金是否充足
        let min_free_cash =
            min_single_equity - (current_active_notional_usd / self.target_leverage);
        let is_cash_sufficient = min_free_cash >= margin_req_per_leg;

        let projected_total_notional = current_active_notional_usd + final_notional;
        let utilization_pct = (projected_total_notional / total_equity) * 100.0;
        let effective_lev = projected_total_notional / total_equity;

        let (is_safe, reject_reason) = if !is_cash_sufficient {
            (
                false,
                Some(format!(
                    "两所可用现金不足: 需单边 ${:.2}, 弱侧仅剩 ${:.2}",
                    margin_req_per_leg, min_free_cash
                )),
            )
        } else if skew_pct > self.max_skew_threshold_pct {
            (
                false,
                Some(format!(
                    "跨所资金严重失衡 (偏离度 {:.1}% > {:.1}%), 暂停开新仓以防弱侧爆仓",
                    skew_pct, self.max_skew_threshold_pct
                )),
            )
        } else {
            (true, None)
        };

        DynamicAllocationDecision {
            target_notional_usd: final_notional,
            margin_required_per_leg_usd: margin_req_per_leg,
            portfolio_utilization_pct: utilization_pct,
            effective_leverage: effective_lev,
            cross_exchange_skew_pct: skew_pct,
            is_safe,
            reject_reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_pro_rata_sizing_500_capital() {
        let allocator = CapitalAllocator::new(true, 15.0, 2.0, 200.0, 3);
        // 初始状态: 双所各 $250, 0 持仓
        // safe_single = 250 * 0.85 = 212.5. total_safe_notional = 425.0
        // 3 slots -> 425 / 3 = $141.666 per slot
        let d1 = allocator.calculate_slot_allocation(250.0, 250.0, 0.0, 0, None);
        assert!(d1.is_safe);
        assert!((d1.target_notional_usd - 141.666).abs() < 1.0);
        assert!((d1.margin_required_per_leg_usd - 70.83).abs() < 1.0);

        // 已开 1 仓 ($141.66), 剩余 2 槽
        let d2 = allocator.calculate_slot_allocation(250.0, 250.0, 141.66, 1, None);
        assert!(d2.is_safe);
        assert!((d2.target_notional_usd - 141.666).abs() < 1.0);

        // 已开 2 仓 ($283.33), 剩余 1 槽
        let d3 = allocator.calculate_slot_allocation(250.0, 250.0, 283.33, 2, None);
        assert!(d3.is_safe);
        assert!((d3.target_notional_usd - 141.666).abs() < 1.0);

        // 3 仓全满 ($425.00), 尝试开第 4 仓 -> 拒绝
        let d4 = allocator.calculate_slot_allocation(250.0, 250.0, 425.0, 3, None);
        assert!(!d4.is_safe);
    }

    #[test]
    fn test_cross_exchange_skew_guard() {
        let allocator = CapitalAllocator::new(true, 15.0, 2.0, 200.0, 3);
        // 严重失衡: BN $400, HL $100 -> skew = 300 / 500 = 60% > 35%
        let d = allocator.calculate_slot_allocation(400.0, 100.0, 0.0, 0, None);
        assert!(!d.is_safe);
        assert!(d.reject_reason.unwrap().contains("跨所资金严重失衡"));
    }
}

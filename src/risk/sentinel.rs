use crate::config::RiskConfig;
use crate::types::{ActiveArbitragePosition, ArbitrageOpportunity, PositionSide};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExitSignal {
    Hold,
    SpreadDecay {
        current_apr: f64,
        min_exit_apr: f64,
        reason: String,
    },
    SpreadInverted {
        current_apr: f64,
        reason: String,
    },
    BasisStopLoss {
        basis_pnl_bps: f64,
        max_loss_bps: f64,
        reason: String,
    },
    BasisTakeProfit {
        basis_pnl_bps: f64,
        min_tp_bps: f64,
        reason: String,
    },
    MaxDurationExceeded {
        holding_hours: f64,
        max_hours: f64,
        reason: String,
    },
    DeltaDriftCritical {
        delta_pct: f64,
        max_delta_pct: f64,
        reason: String,
    },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RiskAssessment {
    pub is_safe: bool,
    pub delta_drift_usd: f64,
    pub delta_drift_pct: f64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RiskSentinel {
    config: RiskConfig,
}

impl RiskSentinel {
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }

    /// 评估活跃仓位的 Delta 中性健康度
    #[allow(dead_code)]
    pub fn assess_position(&self, pos: &ActiveArbitragePosition) -> RiskAssessment {
        let mut warnings = Vec::new();
        let total_notional = pos.nominal_value_usd.max(1.0);
        let delta_drift_pct = (pos.net_delta_usd.abs() / total_notional) * 100.0;

        if delta_drift_pct > self.config.max_delta_drift_pct {
            let msg = format!(
                "⚠️ Delta Drift Alert [{}]: 净 Delta 为 ${:.2} ({:.2}% of notional, 阈值: {:.1}%)",
                pos.symbol, pos.net_delta_usd, delta_drift_pct, self.config.max_delta_drift_pct
            );
            warn!("{}", msg);
            warnings.push(msg);
        }

        let is_safe = warnings.is_empty();
        RiskAssessment {
            is_safe,
            delta_drift_usd: pos.net_delta_usd,
            delta_drift_pct,
            warnings,
        }
    }

    /// 核心判定: 动态评估持仓的生命周期并生成退出信号
    pub fn evaluate_position_exit(
        &self,
        pos: &ActiveArbitragePosition,
        current_opp: Option<&ArbitrageOpportunity>,
        live_bn_price: f64,
        live_hl_price: f64,
    ) -> ExitSignal {
        let notional = pos.nominal_value_usd.max(1.0);

        // 1. 基差盈亏测算 (Basis PnL)
        let hl_pnl = match pos.hyperliquid_side {
            PositionSide::Short => {
                (pos.hyperliquid_entry_price - live_hl_price) * pos.hyperliquid_qty
            }
            PositionSide::Long => {
                (live_hl_price - pos.hyperliquid_entry_price) * pos.hyperliquid_qty
            }
        };
        let bn_pnl = match pos.binance_side {
            PositionSide::Short => (pos.binance_entry_price - live_bn_price) * pos.binance_qty,
            PositionSide::Long => (live_bn_price - pos.binance_entry_price) * pos.binance_qty,
        };
        let basis_pnl_usd = hl_pnl + bn_pnl;
        let basis_pnl_bps = (basis_pnl_usd / notional) * 10_000.0;

        // 2. 持仓时长测算 (Holding duration in hours)
        let holding_hours = (Utc::now() - pos.opened_at).num_seconds() as f64 / 3600.0;

        // 3. 实时 Delta 敞口校验 (Unit Delta Imbalance Check)
        let bn_signed_qty = match pos.binance_side {
            PositionSide::Long => pos.binance_qty,
            PositionSide::Short => -pos.binance_qty,
        };
        let hl_signed_qty = match pos.hyperliquid_side {
            PositionSide::Long => pos.hyperliquid_qty,
            PositionSide::Short => -pos.hyperliquid_qty,
        };
        let net_unit_delta = bn_signed_qty + hl_signed_qty;
        let avg_price = if live_bn_price > 0.0 && live_hl_price > 0.0 {
            (live_bn_price + live_hl_price) / 2.0
        } else {
            pos.binance_entry_price
        };
        let delta_imbalance = net_unit_delta.abs() * avg_price;
        let delta_pct = (delta_imbalance / notional) * 100.0;

        if delta_pct > self.config.max_delta_drift_pct {
            return ExitSignal::DeltaDriftCritical {
                delta_pct,
                max_delta_pct: self.config.max_delta_drift_pct,
                reason: format!(
                    "净 Delta 敞口达到 {:.2}% (${:.2}, 数量偏离 {:.4}), 超过风控阈值 {:.1}%",
                    delta_pct, delta_imbalance, net_unit_delta, self.config.max_delta_drift_pct
                ),
            };
        }

        // 4. 基差止损保护 (Basis Stop Loss)
        if basis_pnl_bps < -self.config.stop_loss_basis_bps {
            return ExitSignal::BasisStopLoss {
                basis_pnl_bps,
                max_loss_bps: self.config.stop_loss_basis_bps,
                reason: format!(
                    "基差反向扩大导致浮亏 {:.2} bps (${:.3}), 触发止损阈值 {:.1} bps",
                    basis_pnl_bps, basis_pnl_usd, self.config.stop_loss_basis_bps
                ),
            };
        }

        // 5. 最大持仓时长超时平仓 (Max Duration Decay)
        if holding_hours >= self.config.max_holding_hours {
            return ExitSignal::MaxDurationExceeded {
                holding_hours,
                max_hours: self.config.max_holding_hours,
                reason: format!(
                    "持仓时长达到 {:.2}h, 超过系统预设最大持仓周期 {:.1}h",
                    holding_hours, self.config.max_holding_hours
                ),
            };
        }

        // 6. 利差衰减与逆转判定 (Spread Decay / Inversion)
        if let Some(opp) = current_opp {
            let current_effective_apr = match pos.hyperliquid_side {
                PositionSide::Short => opp.hyperliquid_apr_pct - opp.binance_apr_pct,
                PositionSide::Long => opp.binance_apr_pct - opp.hyperliquid_apr_pct,
            };

            if current_effective_apr < 0.0 {
                return ExitSignal::SpreadInverted {
                    current_apr: current_effective_apr,
                    reason: format!(
                        "跨所资金费率已反向倒挂至 {:.2}% APR, 立即平仓避免持续付息",
                        current_effective_apr
                    ),
                };
            }

            if current_effective_apr < self.config.min_exit_apr_pct {
                return ExitSignal::SpreadDecay {
                    current_apr: current_effective_apr,
                    min_exit_apr: self.config.min_exit_apr_pct,
                    reason: format!(
                        "利差已衰减至 {:.2}% APR (低于平仓退出线 {:.1}%)",
                        current_effective_apr, self.config.min_exit_apr_pct
                    ),
                };
            }
        }

        ExitSignal::Hold
    }

    /// 评估利差衰减与平仓信号 (Spread Decay Guard)
    #[allow(dead_code)]
    pub fn should_exit_position(&self, current_spread_apr: f64, min_exit_apr: f64) -> bool {
        current_spread_apr < min_exit_apr
    }
}

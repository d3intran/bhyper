use crate::config::RiskConfig;
use crate::types::ActiveArbitragePosition;
use tracing::warn;

#[allow(dead_code)]
pub struct RiskSentinel {
    config: RiskConfig,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RiskAssessment {
    pub is_safe: bool,
    pub delta_drift_usd: f64,
    pub delta_drift_pct: f64,
    pub warnings: Vec<String>,
}

#[allow(dead_code)]
impl RiskSentinel {
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }

    /// 评估活跃仓位的 Delta 中性健康度
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

    /// 评估利差衰减与平仓信号 (Spread Decay Guard)
    pub fn should_exit_position(&self, current_spread_apr: f64, min_exit_apr: f64) -> bool {
        current_spread_apr < min_exit_apr
    }
}

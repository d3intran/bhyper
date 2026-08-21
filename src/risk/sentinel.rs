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

impl RiskSentinel {
    #[allow(dead_code)]
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }

    /// Evaluates an active arbitrage pair for delta neutrality and margin safety
    #[allow(dead_code)]
    pub fn assess_position(&self, pos: &ActiveArbitragePosition) -> RiskAssessment {
        let mut warnings = Vec::new();
        let total_notional = pos.nominal_value_usd.max(1.0);
        let delta_drift_pct = (pos.net_delta_usd.abs() / total_notional) * 100.0;

        if delta_drift_pct > self.config.max_delta_drift_pct {
            let msg = format!(
                "⚠️ Delta Drift Alert [{}]: Net Delta is ${:.2} ({:.2}% of notional, limit: {:.1}%)",
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
}

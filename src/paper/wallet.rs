use crate::types::{CrossExchangeMarginAssessment, Exchange, ExchangeMarginHealth};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualAccount {
    pub exchange: Exchange,
    pub initial_cash_usd: f64,
    pub cash_balance_usd: f64,
    pub allocated_margin_usd: f64,
    pub unrealized_pnl_usd: f64,
    pub realized_pnl_usd: f64,
    pub total_funding_usd: f64,
    pub total_fees_paid_usd: f64,
}

#[allow(dead_code)]
impl VirtualAccount {
    pub fn new(exchange: Exchange, initial_cash: f64) -> Self {
        Self {
            exchange,
            initial_cash_usd: initial_cash,
            cash_balance_usd: initial_cash,
            allocated_margin_usd: 0.0,
            unrealized_pnl_usd: 0.0,
            realized_pnl_usd: 0.0,
            total_funding_usd: 0.0,
            total_fees_paid_usd: 0.0,
        }
    }

    #[inline]
    pub fn total_equity_usd(&self) -> f64 {
        self.cash_balance_usd + self.unrealized_pnl_usd
    }

    #[inline]
    pub fn free_margin_usd(&self) -> f64 {
        (self.total_equity_usd() - self.allocated_margin_usd).max(0.0)
    }

    #[inline]
    pub fn utilization_pct(&self) -> f64 {
        let eq = self.total_equity_usd();
        if eq > 0.0 {
            (self.allocated_margin_usd / eq) * 100.0
        } else {
            0.0
        }
    }

    pub fn lock_margin(&mut self, margin: f64) -> Result<()> {
        if self.free_margin_usd() < margin {
            bail!(
                "Insufficient virtual {} margin: Required ${:.2}, Free ${:.2}",
                self.exchange,
                margin,
                self.free_margin_usd()
            );
        }
        self.allocated_margin_usd += margin;
        Ok(())
    }

    pub fn release_margin(&mut self, margin: f64, realized_pnl: f64) {
        self.allocated_margin_usd = (self.allocated_margin_usd - margin).max(0.0);
        self.cash_balance_usd += realized_pnl;
        self.realized_pnl_usd += realized_pnl;
    }

    pub fn debit_fee(&mut self, fee: f64) {
        self.cash_balance_usd -= fee;
        self.total_fees_paid_usd += fee;
    }

    pub fn apply_funding(&mut self, payment: f64) {
        self.cash_balance_usd += payment;
        self.total_funding_usd += payment;
    }

    pub fn to_margin_health(&self) -> ExchangeMarginHealth {
        ExchangeMarginHealth {
            exchange: self.exchange,
            account_value_usd: self.total_equity_usd(),
            total_margin_used_usd: self.allocated_margin_usd,
            free_margin_usd: self.free_margin_usd(),
            margin_utilization_pct: self.utilization_pct(),
            min_liquidation_distance_pct: (100.0 - self.utilization_pct()).max(10.0),
            is_healthy: self.utilization_pct() < 80.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperDualWallet {
    pub binance: VirtualAccount,
    pub hyperliquid: VirtualAccount,
    pub initial_capital_usd: f64,
}

#[allow(dead_code)]
impl PaperDualWallet {
    pub fn new(initial_capital_usd: f64) -> Self {
        let half = initial_capital_usd / 2.0;
        Self {
            binance: VirtualAccount::new(Exchange::Binance, half),
            hyperliquid: VirtualAccount::new(Exchange::Hyperliquid, half),
            initial_capital_usd,
        }
    }

    pub fn total_equity_usd(&self) -> f64 {
        self.binance.total_equity_usd() + self.hyperliquid.total_equity_usd()
    }

    pub fn total_free_margin_usd(&self) -> f64 {
        self.binance.free_margin_usd() + self.hyperliquid.free_margin_usd()
    }

    pub fn total_realized_pnl_usd(&self) -> f64 {
        self.binance.realized_pnl_usd + self.hyperliquid.realized_pnl_usd
    }

    pub fn total_fees_paid_usd(&self) -> f64 {
        self.binance.total_fees_paid_usd + self.hyperliquid.total_fees_paid_usd
    }

    pub fn total_funding_income_usd(&self) -> f64 {
        self.binance.total_funding_usd + self.hyperliquid.total_funding_usd
    }

    pub fn can_allocate(&self, bn_margin: f64, hl_margin: f64) -> Result<()> {
        if self.binance.free_margin_usd() < bn_margin {
            bail!(
                "Binance virtual free margin insufficient: required ${:.2}, available ${:.2}",
                bn_margin,
                self.binance.free_margin_usd()
            );
        }
        if self.hyperliquid.free_margin_usd() < hl_margin {
            bail!(
                "Hyperliquid virtual free margin insufficient: required ${:.2}, available ${:.2}",
                hl_margin,
                self.hyperliquid.free_margin_usd()
            );
        }
        Ok(())
    }

    pub fn get_margin_assessment(&self, rebalance_threshold_pct: f64) -> CrossExchangeMarginAssessment {
        let bn_h = self.binance.to_margin_health();
        let hl_h = self.hyperliquid.to_margin_health();
        crate::state::StateStore::compute_rebalance_advisory(&bn_h, &hl_h, rebalance_threshold_pct)
    }
}

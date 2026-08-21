use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Exchange {
    Binance,
    Hyperliquid,
}

impl fmt::Display for Exchange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Exchange::Binance => write!(f, "Binance"),
            Exchange::Hyperliquid => write!(f, "Hyperliquid"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    Long,
    Short,
}

impl fmt::Display for PositionSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PositionSide::Long => write!(f, "LONG"),
            PositionSide::Short => write!(f, "SHORT"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum OrderType {
    Limit,
    Market,
    PostOnly, // Hyperliquid ALO (Add Liquidity Only) / Binance GTX
    Ioc,      // Immediate Or Cancel
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRateInfo {
    pub symbol: String,
    pub exchange: Exchange,
    pub mark_price: f64,
    pub index_price: f64,
    pub funding_rate: f64,
    pub funding_interval_hours: f64,
    pub annualized_apr_pct: f64,
    pub next_funding_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub symbol: String,
    pub binance_mark_price: f64,
    pub hyperliquid_mark_price: f64,
    pub price_spread_pct: f64,
    pub binance_rate_8h_pct: f64,
    pub hyperliquid_rate_1h_pct: f64,
    pub binance_apr_pct: f64,
    pub hyperliquid_apr_pct: f64,
    pub net_spread_apr_pct: f64,
    pub hyperliquid_side: PositionSide,
    pub binance_side: PositionSide,
    pub est_hourly_return_bps: f64,
    pub est_break_even_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolPrecisionInfo {
    pub symbol: String,
    pub binance_step_size: f64,
    pub binance_tick_size: f64,
    pub binance_min_qty: f64,
    pub binance_min_notional: f64,
    pub hyperliquid_sz_decimals: u32,
    pub hyperliquid_asset_index: u32,
    pub hyperliquid_min_notional: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignedQuantity {
    pub symbol: String,
    pub qty: f64,
    pub notional_usd: f64,
    pub binance_formatted_qty: String,
    pub hyperliquid_formatted_qty: String,
    pub is_aligned: bool,
    pub delta_imbalance_usd: f64,
    pub delta_imbalance_pct: f64,
    pub reject_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct OrderExecutionResult {
    pub exchange: Exchange,
    pub symbol: String,
    pub order_id: String,
    pub side: PositionSide,
    pub price: f64,
    pub requested_qty: f64,
    pub filled_qty: f64,
    pub is_filled: bool,
    pub fee_usd: f64,
    pub timestamp: DateTime<Utc>,
    pub raw_response: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveArbitragePosition {
    pub symbol: String,
    pub binance_side: PositionSide,
    pub binance_qty: f64,
    pub binance_entry_price: f64,
    pub hyperliquid_side: PositionSide,
    pub hyperliquid_qty: f64,
    pub hyperliquid_entry_price: f64,
    pub nominal_value_usd: f64,
    pub net_delta_usd: f64,
    pub entry_spread_apr: f64,
    pub current_spread_apr: f64,
    pub accumulated_funding_usd: f64,
    pub opened_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub is_closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct BalanceInfo {
    pub exchange: Exchange,
    pub asset: String,
    pub total_equity_usd: f64,
    pub available_margin_usd: f64,
    pub margin_usage_pct: f64,
}

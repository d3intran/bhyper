//! ⚡ BHyper: Ultra Low-Latency Binance x Hyperliquid Cross-Exchange Arbitrage Engine
//!
//! A high-performance, delta-neutral funding rate and basis arbitrage framework written in pure Rust.

pub mod binance;
pub mod config;
pub mod hyperliquid;
pub mod risk;
pub mod strategy;
pub mod telemetry;
pub mod types;

pub use binance::BinanceFuturesClient;
pub use config::Config;
pub use hyperliquid::HyperliquidClient;
pub use risk::{RiskAssessment, RiskSentinel};
pub use strategy::{ArbitrageScanner, ProfitTriggerEngine, TriggerDecision};
pub use telemetry::TelemetryNotifier;
pub use types::{ArbitrageOpportunity, Exchange, FundingRateInfo, PositionSide};

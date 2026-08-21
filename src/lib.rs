//! ⚡ BHyper: Ultra Low-Latency Binance x Hyperliquid Cross-Exchange Arbitrage Engine
//!
//! A high-performance, delta-neutral funding rate and basis arbitrage framework written in pure Rust.

pub mod binance;
pub mod config;
pub mod hyperliquid;
pub mod risk;
pub mod state;
pub mod strategy;
pub mod telemetry;
pub mod types;
pub mod ws;

pub use binance::BinanceFuturesClient;
pub use config::Config;
pub use hyperliquid::signing::HyperliquidSigner;
pub use hyperliquid::HyperliquidClient;
pub use risk::{ExitSignal, RiskAssessment, RiskSentinel};
pub use state::StateStore;
pub use strategy::{
    ArbitrageScanner, LotPrecisionMatcher, ProfitTriggerEngine, TriggerDecision, TwoLegExecutor,
};
pub use telemetry::TelemetryNotifier;
pub use types::{
    ActiveArbitragePosition, AlignedQuantity, ArbitrageOpportunity, Exchange, ExecutionMode,
    FundingRateInfo, OrderExecutionResult, OrderType, PositionSide, SymbolPrecisionInfo,
    TradeHistoryRecord,
};
pub use ws::{BinanceWsStream, HyperliquidWsStream, MarketDataCache};

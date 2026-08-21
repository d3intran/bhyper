use crate::journal::{
    FundingSettlementEvent, JournalEntry, TradeCloseFillEvent, TradeIntentEvent, TradeJournal,
    TradeOpenFillEvent,
};
use crate::paper::wallet::PaperDualWallet;
use crate::strategy::trigger::TriggerDecision;
use crate::types::{
    ActiveArbitragePosition, ArbitrageOpportunity, Exchange, ExecutionMode, PositionSide,
    SymbolPrecisionInfo,
};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperPosition {
    pub symbol: String,
    pub binance_side: PositionSide,
    pub binance_qty: f64,
    pub binance_entry_price: f64,
    pub binance_entry_fee_usd: f64,
    pub hyperliquid_side: PositionSide,
    pub hyperliquid_qty: f64,
    pub hyperliquid_entry_price: f64,
    pub hyperliquid_entry_fee_usd: f64,
    pub nominal_value_usd: f64,
    pub entry_spread_apr: f64,
    pub current_spread_apr: f64,
    pub opened_at: DateTime<Utc>,
    pub last_hl_funding_time: DateTime<Utc>,
    pub last_bn_funding_time: DateTime<Utc>,
    pub accumulated_hl_funding_usd: f64,
    pub accumulated_bn_funding_usd: f64,
    pub total_funding_usd: f64,
    pub funding_ticks_count: u32,
    pub is_closed: bool,
    pub closed_at: Option<DateTime<Utc>>,
    pub realized_pnl_usd: Option<f64>,
}

impl PaperPosition {
    pub fn to_active_position(&self) -> ActiveArbitragePosition {
        ActiveArbitragePosition {
            symbol: self.symbol.clone(),
            binance_side: self.binance_side,
            binance_qty: self.binance_qty,
            binance_entry_price: self.binance_entry_price,
            hyperliquid_side: self.hyperliquid_side,
            hyperliquid_qty: self.hyperliquid_qty,
            hyperliquid_entry_price: self.hyperliquid_entry_price,
            nominal_value_usd: self.nominal_value_usd,
            net_delta_usd: 0.0,
            entry_spread_apr: self.entry_spread_apr,
            current_spread_apr: self.current_spread_apr,
            accumulated_funding_usd: self.total_funding_usd,
            opened_at: self.opened_at,
            last_updated_at: Utc::now(),
            is_closed: self.is_closed,
            closed_at: self.closed_at,
            realized_pnl_usd: self.realized_pnl_usd,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperTradingState {
    pub wallet: PaperDualWallet,
    pub active_positions: HashMap<String, PaperPosition>,
    pub total_trades_count: usize,
    pub last_updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PaperTradingStore {
    path: PathBuf,
    pub state: PaperTradingState,
    journal: Arc<TradeJournal>,
}

impl PaperTradingStore {
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/bhyper/paper_state.json")
    }

    pub fn load_or_create(path_opt: Option<PathBuf>, initial_capital_usd: f64) -> Result<Self> {
        let path = path_opt.unwrap_or_else(Self::default_path);
        let journal = Arc::new(TradeJournal::new(None));

        if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read paper state from {}", path.display()))?;
            let state: PaperTradingState = serde_json::from_str(&content).with_context(|| {
                format!("Failed to parse paper state JSON from {}", path.display())
            })?;
            info!(
                "🧪 Loaded Paper Trading State: Total Equity ${:.2}, {} active positions",
                state.wallet.total_equity_usd(),
                state.active_positions.len()
            );
            Ok(Self {
                path,
                state,
                journal,
            })
        } else {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let state = PaperTradingState {
                wallet: PaperDualWallet::new(initial_capital_usd),
                active_positions: HashMap::new(),
                total_trades_count: 0,
                last_updated_at: Utc::now(),
            };
            let mut store = Self {
                path,
                state,
                journal,
            };
            store.save()?;
            info!(
                "🧪 Initialized fresh Paper Trading State with ${:.2} virtual capital",
                initial_capital_usd
            );
            Ok(store)
        }
    }

    pub fn save(&mut self) -> Result<()> {
        self.state.last_updated_at = Utc::now();
        let json_str = serde_json::to_string_pretty(&self.state)
            .context("Failed to serialize paper trading state")?;

        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let tmp_path = self.path.with_extension("tmp");
        fs::write(&tmp_path, json_str)?;
        fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }

    pub fn reset(&mut self, initial_capital_usd: f64) -> Result<()> {
        self.state.wallet = PaperDualWallet::new(initial_capital_usd);
        self.state.active_positions.clear();
        self.state.total_trades_count = 0;
        self.save()
    }
}

pub struct PaperExecutionEngine {
    pub store: PaperTradingStore,
    journal: Arc<TradeJournal>,
}

impl PaperExecutionEngine {
    pub fn new(store: PaperTradingStore) -> Self {
        let journal = store.journal.clone();
        Self { store, journal }
    }

    /// 严格模拟开仓建仓 (Simulate Fill-Verified Two-Leg Opening)
    pub fn simulate_open(
        &mut self,
        opp: &ArbitrageOpportunity,
        decision: &TriggerDecision,
        _precision: &SymbolPrecisionInfo,
        execution_mode: ExecutionMode,
    ) -> Result<PaperPosition> {
        let aligned = match &decision.aligned_quantity {
            Some(a) => a,
            None => bail!("Missing aligned quantity for paper trade"),
        };

        let notional = aligned.notional_usd;
        let margin_req = notional * 0.5; // 2x leverage margin requirement

        // 1. 验证双所虚拟保证金可用性
        self.store
            .state
            .wallet
            .can_allocate(margin_req, margin_req)?;

        // 2. 模拟真实执行滑点与手续费
        // Maker-Taker: HL Maker 0.00% fee, BN Taker 0.04% fee + slippage
        // Taker-Taker: HL Taker 0.035% fee, BN Taker 0.04% fee + slippage
        let (hl_fee_rate, hl_mode_str) = match execution_mode {
            ExecutionMode::MakerTaker => (0.0000, "MAKER_POST_ONLY"),
            ExecutionMode::TakerTaker => (0.00035, "TAKER_IOC"),
        };
        let bn_fee_rate = 0.0004; // Binance Taker 0.04%

        // Slippage based on liquidity tier
        let slippage_bps = match opp.liquidity_tier.as_str() {
            "TIER_1_PRIME" => 0.5,
            "TIER_2_LIQUID" => 1.5,
            "TIER_3_MID" => 3.0,
            _ => 5.0,
        };
        let slip_factor = slippage_bps / 10_000.0;

        let hl_actual_price = opp.hyperliquid_mark_price;
        let bn_actual_price = match decision.bn_side {
            PositionSide::Long => opp.binance_mark_price * (1.0 + slip_factor),
            PositionSide::Short => opp.binance_mark_price * (1.0 - slip_factor),
        };

        let hl_fee_usd = notional * hl_fee_rate;
        let bn_fee_usd = notional * bn_fee_rate;

        // 3. 锁定虚拟账户保证金并扣除开仓交易费
        self.store.state.wallet.binance.lock_margin(margin_req)?;
        self.store
            .state
            .wallet
            .hyperliquid
            .lock_margin(margin_req)?;
        self.store.state.wallet.binance.debit_fee(bn_fee_usd);
        self.store.state.wallet.hyperliquid.debit_fee(hl_fee_usd);

        let trade_id = format!("{}-paper-{}", opp.symbol, Utc::now().timestamp_millis());

        let pos = PaperPosition {
            symbol: opp.symbol.clone(),
            binance_side: decision.bn_side,
            binance_qty: aligned.qty,
            binance_entry_price: bn_actual_price,
            binance_entry_fee_usd: bn_fee_usd,
            hyperliquid_side: decision.hl_side,
            hyperliquid_qty: aligned.qty,
            hyperliquid_entry_price: hl_actual_price,
            hyperliquid_entry_fee_usd: hl_fee_usd,
            nominal_value_usd: notional,
            entry_spread_apr: opp.net_spread_apr_pct,
            current_spread_apr: opp.net_spread_apr_pct,
            opened_at: Utc::now(),
            last_hl_funding_time: Utc::now(),
            last_bn_funding_time: Utc::now(),
            accumulated_hl_funding_usd: 0.0,
            accumulated_bn_funding_usd: 0.0,
            total_funding_usd: 0.0,
            funding_ticks_count: 0,
            is_closed: false,
            closed_at: None,
            realized_pnl_usd: None,
        };

        // 4. 记录全息交易流水到 Journal
        let intent_event = TradeIntentEvent {
            id: format!("intent-{}", trade_id),
            symbol: opp.symbol.clone(),
            timestamp: Utc::now(),
            is_paper: true,
            hyperliquid_side: decision.hl_side,
            binance_side: decision.bn_side,
            hyperliquid_apr_pct: opp.hyperliquid_apr_pct,
            binance_apr_pct: opp.binance_apr_pct,
            net_spread_apr_pct: opp.net_spread_apr_pct,
            projected_1h_net_bps: decision.single_cycle_income_bps,
            projected_4h_net_bps: decision.projected_4h_net_bps,
            target_notional_usd: notional,
            aligned_qty: aligned.qty,
            friction_cost_bps: decision.total_friction_cost_bps,
            est_hourly_return_bps: opp.est_hourly_return_bps,
            reason: format!("Deterministic profit trigger matched ({})", execution_mode),
        };
        let _ = self.journal.append(&JournalEntry::Intent(intent_event));

        let open_event = TradeOpenFillEvent {
            id: trade_id.clone(),
            intent_id: format!("intent-{}", trade_id),
            symbol: opp.symbol.clone(),
            timestamp: Utc::now(),
            is_paper: true,
            hyperliquid_side: decision.hl_side,
            hyperliquid_qty: aligned.qty,
            hyperliquid_price: hl_actual_price,
            hyperliquid_fee_usd: hl_fee_usd,
            hyperliquid_mode: hl_mode_str.to_string(),
            binance_side: decision.bn_side,
            binance_qty: aligned.qty,
            binance_price: bn_actual_price,
            binance_fee_usd: bn_fee_usd,
            binance_mode: "TAKER_MARKET".to_string(),
            total_notional_usd: notional,
            entry_price_spread_bps: ((hl_actual_price - bn_actual_price) / bn_actual_price)
                * 10_000.0,
            total_open_fees_usd: hl_fee_usd + bn_fee_usd,
            execution_latency_ms: 12,
        };
        let _ = self.journal.append(&JournalEntry::OpenFill(open_event));

        self.store
            .state
            .active_positions
            .insert(opp.symbol.clone(), pos.clone());
        self.store.state.total_trades_count += 1;
        self.store.save()?;

        info!(
            "🧪 [PAPER TRADING OPEN] Position created on {}: Notional ${:.2} (HL: {} ${:.4} | BN: {} ${:.4})",
            opp.symbol, notional, decision.hl_side, hl_actual_price, decision.bn_side, bn_actual_price
        );

        Ok(pos)
    }

    /// 精确时钟资金费率结算与自动流水记账 (Deterministic Funding Cashflow Accrual)
    pub fn accrue_funding_payments(
        &mut self,
        opportunities: &[ArbitrageOpportunity],
    ) -> Result<Vec<FundingSettlementEvent>> {
        let now = Utc::now();
        let opp_map: HashMap<String, &ArbitrageOpportunity> = opportunities
            .iter()
            .map(|o| (o.symbol.clone(), o))
            .collect();

        let mut events = Vec::new();
        let symbols: Vec<String> = self.store.state.active_positions.keys().cloned().collect();

        for sym in symbols {
            let opp = match opp_map.get(&sym) {
                Some(o) => *o,
                None => continue,
            };

            let pos = match self.store.state.active_positions.get_mut(&sym) {
                Some(p) => p,
                None => continue,
            };

            // 1. Hyperliquid 1-Hour Settlement Check (Settles every hour)
            let hl_elapsed = now.signed_duration_since(pos.last_hl_funding_time);
            if hl_elapsed >= Duration::hours(1) {
                let rate_1h = opp.hyperliquid_rate_1h_pct / 100.0;
                let hl_notional = pos.hyperliquid_qty * opp.hyperliquid_mark_price;
                // If Short: receives positive funding rate, pays negative
                // If Long: pays positive funding rate, receives negative
                let hl_cashflow = match pos.hyperliquid_side {
                    PositionSide::Short => hl_notional * rate_1h,
                    PositionSide::Long => -hl_notional * rate_1h,
                };

                self.store
                    .state
                    .wallet
                    .hyperliquid
                    .apply_funding(hl_cashflow);
                pos.accumulated_hl_funding_usd += hl_cashflow;
                pos.total_funding_usd += hl_cashflow;
                pos.last_hl_funding_time = now;
                pos.funding_ticks_count += 1;

                let event = FundingSettlementEvent {
                    id: format!("fund-hl-{}-{}", sym, now.timestamp_millis()),
                    position_id: format!("{}-paper", sym),
                    symbol: sym.clone(),
                    timestamp: now,
                    is_paper: true,
                    exchange: Exchange::Hyperliquid,
                    side: pos.hyperliquid_side,
                    rate_bps: rate_1h * 10_000.0,
                    annualized_apr_pct: opp.hyperliquid_apr_pct,
                    mark_price: opp.hyperliquid_mark_price,
                    position_qty: pos.hyperliquid_qty,
                    notional_usd: hl_notional,
                    funding_payment_usd: hl_cashflow,
                    cumulative_funding_usd: pos.accumulated_hl_funding_usd,
                };
                let _ = self.journal.append(&JournalEntry::Funding(event.clone()));
                events.push(event);

                info!(
                    "💰 [PAPER FUNDING ACCRUAL] Hyperliquid 1h settlement on {}: Payment ${:.4} (Rate: {:.2} bps)",
                    sym, hl_cashflow, rate_1h * 10_000.0
                );
            }

            // 2. Binance 8-Hour Settlement Check (Settles at 00:00, 08:00, 16:00 UTC)
            let bn_elapsed = now.signed_duration_since(pos.last_bn_funding_time);
            let is_settlement_hour =
                now.minute() == 0 && (now.hour() == 0 || now.hour() == 8 || now.hour() == 16);
            if bn_elapsed >= Duration::hours(8)
                || (is_settlement_hour && bn_elapsed >= Duration::minutes(30))
            {
                let rate_8h = opp.binance_rate_8h_pct / 100.0;
                let bn_notional = pos.binance_qty * opp.binance_mark_price;
                let bn_cashflow = match pos.binance_side {
                    PositionSide::Short => bn_notional * rate_8h,
                    PositionSide::Long => -bn_notional * rate_8h,
                };

                self.store.state.wallet.binance.apply_funding(bn_cashflow);
                pos.accumulated_bn_funding_usd += bn_cashflow;
                pos.total_funding_usd += bn_cashflow;
                pos.last_bn_funding_time = now;
                pos.funding_ticks_count += 1;

                let event = FundingSettlementEvent {
                    id: format!("fund-bn-{}-{}", sym, now.timestamp_millis()),
                    position_id: format!("{}-paper", sym),
                    symbol: sym.clone(),
                    timestamp: now,
                    is_paper: true,
                    exchange: Exchange::Binance,
                    side: pos.binance_side,
                    rate_bps: rate_8h * 10_000.0,
                    annualized_apr_pct: opp.binance_apr_pct,
                    mark_price: opp.binance_mark_price,
                    position_qty: pos.binance_qty,
                    notional_usd: bn_notional,
                    funding_payment_usd: bn_cashflow,
                    cumulative_funding_usd: pos.accumulated_bn_funding_usd,
                };
                let _ = self.journal.append(&JournalEntry::Funding(event.clone()));
                events.push(event);

                info!(
                    "💰 [PAPER FUNDING ACCRUAL] Binance 8h settlement on {}: Payment ${:.4} (Rate: {:.2} bps)",
                    sym, bn_cashflow, rate_8h * 10_000.0
                );
            }
        }

        if !events.is_empty() {
            self.store.save()?;
        }

        Ok(events)
    }

    /// 严格模拟平仓与收益全息归因 (Simulate Closing & Full PnL Attribution)
    pub fn simulate_close(
        &mut self,
        symbol: &str,
        live_bn_price: f64,
        live_hl_price: f64,
        exit_reason: &str,
    ) -> Result<Option<TradeCloseFillEvent>> {
        let pos = match self.store.state.active_positions.remove(symbol) {
            Some(p) => p,
            None => return Ok(None),
        };

        let notional = pos.nominal_value_usd;
        let margin_released = notional * 0.5;

        // Closing transaction fees
        let hl_exit_fee_usd = notional * 0.00035; // Taker 0.035%
        let bn_exit_fee_usd = notional * 0.00040; // Taker 0.040%
        let total_exit_fees_usd = hl_exit_fee_usd + bn_exit_fee_usd;
        let total_roundtrip_fees_usd =
            pos.binance_entry_fee_usd + pos.hyperliquid_entry_fee_usd + total_exit_fees_usd;

        // Basis PnL
        let hl_basis_pnl = match pos.hyperliquid_side {
            PositionSide::Long => {
                pos.hyperliquid_qty * (live_hl_price - pos.hyperliquid_entry_price)
            }
            PositionSide::Short => {
                pos.hyperliquid_qty * (pos.hyperliquid_entry_price - live_hl_price)
            }
        };

        let bn_basis_pnl = match pos.binance_side {
            PositionSide::Long => pos.binance_qty * (live_bn_price - pos.binance_entry_price),
            PositionSide::Short => pos.binance_qty * (pos.binance_entry_price - live_bn_price),
        };

        let gross_basis_pnl_usd = hl_basis_pnl + bn_basis_pnl;
        let gross_funding_earned_usd = pos.total_funding_usd;

        let net_realized_pnl_usd =
            gross_basis_pnl_usd + gross_funding_earned_usd - total_roundtrip_fees_usd;
        let net_return_bps = (net_realized_pnl_usd / notional) * 10_000.0;
        let return_on_capital_pct = (net_realized_pnl_usd / notional) * 100.0;
        let duration_secs = (Utc::now() - pos.opened_at).num_seconds().max(1) as u64;

        // Release margin and apply PnL
        self.store
            .state
            .wallet
            .binance
            .release_margin(margin_released, bn_basis_pnl);
        self.store
            .state
            .wallet
            .hyperliquid
            .release_margin(margin_released, hl_basis_pnl);
        self.store.state.wallet.binance.debit_fee(bn_exit_fee_usd);
        self.store
            .state
            .wallet
            .hyperliquid
            .debit_fee(hl_exit_fee_usd);

        let close_event = TradeCloseFillEvent {
            id: format!("close-{}-{}", symbol, Utc::now().timestamp_millis()),
            open_trade_id: format!("{}-paper", symbol),
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            is_paper: true,
            holding_duration_secs: duration_secs,
            exit_reason: exit_reason.to_string(),
            hyperliquid_exit_price: live_hl_price,
            hyperliquid_exit_fee_usd: hl_exit_fee_usd,
            binance_exit_price: live_bn_price,
            binance_exit_fee_usd: bn_exit_fee_usd,
            total_exit_fees_usd,
            total_roundtrip_fees_usd,
            gross_basis_pnl_usd,
            gross_funding_earned_usd,
            net_realized_pnl_usd,
            net_return_bps,
            return_on_capital_pct,
        };

        let _ = self
            .journal
            .append(&JournalEntry::CloseFill(close_event.clone()));
        self.store.save()?;

        info!(
            "🧪 [PAPER TRADING CLOSE] Closed {} | Net PnL: ${:.4} ({:.2} bps, ROC: {:.2}%) | Gross Funding: ${:.4}, Basis PnL: ${:.4}, Fees: ${:.4} (Held: {:.1}h)",
            symbol,
            net_realized_pnl_usd,
            net_return_bps,
            return_on_capital_pct,
            gross_funding_earned_usd,
            gross_basis_pnl_usd,
            total_roundtrip_fees_usd,
            duration_secs as f64 / 3600.0
        );

        Ok(Some(close_event))
    }
}

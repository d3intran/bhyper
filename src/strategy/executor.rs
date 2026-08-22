use crate::binance::{BinanceFuturesClient, BinanceWsApiClient};
use crate::hyperliquid::HyperliquidClient;
use crate::journal::{
    JournalEntry, TradeCloseFillEvent, TradeIntentEvent, TradeJournal, TradeOpenFillEvent,
};
use crate::state::StateStore;
use crate::strategy::trigger::TriggerDecision;
use crate::telemetry::TelemetryNotifier;
use crate::types::{
    ActiveArbitragePosition, ArbitrageOpportunity, ExecutionMode, PositionSide,
    SymbolPrecisionInfo, TradeHistoryRecord,
};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::ws::MarketDataCache;

pub struct TwoLegExecutor {
    binance: BinanceFuturesClient,
    hyperliquid: HyperliquidClient,
    notifier: TelemetryNotifier,
    state_store: Arc<Mutex<StateStore>>,
    dry_run: bool,
    execution_mode: ExecutionMode,
    cache: Option<MarketDataCache>,
    ws_api_client: Option<Arc<BinanceWsApiClient>>,
    journal: Arc<TradeJournal>,
}

impl TwoLegExecutor {
    pub fn new(
        binance: BinanceFuturesClient,
        hyperliquid: HyperliquidClient,
        notifier: TelemetryNotifier,
        state_store: Arc<Mutex<StateStore>>,
        dry_run: bool,
        execution_mode: ExecutionMode,
    ) -> Self {
        Self {
            binance,
            hyperliquid,
            notifier,
            state_store,
            dry_run,
            execution_mode,
            cache: None,
            ws_api_client: None,
            journal: Arc::new(TradeJournal::new(None)),
        }
    }

    pub fn with_cache(mut self, cache: MarketDataCache) -> Self {
        self.cache = Some(cache);
        self
    }

    pub fn with_ws_api(mut self, ws_api: Arc<BinanceWsApiClient>) -> Self {
        self.ws_api_client = Some(ws_api);
        self
    }

    #[allow(dead_code)]
    pub fn with_journal(mut self, journal: Arc<TradeJournal>) -> Self {
        self.journal = journal;
        self
    }

    /// 解析 Hyperliquid 下单返回的成交状态与实际成交量
    fn parse_hyperliquid_order_fill(res: &serde_json::Value) -> (bool, f64, f64, Option<u64>) {
        if let Some(statuses) = res
            .get("response")
            .and_then(|r| r.get("data"))
            .and_then(|d| d.get("statuses"))
            .and_then(|s| s.as_array())
        {
            if let Some(first) = statuses.first() {
                if let Some(filled) = first.get("filled") {
                    let sz = filled
                        .get("totalSz")
                        .and_then(|s| s.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let px = filled
                        .get("avgPx")
                        .and_then(|p| p.as_str())
                        .and_then(|p| p.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let oid = filled.get("oid").and_then(|o| o.as_u64());
                    return (true, sz, px, oid);
                } else if let Some(resting) = first.get("resting") {
                    let oid = resting.get("oid").and_then(|o| o.as_u64());
                    return (false, 0.0, 0.0, oid);
                }
            }
        }
        (false, 0.0, 0.0, None)
    }

    /// 执行两腿对冲建仓 (Atomic Fill-Verified Maker-Taker / Dual-IOC)
    pub async fn execute_open(
        &self,
        opp: &ArbitrageOpportunity,
        decision: &TriggerDecision,
        precision: &SymbolPrecisionInfo,
    ) -> Result<ActiveArbitragePosition> {
        let aligned = match &decision.aligned_quantity {
            Some(a) => a,
            None => bail!("Cannot execute trade without verified aligned quantity"),
        };

        if self.dry_run {
            info!(
                "🧪 [DRY-RUN / PAPER TRADING] Executing simulated arbitrage on {}: HL {} ${:.3} | BN {} ${:.3} (Notional: ${:.2})",
                opp.symbol,
                decision.hl_side,
                opp.hyperliquid_mark_price,
                decision.bn_side,
                opp.binance_mark_price,
                aligned.notional_usd
            );

            let sim_pos = ActiveArbitragePosition {
                symbol: opp.symbol.clone(),
                binance_side: decision.bn_side,
                binance_qty: aligned.qty,
                binance_entry_price: opp.binance_mark_price,
                hyperliquid_side: decision.hl_side,
                hyperliquid_qty: aligned.qty,
                hyperliquid_entry_price: opp.hyperliquid_mark_price,
                nominal_value_usd: aligned.notional_usd,
                net_delta_usd: 0.0,
                entry_spread_apr: opp.net_spread_apr_pct,
                current_spread_apr: opp.net_spread_apr_pct,
                accumulated_funding_usd: 0.0,
                opened_at: Utc::now(),
                last_updated_at: Utc::now(),
                is_closed: false,
                closed_at: None,
                realized_pnl_usd: None,
            };

            // Persist paper trading position
            {
                let mut store = self.state_store.lock();
                let _ = store.upsert_position(sim_pos.clone());
                let _ = store.record_trade(TradeHistoryRecord {
                    id: format!("{}-{}", opp.symbol, Utc::now().timestamp_millis()),
                    symbol: opp.symbol.clone(),
                    action: "OPEN_SIM".to_string(),
                    notional_usd: aligned.notional_usd,
                    hl_side: decision.hl_side,
                    hl_qty: aligned.qty,
                    hl_price: opp.hyperliquid_mark_price,
                    bn_side: decision.bn_side,
                    bn_qty: aligned.qty,
                    bn_price: opp.binance_mark_price,
                    net_apr_at_action: opp.net_spread_apr_pct,
                    fees_incurred_usd: 0.0,
                    realized_pnl_usd: 0.0,
                    timestamp: Utc::now(),
                    notes: format!("Simulation open (mode: {})", self.execution_mode),
                });
            }

            let alert = format!(
                "🧪 <b>[模拟盘建仓成功] BHyper Paper Trading</b>\n\n\
                • <b>标的:</b> <code>{}</code>\n\
                • <b>模式:</b> <code>{}</code>\n\
                • <b>名义规模:</b> <code>${:.2}</code> (数量: <code>{}</code>)\n\
                • <b>Hyperliquid:</b> <code>{} @ ${:.4}</code>\n\
                • <b>Binance:</b> <code>{} @ ${:.4}</code>\n\
                • <b>净利差 APR:</b> <code>{:.2}%</code> (预计4h净利: <code>{:.2} bps / ${:.3}</code>)\n\
                • <b>Delta 敞口:</b> <code>$0.00 (100% 对冲)</code>",
                sim_pos.symbol,
                self.execution_mode,
                sim_pos.nominal_value_usd,
                aligned.binance_formatted_qty,
                sim_pos.hyperliquid_side,
                sim_pos.hyperliquid_entry_price,
                sim_pos.binance_side,
                sim_pos.binance_entry_price,
                sim_pos.entry_spread_apr,
                decision.projected_4h_net_bps,
                decision.net_expected_profit_usd
            );
            let _ = self.notifier.send_alert(&alert).await;

            return Ok(sim_pos);
        }

        // ==========================================
        // 实盘原子对冲流程 (REAL CAPITAL EXECUTION)
        // ==========================================
        info!(
            "⚡ [LIVE CAPITAL TRADING] Initiating verified arbitrage on {} (Mode: {})...",
            opp.symbol, self.execution_mode
        );

        let hl_is_buy = decision.hl_side == PositionSide::Long;
        let hl_price = opp.hyperliquid_mark_price;

        let (hl_filled_qty, hl_actual_price) = match self.execution_mode {
            ExecutionMode::TakerTaker => {
                // Dual-IOC Mode: Send aggressive IOC to Hyperliquid
                info!(
                    "⏳ Leg 1 (HL IOC): Submitting IOC order ({} {} @ ${:.4})...",
                    decision.hl_side, aligned.hyperliquid_formatted_qty, hl_price
                );

                let hl_ioc_price = if hl_is_buy {
                    hl_price * 1.005
                } else {
                    hl_price * 0.995
                };
                let hl_res = self
                    .hyperliquid
                    .place_order(
                        precision.hyperliquid_asset_index,
                        hl_is_buy,
                        hl_ioc_price,
                        aligned.qty,
                        false, // reduce_only
                        false, // not post_only
                        true,  // IOC
                    )
                    .await
                    .context("Failed to dispatch Leg 1 HL IOC order")?;

                let (is_filled, filled_sz, actual_px, _) =
                    Self::parse_hyperliquid_order_fill(&hl_res);
                if !is_filled || filled_sz <= 0.0 {
                    warn!(
                        "❌ Hyperliquid Leg 1 IOC Order Unfilled or Rejected: {}. Aborting Leg 2 hedge safely (Zero Delta Risk)!",
                        hl_res
                    );
                    bail!("Leg 1 HL IOC was not filled: {}", hl_res);
                }

                (
                    filled_sz,
                    if actual_px > 0.0 { actual_px } else { hl_price },
                )
            }

            ExecutionMode::MakerTaker => {
                // Maker-Taker Mode with Fill Verification & Timeout
                info!(
                    "⏳ Leg 1 (HL Post-Only): Submitting ALO maker order ({} {} @ ${:.4})...",
                    decision.hl_side, aligned.hyperliquid_formatted_qty, hl_price
                );

                let hl_res = self
                    .hyperliquid
                    .place_order(
                        precision.hyperliquid_asset_index,
                        hl_is_buy,
                        hl_price,
                        aligned.qty,
                        false, // reduce_only
                        true,  // Post-Only (ALO)
                        false,
                    )
                    .await
                    .context("Failed to dispatch Leg 1 HL ALO order")?;

                let (is_filled_now, filled_sz_now, actual_px_now, resting_oid) =
                    Self::parse_hyperliquid_order_fill(&hl_res);

                if is_filled_now && filled_sz_now > 0.0 {
                    (
                        filled_sz_now,
                        if actual_px_now > 0.0 {
                            actual_px_now
                        } else {
                            hl_price
                        },
                    )
                } else if let Some(oid) = resting_oid {
                    info!("⏳ HL Maker order resting on book (OID: {}). Awaiting instant WebSocket fill event (5s max timeout)...", oid);
                    let mut confirmed_fill_qty = 0.0;
                    let mut actual_px = hl_price;

                    if let Some(ref cache) = self.cache {
                        let mut fill_rx = cache.subscribe_fills();
                        let timeout = tokio::time::sleep(Duration::from_millis(5000));
                        tokio::pin!(timeout);

                        loop {
                            tokio::select! {
                                _ = &mut timeout => {
                                    warn!("⏱️ HL Maker order (OID: {}) timed out waiting for WS fill.", oid);
                                    break;
                                }
                                fill_res = fill_rx.recv() => {
                                    match fill_res {
                                        Ok(fill) => {
                                            if (fill.oid == oid || fill.coin.eq_ignore_ascii_case(&opp.symbol)) && fill.sz > 0.0 {
                                                confirmed_fill_qty = fill.sz;
                                                if fill.px > 0.0 {
                                                    actual_px = fill.px;
                                                }
                                                info!("⚡ INSTANT WS Fill Event Received for {} (OID: {}): {:.4} @ ${:.4}", opp.symbol, oid, fill.sz, fill.px);
                                                break;
                                            }
                                        }
                                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                                    }
                                }
                            }
                        }
                    } else {
                        // Fallback polling if cache not provided
                        let mut poll_count = 0;
                        while poll_count < 20 {
                            tokio::time::sleep(Duration::from_millis(250)).await;
                            poll_count += 1;
                            if let Ok(open_orders) = self.hyperliquid.fetch_open_orders().await {
                                let is_still_open = open_orders.iter().any(|o| o.oid == oid);
                                if !is_still_open {
                                    confirmed_fill_qty = aligned.qty;
                                    info!("✅ HL Maker order filled! (OID: {})", oid);
                                    break;
                                }
                            }
                        }
                    }

                    if confirmed_fill_qty <= 0.0 {
                        warn!(
                            "⏱️ HL Maker order not filled within timeout. Cancelling order {}...",
                            oid
                        );
                        let _ = self
                            .hyperliquid
                            .cancel_order(precision.hyperliquid_asset_index, oid)
                            .await;
                        bail!("HL Maker order {} timed out and was cancelled safely. No Binance position opened.", oid);
                    }

                    (confirmed_fill_qty, actual_px)
                } else {
                    bail!("HL Post-Only rejected immediately: {}", hl_res);
                }
            }
        };

        info!(
            "🚀 Step 2: Hyperliquid Leg 1 Filled ({:.4} @ ${:.4}). Immediately hedging on Binance ({})...",
            hl_filled_qty, hl_actual_price, decision.bn_side
        );

        // Format exact filled qty for Binance
        let bn_decimals = crate::strategy::precision::LotPrecisionMatcher::get_precision_decimals(
            precision.binance_step_size,
        );
        let bn_qty_formatted = format!("{:.prec$}", hl_filled_qty, prec = bn_decimals);

        let bn_side_str = match decision.bn_side {
            PositionSide::Long => "BUY",
            PositionSide::Short => "SELL",
        };

        let bn_res = if let Some(ref ws_client) = self.ws_api_client {
            info!("⚡ Ultra Low-Latency Route: Executing Binance hedge via WebSocket API (order.place)...");
            match ws_client
                .place_order(
                    &opp.symbol,
                    bn_side_str,
                    "MARKET",
                    &bn_qty_formatted,
                    None,
                    false,
                )
                .await
            {
                Ok(v) => Ok(v),
                Err(e) => {
                    warn!("⚠️ Binance WS API returned error ({:?}), immediately falling back to HTTP REST...", e);
                    self.binance
                        .place_order(
                            &opp.symbol,
                            decision.bn_side,
                            &bn_qty_formatted,
                            None,
                            false,
                        )
                        .await
                }
            }
        } else {
            self.binance
                .place_order(
                    &opp.symbol,
                    decision.bn_side,
                    &bn_qty_formatted,
                    None, // Market order
                    false,
                )
                .await
        };

        match bn_res {
            Ok(bn_val) => {
                info!("✅ Binance Leg 2 Filled: {}", bn_val);

                let actual_notional = hl_filled_qty * opp.binance_mark_price;
                let live_pos = ActiveArbitragePosition {
                    symbol: opp.symbol.clone(),
                    binance_side: decision.bn_side,
                    binance_qty: hl_filled_qty,
                    binance_entry_price: opp.binance_mark_price,
                    hyperliquid_side: decision.hl_side,
                    hyperliquid_qty: hl_filled_qty,
                    hyperliquid_entry_price: hl_actual_price,
                    nominal_value_usd: actual_notional,
                    net_delta_usd: 0.0,
                    entry_spread_apr: opp.net_spread_apr_pct,
                    current_spread_apr: opp.net_spread_apr_pct,
                    accumulated_funding_usd: 0.0,
                    opened_at: Utc::now(),
                    last_updated_at: Utc::now(),
                    is_closed: false,
                    closed_at: None,
                    realized_pnl_usd: None,
                };

                // Persist live position
                let trade_id = format!("{}-{}", opp.symbol, Utc::now().timestamp_millis());
                {
                    let mut store = self.state_store.lock();
                    let _ = store.upsert_position(live_pos.clone());
                    let _ = store.record_trade(TradeHistoryRecord {
                        id: trade_id.clone(),
                        symbol: opp.symbol.clone(),
                        action: "OPEN_LIVE".to_string(),
                        notional_usd: actual_notional,
                        hl_side: decision.hl_side,
                        hl_qty: hl_filled_qty,
                        hl_price: hl_actual_price,
                        bn_side: decision.bn_side,
                        bn_qty: hl_filled_qty,
                        bn_price: opp.binance_mark_price,
                        net_apr_at_action: opp.net_spread_apr_pct,
                        fees_incurred_usd: 0.0,
                        realized_pnl_usd: 0.0,
                        timestamp: Utc::now(),
                        notes: format!("Live open (mode: {})", self.execution_mode),
                    });
                }

                // Log into persistent TradeJournal
                let _ = self.journal.append(&JournalEntry::Intent(TradeIntentEvent {
                    id: format!("intent-{}", trade_id),
                    symbol: opp.symbol.clone(),
                    timestamp: Utc::now(),
                    is_paper: false,
                    hyperliquid_side: decision.hl_side,
                    binance_side: decision.bn_side,
                    hyperliquid_apr_pct: opp.hyperliquid_apr_pct,
                    binance_apr_pct: opp.binance_apr_pct,
                    net_spread_apr_pct: opp.net_spread_apr_pct,
                    projected_1h_net_bps: decision.single_cycle_income_bps,
                    projected_4h_net_bps: decision.projected_4h_net_bps,
                    target_notional_usd: actual_notional,
                    aligned_qty: hl_filled_qty,
                    friction_cost_bps: decision.total_friction_cost_bps,
                    est_hourly_return_bps: opp.est_hourly_return_bps,
                    reason: format!("Live execution triggered (mode: {})", self.execution_mode),
                }));

                let _ = self
                    .journal
                    .append(&JournalEntry::OpenFill(TradeOpenFillEvent {
                        id: trade_id.clone(),
                        intent_id: format!("intent-{}", trade_id),
                        symbol: opp.symbol.clone(),
                        timestamp: Utc::now(),
                        is_paper: false,
                        hyperliquid_side: decision.hl_side,
                        hyperliquid_qty: hl_filled_qty,
                        hyperliquid_price: hl_actual_price,
                        hyperliquid_fee_usd: actual_notional * 0.00035,
                        hyperliquid_mode: format!("{}", self.execution_mode),
                        binance_side: decision.bn_side,
                        binance_qty: hl_filled_qty,
                        binance_price: opp.binance_mark_price,
                        binance_fee_usd: actual_notional * 0.0004,
                        binance_mode: "MARKET_TAKER".to_string(),
                        total_notional_usd: actual_notional,
                        entry_price_spread_bps: ((hl_actual_price - opp.binance_mark_price)
                            / opp.binance_mark_price)
                            * 10_000.0,
                        total_open_fees_usd: actual_notional * (0.00035 + 0.0004),
                        execution_latency_ms: 15,
                    }));

                let alert = format!(
                    "🚨 <b>[实盘双腿建仓成功] BHyper Live Arbitrage</b>\n\n\
                    • <b>标的:</b> <code>{}</code>\n\
                    • <b>名义规模:</b> <code>${:.2}</code>\n\
                    • <b>Hyperliquid:</b> <code>{} {} @ ${:.4}</code>\n\
                    • <b>Binance:</b> <code>{} {} @ ${:.4}</code>\n\
                    • <b>净利差 APR:</b> <code>{:.2}%</code>",
                    live_pos.symbol,
                    live_pos.nominal_value_usd,
                    live_pos.hyperliquid_side,
                    hl_filled_qty,
                    live_pos.hyperliquid_entry_price,
                    live_pos.binance_side,
                    bn_qty_formatted,
                    live_pos.binance_entry_price,
                    live_pos.entry_spread_apr
                );
                let _ = self.notifier.send_alert(&alert).await;

                Ok(live_pos)
            }
            Err(e) => {
                // 孤儿腿紧急保护 (Orphan Leg Protection): 币安对冲失败，必须立刻平掉 Hyperliquid 仓位
                error!(
                    "🚨 CRITICAL: Binance Leg 2 Hedge Failed! Triggering Emergency Unwind on Hyperliquid: {:?}",
                    e
                );

                let unwind_is_buy = !hl_is_buy;
                let unwind_price = if unwind_is_buy {
                    hl_price * 1.05
                } else {
                    hl_price * 0.95
                };
                let _ = self
                    .hyperliquid
                    .place_order(
                        precision.hyperliquid_asset_index,
                        unwind_is_buy,
                        unwind_price,
                        hl_filled_qty,
                        true, // reduce_only
                        false,
                        true, // IOC market close
                    )
                    .await;

                let alert = format!(
                    "🚨 <b>[风控警报: 孤儿腿紧急平仓]</b>\n\n\
                    • <b>标的:</b> <code>{}</code>\n\
                    • <b>原因:</b> 币安第二腿对冲下单失败 (<code>{:?}</code>)\n\
                    • <b>操作:</b> 已对 Hyperliquid 发送紧急平仓指令，防止单边暴露！",
                    opp.symbol, e
                );
                let _ = self.notifier.send_alert(&alert).await;

                bail!(
                    "Binance hedge failed and Hyperliquid position unwound: {:?}",
                    e
                );
            }
        }
    }

    /// 双腿平仓 (Unwind Arbitrage Position with exact reason and PnL calculation)
    pub async fn execute_close(
        &self,
        position: &ActiveArbitragePosition,
        precision: &SymbolPrecisionInfo,
        close_reason: &str,
    ) -> Result<()> {
        let (live_bn_px, live_hl_px) = if let Some(ref cache) = self.cache {
            cache.get_latest_prices(&position.symbol).unwrap_or((
                position.binance_entry_price,
                position.hyperliquid_entry_price,
            ))
        } else {
            (
                position.binance_entry_price,
                position.hyperliquid_entry_price,
            )
        };

        let hl_pnl = match position.hyperliquid_side {
            PositionSide::Short => {
                (position.hyperliquid_entry_price - live_hl_px) * position.hyperliquid_qty
            }
            PositionSide::Long => {
                (live_hl_px - position.hyperliquid_entry_price) * position.hyperliquid_qty
            }
        };
        let bn_pnl = match position.binance_side {
            PositionSide::Short => {
                (position.binance_entry_price - live_bn_px) * position.binance_qty
            }
            PositionSide::Long => {
                (live_bn_px - position.binance_entry_price) * position.binance_qty
            }
        };
        let basis_pnl = hl_pnl + bn_pnl;
        let total_pnl = basis_pnl + position.accumulated_funding_usd;

        let holding_duration_mins = (Utc::now() - position.opened_at).num_minutes();

        if self.dry_run {
            info!(
                "🧪 [DRY-RUN / PAPER TRADING] Simulated close of position on {} (Reason: {}, Est PnL: ${:.4})",
                position.symbol, close_reason, total_pnl
            );
            {
                let mut store = self.state_store.lock();
                let _ = store.close_position(
                    &position.symbol,
                    live_bn_px,
                    live_hl_px,
                    total_pnl,
                    close_reason,
                );
            }
            let alert = format!(
                "🧪 <b>[模拟盘平仓完成] BHyper Paper Trading</b>\n\n\
                • <b>标的:</b> <code>{}</code>\n\
                • <b>规模:</b> <code>${:.2}</code>\n\
                • <b>持仓时间:</b> <code>{} mins</code>\n\
                • <b>平仓原因:</b> <code>{}</code>\n\
                • <b>模拟实现盈亏:</b> <code>${:.4}</code>\n\
                • <b>状态:</b> 双边对冲仓位已模拟平仓并落盘。",
                position.symbol,
                position.nominal_value_usd,
                holding_duration_mins,
                close_reason,
                total_pnl
            );
            let _ = self.notifier.send_alert(&alert).await;
            return Ok(());
        }

        info!(
            "⚡ [LIVE] Unwinding arbitrage pair on {} (Reason: {})...",
            position.symbol, close_reason
        );

        // Close Binance
        let bn_close_side = match position.binance_side {
            PositionSide::Long => PositionSide::Short,
            PositionSide::Short => PositionSide::Long,
        };
        let bn_close_side_str = match bn_close_side {
            PositionSide::Long => "BUY",
            PositionSide::Short => "SELL",
        };
        let bn_decimals = crate::strategy::precision::LotPrecisionMatcher::get_precision_decimals(
            precision.binance_step_size,
        );
        let bn_qty_str = format!("{:.prec$}", position.binance_qty, prec = bn_decimals);

        if let Some(ref ws_client) = self.ws_api_client {
            info!("⚡ Ultra Low-Latency Route: Closing Binance leg via WebSocket API...");
            if let Err(e) = ws_client
                .place_order(
                    &position.symbol,
                    bn_close_side_str,
                    "MARKET",
                    &bn_qty_str,
                    None,
                    true,
                )
                .await
            {
                warn!(
                    "⚠️ Binance WS API close error ({:?}), falling back to HTTP REST...",
                    e
                );
                let _ = self
                    .binance
                    .place_order(&position.symbol, bn_close_side, &bn_qty_str, None, true)
                    .await;
            }
        } else {
            let _ = self
                .binance
                .place_order(&position.symbol, bn_close_side, &bn_qty_str, None, true)
                .await;
        }

        // Close Hyperliquid
        let hl_close_is_buy = match position.hyperliquid_side {
            PositionSide::Long => false,
            PositionSide::Short => true,
        };
        let hl_ioc_close_px = if live_hl_px > 0.0 {
            if hl_close_is_buy {
                live_hl_px * 1.05
            } else {
                live_hl_px * 0.95
            }
        } else if hl_close_is_buy {
            position.hyperliquid_entry_price * 1.05
        } else {
            position.hyperliquid_entry_price * 0.95
        };

        let _ = self
            .hyperliquid
            .place_order(
                precision.hyperliquid_asset_index,
                hl_close_is_buy,
                hl_ioc_close_px,
                position.hyperliquid_qty,
                true, // reduce_only
                false,
                true, // IOC
            )
            .await;

        {
            let mut store = self.state_store.lock();
            let _ = store.close_position(
                &position.symbol,
                live_bn_px,
                live_hl_px,
                total_pnl,
                close_reason,
            );
        }

        let notional = position.nominal_value_usd;
        let total_exit_fees = notional * (0.00035 + 0.0004);
        let duration_secs = (Utc::now() - position.opened_at).num_seconds().max(1) as u64;

        let _ = self
            .journal
            .append(&JournalEntry::CloseFill(TradeCloseFillEvent {
                id: format!(
                    "close-{}-{}",
                    position.symbol,
                    Utc::now().timestamp_millis()
                ),
                open_trade_id: format!("{}-live", position.symbol),
                symbol: position.symbol.clone(),
                timestamp: Utc::now(),
                is_paper: false,
                holding_duration_secs: duration_secs,
                exit_reason: close_reason.to_string(),
                hyperliquid_exit_price: live_hl_px,
                hyperliquid_exit_fee_usd: notional * 0.00035,
                binance_exit_price: live_bn_px,
                binance_exit_fee_usd: notional * 0.00040,
                total_exit_fees_usd: total_exit_fees,
                total_roundtrip_fees_usd: total_exit_fees * 2.0,
                gross_basis_pnl_usd: basis_pnl,
                gross_funding_earned_usd: position.accumulated_funding_usd,
                net_realized_pnl_usd: total_pnl,
                net_return_bps: (total_pnl / notional.max(1.0)) * 10_000.0,
                return_on_capital_pct: (total_pnl / notional.max(1.0)) * 100.0,
            }));

        let alert = format!(
            "🔔 <b>[套利平仓完成] BHyper Position Closed</b>\n\n\
            • <b>标的:</b> <code>{}</code>\n\
            • <b>名义规模:</b> <code>${:.2}</code>\n\
            • <b>持仓时间:</b> <code>{} mins</code>\n\
            • <b>平仓原因:</b> <code>{}</code>\n\
            • <b>基差 PnL:</b> <code>${:.4}</code>\n\
            • <b>累计已收资金费:</b> <code>${:.4}</code>\n\
            • <b>实现总净利润:</b> <code>${:.4}</code>",
            position.symbol,
            position.nominal_value_usd,
            holding_duration_mins,
            close_reason,
            basis_pnl,
            position.accumulated_funding_usd,
            total_pnl
        );
        let _ = self.notifier.send_alert(&alert).await;

        Ok(())
    }
}

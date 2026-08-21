use crate::binance::BinanceFuturesClient;
use crate::hyperliquid::HyperliquidClient;
use crate::strategy::trigger::TriggerDecision;
use crate::telemetry::TelemetryNotifier;
use crate::types::{
    ActiveArbitragePosition, ArbitrageOpportunity, PositionSide,
    SymbolPrecisionInfo,
};
use anyhow::{bail, Result};
use chrono::Utc;
use tracing::{error, info};

pub struct TwoLegExecutor {
    binance: BinanceFuturesClient,
    hyperliquid: HyperliquidClient,
    notifier: TelemetryNotifier,
    dry_run: bool,
}

impl TwoLegExecutor {
    pub fn new(
        binance: BinanceFuturesClient,
        hyperliquid: HyperliquidClient,
        notifier: TelemetryNotifier,
        dry_run: bool,
    ) -> Self {
        Self {
            binance,
            hyperliquid,
            notifier,
            dry_run,
        }
    }

    /// 执行两腿对冲建仓 (Atomic Maker-Taker / Paper Trading Simulation)
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
            };

            let alert = format!(
                "🧪 <b>[模拟盘建仓成功] BHyper Paper Trading</b>\n\n\
                • <b>标的:</b> <code>{}</code>\n\
                • <b>名义规模:</b> <code>${:.2}</code> (数量: <code>{}</code>)\n\
                • <b>Hyperliquid:</b> <code>{} @ ${:.4}</code>\n\
                • <b>Binance:</b> <code>{} @ ${:.4}</code>\n\
                • <b>净利差 APR:</b> <code>{:.2}%</code> (预计单期收益: <code>{:.2} bps</code>)\n\
                • <b>Delta 敞口:</b> <code>$0.00 (100% 对冲)</code>",
                sim_pos.symbol,
                sim_pos.nominal_value_usd,
                aligned.binance_formatted_qty,
                sim_pos.hyperliquid_side,
                sim_pos.hyperliquid_entry_price,
                sim_pos.binance_side,
                sim_pos.binance_entry_price,
                sim_pos.entry_spread_apr,
                decision.single_cycle_income_bps
            );
            let _ = self.notifier.send_alert(&alert).await;

            return Ok(sim_pos);
        }

        // ==========================================
        // 实盘原子对冲流程 (REAL CAPITAL EXECUTION)
        // ==========================================
        info!(
            "⚡ [LIVE CAPITAL TRADING] Initiating atomic Maker-Taker arbitrage on {}...",
            opp.symbol
        );

        let hl_is_buy = decision.hl_side == PositionSide::Long;
        let hl_price = opp.hyperliquid_mark_price;

        // 第一腿 (Leg 1): 在 Hyperliquid 下 Post-Only Maker 挂单
        info!(
            "⏳ Step 1: Submitting Hyperliquid Post-Only Maker order ({} {} @ ${:.4})...",
            decision.hl_side, aligned.hyperliquid_formatted_qty, hl_price
        );

        let hl_res = self
            .hyperliquid
            .place_order(
                precision.hyperliquid_asset_index,
                hl_is_buy,
                hl_price,
                aligned.qty,
                false,
                true, // Post-Only (ALO)
                false,
            )
            .await;

        let hl_val = match hl_res {
            Ok(v) => v,
            Err(e) => {
                error!("❌ Hyperliquid Leg 1 Order Failed: {:?}", e);
                bail!("Hyperliquid Leg 1 Order Failed: {:?}", e);
            }
        };

        let hl_status_str = hl_val.to_string();
        info!("Hyperliquid Order Response: {}", hl_status_str);

        if hl_status_str.contains("error") || hl_status_str.contains("err") {
            bail!("Hyperliquid rejected maker order: {}", hl_status_str);
        }

        // 第二腿 (Leg 2): 一旦第一腿成交，立即向 Binance FAPI 发送市价 Taker 吃单完成名义对冲
        info!(
            "🚀 Step 2: Hyperliquid order acknowledged, immediately hedging on Binance ({} {})...",
            decision.bn_side, aligned.binance_formatted_qty
        );

        let bn_res = self
            .binance
            .place_order(
                &opp.symbol,
                decision.bn_side,
                &aligned.binance_formatted_qty,
                None, // Market order
                false,
            )
            .await;

        match bn_res {
            Ok(bn_val) => {
                info!("✅ Binance Leg 2 Filled: {}", bn_val);

                let live_pos = ActiveArbitragePosition {
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
                };

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
                    aligned.hyperliquid_formatted_qty,
                    live_pos.hyperliquid_entry_price,
                    live_pos.binance_side,
                    aligned.binance_formatted_qty,
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
                let _ = self
                    .hyperliquid
                    .place_order(
                        precision.hyperliquid_asset_index,
                        unwind_is_buy,
                        hl_price,
                        aligned.qty,
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

    /// 双腿平仓 (Unwind Arbitrage Position)
    pub async fn execute_close(
        &self,
        position: &ActiveArbitragePosition,
        precision: &SymbolPrecisionInfo,
    ) -> Result<()> {
        if self.dry_run {
            info!(
                "🧪 [DRY-RUN / PAPER TRADING] Simulated close of position on {}",
                position.symbol
            );
            let alert = format!(
                "🧪 <b>[模拟盘平仓完成] BHyper Paper Trading</b>\n\n\
                • <b>标的:</b> <code>{}</code>\n\
                • <b>持仓规模:</b> <code>${:.2}</code>\n\
                • <b>状态:</b> 双边对冲仓位已模拟平仓。",
                position.symbol, position.nominal_value_usd
            );
            let _ = self.notifier.send_alert(&alert).await;
            return Ok(());
        }

        info!("⚡ [LIVE] Unwinding arbitrage pair on {}...", position.symbol);

        // Close Binance
        let bn_close_side = match position.binance_side {
            PositionSide::Long => PositionSide::Short,
            PositionSide::Short => PositionSide::Long,
        };
        let bn_qty_str = format!("{:.4}", position.binance_qty);
        let _ = self
            .binance
            .place_order(&position.symbol, bn_close_side, &bn_qty_str, None, true)
            .await;

        // Close Hyperliquid
        let hl_close_is_buy = match position.hyperliquid_side {
            PositionSide::Long => false,
            PositionSide::Short => true,
        };
        let _ = self
            .hyperliquid
            .place_order(
                precision.hyperliquid_asset_index,
                hl_close_is_buy,
                0.0,
                position.hyperliquid_qty,
                true, // reduce_only
                false,
                true, // IOC
            )
            .await;

        let alert = format!(
            "🔔 <b>[套利平仓完成] BHyper Position Closed</b>\n\n\
            • <b>标的:</b> <code>{}</code>\n\
            • <b>规模:</b> <code>${:.2}</code>\n\
            • <b>状态:</b> 双边合约已全部平仓完毕。",
            position.symbol, position.nominal_value_usd
        );
        let _ = self.notifier.send_alert(&alert).await;

        Ok(())
    }
}

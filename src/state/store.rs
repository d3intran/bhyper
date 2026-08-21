use crate::binance::client::BinancePositionRiskItem;
use crate::hyperliquid::client::AssetPositionWrapper;
use crate::types::{
    ActiveArbitragePosition, PositionSide, ReconciliationReport, TradeHistoryRecord,
};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistentStateData {
    pub version: u32,
    pub active_positions: HashMap<String, ActiveArbitragePosition>,
    pub trade_history: Vec<TradeHistoryRecord>,
    pub total_realized_pnl_usd: f64,
    pub total_accumulated_funding_usd: f64,
    pub last_updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StateStore {
    path: PathBuf,
    data: PersistentStateData,
}

impl StateStore {
    pub fn default_state_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/bhyper/state.json")
    }

    pub fn load_or_create(path_opt: Option<PathBuf>) -> Result<Self> {
        let path = path_opt.unwrap_or_else(Self::default_state_path);

        if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read state file at {}", path.display()))?;
            let data: PersistentStateData = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse state JSON at {}", path.display()))?;
            info!(
                "📂 Loaded state store: {} active positions, {} trade history records from {}",
                data.active_positions.len(),
                data.trade_history.len(),
                path.display()
            );
            Ok(Self { path, data })
        } else {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let initial_data = PersistentStateData {
                version: 1,
                active_positions: HashMap::new(),
                trade_history: Vec::new(),
                total_realized_pnl_usd: 0.0,
                total_accumulated_funding_usd: 0.0,
                last_updated_at: Utc::now(),
            };
            let json_str = serde_json::to_string_pretty(&initial_data)?;
            let _ = fs::write(&path, json_str);
            info!("📂 Initialized new state store at {}", path.display());
            Ok(Self {
                path,
                data: initial_data,
            })
        }
    }

    pub fn save(&mut self) -> Result<()> {
        self.data.last_updated_at = Utc::now();
        let json_str = serde_json::to_string_pretty(&self.data)
            .context("Failed to serialize persistent state data")?;

        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Atomic write via temp file
        let tmp_path = self.path.with_extension("tmp");
        fs::write(&tmp_path, json_str).with_context(|| {
            format!("Failed to write temporary state to {}", tmp_path.display())
        })?;
        fs::rename(&tmp_path, &self.path).with_context(|| {
            format!(
                "Failed to rename {} to {}",
                tmp_path.display(),
                self.path.display()
            )
        })?;

        Ok(())
    }

    pub fn upsert_position(&mut self, pos: ActiveArbitragePosition) -> Result<()> {
        self.data.active_positions.insert(pos.symbol.clone(), pos);
        self.save()
    }

    pub fn get_position(&self, symbol: &str) -> Option<&ActiveArbitragePosition> {
        self.data.active_positions.get(symbol)
    }

    pub fn get_active_positions(&self) -> Vec<ActiveArbitragePosition> {
        self.data.active_positions.values().cloned().collect()
    }

    #[allow(dead_code)]
    pub fn remove_position(&mut self, symbol: &str) -> Result<Option<ActiveArbitragePosition>> {
        let removed = self.data.active_positions.remove(symbol);
        if removed.is_some() {
            self.save()?;
        }
        Ok(removed)
    }

    pub fn close_position(
        &mut self,
        symbol: &str,
        exit_bn_price: f64,
        exit_hl_price: f64,
        realized_pnl: f64,
        notes: &str,
    ) -> Result<Option<ActiveArbitragePosition>> {
        if let Some(mut pos) = self.data.active_positions.remove(symbol) {
            pos.is_closed = true;
            pos.closed_at = Some(Utc::now());
            pos.realized_pnl_usd = Some(realized_pnl);

            self.data.total_realized_pnl_usd += realized_pnl;

            let history_record = TradeHistoryRecord {
                id: format!("{}-{}", symbol, Utc::now().timestamp_millis()),
                symbol: symbol.to_string(),
                action: "CLOSE".to_string(),
                notional_usd: pos.nominal_value_usd,
                hl_side: match pos.hyperliquid_side {
                    PositionSide::Long => PositionSide::Short,
                    PositionSide::Short => PositionSide::Long,
                },
                hl_qty: pos.hyperliquid_qty,
                hl_price: exit_hl_price,
                bn_side: match pos.binance_side {
                    PositionSide::Long => PositionSide::Short,
                    PositionSide::Short => PositionSide::Long,
                },
                bn_qty: pos.binance_qty,
                bn_price: exit_bn_price,
                net_apr_at_action: pos.current_spread_apr,
                fees_incurred_usd: 0.0,
                realized_pnl_usd: realized_pnl,
                timestamp: Utc::now(),
                notes: notes.to_string(),
            };

            self.data.trade_history.push(history_record);
            self.save()?;
            Ok(Some(pos))
        } else {
            Ok(None)
        }
    }

    pub fn record_trade(&mut self, record: TradeHistoryRecord) -> Result<()> {
        self.data.trade_history.push(record);
        self.save()
    }

    /// 跨所持仓对账与孤儿腿检测 (Reconciliation)
    pub fn reconcile(
        &mut self,
        binance_positions: &[BinancePositionRiskItem],
        hl_positions: &[AssetPositionWrapper],
    ) -> ReconciliationReport {
        let mut report = ReconciliationReport::default();
        let mut bn_live_map: HashMap<String, (f64, f64)> = HashMap::new(); // symbol -> (qty signed, entry_price)
        let mut hl_live_map: HashMap<String, (f64, f64)> = HashMap::new();

        for item in binance_positions {
            if let Ok(qty) = item.position_amt.parse::<f64>() {
                if qty.abs() > 1e-6 {
                    let sym = item.symbol.trim_end_matches("USDT").to_string();
                    let price = item.entry_price.parse::<f64>().unwrap_or(0.0);
                    bn_live_map.insert(sym, (qty, price));
                }
            }
        }

        for item in hl_positions {
            let p = &item.position;
            if let Ok(szi) = p.szi.parse::<f64>() {
                if szi.abs() > 1e-6 {
                    let sym = p.coin.to_uppercase();
                    let price = p
                        .entry_px
                        .as_ref()
                        .and_then(|x| x.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    hl_live_map.insert(sym, (szi, price));
                }
            }
        }

        // 1. 检查已记录的 Active Positions
        let known_symbols: Vec<String> = self.data.active_positions.keys().cloned().collect();
        for sym in &known_symbols {
            let local_pos = match self.data.active_positions.get(sym) {
                Some(p) => p,
                None => continue,
            };

            let bn_live = bn_live_map.get(sym);
            let hl_live = hl_live_map.get(sym);

            match (bn_live, hl_live) {
                (Some((bn_q, _)), Some((hl_q, _))) => {
                    let expected_bn_sign = match local_pos.binance_side {
                        PositionSide::Long => 1.0,
                        PositionSide::Short => -1.0,
                    };
                    let expected_hl_sign = match local_pos.hyperliquid_side {
                        PositionSide::Long => 1.0,
                        PositionSide::Short => -1.0,
                    };

                    let bn_ok = (bn_q.signum() - expected_bn_sign).abs() < 1e-3;
                    let hl_ok = (hl_q.signum() - expected_hl_sign).abs() < 1e-3;

                    let delta_diff =
                        (bn_q.abs() - hl_q.abs()).abs() * local_pos.binance_entry_price;
                    if delta_diff > 1.0 {
                        report.delta_discrepancies.push((sym.clone(), delta_diff));
                        report.warnings.push(format!(
                            "Delta mismatch on {}: Binance qty = {}, Hyperliquid qty = {} (Diff: ${:.2})",
                            sym, bn_q, hl_q, delta_diff
                        ));
                    }

                    if !bn_ok || !hl_ok {
                        report.warnings.push(format!(
                            "Side mismatch on {}: Live BN side sign = {}, HL side sign = {}",
                            sym,
                            bn_q.signum(),
                            hl_q.signum()
                        ));
                    }
                }
                (Some(_), None) => {
                    report.orphaned_binance_positions.push(sym.clone());
                    report.warnings.push(format!(
                        "CRITICAL: Orphaned position on Binance for {} (Hyperliquid position closed or missing!)",
                        sym
                    ));
                }
                (None, Some(_)) => {
                    report.orphaned_hyperliquid_positions.push(sym.clone());
                    report.warnings.push(format!(
                        "CRITICAL: Orphaned position on Hyperliquid for {} (Binance position closed or missing!)",
                        sym
                    ));
                }
                (None, None) => {
                    report.warnings.push(format!(
                        "Position {} recorded locally but missing on both exchanges. Pruning from active list.",
                        sym
                    ));
                    self.data.active_positions.remove(sym);
                }
            }
        }

        // 2. 检查交易所存在但本地未记录的仓位 (Unmanaged Exchange Positions)
        for (sym, (bn_q, _)) in &bn_live_map {
            if !self.data.active_positions.contains_key(sym) {
                if let Some((hl_q, _)) = hl_live_map.get(sym) {
                    // 两边都有，尝试自动领养接管 (Auto-adopt)
                    let bn_side = if *bn_q > 0.0 {
                        PositionSide::Long
                    } else {
                        PositionSide::Short
                    };
                    let hl_side = if *hl_q > 0.0 {
                        PositionSide::Long
                    } else {
                        PositionSide::Short
                    };

                    info!(
                        "Adopting external matched position for {}: BN {} {:.4}, HL {} {:.4}",
                        sym,
                        bn_side,
                        bn_q.abs(),
                        hl_side,
                        hl_q.abs()
                    );

                    let adopted_pos = ActiveArbitragePosition {
                        symbol: sym.clone(),
                        binance_side: bn_side,
                        binance_qty: bn_q.abs(),
                        binance_entry_price: 0.0,
                        hyperliquid_side: hl_side,
                        hyperliquid_qty: hl_q.abs(),
                        hyperliquid_entry_price: 0.0,
                        nominal_value_usd: 0.0,
                        net_delta_usd: 0.0,
                        entry_spread_apr: 0.0,
                        current_spread_apr: 0.0,
                        accumulated_funding_usd: 0.0,
                        opened_at: Utc::now(),
                        last_updated_at: Utc::now(),
                        is_closed: false,
                        closed_at: None,
                        realized_pnl_usd: None,
                    };
                    self.data.active_positions.insert(sym.clone(), adopted_pos);
                } else {
                    report.orphaned_binance_positions.push(sym.clone());
                    report.warnings.push(format!(
                        "Unmanaged single-sided Binance position found: {} (Qty: {})",
                        sym, bn_q
                    ));
                }
            }
        }

        for (sym, (hl_q, _)) in &hl_live_map {
            if !self.data.active_positions.contains_key(sym) && !bn_live_map.contains_key(sym) {
                report.orphaned_hyperliquid_positions.push(sym.clone());
                report.warnings.push(format!(
                    "Unmanaged single-sided Hyperliquid position found: {} (Qty: {})",
                    sym, hl_q
                ));
            }
        }

        report.active_pairs_count = self.data.active_positions.len();
        report.is_consistent = report.orphaned_binance_positions.is_empty()
            && report.orphaned_hyperliquid_positions.is_empty()
            && report.delta_discrepancies.is_empty();

        let _ = self.save();
        report
    }
}

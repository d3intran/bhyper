use crate::types::{AlignedQuantity, SymbolPrecisionInfo};

pub struct LotPrecisionMatcher;

impl LotPrecisionMatcher {
    /// 计算两所共同兼容的精确对齐数量 (GCD Aligned Quantity)
    /// 确保小资金 ($50 ~ $100) 下两所下单数量 100% 严格一致，零 Delta 敞口泄漏
    pub fn calculate_aligned_quantity(
        symbol: &str,
        mark_price: f64,
        target_usd: f64,
        precision: &SymbolPrecisionInfo,
    ) -> AlignedQuantity {
        if mark_price <= 0.0 {
            return AlignedQuantity {
                symbol: symbol.to_string(),
                qty: 0.0,
                notional_usd: 0.0,
                binance_formatted_qty: "0".to_string(),
                hyperliquid_formatted_qty: "0".to_string(),
                is_aligned: false,
                delta_imbalance_usd: 0.0,
                delta_imbalance_pct: 0.0,
                reject_reason: Some("标的标记价格 <= 0".to_string()),
            };
        }

        // 1. 最小名义价值硬门槛 (Hyperliquid $10 + Binance $5 + 20% 安全垫 = $12.0)
        let min_required_notional = precision
            .binance_min_notional
            .max(precision.hyperliquid_min_notional)
            .max(12.0);

        if target_usd < min_required_notional {
            return AlignedQuantity {
                symbol: symbol.to_string(),
                qty: 0.0,
                notional_usd: 0.0,
                binance_formatted_qty: "0".to_string(),
                hyperliquid_formatted_qty: "0".to_string(),
                is_aligned: false,
                delta_imbalance_usd: 0.0,
                delta_imbalance_pct: 0.0,
                reject_reason: Some(format!(
                    "目标金额 ${:.2} 低于两所最小名义面值 ${:.2}",
                    target_usd, min_required_notional
                )),
            };
        }

        let raw_qty = target_usd / mark_price;

        // 2. 对齐币安 step_size
        let bn_step = if precision.binance_step_size > 0.0 {
            precision.binance_step_size
        } else {
            1.0
        };
        let bn_steps_count = (raw_qty / bn_step).floor();
        let bn_qty = bn_steps_count * bn_step;

        // 3. 对齐 Hyperliquid sz_decimals
        let hl_decimals = precision.hyperliquid_sz_decimals;
        let hl_factor = 10_f64.powi(hl_decimals as i32);
        let hl_qty = (raw_qty * hl_factor).floor() / hl_factor;

        // 4. 计算两所同时合法的数量 (取较小值以满足两所步长与小数位要求)
        let candidate_qty = bn_qty.min(hl_qty);

        // 再次根据精度格式化 candidate_qty
        let bn_decimals = Self::get_precision_decimals(bn_step);
        let aligned_decimals = bn_decimals.min(hl_decimals as usize);

        let final_factor = 10_f64.powi(aligned_decimals as i32);
        let aligned_qty = (candidate_qty * final_factor).floor() / final_factor;

        let final_notional = aligned_qty * mark_price;

        // 5. 校验对齐后的名义价值是否仍满足两所门槛
        if final_notional < min_required_notional {
            return AlignedQuantity {
                symbol: symbol.to_string(),
                qty: aligned_qty,
                notional_usd: final_notional,
                binance_formatted_qty: format!("{:.prec$}", aligned_qty, prec = bn_decimals),
                hyperliquid_formatted_qty: format!(
                    "{:.prec$}",
                    aligned_qty,
                    prec = hl_decimals as usize
                ),
                is_aligned: false,
                delta_imbalance_usd: 0.0,
                delta_imbalance_pct: 0.0,
                reject_reason: Some(format!(
                    "步长精度截断后名义价值 ${:.2} 低于最低要求 ${:.2} (单价 ${:.2}, 最小下单数量要求过大)",
                    final_notional, min_required_notional, mark_price
                )),
            };
        }

        // 6. 校验 Binance min_qty
        if aligned_qty < precision.binance_min_qty {
            return AlignedQuantity {
                symbol: symbol.to_string(),
                qty: aligned_qty,
                notional_usd: final_notional,
                binance_formatted_qty: format!("{:.prec$}", aligned_qty, prec = bn_decimals),
                hyperliquid_formatted_qty: format!(
                    "{:.prec$}",
                    aligned_qty,
                    prec = hl_decimals as usize
                ),
                is_aligned: false,
                delta_imbalance_usd: 0.0,
                delta_imbalance_pct: 0.0,
                reject_reason: Some(format!(
                    "数量 {:.4} 低于币安最小下单数量 {:.4}",
                    aligned_qty, precision.binance_min_qty
                )),
            };
        }

        let bn_formatted = format!("{:.prec$}", aligned_qty, prec = bn_decimals);
        let hl_formatted = format!("{:.prec$}", aligned_qty, prec = hl_decimals as usize);

        // 7. 严格 Delta 对称性验证
        let bn_parsed: f64 = bn_formatted.parse().unwrap_or(aligned_qty);
        let hl_parsed: f64 = hl_formatted.parse().unwrap_or(aligned_qty);
        let delta_imbalance = (bn_parsed - hl_parsed).abs() * mark_price;
        let delta_pct = if final_notional > 0.0 {
            (delta_imbalance / final_notional) * 100.0
        } else {
            0.0
        };

        if delta_pct > 0.01 {
            return AlignedQuantity {
                symbol: symbol.to_string(),
                qty: aligned_qty,
                notional_usd: final_notional,
                binance_formatted_qty: bn_formatted,
                hyperliquid_formatted_qty: hl_formatted,
                is_aligned: false,
                delta_imbalance_usd: delta_imbalance,
                delta_imbalance_pct: delta_pct,
                reject_reason: Some(format!(
                    "格式化后产生 Delta 漂移 ${:.4} ({:.4}%)",
                    delta_imbalance, delta_pct
                )),
            };
        }

        AlignedQuantity {
            symbol: symbol.to_string(),
            qty: aligned_qty,
            notional_usd: final_notional,
            binance_formatted_qty: bn_formatted,
            hyperliquid_formatted_qty: hl_formatted,
            is_aligned: true,
            delta_imbalance_usd: 0.0,
            delta_imbalance_pct: 0.0,
            reject_reason: None,
        }
    }

    /// 从 step_size 快速计算小数位数 (基于纯算术与查表，零堆分配)
    #[inline]
    pub fn get_precision_decimals(step_size: f64) -> usize {
        if step_size <= 0.0 || step_size >= 1.0 {
            return 0;
        }
        if (step_size - 0.1).abs() < 1e-6 {
            1
        } else if (step_size - 0.01).abs() < 1e-6 {
            2
        } else if (step_size - 0.001).abs() < 1e-6 {
            3
        } else if (step_size - 0.0001).abs() < 1e-6 {
            4
        } else if (step_size - 0.00001).abs() < 1e-6 {
            5
        } else if (step_size - 0.000001).abs() < 1e-6 {
            6
        } else if (step_size - 0.0000001).abs() < 1e-6 {
            7
        } else if (step_size - 0.00000001).abs() < 1e-6 {
            8
        } else {
            let log = -step_size.log10();
            if log.is_finite() && log > 0.0 {
                (log.round() as usize).min(8)
            } else {
                0
            }
        }
    }

    /// 格式化 Hyperliquid 订单价格 (严格遵循 L1 规则: 最多 5 位有效数字，最多 6 位小数，且 >= 100000 时为整数)
    pub fn format_hyperliquid_price(price: f64) -> String {
        if price <= 0.0 {
            return "0".to_string();
        }
        if price >= 100_000.0 {
            return format!("{:.0}", price.round());
        }

        let magnitude = price.log10().floor() as i32;
        let decimals = (4 - magnitude).clamp(0, 6) as usize;
        let factor = 10_f64.powi(decimals as i32);
        let rounded = (price * factor).round() / factor;

        let s = format!("{:.prec$}", rounded, prec = decimals);
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    }

    /// 格式化 Hyperliquid 下单数量 (严格对齐 sz_decimals)
    pub fn format_hyperliquid_size(size: f64, sz_decimals: u32) -> String {
        if size <= 0.0 {
            return "0".to_string();
        }
        let decimals = sz_decimals.min(8) as usize;
        let factor = 10_f64.powi(decimals as i32);
        let rounded = (size * factor).floor() / factor;
        let s = format!("{:.prec$}", rounded, prec = decimals);
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    }
}

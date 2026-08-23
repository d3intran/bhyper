/**
 * BHyper Terminal - Active Positions Component (Deterministic & Non-Jumping)
 */
import { formatCurrency, formatPrice, formatTimeUtc8 } from '../utils/format.js';

export function renderPositions(livePositions, paperPositions, onUnwind) {
  const all = [...(livePositions || []), ...(paperPositions || [])];

  // 1. 核心确定性防抖排序：按 symbol 字符绝对升序排序
  all.sort((a, b) => a.symbol.localeCompare(b.symbol));

  // 更新各种计数角标
  const countBadge = document.getElementById('pos-count-badge');
  const mobBadge = document.getElementById('mob-pos-badge');
  if (countBadge) countBadge.innerText = all.length;
  if (mobBadge) {
    mobBadge.className = all.length > 0 ? 'absolute top-0 right-1 w-2 h-2 rounded-full bg-amber-500 inline-block' : 'hidden';
  }
  const elActivePairs = document.getElementById('stat-active-pairs');
  if (elActivePairs) elActivePairs.innerText = `${all.length} 对`;

  // 计算加权预计每小时资金费收入
  let hourlyRunrate = 0.0;
  for (const p of all) {
    const apr = p.entry_spread_apr || p.current_spread_apr || 0;
    const notional = p.nominal_value_usd || 0;
    hourlyRunrate += notional * (apr / 100.0) / 8760.0;
  }
  const elRunrate = document.getElementById('stat-hourly-runrate');
  if (elRunrate) {
    elRunrate.innerText = `~$${hourlyRunrate.toFixed(4)}/h`;
  }

  // 2. Dashboard 简表渲染 (确定性排序)
  const dashDeck = document.getElementById('dashboard-positions-deck');
  if (dashDeck) {
    if (all.length === 0) {
      dashDeck.innerHTML = `<div class="text-center py-5 text-xs text-[var(--text-muted)]">当前无活跃持仓，套利引擎正在全时段扫描中...</div>`;
    } else {
      dashDeck.innerHTML = all.map(p => {
        const funding = p.total_funding_usd !== undefined ? p.total_funding_usd : (p.accumulated_funding_usd || 0);
        return `
          <div class="bg-[var(--bg-elevated)] p-3 rounded-lg border border-[var(--border-subtle)] flex items-center justify-between font-num">
            <div class="flex items-center space-x-3">
              <span class="font-bold text-xs text-[var(--text-primary)]">${p.symbol}</span>
              <span class="px-2 py-0.5 rounded text-[10px] font-medium bg-slate-200 dark:bg-slate-800 text-[var(--text-muted)] border border-[var(--border-subtle)]">BN: ${p.binance_side} / HL: ${p.hyperliquid_side}</span>
              <span class="text-xs text-[var(--text-muted)]">名义价值: <b class="text-[var(--text-primary)] font-semibold">${formatCurrency(p.nominal_value_usd || 0)}</b></span>
            </div>
            <div class="flex items-center space-x-3">
              <div class="text-right text-xs">
                <span class="text-[var(--text-muted)]">已收资金费: </span>
                <span class="text-cyan-500 font-semibold">+${formatCurrency(funding, 4)}</span>
              </div>
              <button data-symbol="${p.symbol}" class="btn-pos-unwind px-2.5 py-1 rounded-md bg-rose-500/10 hover:bg-rose-500/20 text-rose-500 font-medium text-xs border border-rose-500/20 transition">
                平仓
              </button>
            </div>
          </div>
        `;
      }).join('');
    }
  }

  // 3. Positions Tab 卡片全量渲染 (确定性排序)
  const container = document.getElementById('positions-container');
  if (!container) return;

  if (all.length === 0) {
    container.innerHTML = `
      <div class="surface-card rounded-xl p-10 text-center space-y-2.5">
        <i data-lucide="shield-check" class="w-8 h-8 text-emerald-500 mx-auto"></i>
        <div class="font-semibold text-xs text-[var(--text-primary)]">当前无运行中的套利仓位</div>
        <p class="text-xs text-[var(--text-muted)] max-w-sm mx-auto">套利引擎处于全自动守护状态，当两所资金费率利差与回本窗口满足条件时将自动建仓。</p>
      </div>
    `;
    return;
  }

  container.innerHTML = all.map(p => {
    const pnl = p.realized_pnl_usd !== undefined && p.realized_pnl_usd !== null ? p.realized_pnl_usd : 0;
    const pnlColor = pnl >= 0 ? 'text-emerald-500' : 'text-rose-500';
    const isLive = (livePositions || []).some(lp => lp.symbol === p.symbol);
    const funding = p.total_funding_usd !== undefined ? p.total_funding_usd : (p.accumulated_funding_usd || 0);
    const ticksCount = p.funding_ticks_count || 0;
    const bnFee = p.binance_entry_fee_usd || 0;
    const hlFee = p.hyperliquid_entry_fee_usd || 0;

    return `
      <div class="surface-card rounded-xl p-4 space-y-3.5 font-num transition-none">
        <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 border-b border-[var(--border-subtle)] pb-3">
          <div class="flex items-center space-x-3">
            <span class="w-8 h-8 rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-subtle)] flex items-center justify-center font-bold text-emerald-500 text-xs">${p.symbol.slice(0, 2)}</span>
            <div>
              <div class="flex items-center space-x-2">
                <span class="text-sm font-bold text-[var(--text-primary)]">${p.symbol} 对冲套利</span>
                ${isLive ? '<span class="px-2 py-0.2 rounded text-[10px] font-semibold bg-emerald-500/10 text-emerald-500 border border-emerald-500/20">LIVE</span>' : '<span class="px-2 py-0.2 rounded text-[10px] font-semibold bg-blue-500/10 text-blue-400 border border-blue-500/20">SIMULATED</span>'}
              </div>
              <div class="text-[11px] text-[var(--text-muted)] mt-0.5">开仓时间: ${formatTimeUtc8(p.opened_at)} (UTC+8)</div>
            </div>
          </div>
          <div class="flex items-center space-x-3">
            <div class="text-right">
              <div class="text-xs text-[var(--text-muted)]">名义对冲价值</div>
              <div class="text-sm font-bold text-[var(--text-primary)]">${formatCurrency(p.nominal_value_usd || 0)}</div>
            </div>
            <button data-symbol="${p.symbol}" class="btn-pos-unwind px-3 py-1.5 rounded-lg bg-rose-500/10 hover:bg-rose-500/20 text-rose-500 font-medium text-xs border border-rose-500/20 transition flex items-center space-x-1">
              <i data-lucide="x-circle" class="w-3.5 h-3.5"></i>
              <span>一键对冲平仓</span>
            </button>
          </div>
        </div>

        <!-- Two Legs Detail Deck -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
          <!-- Binance Leg -->
          <div class="bg-[var(--bg-elevated)] p-3 rounded-lg border border-[var(--border-subtle)] space-y-1.5">
            <div class="flex justify-between text-xs">
              <span class="font-semibold text-amber-500/90 flex items-center space-x-1">
                <span class="w-1.5 h-1.5 rounded-full bg-amber-500"></span>
                <span>Binance 腿</span>
              </span>
              <span class="font-semibold ${p.binance_side === 'Long' ? 'text-emerald-500' : 'text-rose-500'}">${p.binance_side} ${p.binance_qty} ${p.symbol}</span>
            </div>
            <div class="flex justify-between text-[11px] text-[var(--text-muted)]">
              <span>开仓均价: <b class="text-[var(--text-primary)] font-semibold">${formatPrice(p.binance_entry_price)}</b></span>
              <span>开仓费用: <b class="text-rose-500">-${formatCurrency(bnFee, 4)}</b></span>
            </div>
          </div>

          <!-- Hyperliquid Leg -->
          <div class="bg-[var(--bg-elevated)] p-3 rounded-lg border border-[var(--border-subtle)] space-y-1.5">
            <div class="flex justify-between text-xs">
              <span class="font-semibold text-cyan-500/90 flex items-center space-x-1">
                <span class="w-1.5 h-1.5 rounded-full bg-cyan-500"></span>
                <span>Hyperliquid 腿</span>
              </span>
              <span class="font-semibold ${p.hyperliquid_side === 'Long' ? 'text-emerald-500' : 'text-rose-500'}">${p.hyperliquid_side} ${p.hyperliquid_qty} ${p.symbol}</span>
            </div>
            <div class="flex justify-between text-[11px] text-[var(--text-muted)]">
              <span>开仓均价: <b class="text-[var(--text-primary)] font-semibold">${formatPrice(p.hyperliquid_entry_price)}</b></span>
              <span>开仓费用: <b class="${hlFee === 0 ? 'text-emerald-500' : 'text-rose-500'} font-semibold">${hlFee === 0 ? '$0.00 (Maker)' : `-${formatCurrency(hlFee, 4)}`}</b></span>
            </div>
          </div>
        </div>

        <!-- Funding & PnL Ribbon -->
        <div class="grid grid-cols-3 gap-2 bg-[var(--bg-elevated)] p-2.5 rounded-lg border border-[var(--border-subtle)] text-center text-xs">
          <div>
            <div class="text-[10px] text-[var(--text-muted)]">入场利差 APR</div>
            <div class="font-bold text-emerald-500 text-xs">${(p.entry_spread_apr || 0).toFixed(2)}%</div>
          </div>
          <div>
            <div class="text-[10px] text-[var(--text-muted)]">累计已收资金费</div>
            <div class="font-bold text-cyan-500 text-xs">+${formatCurrency(funding, 4)} (${ticksCount} 次)</div>
          </div>
          <div>
            <div class="text-[10px] text-[var(--text-muted)]">实时基差未实现 PnL</div>
            <div class="font-bold ${pnlColor} text-xs">${pnl >= 0 ? '+' : ''}${formatCurrency(pnl, 4)}</div>
          </div>
        </div>

      </div>
    `;
  }).join('');

  // 绑定平仓事件
  document.querySelectorAll('.btn-pos-unwind').forEach(btn => {
    btn.addEventListener('click', () => {
      const sym = btn.getAttribute('data-symbol');
      if (sym && onUnwind) onUnwind(sym);
    });
  });
}

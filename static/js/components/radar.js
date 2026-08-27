/**
 * BHyper Terminal - Radar Matrix Component
 * Pure English Institutional Layout
 */
import { formatCurrency, formatPrice } from '../utils/format.js';

export function renderRadarTable(opportunities, activeFilter, keyword, onOpenTrade) {
  const tbody = document.getElementById('radar-table-body');
  if (!tbody) return;

  let list = opportunities || [];
  const elCountAll = document.getElementById('count-all');
  if (elCountAll) elCountAll.innerText = list.length;

  // Filter Logic
  if (activeFilter === 'high_apr') {
    list = list.filter(o => o.net_spread_apr_pct >= 50.0);
  } else if (activeFilter === 'pos_cashflow') {
    list = list.filter(o => o.projected_1h_net_bps > 0);
  } else if (activeFilter === 'liquid') {
    list = list.filter(o => (o.total_open_interest_usd || 0) >= 1_000_000);
  }

  // Keyword Search Filter
  if (keyword) {
    const kw = keyword.toUpperCase().trim();
    list = list.filter(o => o.symbol.includes(kw));
  }

  if (list.length === 0) {
    tbody.innerHTML = `
      <tr>
        <td colspan="10" class="text-center py-12 text-[var(--text-muted)]">
          <div class="flex flex-col items-center justify-center space-y-2">
            <i data-lucide="search-x" class="w-6 h-6 text-slate-500 opacity-60"></i>
            <span class="text-xs">No matching arbitrage targets found.</span>
          </div>
        </td>
      </tr>
    `;
    return;
  }

  tbody.innerHTML = list.map(o => {
    const isHlShort = o.hyperliquid_side === 'Short';
    const actionHtml = `
      <span class="inline-flex items-center space-x-1 px-2 py-0.5 rounded text-[11px] bg-[var(--bg-elevated)] border border-[var(--border-subtle)]">
        <span class="text-[var(--text-muted)]">HL:</span><b class="${isHlShort ? 'text-rose-500' : 'text-emerald-500'} font-semibold">${o.hyperliquid_side}</b>
        <span class="text-slate-300 dark:text-slate-700">/</span>
        <span class="text-[var(--text-muted)]">BN:</span><b class="${!isHlShort ? 'text-rose-500' : 'text-emerald-500'} font-semibold">${o.binance_side}</b>
      </span>
    `;

    const beStr = o.est_break_even_hours > 500 ? '>500h' : `${o.est_break_even_hours.toFixed(1)}h`;
    const proj1hColor = o.projected_1h_net_bps > 0 ? 'text-emerald-500 font-semibold' : 'text-slate-400';
    
    let tierBadge = `<span class="px-1.5 py-0.2 rounded text-[10px] font-medium bg-slate-200 dark:bg-slate-800 text-[var(--text-muted)]">MID</span>`;
    if (o.liquidity_tier && o.liquidity_tier.includes('PRIME')) {
      tierBadge = `<span class="px-1.5 py-0.2 rounded text-[10px] font-medium bg-emerald-500/10 text-emerald-500 border border-emerald-500/25">PRIME</span>`;
    } else if (o.liquidity_tier && o.liquidity_tier.includes('LIQUID')) {
      tierBadge = `<span class="px-1.5 py-0.2 rounded text-[10px] font-medium bg-cyan-500/10 text-cyan-500 border border-cyan-500/25">LIQUID</span>`;
    }

    return `
      <tr class="hover:bg-[var(--table-hover)] transition">
        <td class="py-2.5 px-3.5 font-bold text-[var(--text-primary)]">
          <div class="flex items-center space-x-2">
            <span class="w-6 h-6 rounded-md bg-[var(--bg-elevated)] flex items-center justify-center font-bold text-[11px] text-emerald-500 border border-[var(--border-subtle)]">${o.symbol.slice(0, 2)}</span>
            <span class="text-xs font-semibold">${o.symbol}</span>
          </div>
        </td>
        <td class="py-2.5 px-3">
          <div class="text-[var(--text-primary)] font-medium">${formatPrice(o.binance_mark_price)}</div>
          <div class="text-[10px] text-amber-500/90 font-medium font-num">${o.binance_apr_pct >= 0 ? '+' : ''}${o.binance_apr_pct.toFixed(2)}% APR</div>
        </td>
        <td class="py-2.5 px-3">
          <div class="text-[var(--text-primary)] font-medium">${formatPrice(o.hyperliquid_mark_price)}</div>
          <div class="text-[10px] text-cyan-500/90 font-medium font-num">${o.hyperliquid_apr_pct >= 0 ? '+' : ''}${o.hyperliquid_apr_pct.toFixed(2)}% APR</div>
        </td>
        <td class="py-2.5 px-3 text-right">
          <span class="text-xs font-bold ${o.net_spread_apr_pct >= 50 ? 'text-emerald-500' : 'text-[var(--text-primary)]'}">${o.net_spread_apr_pct.toFixed(2)}%</span>
        </td>
        <td class="py-2.5 px-3 text-center">${actionHtml}</td>
        <td class="py-2.5 px-2 text-right text-[var(--text-muted)]">${beStr}</td>
        <td class="py-2.5 px-2 text-right ${proj1hColor}">${o.projected_1h_net_bps > 0 ? '+' : ''}${o.projected_1h_net_bps.toFixed(2)} bps</td>
        <td class="py-2.5 px-2 text-right text-[var(--text-muted)]">$${((o.binance_volume_24h_usd || 0) / 1_000_000).toFixed(1)}M</td>
        <td class="py-2.5 px-2 text-center">${tierBadge}</td>
        <td class="py-2.5 px-3 text-right">
          <button data-symbol="${o.symbol}" class="btn-radar-open px-2.5 py-1 rounded-md bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 font-medium border border-emerald-500/25 transition text-[11px] active:scale-95">
            Open
          </button>
        </td>
      </tr>
    `;
  }).join('');

  // Bind Open Trade buttons
  tbody.querySelectorAll('.btn-radar-open').forEach(btn => {
    btn.addEventListener('click', () => {
      const sym = btn.getAttribute('data-symbol');
      if (sym && onOpenTrade) onOpenTrade(sym);
    });
  });
}

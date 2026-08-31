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
        <td colspan="10" class="text-center py-12 text-ink-mute">
          <div class="flex flex-col items-center justify-center space-y-2">
            <i data-lucide="search-x" class="w-6 h-6 text-ink-mute opacity-60"></i>
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
      <span class="inline-flex items-center space-x-1 px-2 py-0.5 rounded text-2xs bg-elevated border border-edge">
        <span class="text-ink-mute">HL:</span><b class="${isHlShort ? 'text-rose-500' : 'text-emerald-500'} font-semibold">${o.hyperliquid_side}</b>
        <span class="text-edge-strong">/</span>
        <span class="text-ink-mute">BN:</span><b class="${!isHlShort ? 'text-rose-500' : 'text-emerald-500'} font-semibold">${o.binance_side}</b>
      </span>
    `;

    const beStr = o.est_break_even_hours > 500 ? '>500h' : `${o.est_break_even_hours.toFixed(1)}h`;
    const proj1hColor = o.projected_1h_net_bps > 0 ? 'text-emerald-500 font-semibold' : 'text-ink-mute';
    
    let tierBadge = `<span class="px-1.5 py-0.2 rounded text-2xs font-medium bg-subtle text-ink-mute">MID</span>`;
    if (o.liquidity_tier && o.liquidity_tier.includes('PRIME')) {
      tierBadge = `<span class="px-1.5 py-0.2 rounded text-2xs font-medium bg-emerald-500/10 text-emerald-500 border border-emerald-500/25">PRIME</span>`;
    } else if (o.liquidity_tier && o.liquidity_tier.includes('LIQUID')) {
      tierBadge = `<span class="px-1.5 py-0.2 rounded text-2xs font-medium bg-cyan-500/10 text-cyan-500 border border-cyan-500/25">LIQUID</span>`;
    }

    return `
      <tr>
        <td class="py-2.5 px-3.5 font-bold text-ink">
          <div class="flex items-center space-x-2">
            <span class="w-6 h-6 rounded-md bg-elevated flex items-center justify-center font-bold text-2xs text-ink-soft border border-edge">${o.symbol.slice(0, 2)}</span>
            <span class="text-xs font-semibold">${o.symbol}</span>
          </div>
        </td>
        <td class="py-2.5 px-3">
          <div class="text-ink font-medium">${formatPrice(o.binance_mark_price)}</div>
          <div class="text-2xs text-ink-mute font-medium font-num">${o.binance_apr_pct >= 0 ? '+' : ''}${o.binance_apr_pct.toFixed(2)}% APR</div>
        </td>
        <td class="py-2.5 px-3">
          <div class="text-ink font-medium">${formatPrice(o.hyperliquid_mark_price)}</div>
          <div class="text-2xs text-ink-mute font-medium font-num">${o.hyperliquid_apr_pct >= 0 ? '+' : ''}${o.hyperliquid_apr_pct.toFixed(2)}% APR</div>
        </td>
        <td class="py-2.5 px-3 text-right">
          <span class="text-xs font-bold ${o.net_spread_apr_pct >= 50 ? 'text-emerald-500' : 'text-ink'}">${o.net_spread_apr_pct.toFixed(2)}%</span>
        </td>
        <td class="py-2.5 px-3 text-center">${actionHtml}</td>
        <td class="py-2.5 px-2 text-right text-ink-mute">${beStr}</td>
        <td class="py-2.5 px-2 text-right ${proj1hColor}">${o.projected_1h_net_bps > 0 ? '+' : ''}${o.projected_1h_net_bps.toFixed(2)} bps</td>
        <td class="py-2.5 px-2 text-right text-ink-mute">$${((o.binance_volume_24h_usd || 0) / 1_000_000).toFixed(1)}M</td>
        <td class="py-2.5 px-2 text-center">${tierBadge}</td>
        <td class="py-2.5 px-3 text-right">
          <button data-symbol="${o.symbol}" class="btn-radar-open px-2.5 py-1 rounded-md bg-elevated hover:bg-hover text-ink border border-edge font-medium transition text-2xs active:scale-95">
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

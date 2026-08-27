/**
 * BHyper Terminal - Holographic Journal Component
 * Pure English Institutional Layout
 */
import { apiFetch } from '../api.js';
import { formatCurrency, formatPrice, formatTimeOnlyUtc8 } from '../utils/format.js';

export async function fetchAndRenderJournal(filterType = '') {
  const tbody = document.getElementById('journal-table-body');
  if (!tbody) return;

  try {
    const res = await apiFetch(`/api/journal?event_type=${encodeURIComponent(filterType)}&limit=50`);
    if (!res.entries || res.entries.length === 0) {
      tbody.innerHTML = `
        <tr>
          <td colspan="7" class="text-center py-12 text-[var(--text-muted)]">
            <div class="flex flex-col items-center justify-center space-y-2">
              <i data-lucide="file-text" class="w-6 h-6 text-slate-500 opacity-60"></i>
              <span class="text-xs">No ledger entries recorded yet.</span>
            </div>
          </td>
        </tr>
      `;
      return;
    }

    tbody.innerHTML = res.entries.map(e => {
      const typeName = e.type;
      const d = e.data;
      const time = formatTimeOnlyUtc8(d.timestamp);

      let typeBadge = `<span class="px-2 py-0.2 rounded text-[10px] font-medium bg-slate-200 dark:bg-slate-800 text-[var(--text-primary)]">${typeName}</span>`;
      let detail = '';
      let notional = '-';
      let pnlOrFunding = '-';
      let fees = '-';

      if (typeName === 'Funding') {
        typeBadge = `<span class="px-2 py-0.2 rounded text-[10px] font-medium bg-cyan-500/10 text-cyan-500 border border-cyan-500/25">FUNDING</span>`;
        detail = `${d.exchange || ''} ${d.side || ''} (Rate: ${(d.rate_bps || 0).toFixed(2)} bps)`;
        notional = formatCurrency(d.notional_usd || 0);
        const pay = d.funding_payment_usd || 0;
        pnlOrFunding = `<span class="${pay >= 0 ? 'text-cyan-500' : 'text-rose-500'} font-semibold">${pay >= 0 ? '+' : ''}${formatCurrency(pay, 4)}</span>`;
        fees = '$0.0000';
      } else if (typeName === 'OpenFill') {
        typeBadge = `<span class="px-2 py-0.2 rounded text-[10px] font-medium bg-emerald-500/10 text-emerald-500 border border-emerald-500/25">OPEN</span>`;
        detail = `HL: ${d.hyperliquid_side} ${formatPrice(d.hyperliquid_price)} / BN: ${d.binance_side} ${formatPrice(d.binance_price)}`;
        notional = formatCurrency(d.total_notional_usd || d.notional_usd || 0);
        pnlOrFunding = `<span class="text-slate-400">Fully Hedged</span>`;
        fees = formatCurrency(d.total_open_fees_usd || 0, 4);
      } else if (typeName === 'CloseFill') {
        typeBadge = `<span class="px-2 py-0.2 rounded text-[10px] font-medium bg-rose-500/10 text-rose-500 border border-rose-500/25">CLOSE</span>`;
        detail = `HL: ${formatPrice(d.hyperliquid_exit_price)} / BN: ${formatPrice(d.binance_exit_price)} (Basis PnL: ${formatCurrency(d.gross_basis_pnl_usd || 0, 4)})`;
        const pnlVal = d.net_realized_pnl_usd || 0;
        pnlOrFunding = `<span class="${pnlVal >= 0 ? 'text-emerald-500' : 'text-rose-500'} font-semibold">${pnlVal >= 0 ? '+' : ''}${formatCurrency(pnlVal, 4)}</span>`;
        fees = formatCurrency(d.total_roundtrip_fees_usd || 0, 4);
      } else if (typeName === 'Intent') {
        typeBadge = `<span class="px-2 py-0.2 rounded text-[10px] font-medium bg-purple-500/10 text-purple-500 border border-purple-500/25">INTENT</span>`;
        detail = `${d.reason || 'Arbitrage Trigger Signal'}`;
        notional = formatCurrency(d.target_notional_usd || 0);
        pnlOrFunding = `<span class="text-emerald-500 font-medium">+${(d.net_spread_apr_pct || 0).toFixed(2)}% APR</span>`;
        fees = '-';
      } else if (typeName === 'RiskAlert') {
        typeBadge = `<span class="px-2 py-0.2 rounded text-[10px] font-medium bg-amber-500/10 text-amber-500 border border-amber-500/25">RISK</span>`;
        detail = `${d.event_type || ''}: ${d.details || ''} (${d.action_taken || ''})`;
        pnlOrFunding = `<span class="text-amber-500 font-medium">Risk Guard</span>`;
      }

      return `
        <tr class="hover:bg-[var(--table-hover)] transition">
          <td class="py-2.5 px-3.5 text-[var(--text-muted)]">${time}</td>
          <td class="py-2.5 px-2">${typeBadge}</td>
          <td class="py-2.5 px-2 font-semibold text-[var(--text-primary)]">${d.symbol}</td>
          <td class="py-2.5 px-3 text-[var(--text-secondary)]">${detail}</td>
          <td class="py-2.5 px-2 text-right text-[var(--text-muted)]">${notional}</td>
          <td class="py-2.5 px-2 text-right">${pnlOrFunding}</td>
          <td class="py-2.5 px-3 text-right ${fees.startsWith('$') && fees !== '$0.0000' ? 'text-rose-500' : 'text-[var(--text-muted)]'}">${fees.startsWith('$') && fees !== '$0.0000' ? '-' + fees : fees}</td>
        </tr>
      `;
    }).join('');

    if (window.lucide) {
      window.lucide.createIcons();
    }
  } catch (e) {
    console.warn('Journal error:', e);
  }
}

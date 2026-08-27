/**
 * BHyper Terminal - Dashboard Overview Component
 * Pure English Institutional Layout
 */
import { formatCurrency, formatPnl, formatPrice } from '../utils/format.js';

export function renderWalletStats(wallet) {
  if (!wallet) return;

  const bnEq = (wallet.binance?.cash_balance_usd || 0) + (wallet.binance?.unrealized_pnl_usd || 0);
  const hlEq = (wallet.hyperliquid?.cash_balance_usd || 0) + (wallet.hyperliquid?.unrealized_pnl_usd || 0);
  const totalEq = bnEq + hlEq;

  const elTotalEq = document.getElementById('stat-total-equity');
  if (elTotalEq) elTotalEq.innerText = formatCurrency(totalEq);
  
  const elBnEq = document.getElementById('stat-bn-equity');
  if (elBnEq) elBnEq.innerText = formatCurrency(bnEq);

  const elHlEq = document.getElementById('stat-hl-equity');
  if (elHlEq) elHlEq.innerText = formatCurrency(hlEq);

  // Funding and Fees
  const totalFees = (wallet.binance?.total_fees_paid_usd || 0) + (wallet.hyperliquid?.total_fees_paid_usd || 0);
  const totalFunding = (wallet.binance?.total_funding_usd || 0) + (wallet.hyperliquid?.total_funding_usd || 0);
  const netCarry = totalFunding - totalFees;

  const elNetCarry = document.getElementById('stat-funding-income');
  if (elNetCarry) {
    elNetCarry.innerText = formatPnl(netCarry, 4);
    elNetCarry.className = `text-2xl font-num font-bold mt-1 tracking-tight ${netCarry >= 0 ? 'text-emerald-500' : 'text-rose-500'}`;
  }

  const elGross = document.getElementById('stat-gross-funding');
  if (elGross) elGross.innerText = `+${formatCurrency(totalFunding, 4)}`;

  const elFees = document.getElementById('stat-total-fees');
  if (elFees) elFees.innerText = `-${formatCurrency(totalFees, 4)}`;

  // Margin Utilization
  const totalAllocated = (wallet.binance?.allocated_margin_usd || 0) + (wallet.hyperliquid?.allocated_margin_usd || 0);
  const utilPct = totalEq > 0 ? (totalAllocated / totalEq) * 100 : 0;
  const elMarginUtil = document.getElementById('stat-margin-util');
  if (elMarginUtil) elMarginUtil.innerText = `(${utilPct.toFixed(1)}% Used)`;

  // Margin Progress Bars
  const bnAlloc = wallet.binance?.allocated_margin_usd || 0;
  const hlAlloc = wallet.hyperliquid?.allocated_margin_usd || 0;
  const bnUtil = bnEq > 0 ? (bnAlloc / bnEq) * 100 : 0;
  const hlUtil = hlEq > 0 ? (hlAlloc / hlEq) * 100 : 0;

  const elBnHealth = document.getElementById('bn-health-util');
  if (elBnHealth) elBnHealth.innerText = `Allocated: ${formatCurrency(bnAlloc)} (${bnUtil.toFixed(1)}%)`;
  const elBnBar = document.getElementById('bn-util-bar');
  if (elBnBar) elBnBar.style.width = `${Math.min(bnUtil, 100)}%`;
  const elBnFree = document.getElementById('bn-free-margin');
  if (elBnFree) elBnFree.innerText = formatCurrency(Math.max(0, bnEq - bnAlloc));

  const elHlHealth = document.getElementById('hl-health-util');
  if (elHlHealth) elHlHealth.innerText = `Allocated: ${formatCurrency(hlAlloc)} (${hlUtil.toFixed(1)}%)`;
  const elHlBar = document.getElementById('hl-util-bar');
  if (elHlBar) elHlBar.style.width = `${Math.min(hlUtil, 100)}%`;
  const elHlFree = document.getElementById('hl-free-margin');
  if (elHlFree) elHlFree.innerText = formatCurrency(Math.max(0, hlEq - hlAlloc));
}

export function renderHealthAssessment(assessment) {
  if (!assessment) return;

  const badge = document.getElementById('rebalance-status-badge');
  const advText = document.getElementById('rebalance-advisory-text');
  const healthBadge = document.getElementById('stat-health-badge');

  if (assessment.rebalance_required) {
    if (badge) {
      badge.className = 'px-2 py-0.5 rounded text-[11px] font-medium bg-amber-500/10 text-amber-500 border border-amber-500/25';
      badge.innerText = 'Rebalance Advised';
    }
    if (advText) {
      advText.innerHTML = `<i data-lucide="alert-circle" class="w-3.5 h-3.5 text-amber-500 inline mr-1"></i><span>${assessment.risk_status || 'Cross-exchange margin transfer recommended.'}</span>`;
    }
    if (healthBadge) {
      healthBadge.className = 'px-1.5 py-0.2 rounded bg-amber-500/10 text-amber-500 text-[10px] font-semibold border border-amber-500/25';
      healthBadge.innerText = 'REBALANCE';
    }
  } else {
    if (badge) {
      badge.className = 'px-2 py-0.5 rounded text-[11px] font-medium bg-emerald-500/10 text-emerald-500 border border-emerald-500/25';
      badge.innerText = 'Balanced';
    }
    if (advText) {
      advText.innerHTML = `<i data-lucide="check-circle-2" class="w-3.5 h-3.5 text-emerald-500 inline mr-1"></i><span>${assessment.risk_status || 'Cross-exchange margin allocation is optimal.'}</span>`;
    }
    if (healthBadge) {
      healthBadge.className = 'px-1.5 py-0.2 rounded bg-emerald-500/10 text-emerald-500 text-[10px] font-semibold border border-emerald-500/25';
      healthBadge.innerText = 'HEALTHY';
    }
  }
}

export function renderDashboardTopRadar(opportunities, onOpenTrade) {
  const container = document.getElementById('dashboard-top-radar-table');
  if (!container) return;

  const top5 = (opportunities || []).slice(0, 5);
  if (top5.length === 0) {
    container.innerHTML = `
      <div class="text-center py-8 text-[var(--text-muted)] text-xs">
        <i data-lucide="radar" class="w-6 h-6 text-slate-500 mx-auto mb-1 opacity-50"></i>
        <span>Scanning perpetual markets for spread alpha...</span>
      </div>
    `;
    return;
  }

  container.innerHTML = `
    <table class="w-full text-left border-collapse text-xs">
      <thead>
        <tr class="border-b border-[var(--border-subtle)] text-[var(--text-muted)] text-[11px] font-semibold bg-[var(--bg-elevated)]">
          <th class="py-2.5 px-3">Asset</th>
          <th class="py-2.5 px-2">Mark Price (BN / HL)</th>
          <th class="py-2.5 px-2 text-right">Net Spread (APR)</th>
          <th class="py-2.5 px-3 text-center">Hedged Legs</th>
          <th class="py-2.5 px-2 text-right">Est. 1h Net Cashflow</th>
          <th class="py-2.5 px-3 text-right">Action</th>
        </tr>
      </thead>
      <tbody class="divide-y divide-[var(--border-subtle)] font-num">
        ${top5.map(o => `
          <tr class="hover:bg-[var(--table-hover)] transition">
            <td class="py-2.5 px-3 font-semibold text-[var(--text-primary)]">
              <span class="inline-flex items-center space-x-1.5">
                <span class="w-5 h-5 rounded bg-[var(--bg-elevated)] border border-[var(--border-subtle)] inline-flex items-center justify-center text-[10px] font-bold text-emerald-500">${o.symbol.slice(0, 2)}</span>
                <span>${o.symbol}</span>
              </span>
            </td>
            <td class="py-2.5 px-2 text-[var(--text-secondary)]">${formatPrice(o.binance_mark_price)} / ${formatPrice(o.hyperliquid_mark_price)}</td>
            <td class="py-2.5 px-2 text-right font-bold text-emerald-500">${o.net_spread_apr_pct.toFixed(2)}%</td>
            <td class="py-2.5 px-3 text-center text-[11px]">
              <span class="px-2 py-0.5 rounded bg-[var(--bg-elevated)] text-[var(--text-secondary)] font-medium border border-[var(--border-subtle)]">HL: <b class="${o.hyperliquid_side === 'Short' ? 'text-rose-500' : 'text-emerald-500'}">${o.hyperliquid_side}</b> / BN: <b class="${o.binance_side === 'Short' ? 'text-rose-500' : 'text-emerald-500'}">${o.binance_side}</b></span>
            </td>
            <td class="py-2.5 px-2 text-right ${o.projected_1h_net_bps > 0 ? 'text-emerald-500 font-semibold' : 'text-slate-400'}">${o.projected_1h_net_bps > 0 ? '+' : ''}${o.projected_1h_net_bps.toFixed(2)} bps</td>
            <td class="py-2.5 px-3 text-right">
              <button data-symbol="${o.symbol}" class="btn-top-open px-2.5 py-1 rounded bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 font-medium border border-emerald-500/25 text-[11px] transition active:scale-95">
                Open
              </button>
            </td>
          </tr>
        `).join('')}
      </tbody>
    </table>
  `;

  // Bind Open Trade buttons
  container.querySelectorAll('.btn-top-open').forEach(btn => {
    btn.addEventListener('click', () => {
      const sym = btn.getAttribute('data-symbol');
      if (sym && onOpenTrade) onOpenTrade(sym);
    });
  });
}

/**
 * BHyper Terminal - Dashboard Overview Component
 */
import { formatCurrency, formatPnl, formatPrice } from '../utils/format.js';

export function renderWalletStats(wallet) {
  if (!wallet) return;

  const bnEq = (wallet.binance.cash_balance_usd || 0) + (wallet.binance.unrealized_pnl_usd || 0);
  const hlEq = (wallet.hyperliquid.cash_balance_usd || 0) + (wallet.hyperliquid.unrealized_pnl_usd || 0);
  const totalEq = bnEq + hlEq;

  const elTotalEq = document.getElementById('stat-total-equity');
  if (elTotalEq) elTotalEq.innerText = formatCurrency(totalEq);
  
  const elBnEq = document.getElementById('stat-bn-equity');
  if (elBnEq) elBnEq.innerText = formatCurrency(bnEq);

  const elHlEq = document.getElementById('stat-hl-equity');
  if (elHlEq) elHlEq.innerText = formatCurrency(hlEq);

  // 资金费与手续费
  const totalFees = (wallet.binance.total_fees_paid_usd || 0) + (wallet.hyperliquid.total_fees_paid_usd || 0);
  const totalFunding = (wallet.binance.total_funding_usd || 0) + (wallet.hyperliquid.total_funding_usd || 0);
  const netCarry = totalFunding - totalFees;

  const elNetCarry = document.getElementById('stat-funding-income');
  if (elNetCarry) {
    elNetCarry.innerText = formatPnl(netCarry, 4);
    elNetCarry.className = `text-2xl font-num font-bold mt-1.5 tracking-tight ${netCarry >= 0 ? 'text-emerald-500' : 'text-rose-500'}`;
  }

  const elGross = document.getElementById('stat-gross-funding');
  if (elGross) elGross.innerText = `+${formatCurrency(totalFunding, 4)}`;

  const elFees = document.getElementById('stat-total-fees');
  if (elFees) elFees.innerText = `-${formatCurrency(totalFees, 4)}`;

  // 保证金利用率
  const totalAllocated = (wallet.binance.allocated_margin_usd || 0) + (wallet.hyperliquid.allocated_margin_usd || 0);
  const utilPct = totalEq > 0 ? (totalAllocated / totalEq) * 100 : 0;
  const elMarginUtil = document.getElementById('stat-margin-util');
  if (elMarginUtil) elMarginUtil.innerText = `(已用 ${utilPct.toFixed(1)}%)`;

  // 保证金进度条
  const bnAlloc = wallet.binance.allocated_margin_usd || 0;
  const hlAlloc = wallet.hyperliquid.allocated_margin_usd || 0;
  const bnUtil = bnEq > 0 ? (bnAlloc / bnEq) * 100 : 0;
  const hlUtil = hlEq > 0 ? (hlAlloc / hlEq) * 100 : 0;

  const elBnHealth = document.getElementById('bn-health-util');
  if (elBnHealth) elBnHealth.innerText = `已用 ${formatCurrency(bnAlloc)} (${bnUtil.toFixed(1)}%)`;
  const elBnBar = document.getElementById('bn-util-bar');
  if (elBnBar) elBnBar.style.width = `${Math.min(bnUtil, 100)}%`;
  const elBnFree = document.getElementById('bn-free-margin');
  if (elBnFree) elBnFree.innerText = formatCurrency(Math.max(0, bnEq - bnAlloc));

  const elHlHealth = document.getElementById('hl-health-util');
  if (elHlHealth) elHlHealth.innerText = `已用 ${formatCurrency(hlAlloc)} (${hlUtil.toFixed(1)}%)`;
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
      badge.className = 'px-2 py-0.5 rounded text-[11px] font-medium bg-amber-500/10 text-amber-500 border border-amber-500/20';
      badge.innerText = '建议再平衡';
    }
    if (advText) {
      advText.innerHTML = `<i data-lucide="alert-circle" class="w-3.5 h-3.5 text-amber-500 inline mr-1"></i><span>${assessment.risk_status || '需执行跨所资金划转'}</span>`;
    }
    if (healthBadge) {
      healthBadge.className = 'px-1.5 py-0.2 rounded bg-amber-500/10 text-amber-500 text-[10px] font-semibold border border-amber-500/20';
      healthBadge.innerText = 'REBALANCE';
    }
  } else {
    if (badge) {
      badge.className = 'px-2 py-0.5 rounded text-[11px] font-medium bg-emerald-500/10 text-emerald-500 border border-emerald-500/20';
      badge.innerText = '资金完全平衡';
    }
    if (advText) {
      advText.innerHTML = `<i data-lucide="check-circle-2" class="w-3.5 h-3.5 text-emerald-500 inline mr-1"></i><span>${assessment.risk_status || '两所资金配比健康，无需执行划转。'}</span>`;
    }
    if (healthBadge) {
      healthBadge.className = 'px-1.5 py-0.2 rounded bg-emerald-500/10 text-emerald-500 text-[10px] font-semibold border border-emerald-500/20';
      healthBadge.innerText = 'HEALTHY';
    }
  }
}

export function renderDashboardTopRadar(opportunities, onOpenTrade) {
  const container = document.getElementById('dashboard-top-radar-table');
  if (!container) return;

  const top5 = (opportunities || []).slice(0, 5);
  if (top5.length === 0) {
    container.innerHTML = `<div class="text-center py-6 text-[var(--text-muted)] text-xs">暂无数据</div>`;
    return;
  }

  container.innerHTML = `
    <table class="w-full text-left border-collapse text-xs">
      <thead>
        <tr class="border-b border-[var(--border-subtle)] text-[var(--text-muted)] text-[11px] font-medium">
          <th class="py-2 px-3">标的</th>
          <th class="py-2 px-2">两所标记价 (BN / HL)</th>
          <th class="py-2 px-2 text-right">年化净利差 (APR)</th>
          <th class="py-2 px-3 text-center">对冲方向</th>
          <th class="py-2 px-2 text-right">预计1h净现金流</th>
          <th class="py-2 px-3 text-right">操作</th>
        </tr>
      </thead>
      <tbody class="divide-y divide-[var(--border-subtle)] font-num">
        ${top5.map(o => `
          <tr class="hover:bg-[var(--table-hover)] transition">
            <td class="py-2 px-3 font-semibold text-[var(--text-primary)]">${o.symbol}</td>
            <td class="py-2 px-2 text-[var(--text-muted)]">${formatPrice(o.binance_mark_price)} / ${formatPrice(o.hyperliquid_mark_price)}</td>
            <td class="py-2 px-2 text-right font-bold text-emerald-500">${o.net_spread_apr_pct.toFixed(2)}%</td>
            <td class="py-2 px-3 text-center text-[11px]">
              <span class="px-2 py-0.5 rounded bg-[var(--bg-elevated)] text-[var(--text-secondary)] font-medium border border-[var(--border-subtle)]">HL: ${o.hyperliquid_side} / BN: ${o.binance_side}</span>
            </td>
            <td class="py-2 px-2 text-right ${o.projected_1h_net_bps > 0 ? 'text-emerald-500 font-medium' : 'text-slate-400'}">${o.projected_1h_net_bps > 0 ? '+' : ''}${o.projected_1h_net_bps.toFixed(2)} bps</td>
            <td class="py-2 px-3 text-right">
              <button data-symbol="${o.symbol}" class="btn-top-open px-2 py-0.5 rounded bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 font-medium border border-emerald-500/20 text-[11px] transition">
                开仓
              </button>
            </td>
          </tr>
        `).join('')}
      </tbody>
    </table>
  `;

  // 绑定事件
  container.querySelectorAll('.btn-top-open').forEach(btn => {
    btn.addEventListener('click', () => {
      const sym = btn.getAttribute('data-symbol');
      if (sym && onOpenTrade) onOpenTrade(sym);
    });
  });
}

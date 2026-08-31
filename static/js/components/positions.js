/**
 * BHyper Terminal - Active Positions Component (Deterministic & Non-Jumping)
 * Pure English Institutional Layout
 */
import { formatCurrency, formatPrice, formatTimeUtc8 } from '../utils/format.js';

export function renderPositions(livePositions, paperPositions, onUnwind) {
  const all = [...(livePositions || []), ...(paperPositions || [])];

  // 1. Core deterministic anti-jitter sort by symbol
  all.sort((a, b) => a.symbol.localeCompare(b.symbol));

  // Update badge counts
  const countBadge = document.getElementById('pos-count-badge');
  const mobBadge = document.getElementById('mob-pos-badge');
  if (countBadge) countBadge.innerText = all.length;
  if (mobBadge) {
    mobBadge.className = all.length > 0 ? 'absolute top-1 right-2 w-2 h-2 rounded-full bg-amber-500 inline-block' : 'hidden';
  }
  const elActivePairs = document.getElementById('stat-active-pairs');
  if (elActivePairs) elActivePairs.innerText = `${all.length} Pairs`;

  // Calculate weighted projected hourly funding run-rate
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

  // 2. Dashboard Deck Summary
  const dashDeck = document.getElementById('dashboard-positions-deck');
  if (dashDeck) {
    if (all.length === 0) {
      dashDeck.innerHTML = `
        <div class="text-center py-6 text-xs text-ink-mute border border-dashed border-edge rounded-lg">
          <i data-lucide="shield-check" class="w-5 h-5 text-ink-mute/70 mx-auto mb-1"></i>
          <span>No active hedged positions. Execution engine is actively scanning...</span>
        </div>
      `;
    } else {
      dashDeck.innerHTML = all.map(p => {
        const funding = p.total_funding_usd !== undefined ? p.total_funding_usd : (p.accumulated_funding_usd || 0);
        return `
          <div class="bg-elevated p-3 rounded-lg border border-edge flex items-center justify-between font-num">
            <div class="flex items-center space-x-3">
              <span class="font-bold text-xs text-ink">${p.symbol}</span>
              <span class="px-2 py-0.5 rounded text-2xs font-medium bg-subtle text-ink-mute border border-edge">BN: ${p.binance_side} / HL: ${p.hyperliquid_side}</span>
              <span class="text-xs text-ink-mute">Notional: <b class="text-ink font-semibold">${formatCurrency(p.nominal_value_usd || 0)}</b></span>
            </div>
            <div class="flex items-center space-x-3">
              <div class="text-right text-xs">
                <span class="text-ink-mute">Accrued Carry: </span>
                <span class="text-cyan-500 font-semibold">+${formatCurrency(funding, 4)}</span>
              </div>
              <button data-symbol="${p.symbol}" class="btn-pos-unwind px-2.5 py-1 rounded-md bg-rose-500/10 hover:bg-rose-500/20 text-rose-500 font-medium text-xs border border-rose-500/25 transition active:scale-95">
                Close
              </button>
            </div>
          </div>
        `;
      }).join('');
    }
  }

  // 3. Positions Tab Cards Deck
  const container = document.getElementById('positions-container');
  if (!container) return;

  if (all.length === 0) {
    container.innerHTML = `
      <div class="surface-card rounded-xl p-10 text-center space-y-2.5">
        <div class="w-12 h-12 rounded-full bg-subtle flex items-center justify-center text-ink-mute mx-auto">
          <i data-lucide="shield-check" class="w-6 h-6"></i>
        </div>
        <div class="font-semibold text-xs text-ink">No Running Arbitrage Positions</div>
        <p class="text-xs text-ink-mute max-w-sm mx-auto">The execution daemon is active. Hedged pairs will be entered automatically when cross-exchange spreads satisfy entry APR thresholds.</p>
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
        <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 border-b border-edge pb-3">
          <div class="flex items-center space-x-3">
            <span class="w-8 h-8 rounded-lg bg-elevated border border-edge flex items-center justify-center font-bold text-ink-soft text-xs">${p.symbol.slice(0, 2)}</span>
            <div>
              <div class="flex items-center space-x-2">
                <span class="text-sm font-bold text-ink">${p.symbol} Delta-Neutral Pair</span>
                ${isLive ? '<span class="px-2 py-0.2 rounded text-2xs font-semibold bg-emerald-500/10 text-emerald-500 border border-emerald-500/25">LIVE</span>' : '<span class="px-2 py-0.2 rounded text-2xs font-semibold bg-blue-500/10 text-blue-400 border border-blue-500/25">SIMULATED</span>'}
              </div>
              <div class="text-2xs text-ink-mute mt-0.5">Opened: ${formatTimeUtc8(p.opened_at)} (UTC+8)</div>
            </div>
          </div>
          <div class="flex items-center space-x-3">
            <div class="text-right">
              <div class="text-xs text-ink-mute">Notional Value</div>
              <div class="text-sm font-bold text-ink">${formatCurrency(p.nominal_value_usd || 0)}</div>
            </div>
            <button data-symbol="${p.symbol}" class="btn-pos-unwind px-3 py-1.5 rounded-lg bg-rose-500/10 hover:bg-rose-500/20 text-rose-500 font-medium text-xs border border-rose-500/25 transition flex items-center space-x-1 active:scale-95">
              <i data-lucide="x-circle" class="w-3.5 h-3.5"></i>
              <span>Unwind Pair</span>
            </button>
          </div>
        </div>

        <!-- Two Legs Detail Deck -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
          <!-- Binance Leg -->
          <div class="bg-elevated p-3 rounded-lg border border-edge space-y-1.5">
            <div class="flex justify-between text-xs">
              <span class="font-semibold text-amber-500 flex items-center space-x-1">
                <span class="w-1.5 h-1.5 rounded-full bg-amber-500"></span>
                <span>Binance Leg</span>
              </span>
              <span class="font-semibold ${p.binance_side === 'Long' ? 'text-emerald-500' : 'text-rose-500'}">${p.binance_side} ${p.binance_qty} ${p.symbol}</span>
            </div>
            <div class="flex justify-between text-2xs text-ink-mute">
              <span>Entry Price: <b class="text-ink font-semibold">${formatPrice(p.binance_entry_price)}</b></span>
              <span>Entry Fee: <b class="text-rose-500">-${formatCurrency(bnFee, 4)}</b></span>
            </div>
          </div>

          <!-- Hyperliquid Leg -->
          <div class="bg-elevated p-3 rounded-lg border border-edge space-y-1.5">
            <div class="flex justify-between text-xs">
              <span class="font-semibold text-cyan-500 flex items-center space-x-1">
                <span class="w-1.5 h-1.5 rounded-full bg-cyan-500"></span>
                <span>Hyperliquid Leg</span>
              </span>
              <span class="font-semibold ${p.hyperliquid_side === 'Long' ? 'text-emerald-500' : 'text-rose-500'}">${p.hyperliquid_side} ${p.hyperliquid_qty} ${p.symbol}</span>
            </div>
            <div class="flex justify-between text-2xs text-ink-mute">
              <span>Entry Price: <b class="text-ink font-semibold">${formatPrice(p.hyperliquid_entry_price)}</b></span>
              <span>Entry Fee: <b class="${hlFee === 0 ? 'text-emerald-500' : 'text-rose-500'} font-semibold">${hlFee === 0 ? '$0.00 (Maker)' : `-${formatCurrency(hlFee, 4)}`}</b></span>
            </div>
          </div>
        </div>

        <!-- Funding & PnL Ribbon -->
        <div class="grid grid-cols-3 gap-2 bg-elevated p-2.5 rounded-lg border border-edge text-center text-xs">
          <div>
            <div class="text-2xs text-ink-mute">Entry Spread APR</div>
            <div class="font-semibold text-ink text-xs">${(p.entry_spread_apr || 0).toFixed(2)}%</div>
          </div>
          <div>
            <div class="text-2xs text-ink-mute">Accrued Carry</div>
            <div class="font-bold text-cyan-500 text-xs">+${formatCurrency(funding, 4)} (${ticksCount} settlements)</div>
          </div>
          <div>
            <div class="text-2xs text-ink-mute">Unrealized Basis PnL</div>
            <div class="font-bold ${pnlColor} text-xs">${pnl >= 0 ? '+' : ''}${formatCurrency(pnl, 4)}</div>
          </div>
        </div>

      </div>
    `;
  }).join('');

  // Bind Unwind buttons
  document.querySelectorAll('.btn-pos-unwind').forEach(btn => {
    btn.addEventListener('click', () => {
      const sym = btn.getAttribute('data-symbol');
      if (sym && onUnwind) onUnwind(sym);
    });
  });
}

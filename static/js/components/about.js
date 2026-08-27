/**
 * BHyper Terminal - System Architecture & Methodology Component
 * Pure English Institutional Documentation & Architecture Explorer
 */

export function renderAboutSection() {
  const container = document.getElementById('tab-about');
  if (!container) return;

  container.innerHTML = `
    <div class="space-y-6">
      
      <!-- Header Banner -->
      <div class="surface-card p-5 sm:p-6 border-l-4 border-l-emerald-500">
        <div class="flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div>
            <div class="flex items-center space-x-2 text-emerald-500 font-semibold text-xs tracking-wider uppercase">
              <i data-lucide="shield-check" class="w-4 h-4"></i>
              <span>Institutional Quantitative Architecture</span>
            </div>
            <h2 class="text-xl font-bold text-[var(--text-primary)] mt-1 tracking-tight">Delta-Neutral Funding Arbitrage & Carry Engine</h2>
            <p class="text-xs text-[var(--text-secondary)] mt-1 max-w-2xl leading-relaxed">
              BHyper is a high-frequency, sub-millisecond execution engine that captures cross-exchange perpetual funding rate disparities between Hyperliquid L1 and Binance Futures while maintaining zero net market delta.
            </p>
          </div>
          <div class="flex items-center space-x-2">
            <span class="px-2.5 py-1 rounded-lg bg-emerald-500/10 text-emerald-500 border border-emerald-500/25 font-num font-semibold text-xs">
              Δ ≈ 0.0000 Neutral
            </span>
            <span class="px-2.5 py-1 rounded-lg bg-cyan-500/10 text-cyan-500 border border-cyan-500/25 font-num font-semibold text-xs">
              Rust 2024 Edition
            </span>
          </div>
        </div>
      </div>

      <!-- Core Mechanism Pillars (3 Columns) -->
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        
        <!-- Pillar 1: Delta Neutrality -->
        <div class="surface-card p-4 space-y-2.5">
          <div class="w-8 h-8 rounded-lg bg-emerald-500/10 text-emerald-500 flex items-center justify-center border border-emerald-500/20">
            <i data-lucide="scale" class="w-4 h-4"></i>
          </div>
          <h3 class="font-bold text-sm text-[var(--text-primary)]">1. Pure Delta Neutrality</h3>
          <p class="text-xs text-[var(--text-muted)] leading-relaxed">
            Simultaneously enters an exact equal-sized Long on Exchange A and Short on Exchange B. Zero directional price risk to underlying market fluctuations, isolating 100% of yield from the net funding spread.
          </p>
          <div class="pt-2 border-t border-[var(--border-subtle)] text-[11px] font-num text-emerald-500 font-medium">
            Risk Profile: Market Direction Agnostic
          </div>
        </div>

        <!-- Pillar 2: Asymmetric Settlement -->
        <div class="surface-card p-4 space-y-2.5">
          <div class="w-8 h-8 rounded-lg bg-cyan-500/10 text-cyan-500 flex items-center justify-center border border-cyan-500/20">
            <i data-lucide="clock" class="w-4 h-4"></i>
          </div>
          <h3 class="font-bold text-sm text-[var(--text-primary)]">2. Asymmetric Settlement</h3>
          <p class="text-xs text-[var(--text-muted)] leading-relaxed">
            Hyperliquid settles hourly (1h cadence, continuous L1 yield) while Binance settles every 8 hours. The mathematical engine normalizes both rates to annualized 365-day APR with real-time fee amortization.
          </p>
          <div class="pt-2 border-t border-[var(--border-subtle)] text-[11px] font-num text-cyan-500 font-medium">
            Normalization: APR = Spread * 8760h
          </div>
        </div>

        <!-- Pillar 3: Dynamic Swapper -->
        <div class="surface-card p-4 space-y-2.5">
          <div class="w-8 h-8 rounded-lg bg-purple-500/10 text-purple-500 flex items-center justify-center border border-purple-500/20">
            <i data-lucide="repeat" class="w-4 h-4"></i>
          </div>
          <h3 class="font-bold text-sm text-[var(--text-primary)]">3. Dynamic Opportunity Swapper</h3>
          <p class="text-xs text-[var(--text-muted)] leading-relaxed">
            Continuously evaluates holding opportunity cost. When a newly emergent pair offers an APR delta exceeding swap friction costs after fees, capital is automatically rotated to maximize aggregate yield.
          </p>
          <div class="pt-2 border-t border-[var(--border-subtle)] text-[11px] font-num text-purple-400 font-medium">
            Yield Optimization: Continuous Alpha Harvesting
          </div>
        </div>

      </div>

      <!-- Execution Pipeline Architecture -->
      <div class="surface-card p-5 space-y-4">
        <div class="flex items-center space-x-2">
          <i data-lucide="cpu" class="w-4 h-4 text-emerald-500"></i>
          <h3 class="font-bold text-sm text-[var(--text-primary)]">Low-Latency Execution Pipeline</h3>
        </div>

        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3 text-xs">
          
          <div class="bg-[var(--bg-elevated)] p-3.5 rounded-lg border border-[var(--border-subtle)] space-y-1.5">
            <div class="flex items-center space-x-1.5 text-cyan-400 font-semibold font-num">
              <span class="w-4 h-4 rounded-full bg-cyan-500/20 text-cyan-400 flex items-center justify-center text-[10px]">1</span>
              <span>Tick Ingestion</span>
            </div>
            <p class="text-[var(--text-muted)] text-[11px] leading-relaxed">
              Concurrent async WebSockets stream raw orderbook mids and funding rates for 200+ markets with zero GC pauses.
            </p>
          </div>

          <div class="bg-[var(--bg-elevated)] p-3.5 rounded-lg border border-[var(--border-subtle)] space-y-1.5">
            <div class="flex items-center space-x-1.5 text-emerald-400 font-semibold font-num">
              <span class="w-4 h-4 rounded-full bg-emerald-500/20 text-emerald-400 flex items-center justify-center text-[10px]">2</span>
              <span>Spread & Fee Matrix</span>
            </div>
            <p class="text-[var(--text-muted)] text-[11px] leading-relaxed">
              Computes net basis APR, round-trip taker/maker fee thresholds, and deterministic break-even payback windows.
            </p>
          </div>

          <div class="bg-[var(--bg-elevated)] p-3.5 rounded-lg border border-[var(--border-subtle)] space-y-1.5">
            <div class="flex items-center space-x-1.5 text-amber-400 font-semibold font-num">
              <span class="w-4 h-4 rounded-full bg-amber-500/20 text-amber-400 flex items-center justify-center text-[10px]">3</span>
              <span>Margin Sentinel</span>
            </div>
            <p class="text-[var(--text-muted)] text-[11px] leading-relaxed">
              Verifies liquidation safety buffers, margin health across both accounts, and enforces single-slot allocation caps.
            </p>
          </div>

          <div class="bg-[var(--bg-elevated)] p-3.5 rounded-lg border border-[var(--border-subtle)] space-y-1.5">
            <div class="flex items-center space-x-1.5 text-rose-400 font-semibold font-num">
              <span class="w-4 h-4 rounded-full bg-rose-500/20 text-rose-400 flex items-center justify-center text-[10px]">4</span>
              <span>Atomic Execution</span>
            </div>
            <p class="text-[var(--text-muted)] text-[11px] leading-relaxed">
              Submits EIP-712 typed order signatures to Hyperliquid L1 and HMAC-signed orders to Binance in parallel.
            </p>
          </div>

        </div>
      </div>

      <!-- Risk Controls & Safety Policies -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4 text-xs">
        
        <div class="surface-card p-4 space-y-3">
          <h3 class="font-bold text-sm text-[var(--text-primary)] flex items-center space-x-2">
            <i data-lucide="shield-alert" class="w-4 h-4 text-rose-500"></i>
            <span>Multi-Layer Risk Safeguards</span>
          </h3>
          <ul class="space-y-2 text-[var(--text-secondary)]">
            <li class="flex items-start space-x-2">
              <span class="text-rose-500 font-bold">•</span>
              <span><strong>Basis Divergence Stop-Loss:</strong> Auto-unwinds positions if cross-exchange price divergence widens beyond parameter thresholds.</span>
            </li>
            <li class="flex items-start space-x-2">
              <span class="text-emerald-500 font-bold">•</span>
              <span><strong>Basis Convergence Take-Profit:</strong> Locks in capital gains when spot-perp basis narrows to zero.</span>
            </li>
            <li class="flex items-start space-x-2">
              <span class="text-amber-500 font-bold">•</span>
              <span><strong>Cross-Exchange Margin Balancer:</strong> Monitors equity skew and triggers automated rebalance advisories when balance diverges.</span>
            </li>
          </ul>
        </div>

        <div class="surface-card p-4 space-y-3">
          <h3 class="font-bold text-sm text-[var(--text-primary)] flex items-center space-x-2">
            <i data-lucide="terminal" class="w-4 h-4 text-cyan-500"></i>
            <span>Interface & Telegram Ergonomics</span>
          </h3>
          <ul class="space-y-2 text-[var(--text-secondary)]">
            <li class="flex items-start space-x-2">
              <span class="text-cyan-500 font-bold">•</span>
              <span><strong>Hot-Reload Strategy Workbench:</strong> Parameter tuning writes atomically to disk and activates in memory with zero daemon downtime.</span>
            </li>
            <li class="flex items-start space-x-2">
              <span class="text-emerald-500 font-bold">•</span>
              <span><strong>Telegram WebApp Integration:</strong> Native HMAC validation, haptic feedback pulses on execution, and responsive single-thumb navigation.</span>
            </li>
            <li class="flex items-start space-x-2">
              <span class="text-purple-400 font-bold">•</span>
              <span><strong>Holographic Audit Journal:</strong> Deterministic immutable ledger recording all order intents, fills, fee receipts, and settlements.</span>
            </li>
          </ul>
        </div>

      </div>

    </div>
  `;

  if (window.lucide) {
    window.lucide.createIcons();
  }
}

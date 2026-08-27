# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Quantitative traders, delta-neutral arbitrageurs, crypto hedge fund managers, and institutional crypto capital operators who execute and monitor automated funding rate arbitrage between Hyperliquid L1 Perp and Binance USDT-M Futures.

## Product Purpose

BHyper is an ultra-reliable, institutional delta-neutral arbitrage and funding carry execution terminal. It automates 24/7 opportunity discovery across 200+ perpetual markets, computes net annualized carry yields (APR) after full round-trip fee deduction, executes atomic two-legged hedged positions, dynamically manages cross-exchange margin health, and enables hot-reload strategy parameter tuning with zero downtime.

## Positioning

Unlike generic crypto portfolio trackers or complex multi-hop DEX bots, BHyper provides a dedicated, deterministic, sub-millisecond cross-exchange execution engine pairing Hyperliquid's zero-gas L1 with Binance's deep liquidity, featuring dynamic opportunity cost swapping and automated liquidation protection.

## Operating Context

- Real-time 24/7 continuous operation on dedicated VPS infrastructure.
- High-frequency WebSockets broadcasting market ticks, live margin health, and position states.
- Dual-access modality: High-density desktop institutional terminal and one-thumb Telegram Mini App for on-the-go monitoring and emergency unwinds.
- Critical financial operations where latency, clarity, and precision prevent liquidation and slippage.

## Capabilities and Constraints

- **Live Opportunity Radar**: Real-time cross-exchange funding rate matrix, net APR calculation, break-even period estimation, and liquidity tier classification.
- **Deterministic Position Tracker**: Real-time tracking of dual-legged hedged positions, cumulative funding payments, and basis PnL with anti-jitter sorting.
- **Margin Sentinel & Health**: Cross-exchange margin utilization gauge, liquidation distance monitoring, and automated rebalance advisories.
- **Hot-Reload Strategy Workbench**: Dynamic configuration hot-reloading for open/exit APR thresholds, leverage, position caps, and opportunity-cost swapper settings.
- **Holographic Event Journal**: Full audit trail of intents, two-legged fills, hourly funding settlements, and risk alerts.
- **Emergency Unwind**: Instant atomic liquidation and market-neutral position unwinding.

## Brand Commitments

- **Name**: BHyper (Binance × Hyperliquid Institutional Terminal).
- **Tone & Voice**: Restrained, authoritative, precise, institutional, distraction-free.
- **Visual Identity**: Obsidian & Slate canvas, Emerald carry/profit, Cyan Hyperliquid funding, Amber Binance/warning, Rose danger/risk.

## Evidence on Hand

- Native Rust backend (`axum`, `tokio`, WebSocket feed, REST API routes).
- Production endpoints: `/api/status`, `/api/scan`, `/api/positions`, `/api/health`, `/api/config`, `/api/journal`, `/api/ws`.
- Telegram WebApp SDK (`telegram-web-app.js`) integration for native mobile haptic feedback and authentication.

## Product Principles

1. **Precision & Tabular Stability**: All financial metrics (prices, APR, funding payments, timestamps) must use tabular monospace formatting (`JetBrains Mono`) to eliminate layout shift.
2. **Restrained Institutional Clarity**: High information density without visual noise. Restrained palettes, subtle translucent elevations, and clear hierarchy over flashy gradients.
3. **Failsafe Ergonomics**: Every action (especially emergency unwind and config saves) must provide immediate, unambiguous visual and haptic feedback.
4. **Desktop Depth & Mobile Agility**: Seamless adaptation from multi-column desktop workstation to single-thumb mobile Telegram Mini App.

## Accessibility & Inclusion

- WCAG AA compliant contrast ratios (≥ 4.5:1 for body and data text).
- Full keyboard navigability (Esc modal dismissal, focus rings, logical tab order).
- Respect `prefers-reduced-motion` for accessibility without sacrificing feedback.

---
name: BHyper Terminal
description: Institutional Quantitative Arbitrage & Delta-Neutral Carry Terminal
colors:
  bg-canvas-dark: "#090d16"
  bg-surface-dark: "#0f172a"
  bg-elevated-dark: "#162035"
  bg-hover-dark: "#1e2d4a"
  bg-active-dark: "#26395c"
  border-subtle-dark: "rgba(255, 255, 255, 0.08)"
  border-strong-dark: "rgba(255, 255, 255, 0.15)"
  border-focus: "#10b981"
  text-primary-dark: "#f8fafc"
  text-secondary-dark: "#94a3b8"
  text-muted-dark: "#64748b"
  accent-carry: "#10b981"
  accent-carry-bg: "rgba(16, 185, 129, 0.12)"
  accent-loss: "#f43f5e"
  accent-loss-bg: "rgba(244, 63, 94, 0.12)"
  accent-funding: "#06b6d4"
  accent-funding-bg: "rgba(6, 182, 212, 0.12)"
  accent-binance: "#f59e0b"
  accent-binance-bg: "rgba(245, 158, 11, 0.12)"
typography:
  display:
    fontFamily: "'Inter', system-ui, -apple-system, sans-serif"
    fontSize: "1.5rem"
    fontWeight: 700
    lineHeight: "1.2"
    letterSpacing: "-0.025em"
  headline:
    fontFamily: "'Inter', system-ui, -apple-system, sans-serif"
    fontSize: "1.125rem"
    fontWeight: 600
    lineHeight: "1.3"
    letterSpacing: "-0.015em"
  title:
    fontFamily: "'Inter', system-ui, -apple-system, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 600
    lineHeight: "1.4"
    letterSpacing: "-0.01em"
  body:
    fontFamily: "'Inter', 'Noto Sans SC', system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: "1.5"
    letterSpacing: "-0.01em"
  label:
    fontFamily: "'Inter', system-ui, sans-serif"
    fontSize: "0.6875rem"
    fontWeight: 600
    lineHeight: "1.2"
    letterSpacing: "0.04em"
  numeric:
    fontFamily: "'JetBrains Mono', monospace"
    fontSize: "0.8125rem"
    fontWeight: 500
    lineHeight: "1.4"
    fontFeature: "tnum 1, zero 1"
rounded:
  sm: "6px"
  md: "8px"
  lg: "12px"
  full: "9999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "24px"
components:
  button-primary:
    backgroundColor: "{colors.accent-carry}"
    textColor: "#ffffff"
    rounded: "{rounded.md}"
    padding: "6px 14px"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.text-secondary-dark}"
    rounded: "{rounded.md}"
    padding: "6px 10px"
---

# Design System: BHyper Terminal

## Overview

**Creative North Star: "The Obsidian Quant Terminal"**

BHyper is designed for professional delta-neutral arbitrage and carry traders who spend hours analyzing fast-moving spreads, managing multi-exchange margin health, and executing critical trades. The interface emphasizes razor-sharp clarity, dense information architecture without visual clutter, and instant cognitive recognition.

The system adopts a restrained dark obsidian foundation paired with translucent layered elevations and precision semantic accents. Financial metrics always use tabular monospace typography to guarantee zero layout jitter during high-frequency WebSocket updates.

**Key Characteristics:**
- Deep obsidian and slate surfaces with 1px translucent borders (`border-subtle`).
- Tabular monospace numbers (`JetBrains Mono`) for all financial rates, prices, and timestamps.
- Highly disciplined semantic color coding (Carry Emerald, Loss Rose, Hyperliquid Cyan, Binance Amber).
- Zero extraneous visual ornament: no rainbow gradients, no heavy drop-shadows, no slow animated transitions.
- Fully responsive layout optimized for multi-screen desktop monitoring and Telegram Mini App single-thumb control.

## Colors

The palette character is restrained, high-contrast, and functionally semantic.

### Primary
- **Carry Emerald** (`#10b981` / `rgba(16, 185, 129, 0.12)`): Primary actions, positive realized/unrealized PnL, profitable net carry APR, and healthy system status.

### Secondary
- **Hyperliquid Cyan** (`#06b6d4` / `rgba(6, 182, 212, 0.12)`): Hyperliquid exchange data, cumulative funding income, and 1h projected cashflow indicators.
- **Binance Amber** (`#f59e0b` / `rgba(245, 158, 11, 0.12)`): Binance exchange data, margin utilization warnings, and rebalancing alerts.
- **Risk Rose** (`#f43f5e` / `rgba(244, 63, 94, 0.12)`): Negative PnL, stop-loss triggers, emergency unwind buttons, and disconnection alerts.

### Neutral
- **Obsidian Canvas** (`#090d16`): The foundational background canvas for dark mode.
- **Surface Layer** (`#0f172a`): Card backgrounds and primary content containers.
- **Elevated Layer** (`#162035`): Inner panels, table headers, and secondary wells.
- **Border Subtle** (`rgba(255, 255, 255, 0.08)`): 1px structural container divider lines.
- **Text Primary** (`#f8fafc`): Primary headers, active values, and table numbers.
- **Text Secondary** (`#94a3b8`): Metric labels, secondary descriptions, and column titles.
- **Text Muted** (`#64748b`): Tertiary hints, timestamps, and inactive controls (WCAG AA compliant ≥ 4.5:1).

### Named Rules
**The One Meaning Rule.** A color never changes its semantic meaning across screens. Emerald always means positive carry/gain/health; Rose always means risk/loss/exit; Cyan always represents Hyperliquid; Amber always represents Binance.

**The 10% Accent Rule.** Accent hues occupy less than 10% of any viewport. The vast majority of the interface is neutral slate/obsidian to prevent visual fatigue.

## Typography

**Display & Body Font:** Inter (Latin) paired with Noto Sans SC (Simplified Chinese)
**Data & Numeric Font:** JetBrains Mono (with `tnum` tabular numbers and slashed zero)

**Character:** Clean, highly legible, modern sans-serif paired with a high-precision monospace for financial figures.

### Hierarchy
- **Display** (Bold 700, 24px / 1.5rem, line-height 1.2, tracking -0.025em): Main KPI card totals and major modal titles.
- **Headline** (SemiBold 600, 18px / 1.125rem, line-height 1.3, tracking -0.015em): Section titles and tab headers.
- **Title** (SemiBold 600, 14px / 0.875rem, line-height 1.4, tracking -0.01em): Sub-section headers, position pair names.
- **Body** (Regular 400, 13px / 0.8125rem, line-height 1.5): Descriptive copy, modal dialog bodies, and table content.
- **Label** (SemiBold 600, 11px / 0.6875rem, tracking 0.04em, uppercase): Column headers, metric categories, status badges.
- **Numeric** (Medium 500, 12-14px, `font-variant-numeric: tabular-nums`): All prices, spreads, basis bps, timestamps, and balances.

### Named Rules
**The Tabular Number Invariant.** Every number that updates via WebSocket or represents financial value MUST use `.font-num` (`JetBrains Mono` with `tabular-nums`). Numbers must never cause horizontal layout shifts when changing values.

## Layout

- **Container Model**: Max-width `80rem` (`1280px`) centered layout on desktop with `16px`-`20px` horizontal gutter padding.
- **Grid Structure**: 4-column KPI ribbon on desktop (`grid-cols-4`), collapsing gracefully to 2 columns on tablet (`grid-cols-2`) and 2 columns on mobile.
- **Density**: High information density with strict 8px/12px/16px spatial rhythm (`p-3` / `p-4`).
- **Responsive Navigation**: Desktop uses top segmented tabs (`nav-btn`); mobile uses fixed bottom thumb-accessible navigation bar with safe-area padding.

## Elevation & Depth

Surfaces rely on subtle translucent layering and fine 1px border dividers rather than heavy drop-shadows.

### Shadow Vocabulary
- **Card Subtle** (`0 1px 3px 0 rgba(0, 0, 0, 0.2), 0 1px 2px -1px rgba(0, 0, 0, 0.2)`): Default elevation for surface cards.
- **Modal Elevation** (`0 20px 25px -5px rgba(0, 0, 0, 0.5), 0 8px 10px -6px rgba(0, 0, 0, 0.5)`): Deep scrim elevation for confirmation modals.

### Named Rules
**The Single Elevation Rule.** Depth is established by background luminance contrast (`--bg-canvas` < `--bg-surface` < `--bg-elevated`) and 1px borders, not stacked shadows.

## Shapes

- **Surface Containers**: `12px` (`0.75rem` / `rounded-xl`) for main cards and matrix tables.
- **Interactive Controls**: `8px` (`0.5rem` / `rounded-lg`) for buttons, inputs, and search bars.
- **Status Chips & Pills**: `6px` (`rounded-md`) or full pill (`rounded-full`) for compact tags.
- **Dividers & Strokes**: Crisp `1px solid var(--border-subtle)`.

## Components

### Buttons
- **Shape**: `rounded-lg` (8px), padding `6px 12px` for normal actions, `4px 8px` for table inline actions.
- **Primary / Action**: Emerald background (`bg-emerald-500 hover:bg-emerald-600`), text white, subtle `active:scale-[0.98]` micro-interaction.
- **Danger / Unwind**: Rose tint background (`bg-rose-500/10 hover:bg-rose-500/20 text-rose-500 border border-rose-500/25`).
- **Secondary / Ghost**: Elevated background (`bg-[var(--bg-elevated)] hover:bg-[var(--bg-hover)] text-[var(--text-secondary)]`).
- **Focus Ring**: `focus-visible:outline-2 focus-visible:outline-emerald-500 focus-visible:outline-offset-2`.

### Chips & Badges
- **Style**: Compact height (20px), font-size 10px-11px, semi-bold font weight, subtle border.
- **Variants**: `.chip-emerald` (Live/Profit), `.chip-cyan` (Hyperliquid/Funding), `.chip-amber` (Binance/Rebalance), `.chip-rose` (Risk/Loss), `.chip-neutral` (Default/Info).

### Cards & Tables
- **Card**: Surface background, 1px subtle border, padding 16px (`p-4`), rounded 12px.
- **Table**: Dense rows (`py-2.5 px-3`), crisp sticky-feeling headers with elevated background, smooth hover background transition (`150ms`).

### Inputs & Selects
- **Style**: Surface background, 1px border, 8px radius, padding 8px 12px, font-num for numeric inputs.
- **Focus**: Emerald border with 2px translucent emerald outline.

## Do's and Don'ts

### Do:
- **Do** format all financial figures with fixed decimal places and tabular numbers.
- **Do** ensure all interactive elements have visible `:focus-visible` rings for keyboard navigation.
- **Do** display clear loading skeletons during data fetching and reconnection phases.
- **Do** keep button click and tab transition animations between 150ms and 200ms.
- **Do** test all screens in both Dark (Obsidian) and Light (Studio) modes.

### Don't:
- **Don't** use multi-color gradient text or decorative zero-blur heavy block shadows.
- **Don't** allow numbers or cards to jump or shift when live WebSocket ticks arrive.
- **Don't** use emojis instead of consistent vector icons (`Lucide`).
- **Don't** hide critical risk states, liquidation distances, or margin warnings behind obscure menus.
- **Don't** introduce slow, multi-stage choreographed page load animations.

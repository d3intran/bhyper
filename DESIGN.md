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
    fontFamily: "'IBM Plex Sans', 'Noto Sans SC', system-ui, sans-serif"
    fontSize: "1.5rem"
    fontWeight: 700
    lineHeight: "1.15"
    letterSpacing: "-0.025em"
  headline:
    fontFamily: "'IBM Plex Sans', 'Noto Sans SC', system-ui, sans-serif"
    fontSize: "1.125rem"
    fontWeight: 600
    lineHeight: "1.35"
    letterSpacing: "-0.015em"
  lede:
    fontFamily: "'IBM Plex Sans', 'Noto Sans SC', system-ui, sans-serif"
    fontSize: "1rem"
    fontWeight: 600
    lineHeight: "1.4"
    letterSpacing: "-0.012em"
  title:
    fontFamily: "'IBM Plex Sans', 'Noto Sans SC', system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 600
    lineHeight: "1.45"
    letterSpacing: "-0.01em"
  body:
    fontFamily: "'IBM Plex Sans', 'Noto Sans SC', system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: "1.5"
    letterSpacing: "-0.006em"
  caption:
    fontFamily: "'IBM Plex Sans', 'Noto Sans SC', system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 400
    lineHeight: "1.4"
    letterSpacing: "0em"
  label:
    fontFamily: "'IBM Plex Sans', 'Noto Sans SC', system-ui, sans-serif"
    fontSize: "0.6875rem"
    fontWeight: 600
    lineHeight: "1.45"
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
- Cool near-black obsidian surfaces with 1px translucent borders (`border-edge`).
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

Every neutral is a CSS custom property with two values — one under `:root` (Studio / light), one under `html.dark` (Obsidian). Themes swap by toggling a single class on `<html>`; no component ever branches on theme.

| Token | Tailwind class | Obsidian (dark) | Studio (light) |
|---|---|---|---|
| Canvas | `bg-canvas` | `#0a0b0e` | `#fafafb` |
| Surface | `bg-surface` | `#101216` | `#ffffff` |
| Elevated | `bg-elevated` | `#171a1f` | `#f4f5f7` |
| Subtle | `bg-subtle` | `#1f232a` | `#e9ebef` |
| Hover | `hover:bg-hover` | `#1f232a` | `#e9ebef` |
| Header | `bg-header` | `rgba(10,11,14,.94)` | `rgba(255,255,255,.94)` |
| Inverse | `bg-inverse` | `#1f232a` | `#101216` |
| Scrim | `bg-scrim` | `rgba(0,0,0,.7)` | `rgba(12,13,15,.35)` |
| Edge | `border-edge` | `rgba(255,255,255,.07)` | `rgba(12,13,15,.09)` |
| Edge strong | `border-edge-strong` | `rgba(255,255,255,.13)` | `rgba(12,13,15,.16)` |
| Ink | `text-ink` | `#f7f8f9` | `#0c0d0f` |
| Ink soft | `text-ink-soft` | `#a0a7b4` | `#4e545e` |
| Ink mute | `text-ink-mute` | `#6d7480` | `#71777f` |

The dark neutrals are a **cool near-black** (`#0a0b0e`), not a blue slate. Blue-shifted darks tint every accent sitting on them and read as "default theme" rather than a deliberate choice.

**Inverse surface** (`bg-inverse` / `text-inverse`) carries content that must float clear of the canvas — toasts and popovers. In Obsidian it is a lifted `#1f232a`; in Studio it drops to `#101216`. A near-black toast in dark mode, which is what shipped before, disappeared into the canvas.

### Named Rules
**The One Meaning Rule.** A color never changes its semantic meaning across screens. Emerald always means positive carry/gain/health; Rose always means risk/loss/exit; Cyan always represents Hyperliquid; Amber always represents Binance.

**The 10% Accent Rule.** Accent hues occupy less than 10% of any viewport. The vast majority of the interface is neutral to prevent visual fatigue.

**Colour Marks State, Ink Carries Structure.** An accent may only appear where it is doing semantic work. This is the rule that keeps the palette from collapsing into decoration — when one hue carries five jobs, it carries none.

| Accent earns its place on | Ink owns |
|---|---|
| Signed values (PnL, funding, net APR) | Labels, captions, column heads |
| State: active tab, active filter, live/healthy badges | Neutral statistics (counts, win rate, version) |
| Primary action buttons | Row-level and secondary actions |
| Exchange identity (Amber = Binance, Cyan = Hyperliquid) | Navigational affordances ("View all →") |
| KPI hero figures and their icon tiles, as a domain key | Decorative section icons, empty states, spinners |
| Risk thresholds crossing into warning | Modal titles, form labels, checkboxes |

A table column where every row is positive is not signed — colouring all 200 cells emerald produces noise, not signal. Colour the sign, not the column.

**Tokens Are Named, Never Inlined.** Markup uses the Tailwind class — `text-ink-mute` — never `text-[var(--text-muted)]`. The arbitrary-value form defeats Tailwind's opacity modifiers, is invisible to design tooling, and makes a theme audit unreadable. Register tokens in `tailwind.config`; consume them by name.

## Typography

**UI Font:** IBM Plex Sans (Latin) paired with Noto Sans SC (Simplified Chinese)
**Data & Numeric Font:** JetBrains Mono (with `tnum` tabular numbers and slashed zero)

**Character:** A tight, technical neo-grotesque for chrome, paired with a high-precision monospace for financial figures. Plex replaces Inter and Geist, both of which sit on the short list of faces every generated UI converges on. Plex is IBM's own grotesque: technical and authoritative enough for an execution terminal, and uncommon enough to still read as a choice. JetBrains Mono is retained deliberately — its slashed zero and unambiguous `1/l/I` matter more in a price table than matching the sans serif's width.

### Hierarchy

Seven steps. Nothing between them; pick the nearest step rather than inventing a size.

| Step | Class | Size | Weight / leading | Used for |
|---|---|---|---|---|
| Display | `text-2xl` | 24px | 700 / 1.15 | KPI card totals, modal titles |
| Headline | `text-lg` | 18px | 600 / 1.35 | Panel headlines |
| Lede | `text-md` | 16px | 600 / 1.4 | Page headings |
| Title | `text-base` | 14px | 600 / 1.45 | Sub-section headers, pair names |
| Body | `text-sm` | 13px | 400 / 1.5 | Table content, descriptive copy |
| Caption | `text-xs` | 12px | 400 / 1.4 | Default table body text, column heads |
| Label | `text-2xs` | 11px | 600 / 1.45, tracking 0.04em | Chips, badges, micro-labels |

**Numeric** (`JetBrains Mono`, `font-variant-numeric: tabular-nums`) inherits the step of whatever it sits in — usually Caption, Body, or Display.

**11px is the floor.** Below it, dense labels stop being legible on a phone held at arm's length. If a layout needs something smaller, it needs less content instead.

**11px is also capped by length.** The bottom step is for short strings only — chips, badges, and micro-labels of roughly 20 characters or fewer. Anything longer (a column head like *Net Flow (PnL / Carry)*, a readout like *Rebalance Threshold: 40.0%*) moves up to Caption even when it sits in the furniture layer, because sustained reading at 11px is a different task from glancing at a badge.

### Named Rules
**The Tabular Number Invariant.** Every number that updates via WebSocket or represents financial value MUST use `.font-num` (`JetBrains Mono` with `tabular-nums`). Numbers must never cause horizontal layout shifts when changing values.

**The Ramp Is Closed.** Font sizes come from the seven steps above, never from `text-[…]` literals. A literal size is a decision made outside the design system.

## Layout

- **Container Model**: Max-width `80rem` (`1280px`) centered layout on desktop with `16px`-`20px` horizontal gutter padding.
- **Grid Structure**: 4-column KPI ribbon on desktop (`grid-cols-4`), collapsing gracefully to 2 columns on tablet (`grid-cols-2`) and 2 columns on mobile.
- **Density**: High information density with strict 8px/12px/16px spatial rhythm (`p-3` / `p-4`).
- **Responsive Navigation**: Desktop uses top segmented tabs (`nav-btn`); mobile uses fixed bottom thumb-accessible navigation bar with safe-area padding.

## Elevation & Depth

Surfaces rely on subtle translucent layering and fine 1px border dividers rather than heavy drop-shadows.

### Shadow Vocabulary
- **Card Subtle**: Default elevation for surface cards.
- **Card Hover**: Slightly deeper cast on `:hover`, paired with the border moving from `edge` to `edge-strong`.

Both carry a `inset 0 1px 0 var(--edge-highlight)` hairline along the top edge. That hairline — not the shadow — is what makes a dark surface read as a physical plane; without it, `bg-surface` and `bg-canvas` blur together. In Studio the highlight is `transparent`, where ambient light does the work.

### Named Rules
**The Single Elevation Rule.** Depth is established by background luminance contrast (`--bg-canvas` < `--bg-surface` < `--bg-elevated`), the top-edge hairline, and 1px borders — not stacked shadows.

**No Bounce.** Motion uses `--ease-out` (`cubic-bezier(.16,1,.3,1)`) or `--ease-in-out`. Overshoot and elastic curves are banned: money moving on screen should decelerate, never spring.

## Shapes

- **Surface Containers**: `12px` (`0.75rem` / `rounded-xl`) for main cards and matrix tables.
- **Interactive Controls**: `8px` (`0.5rem` / `rounded-lg`) for buttons, inputs, and search bars.
- **Status Chips & Pills**: `6px` (`rounded-md`) or full pill (`rounded-full`) for compact tags.
- **Dividers & Strokes**: Crisp `1px solid var(--border-subtle)` via `border-edge`.

## Components

### Buttons
- **Shape**: `rounded-lg` (8px), padding `6px 12px` for normal actions, `4px 8px` for table inline actions.
- **Primary / Action**: `bg-emerald-500 hover:bg-emerald-400 text-emerald-950`, subtle `active:scale-95`. Reserved for committing an action — Save & Apply, Execute, Confirm.
  White-on-emerald measures **2.54:1**, which fails WCAG AA at the 13px these labels use. Dark ink on emerald measures **5.97:1** (7.88:1 on hover) and both ends sit inside the documented Carry Emerald tonal ramp.
- **Danger / Unwind**: Rose tint background (`bg-rose-500/10 hover:bg-rose-500/20 text-rose-600 dark:text-rose-400 border border-rose-500/25`).
- **Secondary / Ghost**: Elevated background (`bg-elevated hover:bg-hover text-ink`).
- **Focus Ring**: `2px solid var(--border-focus)` at `2px` offset on `:focus-visible`; inputs add a `3px` emerald halo.

### Segmented Controls & Tabs
- **Selected**: emerald *tint*, not fill — `bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/25`.
- **Unselected**: `text-ink-mute hover:text-ink` with `border border-transparent`.

The transparent border on the unselected state is load-bearing: without it, selecting a tab shifts the row by 1px. Six solid emerald pills would also blow past the 10% accent budget, so selection is carried by tint plus a transparent-to-visible border.

### Chips & Badges
- **Style**: Compact height (~20px), `text-2xs` (11px), semi-bold weight, subtle border.
- **Variants**: `.chip-emerald` (Live/Profit), `.chip-cyan` (Hyperliquid/Funding), `.chip-amber` (Binance/Rebalance), `.chip-rose` (Risk/Loss), `.chip-neutral` (Default/Info).

### Toasts
- **Surface**: `bg-inverse text-inverse` with `border-edge-inverse`, so it floats in both themes.
- **Accent**: the icon and border hairline only — `border-rose-500/40` for errors, `border-emerald-500/40` for success. Never a tinted fill.

### Cards & Tables
- **Card**: Surface background, 1px subtle border, padding 16px (`p-4`), rounded 12px.
- **Table**: Dense rows (`py-2.5 px-3`), crisp headers on an elevated background, smooth hover transition (`120ms`).

### Inputs & Selects
- **Style**: Surface background, 1px border, 8px radius, padding 8px 12px, font-num for numeric inputs.
- **Focus**: Emerald border with a 3px translucent emerald halo.

## Do's and Don'ts

### Do:
- **Do** format all financial figures with fixed decimal places and tabular numbers.
- **Do** ensure all interactive elements have visible `:focus-visible` rings for keyboard navigation.
- **Do** display clear loading skeletons during data fetching and reconnection phases.
- **Do** keep button click and tab transition animations between 120ms and 180ms.
- **Do** test all screens in both Dark (Obsidian) and Light (Studio) modes.
- **Do** honour `prefers-reduced-motion`; the token layer already collapses durations.

### Don't:
- **Don't** use multi-color gradient text or decorative zero-blur heavy block shadows.
- **Don't** allow numbers or cards to jump or shift when live WebSocket ticks arrive.
- **Don't** use emojis instead of consistent vector icons (`Lucide`).
- **Don't** hide critical risk states, liquidation distances, or margin warnings behind obscure menus.
- **Don't** introduce slow, multi-stage choreographed page load animations.
- **Don't** reach for a thick coloured left border to flag a card — it is the single most recognisable tell of a generated UI. Use a tonal wash (`.hero-card`) or a chip instead.
- **Don't** write `text-[…]` or `bg-[…]` literals anywhere in markup; every value has a token.
- **Don't** tint a decorative icon with an accent. Monochrome icons at `text-ink-soft` are what make an accent land when it does appear.
- **Don't** colour a table column whose every value shares a sign — that is a statistic, not a signal.

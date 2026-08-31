# Detector waivers

Every entry here was verified against the rendered DOM, not assumed.
Re-check before renewing — a waiver that outlives its reason is just a blind spot.

---

## `ai-color-palette` — waived 2026-08-31

**Config:** `.impeccable/config.json` → `detector.ignoreRules: ["ai-color-palette"]`

### What the rule actually tests

`scripts/detector/rules/checks.mjs` → `checkElementAIPaletteDOM()`:

```
textColor = getComputedStyle(el).color
if (hasChroma(textColor, 80) && hue ∈ [160,200] ∪ [260,310]
    && luminance(first opaque ancestor background) < 0.1)
  → "Cyan neon text on dark background"
```

It fires on **every element with a computed text colour**, including `<svg>`,
`<path>`, `<rect>` and `<circle>` nodes that carry no text at all.

### Why it fires here

- Emerald `#10b981` computes to **hue 160** — exactly the *inclusive lower bound*
  of the rule's cyan band. It is labelled "Cyan" by the rule's ternary
  (`hue >= 260 ? 'Purple/violet' : 'Cyan'`), which is simply wrong for emerald.
- Cyan `#06b6d4` / `#22d3ee` compute to **hue 189 / 188** — inside the band, and
  genuinely cyan.

### What the 37 remaining hits are

Reproduced the predicate exactly against the live DOM (`probe_palette.ts`) —
37 hits, matching the detector:

| # | What | Text? |
|---|------|-------|
| 18 | Lucide `<svg>` / `<path>` / `<rect>` / `<circle>` nodes inheriting `currentColor` | none |
| 3 | `>50%` liquidation-buffer / distance health values | state |
| 3 | KPI icon tiles (`bg-emerald-500/10`, `bg-cyan-500/10`) + logo tile | domain key |
| 4 | Funding readouts: countdown, gross, hourly run-rate, hero figure | domain |
| 1 | Realized-PnL hero figure | domain |
| 2 | `HEALTHY` / `Balanced` status badges | state |
| 2 | `Hyperliquid L1 (USDC)` label + identity dot | domain |
| 2 | Active mobile nav tab (hidden at desktop) | state |
| 2 | Other | — |

### Why waiving is correct

1. Both flagged hues are the **documented brand accents** in
   `.impeccable/design.json`: `accent-carry` `#10b981` (role `primary`) and
   `accent-funding` `#06b6d4` (Hyperliquid Cyan, role `secondary`). The rule
   cannot be satisfied without abandoning the brand.
2. Every hit passes WCAG comfortably — emerald on `--bg-surface` is **7.39:1**,
   cyan-400 on `--bg-canvas` is **10.89:1**. Both exceed AAA (7:1).
3. The rule targets the landing-page tell it was tuned on — vivid cyan/purple
   neon body copy on near-black. A two-hue *exchange-identity* system
   (Binance = amber, Hyperliquid = cyan) on a professional trading terminal is a
   pre-AI convention, not generated slop.
4. Rule-level waiver is the only available granularity: `detector.ignoreValues`
   matches on a value extracted from the finding snippet, and this snippet
   (`"Cyan neon text on dark background"`) carries no colour to match.

**Cost of the waiver:** a genuine purple/violet gradient would no longer be
caught. Guarded by DESIGN.md → Don'ts ("no decorative rainbow gradients") and by
`accent-*` being the only chromatic tokens registered in `tailwind.config`.

---

## `low-contrast` (2 hits) — **NOT waived**, accepted as a measurement artifact

Deliberately left standing. Do not silence an accessibility rule.

### The two hits

```
pixel contrast 2.1:1 median 10.1:1 (need 4.5:1) on backdrop filter "01:06:34 UTC+8"
pixel contrast 1.2:1 median  4.7:1 (need 4.5:1) on backdrop filter "Settlement in 53m 25s"
```

### What is measured

`scripts/detector/engines/visual/screenshot-contrast.mjs`:

1. Screenshot the text's clip rect.
2. Re-screenshot with `color: transparent !important` applied → the true backdrop.
3. Pixels whose channel-sum changed by ≥ 10 are "glyph pixels".
4. For each: `ratio(beforePixel, afterPixel)` — the **rendered, antialiased**
   pixel against the backdrop.
5. Report `p10Ratio` (10th percentile) and `medianRatio`.

For a 13px / weight-500 glyph the stroke is ~1px wide, so a large share of
"glyph pixels" are **partially covered edge pixels**. A 50%-covered emerald
pixel scores ~3.4:1 even when the specified colour scores 10.9:1. `p10`
therefore measures sub-pixel glyph coverage, not contrast.

### The specified contrast (WCAG, what actually governs legibility)

| Element | Colour | Background | Ratio |
|---|---|---|---|
| `#clock-utc` | `#f7f8f9` | `#0a0b0e` | **18.5:1** |
| `#funding-countdown` | `#22d3ee` | `#0a0b0e` | **10.9:1** |

Both pass AAA (7:1). The header is `rgba(10,11,14,0.94)` over the same canvas
colour, so translucency adds at most ~`rgb(30,30,30)` — still ~8:1.

### Mitigation already applied

Header readouts promoted 11px → 13px and weight 400/500 → 600/500, which thickens
glyph strokes and raises the p10 floor. Remaining delta is inherent to the
statistic, not to the design.

**Trigger:** any `backdrop-filter` ancestor opts an element into this screenshot
path. Removing `backdrop-blur-md` from the sticky header would clear the finding,
but that trades a real design choice for a statistic — not worth it.

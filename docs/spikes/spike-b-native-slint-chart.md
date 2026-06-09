# Spike B — native-Slint draggable judgment line + <100 ms zone recolor

**Story:** 1.5 · **GitHub issue:** #4 · **Date:** 2026-06-09 · **Type:** throwaway spike (go/no-go).
**Question:** can we draw the SSG chart and drag a judgment line with the Buy/Neutral/Sell zone
recolouring within **~100 ms (click-to-pixel, incl. recompute)** — **natively in Slint**, no egui/web?

## What was built

`app/examples/spike_b_chart.rs` (throwaway, inline `slint::slint!` markup; run with `just spike` or
`cargo run -p steadyinvest-app --example spike_b_chart`):
- A **semi-log chart** (1→200 log axis) drawing synthetic Sales/EPS/Price series as native Slint
  `Path`s (commands computed in Rust via `log10`, viewbox = chart px).
- A **draggable white judgment line** (`TouchArea.moved` → maps pointer-y to an estimated future EPS
  via the inverse-log mapping).
- A **§4 zone bar** (Buy/Neutral/Sell thirds, Okabe-Ito hues) that **recolours live** as the line
  moves; a present-price marker; a one-line signal readout (forecast high/low, zone, U/D ratio).
- The signal recompute is done in **exact `rust_decimal`** (mirrors the method-spec thirds zoning);
  only pixel mapping uses floats.
- Each drag logs `[spike-b] recompute+property-set: <N> µs` to **stderr**.

## How to measure (Guy, on a display)

1. `just spike` (Linux needs `libfontconfig1-dev`).
2. Drag the white line up/down continuously; watch the zone bar recolour under the cursor.
3. Read the stderr `µs` lines — that's the **recompute+property-set** cost (the Decimal math + Slint
   property writes). Slint then repaints in its dirty-driven retained mode.
4. Judge the **perceived** click-to-pixel latency: does the recolor track your hand with no lag?

## Results — RUN 2026-06-09 (Guy, on display)

| Metric | Value |
|--------|-------|
| Builds + clippy `-D warnings` clean | ✅ (CI, Linux) |
| Recompute+property-set latency (stderr µs, **660 drag events**) | **~40–60 µs typical · 235 µs max** (≈ 0.04–0.24 ms) |
| Perceived click-to-pixel < ~100 ms while dragging | ✅ **Yes — "suit mon geste instantanément, aucune perception de délai"** (Guy) |
| Rendering correct (axis/lines/zone recolour live) | ✅ Yes |

The recompute (exact `rust_decimal` signal + Slint property writes) costs **microseconds** — ~400×–2500×
under the 100 ms budget — and Slint's dirty-driven retained-mode repaint keeps the recolor visually
instant under the cursor. The native `Path` + `TouchArea` approach is comfortably fast.

## Decision — **GO** (2026-06-09)

- [x] **GO** — native Slint meets <100 ms (by a wide margin); the **"Slint-only, no egui/web"
  decision is LOCKED**. Story 2.8 builds the real growth chart + zone bar natively (`Path` +
  `TouchArea`, `log10` in Rust), reusing this spike's approach. GitHub issue #4 resolved.
- [ ] ~~NO-GO fallback~~ — not needed.

The throwaway example `app/examples/spike_b_chart.rs` has served its purpose (the decision above). It
is kept for now as a working reference for Story 2.8 and may be deleted when the production chart lands.

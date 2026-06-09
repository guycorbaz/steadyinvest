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

## Results

| Metric | Value |
|--------|-------|
| Builds + clippy `-D warnings` clean | ✅ (verified in CI, Linux) |
| Recompute+property-set latency (stderr µs) | _to fill — Guy's run_ |
| Perceived click-to-pixel < ~100 ms while dragging | _to fill — Guy's run_ |
| Rendering correct (axes/lines/zone make sense) | _to fill — Guy's run_ |

> **Status: PENDING Guy's on-display run.** The agent built it headless: it compiles, is clippy-clean,
> and the event loop starts; the **perceptual <100 ms verdict requires a display** (same as the
> Story-1.1 window check). The Decimal recompute itself is expected to be low-µs (well under 100 ms);
> the open question the spike answers is whether Slint's native `Path` redraw + drag feel live.

## Decision

- [ ] **GO** — native Slint meets <100 ms; lock the "Slint-only, no egui/web" decision; Story 2.8
  builds the real chart natively.
- [ ] **NO-GO** — fall back (record which, and why):
  - [ ] dedicated Slint canvas/window, or
  - [ ] `plotters` → `SharedPixelBuffer` static backdrop + Slint `TouchArea` overlay (drag stays Slint).
  - **NOT egui, NOT web** (architecture-locked).

_Fill the table + tick a box after running; then this spike is complete and `app/examples/spike_b_chart.rs` can be deleted (its job is the decision above)._

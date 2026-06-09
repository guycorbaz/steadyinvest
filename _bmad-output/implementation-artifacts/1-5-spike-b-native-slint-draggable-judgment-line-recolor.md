# Story 1.5: Spike B — native-Slint draggable judgment line, <100 ms recolor (hardened go/no-go)

Status: done

<!-- Note: THROWAWAY SPIKE. Deliverable = a GO/NO-GO decision + findings note, NOT production code. -->
<!-- Epic 1. Done out of order (before 1.4) to de-risk the project's #1 technical unknown early. -->

## Story

As the developer (Guy, solo),
I want to prove the signature interaction is feasible **natively in Slint**,
so that the "Slint-only, no egui/web" decision is locked (or the fallback is chosen) **before** any UI investment in Epic 2.

## Acceptance Criteria

1. **A throwaway Slint example renders a real semi-log SSG-style chart.** A 1→200 **log axis** with a Sales/EPS/Price series (synthetic golden-ish data is fine for the spike), drawn **natively in Slint** (`Path` with commands computed in Rust using `log10`), plus a **zone bar** (Buy/Neutral/Sell bands).
2. **A judgment line is draggable.** The user can drag a judgment trend line (or the forecast point) via a Slint `TouchArea`; the drag updates an estimated-future value.
3. **Live recompute + recolor within ~100 ms.** On drag, the forecast and the Buy/Neutral/Sell zone band **recompute and recolour**, measured **click-to-pixel including the recompute**, on the target hardware. The recompute cost is instrumented and logged.
4. **Explicit GO / NO-GO conclusion.** The spike concludes with a written **GO/NO-GO** note (suggested: `docs/spikes/spike-b-native-slint-chart.md`) recording the measured latency and the decision. **NO-GO triggers the architecture fallback** (dedicated Slint canvas, or `plotters`→`SharedPixelBuffer` static backdrop + `TouchArea` overlay) — **NOT egui, NOT web** — before Epic 2 begins.
5. **Throwaway & isolated.** The spike lives outside the shipping app surface (an `examples/` binary), is runnable via `just spike` / `cargo run -p steadyinvest-app --example spike_b_chart`, and does **not** become production code (Epic 2 Story 2.8 builds the real chart). It must still pass the repo gates (fmt, clippy `-D warnings`) so CI stays green.

## Tasks / Subtasks

- [x] **Task 1 — Throwaway native-Slint chart example (AC: 1, 5)**
  - [x] Add `app/examples/spike_b_chart.rs` using the inline `slint::slint!{ … }` macro (no `build.rs` change — keeps it isolated and deletable).
  - [x] In Rust: a small synthetic series (≈10 years Sales/EPS/Price), a `log10`-based mapping from value→y on a 1→200 axis, and a helper that builds the Slint `Path` `commands` string (`M x y L x y …`) for each series.
  - [x] Render the three series + a zone bar (3 stacked `Rectangle`s, Buy/Neutral/Sell) using the Okabe-Ito hues (Buy #009E73 / Hold #E69F00 / Sell #D55E00) for visual realism.
- [x] **Task 2 — Draggable judgment line + live recompute (AC: 2, 3)**
  - [x] Add a draggable judgment line (a `TouchArea` over the chart; `moved` callback maps pointer-y → an estimated future value).
  - [x] On each drag delta, recompute in **Rust** (exact: reuse `rust_decimal` for the forecast/zone math, mirroring the method-spec thirds zoning) and push updated properties (zone-band heights/colours, a present-price marker, a numeric signal) back to the Slint model.
  - [x] **Instrument latency:** wrap the recompute + property-set in `std::time::Instant::now()…elapsed()` and `eprintln!` the microseconds each drag; also note that perceived click-to-pixel (incl. Slint's redraw) is judged visually.
- [x] **Task 3 — Run wiring (AC: 5)**
  - [x] Update the `justfile` `spike` task to `cargo run -p steadyinvest-app --example spike_b_chart`.
  - [x] Confirm `cargo build -p steadyinvest-app --example spike_b_chart` compiles and `cargo clippy --all-targets -- -D warnings` stays clean.
- [x] **Task 4 — GO/NO-GO findings note (AC: 4)**
  - [x] Create `docs/spikes/spike-b-native-slint-chart.md`: what was built, **measured recompute latency** (from the logs), the **perceptual verdict** (Guy runs it on a display), the **GO/NO-GO decision**, and — if NO-GO — which fallback (dedicated Slint canvas vs `plotters`→`SharedPixelBuffer` + `TouchArea`). Leave the perceptual verdict + final decision for Guy to fill after running on real hardware.

## Dev Notes

### This is a SPIKE — what "done" means
The **deliverable is the GO/NO-GO decision**, not shippable code. The example is throwaway (it is deleted/ignored once Epic 2's real chart, Story 2.8, exists). Optimise for *answering the question* (is native-Slint drag-recolor < 100 ms feasible?), not for code polish — but keep it gate-clean so CI passes. [Source: epics.md Epic 1 "(Spikes are throwaway: their deliverable is a go/no-go decision + a short findings note, not production code)"; architecture.md "Week-1 spikes"]

### The locked technical approach (and the fallback)
- **Charts are native Slint:** `Path` + `TouchArea`, **`log10` computed in Rust**; recolor is "trivial in Slint's dirty-driven retained mode" per the architecture. NO egui, NO web. [Source: architecture.md#Core Technical Decisions, #Frontend Architecture]
- **If NO-GO**, the fallback (in order): a **dedicated Slint canvas/window**, or **`plotters`→`SharedPixelBuffer`** static backdrop with a Slint **`TouchArea` overlay** (the drag stays Slint). Record which, and why. **Never egui, never web.** [Source: architecture.md#Core Technical Decisions; epics.md Story 1.5 AC]
- **Headless caveat:** this environment has no display, so the **perceptual <100 ms verdict must be made by Guy** on his desktop (as with the Story-1.1 window). The agent's job: make it compile, instrument the recompute time, and write the findings template. The recompute-microseconds log is objective; the click-to-pixel feel is Guy's call.

### UX target the spike emulates (from the UX spec, Story 2.8)
- §1 semi-log growth chart (Sales/EPS/Price; historical solid / projected dashed), 1→200 log axis, 5–30% guide fan; the **zoning is a separate §4 zone bar** (Buy/Neutral/Sell thirds) — the chart itself has NO zones. The judgment line drag updates the forecast → the §4 zone bar recolours. [Source: ux-design-specification.md §1/§4; epics.md UX-DR10/UX-DR11]
- Zone hues: Buy `#009E73`, Hold/Neutral `#E69F00`, Sell `#D55E00` (Okabe-Ito, colour-blind-safe). [Source: ux UX-DR2]
- Performance bar: **NFR-P1 = judgment recalc + zone re-render feel live within ~100 ms while dragging.** That is the spike's pass/fail threshold. [Source: prd.md#NFR-P1]

### Exact-decimal note
Even in the spike, do the forecast/zone **math in `rust_decimal`** (not f64) to mirror the real engine and confirm decimal math is fast enough in the drag loop. Pixel coordinates / `log10` mapping for *rendering* may use `f32`/`f64` (rendering is not the decision chain) — but the *signal* recompute uses Decimal. [Source: architecture.md Cardinal Rule; core/src/rounding.rs, core/src/method]

### Previous story intelligence
- **MSRV 1.96**, CI **Linux-only**, gates run `--locked`; clippy `-D warnings` covers `--all-targets` (so the example is linted — keep it clean). [1-1/1-2/1-3 dev records]
- `core` has `method` constants (zone thirds, `ZONE_COUNT=3`) and `rounding` (half-up display) usable by the spike's recompute; `contract` has `Money`. Reuse rather than reinvent if convenient (but the spike may also stay self-contained — it's throwaway).
- Slint 1.16 is the pinned UI dep on `app`; the `slint!` inline macro avoids touching `app/build.rs` (which compiles `ui/app.slint`). Examples inherit `app`'s deps.
- Linux Slint runtime needs `libfontconfig1-dev` (already in CI); a display is needed only to *see/measure* it (Guy's machine).

### Project Structure Notes
- New (throwaway): `app/examples/spike_b_chart.rs`; new doc `docs/spikes/spike-b-native-slint-chart.md`; modify `justfile` (`spike` task).
- Do **not** modify `app/ui/app.slint`, `app/src/main.rs`, or any crate's production code. No new workspace dependencies (use `slint`, optionally `rust_decimal` already available to `app` via `core`/`contract`).
- If the spike needs `rust_decimal` directly in `app`, it is already transitively available; add it to `app`'s `[dev-dependencies]` (examples can use dev-deps) rather than runtime deps to keep the shipping binary lean.

### References
- [Source: epics.md#Story 1.5: Spike B] — user story + AC + GO/NO-GO + fallback
- [Source: architecture.md#Core Technical Decisions / #Frontend Architecture] — native Slint Path/TouchArea, log10 in Rust, fallback (NOT egui/web)
- [Source: prd.md#NFR-P1] — <~100 ms judgment recalc/recolor (the pass threshold)
- [Source: ux-design-specification.md §1 growth chart / §4 zone bar; UX-DR10/UX-DR11/UX-DR2] — what the interaction looks like
- [Source: GitHub issue #4] — the charting-spike risk this story closes

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — claude-opus-4-8 — via Claude Code dev-story (2026-06-09).

### Debug Log References

- `cargo build -p steadyinvest-app --example spike_b_chart` → compiles first try.
- Gates green: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings` (the example is linted), `cargo test --all --locked`, `cargo deny check`.
- Headless run attempted here (no display) — event loop starts; **perceptual <100 ms measurement requires a display (Guy)**.

### Completion Notes List

- Built the throwaway native-Slint spike `app/examples/spike_b_chart.rs` (inline `slint::slint!` markup — no `build.rs` change, fully isolated/deletable): semi-log 1→200 chart with Sales/EPS/Price as native `Path`s (`log10` mapping in Rust), a draggable white judgment line (`TouchArea.moved`), a §4 Buy/Neutral/Sell zone bar (Okabe-Ito) that recolours live, present-price marker, and a signal readout.
- **Signal recompute in exact `rust_decimal`** (forecast high/low, thirds zoning, U/D) — mirrors the method spec; only pixel/`log10` mapping uses floats (rendering ≠ decision chain). Each drag `eprintln!`s the recompute+property-set latency in µs.
- `justfile` `spike` task wired to run it; `rust_decimal` added as an `app` **dev-dependency** (example-only — shipping binary stays lean).
- **No production code touched** (`app/ui/app.slint`, `app/src/main.rs`, crates untouched). Gates green; CI stays green.
- ✅ **VERDICT: GO (2026-06-09, Guy's on-display run).** Across **660 drag events** the recompute+property-set latency was **~40–60 µs typical, 235 µs max** (≈0.04–0.24 ms — far under the 100 ms budget), and Guy confirmed the recolor **"suit mon geste instantanément, aucune perception de délai."** The **"Slint-only, no egui/web" decision is LOCKED**; Story 2.8 builds the production chart natively (`Path`+`TouchArea`, log10 in Rust). GitHub issue #4 resolved. Findings recorded in `docs/spikes/spike-b-native-slint-chart.md`.

### File List

**Added (throwaway / docs):**
- `app/examples/spike_b_chart.rs` (throwaway native-Slint chart spike)
- `docs/spikes/spike-b-native-slint-chart.md` (GO/NO-GO findings note — verdict pending Guy's run)

**Modified:**
- `app/Cargo.toml` (added `rust_decimal` dev-dependency for the example)
- `justfile` (`spike` task → run the example)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (1-5 → in-progress → review)

## Change Log

| Date | Change |
|------|--------|
| 2026-06-09 | Story 1.5 created (ready-for-dev): throwaway native-Slint chart spike (draggable judgment line + <100 ms zone recolor) → GO/NO-GO. Done out of order (before 1.4) to de-risk the principal technical unknown (issue #4). |
| 2026-06-09 | Story 1.5 implemented: throwaway `app/examples/spike_b_chart.rs` (native Slint Path/TouchArea, log10, exact-decimal signal recompute, live zone recolor, µs latency logging) + `just spike` + findings doc. Builds, clippy-clean, gates green. **GO/NO-GO verdict pending Guy's on-display run.** Status → review. |

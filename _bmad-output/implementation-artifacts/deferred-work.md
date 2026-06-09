# Deferred Work

Items surfaced during reviews that are real but not actionable in the originating story.

## Deferred from: code review of 1-1-workspace-scaffold-ci-gate-skeleton (2026-06-09)

- **macOS/Windows runners get no explicit Slint system-dependency step** (.github/workflows/ci.yml) — only Linux installs `libfontconfig1-dev`; the macOS/Windows `app` build relies on undocumented runner-image contents. Add explicit provisioning if/when those runners break. Non-blocking today.
- **`round_dp` uses banker's rounding (MidpointNearestEven), not NAIC half-up** (core/src/lib.rs) — the project's named rounding mode must be defined in the Story 1.2 method spec and applied consistently in `core`; revisit the probe/engine rounding then.
- **No overflow / `Result` handling in core decimal math** (core/src/lib.rs) — the scaffold probe can't overflow, but the real SSG compound-growth math (Story 1.8) over large share counts/years must use checked paths so the pure `core` never panics.
- **`concurrency.cancel-in-progress` keyed on `github.ref` can cancel in-flight `main` CI** (.github/workflows/ci.yml) — a green check on `main` could reflect a canceled (untested) commit; consider not cancelling on the default branch.
- **Cross-OS determinism assertion silently degrades to Linux-only** (.github/workflows/ci.yml) — if a non-Linux build breaks earlier in the `quality` job, the determinism step never runs on that OS with no signal that the cross-OS check was skipped. *(Partly mooted 2026-06-09 — CI is Linux-only for now.)*

## Deferred from: code review of 1-2-ssg-method-specification (2026-06-09)

- **Engine handling of degenerate inputs** (core, Story 1.8) — the method spec now *defines* the rules (U/D denominator ≤ 0, CAGR start ≤ 0 / sign-cross, TTM EPS ≤ 0, forecast-low option (d) needs dividend > 0, PTP gross-up needs tax_rate < 1); the actual computation/guards (Result/unknown propagation, no panics) land with the engine in Story 1.8.
- **`split_series_break` precision** (docs/method + Story 1.8) — the "inconsistent with sales" divergence threshold is unquantified and the down-split factor 0.67 is an inexact reciprocal of 1.5 (true 2/3 = 0.6667, not exactly representable in Decimal). Quantify/refine when the plausibility engine is implemented.
- **`EXPECTED` fingerprint regeneration can be rubber-stamped** (core/src/method/mod.rs) — inherent to snapshot gates; the gate forces attention on any method change but does not, by itself, force the coupled METHOD_VERSION bump. Acceptable for v1; revisit if a stronger version-coupling is wanted.

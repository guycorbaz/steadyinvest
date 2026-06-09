# Deferred Work

Items surfaced during reviews that are real but not actionable in the originating story.

## Deferred from: code review of 1-1-workspace-scaffold-ci-gate-skeleton (2026-06-09)

- **macOS/Windows runners get no explicit Slint system-dependency step** (.github/workflows/ci.yml) — only Linux installs `libfontconfig1-dev`; the macOS/Windows `app` build relies on undocumented runner-image contents. Add explicit provisioning if/when those runners break. Non-blocking today.
- **`round_dp` uses banker's rounding (MidpointNearestEven), not NAIC half-up** (core/src/lib.rs) — the project's named rounding mode must be defined in the Story 1.2 method spec and applied consistently in `core`; revisit the probe/engine rounding then.
- **No overflow / `Result` handling in core decimal math** (core/src/lib.rs) — the scaffold probe can't overflow, but the real SSG compound-growth math (Story 1.8) over large share counts/years must use checked paths so the pure `core` never panics.
- **`concurrency.cancel-in-progress` keyed on `github.ref` can cancel in-flight `main` CI** (.github/workflows/ci.yml) — a green check on `main` could reflect a canceled (untested) commit; consider not cancelling on the default branch.
- **Cross-OS determinism assertion silently degrades to Linux-only** (.github/workflows/ci.yml) — if a non-Linux build breaks earlier in the `quality` job, the determinism step never runs on that OS with no signal that the cross-OS check was skipped.

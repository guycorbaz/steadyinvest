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

## Deferred from: code review of 1-3-contract-v1 (2026-06-09)

- **Unknown enum-value tolerance across schema versions** (contract) — by design, adding an enum variant (`Source`/`Review`/`Coverage`/`ForecastLowOption`) is a `schema_version` bump; an older build failing to deserialize a newer file's unknown enum value is the intended fail-loud behavior (domain correctness > silent fallback). No `#[non_exhaustive]`/`#[serde(other)]`. Revisit only if cross-version graceful enum degradation becomes a requirement.
- **Runtime validation of free-string contract fields** (contract or producers) — `Timestamp` (RFC3339 UTC), `native_currency` (ISO-4217), `hash_of_dependencies` (hex) are stored as unvalidated `String`s in v1. Add validating constructors / `TryFrom` when the producing layers land (app clock; ingestion in Epic 3).
- **Required-field forward-evolution** (contract) — current required (non-`serde(default)`) fields are intended mandatory; if any becomes optional later, add `#[serde(default)]` + a migration at that point.

## Deferred from: code review of 1-4-spike-a (2026-06-10)

_Throwaway spike (GO). These apply to the **Epic 2 production grid** (Stories 2.3/2.4), not the spike itself._

- **Typed-edit path is not `Decimal`-validated** (app grid, Epic 2) — in the spike, typing `1.2.3` / `--` / `,` produces a `filled=true` cell that is not a valid number; only the *paste* path is validated. The production entry grid must validate on commit so the "missing ≠ 0 / never a bad number" guarantee holds for manual typing too. [spike ref: app/examples/spike_a_grid.rs:434]
- **Locale / thousands-separator parsing** (app grid, Story 2.4) — `1 234.50`, `1,234.50`, `$1234`, and the CH/EU decimal comma `1,5` are all rejected by the canonical `from_str_exact`. A realistically-formatted spreadsheet column would leave many cells empty. Production needs locale-aware parsing; the spike's GO test used raw unformatted numbers. [spike ref: app/examples/spike_a_grid.rs:391]
- **Spreadsheet-grade navigation** (app grid, Epic 2) — Tab/Enter clamp at edges with no row-wrap, no Shift+Tab, and no explicit "commit" (live editing makes it vacuous in the spike). Define wrap/commit/backward-tab semantics for the real grid.
- **i18n entry: multi-codepoint / IME / non-ASCII digits** (app grid, Epic 2) — `type_char` ignores anything that isn't a single ASCII digit/`.`/`,`/`-`; IME-composed or localized-digit input is dropped. Handle in production entry. [spike ref: app/examples/spike_a_grid.rs:436]
- **Silent overflow at grid bottom** (app grid, Epic 2) — paste clips when it runs past the fixed grid height with only a (currently inconsistent) status count; the production virtualized model must grow or surface overflow. Also note: the spike's full-model `refresh` does **not** exercise virtualization, so the production `TableModel` + virtualized `ListView` nav/paste behavior remains unproven by this spike.

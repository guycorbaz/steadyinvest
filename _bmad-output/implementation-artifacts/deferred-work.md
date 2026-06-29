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

## Deferred from: code review of story-2.8 (2026-06-14)

> Filed as GitHub issues **#25–#31** (the canonical tracking source): #25 axis 1→200 vs sub-$1/>$200 EPS · #26 orphan grip (all-None EPS) · #27 single-point series renders nothing · #28 slider a11y keyboard step · #29 grip/endpoint offset + dead `judgment_x` · #30 mouse-y fixed-height coupling · #31 derived→direct forecast conversion.

- **Drag converts a growth-%-derived forecast into a direct `est_high_eps`** — when the forecast EPS is derived from `projected_eps_growth_pct`, the line is drawn but a drag writes the direct `est_high_eps` field, silently shadowing the growth-% link. Spec Q2 chose `est_high_eps` as the handle, so this is by design; revisit if a smarter mapping is wanted. [app/src/viewmodel/chart.rs, app/src/main.rs]
- **1→200 axis can't represent sub-$1 or >$200 EPS** — a stock with est-high-EPS < 1.00 or > 200 can't be set by drag (the line pins to an edge) and a drag/commit can snap a typed sub-dollar forecast to 1.00. Strengthens the per-series axis-scaling refinement (deferred item #1). [app/src/viewmodel/chart.rs value_for_y]
- **Drag strip a11y** — `accessible-role: slider` with no keyboard step handler; the keyboard path is the exact-value JudgmentField (NFR-U2 satisfied), but AT announces an adjustable slider that arrows can't move. Add arrow-key stepping or adjust the role. [app/ui/components/growth_chart.slint]
- **Orphan grip on an all-`None`-EPS series** — if every year's EPS is `None` but a forecast est-high-EPS is set, the grip + label render with no anchored trend line (line gated on non-empty commands, handle gated on judgment-y). Rare degenerate. [chart.rs / growth_chart.slint]
- **Single-point series renders nothing** — a 1-year study (or a series with one non-`None` point) emits a lone `M` command → blank plot, no isolated-point marker. [app/src/viewmodel/chart.rs path_commands]
- **Grip vs line-endpoint offset + dead `judgment_x`** — the grip sits centered in the right strip while the trend-line endpoint renders at the plot's right edge; `judgment_x` (always `CHART_W`) is exported but unused. Cosmetic; GUI polish deferred post-MVP. [growth_chart.slint / chart.rs]
- **mouse-y→viewbox-y coupling to fixed height** — the 1:1 drag mapping holds only because the plot is rendered at exactly `CHART_H` px; add a guard/test before making the chart responsive/zoomable. [chart.rs / growth_chart.slint]

## Deferred from: code review of story-2.9 (2026-06-14)

- **Scenario-compare alternate input is a placeholder, not a pre-filled value** — the seeded est-high-EPS binds to `placeholder:` (ghost hint) rather than `text:`, so the field looks empty on open. Minor UX; bind the seed as the editable value. [app/ui/components/scenario_compare.slint]
- **Alternate placement varies only est-high-EPS (Phase-1)** — current price / est-low-EPS / forecast-low option can't be varied in the alternate (Q4 exact-value default). Broaden if richer what-if is wanted (Phase-2 multi-scenario territory). [app/src/main.rs, app/src/viewmodel/engine.rs]
- **Keyboard undo path (AC3) lacks automated coverage + FocusScope focus-order unverified** — the form-wrapping study-screen `FocusScope` relies on key bubbling; manual AT-SPI pass needed on Ctrl+Z with (a) no focus, (b) a validated cell focused, (c) the compare TextField focused. [app/ui/screens/study_screen.slint]
- **Blank/negative/non-numeric alternate input is silently calm** — collapses to an em-dash / withheld alternate column with no rejection signal, indistinguishable from a legitimately missing input. Calm-by-design; consider a neutral "valeur non reconnue" hint. [app/src/main.rs on_set_alternate]

## Deferred from: code review of story-4.6 (2026-06-29)

- **Decimal overflow → panic on unbounded manual qty/price** — `core::risk::{capital_at_risk,total_invested}` multiply/`.sum()` panic if a product or running sum exceeds `Decimal::MAX` (~7.9e28). `validate_holding_amounts` checks only sign (qty>0, price>=0), never magnitude, so an absurd persisted holding crashes the Portefeuille screen on every render/refresh. Pre-existing write-side validation gap; trigger is unrealistic for a real portfolio but it is crash-class (not a neutral notice). Fix belongs on the write validator (cap magnitude) or via checked Decimal arithmetic. [app/src/state.rs:1837 validate_holding_amounts; core/src/risk/mod.rs]
- **`0,0 % du capital investi` shown when no stop is set** — spec-compliant (AC3: no-at-risk holdings -> figure 0; % omitted only when invested=0), but a portfolio with stops absent on every holding shows "0 % du capital investi", which can read as "no downside exposure" rather than "no stop-loss protection". Product/UX decision for Guy — surface un-stopped exposure separately, or suppress the % when CaR=0? [app/src/main.rs:336-344]

## Deferred from: code review of story-5.3 (2026-06-29)

- **Whole-journal import of an older file has no version arbitration** — `import_journal` is an identity-preserving merge (upsert by id) with no version check. Re-importing an OLDER envelope after local changes resurrects a sold holding (`sold_at` → NULL while a SELL transaction still references it — a contradictory ledger) and un-archives a study, silently losing local lifecycle state. Deferred → **GitHub #65**: this is Story 5.4 (restore-from-backup, FR61) territory — integrity + version-compatibility checks BEFORE overwrite, surfacing a stale backup ("you saw v57, this is v41"). The envelope already carries `(journal_id, logical_version)` and `import_journal` returns `source_logical_version`, so 5.4 has the inputs. [persistence/src/export.rs import_journal — holdings `sold_at = excluded.sold_at`, studies `status = excluded.status`]

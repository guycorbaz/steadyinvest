# Story 1.7: `core` normalization layer (pure, metamorphic-tested)

Status: done

<!-- Epic 1. First PRODUCTION story after the three spikes (A/B/C all GO). This is "Murat lever #1":
     the most dangerous source of a silent-wrong-signal (raw → canonical financials) is built and
     metamorphic-tested BEFORE the engine (1.8) consumes it. Both manual entry (Epic 2) and providers
     (Epic 3) reuse this single function — there is exactly ONE place raw data becomes canonical. -->

## Story

As the developer (Guy, solo),
I want a pure normalization function turning raw financial inputs into a canonical form,
so that the most dangerous source of a silent-wrong-signal is built and tested first, and reused by both manual entry (Epic 2) and providers (Epic 3).

## Acceptance Criteria

1. **A pure `normalize(raw: RawFinancials) -> CanonicalFinancials` exists in `core`** (new module `core::normalize`), deterministic (identical input ⇒ identical output), with **no I/O / UI / SQL / network** (Cardinal Rule), all math in exact `Decimal` (never `f32`/`f64`), **no rounding anywhere** (rounding is display-only, `core::rounding`), and **no panic on any input** (checked division / guards; degenerate inputs become typed `unknown/insufficient` states, never a panic and never a silent 0).
2. **Split / series breaks are handled.** (a) *Declared* split events (exact integer ratio, e.g. 3:1, with an effective year) are applied to the **per-share** series only (`eps`, `high_price`, `low_price`, `dividend_per_share`, `book_value_per_share`) — pre-split years rebased into post-split shares; `sales` (aggregate revenue) is never split-adjusted. (b) *Suspected undeclared* breaks are **detected and flagged** with the pinned key `split_series_break` (spec §3: year-over-year EPS or price factor ≥ `split_jump_high()` (1.5) or ≤ `split_jump_low()` (0.67) while sales does not move beyond the same band in the same direction) — **flag only, never auto-corrected** (the system never silently rewrites the user's data).
3. **IFRS↔US-GAAP representational differences canonicalize to the same output.** v1 scope = the spec-named equivalence: pre-tax profit is taken directly when provided, or derived by the §2 gross-up `pre_tax_profit = net_profit / (1 − tax_rate)`; `tax_rate ≥ 1` ⇒ PTP is `unknown` (spec §9), never computed. Equivalent representations (direct PTP vs net+tax_rate) produce the **same canonical year**.
4. **Currency-of-report is checked, never converted.** Every raw amount carries its reporting currency; a cell/year whose currency ≠ the study's native currency is flagged with the pinned key `currency_mismatch`. `core` performs **no FX conversion** (FX exists only at the future consolidation layer, FR5/NFR-C4).
5. **Fiscal-period misalignment is detected** with the pinned key `fiscal_period_misalignment` (spec §3: reported period length ≠ ~12 months, or fiscal-year-end shift between consecutive years) — flag only; the year still passes through.
6. **A year missing a load-bearing field (`sales`, `eps`, `high_price`, `low_price`) is marked `unknown/insufficient`, never coerced to 0** — typed per-year usability state with the missing fields named; `CanonicalFinancials` exposes the **usable-year count** (the FR8 low-confidence *input*; the low-confidence verdict state itself is Story 1.8). The deferred coherence check from Story 1.2 is closed: a test asserts `core::method::LOAD_BEARING_YEAR_FIELDS` ⊆ the year-struct's field names (the comment hook in `core/src/method/mod.rs::load_bearing_lists_are_coherent`).
7. **Metamorphic + property tests hold** (proptest + explicit cases, in `core/tests/`):
   - **Split-invariance:** a 3:1 split applied to inputs (pre-split per-share values × 3 + declared 3:1 event) yields the **same canonical series, exactly** (`assert_eq!`, no tolerance);
   - **IFRS/GAAP-equivalence:** direct-PTP vs net+tax_rate representations of the same economics yield **identical** `CanonicalFinancials`;
   - **Scale-homogeneity:** multiplying all monetary amounts by k > 0 yields the canonical series scaled by exactly k, with identical findings/usability (the ratios/verdict-level half of this AC completes in Story 1.8);
   - **Determinism:** `normalize(x) == normalize(x)` (proptest over generated inputs);
   - **Never-0:** a missing load-bearing field never appears as `Some(0)` anywhere in the canonical output.
8. **Method discipline intact, gates green.** No method constant added/changed ⇒ **no `METHOD_VERSION` bump**, `method_fingerprint` pinned snapshot untouched, determinism probe/hash untouched. `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --all --locked`, `cargo deny check` all green.

## Tasks / Subtasks

- [x] **Task 1 — Types: raw + canonical model (AC: 1, 6)**
  - [x] New module `core/src/normalize/` (`mod.rs` + focused submodules, e.g. `types.rs`, `splits.rs`, `gaap.rs`, `checks.rs` — by domain, no `utils.rs`).
  - [x] Define `RawFinancials { native_currency, years: Vec<RawYear>, splits: Vec<SplitEvent> }`. `RawYear`: `year: i32`, period metadata (`period_months: Option<u32>`, `fiscal_year_end_month: Option<u32>`), per-field `Option<Decimal>` + per-field (or per-year) reporting currency: `sales`, `eps`, `high_price`, `low_price`, `dividend_per_share`, `pre_tax_profit`, `net_profit`, `tax_rate`, `book_value_per_share`. Field names MUST match `core::method::LOAD_BEARING_YEAR_FIELDS` and `contract::YearData` (see Dev Notes — alignment, not dependency).
  - [x] `SplitEvent { effective_year: i32, numerator: u32, denominator: u32 }` — exact integer ratio (3:1, 1:3 reverse), never a `Decimal` ratio.
  - [x] Define `CanonicalFinancials { years: Vec<CanonicalYear>, findings: Vec<Finding>, usable_years: u32 }`; `CanonicalYear` carries the canonical values (`Option<Decimal>`, `None` = unknown/insufficient — never 0) + a typed usability state naming the missing load-bearing fields; `Finding { key: PlausibilityKey, year, field/context }` where the key maps 1:1 onto the pinned strings of `core::quality_flags::PLAUSIBILITY_RULES` (no invented keys).
  - [x] No serde derives on these types in this story (workspace `rust_decimal` has no `serde` feature; the golden-fixture format is Story 1.9's decision).
- [x] **Task 2 — `normalize` skeleton: ordering, usability, never-0 (AC: 1, 6)**
  - [x] Sort years ascending by `year` (deterministic regardless of input order). Duplicate years are a **structural input error** (typed `NormalizeError` or a documented deterministic precedence rule — dev's choice) — NOT a new plausibility key: the `PLAUSIBILITY_RULES` catalog is pinned and must not grow in this story.
  - [x] Mark each year usable iff ALL of `sales`, `eps`, `high_price`, `low_price` are present (spec §4); count `usable_years`.
  - [x] Close the Story-1.2 deferral: test asserting every entry of `LOAD_BEARING_YEAR_FIELDS` is a field of the year struct (e.g. compile-time destructuring or a name list owned by `normalize` asserted equal); update the stale comment in `core/src/method/mod.rs` tests (comment-only edit — does NOT touch the fingerprint).
- [x] **Task 3 — Declared-split adjustment + undeclared-break detection (AC: 2)**
  - [x] Cumulative split factor per year; pre-split per-share values rebased (multiply by `denominator`, divide by `numerator` — checked, exact-integer ops where possible); `sales` untouched.
  - [x] Detection runs **on the post-adjustment series** (after declared splits are applied) — so a correctly declared split no longer trips the detector; only *unexplained* breaks are flagged. y/y factor via `checked_div` (prior-year value 0/None ⇒ no factor, no flag, no panic); EPS-or-price factor outside `[split_jump_low(), split_jump_high()]` band while sales factor stays inside (or moves opposite) ⇒ `split_series_break` finding on that year. Uses ONLY the existing `core::method` constants. Test both: a declared 3:1 split is NOT flagged; the same jump undeclared IS flagged.
  - [x] File a GitHub issue (repo `guycorbaz/steadyinvest`): "Spec §3 `split_series_break`: quantify 'inconsistent with sales'" recording the implemented interpretation, for spec formalization at the next METHOD_VERSION bump (deferred-work item from the 1-2 review; do NOT edit the spec in this story).
- [x] **Task 4 — IFRS/GAAP canonicalization (AC: 3)**
  - [x] PTP: direct `pre_tax_profit` wins; else gross-up `net_profit / (1 − tax_rate)` via `checked_div` with the `tax_rate ≥ 1` guard ⇒ `None` (unknown). Both inputs absent ⇒ `None`. `tax_rate` is a **fraction** in `[0, 1)` (e.g. `0.30`), matching the spec §2 formula — document this on the field.
  - [x] Metamorphic case: same economics, both representations ⇒ identical `CanonicalYear`.
- [x] **Task 5 — Currency + fiscal-period checks (AC: 4, 5)**
  - [x] `currency_mismatch`: any amount whose currency ≠ `native_currency` ⇒ finding (per year/field); value passes through UNCONVERTED.
  - [x] `fiscal_period_misalignment`: `period_months` present and ≠ 12, or `fiscal_year_end_month` shifts between consecutive years ⇒ finding; year still passes through (plausibility never blocks, spec §3).
- [x] **Task 6 — Metamorphic & property tests (AC: 7)**
  - [x] Add `proptest` to `core` `[dev-dependencies]` (workspace-pinned; first dev-dep section in `core/Cargo.toml`).
  - [x] `core/tests/normalize_metamorphic.rs`: split-invariance (construct the split input FROM the canonical one by exact multiplication so `assert_eq!` holds with zero tolerance — same trick as Spike C's exact-by-construction series), IFRS/GAAP-equivalence, scale-homogeneity (×k, k from a positive Decimal strategy), determinism, never-0. Explicit assert messages carrying the diverging values (1-6 lesson: failures must be self-explaining).
  - [x] Unit tests co-located in `#[cfg(test)] mod tests` per submodule (split factor math, gross-up guard, band edges: factor exactly 1.5 ⇒ flagged (≥), exactly 0.67 ⇒ flagged (≤) — spec §7 "normative comparators").
- [x] **Task 7 — Gates & status (AC: 8)**
  - [x] `cargo fmt --all --check` · `cargo clippy --all-targets --all-features --locked -- -D warnings` · `cargo test --all --locked` · `cargo deny check` — all green; verify `method_fingerprint_is_pinned_to_version` and `determinism_hash_matches_cross_os_contract` still pass UNCHANGED.
  - [x] Update `sprint-status.yaml` (1-7 transitions) and the story Dev Agent Record / File List.

## Dev Notes

### Where `normalize` lives — epics override the architecture tree (READ FIRST)

The architecture's directory tree places `normalize/` under `ingestion/src/` — but the **epics file deliberately supersedes that** (post party-mode structure rationale): *"pure `normalize` function (IFRS/GAAP, split/series, fiscal-period, currency-of-report) **inside `core`** (Murat lever #1)"*, and the story title itself is "**`core`** normalization layer". Epic 3 confirms: provider data flows "**through Epic 1's canonical `normalize`**". Rationale: the function is pure calculation (Cardinal Rule ⇒ it belongs in `core`), and it must be reusable by manual entry (Epic 2, no `ingestion` dependency) as well as providers. **Build it in `core/src/normalize/`.** `ingestion` (Story 3.1) will call `core::normalize`; `ingestion/src/normalize/` from the architecture tree is NOT created. This is a documented variance, not an oversight. [Source: epics.md#Epic 1 "Includes:"; epics.md#Epic 3 intro; architecture.md#Complete Project Directory Structure]

### Type boundary: core does NOT depend on contract — alignment by name, not by import

`core`'s deps are `rust_decimal`, `serde`, `sha2` only; `contract` is not among them and must not become one (the architecture lists core deps as "rust_decimal (+maths), serde (types only)"). `normalize` therefore defines its own plain `Decimal`-based input/output structs. The **mapping** raw↔`contract::Cell`/`YearData` (provenance stamping, source/freshness/review state) is the **caller's job** (app in Epic 2, ingestion in Epic 3) — Story 3.1's AC already words it that way ("passed through `normalize` (Epic 1) **into** canonical `contract` types"). What keeps the two vocabularies glued is **field-name alignment**: `RawYear`/`CanonicalYear` field names must match `contract::YearData` (`sales`, `eps`, `high_price`, `low_price`, `dividend_per_share`, `pre_tax_profit`, `book_value_per_share`) and `core::method::LOAD_BEARING_YEAR_FIELDS` — and AC 6's subset test pins that mechanically. [Source: core/Cargo.toml; contract/src/study.rs; core/src/method/mod.rs; architecture.md#Complete Project Directory Structure]

### What ALREADY exists — consume it, do not reinvent or modify

- **`core::method`** pins every threshold this story needs: `split_jump_high()` = 1.5, `split_jump_low()` = 0.67, `LOAD_BEARING_YEAR_FIELDS`, `USABLE_YEARS_FLOOR` = 5. **Use these functions — never re-literal the numbers.** Adding/changing ANY constant in `core::method`/`quality_flags`/`rounding` changes `method_fingerprint()` and fails the pinned snapshot until `METHOD_VERSION` is bumped — this story is designed to need **no new method constant** (see split-detection note below). [Source: core/src/method/mod.rs]
- **`core::quality_flags::PLAUSIBILITY_RULES`** pins the 6 finding keys. 1.7 raises the three **input-shape** ones: `split_series_break`, `currency_mismatch`, `fiscal_period_misalignment`. The three **calc-time** ones (`out_of_bounds_ratio`, `negative_or_zero_denominator`, `low_price_above_current`) belong to the engine, Story 1.8 — do not implement them here. A typed `PlausibilityKey` enum in `normalize` should map 1:1 onto (a subset of) those pinned strings; add a test asserting each enum key's string is in `PLAUSIBILITY_RULES`. [Source: core/src/quality_flags.rs; docs/method/ssg-method-spec-v1.md §3]
- **`core::rounding`** exists for display only. `normalize` must call **none of it** — no `round_dp`, no quantization, full precision end to end (spec §8: "rounding is applied ONLY at display, never mid-calculation"). [Source: core/src/rounding.rs; docs/method/ssg-method-spec-v1.md §8]
- **`core::determinism_probe`/`determinism_hash` + pinned `EXPECTED`** (core/src/lib.rs) and the **Spike-C test** (`core/tests/spike_c_cagr_precision.rs`, pinned digest `d9af5553…5557`) are permanent CI gates — untouched by this story. `core/src/lib.rs` gets exactly one new line (`pub mod normalize;`) plus optional re-exports. [Source: core/src/lib.rs; 1-6 story Dev Agent Record]
- **`contract` v1 types** (Cell with source×freshness×review×coverage, Money-as-string, Provenance) already model the per-cell state — 1.7 does NOT duplicate any of that; unknown/insufficient in `normalize` is plain `Option::None` + the typed usability state. [Source: contract/src/{cell,study,money}.rs]

### Split handling — the precision trap and the exact-test trick

- **Declared events**: cumulative factor per year. A 3:1 split effective year Y rebases per-share values of years < Y by ÷3. **`1/3` is non-terminating in `Decimal`** — a bare division truncates at 28 significant digits (deterministic, but inexact). That is acceptable for real data, but the **metamorphic test must be exact**: construct the split-applied input FROM the unsplit canonical series by exact multiplication (`eps × 3`), then assert `normalize(split_input) == unsplit_canonical` — `3.00 × 3 = 9.00; 9.00 / 3 = 3.00` is exact, so `assert_eq!` holds with **zero tolerance** (same exact-by-construction discipline as Spike C). Keep ratios as integer `numerator`/`denominator`, never pre-divided `Decimal`s.
- **Per-share vs aggregate**: split-adjust `eps`, `high_price`, `low_price`, `dividend_per_share`, `book_value_per_share`; **never `sales`** (total revenue is share-count-independent). PTP/net profit are aggregates too — untouched.
- **Undeclared-break detection ("inconsistent with sales")**: the spec leaves this clause unquantified (deferred-work, 1-2 review). Implement using ONLY existing constants: flag year t when (EPS or high/low price) y/y factor is ≥ `split_jump_high()` or ≤ `split_jump_low()` **and** the sales y/y factor stays within the band (or is absent/moves the opposite way). This adds no method constant ⇒ no fingerprint change ⇒ no METHOD_VERSION bump. Record the interpretation in a **GitHub issue** (single source of truth for spec follow-ups — do NOT edit `docs/method/ssg-method-spec-v1.md` here; spec edits couple to a METHOD_VERSION bump, out of scope). The 0.67-vs-2/3 imprecision noted in deferred-work stays as-is: 0.67 is the spec-pinned constant.
- **Comparators are normative** (spec §7): `≥ 1.5` includes exactly 1.5; `≤ 0.67` includes exactly 0.67. Test the exact boundary values.
- **Detection never mutates**: a suspected break is a finding at the year; the values pass through unchanged. Only *declared* splits adjust data. Silently "fixing" data would be the exact silent-wrong-signal this story exists to kill.

### Degenerate inputs — no panic, no silent 0 (binding)

`rust_decimal` bare ops panic: division by zero, `powd` overflow, `ln(≤0)` (Spike C measured this; `checked_*` variants return `None`). `normalize` does divisions (split rebasing, y/y factors, PTP gross-up) ⇒ **use `checked_div` everywhere**, and guard `tax_rate ≥ 1` explicitly (spec §9: gross-up denominator non-positive ⇒ unknown). A `None`/zero prior-year value ⇒ no y/y factor ⇒ no flag (absence of evidence, not evidence). Missing load-bearing field ⇒ year `unknown/insufficient` with the missing fields named — `Option::None`, **never** `Some(Decimal::ZERO)`. No `.unwrap()`/`.expect()` outside tests. [Source: docs/spikes/spike-c-decimal-cagr-determinism.md; deferred-work.md "No overflow / Result handling in core decimal math"; docs/method/ssg-method-spec-v1.md §9]

### Scope boundaries — what 1.7 does NOT do

- **No verdict, no ratios, no flags from §2**: the engine (CAGR, PTP%, ROE, P/E, zoning, quality flags, low-confidence verdict state) is Story 1.8. The metamorphic ACs phrased at verdict level ("equivalent IFRS/GAAP inputs yield the same **verdict**", "ratios/verdict unchanged") are satisfied at 1.7 by **identical/scaled `CanonicalFinancials`** — which deterministically implies the verdict-level property once 1.8 exists; 1.8's test suite extends these metamorphic tests through the engine. State this in the test comments so 1.8 picks it up.
- **No calc-time plausibility** (`out_of_bounds_ratio`, `negative_or_zero_denominator`, `low_price_above_current`) — Story 1.8.
- **No UI surfacing** of findings (FR10 display = Story 2.7); no locale parsing of raw strings (raw input is already `Decimal` — locale-aware string→Decimal parsing is the grid's job, Story 2.4); no provider mapping (Story 3.1); no FX (Epic 6).
- **No quarterly/TTM inputs**: the v1 raw model is the annual series. The §1 recent-quarter deltas and §3 TTM-EPS current P/E are engine inputs whose shape Story 1.8 defines — do not speculatively add quarterly fields to `RawYear` here.
- **No serde / fixture format** for normalize types — Story 1.9 (golden runner) owns the fixture decision; workspace `rust_decimal` lacks the `serde` feature today, so adding derives now would force a feature change for no consumer.

### Previous story intelligence (1-1 → 1-6 dev records)

- **MSRV 1.96** (`rust-toolchain.toml`; the architecture's "1.88" is stale). **CI Linux-only** (decision 2026-06-09) — do not touch the matrix; determinism holds by construction (pure-Rust `rust_decimal`).
- Gates run `--locked`; clippy `-D warnings` covers `--all-targets` — integration tests and proptest closures ARE linted, keep them clean.
- "Done" = **demonstrably works** (evidence in test output), not "it compiles". Prefer explicit `assert!`/`assert_eq!` messages carrying measured values so a CI failure is self-explaining (the 1-6 pattern).
- Don't silence errors (no `.ok()`); the 1-1 review's checked-math deferral applies to THIS story's divisions, not just 1.8.
- Spike C measured `powd` fractional error ≤1e-27 and pinned panic-vs-`checked_*` behaviour — 1.7 needs no `powd` at all (no exponentiation in normalization), only `checked_div`/multiplication.
- Tech research note: no new external crates beyond promoting workspace-pinned `proptest` into `core` dev-deps; `rust_decimal` 1.42 behaviour was just empirically validated by Spike C — no version-currency risk in this story.

### Project Structure Notes

- **New:** `core/src/normalize/` (`mod.rs`, `types.rs`, `splits.rs`, `gaap.rs`, `checks.rs` — adjust granularity, by domain); `core/tests/normalize_metamorphic.rs`.
- **Modified:** `core/src/lib.rs` (add `pub mod normalize;` + re-exports; do NOT touch probe/hash); `core/Cargo.toml` (add `[dev-dependencies] proptest`, workspace-pinned); `core/src/method/mod.rs` (**comment-only**: the "deferred to Story 1.7" note in `load_bearing_lists_are_coherent` — constants untouched, fingerprint unchanged); `_bmad-output/implementation-artifacts/sprint-status.yaml` (1-7 transitions).
- **Do NOT modify:** `core/src/method/mod.rs` constants / `method_version.rs` / `quality_flags.rs` / `rounding.rs` (any constant change breaks the pinned fingerprint and demands a METHOD_VERSION bump — out of scope); `core/src/lib.rs` probe/hash + pinned `EXPECTED`; `core/tests/spike_c_cagr_precision.rs`; `docs/method/ssg-method-spec-v1.md` (file the split-detection interpretation as a GitHub issue instead); `contract/` (no changes needed — and core must not depend on it); `.github/workflows/ci.yml` (`cargo test --all --locked` already runs the new tests; Linux-only stays); `ingestion/` (stays a stub until Epic 3).
- **Naming:** module/files `snake_case` by domain; types `PascalCase` (`RawFinancials`, `CanonicalFinancials`, `SplitEvent`, `Finding`); no `utils.rs`.

### References

- [Source: epics.md#Story 1.7] — user story + the three ACs (pure normalize, metamorphic invariances, never-coerce-to-0)
- [Source: epics.md#Epic 1 "Includes:"] — "pure `normalize` function … **inside `core`** (Murat lever #1)"; metamorphic runner with split-invariance is part of the Epic-1 test harness
- [Source: epics.md#Epic 3 intro + Story 3.1] — providers flow "through Epic 1's canonical `normalize`"; ingestion maps raw → normalize → contract types (the caller owns the mapping)
- [Source: architecture.md#Technical Constraints] — "Ingestion/normalization is a first-order architectural boundary … the real birthplace of the silent-wrong-signal … metamorphic tests (equivalent IFRS/GAAP inputs ⇒ same verdict)"
- [Source: architecture.md#Data Architecture / #Enforcement Guidelines] — `unknown/insufficient` first-class, never coerced to 0; Cardinal Rule; no floats; checked paths; structure tree (superseded for normalize placement by epics — documented variance above)
- [Source: docs/method/ssg-method-spec-v1.md §2] — PTP gross-up `net_profit / (1 − tax_rate)` (the v1 IFRS/GAAP equivalence)
- [Source: docs/method/ssg-method-spec-v1.md §3] — plausibility rules: `split_series_break` (1.5 / 0.67), `currency_mismatch`, `fiscal_period_misalignment`; "never block computation"
- [Source: docs/method/ssg-method-spec-v1.md §4/§5] — usable year = all load-bearing fields present; load-bearing list
- [Source: docs/method/ssg-method-spec-v1.md §7/§8/§9] — normative comparators; rounding display-only; degenerate-input rules (`tax_rate ≥ 1` ⇒ unknown)
- [Source: core/src/method/mod.rs] — `split_jump_high/low()`, `LOAD_BEARING_YEAR_FIELDS`, fingerprint discipline, the "deferred to Story 1.7" subset-check hook
- [Source: core/src/quality_flags.rs] — pinned `PLAUSIBILITY_RULES` keys (use, don't invent)
- [Source: contract/src/study.rs / cell.rs] — `YearData` field names to align with; per-cell state model that 1.7 must NOT duplicate
- [Source: deferred-work.md (1-2 review)] — `split_series_break` "inconsistent with sales" unquantified → implement with existing constants + GitHub issue
- [Source: deferred-work.md (1-1 review)] — checked decimal math so pure `core` never panics
- [Source: docs/spikes/spike-c-decimal-cagr-determinism.md / 1-6 Dev Agent Record] — `checked_*` semantics measured; exact-by-construction test discipline; gates `--locked`; Linux-only CI
- [Source: prd.md FR10] — "unadjusted split / series break, currency mismatch, fiscal-period misalignment" wording

## Dev Agent Record

### Agent Model Used

Claude Fable 5 (claude-fable-5) — Claude Code

### Debug Log References

- Initial run of `declared_split_is_not_flagged_but_undeclared_jump_is` failed: the test fixture reported identical prices for both years, so the declared-split rebase itself created a price jump. Fixed the fixture (pre-split prices ×3, consistent with the eps series) — implementation was correct; the failure was the test's own series being internally inconsistent.
- `cargo clippy --all-targets -D warnings`: 5 `redundant_closure` in the metamorphic suite (`.and_then(|v| amt(v))` → `.and_then(amt)`) — fixed; gates green afterwards.

### Implementation Plan

- `core/src/normalize/` split by domain: `types.rs` (raw/canonical model, `PlausibilityKey`, `NormalizeError`, load-bearing presence lookup), `splits.rs` (cumulative integer ratio, checked rebase, post-adjustment break detection), `gaap.rs` (PTP precedence + §2 gross-up with the `tax_rate ≥ 1` guard), `checks.rs` (currency + fiscal passes), `mod.rs` (orchestration: validate splits → sort/dedup years → currency → fiscal → build canonical years → detect breaks → usable count).
- **Dev's choice (Task 2):** duplicate years are a typed `NormalizeError::DuplicateYear` (not a precedence rule); zero split numerator/denominator is `NormalizeError::InvalidSplitRatio`. So the public signature is `normalize(raw: RawFinancials) -> Result<CanonicalFinancials, NormalizeError>` — structural input errors are typed, everything else is total (no panic).
- Usability is derived from the **canonical** presence of the load-bearing fields (post-rebase), driven mechanically by `LOAD_BEARING_YEAR_FIELDS` (never a re-literal'd list); an unresolvable constant name counts as missing (degrade-safe) and the subset test pins resolvability.
- Split-break "inconsistent with sales" interpretation (spec leaves it unquantified): flag when the per-share factor breaches the band while the sales factor does NOT move beyond the same band in the same direction; absent/opposite sales ⇒ flagged; comparison only across calendar-consecutive years. Recorded in GitHub issue [#12](https://github.com/guycorbaz/steadyinvest/issues/12) for the next METHOD_VERSION bump.
- Per-field reporting currency via `RawAmount { value, currency }` (the "per-field" option of Task 1); `tax_rate` is a unitless fraction, no currency.

### Completion Notes List

- AC1 — `core::normalize::normalize` is pure (no I/O/UI/SQL/network; only `rust_decimal` + `crate::method`/`quality_flags`), exact `Decimal` end to end, zero calls into `core::rounding`, all divisions/multiplications checked (`checked_div`/`checked_mul`/`checked_sub`), no `.unwrap()`/`.expect()` outside tests.
- AC2 — declared splits rebase only `eps`, `high_price`, `low_price`, `dividend_per_share`, `book_value_per_share` (×denominator ÷numerator, exact integer cumulative ratio); `sales` and PTP/net are never adjusted. Undeclared-break detection runs on the post-adjustment series with the pinned `split_jump_high()`/`split_jump_low()` constants — both directions tested, declared-split-not-flagged and undeclared-jump-flagged both asserted. Flag only, values untouched.
- AC3 — direct PTP wins; §2 gross-up `net / (1 − tax_rate)` with `tax_rate ≥ 1 ⇒ None` (spec §9). Metamorphic equivalence holds exactly (proptest + explicit case).
- AC4 — `currency_mismatch` per year/field, verbatim comparison, value passes through unconverted; no FX anywhere in `core`.
- AC5 — `fiscal_period_misalignment` on `period_months != 12` and on fiscal-year-end shift between consecutive reported years; year still passes through.
- AC6 — `YearUsability::Insufficient { missing }` names the missing load-bearing fields; `usable_years` exposed on `CanonicalFinancials`. Story-1.2 deferral closed: `load_bearing_year_fields_are_subset_of_year_struct` (+ exhaustive-destructuring guard `value_field_names_match_struct`), stale comment in `core/src/method/mod.rs` updated (comment-only).
- AC7 — `core/tests/normalize_metamorphic.rs`: split-invariance (exact-by-construction, zero tolerance), IFRS/GAAP equivalence, scale-homogeneity (k > 0, terminating-quotient strategy sets; findings/usability invariant), determinism, never-0 — all proptest, with explicit boundary cases (factor exactly 1.5 ⇒ flagged, exactly 0.67 ⇒ flagged) unit-tested in `splits.rs`. Test comments state the 1.8 verdict-level handoff.
- AC8 — no method constant added/changed: `method_fingerprint_is_pinned_to_version` and `determinism_hash_matches_cross_os_contract` pass UNCHANGED. Gates all green: `cargo fmt --all --check` · `cargo clippy --all-targets --all-features --locked -- -D warnings` · `cargo test --all --locked` (72 core tests: 44 unit + 9 e2e + 5 metamorphic + 14 Spike C; contract suite unaffected) · `cargo deny check` (advisories/bans/licenses/sources ok).
- GitHub issue filed: [guycorbaz/steadyinvest#12](https://github.com/guycorbaz/steadyinvest/issues/12) — "Spec §3 `split_series_break`: quantify 'inconsistent with sales'" (implemented interpretation recorded for the next METHOD_VERSION bump).
- `Cargo.lock` updated only by the `proptest` dev-dependency edge on `steadyinvest-core` (workspace-pinned version already in the lock via `contract`).

### File List

- `core/src/normalize/mod.rs` (new) — `normalize` orchestration + module docs + re-exports
- `core/src/normalize/types.rs` (new) — raw/canonical types, `PlausibilityKey`, `Finding`, `NormalizeError`, load-bearing presence lookup + subset test
- `core/src/normalize/splits.rs` (new) — cumulative split ratio, checked per-share rebase, undeclared-break detection
- `core/src/normalize/gaap.rs` (new) — canonical PTP (direct precedence + §2 gross-up, §9 guard)
- `core/src/normalize/checks.rs` (new) — currency-of-report + fiscal-period findings
- `core/tests/normalize_metamorphic.rs` (new) — metamorphic & property suite (AC 7)
- `core/tests/normalize_e2e.rs` (new) — feature-level e2e suite through the public `normalize` API (QA pass, `bmad-qa-generate-e2e-tests`)
- `core/src/lib.rs` (modified) — `pub mod normalize;` + re-exports (probe/hash untouched)
- `core/src/method/mod.rs` (modified) — comment-only: deferral note in `load_bearing_lists_are_coherent` closed (constants/fingerprint untouched)
- `core/Cargo.toml` (modified) — `[dev-dependencies] proptest` (workspace-pinned)
- `Cargo.lock` (modified) — proptest dev-dep edge for `steadyinvest-core`
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified) — 1-7 transitions
- `_bmad-output/implementation-artifacts/tests/test-summary.md` (modified) — QA gap-analysis summary for 1.7
- `_bmad-output/implementation-artifacts/1-7-core-normalization-layer.md` (modified) — this story file

## Senior Developer Review (AI)

**Reviewer:** Guy (autonomous story-automator review) — 2026-06-11
**Outcome:** Approve — all 8 ACs verified implemented; 0 Critical, 0 High, 2 Medium, 3 Low findings; all fixed in-review.

### AC validation (all IMPLEMENTED)

- AC1 ✓ `core::normalize::normalize` pure (deps: `rust_decimal` + `crate::{method,quality_flags}` only), all divisions `checked_*`, no `.unwrap()`/`.expect()` outside tests, no rounding calls.
- AC2 ✓ declared splits rebase exactly the 5 per-share fields (`splits.rs`); `sales`/PTP never adjusted; detection on the post-adjustment series, flag-only, both declared-not-flagged and undeclared-flagged asserted (`mod.rs` test + e2e).
- AC3 ✓ direct-PTP precedence + §2 gross-up with `tax_rate ≥ 1 ⇒ None` (`gaap.rs`); equivalence proven (proptest + explicit).
- AC4 ✓ `currency_mismatch` per year/field, verbatim compare, value unconverted; no FX in `core`.
- AC5 ✓ `fiscal_period_misalignment` on `period_months ≠ 12` and FYE-month shift; year passes through.
- AC6 ✓ `YearUsability::Insufficient { missing }` + `usable_years`; 1.2 deferral closed (`load_bearing_year_fields_are_subset_of_year_struct` + exhaustive-destructuring guard).
- AC7 ✓ all 5 metamorphic/property tests present with zero-tolerance equality; band edges 1.5/0.67 unit-tested inclusive.
- AC8 ✓ re-ran all four gates during review: fmt, clippy `-D warnings`, `cargo test --all --locked` (72 core tests green), `cargo deny check` — fingerprint & determinism-hash snapshots unchanged. GitHub issue #12 verified open.

### Findings & resolutions (all fixed)

1. **[MEDIUM][fixed] File List incomplete** — `core/tests/normalize_e2e.rs` (new, 9 tests, QA pass) and `_bmad-output/implementation-artifacts/tests/test-summary.md` existed in git but were absent from the story File List. → Both added.
2. **[MEDIUM][fixed] Stale test count in Completion Notes** — claimed "63 core tests" but the e2e suite brings the real count to 72 (44 unit + 9 e2e + 5 metamorphic + 14 Spike C). → Corrected.
3. **[LOW][fixed] Stale crate doc in `core/src/lib.rs`** — still said the consuming layers "arrive in later Epic 1 stories (1.7–1.11)" although 1.7's `normalize` now lives in the crate. → Reworded to 1.8–1.11 with a `normalize` pointer (comment-only; hash/fingerprint unaffected).
4. **[LOW][fixed] Undocumented non-validation of fiscal metadata** — `fiscal_year_end_month` doc said "(1–12)" but out-of-range values are accepted silently (harmless: only ever compared for equality). → Doc on the field now states values are compared verbatim, not range-validated.
5. **[LOW][fixed] Property-test coverage gap** — the metamorphic generators never produced `book_value_per_share`, so split-invariance/scale-homogeneity exercised its rebase path only via fixed-value unit/e2e tests. → `year_parts()` extended to generate it (split world scales it ×3 like the other per-share fields).

Non-blocking note (no action): the undeclared-break detector compares only calendar-consecutive years while the FYE-shift check compares consecutive *reported* years across gaps — both are defensible readings of spec §3; revisit alongside GitHub issue #12 at the next METHOD_VERSION bump.

## Change Log

| Date | Change |
|------|--------|
| 2026-06-11 | Senior Developer Review (AI) appended (status → done): Approve — all 8 ACs verified, gates re-run green (72 core tests, fingerprint/hash unchanged). 5 findings (2 Medium documentation, 3 Low) all auto-fixed: File List completed (e2e suite + test-summary), test count corrected, stale `lib.rs` crate doc refreshed, fiscal-metadata non-validation documented, `book_value_per_share` added to the metamorphic generators. |
| 2026-06-11 | Story 1.7 implemented (status → review): `core::normalize` built and metamorphic-tested — types + skeleton (typed `NormalizeError`, usability with named missing fields, never-0), declared-split rebasing + post-adjustment undeclared-break detection (existing constants only, interpretation filed as GitHub issue #12), §2 PTP gross-up canonicalization, currency/fiscal findings, proptest metamorphic suite (split-invariance, GAAP-equivalence, scale-homogeneity, determinism, never-0). Story-1.2 subset-check deferral closed. No METHOD_VERSION bump; fingerprint and determinism hash pinned snapshots pass unchanged; all gates green. |
| 2026-06-11 | Story 1.7 created (ready-for-dev): pure `core::normalize` (raw → CanonicalFinancials) — declared-split rebasing (per-share only) + undeclared-break detection with existing method constants, PTP IFRS/GAAP gross-up canonicalization, currency-of-report & fiscal-period findings (pinned plausibility keys), usable-year marking with never-coerce-to-0, metamorphic suite (split-invariance exact, GAAP-equivalence, scale-homogeneity, determinism) + closure of the Story-1.2 load-bearing subset-check deferral. No METHOD_VERSION bump, fingerprint/probe untouched. Ultimate context engine analysis completed — comprehensive developer guide created. |

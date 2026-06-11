# Test Automation Summary — Story 1.8 (`core` SSG calculation engine)

Date: 2026-06-11
Workflow: `bmad-qa-generate-e2e-tests` (auto-apply gaps mode)
Framework: Rust integration tests (`cargo test` + `proptest`) — the project's existing
framework. No UI or HTTP surface exists in this story (pure `core` library, Cardinal Rule),
so "E2E" here = feature-level tests through the public pipeline
`normalize(RawFinancials)` → `compute(&CanonicalFinancials, &JudgmentInputs, &QuarterlyObservations)` → `SsgOutputs`,
exactly as the Epic-2 app will call it.

## Scope

Story 1.8 already ships a strong dev suite: 23 explicit tests (`core/tests/ssg_engine.rs` —
worked example, full spec-§9 table, boundary comparators), 6 properties
(`core/tests/ssg_metamorphic.rs` — determinism, NFR-C3, U/D ≥ 0, the three 1.7-handoff
invariances) and 17 in-module unit tests. QA pass = gap analysis at the **public-API
(consumer) level**: behaviours pinned only in unit tests, or documented contracts never
asserted through `compute`. All discovered gaps were auto-applied as a new integration suite.

## Discovered Gaps → Generated Tests

All in `core/tests/ssg_qa_e2e.rs` (new file, 9 tests):

- [x] `forecast_low_option_b_uses_the_average_low_price` — §4 forecast-low option (b)
  `AvgLowPriceLast5y` was never exercised anywhere; pins forecast_low = the §3 average low
  price (12.2102 exact) and the resulting zone classification.
- [x] `forecast_low_option_c_uses_the_recent_severe_low_and_degrades_without_it` — option (c)
  `RecentSevereLow` had no explicit engine test (metamorphic-only): happy path verbatim, plus
  the unknown-cascade (forecast_low/zones/U-D/verdict fact) when the input is absent.
- [x] `eps_lags_sales_fires_when_eps_cagr_is_strictly_below_sales_cagr` — the flag had only
  never-raised tests (equal CAGRs, unknown CAGR); positive firing on flat EPS (CAGR exactly
  0%) vs 10%/yr sales, with the full flag vector pinned to exactly `[EpsLagsSales]`.
- [x] `roe_low_and_roe_trend_declining_fire_on_a_collapsing_roe` — both ROE flags had no
  positive firing; ROE marching 25 → 8 (terminating quotients by construction) fires both,
  and the pinned-catalog flag ORDER is asserted.
- [x] `ud_below_target_fires_and_a_neutral_zone_price_is_a_measured_miss` — `ud_below_target`
  had no positive firing, and `Zone::Neutral` was never asserted through the engine; also
  pins the measured-miss tri-state (`Unmet`, distinct from `UnmetByInsufficiency`) for three
  criteria at once and the broken candidate fact.
- [x] `a_price_in_the_top_third_classifies_as_sell` — `Zone::Sell` was never asserted
  anywhere through `compute` (unit-level only).
- [x] `out_of_bounds_ratios_are_flagged_through_the_engine_and_still_reported` —
  `out_of_bounds_ratio` was unit-test-only; PTP 200% (> 100 bound) and high P/E 300
  (> 200 axis) driven through the engine, exact findings vector (documented pass order:
  management before valuation) AND the values still reported (plausibility never blocks).
- [x] `absent_low_eps_is_never_derived_and_degrades_the_low_side_only` — the v1 direct-only
  low-EPS rule was only asserted with the value present; with it absent (growth judgment
  still present) nothing is derived, option (a) degrades the whole §4 low side while the
  high side keeps computing.
- [x] `a_rising_ptp_trends_up_and_raises_no_flag` — `Trend::Up` was never asserted anywhere
  (only Even/Down); an improving margin raises no flag (empty flag vector pinned).

## Verification

- `cargo test -p steadyinvest-core --test ssg_qa_e2e --locked`: **9 passed, 0 failed**.
- `cargo fmt --all --check`: clean. `cargo clippy --all-targets --all-features --locked -- -D warnings`: 0 warnings.
- `cargo test --all --locked`: **all green** — core now 126 tests (60 unit + 5 + 9
  normalize + 14 Spike C + 23 engine + 6 metamorphic + 9 QA e2e); contract suite unaffected.
- Method discipline intact: no constant touched, `method_fingerprint_is_pinned_to_version`,
  `determinism_hash_matches_cross_os_contract` and the Spike-C digest pass unchanged.

## Coverage

- Story 1.8 ACs at the public-API level: AC 2–8 each now have engine-level coverage of every
  enumerated branch this QA pass could identify; AC 9–10 were already covered and re-verified.
- §4 forecast-low options: 4/4 now engine-tested (previously 2/4).
- Zone classification: 3/3 zones now engine-tested (previously 1/3).
- Quality flags: 9/9 raisable flags now have a positive firing or an explicit
  boundary/never-on-unknown engine test (previously 4/9 had positive firings).
- Calc-time plausibility keys: 3/3 now engine-tested (previously 2/3).
- `CriterionFact` tri-state: 3/3 states now asserted through the engine.

## Next Steps

- Nothing to wire: `cargo test --all --locked` in CI already runs the new file.
- Story 1.9 (golden fixtures) adds the fixture-driven oracle layer on top of these
  constructed-input tests; the tutorial company here is a natural first golden candidate.

---

# Test Automation Summary — Story 1.7 (`core` normalization layer)

Date: 2026-06-11
Workflow: `bmad-qa-generate-e2e-tests` (auto-apply gaps mode)
Framework: Rust integration tests (`cargo test` + `proptest`) — the project's existing
framework. No UI or HTTP surface exists in this story (pure `core` library), so browser-E2E
and HTTP-API tests do not apply; "E2E" here = feature-level tests through the one public
entry point `core::normalize::normalize`, exactly as Epic-2 manual entry and Epic-3
providers will call it.

## Scope

Story 1.7 already ships a strong metamorphic/property suite (`core/tests/normalize_metamorphic.rs`,
AC 7) and per-submodule unit tests. QA pass = gap analysis at the **public-API (consumer) level**:
behaviours pinned only on internal `pub(super)` functions, or documented contracts never asserted
end to end. All discovered gaps were auto-applied as a new integration suite.

## Discovered Gaps → Generated Tests

All in `core/tests/normalize_e2e.rs` (new file, 9 tests):

- [x] `full_messy_series_normalizes_end_to_end_with_deterministic_findings_order` — realistic
  six-year SSG series given out of order, carrying every input-shape issue at once (declared
  2:1 split, EUR amount, 9-month period, fiscal-year-end shift, undeclared EPS jump, missing
  `low_price`). Pins the documented **cross-pass findings order** (currency → fiscal →
  split-break) — a determinism contract previously untested — plus rebasing, pass-through,
  usability naming, and `usable_years == USABLE_YEARS_FLOOR` (the FR8 input).
- [x] `empty_input_yields_empty_canonical_output_without_panic` — zero years was never
  exercised: empty output, no findings, `usable_years = 0`, no panic.
- [x] `reverse_split_one_for_three_multiplies_pre_split_per_share_values` — 1:3 reverse split
  through the public API (previously unit-only on `rebase_per_share`); declared reverse split
  does not trip the break detector.
- [x] `multiple_splits_compound_cumulatively_across_the_series` — two declared splits (2:1 +
  3:1) compound ÷6/÷3/÷1 across the series, exact by construction (previously unit-only on
  `cumulative_ratio`).
- [x] `aggregate_ptp_and_gross_up_are_never_split_adjusted` — PTP (direct AND §2 gross-up
  derived) is share-count independent: untested anywhere before (the existing aggregate test
  only covered `sales`).
- [x] `degenerate_tax_rate_yields_unknown_ptp_but_year_stays_usable` — `tax_rate = 1` through
  the public API: PTP `None` (spec §9), year stays usable (PTP not load-bearing), no finding,
  no panic.
- [x] `non_adjacent_duplicate_years_are_rejected_after_sorting` — duplicates separated in the
  input order (the structural check runs post-sort; only adjacent duplicates were tested).
- [x] `split_effective_at_first_year_rebases_nothing` — boundary: only years strictly BEFORE
  the effective year are rebased.
- [x] `split_effective_after_last_year_rebases_every_year` — boundary: a uniform rebase
  changes values but preserves every y/y factor (detector silent, usability intact).

## Verification

- `cargo test -p steadyinvest-core --test normalize_e2e --locked`: **9 passed, 0 failed**.
- `cargo fmt --all --check`: clean. `cargo clippy --all-targets --all-features --locked -- -D warnings`: 0 warnings.
- `cargo test --all --locked`: **all green** — core now 72 tests (44 unit + 5 metamorphic +
  9 e2e + 14 Spike C); contract suite unaffected.
- `cargo deny check`: advisories/bans/licenses/sources ok.
- Method discipline intact: no constant touched, `method_fingerprint_is_pinned_to_version`
  and `determinism_hash_matches_cross_os_contract` pass unchanged.

## Coverage

- Story 1.7 ACs at the public-API level: 8/8 exercised end to end (AC 1–6 each have at least
  one e2e scenario; AC 7 was already covered by the metamorphic suite; AC 8 re-verified).
- Plausibility findings: 3/3 pinned keys now asserted through `normalize` (fiscal-period
  findings previously unit-only).
- Findings cross-pass deterministic ordering: now regression-gated (previously 0 tests).

## Next Steps

- Nothing to wire: `cargo test --all --locked` in CI already runs the new file.
- Story 1.8 (engine) can reuse `full_messy_series_…` as the seed scenario for extending the
  metamorphic properties through the verdict level (see the handoff note in
  `normalize_metamorphic.rs`).

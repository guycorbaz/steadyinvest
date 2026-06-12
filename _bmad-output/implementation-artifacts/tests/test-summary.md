# Test Automation Summary — Story 1.9 (golden reference studies & self-check gate)

Date: 2026-06-12
Workflow: `bmad-qa-generate-e2e-tests` (auto-apply gaps mode)
Framework: Rust integration tests (`cargo test`) — the project's existing framework. No UI
or HTTP surface exists in this story (pure `core` library, Cardinal Rule), so "E2E" here =
consumer-level tests over the exact public surface Story 2.13 will call:
`GoldenStudy` (serde) → `check` / `check_all` → `GoldenReport` / `GoldenDeviation`.

## Scope

Story 1.9 already ships a strong dev suite: the CI gate `core/tests/golden_gate.rs`
(11-fixture bundle, AC-6 negative controls a–f, app-asset drift test) and 18 in-module unit
tests (parse discipline, tolerance boundary, `null`⇔`None`, posture gate). QA pass = gap
analysis at the **public-API (consumer) level**: behaviours pinned only on private
comparison primitives, or documented report semantics never asserted through the full
`check`. All discovered gaps were auto-applied as a new integration suite. Same tamper
discipline as the gate: variants exist only in memory, never as fixture files.

## Discovered Gaps → Generated Tests

All in `core/tests/golden_qa_e2e.rs` (new file, 10 tests):

- [x] `check_all_over_the_real_bundle_reports_in_input_order_and_isolates_failures` —
  `check_all` (the literal Story-2.13 entry point) was only tested with a single passing
  study in-module; now driven over the real 11-fixture bundle (report order = input order,
  ids carried) plus a mixed bundle where an appended stale-method study fails alone.
- [x] `categorical_deviations_accumulate_across_sections_in_pass_order` — the report being
  a full deviation LIST (not first-failure) was never asserted: six categorical tampers
  across five sections (trend, zone ×2, `low_confidence`, criterion fact, candidate) all
  reported, exact pass-order pinned, `relative_error` always `None` for categoricals.
- [x] `tampered_quality_flag_list_is_one_deviation_carrying_both_lists` — the ordered-list
  comparison (`compare_list`) had zero tests at any level; a flag mismatch is ONE deviation
  rendering both full lists (`[]` vs `[ptp_trend_declining]`).
- [x] `tampered_findings_and_normalize_findings_are_both_reported` — a MISMATCH on
  `findings` / `normalize_findings` was never proven to be reported (only matching values
  passed in the gate); also pins the documented rendering (`study-level` vs `year N`).
- [x] `upside_downside_state_is_exact_while_its_ratio_value_is_tolerance_compared` — the
  U/D three-way state mismatch arm was untested at any level: expected `unknown` vs actual
  `Ratio` deviates exactly, while a Ratio value inside ±0.5% passes (state exact, value
  numeric — the AC-2 split).
- [x] `expected_null_zones_against_computed_bounds_is_a_structural_deviation` — the
  `zones` null ⇔ present mismatch arm was untested: one structural deviation for the whole
  bounds block, not four numeric misses.
- [x] `omitted_optional_blocks_are_not_asserted` — the AC-3 "omitted = not asserted" rule
  was only implicit (fixtures g03–g11 omit the tables); now proven by structurally removing
  both per-year tables AND `normalize_findings` from the fullest fixture (still passes).
- [x] `present_per_year_tables_are_asserted_by_row_and_by_count` — a PRESENT table being
  really asserted was never tested: a tampered row value deviates under its year-indexed
  path (`management.per_year[2023].ptp_pct`), a popped row is one `"4 rows"` vs `"5 rows"`
  structural deviation.
- [x] `tolerance_is_relative_to_the_magnitude_of_a_negative_expected_value` — the spec-§7
  `|expected|` (symmetric tolerance around a NEGATIVE expected) was only tested with
  positive values; proven through the full check on g09's hand-computed TTM EPS −0.40
  (−0.4018 passes, −0.4021 deviates).
- [x] `deviation_display_renders_path_values_and_optional_relative_error` — the
  `GoldenDeviation` Display string (the line Story 2.13 renders) had no test: numeric
  deviations carry the `(relative error …)` suffix, categorical ones end at the actual
  value.

## Verification

- `cargo test -p steadyinvest-core --test golden_qa_e2e --locked`: **10 passed, 0 failed**
  (all on the first run — no engine or fixture changes were needed).
- `cargo fmt --all --check`: clean. `cargo clippy --all-targets --all-features --locked -- -D warnings`: 0 warnings.
- `cargo test --all --locked`: **all green** — workspace now 186 tests (core 159: 75 unit +
  8 gate + 10 golden QA e2e + 9 normalize e2e + 5 metamorphic + 14 Spike C + 23 engine +
  6 ssg metamorphic + 9 ssg QA e2e; contract 27).
- `cargo deny check`: advisories/bans/licenses/sources ok.
- Method discipline intact: no constant touched, `method_fingerprint_is_pinned_to_version`,
  `determinism_hash_matches_cross_os_contract` and the Spike-C digest pass unchanged. No
  fixture file modified (tampered variants are in-memory only); `app/assets` drift test
  still green.

## Coverage

- Story 1.9 ACs at the consumer level: AC 1–3 and 5–7 each now have public-API coverage of
  every comparison/reporting branch this QA pass could identify (AC 4 is the fixture bundle
  itself; AC 8 re-verified above).
- `check_all`: real-bundle + mixed pass/fail now tested (previously single-study only).
- Comparison arms through the full `check`: list (flags/findings/normalize_findings) 3/3
  (previously 0/3 mismatch-tested), U/D state mismatch 1/1 (previously 0), `zones`
  null⇔present 1/1 (previously 0), per-year tables omitted/tampered/count 3/3 (previously
  0 explicit), negative-expected tolerance 1/1 (previously 0).
- `GoldenDeviation` Display: regression-gated (previously 0 tests).

## Next Steps

- Nothing to wire: `cargo test --all --locked` in CI already runs the new file.
- Story 2.13 ("verify engine" UI) can consume `check_all` exactly as
  `check_all_over_the_real_bundle_…` demonstrates, including the Display strings for the
  deviation list.

---

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

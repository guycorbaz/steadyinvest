# Story 1.9: Golden reference studies & self-check gate

Status: done

<!-- Epic 1. The trust capstone of the headless engine: a fixture format + pure self-check runner
     that replays synthetic golden studies through normalize → compute and gates CI on the result.
     The engine (1.8) and its tolerance constant (golden_relative_tolerance, pinned since 1.2) both
     already exist — this story adds NO math and NO method change; it adds the oracle harness.
     The FR9 "verify engine" UI is Story 2.13; this story makes it a file-read + one function call. -->

## Story

As the developer (Guy, solo),
I want bundled golden reference studies run as a self-check,
so that any deviation of the engine from the canonical method is caught automatically.

## Acceptance Criteria

1. **A golden-fixture format exists in `core::golden`** (new module, pure — no file I/O in
   `core/src`): a serde-deserializable `GoldenStudy` schema with three parts — **meta**
   (`id`, `title`, `description`, `provenance` (derivation: who/when/how computed and how
   cross-checked), `method_version`, `fixture_format_version: u32` = 1), **input** (raw financials
   mirroring `normalize`'s input shape — `native_currency`, years with the 12 `RawYear` fields,
   split events — plus the engine's judgment inputs and quarterly observations), and **expected**
   (AC 3). Fixture structs are golden-local (they convert into `RawFinancials`/`JudgmentInputs`/
   `QuarterlyObservations`; no serde derives are added to any existing `normalize`/`ssg` type).
   **Every fixture struct carries `#[serde(deny_unknown_fields)]` and no `#[serde(default)]`**
   — a typo'd or omitted field fails the parse, never silently weakens a golden (the journal's
   forward-compat rule is deliberately INVERTED here: fixtures are oracles, not user data).
   **Beware serde's `Option` trap**: a derive treats a *missing* `Option<T>` field as `None` with
   no `serde(default)` needed — so required-but-nullable expected fields MUST use a
   presence-enforcing mechanism (e.g. `#[serde(deserialize_with = …)]` helper or a missing-vs-null
   distinguishing wrapper) such that an *omitted* required field is a parse error while explicit
   `null` means "expected unknown"; only the AC-3 optional per-year tables use plain `Option`
   semantics. `provenance` is a free-text `String` for v1 (multi-line fine — schema churn across
   10+ fixtures is costlier than structure).
   Decimals are JSON **strings** parsed with `Decimal::from_str_exact` via a local serde helper
   (the `contract::Money` exactness discipline, without `core` depending on `contract`); JSON
   `null` means unknown/absent, never 0.
2. **A pure, callable self-check path exists**: `core::golden::check(&GoldenStudy) -> GoldenReport`
   plus `check_all(&[GoldenStudy]) -> Vec<GoldenReport>` — this pair is the exact API Story 2.13
   will consume. `GoldenReport` carries at minimum the golden `id`, a `passed: bool`, and the
   deviation list (each deviation: field path, expected, actual, relative error where numeric).
   `check` runs the real pipeline `normalize(raw)` → `ssg::compute` and compares actual vs expected. **Exact (zero tolerance) for everything categorical**: present-price
   zone, zone classification, `VerdictFacts` (all four `CriterionFact`s + `quality_value_candidate`),
   quality flags (the pinned catalog strings, in raise order), findings (key + year + context),
   `low_confidence`, PTP/ROE trends, the `UpsideDownside` state (Ratio vs Undefined vs Unknown), and
   every unknown: expected `null` ⇔ actual `None`, both directions. **Derived numerics within the
   method tolerance**: `|actual − expected| ≤ golden_relative_tolerance() × |expected|` (spec §7,
   symmetric ±, relative to the EXPECTED value, all in exact `Decimal` — note the formula itself
   makes `expected == 0` demand exact equality).
   The report's deviation strings are **neutral** (no `BANNED_VERBS_EN/FR` entry), gated by a
   `core::golden`-LOCAL posture test whose exemption extends the §6 zone-label rule to zone-derived
   field-path nouns (`buy_top`, `present_price_in_buy_zone`). Do NOT add those paths to the `ssg`
   module's `engine_emitted_strings_contain_no_banned_verbs` inventory: its `contains_word` matcher
   treats `_` as a word boundary, so `"buy_top"` contains whole-word "buy" and would fail that
   suite (record this interpretation in the Task 6 issue).
   A fixture whose `meta.method_version ≠ core::METHOD_VERSION` **fails its check** (a stale golden
   must be re-validated at a method bump, never silently replayed). `core` gains NO non-dev
   dependency: `serde_json` enters as a **dev-dependency only**; callers (the test runner now, the
   2.13 UI later) own file reading and JSON parsing of the derived `Deserialize` impls.
3. **The expected block asserts the full SSG output surface**: required (must be present, `null` =
   expected unknown) — §1 sales/EPS CAGR %, quarterly sales/EPS change %, estimated high/low EPS;
   §2 5-yr avg PTP/ROE + latest-year PTP/ROE (`latest_ptp_pct`/`latest_roe_pct` — the trend's
   "recent" side and the `roe_low` input) + both trends; §3 the six averages (avg high/low/mean
   P/E, payout, high yield, low price), TTM EPS, current P/E, relative value; §4 forecast high/low,
   the four `ZoneBounds` values, present-price zone, U/D; §5 present yield, avg annual
   EPS/dividend, avg yield, projected appreciation %, projected total annualised return %; plus
   engine flags, engine findings (`CalcFinding`), `low_confidence`, verdict facts. Optional
   (omitted = not asserted, documented in the README): the §2/§3 per-year tables, and a
   `normalize_findings` list asserting the normalize-side `Finding`s (fixture (b) SHOULD assert
   it — it exists precisely to prove the pipeline runs through `normalize`).
4. **A frontier set of synthetic goldens is bundled** at `core/tests/golden/*.json` — **at least
   10 fixtures**, each with documented provenance (hand-computed independently; NEVER by pasting
   engine output back as expected — see Dev Notes circularity trap), covering at minimum:
   (a) the Story 1.8 hand-computed worked example promoted to a fixture (fullest expected coverage,
   per-year tables included); (b) a pipeline golden exercising `normalize` for real — a split event
   plus a tax-rate PTP gross-up year; (c) an all-four-criteria-Met quality-value candidate;
   (d) present price exactly on `buy_top` (closed Buy interval); (e) U/D exactly 3.0 (`≥` met,
   `ud_below_target` NOT raised); (f) relative value exactly 100 (strict `<` unmet AND
   `relative_value_high` raised); (g) a Sell-zone study with declining-trend/roe_low/eps_lags_sales
   flags firing; (h) a low-confidence study (4 usable years — computes, `low_confidence: true`);
   (i) an unknown-rich degenerate study (TTM EPS ≤ 0, sign-crossing CAGR, option (d) unselectable —
   explicit `null`s and `UnmetByInsufficiency` facts); (j) forecast-low options (b)/(c)/(d) each
   exercised (one fixture each or combined with the above). A `core/tests/golden/README.md`
   documents the fixture schema, the provenance requirement, and the authoring rules.
5. **A CI-gating runner exists** (`core/tests/golden_gate.rs`): it discovers **every** `*.json`
   under `core/tests/golden/`, **fails if fewer than the AC-4 minimum (10) are found** (an empty or
   thinned glob must never pass), fails on any parse error, runs `check` on each, and fails with
   the report's deviation list in the assert message on any deviation. It is picked up by the existing `cargo test --all --locked`
   — **no CI workflow change required** for the gate to block merge.
6. **An intentionally wrong golden provably fails the gate (no silent pass)**: negative controls
   embedded as JSON strings **inside the test file** (never as files in the fixtures directory) —
   (a) a tampered categorical (wrong zone) is reported as a deviation; (b) a numeric tampered just
   beyond +0.5% is reported, while the same value just inside the tolerance passes (the tolerance
   boundary itself is tested); (c) a stale `method_version` fails; (d) a fixture with a typo'd
   field name fails to parse (`deny_unknown_fields` proven); (e) a fixture *omitting* a required
   expected field fails to parse (the AC-1 presence-enforcement proven); (f) the `null`⇔`None` rule
   proven in both directions (expected `null` vs actual value, and expected value vs actual `None`
   — both reported as deviations).
7. **The bundled goldens are available as app assets** (ADD12, FR9 UI prep): every fixture under
   `core/tests/golden/*.json` is copied to `app/assets/golden/` (plus a short README naming
   `core/tests/golden/` as the single source of truth), and a **drift test** asserts the two
   `*.json` sets are equal and each pair byte-identical (READMEs excluded — they intentionally
   differ) — the 2.13 "verify engine" path will read these assets and call `core::golden::check`,
   nothing else.
8. **Method discipline intact, gates green**: no constant in `core::method`/`quality_flags`/
   `rounding`/`method_version` added or changed ⇒ no `METHOD_VERSION` bump;
   `method_fingerprint_is_pinned_to_version`, `determinism_hash_matches_cross_os_contract` and the
   Spike-C digest pass UNCHANGED. **No configurable-tolerance mechanism is built**: spec §7 pins
   ±0.5% as the fixed method default (`golden_relative_tolerance()`) — the PRD's "tolerance
   configurable" is superseded by the spec; a test may use a tighter local epsilon, nothing looser.
   `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`,
   `cargo test --all --locked`, `cargo deny check` all green. Every spec-underspecified
   interpretation is recorded as a **GitHub issue** (repo `guycorbaz/steadyinvest`), never an
   inline debt note.

## Tasks / Subtasks

- [x] **Task 1 — Fixture schema & parse discipline (AC: 1, 3)**
  - [x] New module `core/src/golden/` (`mod.rs` + `schema.rs`/`compare.rs` as granularity dictates;
        no `utils.rs`); `pub mod golden;` + re-exports in `core/src/lib.rs` (probe/hash untouched).
  - [x] Golden-local serde types: `GoldenStudy { meta, input, expected }`, meta per AC 1, input
        mirroring `RawFinancials`/`RawYear`/`RawAmount`/`SplitEvent` + `JudgmentInputs` (incl.
        `forecast_low_option` as a snake_case string enum mirroring the four variants) +
        `QuarterlyObservations`; all structs `deny_unknown_fields`, required expected fields with
        no `serde(default)`; conversion `impl From<…>` into the engine input types.
  - [x] Decimal-as-canonical-string serde helper using `Decimal::from_str_exact` (reject
        non-canonical spellings — same posture as `contract::Money`; cover `Option<Decimal>`,
        `Vec`, and the `[Decimal; 4]` TTM array).
  - [x] Presence-enforcing deserialization for required-nullable expected fields (AC 1 serde
        `Option` trap): omitted required field = parse error, explicit `null` = expected unknown;
        unit-test both. `provenance: String` (free text).
  - [x] Expected flags/findings as the pinned snake_case catalog strings; parse-time validation
        that each string is a member of `QUALITY_FLAGS` / a known `PlausibilityKey` (unknown
        string ⇒ parse error, not a silently never-matching expectation).
- [x] **Task 2 — Pure check & report (AC: 2)**
  - [x] `check(&GoldenStudy) -> GoldenReport`: convert input → `normalize` → resolve
        `NormalizeError` into a report failure (a golden whose input fails normalization is a
        failing golden, not a panic) → `compute` → compare per AC 2/3.
  - [x] Tolerance comparison in exact `Decimal` (`abs_diff ≤ tol × expected.abs()`); categorical
        and `Option`/unknown equality; `method_version` gate; deviation list with field path,
        expected, actual, relative error; neutral wording gated by a `core::golden`-local posture
        test (AC 2 — zone-derived field-path exemption; do NOT touch the `ssg` inventory, whose
        `contains_word` would reject `buy_top`).
  - [x] Unit tests in-module: tolerance boundary (exactly at 0.5% passes, just above fails),
        `expected == 0` ⇒ exact, `null`⇔`None` both directions, method_version mismatch.
- [x] **Task 3 — Author the frontier goldens (AC: 4)**
  - [x] Hand-compute the ≥10 fixtures of AC 4 (reuse the 1.8 worked example's independently
        hand-computed values; for new fixtures compute expected on paper/spreadsheet at full
        precision, cross-check each against a second derivation, and record both in `provenance`).
  - [x] `core/tests/golden/README.md`: schema reference, provenance rule, the
        never-paste-engine-output rule, per-year-optional semantics, how to add a golden.
- [x] **Task 4 — Runner + negative controls (AC: 5, 6)**
  - [x] `core/tests/golden_gate.rs`: glob `core/tests/golden/*.json` via
        `env!("CARGO_MANIFEST_DIR")`, assert count ≥ 1 AND ≥ the AC-4 minimum, parse-or-fail,
        check-or-fail with the full deviation list in the assert message (1-6/1-7/1-8 lesson:
        failures self-explain).
  - [x] Negative controls as embedded JSON strings in the test file (AC 6 a–f) — assert the runner
        REPORTS the deviation (the gate proves it would fail) while the suite itself stays green.
  - [x] `serde_json = { workspace = true }` under `core` `[dev-dependencies]` only.
- [x] **Task 5 — App assets + drift test (AC: 7)**
  - [x] Copy fixtures to `app/assets/golden/` + `app/assets/golden/README.md` (source of truth =
        `core/tests/golden/`; regenerate by copy, symlinks forbidden — they break on Windows).
  - [x] Drift test (in `core/tests/golden_gate.rs`, reaching the sibling crate dir via
        `CARGO_MANIFEST_DIR/../app/assets/golden`): set equality + byte identity per file.
- [x] **Task 6 — Gates, issues & status (AC: 8)**
  - [x] `cargo fmt --all --check` · `cargo clippy --all-targets --all-features --locked -- -D warnings`
        · `cargo test --all --locked` · `cargo deny check` — all green; fingerprint, determinism
        hash, Spike-C digest UNCHANGED.
  - [x] File one consolidated GitHub issue "Story 1.9 golden-fixture interpretations": stale
        `method_version` ⇒ check failure (re-validate, never replay); per-year tables optional
        while aggregates are required; `fixture_format_version` as a fourth version axis scoped to
        fixtures (not `schema_version`); app-assets = full copy (not curated subset) for v1; the
        §6 zone-label exemption extended to zone-derived field-path nouns in the golden posture
        test (`buy_top`, `present_price_in_buy_zone`).
  - [x] Update `sprint-status.yaml` (1-9 transitions) and this story's Dev Agent Record / File List.

## Dev Notes

### What this story is — and the one disaster it must not contain

The engine (1.8) and the tolerance constant (1.2) exist; this story builds the **oracle harness**
around them. The single way to ruin it: **circular goldens** — running the engine, pasting its
output into `expected`, and calling it a reference. Such a golden passes forever and can never
catch a method deviation. Every expected value must come from an **independent derivation**
(hand/spreadsheet computation, recorded in `provenance` with its cross-check). The 1.8 worked
example qualifies: its values were hand-computed first and the engine was tested against them —
promote those numbers, not the engine's output. [Source: epics.md#Story 1.9; 1-8 story AC 9]

### Architecture: two homes, one source of truth (ADD12)

The architecture mandates CI goldens in `core/tests/golden/` AND runtime bundles in
`app/assets/golden/` for the FR9 "verify engine" UI — "distinct from the CI test goldens" refers
to *role*, not necessarily content. For v1 keep them byte-identical (full copy + drift test);
curation can come later if the bundle grows. The 2.13 UI then only reads files and calls
`core::golden::check` — all comparison semantics live here, once.
[Source: architecture.md#Gap Analysis Results (ADD12); architecture.md#Complete Project Directory Structure]

### Cardinal Rule under a file-based feature

`core/src` stays I/O-free: parsing (`serde` derives) and comparison are pure; **file reading
belongs to callers** (the integration test now, the app in 2.13). `serde_json` enters `core` as a
**dev-dependency only** — the shipped engine's dependency surface is unchanged (`rust_decimal`,
`serde`, `sha2`). Do NOT add the `rust_decimal` `serde`/`serde-with-str` feature: the local
`from_str_exact` helper gives exactness + canonicality control and zero workspace-feature creep.
[Source: architecture.md#Implementation Patterns (Cardinal Rule); core/Cargo.toml]

### The fixture-strictness inversion — opposite of the journal rule

`contract` journal types forbid `deny_unknown_fields` and use `#[serde(default)]` (forward-compat
for user data). Fixtures are the opposite kind of artifact: an oracle where silence = weakness.
Hence `deny_unknown_fields` everywhere, no defaults on required expected fields, unknown
flag-strings rejected at parse. A fixture that drifts from the schema must fail loudly in CI, not
quietly assert less. [Source: architecture.md#Format Patterns; contract/src/study.rs]

### Tolerance semantics — spec §7 is already exact

`golden_relative_tolerance()` = `Decimal::new(5, 3)` (0.005) exists in `core::method` since 1.2 and
is inside the method fingerprint — **needs no addition, must not change**. The §7 formula
`|a − b| ≤ 0.005 × |expected|` natively makes `expected == 0` require exact equality (0.005 × 0 = 0)
— implement it literally, no special case, but unit-test it. Categorical outputs (zones, verdicts,
flags, trends, U/D state, unknowns) match exactly — tolerance applies ONLY to derived numerics.
The §4 zone-thirds non-termination (truncation at 28 significant digits) sits ~24 orders inside
the tolerance — hand-computed `ZoneBounds` strings at sensible precision (e.g. 6–10 significant
digits) pass comfortably; do not chase 28-digit expected values.
[Source: docs/method/ssg-method-spec-v1.md §7/§8; core/src/method/mod.rs; 1-8 story Dev Notes rounding]

### Exact engine surface to compare against (verified in code, 2026-06-11)

`core::ssg::compute(&CanonicalFinancials, &JudgmentInputs, &QuarterlyObservations) -> SsgOutputs`
with `SsgOutputs { growth, management, valuation, risk_reward, returns, quality_flags: Vec<QualityFlagKey>,
findings: Vec<CalcFinding>, low_confidence: bool, verdict_facts: VerdictFacts }`. Key categorical
types: `Zone { Buy, Neutral, Sell }`, `Trend { Up, Even, Down }`,
`UpsideDownside { Ratio(Decimal), Undefined, Unknown }` (Ratio's value is a tolerance-compared
numeric; the three-way state is exact), `CriterionFact { Met, Unmet, UnmetByInsufficiency }`,
`ZoneBounds { forecast_low, buy_top, neutral_top, forecast_high }`, `CalcFinding { key, year:
Option<i32>, context: &'static str }`, `Finding { key, year: i32, context }` (normalize-side).
Engine flags raise in pinned-catalog order — expected flag arrays compare as ordered Vec.
Input mirror: `RawFinancials { native_currency: String, years: Vec<RawYear>, splits: Vec<SplitEvent> }`;
`RawYear` = year, period_months, fiscal_year_end_month + 8 `Option<RawAmount>` fields + tax_rate;
`RawAmount { value: Decimal, currency: String }`; `SplitEvent { effective_year, numerator, denominator }`.
`JudgmentInputs` adds `projected_sales_growth_pct`/`projected_eps_growth_pct`/`recent_severe_low`/
`present_full_year_dividend` beyond `contract::Judgment` (issue #14 — the fixture schema follows the
ENGINE, not the contract). [Source: core/src/ssg/types.rs; core/src/normalize/types.rs; core/src/ssg/mod.rs]

### What ALREADY exists — consume, never re-literal, never modify

- `core::method::golden_relative_tolerance()` — THE tolerance; `METHOD_VERSION = "ssg-1.0.0"`.
- Pinned snapshots that must NOT move: method fingerprint `f79e3c11…1d1d`, determinism hash
  `eb45e761…d34f` (`core/src/lib.rs`), Spike-C digest (`core/tests/spike_c_cagr_precision.rs`).
- `QUALITY_FLAGS` / `PLAUSIBILITY_RULES` catalogs (`core/src/quality_flags.rs`) — the fixture
  schema's flag strings map onto these; catalogs must not grow.
- Test helpers/patterns to reuse in fixture authoring: `d("…")` Decimal-literal helper,
  `full_year(…)` builder, the hand-computed worked example in `core/tests/ssg_engine.rs` (5 years
  of exact 1.1-powers, every section cross-checked) — its constants are the seed of golden (a).
- `serde_json = "1"` is already a workspace dependency (used by `contract` since 1.3).
[Source: core/src/method/mod.rs; core/src/lib.rs; core/src/quality_flags.rs; core/tests/ssg_engine.rs; Cargo.toml]

### Authoring frontier goldens — boundary discipline from 1.8

The verdict-boundary fixtures (d)(e)(f) sit exactly on normative comparators: Buy interval top is
**closed** (`[low, low+third]`), `ud_at_or_above_target` is `≥ 3.0`, `relative_value_below_ceiling`
is strict `< 100` while the `relative_value_high` flag fires at `≥ 100` — at exactly 100 the
criterion is Unmet AND the flag raised (one fixture proves both). Construct boundary fixtures so
the boundary value is **exact by construction** (terminating decimals; ranges divisible by 3 where
a zone edge must land on a clean number — the 1.7/1.8 "terminating-quotient" trick), otherwise the
intended boundary hit drifts a digit off and tests the wrong side. `low_confidence` golden: 4
usable years still computes (FR8 — never a hard block). [Source: 1-8 story AC 5/7/8 + Dev Notes precision trap; core/src/ssg/risk_reward.rs]

### Scope boundaries — what 1.9 does NOT do

- **No UI**: the "verify engine" screen is Story 2.13; this story delivers its data + callable.
- **No configurable tolerance** (AC 8) and **no method change** of any kind.
- **No serde on engine/normalize types**, no `contract` dependency in `core`, no new non-dev deps.
- **No CI workflow edits**: `cargo test --all --locked` already gates; Linux-only CI stands.
- **No coverage instrumentation** (NFR-C5's ≥95% target has no tooling in CI yet — pre-existing,
  not this story's gap to close; goldens materially raise calc-path coverage regardless).
- **No persistence/provenance stamping** (goldens are stateless replays) and no demo-study asset
  (FR62's `demo_study.json` is an Epic 2 artifact — do not squat `app/assets/` with it).
- **Negative-control fixtures never live in the fixtures directories** — embedded strings only.

### Previous story intelligence (1-8 dev record + review)

- The 1.8 metamorphic suite hit the predicted precision trap (non-terminating product chains) —
  goldens dodge it via the ±0.5% tolerance, but boundary fixtures still need exact construction.
- Clippy `-D warnings` covers `--all-targets`: integration tests and helpers are linted (1.7 hit
  `redundant_closure`, 1.8 hit `double_ended_iterator_last` exactly there).
- The FR13 posture-gate string inventory (`engine_emitted_strings_contain_no_banned_verbs`) is
  hand-maintained and `ssg`-scoped — this story does NOT extend it (its `contains_word` matcher
  would reject golden field paths like `buy_top`); `core::golden` gets its own posture test per
  AC 2. Any new `&'static str` an engine-side change would add still belongs in the `ssg`
  inventory (the `CalcFinding` doc instructs this; 1.8 review finding #4) — but this story changes
  no engine strings.
- "Done" = demonstrably works: assert messages carry expected/actual/deviation so CI self-explains.
- MSRV 1.96 (`rust-toolchain.toml`; the architecture's "1.88" is stale). Gates always `--locked`.
- Issues, not inline notes: 1.7 → #12, 1.8 → #13/#14 — same pattern here (Task 6).

### Project Structure Notes

- **New:** `core/src/golden/` (`mod.rs` + `schema.rs`/`compare.rs`); `core/tests/golden/*.json`
  (≥10 fixtures) + `core/tests/golden/README.md`; `core/tests/golden_gate.rs`;
  `app/assets/golden/*.json` + `app/assets/golden/README.md` (`app/assets/` exists and is empty).
- **Modified:** `core/src/lib.rs` (add `pub mod golden;` + re-exports; probe/hash untouched);
  `core/Cargo.toml` (`[dev-dependencies] serde_json`); `_bmad-output/implementation-artifacts/
  sprint-status.yaml` (1-9 transitions).
- **Do NOT modify:** `core/src/method/` / `method_version.rs` / `quality_flags.rs` / `rounding.rs`
  (fingerprint!); `core/src/ssg/` and `core/src/normalize/` logic (1.7/1.8 are done and
  review-approved — golden mismatches mean a fixture error or a real engine bug: investigate,
  never "adjust" the engine to make a golden pass without re-deriving the expected value by hand);
  `core/src/lib.rs` probe/hash + pinned `EXPECTED`; `core/tests/spike_c_cagr_precision.rs`;
  `docs/method/ssg-method-spec-v1.md` (issues, not edits); `contract/` (`Money` stays
  contract-side); `.github/workflows/ci.yml`.
- **Naming:** snake_case modules, `PascalCase` types (`GoldenStudy`, `GoldenReport`,
  `GoldenDeviation`, …); fixture files **kebab-case** carrying the golden id (e.g.
  `g01-worked-example.json` — one convention, the drift test compares file names); no `utils.rs`.

### References

- [Source: epics.md#Story 1.9] — user story + ACs (frontier goldens, CI + callable path, exact
  zoning/verdict, ±0.5%, wrong-golden-fails, assets for FR9 UI)
- [Source: epics.md#Epic 1 "Includes:"] — "golden self-check engine"; Epic 1 closes headless with
  a CLI/test self-check, no UI
- [Source: prd.md#FR9 / NFR-C1/C2/C5] — runtime-loadable goldens; exact zoning/verdict + ±0.5%
  derived numerics; golden tests gate CI, failing test blocks merge
- [Source: prd.md#Data licensing / #IP-trademark] — fixtures are SYNTHETIC, no vendor data, no
  NAIC verbatim content; provenance documented (the "synthetic, documented provenance" AC clause)
- [Source: docs/method/ssg-method-spec-v1.md §7] — golden tolerance semantics: categorical exact,
  `|a−b| ≤ 0.005×|expected|`, fixed method default, tighter-only local epsilons
- [Source: architecture.md#Gap Analysis Results] — ADD12: bundled goldens as app assets
  (`app/assets/golden/`) + "verify engine" path, distinct from CI goldens (`core/tests/golden/`)
- [Source: architecture.md#Complete Project Directory Structure] — `core/tests/golden/` "frontier
  golden fixtures (synthetic, documented provenance)"; `app/assets/` layout
- [Source: architecture.md#Format Patterns] — JSON snake_case, Decimal-as-string, version axes
  (`schema_version` int / `method_version` string)
- [Source: core/src/method/mod.rs] — `golden_relative_tolerance()`, `METHOD_VERSION`, fingerprint
  discipline (constants frozen)
- [Source: core/src/ssg/types.rs + mod.rs; core/src/normalize/types.rs] — the exact output/input
  surface the fixture schema mirrors (verified in code, see Dev Notes)
- [Source: contract/src/money.rs] — the `from_str_exact` + canonical-string discipline the golden
  decimal helper replicates
- [Source: .github/workflows/ci.yml] — `cargo test --all --locked` already gates; determinism-hash
  step; Linux-only (2026-06-09 decision)
- [Source: 1-8-core-ssg-calculation-engine.md] — predecessor patterns: worked example to promote,
  posture-gate inventory rule, precision trap, gates `--locked`, issues #13/#14

### Tech currency note (web research consciously skipped)

No new external crate enters the workspace: `serde_json` 1 is already a pinned workspace dependency
(exercised by `contract` round-trip property tests since Story 1.3), and `rust_decimal` 1.42 was
empirically validated against this exact workload by Spike C. The only "new" usage is serde derives
on golden-local structs — standard, version-stable serde 1 surface. Web research would add nothing
the workspace hasn't already proven locally.

## Dev Agent Record

### Agent Model Used

Claude Fable 5 (claude-fable-5) via Claude Code

### Debug Log References

- First full gate run: 1 deviation on `g09-unknown-rich-degenerate.json` —
  `normalize_findings: expected [], actual [split_series_break year 2022 context eps]`.
  Re-derived by hand from the 1.7 detector rule: the 2021→2022 EPS y/y factor is
  `1.10 / −1.00 = −1.1 ≤ 0.67` while sales move +10% (not co-moving down), so the
  sign-crossing year IS an unexplained series break per the recorded interpretation.
  Fixture corrected (expected finding added + provenance updated) — the engine was right;
  no engine code touched. Recorded as point 6 of issue #15.
- All other 10 fixtures (including every hand-computed transcendental: g04/g05/g06
  annualised returns via ln/exp with 5th-power cross-checks, g07's `1.125^(1/5)`, the
  long divisions for U/D and yields) passed the ±0.5% gate on the first run.

### Completion Notes List

- **Task 1** — `core/src/golden/` (`mod.rs`, `schema.rs`, `compare.rs`; no `utils.rs`).
  Serde-strict golden-local types: `deny_unknown_fields` on every struct, zero
  `serde(default)`, presence-enforcing `deserialize_with` helpers on ALL required-nullable
  fields (the serde `Option` trap is closed: omission = parse error, explicit `null` =
  expected unknown; proven by unit test both ways). Decimals are canonical JSON strings via
  a local `Decimal::from_str_exact` + round-trip-canonicality helper (the `contract::Money`
  posture without a `contract` dependency), covering `Option<Decimal>` and the `[Decimal; 4]`
  TTM array. Expected flag/finding strings are validated against `QUALITY_FLAGS` /
  `PLAUSIBILITY_RULES` at parse time. `fixture_format_version` pinned to 1 at parse.
  `From` conversions into `RawFinancials` / `JudgmentInputs` / `QuarterlyObservations`
  (no serde added to any engine type).
- **Task 2** — `check(&GoldenStudy) -> GoldenReport` + `check_all` (the exact Story-2.13
  API): method_version gate first (stale ⇒ single-deviation failure), `NormalizeError`
  resolves to a failing report (never a panic), then `normalize` → `compute` → compare.
  Exact for everything categorical (zones, trends, U/D state, criteria, flags ordered,
  findings ordered, `low_confidence`, `null`⇔`None` both directions); numerics within
  `golden_relative_tolerance()` relative to the EXPECTED value, implemented literally so
  `expected == 0` demands exact equality (unit-tested). Deviations carry field path,
  expected, actual, relative error. Neutral wording gated by a `core::golden`-LOCAL posture
  test with the §6 exemption extended to `buy_top` / `present_price_in_buy_zone`; the `ssg`
  inventory untouched.
- **Task 3** — 11 synthetic fixtures (≥ 10) in `core/tests/golden/`, each with documented
  independent provenance and cross-checks: g01 worked example promoted (fullest coverage,
  both per-year tables); g02 pipeline golden (declared 2:1 split, `91/(1−0.30)=130`
  gross-up, EUR dividend ⇒ asserted `normalize_findings`, derived high-EPS path); g03
  candidate + option (b); g04 price exactly on `buy_top` (closed Buy interval, U/D exactly
  2); g05 U/D exactly 3.0 (`≥` met, flag quiet); g06 relative value exactly 100 (strict `<`
  unmet AND flag raised in one fixture); g07 Sell-zone with 6 flags in pinned order
  (declining trends, `roe_low`, `eps_lags_sales`); g08 low-confidence 4-year study (still
  computes, candidate true); g09 unknown-rich degenerate (TTM ≤ 0, sign-crossing CAGR,
  option (d) unselectable, `UnmetByInsufficiency` across the verdict); g10 option (c);
  g11 option (d) happy path (forecast low exactly 15). Boundary fixtures exact by
  construction (terminating decimals, ranges divisible by 3, exact powers). README with
  schema reference, provenance rule, never-paste-engine-output rule, optional-block
  semantics, add-a-golden checklist.
- **Task 4** — `core/tests/golden_gate.rs`: discovers every `*.json` via
  `CARGO_MANIFEST_DIR`, fails on empty glob AND on fewer than 10 fixtures, parse-or-fail,
  check-or-fail with the full deviation list in the assert message, file-name = `meta.id`
  convention asserted. Negative controls embedded as in-memory tampered JSON (AC 6 a–f):
  wrong zone reported; numeric at 10.06 fails while 10.05 passes (tolerance boundary at the
  gate level); stale method_version fails; typo'd field and omitted required field fail to
  parse; `null`⇔`None` proven in both directions through the full check. `serde_json`
  added under `[dev-dependencies]` only.
- **Task 5** — full byte-identical copy in `app/assets/golden/` + README naming
  `core/tests/golden/` as the single source of truth; drift test (set equality + per-file
  byte identity, READMEs excluded) in `golden_gate.rs`.
- **Task 6** — gates all green: `cargo fmt --all --check`, `cargo clippy --all-targets
  --all-features --locked -- -D warnings`, `cargo test --all --locked` (176 tests at dev
  time; 186 after the QA gap-coverage suite `golden_qa_e2e.rs` landed),
  `cargo deny check` (advisories/bans/licenses/sources ok). Method fingerprint
  `f79e3c11…1d1d`, determinism hash `eb45e761…d34f` and the Spike-C digest pass UNCHANGED —
  no constant added, no `METHOD_VERSION` bump, no configurable tolerance built. `Cargo.lock`
  delta is the single `serde_json` dev-dep edge. Consolidated interpretations filed as
  GitHub issue #15.

### File List

- `core/src/golden/mod.rs` (new)
- `core/src/golden/schema.rs` (new)
- `core/src/golden/compare.rs` (new)
- `core/src/lib.rs` (modified — `pub mod golden;` + re-exports; probe/hash untouched)
- `core/Cargo.toml` (modified — `serde_json` dev-dependency only)
- `Cargo.lock` (modified — serde_json dev-dep edge for steadyinvest-core)
- `core/tests/golden_gate.rs` (new)
- `core/tests/golden_qa_e2e.rs` (new — QA gap-coverage suite over `check`/`check_all`/`GoldenReport`, 10 tests)
- `core/tests/golden/README.md` (new)
- `core/tests/golden/g01-worked-example.json` (new)
- `core/tests/golden/g02-split-grossup-pipeline.json` (new)
- `core/tests/golden/g03-candidate-avg-low-price-option.json` (new)
- `core/tests/golden/g04-price-on-buy-top-boundary.json` (new)
- `core/tests/golden/g05-ud-exactly-at-target.json` (new)
- `core/tests/golden/g06-relative-value-at-ceiling.json` (new)
- `core/tests/golden/g07-sell-zone-flag-cluster.json` (new)
- `core/tests/golden/g08-low-confidence-four-years.json` (new)
- `core/tests/golden/g09-unknown-rich-degenerate.json` (new)
- `core/tests/golden/g10-recent-severe-low-option.json` (new)
- `core/tests/golden/g11-dividend-supported-option.json` (new)
- `app/assets/golden/README.md` (new)
- `app/assets/golden/g01-worked-example.json` (new — copy)
- `app/assets/golden/g02-split-grossup-pipeline.json` (new — copy)
- `app/assets/golden/g03-candidate-avg-low-price-option.json` (new — copy)
- `app/assets/golden/g04-price-on-buy-top-boundary.json` (new — copy)
- `app/assets/golden/g05-ud-exactly-at-target.json` (new — copy)
- `app/assets/golden/g06-relative-value-at-ceiling.json` (new — copy)
- `app/assets/golden/g07-sell-zone-flag-cluster.json` (new — copy)
- `app/assets/golden/g08-low-confidence-four-years.json` (new — copy)
- `app/assets/golden/g09-unknown-rich-degenerate.json` (new — copy)
- `app/assets/golden/g10-recent-severe-low-option.json` (new — copy)
- `app/assets/golden/g11-dividend-supported-option.json` (new — copy)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified — 1-9 transitions)
- `_bmad-output/implementation-artifacts/tests/test-summary.md` (modified — QA automation artifact)
- `_bmad-output/implementation-artifacts/1-9-golden-reference-studies-self-check-gate.md` (modified — this record)

## Senior Developer Review (AI)

**Reviewer:** Guy (autonomous review workflow) — 2026-06-12
**Outcome:** Approve — 0 Critical, 0 High, 1 Medium, 2 Low; all fixed in-review.

Adversarial validation performed against the implementation (not the story's claims):

- **AC 1–3:** `core/src/golden/` schema verified serde-strict (`deny_unknown_fields` on every
  struct, zero `serde(default)`, presence-enforcing `deserialize_with` on all
  required-nullable fields — the `Option` trap is unit-tested both ways); decimals canonical
  via `from_str_exact` with non-canonical spellings rejected; flag/finding strings validated
  against the pinned catalogs at parse time; `check`/`check_all` pure, method_version gate
  first, `NormalizeError` → failing report; §7 tolerance implemented literally
  (`expected == 0` exact, boundary inclusive at ±0.5%, both unit- and gate-tested).
- **AC 4:** 11 fixtures inspected — frontier letters (a)–(j) all covered (g01 worked example
  with both per-year tables; g02 split + 30% gross-up + EUR dividend asserting
  `normalize_findings`; g04 price exactly on `buy_top`; g05 U/D exactly 3 with `ud_below_target`
  quiet; g06 RV exactly 100 with criterion unmet AND flag raised; g07 six-flag Sell cluster;
  g08 four-year low-confidence; g09 unknown-rich; g03/g10/g11 options b/c/d). Every
  provenance documents an independent derivation + cross-check; boundary values exact by
  construction.
- **AC 5–7:** gate discovers all `*.json`, floors at 10, self-explaining deviation asserts;
  negative controls (a)–(f) embedded in-memory only; drift test enforces byte-identical
  `app/assets/golden/` copies. Verified no negative-control file lives in either fixtures dir.
- **AC 8:** all four gates re-run green during review (fmt, clippy `-D warnings`, test
  `--locked` 186 passed, deny); `core/src/method/` untouched; `Cargo.lock` delta is the single
  `serde_json` dev-dep edge; GitHub issue #15 verified (includes the g09 point 6).

Findings fixed during review:

1. **[Medium]** File List omitted `core/tests/golden_qa_e2e.rs` (QA gap-coverage suite) and
   the QA `test-summary.md` artifact → both added to the File List above.
2. **[Low]** Completion Notes' "176 tests" was stale once the QA suite landed → clarified
   (176 at dev time, 186 now).
3. **[Low]** The golden-local posture-test inventory (`compare.rs`) omitted the emitted
   per-year row paths (`management.per_year[Y].ptp_pct/roe_pct`,
   `valuation.per_year[Y].high_pe/low_pe/payout_pct/high_yield_pct`) despite its
   "extend whenever compare.rs gains a new emitted string" contract → representative
   entries added; `cargo fmt --check` + golden module tests re-run green.

## Change Log

| Date | Change |
|------|--------|
| 2026-06-11 | Story 1.9 created (ready-for-dev): golden-fixture format + pure self-check in new `core::golden` (serde-strict schema, `deny_unknown_fields`, decimals as canonical strings via `from_str_exact`, no engine-type serde, `serde_json` dev-only); ≥10 hand-computed frontier goldens in `core/tests/golden/` (worked example, split+gross-up pipeline, verdict-boundary cases at the exact comparators, low-confidence, unknown-rich, forecast-low options); CI gate `golden_gate.rs` (empty-glob guard, embedded negative controls proving no-silent-pass, tolerance-boundary tests); full copy to `app/assets/golden/` + drift test (ADD12, feeds Story 2.13); no method change — fingerprint/hashes pinned, tolerance stays the fixed §7 default. Ultimate context engine analysis completed — comprehensive developer guide created. |
| 2026-06-12 | Senior Developer Review (AI) — Approve, status → done. All 8 ACs verified against the implementation; gates re-run green (186 tests); issue #15 confirmed. 3 findings (1 Medium: File List missing `golden_qa_e2e.rs` + QA artifact; 2 Low: stale test count, posture-test inventory missing per-year row paths) — all fixed in-review. |
| 2026-06-11 | Story 1.9 implemented (review): new `core::golden` module (strict serde schema with presence-enforced nullable fields + canonical-decimal strings; pure `check`/`check_all` returning `GoldenReport` with field-path/expected/actual/relative-error deviations; method_version gate; golden-local FR13 posture test). 11 hand-computed synthetic fixtures (g01–g11, AC-4 frontier coverage a–j) in `core/tests/golden/` with documented independent provenance + README; CI gate `core/tests/golden_gate.rs` (≥10-fixture floor, parse-or-fail, self-explaining deviation asserts, AC-6 negative controls a–f embedded in-memory, app-asset drift test); byte-identical bundle in `app/assets/golden/` (ADD12). One fixture corrected during dev (g09: sign-crossing EPS year legitimately trips `split_series_break` — engine right, fixture re-derived). All gates green (fmt/clippy/test --locked 176 tests/deny); fingerprint, determinism hash, Spike-C digest unchanged; `serde_json` dev-dep only. Interpretations filed as GitHub issue #15. |

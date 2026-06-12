# Test Automation Summary — Story 1.11 (verdict-integrity & coherence invariants)

Date: 2026-06-12
Workflow: `bmad-qa-generate-e2e-tests` (auto-apply gaps mode)
Framework: Rust integration tests (`cargo test` + `proptest`) — the project's existing
framework, no new dependency. The story is headless (no UI, no HTTP surface), so "E2E"
here = workspace-level workflow tests through the full real path:
`contract::Study` → `Cell::edited` rail → glue mapping → `normalize` →
`StudySnapshot::new` (digest + compute + verdict born in one frame) → `Verdict`.

## Scope

Story 1.11 already ships a strong dev suite: verdict-module unit tests (catalog coverage,
state derivation, digest determinism, FR13 posture), `verdict_properties.rs` (2a
equivalence, single-non-green, determinism, 25-slot digest sensitivity, cross-scale digest
equality, Full-orthogonality pin), `verdict_coherence.rs` (all-green → Full, one mutation →
same-frame degradation, judgment withdrawal → Withheld, 25-way mutation proptest) and the
`contract` rail unit + property tests. QA pass = gap analysis at the **user-workflow
level**: five end-to-end journeys were not exercised anywhere. All discovered gaps were
auto-applied.

## Discovered Gaps → Generated Tests

All appended to `core/tests/verdict_coherence.rs` (QA-marked section reusing the existing
Epic-2-preview glue — zero duplication, house style):

- [x] `non_divergent_refresh_keeps_full_and_the_same_content_address` — the Epic-3
  non-divergent annual refresh was never tested e2e: a value-equal rail edit at a
  DIFFERENT `Decimal` scale (`"100.0"` over `"100"`, provider provenance) keeps ✓, keeps
  the verdict `Full`, and keeps the SAME `inputs_hash` — a no-op refresh never orphans the
  prior verdict (ADD9 invalidation is divergence-keyed).
- [x] `stale_load_bearing_cell_derives_provisional_naming_stale` — the `Stale` branch of
  the `gate_of` glue was exercised by NO test (coherence tests only hit `NotValidated` and
  `Missing`): a validated-but-stale cell derives `Provisional` and the evidence names the
  input with the `Stale` state.
- [x] `revalidation_restores_full_with_the_new_content_address` — the recovery half of the
  2b loop was untested: divergent edit → `Provisional`; explicit re-validation (FR20 user
  act) → `Full` again; and the content address follows the VALUES only (the divergent edit
  changes it, re-validation alone does not — gates are not content).
- [x] `clearing_a_load_bearing_year_cell_degrades_via_usability_and_low_confidence` — the
  §4/§5 interplay was untested e2e: `edited(None, …)` reopens the gap (`ToFill`, `ToReview`),
  makes the year unusable (§4 — it is not gated at all), drops usable years below the
  floor of 5, and the degradation arrives via the queryable FR8 `low_confidence` state
  with NO open gate — not via `Withheld`.
- [x] `multiple_degraded_inputs_are_all_named_and_missing_wins_the_split` — evidence
  accumulation was untested: one divergent year edit + one withdrawn judgment input in the
  same frame ⇒ `Withheld` (missing has precedence for the state split) and BOTH inputs are
  named in `open_gates`.

## Verification

- `cargo test -p steadyinvest-core --test verdict_coherence --locked`: **9 passed,
  0 failed** (4 pre-existing + 5 QA-generated; all green on the first run — no crate
  changes were needed).
- `cargo fmt --all` applied (two reflows), `cargo fmt --all --check`: clean.
- `cargo clippy --all-targets --all-features --locked -- -D warnings`: 0 warnings.
- `cargo test --all --locked`: **all suites green, 0 failures** — including the
  persistence pinned-JSON snapshot and frozen `corpus/v1.db` gates (no serde-shape drift)
  and the method-fingerprint / determinism-hash pins (no method change).
- Discipline intact: only `core/tests/verdict_coherence.rs` touched; fixed timestamps and
  provenance throughout (no clock/random anywhere — ADD15); shipped dependency surfaces
  unchanged.

## Coverage

- §5 degradation causes exercised e2e: 3/3 (`Missing`, `NotValidated`, and now `Stale` —
  previously 2/3).
- Verdict states reached e2e: 3/3 (`Full`, `Provisional`, `Withheld`) plus the distinct
  low-confidence cause (previously gate-driven causes only).
- Rail outcomes exercised e2e: divergent edit, non-divergent cross-scale refresh, explicit
  clear, re-validation recovery (previously divergent edit and judgment withdrawal only).
- ADD9 content address e2e: orphaning on change, stability on no-op refresh, and
  gates-are-not-content (previously orphaning only).

## Next Steps

- Nothing to wire: `cargo test --all --locked` in CI already runs the extended file.
- When Epic 2 moves the glue into `app/state.rs`, these five workflow tests are the
  ready-made acceptance set for the production mapping (same scenarios, real state layer).
- When Epic 3 lands the provider refresh, the non-divergent-refresh test is the regression
  gate for the "annual refresh must not strip ✓" rule.

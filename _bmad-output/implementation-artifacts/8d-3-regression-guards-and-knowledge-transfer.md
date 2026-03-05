# Story 8d.3: Regression Guards & Knowledge Transfer

Status: ready-for-dev

## Story

As a **developer**,
I want chart data pipeline integration tests, Cardinal Rule pattern documentation, and story sizing guidelines,
so that Epic 8c's 7 stories start with regression safety nets and dev agent context that prevents recurring mistakes.

## Background

Epic 8b retrospective identified three knowledge/quality gaps:

1. **No automated chart data pipeline regression tests** — the sort-at-source fix in `harvest.rs:121` is the most critical single line in the data pipeline, yet no test verifies records arrive in chronological order. Manual Docker walkthroughs are the only guard against chart rendering regressions.
2. **Cardinal Rule violations recur across epics** — inline `powf()` calculations caught in code review for Epics 8.2, 8.3, and 8b.1. Code review alone is insufficient after 3 consecutive occurrences. The `project_forward()` extraction helped structurally but dev agent needs explicit documentation of the pattern.
3. **Story sizing exceeded guidelines** — Story 8b.1 had 11 ACs and 11 files, requiring 3 code review rounds. The ~5 AC guideline from the retro needs documentation so future story creation (SM agent) and implementation (dev agent) stay within bounds.

## Acceptance Criteria

### AC 1: Harvest Response Record Ordering Test

**Given** the harvest endpoint at `POST /api/harvest/{ticker}`
**When** a backend integration test harvests a ticker
**Then** the test asserts that `data.records` are sorted by `fiscal_year` ascending (oldest to newest)
**And** the test asserts that each consecutive record has `fiscal_year > previous_fiscal_year`
**And** the test is added to `backend/tests/requests/harvest.rs` with `#[serial]`

### AC 2: Growth Analysis Defensive Sort & Sort-Sensitivity Test

**Given** the `calculate_growth_analysis()` function in `steady-invest-logic` currently has no defensive sort (unlike `calculate_pe_ranges()` and `calculate_quality_analysis()`)
**When** a defensive sort is added to `calculate_growth_analysis()` and a unit test exercises unsorted input
**Then** a `sort_by_key` on the input pairs (year, value) is added inside `calculate_growth_analysis()`, matching the established pattern in `calculate_pe_ranges()` (lib.rs:295) and `calculate_quality_analysis()` (lib.rs:387)
**And** a unit test `test_growth_analysis_unsorted_input` verifies that sorted and unsorted inputs produce identical results after the defensive sort
**And** the same defensive sort is applied to any other functions that depend on chronological ordering without sorting internally (audit `extract_snapshot_prices()` and `calculate_projected_trendline()`)

### AC 3: End-to-End Chart Data Pipeline Test (Including `extract_snapshot_prices`)

**Given** the full pipeline from harvest → `apply_adjustments()` → `calculate_growth_analysis()` → `extract_snapshot_prices()` → chart-ready data
**When** a unit test in `steady-invest-logic` exercises this pipeline
**Then** the test verifies:
  - Input records in chronological order produce positive CAGRs for growing data
  - After AC 2's defensive sort, reversed input records produce the same (correct) CAGRs as sorted input
  - `project_forward()` is called with the correct CAGR to produce 5-year projections
  - `extract_snapshot_prices()` produces correct target high/low prices from the pipeline CAGR (this function feeds the comparison view and history sidebar — critical path for Epic 8c)
  - The pipeline output matches the NAIC Handbook O'Hara Cruises reference values already in the test suite (reuse existing golden test data from lib.rs:1182+)

### AC 4: MEMORY.md Updated with Cardinal Rule Patterns

**Given** the project MEMORY file at `/home/gcorbaz/.claude/projects/-home-gcorbaz-synology-devel-steadyinvest/memory/MEMORY.md`
**When** the developer updates it
**Then** the following patterns are documented:
  - **Cardinal Rule**: All calculation logic in `steady-invest-logic`. Never duplicate in frontend or backend. `project_forward()` is the canonical example of extracting inline math.
  - **Sort-at-source pattern**: `harvest.rs:121` sorts records chronologically. Downstream consumers (`calculate_growth_analysis()`, `ssg_chart.rs`) depend on this ordering. Functions that tolerate unsorted input have internal defensive sorts (e.g., `calculate_pe_ranges()`, `calculate_quality_analysis()`).
  - **Golden test pattern**: Unit tests in `steady-invest-logic` validated against NAIC Handbook worked examples (O'Hara Cruises). New calculation functions must include golden tests.
  - **Acceptable display-tier arithmetic**: Simple `PE × EPS` multiplications in frontend closures (`valuation_panel.rs:45-46`) are acceptable when they delegate the core projection to `project_forward()`. Document this boundary.
**And** the MEMORY.md remains under 200 lines (the system truncation limit)

### AC 5: Story Sizing Guidelines Documented

**Given** the project MEMORY file
**When** story sizing guidelines are added
**Then** the following rules are documented:
  - Stories should not exceed ~5 acceptance criteria
  - Stories should not touch more than ~5 files
  - If a story exceeds these limits, the SM agent should split it during `create-story`
  - Infrastructure stories (CI, refactoring) may have 6 ACs if purely structural (no logic changes)
**And** a reference is added pointing to Epic 8b retro for the rationale

## Tasks / Subtasks

- [ ] Task 1: Add harvest record ordering test (AC: 1)
  - [ ] 1.1: In `backend/tests/requests/harvest.rs`, add test `harvest_returns_records_in_chronological_order`
  - [ ] 1.2: Assert records sorted ascending by `fiscal_year`: `for window in records.windows(2) { assert!(window[0].fiscal_year < window[1].fiscal_year) }`
  - [ ] 1.3: Ensure test uses `#[serial]` and follows existing test patterns

- [ ] Task 2: Add defensive sort to `calculate_growth_analysis()` and test (AC: 2)
  - [ ] 2.1: Add `sort_by_key` on input pairs inside `calculate_growth_analysis()` (matching `calculate_pe_ranges()` pattern at lib.rs:295)
  - [ ] 2.2: Audit `extract_snapshot_prices()` and `calculate_projected_trendline()` for same gap — add defensive sorts if needed
  - [ ] 2.3: In `steady-invest-logic` tests, add `test_growth_analysis_unsorted_input` — verify sorted and unsorted inputs produce identical CAGRs
  - [ ] 2.4: Verify all existing tests still pass after defensive sort addition

- [ ] Task 3: Add end-to-end chart data pipeline test including `extract_snapshot_prices` (AC: 3)
  - [ ] 3.1: In `steady-invest-logic` tests, add `test_chart_data_pipeline_end_to_end`
  - [ ] 3.2: Create a test dataset with growing values (e.g., 10% CAGR over 10 years)
  - [ ] 3.3: Verify chronological order → positive CAGR (expected)
  - [ ] 3.4: Verify reversed input produces same CAGR (defensive sort from AC 2 now normalizes)
  - [ ] 3.5: Verify `project_forward()` produces correct 5-year projection from the pipeline CAGR
  - [ ] 3.6: Verify `extract_snapshot_prices()` produces correct target high/low prices from the pipeline output
  - [ ] 3.7: Add golden test case reusing NAIC Handbook O'Hara Cruises EPS data (lib.rs:1182+ existing values)

- [ ] Task 4: Update MEMORY.md with Cardinal Rule patterns (AC: 4)
  - [ ] 4.1: Add "## Dev Agent Patterns" section to MEMORY.md
  - [ ] 4.2: Document Cardinal Rule with `project_forward()` as canonical example
  - [ ] 4.3: Document sort-at-source pattern with harvest.rs:121 reference
  - [ ] 4.4: Document golden test pattern with NAIC Handbook reference
  - [ ] 4.5: Document acceptable display-tier arithmetic boundary
  - [ ] 4.6: Ensure MEMORY.md stays under 200 lines total

- [ ] Task 5: Document story sizing guidelines (AC: 5)
  - [ ] 5.1: Add "## Story Sizing Guidelines" section to MEMORY.md
  - [ ] 5.2: Document the ~5 AC / ~5 files rule with retro reference
  - [ ] 5.3: Document infrastructure story exception (6 ACs acceptable if structural)

- [ ] Task 6: Verify all tests pass (AC: 1-3)
  - [ ] 6.1: `cargo test -p steady-invest-logic` — all existing + new tests pass
  - [ ] 6.2: `cargo test --workspace --exclude e2e-tests` — full workspace tests pass
  - [ ] 6.3: No new warnings from `cargo clippy`

## Dev Notes

### Architecture Compliance

- **Cardinal Rule preserved** — new tests exercise `steady-invest-logic` functions, not inline calculations. No new calculation logic is added outside the crate.
- **No new features** — this story adds tests and documentation only. No API changes, no schema changes, no frontend changes.
- **WASM compatibility** — new tests are `#[cfg(test)]` only; no impact on WASM compilation.

### Current Test Coverage Analysis

**What exists:**
- `backend/tests/requests/harvest.rs` — 6 tests covering harvest response format, split adjustment, currency normalization, invalid ticker. **Gap:** no record ordering assertion.
- `crates/steady-invest-logic/` — 26 unit tests + 9 doc tests = 35 total. NAIC golden tests validate O'Hara Cruises worked example. **Gap:** no sort-sensitivity test for `calculate_growth_analysis()`.
- `tests/e2e/src/` — 45 browser E2E tests across 5 epic suites. No chart data correctness tests (visual verification only).

**What this story adds:**
- 1 harvest ordering integration test (Task 1)
- 1-2 sort-sensitivity unit tests (Tasks 2-3)
- 1 pipeline golden test (Task 3)
- Total: 3-4 new tests

### Chart Data Pipeline Details

```
harvest::run_harvest()
  → generates records newest-first (loop i=1..10, year=current_year-i)
  → records.sort_by_key(|r| r.fiscal_year)     ← CRITICAL: sort-at-source (harvest.rs:121)
  → HistoricalData::apply_adjustments()         ← idempotent, no re-ordering
  → calculate_pe_ranges()                        ← internally sorts defensively
  → persist to DB (already sorted)
  → JSON response → frontend SSGChart
  → calculate_growth_analysis()                  ← NO defensive sort, depends on caller order
  → chart rendered in the order records arrive
```

**Key fragility:** `calculate_growth_analysis()` at lib.rs:684 does NOT sort its input. If records arrive out of order, CAGRs and trendlines will be silently wrong. Other functions (`calculate_pe_ranges()` at lib.rs:295, `calculate_quality_analysis()` at lib.rs:387) defensively sort internally.

### Sort Sensitivity in Functions

| Function | Defensive Sort? | Sort-Dependent? | Action |
|----------|----------------|-----------------|--------|
| `calculate_pe_ranges()` | Yes (lib.rs:295) | No — safe with any order | None |
| `calculate_quality_analysis()` | Yes (lib.rs:387) | No — safe with any order | None |
| `calculate_growth_analysis()` | **No** | **Yes** — CAGR and regression depend on chronological order | **ADD defensive sort (AC 2)** |
| `calculate_projected_trendline()` | **Audit needed** | Likely yes — calls `calculate_growth_analysis()` | **Audit in Task 2.2** |
| `extract_snapshot_prices()` | **No** | **Yes** — calls `calculate_growth_analysis()` | **Audit in Task 2.2, test in Task 3.6** |
| `project_forward()` | N/A | N/A — pure math, no sequence dependency | None |

### Cardinal Rule Compliance Audit (Current State)

**All calculations correctly delegate to `steady-invest-logic`:**
- `harvest.rs` → `calculate_pe_ranges()`, `apply_adjustments()`
- `reporting.rs` → `calculate_quality_analysis()`, `calculate_growth_analysis()`, `calculate_projected_trendline()`
- `snapshot_metrics.rs` → `extract_snapshot_prices()`, `compute_upside_downside_from_snapshot()`
- `ssg_chart.rs` → `calculate_growth_analysis()`, `calculate_projected_trendline()`
- `valuation_panel.rs` → `project_forward()`
- `analyst_hud.rs` → `calculate_growth_analysis()`, `project_forward()`
- `snapshot_hud.rs` → `calculate_growth_analysis()`, `project_forward()`

**Known acceptable pattern:** `valuation_panel.rs:45-46` has `future_high_pe.get() * projected_eps()` — this is display-tier `PE × EPS` multiplication where `projected_eps()` delegates to `project_forward()`. The multiplication itself is trivial arithmetic, not financial logic. This is the documented boundary.

### Previous Story Intelligence

From Story 8d.1 (CI Fix & Dev Environment):
- Pure infrastructure, no logic changes — same pattern here
- CI must be green first (8d.1 is prerequisite)
- 6 ACs is within the retro guideline for infrastructure stories

From Story 8d.2 (Logic Crate Modularization):
- Modularization may move tests to per-module `#[cfg(test)]` blocks
- New tests in this story should be placed in the appropriate module if 8d.2 is done first
- If 8d.2 is NOT done first, tests go in the existing `#[cfg(test)] mod tests` at end of `lib.rs`

**Dependency note:** Story 8d.3 can be implemented before or after 8d.2. If 8d.2 completes first, test placement follows the new module structure. If 8d.3 goes first, tests are added to the monolithic `lib.rs` test block and will be moved during 8d.2.

### Testing Strategy

- **New application tests:** 3-4 new tests (1 integration + 2-3 unit)
- **Validation method:** All tests pass locally + CI (if 8d.1 completed)
- **No E2E tests needed** — chart rendering correctness is covered by data pipeline unit/integration tests

### Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| `calculate_growth_analysis()` doesn't handle unsorted input gracefully | Add defensive sort (matching `calculate_pe_ranges()` pattern) or document as invariant |
| MEMORY.md exceeds 200-line limit after updates | Compact existing sections, link to topic files for details |
| Story 8d.2 changes test file locations | Test placement is flexible — works in either monolithic or modular structure |

### Project Structure Notes

| File | Action |
|------|--------|
| `backend/tests/requests/harvest.rs` | **MODIFY** — add ordering test |
| `crates/steady-invest-logic/src/lib.rs` | **MODIFY** — add sort-sensitivity and pipeline tests (or module files if 8d.2 done first) |
| `MEMORY.md` (project memory) | **MODIFY** — add Cardinal Rule patterns, sort-at-source, sizing guidelines |

### References

- [Source: _bmad-output/implementation-artifacts/epic-8b-retro-2026-02-20.md#Stories] — "8d.3: Regression Guards & Knowledge Transfer — chart data pipeline integration tests, update MEMORY.md with Cardinal Rule patterns and `project_forward()`/sort-at-source conventions, document story sizing guidelines for SM agent"
- [Source: _bmad-output/implementation-artifacts/epic-8b-retro-2026-02-20.md#Key-Insights] — "Cardinal Rule violations need structural help — code review alone isn't enough after 3 epics"
- [Source: _bmad-output/implementation-artifacts/epic-8b-retro-2026-02-20.md#Action-Items] — "Story sizing discipline — stories should not exceed ~5 acceptance criteria or ~5 files modified"
- [Source: backend/src/services/harvest.rs:121] — Sort-at-source implementation
- [Source: crates/steady-invest-logic/src/lib.rs:684] — `calculate_growth_analysis()` (no defensive sort)
- [Source: crates/steady-invest-logic/src/lib.rs:607-614] — `project_forward()` canonical extraction
- [Source: _bmad-output/planning-artifacts/architecture.md#Implementation-Patterns] — Cardinal Rule definition
- [Source: frontend/src/components/valuation_panel.rs:45-46] — Acceptable display-tier PE × EPS pattern

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (create-story workflow)

### Completion Notes List

- Story depends on 8d.1 (CI green) for CI verification but can be implemented locally in parallel
- Story can run before or after 8d.2 (modularization) — test placement adapts
- The `calculate_growth_analysis()` sort-sensitivity finding is the key technical discovery: it's the only calculation function without a defensive sort, and it's the most order-dependent
- MEMORY.md updates must be compact — current file is ~70 lines, limit is 200. Budget ~30 lines for new patterns, ~10 for sizing guidelines
- Consider creating a separate `patterns.md` topic file linked from MEMORY.md if content exceeds budget

### File List

(To be populated by dev agent during implementation)

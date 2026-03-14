# Story 8c.2: Dividend Yield, Payout Ratio & 5-Year Total Return

Status: done

## Story

As an **analyst**,
I want the system to calculate dividend yield, payout ratio, and combined total return,
so that I can evaluate the complete 5-year potential of a stock per NAIC SSG Section 5.

## Background

Story 8c.1 added `dividend_per_share` and `shares_outstanding` to `HistoricalYearlyData`. This story adds the calculation functions that consume those fields: dividend yield, payout ratio, and total return (simple + compound). These are pure logic-crate additions — no frontend, no new API endpoints, no database changes. Downstream stories (8c.3–8c.6) will display the results.

## Acceptance Criteria

### AC 1: Current + Average Yield & Total Return Calculation

**Given** a stock with dividend data available
**When** the analysis is computed
**Then** the system calculates:
  - Current Dividend Yield (5A): `dividend_per_share / current_price × 100`
  - Average Yield Over Next 5 Years (5B): average of per-year `dividend_per_share / price_high × 100` over last 5 years
  - Combined Estimated Annual Return (5C): `projected_price_appreciation_cagr + average_yield`
**And** all values are expressed as percentages
**And** stocks without dividend data return `None` (not zero)

### AC 2: Required Functions in steady-invest-logic

**Given** the `steady-invest-logic` crate
**When** dividend calculations are implemented
**Then** the following public functions exist in `calculations.rs`:
  - `calculate_dividend_yield(dividend_per_share: f64, price: f64) -> Option<f64>`
  - `calculate_payout_ratio(dividend_per_share: f64, eps: f64) -> Option<f64>`
  - `calculate_total_return_simple(price_appreciation_cagr: f64, avg_yield: f64) -> f64`
  - `calculate_total_return_compound(price_appreciation_cagr: f64, avg_yield: f64) -> f64`
**And** all functions follow the Cardinal Rule (logic crate only)
**And** all functions have `///` doc comments with `# Examples` doctests
**And** functions return `None` for invalid inputs (zero/negative price or EPS)

### AC 3: P/E History Table Columns (Section 3)

**Given** the P/E History table data (Section 3)
**When** dividend data is available for a year
**Then** the following per-year values are calculable:
  - Column F: Dividend Per Share (direct from `HistoricalYearlyData.dividend_per_share`)
  - Column G: % Payout = `(dividend_per_share / eps) × 100`
  - Column H: % High Yield = `(dividend_per_share / price_high) × 100`
**And** a new struct `DividendMetrics` holds these per-year values
**And** a function `calculate_dividend_metrics(data: &HistoricalData) -> Vec<DividendMetrics>` produces the collection

### AC 4: Golden Test Validation Against NAIC Handbook

**Given** golden test data from the NAIC Handbook
**When** tests are run
**Then** calculated dividend yield matches handbook examples within ±0.1%
**And** calculated payout ratio matches handbook examples within ±0.1%
**And** all existing 46 tests continue to pass (37 unit + 9 doc)
**And** `cargo clippy --workspace --exclude e2e-tests` passes without new warnings

## Tasks / Subtasks

- [x] Task 1: Add `DividendMetrics` struct to `types.rs` (AC: 3)
  - [x] 1.1: Add `DividendMetrics` struct with fields: `year: i32`, `dividend_per_share: Option<f64>`, `payout_ratio: Option<f64>`, `high_yield: Option<f64>`
  - [x] 1.2: Add `///` doc comment explaining NAIC Section 3 columns F, G, H
  - [x] 1.3: Run `cargo check -p steady-invest-logic` to confirm compilation

- [x] Task 2: Implement core calculation functions in `calculations.rs` (AC: 2)
  - [x] 2.1: Add `calculate_dividend_yield(dividend_per_share: f64, price: f64) -> Option<f64>` — returns `None` if price ≤ 0
  - [x] 2.2: Add `calculate_payout_ratio(dividend_per_share: f64, eps: f64) -> Option<f64>` — returns `None` if eps ≤ 0
  - [x] 2.3: Add `calculate_total_return_simple(price_appreciation_cagr: f64, avg_yield: f64) -> f64` — simple sum
  - [x] 2.4: Add `calculate_total_return_compound(price_appreciation_cagr: f64, avg_yield: f64) -> f64` — `(1 + cagr/100) × (1 + yield/100) - 1) × 100`
  - [x] 2.5: Add `///` doc comments with `# Examples` doctests for all 4 functions
  - [x] 2.6: Run `cargo test -p steady-invest-logic` to confirm doctests pass

- [x] Task 3: Implement `calculate_dividend_metrics` function (AC: 3)
  - [x] 3.1: Add `calculate_dividend_metrics(data: &HistoricalData) -> Vec<DividendMetrics>` in `calculations.rs`
  - [x] 3.2: For each record with `dividend_per_share.is_some()`, compute payout_ratio and high_yield using the functions from Task 2
  - [x] 3.3: Records without dividend data produce `DividendMetrics` with all `None` fields (except year)
  - [x] 3.4: Results sorted chronologically (oldest first, matching existing sort-at-source pattern)
  - [x] 3.5: Add `///` doc comment with `# Examples` doctest

- [x] Task 4: Add unit tests and golden tests (AC: 1, 4)
  - [x] 4.1: Add `test_dividend_yield_basic` — verify `1.25 / 100.0 = 1.25%`
  - [x] 4.2: Add `test_dividend_yield_none_for_zero_price` — verify returns `None`
  - [x] 4.3: Add `test_payout_ratio_basic` — verify `1.25 / 5.0 = 25.0%`
  - [x] 4.4: Add `test_payout_ratio_none_for_zero_eps` — verify returns `None`
  - [x] 4.5: Add `test_total_return_simple` — verify `10.0 + 2.5 = 12.5`
  - [x] 4.6: Add `test_total_return_compound` — verify `(1.10 × 1.025 - 1) × 100 = 12.75`
  - [x] 4.7: Add `test_dividend_metrics_with_mixed_data` — some years have dividends, some don't
  - [x] 4.8: Add `test_naic_handbook_dividend_yield` — golden test with handbook values
  - [x] 4.9: Run `cargo test -p steady-invest-logic` — all existing + new tests pass
  - [x] 4.10: Run `cargo clippy --workspace --exclude e2e-tests` — no new warnings

## Dev Notes

### Architecture Compliance

- **Cardinal Rule preserved** — all 4 calculation functions and `DividendMetrics` live in `steady-invest-logic`. No frontend or backend calculation logic.
- **Sort-at-source preserved** — `calculate_dividend_metrics` sorts results chronologically before returning.
- **WASM compatibility** — all types use `f64` for calculated values (same pattern as `QualityPoint`, `PeRangePoint`). Input data uses `Option<rust_decimal::Decimal>` and is converted via `.to_f64()` at calculation time.

### Critical: Input Type Convention

The existing calculation functions (`calculate_pe_ranges`, `calculate_quality_analysis`) accept `&HistoricalData` and internally extract `Decimal` fields via `.to_f64()`. The new dividend functions should follow the **same pattern**:
- `calculate_dividend_yield` and `calculate_payout_ratio` take `f64` parameters (already converted)
- `calculate_dividend_metrics` takes `&HistoricalData` and handles Decimal→f64 conversion internally
- This matches `calculate_growth_analysis(years: &[i32], values: &[f64])` pattern

### NAIC Section 5 Formulas

Per the NAIC SSG Handbook:
- **5A Current Dividend Yield** = (Annual Dividend Per Share / Current Price) × 100
- **5B Average Yield Over Next 5 Years** = Average of historical `dividend_per_share / price_high` × 100 over last 5 years (same 5-year window as P/E analysis)
- **5C Combined Estimated Annual Return** = Price Appreciation CAGR + Average Yield (simple addition for simple return; compound formula for compound return)

### NAIC Section 3 Columns (P/E History Table)

The P/E History table gains three dividend-related columns:
- **Column F: Dividend Per Share** — directly from `record.dividend_per_share`
- **Column G: % Payout** = `(dividend_per_share / eps) × 100`
- **Column H: % High Yield** = `(dividend_per_share / price_high) × 100`

These are computed per-year for each historical record.

### Handling Missing Dividend Data

- Stocks without dividends have `dividend_per_share: None` (established in 8c.1, conditional on ticker)
- All calculation functions must gracefully handle `None` inputs:
  - `calculate_dividend_yield` / `calculate_payout_ratio` → return `None`
  - `calculate_dividend_metrics` → returns `DividendMetrics` with `None` for all computed fields
  - `calculate_total_return_*` → when avg_yield is 0.0, return just the price appreciation component

### Existing Calculation Pattern to Follow

Reference: `calculate_quality_analysis` at `calculations.rs:92` — this function:
1. Takes `&HistoricalData`
2. Sorts records chronologically
3. Iterates records extracting `Decimal` fields via `.to_f64()`
4. Handles `None` optional fields gracefully
5. Returns a collection struct with per-year data points

Follow this exact pattern for `calculate_dividend_metrics`.

### New Types Location

Add `DividendMetrics` to `types.rs` alongside `QualityPoint` and `PeRangePoint` — these are all per-year analysis structs:

```rust
/// Per-year dividend metrics for NAIC SSG Section 3 P/E History table.
///
/// Corresponds to columns F (Dividend Per Share), G (% Payout),
/// and H (% High Yield) in the P/E History table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DividendMetrics {
    /// Fiscal year of the data point.
    pub year: i32,
    /// Annual dividend per share (Column F).
    pub dividend_per_share: Option<f64>,
    /// Payout ratio as percentage: (DPS / EPS) × 100 (Column G).
    pub payout_ratio: Option<f64>,
    /// High yield as percentage: (DPS / Price High) × 100 (Column H).
    pub high_yield: Option<f64>,
}
```

### What This Story Does NOT Do

- **No frontend changes** — UI display of dividend metrics comes in 8c.4a/8c.4b
- **No new API endpoints** — these are logic crate functions only
- **No database changes** — data model was expanded in 8c.1
- **No P/E breakdown changes** — 5-tier P/E breakdown is 8c.3's scope
- **No comparison grid changes** — enriched comparison is 8c.6's scope

### Previous Story Intelligence

**From Story 8c.1 (completed):**
- `dividend_per_share` and `shares_outstanding` are `Option<rust_decimal::Decimal>` in `HistoricalYearlyData`
- Not all tickers have dividend data — AAPL/MSFT/NESN.SW get mock dividends, others get `None`
- Split adjustment and currency normalization handle both new fields (fixed in code review)
- 46 tests pass (37 unit + 9 doc), clippy clean

**From 8c.1 Code Review (critical learnings):**
- New monetary fields MUST be handled in `apply_normalization()` and `apply_adjustments()` — the code review caught that these were missing
- Doc comments must enumerate all fields they process
- Mock data should realistically model missing data scenarios (not all tickers have all fields)

**From Epic 8d (completed):**
- Logic crate is modularized: `adjustments`, `calculations`, `currency`, `projections`, `types`
- All public functions re-exported from `lib.rs` via `pub use calculations::*;` etc.
- Defensive sorts established in `calculate_growth_analysis()`

**From Epic 8b retro:**
- Story sizing: 4 ACs and ~5 files — within guidelines
- Golden tests must validate against NAIC Handbook values
- Cardinal Rule is non-negotiable

### Git Intelligence

Recent commits follow `feat:`, `fix:`, `review:` prefix pattern. All include `Co-Authored-By` trailer. Current test baseline: 46 tests (37 unit + 9 doc).

### Project Structure Notes

| File | Action | Module |
|------|--------|--------|
| `crates/steady-invest-logic/src/types.rs` | MODIFY | Add `DividendMetrics` struct |
| `crates/steady-invest-logic/src/calculations.rs` | MODIFY | Add 5 functions + tests |

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Epic-8c-Story-8c.2] — BDD acceptance criteria
- [Source: _bmad-output/planning-artifacts/architecture.md#NAIC-Methodology-Expansion] — 4 dividend/return functions spec
- [Source: _bmad-output/planning-artifacts/prd.md#FR2.7] — Dividend yield, payout ratio, total return requirement
- [Source: crates/steady-invest-logic/src/types.rs:55-58] — Current dividend_per_share and shares_outstanding fields
- [Source: crates/steady-invest-logic/src/calculations.rs:92-180] — calculate_quality_analysis pattern to follow
- [Source: crates/steady-invest-logic/src/calculations.rs:41-90] — calculate_pe_ranges (5-year window pattern)
- [Source: _bmad-output/implementation-artifacts/8c-1-data-model-expansion-dividend-harvest.md] — Previous story with review learnings

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (dev-story workflow)

### Implementation Plan

- Task 1: Added `DividendMetrics` struct to `types.rs` with NAIC Section 3 column doc comments
- Task 2: Implemented 4 core calculation functions (`calculate_dividend_yield`, `calculate_payout_ratio`, `calculate_total_return_simple`, `calculate_total_return_compound`) with full `///` doc comments and `# Examples` doctests
- Task 3: Implemented `calculate_dividend_metrics` aggregation function following `calculate_quality_analysis` pattern (clone, sort, iterate, convert Decimal→f64)
- Task 4: Added 8 unit tests + 1 golden test using NAIC Handbook O'Hara Cruises data

### Completion Notes List

- Only 2 source files modified — focused logic-crate-only story. Cardinal Rule preserved.
- `calculate_dividend_metrics` follows the exact same pattern as `calculate_quality_analysis`: clone records, sort chronologically, iterate with Decimal→f64 conversion, handle None gracefully.
- `calculate_total_return_compound` uses geometric formula `((1 + r1)(1 + r2) - 1) × 100` — mathematically correct for independent return sources.
- Golden test validates O'Hara Cruises handbook values: $0.84 DPS / $149.83 current price = 0.56% yield, $0.84 / $165.00 year high = 0.51% high yield, $0.84 / $5.71 EPS = 14.7% payout.
- Test count: 48 unit + 15 doc = 63 total (was 37 unit + 9 doc = 46). All pass, clippy clean.
- `shares_outstanding` NOT used in this story — reserved for 8c.6 comparison metrics.

### Senior Developer Review (AI)

**Review Date:** 2026-03-14
**Review Outcome:** Approve (after fixes)
**Action Items:** 5 total (3 Medium, 2 Low) — all fixed

- [x] [Med] Function ordering: reordered so 4 core AC 2 functions are contiguous
- [x] [Med] Added `calculate_average_yield_5year` for NAIC 5B (Cardinal Rule compliance)
- [x] [Med] Golden test now uses distinct prices for current yield (5A) vs high yield (Column H)
- [x] [Low] Added doc note about negative DPS behavior to `calculate_dividend_yield`
- [x] [Low] Added `# Returns` section to `calculate_dividend_metrics` doc comment

### File List

- `crates/steady-invest-logic/src/types.rs` — MODIFIED: Added `DividendMetrics` struct
- `crates/steady-invest-logic/src/calculations.rs` — MODIFIED: Added 6 public functions (`calculate_dividend_yield`, `calculate_payout_ratio`, `calculate_total_return_simple`, `calculate_total_return_compound`, `calculate_dividend_metrics`, `calculate_average_yield_5year`) + 11 unit tests + 1 golden test (with 5 new doctests)

### Change Log

- 2026-03-14: Implemented dividend yield, payout ratio, total return calculations and DividendMetrics aggregation (Story 8c.2)
- 2026-03-14: Code review fixes — reordered functions, added 5B average yield function, improved golden test, doc improvements

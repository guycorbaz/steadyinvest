# Story 8c.1: Data Model Expansion & Dividend Harvest

Status: done

## Story

As an **analyst**,
I want dividend and shares outstanding data available for my analyses,
so that I can compute yield, payout ratio, and total return per the NAIC methodology.

## Background

Epic 8c extends the NAIC analytical framework with dividend/yield calculations, structured tests, and NAIC-standard visual presentation. This first story lays the data foundation — expanding the shared data model and harvest pipeline so that downstream stories (8c.2–8c.6) can compute dividend yield, payout ratio, total return, and enriched comparison metrics.

The `HistoricalYearlyData` struct in `steady-invest-logic` currently has no dividend or shares outstanding fields. The harvest pipeline populates 10 years of financial data but skips dividend information. This story adds two optional fields, extends the pipeline, creates the database migration, and validates backward compatibility.

## Acceptance Criteria

### AC 1: Data Model Expanded with Dividend Fields

**Given** the `HistoricalYearlyData` struct in `crates/steady-invest-logic/src/types.rs`
**When** the data model is expanded
**Then** it includes:
  - `dividend_per_share: Option<rust_decimal::Decimal>` — annual dividend per share
  - `shares_outstanding: Option<rust_decimal::Decimal>` — total shares outstanding
**And** both fields have `/// ` doc comments explaining their NAIC purpose
**And** both fields default to `None` via the existing `#[derive(Default)]`
**And** existing data without dividends continues to deserialize/work without modification (backward compatible)

### AC 2: Harvest Pipeline Populates Dividend Fields

**Given** the harvest service at `backend/src/services/harvest.rs`
**When** a ticker is harvested via `POST /api/harvest/{ticker}`
**Then** the response includes `dividend_per_share` and `shares_outstanding` for each year
**And** mock data follows the established synthetic pattern (growing values across years)
**And** override support is added for both new fields (match arms in the override loop)

### AC 3: Database Migration Adds Nullable Columns

**Given** the `historicals` table in MariaDB
**When** the new migration runs
**Then** two nullable `DECIMAL(19,4)` columns are added: `dividend_per_share` and `shares_outstanding`
**And** existing rows get `NULL` values (backward compatible)
**And** the SeaORM entity model is updated to include the new columns
**And** the persist logic in `harvest.rs` saves the new fields to the database

### AC 4: All Existing Tests Pass and New Tests Added

**Given** the expanded data model
**When** `cargo test -p steady-invest-logic` is run
**Then** all 41 existing tests pass (32 unit + 9 doc — includes restored golden test from 8d.2 review)
**And** new unit tests validate:
  - `HistoricalYearlyData` defaults with `None` dividend fields (backward compat)
  - Serialization round-trip with and without dividend data
  - Deserialization of legacy JSON (no dividend fields) produces `None` values
**And** `cargo clippy --workspace` passes without new warnings

## Tasks / Subtasks

- [x] Task 1: Expand `HistoricalYearlyData` in `types.rs` (AC: 1)
  - [x] 1.1: Add `dividend_per_share: Option<rust_decimal::Decimal>` with doc comment
  - [x] 1.2: Add `shares_outstanding: Option<rust_decimal::Decimal>` with doc comment
  - [x] 1.3: Verify `Default` derive still works (Option fields default to None)
  - [x] 1.4: Run `cargo check -p steady-invest-logic` to confirm no downstream breakage

- [x] Task 2: Create database migration (AC: 3)
  - [x] 2.1: Create new migration file (e.g., `m20260314_000001_add_dividend_fields_to_historicals.rs`)
  - [x] 2.2: Add `dividend_per_share` and `shares_outstanding` as nullable `Decimal(19,4)` columns to `historicals`
  - [x] 2.3: Register migration in `backend/migration/src/lib.rs`
  - [x] 2.4: Update SeaORM entity in `backend/src/models/_entities/historicals.rs` to include new columns

- [x] Task 3: Expand harvest pipeline (AC: 2, 3)
  - [x] 3.1: In `harvest.rs` record construction (line 75-89), add mock `dividend_per_share` and `shares_outstanding` values
  - [x] 3.2: Add override match arms for `"dividend_per_share"` and `"shares_outstanding"` (line 99-107)
  - [x] 3.3: Update persist logic to save new fields to DB
  - [x] 3.4: Verify the sort-at-source pattern (line 123) is not affected

- [x] Task 4: Add backward compatibility and golden tests (AC: 4)
  - [x] 4.1: In `types.rs` tests or `calculations.rs` tests, add `test_historical_yearly_data_defaults_dividend_fields`
  - [x] 4.2: Add `test_deserialization_without_dividend_fields` — verify legacy JSON without dividend keys produces `None`
  - [x] 4.3: Add `test_serialization_with_dividend_data` — verify round-trip with populated dividend fields
  - [x] 4.4: Run `cargo test -p steady-invest-logic` — all existing + new tests pass
  - [x] 4.5: Run `cargo clippy --workspace --exclude e2e-tests` — no new warnings

## Dev Notes

### Architecture Compliance

- **Cardinal Rule preserved** — new fields are in `steady-invest-logic::types::HistoricalYearlyData`. No calculation logic added (that's 8c.2's scope). Only data model expansion.
- **Sort-at-source preserved** — no changes to `harvest.rs:123` sort logic. New fields are per-record data, not ordering-sensitive.
- **WASM compatibility** — `Option<rust_decimal::Decimal>` is WASM-safe (already used for `net_income`, `pretax_income`, `total_equity`).

### Critical: Use `Option<rust_decimal::Decimal>`, NOT `Option<f64>`

The architecture doc says `Option<f64>` but the **actual codebase** uses `Option<rust_decimal::Decimal>` for all optional financial fields (`net_income`, `pretax_income`, `total_equity`, `exchange_rate`). Follow the established code pattern, not the architecture doc. The `f64` conversion happens at calculation time via `to_f64()`.

### Current `HistoricalYearlyData` Fields (from `types.rs:35-59`)

```rust
pub struct HistoricalYearlyData {
    pub fiscal_year: i32,
    pub sales: rust_decimal::Decimal,
    pub eps: rust_decimal::Decimal,
    pub price_high: rust_decimal::Decimal,
    pub price_low: rust_decimal::Decimal,
    pub net_income: Option<rust_decimal::Decimal>,       // ← follow this pattern
    pub pretax_income: Option<rust_decimal::Decimal>,     // ← follow this pattern
    pub total_equity: Option<rust_decimal::Decimal>,      // ← follow this pattern
    pub adjustment_factor: rust_decimal::Decimal,
    pub exchange_rate: Option<rust_decimal::Decimal>,
    #[serde(default)]
    pub overrides: Vec<ManualOverride>,
    // ADD HERE:
    // pub dividend_per_share: Option<rust_decimal::Decimal>,
    // pub shares_outstanding: Option<rust_decimal::Decimal>,
}
```

### Harvest Service Mock Data Pattern

The harvest service (line 75-89) constructs synthetic records with growing values. Follow the same pattern for dividend data:
- `dividend_per_share`: e.g., `Some(Decimal::from_f32(0.5 + (11 - i) as f32 * 0.1).unwrap().round_dp(2))`
- `shares_outstanding`: e.g., `Some(Decimal::from(1_000_000 + (11 - i) * 50_000))`

Some tickers may not pay dividends — consider leaving `dividend_per_share` as `None` for non-dividend tickers (e.g., growth stocks). The mock can set it for AAPL and leave it `None` for tickers that don't pay dividends.

### Migration Pattern (follow existing migrations)

Reference: `m20260207_101051_add_quality_fields_to_historicals.rs` — this migration added `net_income`, `pretax_income`, `total_equity` as nullable decimals. Copy that exact pattern for the new fields.

```rust
// Pattern from existing migration:
manager.alter_table(
    Table::alter()
        .table(Historicals::Table)
        .add_column(ColumnDef::new(Historicals::DividendPerShare).decimal_len(19, 4).null())
        .add_column(ColumnDef::new(Historicals::SharesOutstanding).decimal_len(19, 4).null())
        .to_owned(),
).await
```

### SeaORM Entity Update

After creating the migration, update `backend/src/models/_entities/historicals.rs` to add the new columns. Follow the pattern of existing optional decimal fields:
```rust
pub dividend_per_share: Option<Decimal>,
pub shares_outstanding: Option<Decimal>,
```

### Persist Logic

In `harvest.rs` (around line 143-169), the persist loop saves records to the database. The new fields must be included in the `ActiveModel` construction. Follow the pattern used for `net_income`:
```rust
net_income: Set(record.net_income),  // existing pattern
dividend_per_share: Set(record.dividend_per_share),  // add this
shares_outstanding: Set(record.shares_outstanding),  // add this
```

### Backward Compatibility via `#[serde(default)]`

The `HistoricalYearlyData` struct uses `#[derive(Deserialize)]`. New `Option<T>` fields automatically deserialize as `None` when missing from JSON, thanks to serde's default behavior for `Option`. No `#[serde(default)]` annotation needed on individual `Option` fields. Verify this with a deserialization test.

### What This Story Does NOT Do

- **No dividend calculation functions** — `calculate_dividend_yield()`, `calculate_payout_ratio()` come in Story 8c.2
- **No frontend changes** — data model only; UI display comes in 8c.4a
- **No new API endpoints** — uses existing harvest endpoint
- **No data gap UI indicators** — the AC mentions displaying data gaps, but the frontend data gap indicator is a 8c.4a/UI concern. This story ensures the fields exist and are `None` when unavailable.

### Previous Story Intelligence

**From Epic 8d (completed):**
- Logic crate is now modularized into 5 modules — new fields go in `types.rs`, new tests in `types.rs` or `calculations.rs`
- All 41 tests pass (32 unit + 9 doc)
- Defensive sorts established in `calculate_growth_analysis()` — new dividend data doesn't affect ordering
- CI is green, Rust 1.93.0 pinned

**From Epic 8b retro:**
- Story sizing: 4 ACs and ~5 files — within guidelines
- Golden tests must validate against NAIC Handbook when applicable (no handbook reference values for dividend fields specifically, but backward compat tests are essential)
- Cardinal Rule: no calculation logic outside `steady-invest-logic`

### Git Intelligence

Recent commits show the team is in infrastructure/review mode. Story 8c.1 transitions back to feature development. Key patterns from recent work:
- Commit messages follow: `feat:`, `fix:`, `review:` prefixes
- All commits include `Co-Authored-By` trailer
- Tests are validated before committing

### Project Structure Notes

| File | Action | Module |
|------|--------|--------|
| `crates/steady-invest-logic/src/types.rs` | MODIFY | Add 2 fields to `HistoricalYearlyData` |
| `backend/src/services/harvest.rs` | MODIFY | Add mock data + override arms + persist |
| `backend/migration/src/m20260314_000001_add_dividend_fields.rs` | NEW | Migration for 2 columns |
| `backend/migration/src/lib.rs` | MODIFY | Register new migration |
| `backend/src/models/_entities/historicals.rs` | MODIFY | Add 2 entity fields |

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Epic-8c-Story-8c.1] — Story spec with BDD acceptance criteria
- [Source: _bmad-output/planning-artifacts/architecture.md#HistoricalYearlyData-Expansion] — Data model expansion spec
- [Source: _bmad-output/planning-artifacts/architecture.md#NAIC-Methodology-Expansion] — ~9 new functions planned for 8c
- [Source: _bmad-output/planning-artifacts/architecture.md#Cardinal-Rule] — All calculation logic in steady-invest-logic
- [Source: backend/migration/src/m20260207_101051_add_quality_fields_to_historicals.rs] — Migration pattern to follow
- [Source: crates/steady-invest-logic/src/types.rs:35-59] — Current HistoricalYearlyData struct
- [Source: backend/src/services/harvest.rs:75-89] — Record construction to expand
- [Source: backend/src/services/harvest.rs:99-107] — Override match arms to extend
- [Source: _bmad-output/implementation-artifacts/epic-8b-retro-2026-02-20.md] — Cardinal Rule, golden test, sizing lessons

## Senior Developer Review (AI)

- **Review Date:** 2026-03-14
- **Reviewer:** Claude Opus 4.6 (code-review workflow)
- **Outcome:** Approve (after fixes)

### Action Items

- [x] [HIGH] `apply_normalization()` missing `dividend_per_share` conversion — foreign ticker DPS would remain in native currency [`adjustments.rs:31-54`]
- [x] [HIGH] `apply_adjustments()` missing `dividend_per_share` split adjustment — historical dividends incorrect for split stocks [`adjustments.rs:11-24`]
- [x] [MED] `shares_outstanding` not split-adjusted in `apply_adjustments()` — pre-split share counts not comparable [`adjustments.rs:11-24`]
- [x] [MED] All tickers get mock dividend data — should be `None` for non-dividend tickers [`harvest.rs:88-93`]
- [x] [MED] Doc comments on `apply_normalization()` and `apply_adjustments()` don't list new fields [`adjustments.rs:4-7,26-29`]
- [ ] [LOW] Architecture doc says `Option<f64>` for dividend fields, code uses `Option<Decimal>` — doc update needed [`architecture.md:175`]

**Summary:** 5 issues fixed automatically, 1 low-severity deferred. Added 2 new tests for adjustment correctness.

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (create-story workflow)

### Completion Notes List

- Architecture doc says `Option<f64>` for dividend fields but codebase consistently uses `Option<rust_decimal::Decimal>` for all optional financial data. Use `Decimal` to match established pattern.
- The harvest service uses synthetic mock data (not a real API). Adding dividend mock data is straightforward — follow the growing-values pattern.
- `shares_outstanding` uses `Decimal(19,4)` even though it's a whole number — this matches the DB column type consistency.
- Story 8c.2 depends on these fields being available. The `calculate_dividend_yield()` function will use `dividend_per_share.to_f64()` at calculation time.
- Override system already handles Optional fields via `Some(ovr.value)` — add two more match arms.
- MEMORY.md patterns (Cardinal Rule, sort-at-source, golden tests) apply — this story is data-only, no calculation logic.
- The `HistoricalData` struct has `is_split_adjusted` and `pe_range_analysis` fields. The `HistoricalYearlyData` struct is what gets expanded — don't confuse the two.

### Implementation Plan

1. Added `dividend_per_share` and `shares_outstanding` as `Option<rust_decimal::Decimal>` to `HistoricalYearlyData` in `types.rs`, following the established pattern for optional financial fields.
2. Created migration `m20260314_000001_add_dividend_fields_to_historicals.rs` adding two nullable `DECIMAL(19,4)` columns, following the quality-fields migration pattern.
3. Updated SeaORM entity and harvest pipeline (mock data, overrides, persist logic).
4. Added 3 new unit tests in `types.rs` for backward compatibility and serialization round-trip.
5. Fixed `adjustments.rs` test that used explicit field listing (added new fields to prevent compilation error).

### File List

| File | Action |
|------|--------|
| `crates/steady-invest-logic/src/types.rs` | MODIFIED — added 2 fields + 3 tests |
| `crates/steady-invest-logic/src/adjustments.rs` | MODIFIED — split/normalization for new fields + 2 tests |
| `backend/migration/src/m20260314_000001_add_dividend_fields_to_historicals.rs` | NEW — migration for 2 columns |
| `backend/migration/src/lib.rs` | MODIFIED — registered new migration |
| `backend/src/models/_entities/historicals.rs` | MODIFIED — added 2 entity fields |
| `backend/src/services/harvest.rs` | MODIFIED — mock data (conditional dividends), overrides, persist |

### Change Log

- 2026-03-14: Story 8c.1 implementation — expanded data model with dividend_per_share and shares_outstanding fields, created DB migration, updated harvest pipeline, added 3 backward-compatibility tests. All 44 logic crate tests pass (35 unit + 9 doc). Clippy clean.
- 2026-03-14: Code review fixes — (H1) added dividend_per_share to apply_normalization, (H2) added dividend_per_share to apply_adjustments, (M1) added shares_outstanding to apply_adjustments, (M2) made dividend mock conditional on ticker, (M3) updated doc comments. Added 2 new adjustment tests. All 46 tests pass (37 unit + 9 doc). Clippy clean.

# Story 1.4: Historical Split and Dividend Adjustment

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a Value Hunter,
I want all historical prices to be automatically adjusted for stock splits and dividends,
so that my growth charts reflect real economic performance without artificial distortions.

## Acceptance Criteria

1. [x] **Given** raw historical data has been fetched [Source: epics.md#L132]
2. [x] **When** the system identifies historical stock splits or significant dividends [Source: epics.md#L133]
3. [x] **Then** it must apply back-adjustment to all Price and EPS figures prior to the event [Source: epics.md#L134]
4. [x] **And** the UI must show a "Split-Adjusted" indicator for the data set [Source: epics.md#L135]

## Tasks / Subtasks

- [x] Update `HistoricalYearlyData` and `HistoricalData` in `naic-logic` (AC: 4)
  - [x] Add `is_split_adjusted` field to `HistoricalData`.
  - [x] Add `adjustment_factor` to `HistoricalYearlyData` (internal use).
- [x] Implement Adjustment Logic in `naic-logic` (AC: 3)
  - [x] Create a utility function to apply split factors to a series of records.
- [x] Update Backend `Harvest` Service (AC: 2, 3)
  - [x] [Mock] Simulate split detection (e.g., if ticker is "AAPL", simulate a 4:1 split).
  - [x] Apply the adjustment logic before persisting to DB or returning to frontend.
- [x] Frontend: Display Adjustment Status (AC: 4)
  - [x] Add a "Split-Adjusted" badge/indicator near the ticker info or data grid.
- [x] Verify Adjustment Correctness (AC: 3)
  - [x] Unit test in `naic-logic` for the adjustment math.
  - [x] Integration test/E2E check for the "Split-Adjusted" UI indicator.

## Dev Notes

- **Adjustment Math**: For a `N:M` split on date `D`, multiply all prices and EPS *before* `D` by `M/N`.
- **MVP Strategy**: Since Story 1.3 uses mocked data for reliability, Story 1.4 should implement the *logic* for adjustment and trigger it via mock signals until the real Yahoo Finance integration is finalized.
- **Source Paths**:
  - Logic: `crates/naic-logic/src/lib.rs`
  - Backend: `backend/src/services/harvest.rs`
  - Frontend: `frontend/src/pages/home.rs` (or dedicated component)

### Project Structure Notes

- Keep adjustment math in `naic-logic` to ensure consistency between backend (storage) and frontend (rendering).

### References

- [Epics: Story 1.4](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/epics.md#L124)
- [Architecture: Domain Logic](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L119)

## Dev Agent Record

### Agent Model Used

Antigravity (BMad Edition)

### Completion Notes List

- Applied ADVERSARIAL Code Review findings:
  - Added intermediate DB migration to persist `is_split_adjusted` and `adjustment_factor`.
  - Removed intermediate rounding in `naic-logic` to preserve decimal precision during adjustment.
  - Added E2E test `test_split_adjustment_indicator` to verify UI badge visibility.
- Verified that "Split-Adjusted" badge only appears when adjustments are actually applied (e.g., AAPL mock).

### File List

- [lib.rs](file:///home/gcorbaz/synology/devel/naic/crates/naic-logic/src/lib.rs)
- [harvest.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/services/harvest.rs)
- [m20260207_000158_add_adjustment_metadata_to_historicals.rs](file:///home/gcorbaz/synology/devel/naic/backend/migration/src/m20260207_000158_add_adjustment_metadata_to_historicals.rs)
- [historicals.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/_entities/historicals.rs)
- [home.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/pages/home.rs)
- [lib.rs](file:///home/gcorbaz/synology/devel/naic/tests/e2e/src/lib.rs)

# Story 2.3: Quality Dashboard (ROE & Profit on Sales)

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a Value Hunter,
I want a dedicated table showing 10-year trends for Pre-tax Profit on Sales and Return on Equity (ROE),
so that I can verify the company's operational efficiency and management quality.

## Acceptance Criteria

1. **High-Density Grid**: Monospace grid using JetBrains Mono for all numerical data.
2. **10-Year Data**: Display 10 fiscal years of ROE (%) and Pre-tax Profit on Sales (%).
3. **Trend Highlighting**: Visual indicators (heat-mapped or arrows) for year-over-year growth/decline.
4. **Data Model Extension**: Database schema and data models must support the required underlying fields (Pre-tax Profit, Net Income, Equity).
5. **Logic Isolation**: Ratio calculations must be implemented in the shared `steady-invest-logic` crate.

## Tasks / Subtasks

- [x] **Data Model Extension** (AC: #4)
  - [x] Create migration to add `pretax_income`, `net_income`, and `total_equity` to `historicals` table.
  - [x] Update `steady-invest-logic` `HistoricalYearlyData` and backend `historicals` model.
  - [x] Update `Seed` task to include sample data for these new fields.
- [x] **Business Logic** (AC: #5)
  - [x] Implement `calculate_quality_analysis` in `steady-invest-logic` to compute % ratios and YoY trends.
  - [x] Add unit tests for ratio accuracy.
- [x] **UI Implementation** (AC: #1, #2, #3)
  - [x] Create `QualityDashboard` component in `frontend/src/components/`.
  - [x] Implement JetBrains Mono styling for the grid.
  - [x] Implement color logic for trend highlighting (Emerald/Crimson).
  - [x] Integrate into the main Analysis page below/beside the SSG Chart.

## Dev Notes

- **Formulas**:
  - ROE = (Net Income / Total Equity) * 100
  - Pre-tax Profit on Sales = (Pre-tax Income / Sales) * 100
- **Aesthetic**: Follow "Institutional HUD" (Deep black background, no rounding).
- **Performance**: Ensure WASM re-renders are <100ms for the entire dashboard.

### Project Structure Notes

- Data logic stays in `crates/steady-invest-logic`.
- Frontend components in `frontend/src/components/`.

### References

- [Epic 2: Kinetic SSG Visualization](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/epics.md#Epic%202:%20Kinetic%20SSG%20Visualization%20(Core%20Analysis))
- [UX: Financial Data Grid](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/ux-design-specification.md#Typography%20System)

## Dev Agent Record

### Agent Model Used

Antigravity (BMad Edition) - 2026-02-07

### File List

- [crates/steady-invest-logic/src/lib.rs](file:///home/gcorbaz/synology/devel/naic/crates/steady-invest-logic/src/lib.rs)
- [backend/src/models/_entities/historicals.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/_entities/historicals.rs)
- [backend/migration/src/m20260207_101051_add_quality_fields_to_historicals.rs](file:///home/gcorbaz/synology/devel/naic/backend/migration/src/m20260207_101051_add_quality_fields_to_historicals.rs)
- [backend/src/services/harvest.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/services/harvest.rs)
- [frontend/src/components/quality_dashboard.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/quality_dashboard.rs)
- [frontend/src/components/mod.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/mod.rs)
- [frontend/src/pages/home.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/pages/home.rs)
- [frontend/public/styles.scss](file:///home/gcorbaz/synology/devel/naic/frontend/public/styles.scss)

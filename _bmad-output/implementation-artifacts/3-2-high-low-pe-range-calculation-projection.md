# Story 3.2: High/Low P/E Range Calculation & Projection

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a Value Hunter,
I want the system to calculate historical P/E ranges and allow me to project a future "Average High" and "Average Low" P/E,
so that I can establish a reasonable valuation floor and ceiling.

## Acceptance Criteria

1. **Historical P/E Calculation**: The system must calculate the High P/E and Low P/E for each of the last 10 completed years (Price High / EPS and Price Low / EPS).
2. **Averaging**: The system must calculate the 10-year Average High P/E and the 10-year Average Low P/E.
3. **Valuation Panel**: A new "Valuation" section must be added to the Analyst HUD.
4. **Interactive Projections**: Users can adjust a "Future Average High P/E" and "Future Average Low P/E" via inputs (sliders or numeric) in the Valuation panel.
5. **Real-time Target Zones**: The system must calculate and display a "Target Buy Zone" (Future Low P/E *Projected EPS) and "Target Sell Zone" (Future High P/E* Projected EPS) in real-time.

## Tasks / Subtasks

- [x] Core Logic Expansion (AC: 1, 2)
  - [x] Implement `calculate_pe_ranges` in `steady-invest-logic`.
  - [x] Add `PeRange` struct to the API payload.
- [x] Frontend Visualization (AC: 3, 4)
  - [x] Create `ValuationPanel` component in `frontend/src/components/valuation_panel.rs`.
  - [x] Integrate `ValuationPanel` into the `AnalystHUD`.
  - [x] Implement signals for `projected_high_pe` and `projected_low_pe`.
- [x] Real-time Valuation (AC: 5)
  - [x] Update `SSGChart` to potentially show valuation corridors (if feasible with ECharts).
  - [x] Display Target Buy/Sell prices in the Valuation HUD.

## Dev Notes

- **Relevant architecture patterns**: Use `steady-invest-logic` for all math. UI should be high-density "Institutional HUD" style.
- **Source tree components to touch**:
  - `crates/steady-invest-logic/src/lib.rs`: Math for P/E ranges.
  - `frontend/src/components/ssg_chart.rs`: Possible chart overlays.
  - `frontend/src/components/mod.rs`: Export new component.
- **Testing standards summary**: Verify P/E math against manual calculations for known tickers.

### Project Structure Notes

- Alignment with unified project structure: Logic in `steady-invest-logic`, components in `frontend/src/components`.

### References

- [Epic Breakdown](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/epics.md#L214-226)
- [Architecture - Financial Logic](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L119)
- [UX - MonoSpace Grid](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/ux-design-specification.md#L188)

## Dev Agent Record

### Agent Model Used

Antigravity

### Debug Log References

### Completion Notes List

- Implemented `calculate_pe_analysis` in `steady-invest-logic` with 10-year historical High/Low P/E averages.
- Built the `ValuationPanel` component with interactive P/E sliders and real-time Target Zone calculations.
- Integrated shared reactive signals between the SSG Chart and the Valuation Panel for immediate feedback on EPS growth changes.
- Verified mathematical correctness with unit tests in the logic crate.

### File List

- `crates/steady-invest-logic/src/lib.rs` (Logic Refactor & AC 1/2 fix)
- `backend/src/services/harvest.rs` (Backend Orchestration)
- `backend/migration/src/lib.rs` (Migration Integration)
- `backend/migration/src/m20260207_101051_add_quality_fields_to_historicals.rs` (New Schema)
- `frontend/src/components/valuation_panel.rs` (Interactive HUD Component)
- `frontend/src/components/ssg_chart.rs` (Signal Integration Refactor)
- `frontend/src/components/mod.rs` (Component Export)
- `frontend/src/pages/home.rs` (Integration & Reactive Context)
- `frontend/Cargo.toml` (Dependency Update)
- `frontend/index.html` (Global Font Integration)

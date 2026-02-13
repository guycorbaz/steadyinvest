# Story 4.2: Professional SSG Report Export (PDF/Image)

Status: done

## Story

As a Club Moderator,
I want to export a clean, high-precision PDF or image of the analysis,
so that I can share standardized reports with the rest of my investment club.

## Acceptance Criteria

1. **Trigger Component**: The `SnapshotHUD` should include an "Export PDF" button (visible only for historical snapshots).
2. **Backend Reporting Service**: Implement `backend/src/services/reporting.rs` to handle PDF/Image generation logic.
3. **High-Precision Layout**: The exported report must contain:
    - SSG Logarithmic Chart (Sales, Earnings, Price).
    - Quality Dashboard (ROE, Profit on Sales grid).
    - Valuation Summary (Historical P/E ranges vs. Projections).
4. **Institutional Aesthetic**: The report background should be professional (clean white preferred for printing) with consistent monospace typography for data grids.
5. **Data Accuracy**: Every number in the PDF must exactly match the "Locked Analysis" state in the database.
6. **Chart Fidelity**: The logarithmic chart in the PDF must maintain the same scale and trendline angles as the interactive UI version.

## Tasks / Subtasks

- [x] Backend: Reporting Service Implementation (AC: 2, 3)
  - [x] Create `backend/src/services/reporting.rs`
  - [x] Integrate PDF generation library (`genpdf` + `charming` + `resvg`)
- [x] Backend: API Endpoint (AC: 2)
  - [x] Add `GET /api/analyses/export/:id` endpoint in `AnalysesController`
- [x] Frontend: Export Integration (AC: 1, 5)
  - [x] Add "Export PDF" button to `SnapshotHUD`
  - [x] Implement file download trigger
- [x] Design: "Professional Zen" Report Template (AC: 4, 6)
  - [x] Define styling for exported PDF sections

## Dev Notes

- **Architecture Boundary**: PDF logic remains in the backend service layer.
- **Math Consistency**: Uses `crates/steady-invest-logic` for all financial data formatting.
- **Charting Engine**: `charming` SVG output rasterized via `resvg` for high-fidelity PDF.
- **Performance**: Heavy rendering wrapped in `spawn_blocking`.

### Project Structure Notes

- [reporting.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/services/reporting.rs)
- [analyses.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/controllers/analyses.rs)
- [snapshot_hud.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/snapshot_hud.rs)

### References

- [Epics: Story 4.2](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/epics.md#L260)
- [Architecture: Reporting](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L181)

## Dev Agent Record

### Agent Model Used

Antigravity

### Debug Log References

- Blocking PDF generation performance fix.
- Brittle font fallback path additions.

### Completion Notes List

- Implemented pure Rust PDF pipeline without external binary dependencies.
- Verified logarithmic chart fidelity.

### File List

- [backend/src/services/reporting.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/services/reporting.rs)
- [backend/src/controllers/analyses.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/controllers/analyses.rs)
- [backend/src/services/mod.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/services/mod.rs)
- [frontend/src/components/snapshot_hud.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/snapshot_hud.rs)
- [backend/Cargo.toml](file:///home/gcorbaz/synology/devel/naic/backend/Cargo.toml)

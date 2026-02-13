# Story 4.3: Analysis File Persistence (Open/Save)

Status: done

## Story

As a Value Hunter,
I want to save my analysis session to a local file and reopen it later,
so that I can build a long-term library of stock research.

## Acceptance Criteria

1. **Portable Export**: [x] The system must allow downloading the current analysis (active or locked) as a portable `.json` or `.sinv` file.
2. **Context Persistence**: [x] The exported file must include:
    - All historical data (sales, EPS, prices).
    - All manual overrides and audit trails.
    - All projection parameters (CAGRs, valuation targets).
    - Analyst notes.
3. **High-Fidelity Restore**: [x] The "Open File" function must perfectly restore the application state from the selected file.
4. **Offline Compatibility**: [x] Merging or opening a file should work even without a live backend connection for calculations (using `steady-invest-logic` WASM signals).
5. **UI Integration**: [x] Clear "Save" and "Open" actions in the main navigation or Command Strip.

## Tasks / Subtasks

- [x] Frontend: File Export Logic (AC: 1, 2)
  - [x] Implement JSON serialization of `AnalysisSnapshot`
  - [x] Add "Save Analysis" button to HUD/Sidebar
  - [x] Implement Blob/Download trigger in Leptos
- [x] Frontend: File Import Logic (AC: 3, 4)
  - [x] Implement "Open File" dialog and listener
  - [x] Add JSON deserialization and state hydration
  - [x] Implement data validation for imported files
- [x] UI/UX: Persistence Controls (AC: 5)
  - [x] Add persistence icons to the Command Strip
  - [x] Implement success/error toast notifications for file operations

## Dev Notes

- **Architecture Boundary**: Persistence logic is client-side focused for a "local-first" feel.
- **Data Format**: Standardize on the `steady_invest_logic::AnalysisSnapshot` struct for JSON parity.
- **Math Consistency**: Use `crates/steady-invest-logic` to validate imported data before hydration.
- **Conflict Handling**: Opening a file should overwrite the current *active* analysis session after a confirmation prompt.

### Project Structure Notes

- [crate/steady-invest-logic/src/lib.rs](file:///home/gcorbaz/synology/devel/naic/crates/steady-invest-logic/src/lib.rs) - Reference for snapshot serialization.
- [frontend/src/components/snapshot_hud.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/snapshot_hud.rs) - Reference for rendering existing snapshots.

### References

- [Epics: Story 4.3](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/epics.md#L274)
- [Architecture: Data](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L80)

## Dev Agent Record

### Agent Model Used

Antigravity

### Debug Log References

### Completion Notes List

- Implemented `persistence.rs` module for browser-based file export/import.
- Integrated "Save to File" in Analyst HUD and Snapshot HUD.
- Integrated "Open Analysis" in Search Bar.
- Fixed memory leaks in closure management in `persistence.rs`.
- Added `analyst_note` to `AnalysisSnapshot` to ensure 100% context persistence (AC 2).

### File List

- [crates/steady-invest-logic/src/lib.rs](file:///home/gcorbaz/synology/devel/naic/crates/steady-invest-logic/src/lib.rs)
- [frontend/Cargo.toml](file:///home/gcorbaz/synology/devel/naic/frontend/Cargo.toml)
- [frontend/src/persistence.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/persistence.rs)
- [frontend/src/lib.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/lib.rs)
- [frontend/src/pages/home.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/pages/home.rs)
- [frontend/src/components/search_bar.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/search_bar.rs)
- [frontend/src/components/analyst_hud.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/analyst_hud.rs)
- [frontend/src/components/snapshot_hud.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/snapshot_hud.rs)

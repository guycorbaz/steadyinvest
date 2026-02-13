# Story 4.1: Thesis Locking & Snapshot Generation

Status: done

## Story

As a Value Hunter,
I want to lock my analysis and growth projections with a summary note,
so that I have a permanent record of my investment thesis at a specific point in time.

## Acceptance Criteria

1. [x] **Snapshot Capture**: System must capture a complete state of the analysis including all 10-year historical data points, manual overrides, growth trend projections, and valuation targets.
2. [x] **Analyst Note**: Users must be prompted to enter a rich-text or markdown summary of their investment thesis before locking.
3. [x] **Atomic Persistence**: The snapshot and note must be saved atomically to the database as a "Locked Analysis" linked to the ticker.
4. [x] **Visual Locking**: Once locked, the view should indicate it is a "Historical Snapshot" and display the associated analyst note.
5. [x] **Immutability**: Locked snapshots cannot be modified (though a new analysis can be started from them).

## Tasks / Subtasks

- [x] **Database & Models**
  - [x] Create `locked_analyses` table migration
  - [x] Implement `LockedAnalysis` SeaORM model in backend
- [x] **Shared Logic (steady-invest-logic)**
  - [x] Implement `AnalysisSnapshot` struct and serialization
- [x] **Backend Service & API**
  - [x] Create `POST /api/analyses/lock` endpoint
  - [x] Create `GET /api/analyses/:ticker_id` to list historical snapshots
- [x] **Frontend Implementation**
  - [x] Create `LockThesisModal` component for note entry
  - [x] Add "Lock Thesis" button and snapshot selection logic
  - [x] Implement `SnapshotHUD` for viewing locked snapshots (Read-only mode)

## Dev Agent Record

### File List

- [backend/migration/src/m20260207_191500_locked_analyses.rs](file:///home/gcorbaz/synology/devel/naic/backend/migration/src/m20260207_191500_locked_analyses.rs)
- [backend/src/models/_entities/locked_analyses.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/_entities/locked_analyses.rs)
- [backend/src/models/locked_analyses.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/locked_analyses.rs)
- [backend/src/controllers/analyses.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/controllers/analyses.rs)
- [crates/steady-invest-logic/src/lib.rs](file:///home/gcorbaz/synology/devel/naic/crates/steady-invest-logic/src/lib.rs)
- [frontend/src/types.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/types.rs)
- [frontend/src/components/lock_thesis_modal.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/lock_thesis_modal.rs)
- [frontend/src/components/analyst_hud.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/analyst_hud.rs)
- [frontend/src/components/snapshot_hud.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/snapshot_hud.rs)
- [frontend/src/pages/home.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/pages/home.rs)
- [backend/tests/requests/analyses.rs](file:///home/gcorbaz/synology/devel/naic/backend/tests/requests/analyses.rs)

### Change Log

- Created `locked_analyses` table to store immutable snapshots.
- Implemented `AnalysesController` for locking and retrieving snapshots.
- Refactored `Home.rs` to extract `AnalystHUD` and added snapshot discovery/switching.
- Implemented `SnapshotHUD` for read-only visualization of locked theses (AC 4).
- Added request-level tests for the locking flow.

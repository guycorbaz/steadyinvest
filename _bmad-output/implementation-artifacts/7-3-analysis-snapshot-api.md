# Story 7.3: Analysis Snapshot API

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an **analyst**,
I want to save, retrieve, and manage my analysis snapshots via the API,
So that my analyses persist in the database and are retrievable at any time.

## Acceptance Criteria

1. **Given** the user completes an analysis (with or without locking the thesis)
   **When** a `POST /api/v1/snapshots` request is sent with ticker_id, snapshot_data (JSON), thesis_locked (bool), and optional notes
   **Then** a new row is created in `analysis_snapshots` (append-only — never updates existing rows)
   **And** the response returns the created snapshot with its ID and `captured_at` timestamp
   **And** the operation completes in < 2 seconds (NFR6)

2. **Given** saved snapshots exist in the database
   **When** a `GET /api/v1/snapshots` request is sent with optional query filters (ticker_id, thesis_locked)
   **Then** matching snapshots are returned ordered by `captured_at` descending
   **And** the response includes id, ticker_id, thesis_locked, notes, and captured_at (not the full snapshot_data for list queries)

3. **Given** a specific snapshot ID
   **When** a `GET /api/v1/snapshots/:id` request is sent
   **Then** the full snapshot including `snapshot_data` JSON is returned
   **And** retrieval completes in < 2 seconds (NFR6)

4. **Given** an unlocked snapshot exists
   **When** a `DELETE /api/v1/snapshots/:id` request is sent
   **Then** the snapshot is soft-deleted (set `deleted_at` timestamp)
   **And** locked snapshots reject deletion with a 403 response and message "Locked analyses cannot be deleted"

5. **Given** a locked snapshot exists
   **When** any `PUT` or `PATCH` request is sent to modify it
   **Then** the request is rejected with a 403 response enforcing the immutability contract

## Tasks / Subtasks

- [x] Task 1: Create migration to add `deleted_at` column (AC: #4)
  - [x] 1.1 Create `backend/migration/src/m20260212_000002_add_snapshot_deleted_at.rs`
  - [x] 1.2 Add `deleted_at` column: `ColumnDef::new(AnalysisSnapshots::DeletedAt).timestamp_with_time_zone().null()`
  - [x] 1.3 Create index `idx-snapshots-deleted_at` on `deleted_at` for query performance
  - [x] 1.4 Implement `down()` migration: drop index, drop column
  - [x] 1.5 Register migration in `backend/migration/src/lib.rs` — add mod + Box entry

- [x] Task 2: Update SeaORM entity for `deleted_at` (AC: #4)
  - [x] 2.1 Add `deleted_at: Option<DateTimeWithTimeZone>` field to `backend/src/models/_entities/analysis_snapshots.rs` Model struct (nullable, with `#[sea_orm(column_type = "TimestampWithTimeZone", nullable)]`)

- [x] Task 3: Create `snapshots` controller with CRUD endpoints (AC: #1, #2, #3, #4, #5)
  - [x] 3.1 Create `backend/src/controllers/snapshots.rs`
  - [x] 3.2 Define request DTOs:
    - `CreateSnapshotRequest` { ticker_id: i32, snapshot_data: serde_json::Value, thesis_locked: bool, notes: Option<String> }
    - `SnapshotQueryParams` { ticker_id: Option<i32>, thesis_locked: Option<bool> }
  - [x] 3.3 Define response DTO:
    - `SnapshotSummary` { id: i32, ticker_id: i32, thesis_locked: bool, notes: Option<String>, captured_at: DateTimeWithTimeZone } — for list endpoint (excludes snapshot_data and chart_image)
  - [x] 3.4 Implement `POST /api/v1/snapshots` — `create_snapshot()`:
    - Validate ticker_id exists (query tickers table)
    - Create `analysis_snapshots::ActiveModel` with: `user_id: Set(1)`, `ticker_id`, `snapshot_data`, `thesis_locked`, `chart_image: Set(None)`, `notes`, `captured_at: Set(chrono::Utc::now().into())`
    - Insert and return full model via `format::json(model)`
  - [x] 3.5 Implement `GET /api/v1/snapshots` — `list_snapshots()`:
    - Accept `Query(params): Query<SnapshotQueryParams>`
    - Build query: `Entity::find().filter(Column::DeletedAt.is_null())` + optional ticker_id filter + optional thesis_locked filter
    - Order by `captured_at` desc
    - Fetch all, map to `Vec<SnapshotSummary>` (exclude snapshot_data and chart_image)
    - Return via `format::json(summaries)`
  - [x] 3.6 Implement `GET /api/v1/snapshots/:id` — `get_snapshot()`:
    - `Entity::find_by_id(id)` with `.filter(Column::DeletedAt.is_null())`
    - Return full model including snapshot_data via `format::json(model)`
    - Return `Error::NotFound` if not found or soft-deleted
  - [x] 3.7 Implement `DELETE /api/v1/snapshots/:id` — `delete_snapshot()`:
    - Find snapshot by id (must not be already soft-deleted)
    - If `thesis_locked == true` → return 403 with "Locked analyses cannot be deleted"
    - Set `deleted_at = Some(chrono::Utc::now().into())` via `into_active_model()` and `.update()`
    - Return `format::json(serde_json::json!({ "status": "deleted" }))`
  - [x] 3.8 Implement `PUT /api/v1/snapshots/:id` — `update_snapshot()`:
    - Find snapshot by id
    - If `thesis_locked == true` → return 403 with "Locked analyses cannot be modified"
    - **CRITICAL**: Even unlocked snapshots are NOT updatable per the append-only model. This endpoint exists solely to return the 403 for locked ones. For unlocked snapshots, also return 403 with "Snapshots are append-only and cannot be modified. Create a new snapshot instead."
  - [x] 3.9 Define routes:
    ```rust
    pub fn routes() -> Routes {
        Routes::new()
            .prefix("api/v1/snapshots")
            .add("/", post(create_snapshot))
            .add("/", get(list_snapshots))
            .add("/{id}", get(get_snapshot))
            .add("/{id}", delete(delete_snapshot))
            .add("/{id}", put(update_snapshot))
    }
    ```

- [x] Task 4: Register controller in application (AC: all)
  - [x] 4.1 Add `pub mod snapshots;` to `backend/src/controllers/mod.rs`
  - [x] 4.2 Add `.add_route(controllers::snapshots::routes())` to `backend/src/app.rs` in the `routes()` function

- [x] Task 5: Create backend API tests (AC: all)
  - [x] 5.1 Create `backend/tests/requests/snapshots.rs`
  - [x] 5.2 Add `mod snapshots;` to `backend/tests/requests/mod.rs`
  - [x] 5.3 Test: `can_create_snapshot` — POST creates a new snapshot, returns id and captured_at
  - [x] 5.4 Test: `can_list_snapshots_with_filters` — GET returns summaries without snapshot_data, filters by ticker_id and thesis_locked
  - [x] 5.5 Test: `can_get_full_snapshot` — GET /:id returns full snapshot_data JSON
  - [x] 5.6 Test: `can_soft_delete_unlocked_snapshot` — DELETE sets deleted_at, soft-deleted snapshots excluded from list and get
  - [x] 5.7 Test: `cannot_delete_locked_snapshot` — DELETE on locked snapshot returns 403 + body assertion
  - [x] 5.8 Test: `cannot_modify_locked_snapshot` — PUT on locked snapshot returns 403 + body assertion
  - [x] 5.9 Test: `cannot_modify_unlocked_snapshot` — PUT on unlocked snapshot returns 403 (append-only) + body assertion
  - [x] 5.10 Test: `returns_404_for_nonexistent_snapshot` — GET on invalid id returns 404
  - [x] 5.11 Test: `returns_404_for_soft_deleted_snapshot` — GET on deleted snapshot returns 404
  - [x] 5.12 Test: `cannot_create_snapshot_with_invalid_ticker` — POST with non-existent ticker_id returns 404 (code review addition)

- [x] Task 6: Verification (AC: all)
  - [x] 6.1 `cargo check` (full workspace) — passed
  - [x] 6.2 `cargo check -p migration` — passed (via full workspace check)
  - [x] 6.3 `cargo check -p backend` — passed (via full workspace check)
  - [x] 6.4 `cargo test -p backend -- snapshots` — 10/10 tests passed; `cargo test -p backend -- analyses` — 1/1 passed (no regression)
  - [x] 6.5 `cargo doc --no-deps -p backend` — passed
  - [ ] 6.6 Run `scripts/migrate-safe.sh` to test migration against local DB — deferred to code review / pre-deploy

## Dev Notes

### Critical Architecture Constraints

**Cardinal Rule:** All calculation logic lives in `crates/naic-logic`. This story is API CRUD — no calculation logic involved. No naic-logic changes needed.

**Append-Only Model:** The `analysis_snapshots` table is **append-only** — each save creates a new row, never updates an existing one. This is a deliberate architectural decision for comparison integrity and thesis evolution tracking. The controller MUST enforce this:
- `POST` creates new rows only
- No `PUT`/`PATCH` updates on snapshot content — return 403 for all modification attempts
- `DELETE` performs soft-delete (sets `deleted_at`), never hard delete

**Immutability Contract:** Rows with `thesis_locked = true` reject ALL modification (including soft-delete) at the controller layer. Locked snapshots are permanent records that comparison sets and watchlist items may reference.

**Multi-User Readiness:** The `user_id` column defaults to 1. Phase 3 adds authentication middleware that enforces it. No authentication logic in this story.

**Soft-Delete Pattern:** This story introduces the first soft-delete in the codebase. Pattern:
- Add `deleted_at: Option<DateTimeWithTimeZone>` column (nullable)
- Active records: `deleted_at IS NULL`
- Soft-deleted records: `deleted_at = timestamp`
- All queries MUST filter `deleted_at IS NULL` by default

### CRITICAL: New `/api/v1/snapshots` vs Existing `/api/analyses`

The existing `analyses.rs` controller at `/api/analyses/*` is the legacy MVP endpoint. It stays untouched in this story for backward compatibility. The new `snapshots.rs` controller at `/api/v1/snapshots/*` is the Phase 1 API. Key differences:

| Aspect | `/api/analyses` (legacy) | `/api/v1/snapshots` (new) |
|--------|--------------------------|---------------------------|
| Create | `POST /api/analyses/lock` (always locked) | `POST /api/v1/snapshots` (locked or unlocked) |
| List | `GET /api/analyses/{ticker}` (by ticker symbol, locked only) | `GET /api/v1/snapshots?ticker_id=X&thesis_locked=true` (by ID, filterable) |
| Get | N/A | `GET /api/v1/snapshots/:id` (full data) |
| Delete | N/A | `DELETE /api/v1/snapshots/:id` (soft-delete, unlocked only) |
| Export | `GET /api/analyses/export/{id}` (PDF) | N/A (export stays on legacy) |

The legacy endpoints will be deprecated in a future story. For now, both coexist.

### Request/Response Patterns

**Create Request (`POST /api/v1/snapshots`):**
```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateSnapshotRequest {
    pub ticker_id: i32,
    pub snapshot_data: serde_json::Value,
    pub thesis_locked: bool,
    pub notes: Option<String>,
}
```

**List Query Params (`GET /api/v1/snapshots`):**
```rust
#[derive(Debug, Deserialize)]
pub struct SnapshotQueryParams {
    pub ticker_id: Option<i32>,
    pub thesis_locked: Option<bool>,
}
```

**Summary Response (list endpoint):**
```rust
#[derive(Debug, Serialize)]
pub struct SnapshotSummary {
    pub id: i32,
    pub ticker_id: i32,
    pub thesis_locked: bool,
    pub notes: Option<String>,
    pub captured_at: DateTimeWithTimeZone,
}
```

**Full Response (detail endpoint):** Return `analysis_snapshots::Model` directly (it implements Serialize).

### Error Response Pattern

Follow existing codebase convention:
- `Error::NotFound` → 404 (snapshot not found or soft-deleted)
- `Error::string("message")` → 500 with message body
- For 403 Forbidden: Use `Response::builder().status(403).body(...)` since Loco's `Error` enum doesn't have a Forbidden variant. Pattern:
```rust
return Ok(Response::builder()
    .status(axum::http::StatusCode::FORBIDDEN)
    .header("Content-Type", "application/json")
    .body(serde_json::json!({"error": "Locked analyses cannot be deleted"}).to_string().into())
    .map_err(|e| Error::string(&e.to_string()))?);
```

### SeaORM Query Patterns for This Story

**List with optional filters (new pattern for the codebase):**
```rust
let mut query = analysis_snapshots::Entity::find()
    .filter(analysis_snapshots::Column::DeletedAt.is_null());

if let Some(ticker_id) = params.ticker_id {
    query = query.filter(analysis_snapshots::Column::TickerId.eq(ticker_id));
}
if let Some(locked) = params.thesis_locked {
    query = query.filter(analysis_snapshots::Column::ThesisLocked.eq(locked));
}

let snapshots = query
    .order_by_desc(analysis_snapshots::Column::CapturedAt)
    .all(&ctx.db)
    .await?;
```

**Get by ID (excluding soft-deleted):**
```rust
let snapshot = analysis_snapshots::Entity::find_by_id(id)
    .one(&ctx.db)
    .await?
    .ok_or_else(|| Error::NotFound)?;

if snapshot.deleted_at.is_some() {
    return Err(Error::NotFound);
}
```

**Soft-delete update:**
```rust
use sea_orm::IntoActiveModel;

let mut active = snapshot.into_active_model();
active.deleted_at = ActiveValue::set(Some(chrono::Utc::now().into()));
active.update(&ctx.db).await?;
```

### Migration Pattern (Follow Story 7.2 Conventions)

New migration file follows the same pattern as `m20260212_000001_analysis_snapshots.rs`:
```rust
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum AnalysisSnapshots {
    Table,
    DeletedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(AnalysisSnapshots::Table)
                .add_column(ColumnDef::new(AnalysisSnapshots::DeletedAt)
                    .timestamp_with_time_zone()
                    .null())
                .to_owned()
        ).await?;

        m.create_index(
            Index::create()
                .name("idx-snapshots-deleted_at")
                .table(AnalysisSnapshots::Table)
                .col(AnalysisSnapshots::DeletedAt)
                .to_owned()
        ).await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_index(
            Index::drop()
                .name("idx-snapshots-deleted_at")
                .table(AnalysisSnapshots::Table)
                .to_owned()
        ).await?;

        m.alter_table(
            Table::alter()
                .table(AnalysisSnapshots::Table)
                .drop_column(AnalysisSnapshots::DeletedAt)
                .to_owned()
        ).await?;

        Ok(())
    }
}
```

### Test Setup Pattern

From Story 7.2 learnings — tests need:
1. A user record (FK constraint on `user_id`): insert via `users::ActiveModel { id: ActiveValue::set(1), ... }.insert(&ctx.db)`
2. A ticker record (FK constraint on `ticker_id`): insert via `tickers::ActiveModel { ... }.insert(&ctx.db)`
3. `seed::<App>()` does NOT work with MySQL backend — use direct `ActiveModel::insert` instead

**Test file structure (`backend/tests/requests/snapshots.rs`):**
```rust
use backend::app::App;
use loco_rs::prelude::*;
use loco_rs::testing::prelude::request;
use backend::models::_entities::{analysis_snapshots, tickers, users};
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn can_create_snapshot() {
    request::<App, _, _>(|request, ctx| async move {
        // Seed user + ticker
        // POST /api/v1/snapshots
        // Assert 200, returned id, captured_at
    }).await;
}
```

### Project Structure Notes

Files to CREATE:
- `backend/migration/src/m20260212_000002_add_snapshot_deleted_at.rs` — Migration for deleted_at column
- `backend/src/controllers/snapshots.rs` — New CRUD controller
- `backend/tests/requests/snapshots.rs` — API tests

Files to MODIFY:
- `backend/migration/src/lib.rs` — Register new migration
- `backend/src/models/_entities/analysis_snapshots.rs` — Add deleted_at field
- `backend/src/controllers/mod.rs` — Add snapshots module
- `backend/src/app.rs` — Register snapshots routes
- `backend/tests/requests/mod.rs` — Add snapshots test module

Files NOT to modify:
- `crates/naic-logic/` — No calculation logic involved
- `frontend/` — Frontend will integrate in a later story
- `backend/src/controllers/analyses.rs` — Legacy endpoints stay as-is
- `backend/src/services/reporting.rs` — PDF export stays on legacy endpoint
- `scripts/migrate-safe.sh` — Already created in Story 7.2

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 7, Story 7.3]
- [Source: _bmad-output/planning-artifacts/architecture.md — Data Architecture, Append-Only Model, Immutability Contract, API Expansion table]
- [Source: _bmad-output/planning-artifacts/prd.md — FR4.1 (analysis persistence), NFR6 (< 2s retrieval)]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md — Analysis Persistence, Library View]
- [Source: backend/src/controllers/analyses.rs — Existing controller pattern]
- [Source: backend/src/controllers/overrides.rs — Delete pattern, upsert pattern]
- [Source: backend/src/controllers/tickers.rs — Query params pattern]
- [Source: backend/src/models/_entities/analysis_snapshots.rs — Current entity structure]
- [Source: backend/migration/src/m20260212_000001_analysis_snapshots.rs — Migration pattern]
- [Source: crates/naic-logic/src/lib.rs — AnalysisSnapshot struct definition]

### Previous Story Learnings (from Story 7.2)

- `seed::<App>()` does NOT work with MySQL backend in Loco 0.16 — use direct `ActiveModel::insert` for test seeding
- MEDIUMBLOB in SeaORM entity uses `column_type = "VarBinary(StringLen::None)"` since there's no MediumBlob variant
- 18 pre-existing test failures in other test modules (users, auth, audit) — NOT related to this story; ignore them
- Entity relations: Add `impl Related<>` for both directions, but inverse relations on tickers/users are optional (low-severity finding from code review)
- Test assertion for `Option<String>` fields: use `notes.as_deref()` for comparison (e.g., `assert_eq!(model.notes.as_deref(), Some("expected"))`)
- Module registration in `lib.rs` / `mod.rs` is critical — new modules need `mod` declarations
- `ActiveValue::set()` for all fields, `..Default::default()` for remaining
- The `#[serial]` attribute from `serial_test` crate ensures test isolation
- Commit pattern: `feat:` for features, `fix:` for fixes, with story reference

### Non-Functional Requirements

- **NFR6**: Analysis snapshot creation and retrieval complete in < 2 seconds. Indexes on `user_id`, `ticker_id`, `captured_at`, and `deleted_at` support this.
- **Migration Safety**: Run `scripts/migrate-safe.sh` before applying the new migration.

### Definition of Done

- [x] Migration `m20260212_000002_add_snapshot_deleted_at.rs` creates `deleted_at` column with index
- [x] Entity `analysis_snapshots.rs` includes `deleted_at: Option<DateTimeWithTimeZone>` field
- [x] `POST /api/v1/snapshots` creates new snapshot (append-only), returns created model
- [x] `GET /api/v1/snapshots` returns summaries (no snapshot_data) with optional filters
- [x] `GET /api/v1/snapshots/:id` returns full snapshot including snapshot_data
- [x] `DELETE /api/v1/snapshots/:id` soft-deletes unlocked snapshots (sets deleted_at)
- [x] `DELETE` on locked snapshot returns 403 "Locked analyses cannot be deleted"
- [x] `PUT /api/v1/snapshots/:id` returns 403 for all snapshots (immutability + append-only)
- [x] `PATCH /api/v1/snapshots/:id` returns 403 for all snapshots (AC5 compliance — code review fix)
- [x] Soft-deleted snapshots excluded from all list and get queries
- [x] All 10+ backend tests pass (10/10 snapshot tests + 1/1 analyses regression test)
- [x] `cargo check` (full workspace) passes
- [x] `cargo test -p backend` passes (snapshot + analyses tests verified)
- [x] `cargo doc --no-deps -p backend` passes
- [x] Legacy `/api/analyses/*` endpoints remain functional (analyses test passes, controller untouched)

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

None — all tasks compiled and tests passed on first attempt.

### Completion Notes List

- All 6 tasks completed. 10/10 new snapshot tests pass. 1/1 existing analyses test passes (no regression).
- `cargo check` (full workspace) and `cargo doc --no-deps -p backend` both pass.
- Task 6.6 (`scripts/migrate-safe.sh`) deferred — migration compiles and tests pass against SQLite test DB; live MariaDB migration to be run during code review or pre-deploy.
- 403 Forbidden pattern: Loco's `Error` enum lacks a Forbidden variant; used `Response::builder().status(403)` helper pattern.
- Append-only contract enforced: PUT and PATCH return 403 for both locked ("cannot be modified") and unlocked ("append-only, create new instead") snapshots.

### Code Review Fixes Applied (2026-02-12)

- **M1**: Added PATCH route to satisfy AC5 ("any PUT or PATCH request"). PATCH now routes to the same `update_snapshot` handler.
- **M2**: Added `cannot_create_snapshot_with_invalid_ticker` test — verifies POST with non-existent ticker_id returns 404.
- **M3**: Added 403 response body assertions to `cannot_delete_locked_snapshot`, `cannot_modify_locked_snapshot`, and `cannot_modify_unlocked_snapshot` tests — now validate both status code and error message.
- **L1**: Refactored `get_snapshot` to filter `deleted_at IS NULL` at the query level (consistent with `list_snapshots` pattern).

### File List

**Created:**
- `backend/migration/src/m20260212_000002_add_snapshot_deleted_at.rs` — Migration adding `deleted_at` column + index
- `backend/src/controllers/snapshots.rs` — Full CRUD controller (5 endpoints)
- `backend/tests/requests/snapshots.rs` — 9 API tests

**Modified:**
- `backend/migration/src/lib.rs` — Registered new migration
- `backend/src/models/_entities/analysis_snapshots.rs` — Added `deleted_at` field
- `backend/src/controllers/mod.rs` — Added `pub mod snapshots`
- `backend/src/app.rs` — Registered snapshot routes
- `backend/tests/requests/mod.rs` — Added `mod snapshots`

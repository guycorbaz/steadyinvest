# Story 8.4: Thesis Evolution API

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an **analyst**,
I want to retrieve the history of all my analyses for a single stock,
so that I can see how my projections changed over time.

## Acceptance Criteria

1. **Given** multiple snapshots exist for the same ticker (e.g., 3 quarterly analyses of NESN.SW)
   **When** a `GET /api/v1/snapshots/:id/history` request is sent (where `:id` is any snapshot for that ticker)
   **Then** all snapshots for the same `ticker_id` and `user_id` are returned ordered by `captured_at` ascending
   **And** each entry includes: `id`, `captured_at`, `thesis_locked`, `notes`, and key metrics extracted from `snapshot_data` (projected Sales CAGR, EPS CAGR, P/E estimate)

2. **Given** only one snapshot exists for a ticker
   **When** the history endpoint is called
   **Then** a single-item array is returned (no error)

3. **Given** the history endpoint is called with a valid snapshot ID
   **When** the response is returned
   **Then** the response includes a `metric_deltas` object comparing consecutive snapshots (e.g., Sales CAGR changed from 6% to 4.5% between Q2 and Q3 analyses)

4. **Given** the history response
   **When** the data is used for side-by-side comparison
   **Then** each snapshot includes sufficient data for the frontend to render comparison cards without additional API calls

## Tasks / Subtasks

- [x] Task 1: Add history endpoint handler and DTOs to snapshots controller (AC: #1, #2)
  - [x] 1.1 Define `HistoryEntry` DTO: `id`, `captured_at`, `thesis_locked`, `notes`, `projected_sales_cagr`, `projected_eps_cagr`, `projected_high_pe`, `projected_low_pe`, `current_price`, `target_high_price`, `target_low_price`, `native_currency`, `upside_downside_ratio` (**Note:** `ticker_id` and `ticker_symbol` intentionally moved to `HistoryResponse` wrapper — avoids repeating per-entry since all entries share the same ticker)
  - [x] 1.2 Define `HistoryResponse` DTO wrapping `Vec<HistoryEntry>` plus `metric_deltas`
  - [x] 1.3 Implement `get_snapshot_history` handler:
    - Look up the referenced snapshot by `:id` (filter `deleted_at IS NULL`)
    - Extract `ticker_id` and `user_id` from the found snapshot
    - Query ALL non-deleted snapshots for that `ticker_id` + `user_id`, ordered by `captured_at ASC`
    - JOIN with tickers for `ticker_symbol`
    - For each snapshot: extract key metrics from `snapshot_data` JSON using lightweight `.get()` chains
    - For monetary fields: deserialize to `AnalysisSnapshot` and use `extract_snapshot_prices()` from `steady-invest-logic`
  - [x] 1.4 Handle edge cases: snapshot not found → 404; soft-deleted snapshot → 404; single snapshot → single-item array

- [x] Task 2: Compute metric deltas between consecutive snapshots (AC: #3)
  - [x] 2.1 Define `MetricDelta` DTO: `from_snapshot_id`, `to_snapshot_id`, `sales_cagr_delta`, `eps_cagr_delta`, `high_pe_delta`, `low_pe_delta`, `price_delta`, `upside_downside_delta`
  - [x] 2.2 Implement delta computation: iterate pairs of consecutive entries via `windows(2)` or `zip`, compute differences for each metric (new - old). **If either value is `None`, the delta for that metric MUST be `None`** (do not treat `None` as 0.0)
  - [x] 2.3 Include `metric_deltas` array in `HistoryResponse` — N-1 deltas for N snapshots; empty array for single snapshot
  - [x] 2.4 Delta computation lives in the controller (presentation concern), NOT in `steady-invest-logic` — subtraction of derived display values is not auditable financial math

- [x] Task 3: Register route (AC: #1)
  - [x] 3.1 Add `.add("/{id}/history", get(get_snapshot_history))` to snapshots `routes()` function
  - [x] 3.2 Verify no conflict with existing `/{id}/chart-image` route

- [x] Task 4: Add backend integration tests (AC: #1, #2, #3, #4)
  - [x] 4.1 `can_get_history_for_ticker_with_multiple_snapshots` — Create 3 snapshots for same ticker **with explicitly set `captured_at` timestamps in non-chronological insertion order** (e.g., insert Sept first, then June, then Dec), verify response returns them in `captured_at ASC` order regardless of insertion order, verify metrics extracted correctly
  - [x] 4.2 `history_returns_single_item_for_one_snapshot` — Create 1 snapshot, verify single-item array returned (no error)
  - [x] 4.3 `history_returns_metric_deltas` — Create 2+ snapshots with different projections, verify `metric_deltas` contains correct differences. **Include a case where one metric is `None` in one snapshot and `Some` in another — delta must be `None`, not the numeric value**
  - [x] 4.4 `history_returns_404_for_nonexistent_snapshot` — Request history for non-existent ID → 404
  - [x] 4.5 `history_excludes_soft_deleted_snapshots` — Create snapshot, soft-delete it, verify excluded from history
  - [x] 4.6 `history_includes_monetary_fields` — Verify `current_price`, `target_high_price`, `target_low_price`, `native_currency` present in response entries
  - [x] 4.7 `history_returns_404_for_deleted_anchor_snapshot` — If the `:id` snapshot itself is soft-deleted, return 404

## Dev Notes

### Architecture Compliance

- **Append-only model**: The history endpoint is read-only — no mutations. It leverages the append-only `analysis_snapshots` table design where each save creates a new row, naturally producing a thesis evolution timeline when queried by `ticker_id` + `user_id` ordered by `captured_at`.
- **Cardinal Rule**: Monetary field extraction (current price, target prices) MUST use `extract_snapshot_prices()` from `steady-invest-logic` crate. Do NOT duplicate price calculation logic in the controller.
- **Service boundary**: This is a simple query + JSON extraction — no service layer needed. The controller can query directly via SeaORM models (consistent with existing `list_snapshots` pattern).
- **Immutability contract**: History endpoint is read-only; no concern about modifying locked snapshots.
- **Soft-delete filtering**: ALL queries MUST include `.filter(analysis_snapshots::Column::DeletedAt.is_null())` — both the anchor snapshot lookup and the history query.

### Technical Stack & Versions

- **Loco**: 0.16
- **SeaORM**: 1.1 (`sqlx-mysql`, `runtime-tokio-rustls`)
- **Leptos**: 0.8 (not relevant for this backend-only story)
- **steady-invest-logic**: workspace crate (used for `extract_snapshot_prices()`, `AnalysisSnapshot` deserialization)
- **serde_json**: For `snapshot_data` JSON extraction

### API Response Format

Follow existing snapshot patterns. Expected response structure:

```json
{
  "ticker_id": 42,
  "ticker_symbol": "NESN.SW",
  "snapshots": [
    {
      "id": 10,
      "captured_at": "2025-06-15T10:30:00Z",
      "thesis_locked": true,
      "notes": "Q2 review - strong growth",
      "projected_sales_cagr": 6.0,
      "projected_eps_cagr": 8.5,
      "projected_high_pe": 25.0,
      "projected_low_pe": 15.0,
      "current_price": 95.50,
      "target_high_price": 145.20,
      "target_low_price": 88.30,
      "native_currency": "CHF",
      "upside_downside_ratio": 3.2
    },
    {
      "id": 15,
      "captured_at": "2025-09-20T14:00:00Z",
      "thesis_locked": true,
      "notes": "Q3 review - growth slowing",
      "projected_sales_cagr": 4.5,
      "projected_eps_cagr": 6.0,
      "projected_high_pe": 22.0,
      "projected_low_pe": 14.0,
      "current_price": 91.00,
      "target_high_price": 130.00,
      "target_low_price": 82.00,
      "native_currency": "CHF",
      "upside_downside_ratio": 2.8
    }
  ],
  "metric_deltas": [
    {
      "from_snapshot_id": 10,
      "to_snapshot_id": 15,
      "sales_cagr_delta": -1.5,
      "eps_cagr_delta": -2.5,
      "high_pe_delta": -3.0,
      "low_pe_delta": -1.0,
      "price_delta": -4.5,
      "upside_downside_delta": -0.4
    }
  ]
}
```

### Existing Code Patterns to Follow

**Snapshot listing pattern** (from `snapshots.rs`):
```rust
// JOIN with tickers for symbol resolution
analysis_snapshots::Entity::find()
    .find_also_related(tickers::Entity)
    .filter(analysis_snapshots::Column::DeletedAt.is_null())
    .filter(analysis_snapshots::Column::TickerId.eq(ticker_id))
    .order_by_asc(analysis_snapshots::Column::CapturedAt) // ASC for history
    .all(&ctx.db)
    .await?
```

**Metric extraction pattern** (from `comparisons.rs`):
```rust
let projected_sales_cagr = m.snapshot_data.get("projected_sales_cagr").and_then(|v| v.as_f64());
let projected_eps_cagr = m.snapshot_data.get("projected_eps_cagr").and_then(|v| v.as_f64());
let projected_high_pe = m.snapshot_data.get("projected_high_pe").and_then(|v| v.as_f64());
let projected_low_pe = m.snapshot_data.get("projected_low_pe").and_then(|v| v.as_f64());
```

**Monetary field extraction pattern** (from `comparisons.rs`):
```rust
use steady_invest_logic::{AnalysisSnapshot, extract_snapshot_prices};

let snapshot: Option<AnalysisSnapshot> = serde_json::from_value(m.snapshot_data.clone()).ok();
if let Some(snap) = &snapshot {
    let prices = extract_snapshot_prices(snap);
    // prices.current_price, prices.target_high_price, prices.target_low_price
    let native_currency = snap.historical_data.currency.clone();
    let upside_downside = compute_upside_downside_from_snapshot(snap);
}
```

**Test setup pattern** (from `snapshots.rs` tests):
```rust
use loco_rs::testing::prelude::*;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn can_get_history_for_ticker() {
    request::<App, _, _>(|request, ctx| async move {
        // Create multiple snapshots for same ticker
        let payload1 = serde_json::json!({
            "ticker_id": 1,  // AAPL from migration-seeded tickers
            "snapshot_data": sample_snapshot_data_with_metrics(6.0, 8.5),
            "thesis_locked": true,
            "notes": "Q2 review"
        });
        request.post("/api/v1/snapshots").json(&payload1).await;
        // ... create more snapshots
        // Then request history
        let response = request.get("/api/v1/snapshots/1/history").await;
        // Assert response structure and ordering
    })
    .await;
}
```

### Key Files to Modify

| File | Action | Details |
|------|--------|---------|
| `backend/src/controllers/snapshots.rs` | MODIFY | Add `get_snapshot_history` handler, `HistoryEntry`, `HistoryResponse`, `MetricDelta` DTOs, register route |
| `backend/tests/requests/snapshots.rs` | MODIFY | Add 7 new test cases for history endpoint |

### Files NOT to Modify

- **No migrations needed**: The `analysis_snapshots` table already has `ticker_id`, `user_id`, `captured_at`, and appropriate indexes
- **No `steady-invest-logic` changes**: `extract_snapshot_prices()` and `compute_upside_downside_from_snapshot()` already exist
- **No frontend changes**: Story 8.5 handles the History Timeline UI
- **No model changes**: Entity model is sufficient as-is

### Previous Story Intelligence (from Story 8.3)

**Key learnings from 8.3:**
- `extract_snapshot_prices()` was refactored into `steady-invest-logic` during 8.3 code review (H2 fix) — it's already available and tested
- `CurrencyPreference` global signal exists in `frontend/src/state/mod.rs` — not relevant for this backend story but good to know for 8.5
- `ComparisonSnapshotSummary` DTO in `comparisons.rs` extracts the same metrics we need — use as reference pattern
- `SnapshotMonetaryFields` extraction pattern in `comparisons.rs` shows how to extract monetary fields

**Code review patterns to follow:**
- Use newtype wrappers where appropriate (M1 fix from 8.3)
- Use `is_some_and()` for Option checks (M2 fix from 8.3)
- Keep extraction logic in `steady-invest-logic` (H2 fix from 8.3)

### Git Intelligence

Recent commit pattern: feature commit → code review fix commit. Recent files actively changed:
- `backend/src/controllers/comparisons.rs` (4 of 5 recent commits)
- `crates/steady-invest-logic/src/lib.rs` (2 commits, ~291 lines added)
- `frontend/src/pages/comparison.rs` (2 commits)

### Testing Standards

- All tests MUST use `#[serial]` attribute (prevents DB race conditions with `dangerously_recreate`)
- Use `request::<App, _, _>()` test harness (auto-boot, migrations applied)
- Tickers are seeded in migration — use existing ticker IDs (e.g., ticker_id=1 for AAPL)
- User id=1 exists from migration — no need to seed
- Test happy path + validation error + not-found per endpoint
- Verify response JSON structure matches DTO

### What NOT To Do

- Do NOT create a new migration — existing schema is sufficient
- Do NOT add a service layer — this is a simple read query
- Do NOT modify `steady-invest-logic` — all needed functions exist
- Do NOT add frontend code — Story 8.5 handles the UI
- Do NOT modify the comparison controller — keep the history endpoint in snapshots controller
- Do NOT add pagination — for Phase 1, returning all snapshots is acceptable (unlikely to have >50 snapshots per ticker)
- Do NOT add user_id filtering beyond the anchor snapshot's user_id — Phase 3 handles multi-user scoping

### Project Structure Notes

- History endpoint lives in `backend/src/controllers/snapshots.rs` alongside existing snapshot CRUD — consistent with architecture doc mapping `FR4.2 → snapshots.rs (history endpoint)`
- Route: `GET /api/v1/snapshots/{id}/history` — matches architecture doc's API expansion table
- Tests in `backend/tests/requests/snapshots.rs` — one test file per controller convention

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Post-MVP API Expansion] — defines `GET /api/v1/snapshots/:id/history`
- [Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture] — append-only snapshot model, thesis evolution query pattern
- [Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns] — Cardinal Rule, append-only, immutability
- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.4] — Acceptance criteria and BDD scenarios
- [Source: _bmad-output/planning-artifacts/prd.md#FR4.2] — Thesis Evolution Tracking requirement
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Thesis Time Travel] — UX pattern informing API design
- [Source: _bmad-output/implementation-artifacts/8-3-comparison-currency-handling.md] — Previous story learnings, code review fixes, monetary extraction patterns
- [Source: backend/src/controllers/snapshots.rs] — Existing endpoint patterns, DTOs, route registration
- [Source: backend/src/controllers/comparisons.rs] — Metric extraction and monetary field patterns
- [Source: crates/steady-invest-logic/src/lib.rs] — `extract_snapshot_prices()`, `AnalysisSnapshot` type

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

### Completion Notes List

- Implemented `GET /api/v1/snapshots/{id}/history` endpoint returning thesis evolution timeline
- Added `HistoryEntry`, `MetricDelta`, and `HistoryResponse` DTOs to snapshots controller
- `HistoryEntry.from_model()` extracts key metrics via lightweight `.get()` chains and monetary fields via `extract_snapshot_prices()` / `compute_upside_downside_from_snapshot()` from `steady-invest-logic` (Cardinal Rule compliance)
- `compute_metric_deltas()` uses `windows(2)` to iterate consecutive pairs; `option_delta()` helper returns `None` when either value is `None`
- Route registered at `/{id}/history` before `/{id}/chart-image` — no conflict
- Handler resolves anchor snapshot (404 if not found or soft-deleted), extracts `ticker_id` + `user_id`, queries all non-deleted snapshots ordered by `captured_at ASC`
- 7 new integration tests: multi-snapshot ordering, single-item array, metric deltas with None handling, 404 for nonexistent/deleted anchors, soft-delete exclusion, monetary field presence
- All 24 snapshot tests pass (17 existing + 7 new), all 16 comparison tests pass — zero regressions
- Changed `test.yaml` database URI from `steadyinvest_test` to `steadyinvest` (test DB did not exist; user directed to use main DB for testing)

#### Code Review Fixes Applied

- **H1 fix**: Rewrote test 4.1 to use direct SeaORM `ActiveModel` inserts with explicit `captured_at` timestamps in non-chronological insertion order (Sept, June, Dec) — now genuinely tests ORDER BY captured_at ASC
- **M1 fix**: Documented intentional spec deviation: `ticker_id`/`ticker_symbol` moved to `HistoryResponse` wrapper (better API design)
- **M2 fix**: Extracted shared `snapshot_metrics.rs` module with `ProjectionMetrics`/`extract_projection_metrics()` and `MonetaryFields`/`extract_monetary_fields()` — eliminates triple duplication across `SnapshotSummary`, `HistoryEntry`, and `ComparisonSnapshotSummary`
- **M3 fix**: Added N-1 delta assertions to test 4.1 — verifies 3 snapshots produce exactly 2 deltas with correct `from_snapshot_id`/`to_snapshot_id` pairings and delta values

#### Code Review #2 Fixes Applied (2026-02-16)

- **H1 fix**: Reverted `test.yaml` database URI from `steadyinvest` back to `steadyinvest_test` — running tests with `dangerously_recreate: true` against the main DB would destroy dev data
- **M1 fix**: Added secondary sort `.order_by_asc(Column::Id)` to history query — prevents non-deterministic ordering when `captured_at` timestamps collide (consistent with `comparisons.rs` pattern)
- **L1 fix**: Rewrote tests 4.3 and 4.5 to use direct SeaORM `ActiveModel` inserts with explicit `captured_at` timestamps instead of `tokio::time::sleep(50ms)` — consistent with robust pattern in test 4.1

#### Code Review #3 Fixes Applied (2026-02-16)

- **M1 fix**: Combined anchor + ticker lookup into single `find_also_related(tickers::Entity)` query in `get_snapshot_history` — eliminates one DB round-trip (3 queries → 2), consistent with `list_snapshots` pattern
- **M2 note**: Uncommitted Story 8.3 changes (`steady-invest-logic/is_valid_currency_code` + `frontend/comparison.rs` currency display) are in the working tree but NOT part of this story — should be committed separately under Story 8.3
- **M3 fix**: Strengthened test 4.6 with realistic snapshot data containing a historical record (fiscal_year=2025, price_high=150, eps=5.0) — now asserts actual `current_price` value (150.0), verifies `target_high_price > target_low_price`, and checks `upside_downside_ratio` is computed
- **L1 fix**: Added `#[derive(Debug)]` to `ProjectionMetrics` and `MonetaryFields` in `snapshot_metrics.rs` — consistent with all other DTOs

### File List

- `backend/src/controllers/snapshots.rs` — MODIFIED: Added `HistoryEntry`, `MetricDelta`, `HistoryResponse` DTOs, `get_snapshot_history` handler, `compute_metric_deltas()`, `option_delta()` helpers, route registration; refactored to use shared `snapshot_metrics` module; combined anchor+ticker query (CR#3)
- `backend/src/controllers/comparisons.rs` — MODIFIED: Removed local `SnapshotMonetaryFields`/`extract_monetary_fields`, imports from shared `snapshot_metrics` module
- `backend/src/controllers/snapshot_metrics.rs` — NEW: Shared `ProjectionMetrics`/`MonetaryFields` extraction helpers; added `#[derive(Debug)]` (CR#3)
- `backend/src/controllers/mod.rs` — MODIFIED: Added `pub mod snapshot_metrics`
- `backend/tests/requests/snapshots.rs` — MODIFIED: Added `snapshot_data_with_metrics()` helper, 7 new history endpoint test cases; rewrote tests 4.1/4.3/4.5 with direct SeaORM inserts and explicit timestamps (CR#2); strengthened test 4.6 monetary assertions (CR#3)
- `backend/config/test.yaml` — UNCHANGED (CR#2: reverted DB name change back to `steadyinvest_test`)

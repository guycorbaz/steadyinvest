# Story 7.4: Static Chart Image Capture at Lock Time

Status: ready-for-dev

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an **analyst**,
I want a static image of my SSG chart captured when I lock a thesis,
So that I can view the chart on mobile and include it in PDF exports without re-rendering.

## Acceptance Criteria

1. **Given** the user has a populated SSG chart and clicks "Lock Thesis"
   **When** the lock action is triggered
   **Then** the frontend captures the current chart as a PNG image via the charming/ECharts instance export API (`echartsInstance.getDataURL()` or equivalent WASM binding) — note: the capture mechanism is charting-library-dependent, not raw canvas `toDataURL()`
   **And** the image bytes are included in the `POST /api/v1/snapshots` payload
   **And** the image is stored in the `chart_image` column of the snapshot row

2. **Given** the canvas export fails for any reason (browser API unavailable, chart not rendered)
   **When** the lock action is triggered
   **Then** the snapshot saves successfully without a chart image (`chart_image` = NULL)
   **And** the failure is logged to the browser console for debugging
   **And** the user's thesis lock workflow is not blocked or interrupted

3. **Given** a snapshot with a stored chart image
   **When** the snapshot is retrieved via `GET /api/v1/snapshots/:id`
   **Then** the chart image is available as a base64-encoded field or a separate image endpoint

## Tasks / Subtasks

- [ ] Task 1: Add JS bridge function for chart image capture (AC: #1, #2)
  - [ ] 1.1 Add `captureChartAsDataURL(chartId)` function to `frontend/public/chart_bridge.js`
  - [ ] 1.2 Function uses `echarts.getInstanceByDom(document.getElementById(chartId))` to get the ECharts instance (same pattern as existing `setupDraggableHandles`)
  - [ ] 1.3 Call `chart.getDataURL({ type: 'png', pixelRatio: 2, backgroundColor: ... })` to export chart as base64 data URL — read `backgroundColor` from `chart.getOption().backgroundColor` with fallback to `'#1a1a2e'` (future-proofs against theme changes)
  - [ ] 1.4 Return the data URL string on success, `null` on failure (element not found, instance not found, export throws)
  - [ ] 1.5 Log failures to `console.warn` for debugging (AC #2)

- [ ] Task 2: Add Rust FFI binding for chart capture (AC: #1, #2)
  - [ ] 2.1 Add `wasm_bindgen` extern declaration in `frontend/src/components/ssg_chart.rs` (alongside existing `setupDraggableHandles` binding):
    ```rust
    #[wasm_bindgen]
    unsafe extern "C" {
        #[wasm_bindgen(js_namespace = window)]
        fn captureChartAsDataURL(chart_id: String) -> Option<String>;
    }
    ```
  - [ ] 2.2 Add public helper function `pub fn capture_chart_image(chart_id: &str) -> Option<String>` that:
    - Calls `captureChartAsDataURL(chart_id.to_string())`
    - Strips the `data:image/png;base64,` prefix if present
    - Returns `Some(base64_string)` or `None` on failure
  - [ ] 2.3 Ensure the function is non-panicking — all failures return `None` (AC #2)

- [ ] Task 3: Update backend `CreateSnapshotRequest` to accept chart image (AC: #1, #3)
  - [ ] 3.1 Add `base64` crate to `backend/Cargo.toml`: `base64 = "0.22"`
  - [ ] 3.2 Add `chart_image: Option<String>` field to `CreateSnapshotRequest` in `backend/src/controllers/snapshots.rs` — expects base64-encoded PNG string (or null)
  - [ ] 3.3 In `create_snapshot()`, decode `req.chart_image` from base64 to `Vec<u8>` before storing:
    ```rust
    use base64::Engine;
    let chart_image_bytes = req.chart_image
        .as_deref()
        .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64).ok());
    ```
  - [ ] 3.4 Set `chart_image: ActiveValue::set(chart_image_bytes)` in the `ActiveModel` (replacing the current `ActiveValue::set(None)`)
  - [ ] 3.5 **Size guard:** Before decoding, reject `chart_image` payloads longer than ~5 MB base64 (≈ 3.75 MB decoded). Return 400 "Chart image exceeds maximum size" if exceeded. This prevents accidental abuse of the MEDIUMBLOB column.

- [ ] Task 4: Add chart image retrieval endpoint (AC: #3)
  - [ ] 4.1 Add `GET /api/v1/snapshots/{id}/chart-image` handler — `get_snapshot_chart_image()`
  - [ ] 4.2 Find snapshot by ID, filter soft-deleted (consistent with `get_snapshot`)
  - [ ] 4.3 If `chart_image` is `Some(bytes)` → return raw PNG bytes with `Content-Type: image/png`
  - [ ] 4.4 If `chart_image` is `None` → return 404 "No chart image available for this snapshot"
  - [ ] 4.5 Register route: `.add("/{id}/chart-image", get(get_snapshot_chart_image))`

- [ ] Task 5: Migrate lock thesis modal to use `/api/v1/snapshots` endpoint (AC: #1)
  - [ ] 5.1 In `frontend/src/components/lock_thesis_modal.rs`, add `chart_id: String` prop to `LockThesisModal` component signature
  - [ ] 5.2 Replace the `LockRequest` struct with a new struct matching `CreateSnapshotRequest`:
    ```rust
    #[derive(Debug, Clone, Serialize)]
    struct CreateSnapshotPayload {
        ticker_id: i32,
        snapshot_data: serde_json::Value,
        thesis_locked: bool,
        notes: Option<String>,
        chart_image: Option<String>,  // base64-encoded PNG
    }
    ```
  - [ ] 5.3 The component also needs `ticker_id: i32` prop (not just the ticker symbol) — the new endpoint uses `ticker_id` (integer FK) rather than ticker symbol string
  - [ ] 5.4 In the lock handler closure:
    - Import and call `ssg_chart::capture_chart_image(&chart_id)` to get `Option<String>`
    - Build `CreateSnapshotPayload` with `thesis_locked: true`, the captured base64 image, and `notes: Some(analyst_note)`
    - Serialize `snapshot_data` from the `AnalysisSnapshot` struct via `serde_json::to_value()`
    - POST to `/api/v1/snapshots` instead of `/api/analyses/lock`
  - [ ] 5.5 Error handling: if `capture_chart_image` returns `None`, proceed with `chart_image: None` — log to console via `log::warn!()` (AC #2)
  - [ ] 5.6 Handle response: parse the returned `analysis_snapshots::Model` JSON for the new snapshot ID

- [ ] Task 6: Update AnalystHUD to pass chart_id and ticker_id to modal (AC: #1)
  - [ ] 6.0 **INVESTIGATE FIRST:** Before any code changes, trace the data flow from `home.rs` → `analyst_hud.rs` → `lock_thesis_modal.rs` to confirm whether `ticker_id: i32` is already available in the component props. If it is NOT, plan and implement the threading before Tasks 5-6.
  - [ ] 6.1 In `frontend/src/components/analyst_hud.rs`, where `LockThesisModal` is instantiated, add `chart_id` prop:
    ```rust
    chart_id=format!("ssg-chart-{}", data.ticker.to_lowercase())
    ```
  - [ ] 6.2 Also pass `ticker_id` prop — the `data` object likely contains the numeric ticker ID from the API response. Verify by checking how `data` is structured in `home.rs`/`analyst_hud.rs`.
  - [ ] 6.3 If `ticker_id` is not directly available in the component, it must be threaded from the home page where the ticker was resolved.

- [ ] Task 7: Backend tests for chart image (AC: #1, #2, #3)
  - [ ] 7.1 Test: `can_create_snapshot_with_chart_image` — POST with base64-encoded PNG string → verify `chart_image` is stored (non-null in DB)
  - [ ] 7.2 Test: `can_create_snapshot_without_chart_image` — POST with `chart_image: null` → verify snapshot created, `chart_image` is NULL (AC #2)
  - [ ] 7.3 Test: `can_retrieve_chart_image` — GET `/api/v1/snapshots/:id/chart-image` for snapshot with image → returns 200 with `Content-Type: image/png`
  - [ ] 7.4 Test: `returns_404_for_missing_chart_image` — GET `/api/v1/snapshots/:id/chart-image` for snapshot without image → returns 404
  - [ ] 7.5 Test: `rejects_oversized_chart_image` — POST with chart_image exceeding 5 MB → returns 400
  - [ ] 7.6 Regression: all 10 existing snapshot tests still pass (chart_image is Optional, so existing tests sending no chart_image must still work)
  - [ ] 7.7 Regression assertion: in existing `can_create_snapshot` test, add `assert!(created.chart_image.is_none())` to confirm that snapshots created without chart_image explicitly have NULL chart_image

- [ ] Task 8: Verification (AC: all)
  - [ ] 8.1 `cargo check` (full workspace) — both backend and frontend
  - [ ] 8.2 `cargo test -p backend -- snapshots` — all snapshot tests pass (10 existing + 5 new = 15)
  - [ ] 8.3 `trunk build` (frontend) — no compilation errors
  - [ ] 8.4 Manual verification in browser: lock a thesis → check DB for chart_image not null
  - [ ] 8.5 `cargo doc --no-deps -p backend` — passes

## Dev Notes

### Critical Architecture Constraints

**Cardinal Rule:** All calculation logic lives in `crates/naic-logic`. This story touches the **frontend rendering pipeline** and **backend API** — no calculation logic is involved. No naic-logic changes needed.

**Architecture Decision (Option A — Lock-time browser capture):** This is the resolved architecture decision from the party mode review (2026-02-11). Zero server infrastructure. Image captured client-side via ECharts export API when user locks a thesis. Non-blocking — if capture fails, snapshot saves without image.

**Append-Only Model:** Still applies. The snapshot with chart_image is created via POST (new row). Never updated after creation.

**Immutability Contract:** Locked snapshots (which are the only ones with chart images) are immutable. The chart image cannot be modified after creation.

**Legacy Endpoint Migration:** The lock thesis modal (`lock_thesis_modal.rs`) currently POSTs to `/api/analyses/lock`. This story migrates it to `POST /api/v1/snapshots` with `thesis_locked: true`. The legacy endpoint remains untouched for backward compatibility but the frontend no longer uses it for locking.

### CRITICAL: ECharts Chart Image Capture Mechanism

**How it works:**
1. ECharts 5.4.3 is loaded from CDN in `frontend/public/index.html`
2. `charming` v0.3 `WasmRenderer` renders charts into DOM elements via `echarts.init()`
3. The ECharts instance is recoverable from the DOM via `echarts.getInstanceByDom(domElement)`
4. `chart.getDataURL({ type: 'png', pixelRatio: 2 })` exports the chart as a base64 data URL
5. **charming does NOT expose getDataURL** — must go through JS interop

**Established JS interop pattern (chart_bridge.js):**
```javascript
// EXISTING pattern — setupDraggableHandles already uses echarts.getInstanceByDom()
window.setupDraggableHandles = function (chartId, ...) {
    const chartDom = document.getElementById(chartId);
    let chart = echarts.getInstanceByDom(chartDom);
    // ... uses chart.getOption(), chart.setOption()
};
```

**NEW function to add (same file, same pattern):**
```javascript
window.captureChartAsDataURL = function (chartId) {
    const chartDom = document.getElementById(chartId);
    if (!chartDom) {
        console.warn('[captureChartAsDataURL] DOM element not found:', chartId);
        return null;
    }
    let chart = echarts.getInstanceByDom(chartDom);
    if (!chart) {
        console.warn('[captureChartAsDataURL] No ECharts instance for:', chartId);
        return null;
    }
    try {
        const bg = (chart.getOption().backgroundColor) || '#1a1a2e';
        return chart.getDataURL({ type: 'png', pixelRatio: 2, backgroundColor: bg });
    } catch (e) {
        console.warn('[captureChartAsDataURL] Export failed:', e);
        return null;
    }
};
```

**Rust FFI binding (same pattern as existing setupDraggableHandles):**
```rust
#[wasm_bindgen]
unsafe extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn captureChartAsDataURL(chart_id: String) -> Option<String>;
}
```

**Chart container ID format:** `ssg-chart-{ticker.to_lowercase()}` — e.g., `ssg-chart-aapl` (see `ssg_chart.rs` line ~60).

### Data Flow: Frontend → Backend

```
[ECharts] → getDataURL() → "data:image/png;base64,iVBOR..."
    → strip prefix → "iVBOR..." (raw base64 string)
    → include in JSON body as chart_image: "iVBOR..."
    → POST /api/v1/snapshots

[Backend] → receive CreateSnapshotRequest { chart_image: Some("iVBOR...") }
    → base64::decode("iVBOR...") → Vec<u8> (raw PNG bytes)
    → store in chart_image MEDIUMBLOB column (up to 16MB)

[Retrieval] → GET /api/v1/snapshots/:id/chart-image
    → read chart_image Vec<u8> from DB
    → return raw bytes with Content-Type: image/png
```

### Backend `base64` Crate Usage

The `base64` crate is NOT currently in the backend's dependencies. Add it:
```toml
# backend/Cargo.toml
base64 = "0.22"
```

Decoding pattern:
```rust
use base64::Engine;

let chart_image_bytes: Option<Vec<u8>> = req.chart_image
    .as_deref()
    .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64).ok());
```

### Frontend Component Wiring Changes

**Current lock flow:**
```
AnalystHUD → [Lock button] → LockThesisModal → POST /api/analyses/lock
                                                 ↑ sends LockRequest { ticker, snapshot, analyst_note }
```

**New lock flow (this story):**
```
AnalystHUD → [Lock button] → LockThesisModal → capture_chart_image(chart_id) → POST /api/v1/snapshots
  ↑ passes chart_id, ticker_id      ↑ sends CreateSnapshotPayload { ticker_id, snapshot_data, thesis_locked: true,
                                                                     notes, chart_image }
```

**LockThesisModal prop changes:**
- ADD: `chart_id: String` — the DOM ID of the chart container
- ADD: `ticker_id: i32` — the database ticker ID (needed by `/api/v1/snapshots`)
- KEEP: all existing props (historical_data, projections, on_close, on_locked)
- The ticker symbol string may still be needed for display but the API call uses `ticker_id`

**AnalystHUD changes:**
- Pass `chart_id=format!("ssg-chart-{}", data.ticker.to_lowercase())` to modal
- Pass `ticker_id` — verify how the ticker ID is available in the component. The `home.rs` page resolves the ticker from the API; the ticker's numeric ID should be available in the data flow.

### Existing File Patterns for Reference

**Existing FFI in `ssg_chart.rs` (lines 13-17):**
```rust
#[wasm_bindgen]
unsafe extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn setupDraggableHandles(chart_id: String, ...);
}
```

**Existing chart render call (ssg_chart.rs ~line 233-234):**
```rust
let renderer = WasmRenderer::new(chart_width, chart_height);
renderer.render(&cid, &chart).unwrap();
```

**Existing lock handler (lock_thesis_modal.rs ~line 50-95):**
Uses `gloo_net::http::Request::post("/api/analyses/lock")` with `LockRequest` body containing ticker symbol string, `AnalysisSnapshot`, and analyst note.

### Error Response Pattern (for chart-image endpoint)

Follow the 404 pattern from `get_snapshot`:
```rust
pub async fn get_snapshot_chart_image(
    State(ctx): State<AppContext>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let snapshot = analysis_snapshots::Entity::find_by_id(id)
        .filter(analysis_snapshots::Column::DeletedAt.is_null())
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    match snapshot.chart_image {
        Some(bytes) => Ok(Response::builder()
            .header("Content-Type", "image/png")
            .body(bytes.into())
            .map_err(|e| Error::string(&e.to_string()))?),
        None => Err(Error::NotFound),
    }
}
```

### Project Structure Notes

Files to MODIFY:
- `frontend/public/chart_bridge.js` — Add `captureChartAsDataURL()` JS function
- `frontend/src/components/ssg_chart.rs` — Add `wasm_bindgen` FFI for `captureChartAsDataURL`, add `capture_chart_image()` helper
- `frontend/src/components/lock_thesis_modal.rs` — Add props, migrate to new endpoint, capture chart image
- `frontend/src/components/analyst_hud.rs` — Pass `chart_id` and `ticker_id` props to modal
- `backend/Cargo.toml` — Add `base64 = "0.22"` dependency
- `backend/src/controllers/snapshots.rs` — Add `chart_image` to `CreateSnapshotRequest`, add chart-image endpoint, register route
- `backend/tests/requests/snapshots.rs` — Add 4 new tests for chart image

Files NOT to modify:
- `crates/naic-logic/` — No calculation logic involved
- `backend/src/controllers/analyses.rs` — Legacy endpoints stay as-is (still functional, just no longer used by lock flow)
- `backend/src/models/_entities/analysis_snapshots.rs` — Entity already has `chart_image` field (Story 7.2)
- `backend/migration/` — No schema changes; `chart_image` MEDIUMBLOB column already exists (Story 7.2)

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 7, Story 7.4]
- [Source: _bmad-output/planning-artifacts/architecture.md — Open Decision: Static Chart Image Capture, Option A recommended]
- [Source: _bmad-output/planning-artifacts/architecture.md — Party Mode Design Decisions #3: Lock-time browser capture resolved]
- [Source: frontend/public/chart_bridge.js — Existing JS interop for ECharts instance access via echarts.getInstanceByDom()]
- [Source: frontend/src/components/ssg_chart.rs — Chart rendering pipeline, WasmRenderer, chart container ID format]
- [Source: frontend/src/components/lock_thesis_modal.rs — Current lock flow, LockRequest struct, POST /api/analyses/lock]
- [Source: frontend/src/components/analyst_hud.rs — Modal instantiation, prop passing]
- [Source: backend/src/controllers/snapshots.rs — CreateSnapshotRequest, create_snapshot handler]
- [Source: backend/src/models/_entities/analysis_snapshots.rs — chart_image: Option<Vec<u8>> field]
- [Source: backend/migration/src/m20260212_000001_analysis_snapshots.rs — chart_image MEDIUMBLOB column]
- [Source: ECharts API — echartsInstance.getDataURL() method]

### Previous Story Learnings (from Story 7.3)

- `seed::<App>()` does NOT work with MySQL backend in Loco 0.16 — use direct `ActiveModel::insert` for test seeding
- MEDIUMBLOB in SeaORM entity uses `column_type = "VarBinary(StringLen::None)"` since there's no MediumBlob variant
- 403 Forbidden pattern: Use `Response::builder().status(403)` since Loco's `Error` enum lacks Forbidden
- `ActiveValue::set()` for all fields, `..Default::default()` for remaining
- The `#[serial]` attribute from `serial_test` crate ensures test isolation
- Code review added PATCH route for AC5 compliance — remember to handle PATCH for any new endpoint that needs it
- Tests should assert both status code AND response body for error cases

### Dual-Compilation Requirement

This story touches both **backend** (Rust server) and **frontend** (Rust → WASM via `trunk`). The developer must verify compilation in both targets:
- `cargo check` — checks the full workspace including backend crates
- `trunk build` — compiles the frontend to WASM (uses different target, different feature flags)

A change that compiles under `cargo check` may fail under `trunk build` (e.g., missing `wasm` feature gate, incompatible dependency). **Always run both** before considering a task complete.

### Recommended Task Execution Order

While tasks are numbered sequentially, the recommended execution order minimizes rework and catches data-flow issues early:

1. **Task 6 (investigate ticker_id)** — Confirm `ticker_id` availability FIRST; if threading is needed, it affects Tasks 5 and 6
2. **Task 3 (backend CreateSnapshotRequest)** — Backend changes are independently testable
3. **Task 4 (chart-image endpoint)** — Also backend, can be tested immediately
4. **Task 7 (backend tests)** — Validate Tasks 3-4 with automated tests before touching frontend
5. **Task 1 (JS bridge)** — Frontend JS layer
6. **Task 2 (Rust FFI)** — Frontend Rust bindings
7. **Task 5 (lock modal migration)** — Wire everything together in the modal
8. **Task 8 (verification)** — Full-stack verification

### Non-Functional Requirements

- **NFR6**: Snapshot creation with chart image should still complete in < 2 seconds. The base64 decoding adds negligible overhead. The chart image (PNG, ~100-500KB at pixelRatio 2) is well within MEDIUMBLOB's 16MB limit.
- **Non-blocking capture**: Chart image capture MUST NOT block or slow down the lock thesis flow. If it fails, proceed without image.

### Definition of Done

- [ ] `captureChartAsDataURL()` JS function added to `chart_bridge.js`
- [ ] Rust FFI binding and `capture_chart_image()` helper in `ssg_chart.rs`
- [ ] `CreateSnapshotRequest` accepts optional base64 `chart_image` field
- [ ] `create_snapshot` decodes base64 and stores raw PNG bytes in DB
- [ ] `GET /api/v1/snapshots/:id/chart-image` returns raw PNG with correct Content-Type
- [ ] Lock thesis modal captures chart image and POSTs to `/api/v1/snapshots`
- [ ] Failed capture gracefully falls back to `chart_image: null` (AC #2)
- [ ] `AnalystHUD` passes `chart_id` and `ticker_id` to modal
- [ ] All 15+ backend tests pass (10 existing + 5 new chart image tests)
- [ ] `cargo check` (full workspace) passes
- [ ] `trunk build` (frontend) compiles
- [ ] Legacy `/api/analyses/*` endpoints remain functional

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List

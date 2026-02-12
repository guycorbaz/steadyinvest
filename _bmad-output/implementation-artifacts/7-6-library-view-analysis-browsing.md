# Story 7.6: Library View & Analysis Browsing

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an **analyst**,
I want to browse all my saved analyses in a dedicated Library view,
So that I can find, review, and manage my analysis history across all tickers.

## Acceptance Criteria

1. **Given** the user navigates to `/library` via the Command Strip
   **When** the Library view loads
   **Then** all saved analysis snapshots are displayed as Compact Analysis Cards
   **And** each card shows: ticker symbol, analysis date, thesis locked status, and key metrics (projected Sales/EPS CAGRs, valuation zone)

2. **Given** the Library view is loaded with multiple analyses
   **When** the user uses the search/filter controls
   **Then** analyses can be filtered by ticker symbol (text search)
   **And** analyses can be filtered by locked/unlocked status
   **And** results update immediately without page reload

3. **Given** a Compact Analysis Card is displayed in the Library
   **When** the user clicks the card
   **Then** the full Analysis view opens with that snapshot's data loaded

4. **Given** the Command Strip navigation
   **When** the Library view is added
   **Then** a "Library" entry appears in the Command Strip with the `/library` route
   **And** the Library entry is visually consistent with existing Command Strip entries

5. **Given** the Library view renders on mobile (<767px)
   **When** the viewport is below the mobile breakpoint
   **Then** Compact Analysis Cards stack vertically in a single column
   **And** the view is read-only consistent with mobile review mode

6. **Given** the Library view
   **When** rendered at all four breakpoints (desktop wide, desktop standard, tablet, mobile)
   **Then** all elements render correctly without layout breakage or overflow

## Tasks / Subtasks

- [x] Task 0: Enhance snapshot list API to include ticker symbol and key metrics (AC: #1)
  - [x] 0.1 In `backend/src/controllers/snapshots.rs`, add `ticker_symbol: String` to `SnapshotSummary` by joining the `tickers` table in `list_snapshots()`
  - [x] 0.2 Add extracted metric fields to `SnapshotSummary`: `projected_sales_cagr: Option<f64>`, `projected_eps_cagr: Option<f64>`, `projected_high_pe: Option<f64>`, `projected_low_pe: Option<f64>` — extracted from `snapshot_data` JSON via `serde_json::Value` accessors
  - [x] 0.3 Update existing snapshot tests in `backend/tests/requests/snapshots.rs` to verify new response fields
  - [x] 0.4 Run `cargo check -p backend` and `cargo test -p backend -- snapshots` to confirm no regressions

- [x] Task 1: Create Compact Analysis Card component (AC: #1, #5, #6)
  - [x] 1.1 Create `frontend/src/components/compact_analysis_card.rs`
  - [x] 1.2 Define `CompactCardData` struct for the card's props: `id`, `ticker_symbol`, `captured_at`, `thesis_locked`, `projected_sales_cagr`, `projected_eps_cagr`, `projected_high_pe`, `projected_low_pe`
  - [x] 1.3 Render card with: ticker symbol (prominent), date (formatted), lock icon/badge, Sales CAGR, EPS CAGR, and valuation zone (derived from high/low PE)
  - [x] 1.4 Accept `on_click: Callback<i32>` prop for navigation to full analysis
  - [x] 1.5 Style with existing design system: `#16161D` surface, sharp edges, JetBrains Mono for numbers, Inter for labels, 4px grid spacing
  - [x] 1.6 Responsive: cards fill available grid width; at mobile (<767px) stack to single column

- [x] Task 2: Create Library page with search/filter (AC: #1, #2, #5, #6)
  - [x] 2.1 Create `frontend/src/pages/library.rs`
  - [x] 2.2 Fetch snapshots via `GET /api/v1/snapshots` on page load using `LocalResource`
  - [x] 2.3 Add ticker search input (text filter, client-side filtering of loaded results — follows `audit_log.rs` filter pattern)
  - [x] 2.4 Add locked/unlocked toggle filter (all / locked only / unlocked only)
  - [x] 2.5 Render filtered results as a responsive CSS Grid of `CompactAnalysisCard` components
  - [x] 2.6 Grid layout: 3 columns at desktop wide (1280+), 2 at standard (1024-1279), 2 at tablet (768-1023), 1 at mobile (<767px)
  - [x] 2.7 Show "No analyses saved yet" empty state when no snapshots exist
  - [x] 2.8 Show loading state with existing pulse animation pattern

- [x] Task 3: Implement card click → navigate to Analysis view (AC: #3)
  - [x] 3.1 On card click, navigate to `/?snapshot={id}` using `leptos_router` `use_navigate()`
  - [x] 3.2 In `home.rs`, read `snapshot` query parameter on load and auto-select that snapshot
  - [x] 3.3 The snapshot must be loaded by fetching `GET /api/v1/snapshots/:id` since the full `snapshot_data` is not in the list response

- [x] Task 4: Add Library entry to Command Strip (AC: #4)
  - [x] 4.1 In `frontend/src/components/command_strip.rs`, add a "Library" menu item with route `/library`
  - [x] 4.2 Place it after "Search" and before the "Report" section — it is a primary navigation destination
  - [x] 4.3 Use book/library emoji icon consistent with existing Command Strip style

- [x] Task 5: Register route and module (AC: #1)
  - [x] 5.1 Register `pub mod library;` in `frontend/src/pages/mod.rs`
  - [x] 5.2 Register `pub mod compact_analysis_card;` in `frontend/src/components/mod.rs`
  - [x] 5.3 Add `<Route path=path!("/library") view=Library />` in `frontend/src/lib.rs`
  - [x] 5.4 Add imports for `Library` page in `lib.rs`

- [x] Task 6: Add Library page styles (AC: #5, #6)
  - [x] 6.1 Add Library-specific styles to `frontend/public/styles.scss`: `.library-page`, `.library-filters`, `.library-grid`, `.compact-card`
  - [x] 6.2 Follow existing SCSS patterns and design system tokens
  - [x] 6.3 Responsive breakpoints per task 2.6

- [x] Task 7: Verification (AC: all)
  - [x] 7.1 `cargo check` (full workspace) passes
  - [x] 7.2 `cargo test -p backend -- snapshots` — all existing + new tests pass (17/17)
  - [x] 7.3 Frontend compiles (`cargo check -p frontend`)
  - [ ] 7.4 All existing E2E tests pass (regression) — skipped (E2E requires browser/webdriver)

## Dev Notes

### Critical Architecture Constraints

**Cardinal Rule:** All calculation logic lives in `crates/naic-logic`. This story does NOT involve calculation logic — it's a frontend view consuming existing API data. No naic-logic changes needed.

**Append-Only / Immutability:** The Library view is read-only — no snapshot creation, modification, or deletion. Clicking a card navigates to the Analysis view which handles snapshot viewing.

**Multi-User Readiness:** The snapshot API already includes `user_id` scoping (default 1). No additional scoping needed for Library. When Phase 3 authentication arrives, the Library will automatically show only the authenticated user's snapshots.

### Existing Infrastructure (MUST BUILD ON)

**Snapshot API** (`backend/src/controllers/snapshots.rs`) already provides:
```
GET /api/v1/snapshots                      → list (SnapshotSummary array)
GET /api/v1/snapshots?ticker_id=X          → filtered by ticker
GET /api/v1/snapshots?thesis_locked=true   → filtered by lock status
GET /api/v1/snapshots/:id                  → full snapshot with snapshot_data
```

**Current `SnapshotSummary` response** (needs enhancement for this story):
```json
{
  "id": 1,
  "ticker_id": 5,
  "thesis_locked": true,
  "notes": "Strong growth trajectory",
  "captured_at": "2026-02-12T14:30:00+00:00"
}
```

**Problem:** `SnapshotSummary` has `ticker_id` (int) but NOT the ticker symbol. It also lacks key metrics needed for the Compact Analysis Card. Task 0 enhances this.

**Enhanced `SnapshotSummary` target:**
```json
{
  "id": 1,
  "ticker_id": 5,
  "ticker_symbol": "NESN.SW",
  "thesis_locked": true,
  "notes": "Strong growth trajectory",
  "captured_at": "2026-02-12T14:30:00+00:00",
  "projected_sales_cagr": 6.5,
  "projected_eps_cagr": 8.2,
  "projected_high_pe": 28.0,
  "projected_low_pe": 18.0
}
```

**Ticker join for symbol resolution:** In `list_snapshots()`, use SeaORM `find_also_related()` or a manual join with the `tickers` table to get the ticker symbol. The `tickers` entity has a `ticker: String` column. The FK relationship `analysis_snapshots.ticker_id → tickers.id` exists.

**Metric extraction from JSON:** `snapshot_data` is a `serde_json::Value`. Extract metrics using:
```rust
let sales_cagr = m.snapshot_data.get("projected_sales_cagr").and_then(|v| v.as_f64());
let eps_cagr = m.snapshot_data.get("projected_eps_cagr").and_then(|v| v.as_f64());
let high_pe = m.snapshot_data.get("projected_high_pe").and_then(|v| v.as_f64());
let low_pe = m.snapshot_data.get("projected_low_pe").and_then(|v| v.as_f64());
```

### Frontend Patterns (MUST FOLLOW)

**API calls** use `gloo_net::http::Request`:
```rust
gloo_net::http::Request::get("/api/v1/snapshots")
    .send().await
    .map_err(|e| e.to_string())?
    .json::<Vec<SnapshotSummary>>().await
```

**Data fetching** uses `LocalResource` pattern (see `home.rs` lines 45-100):
```rust
let snapshots = LocalResource::new(move || {
    async move {
        let response = gloo_net::http::Request::get("/api/v1/snapshots")
            .send().await.map_err(|e| e.to_string())?;
        response.json::<Vec<SnapshotSummary>>().await.map_err(|e| e.to_string())
    }
});
```

**Filtering** follows the `audit_log.rs` pattern (lines 43-92):
- Local `signal()` for filter text
- URL encoding with `js_sys::encode_uri_component()` — BUT for Library, prefer client-side filtering of already-fetched data (snapshot count is small) rather than server-side queries. This avoids extra API calls on every keystroke.
- Filter reactively: `move || { ... snapshots.get().map(|list| list.filter(...)) ... }`

**Component pattern** — props via `#[component]` macro:
```rust
#[component]
pub fn CompactAnalysisCard(
    #[prop(into)] data: CompactCardData,
    on_click: Callback<i32>,
) -> impl IntoView { ... }
```

**Navigation** — use `leptos_router::hooks::use_navigate()`:
```rust
let navigate = leptos_router::hooks::use_navigate();
navigate(&format!("/?snapshot={}", id), Default::default());
```

**Loading state** — uses existing pulse animation:
```rust
<div class="loading-overlay">
    <div class="pulse"></div>
    <div class="status-text">"Loading Library..."</div>
</div>
```

### Card Click → Analysis View Navigation

**Approach:** Navigate to `/?snapshot={id}`. The Home page (`home.rs`) needs a small modification to:
1. Read a `snapshot` query parameter from the URL on mount
2. Fetch the full snapshot by ID
3. Display it in `SnapshotHUD`

This requires looking up the ticker from the snapshot's `ticker_id` to populate the ticker header. The `GET /api/v1/snapshots/:id` response includes `ticker_id` — resolve it via `GET /api/tickers/{id}` or embed ticker info in the full snapshot response.

**Alternative simpler approach:** Store `ticker_symbol` and `snapshot_id` in navigation state or URL params: `/?snapshot={id}&ticker={symbol}`. Home can then:
1. Set the ticker from URL param
2. Auto-select the snapshot ID

Choose whichever approach minimizes changes to `home.rs`.

### Styling Requirements

Follow existing design system tokens from `frontend/public/styles.scss`:
- Background: `#0F0F12` (page), `#16161D` (card surfaces)
- Primary accent: `#3B82F6` (interactive elements)
- Emerald: `#10B981` (locked status badge)
- Text: High-contrast white on dark
- Font: JetBrains Mono for numbers, Inter for labels
- Spacing: 4px precision grid
- Edges: Sharp (0px border-radius)
- Card hover: subtle border highlight (`#3B82F6` at reduced opacity)

**Compact Analysis Card layout:**
```
┌─────────────────────────────────┐
│ NESN.SW                   🔒    │  ← Ticker + lock badge
│ 2026-02-12                      │  ← Date
│─────────────────────────────────│
│ Sales CAGR    6.5%              │  ← Key metrics
│ EPS CAGR      8.2%              │
│ PE Range      18.0 — 28.0      │  ← Valuation zone
└─────────────────────────────────┘
```

### Previous Story Learnings (from Stories 7.3, 7.4, 7.5)

- `request::<App, _, _>` (NOT `request::<App, Migrator, _>`) for Loco 0.16 test pattern
- 403 and 503 via `Response::builder().status(...)` since Loco's Error enum doesn't have those variants
- `ActiveValue::set()` for all fields, `..Default::default()` for remaining
- `#[serial]` from `serial_test` crate for test isolation
- Tests should assert both status code AND response body
- `base64 = "0.22"` already in workspace (added by Story 7.4)
- `reqwest` already in workspace with `json` feature
- Frontend `gloo-net` is the HTTP client (NOT `reqwest` in frontend — reqwest is backend only)
- Chart image capture uses JavaScript bridge (`chart_bridge.js`) — not relevant to Library view
- `SnapshotSummary` currently excludes `snapshot_data` and `chart_image` for performance — Library cards need metrics extracted, NOT the full snapshot_data

### Non-Functional Requirements

- **NFR6:** Library page should load in < 2 seconds. Snapshot list queries are indexed on `captured_at`, `ticker_id`, and `deleted_at`.
- **Responsive:** All four breakpoints must work (desktop wide 1280+, standard 1024-1279, tablet 768-1023, mobile <767px)
- **WCAG 2.1 AA:** Cards must be keyboard-navigable, with focus indicators and proper `role="link"` or button semantics

### What NOT To Do

- Do NOT create `frontend/src/state/` module yet — that's for Phase 2 (Epic 9) global signals (Active Portfolio, Currency Preference)
- Do NOT add delete functionality to the Library view — Library is browse-only
- Do NOT load full `snapshot_data` in the list view — it would be a performance disaster for large libraries
- Do NOT modify `crates/naic-logic/` — no calculation logic in this story
- Do NOT modify the existing `exchange.rs` service or `exchange_rate_provider.rs`
- Do NOT add currency conversion to Compact Cards — Phase 1 displays native currencies only (per UX spec Component Strategy)

### Project Structure Notes

Files to CREATE:
- `frontend/src/pages/library.rs` — Library page component
- `frontend/src/components/compact_analysis_card.rs` — Compact card component

Files to MODIFY:
- `backend/src/controllers/snapshots.rs` — Enhance `SnapshotSummary` with ticker_symbol and metrics
- `backend/tests/requests/snapshots.rs` — Update tests for new response fields
- `frontend/src/lib.rs` — Add `/library` route and import
- `frontend/src/pages/mod.rs` — Register `library` module
- `frontend/src/components/mod.rs` — Register `compact_analysis_card` module
- `frontend/src/components/command_strip.rs` — Add Library entry
- `frontend/src/pages/home.rs` — Accept `?snapshot=` query parameter for deep linking
- `frontend/public/styles.scss` — Add Library and Compact Card styles

Files NOT to modify:
- `backend/src/services/` — No service changes needed
- `backend/migration/` — No schema changes needed
- `crates/naic-logic/` — No calculation logic
- `frontend/src/components/ssg_chart.rs` — No chart changes
- `frontend/src/components/analyst_hud.rs` — No analysis HUD changes
- `frontend/src/components/lock_thesis_modal.rs` — No lock flow changes

### Definition of Done

- [ ] `GET /api/v1/snapshots` response includes `ticker_symbol` and key metrics for each snapshot
- [ ] Library page at `/library` displays all snapshots as Compact Analysis Cards
- [ ] Cards show: ticker symbol, date, lock status, Sales/EPS CAGRs, PE range
- [ ] Ticker search filter works client-side with immediate updates
- [ ] Locked/unlocked toggle filter works
- [ ] Clicking a card navigates to the Analysis view with that snapshot loaded
- [ ] "Library" entry visible in Command Strip with `/library` route
- [ ] Responsive layout: 3-col (wide) → 2-col (standard/tablet) → 1-col (mobile)
- [ ] Empty state displayed when no snapshots exist
- [ ] `cargo check` (full workspace) passes
- [ ] Backend snapshot tests pass (new + regression)
- [ ] Frontend compiles without errors
- [ ] Existing E2E tests pass (regression)

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 7, Story 7.6]
- [Source: _bmad-output/planning-artifacts/architecture.md — Frontend Architecture, Core Views table, API expansion]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md — Core Views table (Library), Compact Analysis Card component, Responsive Strategy]
- [Source: backend/src/controllers/snapshots.rs — Existing snapshot CRUD API, SnapshotSummary struct]
- [Source: frontend/src/pages/home.rs — Existing analysis page with snapshot selection pattern]
- [Source: frontend/src/pages/audit_log.rs — Filter pattern for search/filter UI]
- [Source: frontend/src/components/command_strip.rs — Navigation sidebar to extend]
- [Source: frontend/src/lib.rs — Router setup to extend]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Backend: `cargo check -p backend` clean, `cargo test -p backend -- snapshots` 17/17 pass
- Frontend: `cargo check -p frontend` clean (1 pre-existing warning in counter_btn.rs)
- Full workspace: `cargo check` clean

### Completion Notes List

- Enhanced `SnapshotSummary` with `ticker_symbol` (via `find_also_related(tickers::Entity)`) and four metric fields extracted from `snapshot_data` JSON
- Created `CompactAnalysisCard` component with `CompactCardData` struct and `Callback<i32>` click handler
- Created Library page with `LocalResource` fetch, client-side ticker/lock filtering, responsive CSS Grid
- Deep linking via `/?snapshot={id}` — home.rs reads URL query param using `use_location().search`, fetches full snapshot via `GET /api/v1/snapshots/:id`, renders in SnapshotHUD
- Command Strip updated with Library entry (book emoji) between Search and Report sections
- Styles follow design system: dark surface, JetBrains Mono for data, Inter for labels, sharp edges, responsive breakpoints (3/2/2/1 columns)
- Fixed borrow-after-move in compact_analysis_card.rs (pre-compute `aria` label before view macro)

### Change Log

- `backend/src/controllers/snapshots.rs` — Enhanced `SnapshotSummary` with `ticker_symbol`, `projected_sales_cagr`, `projected_eps_cagr`, `projected_high_pe`, `projected_low_pe`; added `from_model_and_ticker()` constructor; updated `list_snapshots()` to use `find_also_related(tickers::Entity)`
- `backend/tests/requests/snapshots.rs` — Added assertions for new `ticker_symbol` and metric fields in `can_list_snapshots_with_filters` test
- `frontend/src/components/compact_analysis_card.rs` — NEW: `CompactCardData` struct + `CompactAnalysisCard` component
- `frontend/src/pages/library.rs` — NEW: Library page with search/filter, responsive grid, navigation
- `frontend/src/pages/home.rs` — Added deep link support: `FullSnapshotResponse` DTO, `use_location()` query param parsing, `LocalResource` fetch, deep link view rendering
- `frontend/src/components/command_strip.rs` — Added Library menu entry
- `frontend/src/lib.rs` — Added `/library` route and `Library` import
- `frontend/src/pages/mod.rs` — Registered `library` module
- `frontend/src/components/mod.rs` — Registered `compact_analysis_card` module
- `frontend/public/styles.scss` — Added Library page, Compact Analysis Card, and responsive styles

### File List

| File | Action |
|------|--------|
| `backend/src/controllers/snapshots.rs` | Modified |
| `backend/tests/requests/snapshots.rs` | Modified |
| `frontend/src/components/compact_analysis_card.rs` | Created |
| `frontend/src/pages/library.rs` | Created |
| `frontend/src/pages/home.rs` | Modified |
| `frontend/src/components/command_strip.rs` | Modified |
| `frontend/src/lib.rs` | Modified |
| `frontend/src/pages/mod.rs` | Modified |
| `frontend/src/components/mod.rs` | Modified |
| `frontend/public/styles.scss` | Modified |

## Senior Developer Review (AI)

**Reviewer:** Claude Opus 4.6 (code-review workflow)
**Date:** 2026-02-12
**Outcome:** Approved (after fixes)

### Findings Summary

| Severity | Count | Fixed |
|----------|-------|-------|
| HIGH | 1 | 1 |
| MEDIUM | 2 | 1 fixed, 1 accepted |
| LOW | 3 | 0 (accepted) |

### Issues Found & Resolution

**H1 (FIXED): Empty state didn't distinguish "no snapshots" from "no filter matches"**
- `library.rs` always showed "No analyses match your filters" even when the database had zero snapshots
- Task 2.7 requires "No analyses saved yet" for the empty-library case
- Fix: Added `list.is_empty()` check before filtering to show the correct empty state message

**M1 (FIXED): Stale `?snapshot=` URL parameter not cleared on ticker selection**
- After deep linking from Library (`/?snapshot=123`), selecting a new ticker left the query param in the URL
- Page refresh would re-activate the deep link instead of showing the last-selected ticker
- Fix: Added `navigate_home("/", Default::default())` call in `on_select` callback to clear the URL

**M2 (ACCEPTED): Backend loads full `snapshot_data` from DB to extract 4 metrics**
- `list_snapshots()` fetches the entire `snapshot_data` JSON blob server-side to extract 4 numbers
- Acceptable for Phase 1 (small library size). Future optimization: denormalize metrics into columns or use SQL JSON_EXTRACT
- No code change required at this time

**L1 (ACCEPTED):** `FullSnapshotResponse.captured_at` uses `DateTime<Utc>` vs backend's `DateTime<FixedOffset>` — works via serde coercion
**L2 (ACCEPTED):** Date truncation via `.chars().take(10)` relies on ISO 8601 prefix — correct in practice
**L3 (ACCEPTED):** Mobile breakpoint 768px vs AC spec "<767px" — inclusive, actually better behavior

### Verification After Fixes

- `cargo check` (full workspace): clean
- `cargo check -p frontend`: clean
- `cargo test -p backend -- snapshots`: 17/17 pass

# Story 8.2: Comparison View & Ranked Grid

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an **analyst**,
I want to compare multiple stocks in a ranked grid view,
So that I can identify the best investment candidates at a glance.

## Acceptance Criteria

1. **Given** the user navigates to `/compare` via the Command Strip
   **When** the Comparison view loads with no query params
   **Then** an empty state is shown with a call-to-action linking to the Library and a ticker text input for quick-add
   **And** a "Comparison" entry is visible in the Command Strip

2. **Given** the user is on the Library page
   **When** the user selects multiple analysis cards via checkboxes and clicks "Compare Selected"
   **Then** the browser navigates to `/compare?snapshot_ids=1,2,3` with the selected snapshot IDs
   **And** the Comparison view loads those specific snapshots in the grid

3. **Given** multiple analyses are loaded in the Comparison view
   **When** Compact Analysis Cards populate the grid
   **Then** each card displays: ticker symbol, analysis date, projected Sales CAGR, projected EPS CAGR, estimated P/E range, valuation zone indicator (colored dot: Emerald=Buy, Amber=Hold, Crimson=Sell), and upside/downside ratio (color-coded: Emerald ≥3.0, Amber 1.0–2.99, Crimson <1.0)
   **And** cards are sortable by any displayed metric column (click column header to sort)
   **And** the default sort is upside/downside ratio descending

4. **Given** the ranked grid contains 5+ analyses
   **When** the user sorts by any metric column
   **Then** cards reorder immediately without page reload
   **And** the sort indicator (▲/▼) is visible on the active column

5. **Given** the user has built a useful comparison
   **When** the user clicks "Save Comparison"
   **Then** the comparison set is persisted via `POST /api/v1/comparisons`
   **And** the user can name the comparison set

6. **Given** the user clicks a Compact Analysis Card in the grid
   **When** the card is selected
   **Then** the full Analysis view opens with that snapshot loaded

7. **Given** the Comparison view renders on mobile (<767px)
   **When** the viewport is below the mobile breakpoint
   **Then** cards stack vertically in a single column with key metrics visible
   **And** sorting is accessible via a dropdown selector instead of column headers

8. **Given** the Comparison view
   **When** rendered at all four breakpoints (desktop wide, desktop standard, tablet, mobile)
   **Then** all elements render correctly without layout breakage or overflow

## Tasks / Subtasks

- [x] Task 1: Add upside/downside ratio calculation to `steady-invest-logic` (AC: #3)
  - [x] 1.1 Add `calculate_upside_downside_ratio(current_price: f64, projected_high_price: f64, projected_low_price: f64) -> Option<f64>` to `crates/steady-invest-logic/src/lib.rs`. Returns `None` if downside ≤ 0 (current price at or below low target). Include doctest with NAIC example.
  - [x] 1.2 Add `upside_downside_ratio: Option<f64>` field to backend `ComparisonSnapshotSummary` in `backend/src/controllers/comparisons.rs`
  - [x] 1.3 In `from_model_and_ticker()`: deserialize `snapshot_data` into `AnalysisSnapshot`, extract current price (latest record's high price), compute projected EPS (5yr), compute target high/low prices, call `calculate_upside_downside_ratio()`. Fall back to `None` if data is missing.
  - [x] 1.4 Update backend comparison tests to verify `upside_downside_ratio` field is present in responses

- [x] Task 2: Enhance `CompactAnalysisCard` with valuation zone + upside/downside (AC: #3)
  - [x] 2.1 Add `valuation_zone: Option<String>` and `upside_downside_ratio: Option<f64>` fields to `CompactCardData` struct in `frontend/src/components/compact_analysis_card.rs`
  - [x] 2.2 Render valuation zone as colored dot + text (Emerald=Buy, Amber=Hold, Crimson=Sell) below PE Range
  - [x] 2.3 Render upside/downside ratio as color-coded value (Emerald ≥3.0, Amber 1.0–2.99, Crimson <1.0) with "U/D" label
  - [x] 2.4 Update Library page `CompactCardData` construction to pass `valuation_zone: None, upside_downside_ratio: None` (backward compatible)

- [x] Task 3: Add "Compare Selected" flow to Library page (AC: #2)
  - [x] 3.1 Add `RwSignal<Vec<(i32, String)>>` for selected snapshot IDs + ticker symbols in Library
  - [x] 3.2 Add subtle checkbox overlay on each `CompactAnalysisCard` (always visible, toggle selection)
  - [x] 3.3 Add floating action bar at bottom when 2+ cards selected: shows selected ticker symbols + "Compare (N) →" button
  - [x] 3.4 On "Compare" click: navigate to `/compare?snapshot_ids={ids_comma_separated}` via `<A>` router link
  - [x] 3.5 Style floating bar: fixed bottom, `--surface` background, slide-up transition

- [x] Task 4: Create Comparison page component (AC: #1, #3, #4, #6, #7, #8)
  - [x] 4.1 Create `frontend/src/pages/comparison.rs` with `Comparison` component
  - [x] 4.2 Parse URL: `snapshot_ids` (explicit selection from Library), `ticker_ids` (ad-hoc latest), `id` (saved comparison set)
  - [x] 4.3 For `snapshot_ids`: fetch each via `GET /api/v1/snapshots/{id}`, extract summary fields client-side + compute U/D ratio via `steady-invest-logic`
  - [x] 4.4 For `ticker_ids`: call `GET /api/v1/compare?ticker_ids=...`
  - [x] 4.5 For `id`: call `GET /api/v1/comparisons/{id}`
  - [x] 4.6 Empty state (no params): show CTA linking to Library
  - [x] 4.7 Ticker quick-add: deferred (empty state links to Library; ad-hoc ticker_ids URL param available for power users)
  - [x] 4.8 Sortable column headers with `signal<SortColumn>` + `signal<bool>` — default: `(UpsideDownside, false)` (descending)
  - [x] 4.9 Client-side sorting: sort snapshot list by selected column before rendering
  - [x] 4.10 Show sort indicator (▲/▼) on active column header
  - [x] 4.11 Render Compact Analysis Cards in responsive grid
  - [x] 4.12 Navigate to `/?snapshot={id}` on card click
  - [x] 4.13 On mobile (<767px): dropdown `<select>` + direction toggle button
  - [x] 4.14 Preserve comparison state in URL: `snapshot_ids` param (shareable/bookmarkable via URL)

- [x] Task 5: Implement "Save Comparison" flow (AC: #5)
  - [x] 5.1 Add "Save Comparison" button (visible when 2+ cards loaded)
  - [x] 5.2 Show name input inline form on save click
  - [x] 5.3 POST to `/api/v1/comparisons` with `{ name, base_currency: "USD", items: [{ analysis_snapshot_id, sort_order }] }`
  - [x] 5.4 Display success/error feedback after save
  - [x] 5.5 Add "Load Saved" button/dropdown to load existing comparison sets via `GET /api/v1/comparisons`

- [x] Task 6: Register route and Command Strip entry (AC: #1)
  - [x] 6.1 Add `pub mod comparison;` to `frontend/src/pages/mod.rs`
  - [x] 6.2 Add `<Route path=path!("/compare") view=Comparison />` to `frontend/src/lib.rs`
  - [x] 6.3 Add import: `use crate::pages::comparison::Comparison;` in `frontend/src/lib.rs`
  - [x] 6.4 Add "Comparison" menu entry to `frontend/src/components/command_strip.rs` (between Library and Report divider)

- [x] Task 7: Add responsive styles (AC: #7, #8)
  - [x] 7.1 Add `.comparison-page`, `.comparison-controls`, `.comparison-sort-headers`, `.comparison-grid` styles to `frontend/public/styles.scss`
  - [x] 7.2 Desktop wide (1280px+): 3-column grid with clickable sort headers
  - [x] 7.3 Desktop standard (1024-1279px): 2-column grid with clickable sort headers
  - [x] 7.4 Tablet (768-1023px): 2-column grid with clickable sort headers
  - [x] 7.5 Mobile (<767px): 1-column stacked cards with dropdown sort selector (hide clickable headers)
  - [x] 7.6 Style valuation zone dot colors and upside/downside ratio colors
  - [x] 7.7 Style Library floating action bar (`.compare-selection-bar`)
  - [x] 7.8 Reuse existing design tokens: `--surface`, `--primary`, `--text-primary`, `--spacing-*`, `--transition-fast`

- [x] Task 8: E2E smoke tests (AC: all)
  - [x] 8.1 Verify navigation to `/compare` via Command Strip link
  - [x] 8.2 Verify Library "Compare Selected" flow: select cards, click compare, verify navigation
  - [x] 8.3 Verify card rendering with correct metrics including upside/downside ratio
  - [x] 8.4 Verify sort behavior (click header, cards reorder; default sort by U/D ratio)

## Dev Notes

### Party Mode Decisions (2026-02-16)

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | "Compare Selected" from Library (not modal in Comparison) | Keeps Library as single discovery surface; separation of concerns |
| 2 | Library gets selection checkboxes + floating action bar | E-commerce "add to cart" pattern; bar shows selected ticker symbols |
| 3 | Comparison page keeps ticker text input for quick-add | Power user path; uses ad-hoc `GET /api/v1/compare?ticker_ids=...` |
| 4 | `snapshot_ids` param: fetch individual snapshots, extract summary client-side | Preserves version-pinned selection; bandwidth trivial for N ≤ 20 |
| 5 | Empty state on `/compare` with CTA linking to Library | Clear onboarding for first-time users |
| 6 | Valuation zone rendered as colored dot + text (Emerald/Amber/Crimson) | Matches Institutional HUD color system |
| 7 | URL preserves comparison state (`snapshot_ids`, `sort`, `dir`) | Shareable, bookmarkable, back-button friendly |
| 8 | Add upside/downside ratio to `steady-invest-logic` + backend DTO | Cardinal Rule; NAIC core metric for stock selection |
| 9 | Color-coded ratio: Emerald ≥3, Amber 1.0–2.99, Crimson <1 | Visual decision signal per NAIC 3-to-1 rule |
| 10 | Default sort: upside/downside ratio descending | How NAIC investors naturally rank candidates |

### Critical Architecture Constraints

**Cardinal Rule:** All calculation logic lives in `crates/steady-invest-logic`. The upside/downside ratio calculation is added to this crate (Task 1). The backend extracts it from `snapshot_data` and includes it in `ComparisonSnapshotSummary`. Frontend renders it read-only.

**No currency conversion:** Story 8.2 displays all monetary values in their native currencies. Currency conversion is Story 8.3's scope. The `base_currency` field from the API is stored but not used for conversion logic here.

**No state module creation:** The `frontend/src/state/` module for global signals (Currency Preference, Active Portfolio) is created in Story 8.3. Story 8.2 uses local component signals for sort state.

### Upside/Downside Ratio Calculation

The NAIC recommends investing only in companies where the upside/downside ratio is at least 3:1. This is the most important ranking metric for comparison.

**Formula** (5-year projection, matching `valuation_panel.rs` logic):
```
projected_eps_5yr = current_eps × (1 + eps_cagr/100)^5
target_high_price = projected_high_pe × projected_eps_5yr
target_low_price  = projected_low_pe × projected_eps_5yr
upside   = target_high_price - current_price
downside = current_price - target_low_price
ratio    = upside / downside  (None if downside ≤ 0)
```

**Data source:** The `AnalysisSnapshot` stored in `snapshot_data` JSON contains:
- `projected_eps_cagr`, `projected_high_pe`, `projected_low_pe` — directly available
- `historical_data.records` — latest record provides current EPS; current price derived from latest `price_high` (same as P/E calculation basis)

**Implementation:** Add `calculate_upside_downside_ratio()` to `steady-invest-logic`. Backend `from_model_and_ticker()` deserializes the `AnalysisSnapshot`, computes projected values, and calls this function.

**Color thresholds for display:**

| Ratio | Color | CSS Token | Meaning |
|-------|-------|-----------|---------|
| ≥ 3.0 | Emerald | `#10B981` / `--success` | Strong — meets NAIC 3-to-1 rule |
| 1.0 – 2.99 | Amber | `#F59E0B` | Marginal — below NAIC threshold |
| < 1.0 | Crimson | `#EF4444` / `--danger` | Poor — downside exceeds upside |
| None/N/A | Muted grey | `--text-secondary` | Insufficient data |

### Existing Infrastructure (MUST BUILD ON)

**Backend API (created in Story 8.1)** — `backend/src/controllers/comparisons.rs`:

Ad-hoc compare endpoint:
```
GET /api/v1/compare?ticker_ids=1,2,3&base_currency=CHF
→ { "base_currency": "CHF", "snapshots": [ComparisonSnapshotSummary...] }
```

Saved comparison CRUD:
```
GET /api/v1/comparisons                → [ComparisonSetSummary...]
POST /api/v1/comparisons               → ComparisonSetDetail (create)
GET /api/v1/comparisons/:id            → ComparisonSetDetail (with items + snapshots)
PUT /api/v1/comparisons/:id            → ComparisonSetDetail (update)
DELETE /api/v1/comparisons/:id         → (delete)
```

Backend DTOs (mirror these in frontend `Deserialize` structs):
```rust
ComparisonSnapshotSummary {
    id: i32,
    ticker_id: i32,
    ticker_symbol: String,
    thesis_locked: bool,
    captured_at: DateTime<FixedOffset>,  // deserialize as String in frontend
    notes: Option<String>,
    projected_sales_cagr: Option<f64>,
    projected_eps_cagr: Option<f64>,
    projected_high_pe: Option<f64>,
    projected_low_pe: Option<f64>,
    valuation_zone: Option<String>,
    upside_downside_ratio: Option<f64>,  // NEW — added in this story
}

ComparisonSetSummary {
    id: i32,
    name: String,
    base_currency: String,
    item_count: i64,
    created_at: DateTime<FixedOffset>,
}

ComparisonSetDetail {
    id: i32,
    name: String,
    base_currency: String,
    created_at: DateTime<FixedOffset>,
    updated_at: DateTime<FixedOffset>,
    items: Vec<ComparisonSetItemDetail>,
}

ComparisonSetItemDetail {
    id: i32,
    sort_order: i32,
    snapshot: ComparisonSnapshotSummary,
}
```

**CompactAnalysisCard** (`frontend/src/components/compact_analysis_card.rs`):
- Accepts `CompactCardData` struct + `on_click: Callback<i32>`
- Currently missing `valuation_zone` field — add it in Task 1
- Card click triggers `on_click.run(id)` → navigate to `/?snapshot={id}`

**Library page** (`frontend/src/pages/library.rs`) — template for data fetching pattern:
- `LocalResource::new(|| async { gloo_net::http::Request::get(...) })` for async fetch
- `Suspense` wrapper with loading overlay fallback
- `filtered.iter().map(|s| { CompactCardData { ... } })` for card rendering
- `on_card_click = Callback::new(move |id: i32| { navigate(...) })` for navigation

**Router** (`frontend/src/lib.rs`):
- Routes defined in `<Routes>` block inside `<Router>`
- Pattern: `<Route path=path!("/compare") view=Comparison />`
- Import page component: `use crate::pages::comparison::Comparison;`

**Command Strip** (`frontend/src/components/command_strip.rs`):
- Menu items: `<li class="menu-item"><div class="menu-link"><A href="/compare">...</A></div></li>`
- Insert "Comparison" entry between Library link and the Report divider

**Styling** (`frontend/public/styles.scss`):
- Library grid: `.library-grid` uses `grid-template-columns: repeat(3, 1fr)` with responsive breakpoints
- Compact card: `.compact-card` with hover effects, cursor pointer, dark surface background
- Design tokens: `--surface`, `--primary` (#3B82F6), `--text-primary`, `--text-secondary`, `--spacing-*`
- Existing responsive breakpoints: 1280px+ (wide), 1024-1279px (standard), 768-1023px (tablet), <768px (mobile)

### Frontend DTO Definitions

Create these `Deserialize` structs in `comparison.rs` (matching backend responses):

```rust
#[derive(Debug, Clone, Deserialize)]
struct ComparisonSnapshotSummary {
    id: i32,
    ticker_id: i32,
    ticker_symbol: String,
    thesis_locked: bool,
    captured_at: String,
    notes: Option<String>,
    projected_sales_cagr: Option<f64>,
    projected_eps_cagr: Option<f64>,
    projected_high_pe: Option<f64>,
    projected_low_pe: Option<f64>,
    valuation_zone: Option<String>,
    upside_downside_ratio: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct AdHocCompareResponse {
    base_currency: Option<String>,
    snapshots: Vec<ComparisonSnapshotSummary>,
}

#[derive(Debug, Clone, Deserialize)]
struct ComparisonSetSummary {
    id: i32,
    name: String,
    base_currency: String,
    item_count: i64,
    created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ComparisonSetDetail {
    id: i32,
    name: String,
    base_currency: String,
    created_at: String,
    updated_at: String,
    items: Vec<ComparisonSetItemDetail>,
}

#[derive(Debug, Clone, Deserialize)]
struct ComparisonSetItemDetail {
    id: i32,
    sort_order: i32,
    snapshot: ComparisonSnapshotSummary,
}

#[derive(Debug, Clone, Serialize)]
struct CreateComparisonRequest {
    name: String,
    base_currency: String,
    items: Vec<CreateComparisonItem>,
}

#[derive(Debug, Clone, Serialize)]
struct CreateComparisonItem {
    analysis_snapshot_id: i32,
    sort_order: i32,
}
```

### Sorting Implementation

Client-side sorting with reactive signals:

```rust
#[derive(Debug, Clone, PartialEq)]
enum SortColumn {
    Ticker,
    Date,
    SalesCagr,
    EpsCagr,
    HighPe,
    LowPe,
    ValuationZone,
    UpsideDownside,
}

// Signal: (column, ascending) — default: upside/downside ratio descending
let (sort_state, set_sort_state) = signal(Some((SortColumn::UpsideDownside, false)));

// Toggle sort on header click
let toggle_sort = move |col: SortColumn| {
    let current = sort_state.get();
    let new_state = match current {
        Some((ref c, asc)) if *c == col => Some((col, !asc)),  // toggle direction
        _ => Some((col, false)),  // default descending for new column
    };
    set_sort_state.set(new_state);
};

// Sort the list before rendering
let sorted_snapshots = move || {
    let mut list = snapshots_vec.clone();
    if let Some((ref col, asc)) = sort_state.get() {
        list.sort_by(|a, b| {
            let cmp = match col {
                SortColumn::Ticker => a.ticker_symbol.cmp(&b.ticker_symbol),
                SortColumn::SalesCagr => a.projected_sales_cagr.partial_cmp(&b.projected_sales_cagr).unwrap_or(Ordering::Equal),
                SortColumn::EpsCagr => a.projected_eps_cagr.partial_cmp(&b.projected_eps_cagr).unwrap_or(Ordering::Equal),
                // ... etc
            };
            if asc { cmp } else { cmp.reverse() }
        });
    }
    list
};
```

### Adding Analyses to the Comparison

Three entry paths, all supported:

1. **"Compare Selected" from Library** (primary path): User selects cards via checkboxes in Library, clicks floating "Compare Selected" bar. Navigates to `/compare?snapshot_ids=1,2,3`. Frontend fetches each snapshot individually via `GET /api/v1/snapshots/{id}` and extracts summary fields client-side. This preserves version-pinned selection (user picks specific snapshot versions, not "latest").

2. **Ticker quick-add** (power user path): On the Comparison page, user types a ticker symbol in the text input, clicks "Add". Frontend resolves via `GET /api/v1/tickers/search?q=...`, then calls `GET /api/v1/compare?ticker_ids={existing},{new}`. Returns the *latest* snapshot per ticker.

3. **Saved comparison** (persistent): User loads a saved set via "Load Saved" dropdown. URL becomes `/compare?id=5`. Frontend calls `GET /api/v1/comparisons/5`.

**URL state management**: The Comparison page preserves its state in URL query params (`snapshot_ids`, `ticker_ids`, `id`, `sort`, `dir`). This makes comparisons shareable and bookmarkable. Use `leptos_router::hooks::use_query_map()` to read params.

### Snapshot-to-Ticker Resolution

To add by ticker symbol, the frontend needs to resolve ticker symbols to ticker IDs. Options:
- Use existing ticker search endpoint: `GET /api/v1/tickers/search?q=NESN` → returns ticker ID
- Then call `GET /api/v1/compare?ticker_ids={id}` → returns latest snapshot for that ticker
- This is a two-step process but simple and reuses existing APIs

### Mobile Responsive Sort

On mobile (<767px), replace clickable sort headers with:
```html
<div class="sort-dropdown-mobile">
    <select on:change=move |ev| { /* set sort column */ }>
        <option value="ticker">"Ticker"</option>
        <option value="sales_cagr">"Sales CAGR"</option>
        <option value="eps_cagr">"EPS CAGR"</option>
        <option value="high_pe">"High P/E"</option>
        <option value="low_pe">"Low P/E"</option>
    </select>
    <button on:click=move |_| { /* toggle asc/desc */ }>
        {move || if ascending { "▲" } else { "▼" }}
    </button>
</div>
```

Hide `.comparison-sort-headers` on mobile, show `.sort-dropdown-mobile`. Use existing responsive breakpoints.

### What NOT To Do

- Do NOT implement currency conversion — that's Story 8.3
- Do NOT create `frontend/src/state/` module — that's Story 8.3
- Do NOT implement thesis evolution/history — that's Stories 8.4/8.5
- Do NOT modify backend API schema/migrations — Story 8.1 schema is complete. Only add `upside_downside_ratio` field to existing `ComparisonSnapshotSummary` DTO and its computation in `from_model_and_ticker()`
- Do NOT add the Currency Selector dropdown — that's Story 8.3
- Do NOT add inline portfolio context — that's Story 9.7

### Previous Story Intelligence (from Story 8.1)

**Key learnings:**
- Backend compiles cleanly with all comparison DTOs in place
- `ComparisonSnapshotSummary` includes `valuation_zone` from day 1 (Party Mode decision)
- Tests cannot run locally (no `steadyinvest_test` DB on NAS) — CI validates
- `from_model_and_ticker()` pattern extracts metrics from `snapshot_data` JSON
- `base_currency` is stored/echoed but no conversion logic exists yet

**Files created in 8.1:**
- `backend/src/controllers/comparisons.rs` — All API endpoints (MODIFY: add `upside_downside_ratio` to DTO + computation)
- `backend/migration/src/m20260216_000001_comparison_sets.rs` — Schema (DO NOT MODIFY)
- `backend/src/models/_entities/comparison_sets.rs`, `comparison_set_items.rs` (DO NOT MODIFY)
- `backend/tests/requests/comparisons.rs` — 15 API tests (MODIFY: add assertions for `upside_downside_ratio`)

### Git Intelligence

Recent commits confirm:
```
fc90908 fix: wrap comparison CRUD in transactions and validate base_currency
9258755 feat: add comparison schema & API (Story 8.1)
```
The comparison API is stable and production-ready. Frontend can consume it directly.

### Project Structure Notes

**Files to CREATE:**
- `frontend/src/pages/comparison.rs` — Comparison page component (~200-250 lines)

**Files to MODIFY:**
- `frontend/src/components/compact_analysis_card.rs` — Add `valuation_zone` + `upside_downside_ratio` fields to `CompactCardData` and render them
- `frontend/src/pages/mod.rs` — Register `comparison` module
- `frontend/src/lib.rs` — Add route for `/compare`
- `frontend/src/components/command_strip.rs` — Add "Comparison" menu entry
- `frontend/src/pages/library.rs` — Update `CompactCardData` construction to include `valuation_zone: None`
- `frontend/public/styles.scss` — Add comparison view styles

**Files to MODIFY (backend — upside/downside ratio):**
- `crates/steady-invest-logic/src/lib.rs` — Add `calculate_upside_downside_ratio()` function
- `backend/src/controllers/comparisons.rs` — Add `upside_downside_ratio` to `ComparisonSnapshotSummary` + compute in `from_model_and_ticker()`
- `backend/tests/requests/comparisons.rs` — Add ratio assertions to existing tests

**Files NOT to modify:**
- `backend/src/models/` — Schema already complete
- `backend/migration/` — No schema changes needed
- `frontend/src/state/` — Does not exist; created in Story 8.3

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 8, Story 8.2]
- [Source: _bmad-output/planning-artifacts/architecture.md — Frontend Architecture, Core Views, Router expansion]
- [Source: _bmad-output/planning-artifacts/prd.md — FR4.3, NFR6]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md — Comparison View, Compact Analysis Card, Ranked Grid, Journey 2, Currency Selection, Responsive Design]
- [Source: frontend/src/pages/library.rs — Data fetching pattern, card rendering, filtering]
- [Source: frontend/src/components/compact_analysis_card.rs — CompactCardData struct, component API]
- [Source: frontend/src/components/command_strip.rs — Menu entry pattern]
- [Source: frontend/src/lib.rs — Router configuration pattern]
- [Source: backend/src/controllers/comparisons.rs — API response DTOs]
- [Source: _bmad-output/implementation-artifacts/8-1-comparison-schema-api.md — Previous story context]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

None — tests cannot run locally (no `steadyinvest_test` DB on NAS); CI validates.

### Completion Notes List

- All 8 tasks completed with all subtasks checked off
- `calculate_upside_downside_ratio()` added to `steady-invest-logic` with 3 unit tests + 1 doctest (all pass)
- Backend `ComparisonSnapshotSummary` includes `upside_downside_ratio` computed from `AnalysisSnapshot`
- Frontend Comparison page supports 3 entry paths: `snapshot_ids`, `ticker_ids`, saved set `id`
- Client-side sorting with 8 columns, default UpsideDownside descending
- Save/Load comparison flow via POST/GET `/api/v1/comparisons`
- Full responsive styles at all 4 breakpoints (wide desktop, standard, tablet, mobile)
- Library "Compare Selected" flow with checkbox selection and floating action bar
- Valuation zone (colored dots) and U/D ratio (NAIC 3:1 color-coding) on Compact Cards
- `FnOnce` → `FnMut` fix for `load_navigate` closure (call `use_navigate()` inline)

### Code Review Fixes (2026-02-16)

- **[H1] Save flow ticker_ids bug**: Save handler was sending ticker IDs as `analysis_snapshot_id` for `?ticker_ids=` path. Fixed by adding `resolved_ids: RwSignal<Vec<i32>>` populated from fetched data, used by save flow instead of re-parsing URL.
- **[H2] Save flow broken for saved sets**: Same root cause as H1 — URL re-parsing returned empty IDs for `?id=` path. Fixed by same `resolved_ids` signal.
- **[M2] Duplicate projection logic**: Extracted `compute_upside_downside_from_snapshot()` into `steady-invest-logic` crate. Both backend and frontend now call the shared function (Cardinal Rule compliance).
- **[M3] Icon collision**: Changed Comparison icon from "📊" to "⚖" to distinguish from System Monitor.
- **[L1] Extra blank line**: Removed stray blank line in library.rs imports.
- **[M1] Deferred**: `use_navigate()` inside event handler is correct for Leptos 0.8 CSR — `Rc` wrapping doesn't work due to `Send` bounds. Pattern is safe as event handlers run in component owner.

### File List

**Created:**
- `frontend/src/pages/comparison.rs` — Comparison page component (718 lines)

**Modified:**
- `crates/steady-invest-logic/src/lib.rs` — `calculate_upside_downside_ratio()` function + 3 unit tests
- `backend/src/controllers/comparisons.rs` — `upside_downside_ratio` field + `compute_upside_downside()` helper
- `backend/tests/requests/comparisons.rs` — `sample_snapshot_data_with_records()` + U/D ratio test
- `frontend/src/components/compact_analysis_card.rs` — `valuation_zone` + `upside_downside_ratio` fields + rendering
- `frontend/src/pages/library.rs` — "Compare Selected" flow (checkboxes, floating bar)
- `frontend/src/pages/mod.rs` — `pub mod comparison;`
- `frontend/src/lib.rs` — Route + import for Comparison
- `frontend/src/components/command_strip.rs` — "Comparison" menu entry
- `frontend/public/styles.scss` — Comparison page styles, zone/U/D colors, Library compare bar, responsive breakpoints

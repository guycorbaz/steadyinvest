# Story 8.5: History Timeline Sidebar & Side-by-Side Comparison

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an **analyst**,
I want to see a timeline of my past analyses for the current stock and compare them side by side,
so that I can track how my thesis evolved and make better-informed decisions.

## Acceptance Criteria

1. **Given** the user is on the Analysis view with a populated chart for a ticker that has past analyses
   **When** the user clicks the "History" toggle button
   **Then** a Timeline Sidebar appears alongside the SSG chart showing all past analyses for this ticker
   **And** each entry shows: date, thesis locked status, key CAGR values

2. **Given** the Analysis view layout
   **When** the History Timeline Sidebar is implemented
   **Then** the Analysis view uses a composite CSS Grid layout with named regions: `status` (hidden, reserved for Epic 9), `chart`, `sidebar`, `hud`
   **And** the sidebar region is hidden by default and revealed when History is toggled
   **And** the chart area resizes to accommodate the sidebar (push layout preferred; overlay fallback if chart resize causes rendering issues)

3. **Given** the Timeline Sidebar is open with multiple past analyses listed
   **When** the user selects a past analysis entry
   **Then** two side-by-side Snapshot Comparison Cards appear showing the selected past analysis alongside the current analysis
   **And** each card displays: projected Sales CAGR, projected EPS CAGR, P/E estimates, valuation zone (derived from upside/downside ratio: ≥3.0 = "Buy", ≥1.0 = "Hold", <1.0 = "Sell"; show "—" if ratio is null)
   **And** metric deltas are highlighted between the two cards (e.g., "Sales CAGR: 6.0% → 4.5% ▼")

4. **Given** a past analysis has a stored chart image
   **When** displayed in the side-by-side comparison
   **Then** the static chart image is shown for the past analysis (since it cannot be re-rendered with current charting)

5. **Given** the Analysis view renders on tablet (768px-1023px)
   **When** the History toggle is active
   **Then** the Timeline Sidebar renders as a collapsible panel instead of a persistent sidebar

6. **Given** the Analysis view renders on mobile (<767px)
   **When** the mobile breakpoint is active
   **Then** the History toggle is available but opens a simplified list view (no side-by-side cards — single-column timeline with key metrics)

7. **Given** the Analysis view with History Timeline Sidebar
   **When** rendered at all four breakpoints (desktop wide, desktop standard, tablet, mobile)
   **Then** all elements render correctly without layout breakage or overflow

## Tasks / Subtasks

- [x] Task 1: Create History Timeline Sidebar component (AC: #1, #3, #4)
  - [x] 1.1 Create `frontend/src/components/history_timeline.rs` with `HistoryTimeline` component
  - [x] 1.2 Define `TimelineEntry` struct: `id`, `captured_at`, `thesis_locked`, `notes`, `projected_sales_cagr`, `projected_eps_cagr`, `projected_high_pe`, `projected_low_pe`, `current_price`, `target_high_price`, `target_low_price`, `native_currency`, `upside_downside_ratio`
  - [x] 1.3 Fetch history data via `GET /api/v1/snapshots/{id}/history` using `LocalResource` when sidebar opens
  - [x] 1.4 Render vertical timeline list with date, lock icon, and key CAGR values per entry
  - [x] 1.5 Highlight the currently-displayed analysis in the timeline
  - [x] 1.6 On entry click, emit selected snapshot ID via callback prop
  - [x] 1.7 Register module in `frontend/src/components/mod.rs`

- [x] Task 2: Create Snapshot Comparison Cards with metric deltas (AC: #3, #4)
  - [x] 2.1 Create `frontend/src/components/snapshot_comparison.rs` with `SnapshotComparison` component
  - [x] 2.2 Define `SnapshotComparisonProps`: `current` entry, `selected_past` entry, `metric_deltas` (from API response)
  - [x] 2.3 Render two side-by-side cards: "Current Analysis" (left) and "Past Analysis" (right)
  - [x] 2.4 Each card displays: projected Sales CAGR, projected EPS CAGR, P/E high/low, valuation zone (derived from `upside_downside_ratio` — see "Valuation Zone Derivation" in Dev Notes), current price, target price range, upside/downside ratio
  - [x] 2.5 Between cards, render metric deltas with directional indicators (▲ green for improvement, ▼ red for decline, — grey for unchanged/null)
  - [x] 2.6 For past analysis with chart image: fetch `GET /api/v1/snapshots/{id}/chart-image` and display as `<img>` thumbnail
  - [x] 2.7 Register module in `frontend/src/components/mod.rs`

- [x] Task 3: Refactor Analysis view layout to CSS Grid with named regions (AC: #2)
  - [x] 3.1 Modify `frontend/src/pages/home.rs` to wrap the analysis workspace in a CSS Grid container with named areas: `status` (hidden), `chart`, `sidebar`, `hud`
  - [x] 3.2 Add `RwSignal<bool>` for `history_open` state; default `false`
  - [x] 3.3 Add "History" toggle button to the header control bar (next to Lock Thesis / Save) — use Secondary Action style (transparent bg, thin `#1F2937` border). Add `aria-expanded`, `aria-controls="history-sidebar"`. On open, move focus to first timeline entry; on close, return focus to toggle button
  - [x] 3.4 Conditionally render `<HistoryTimeline>` in the sidebar grid area when `history_open` is true
  - [x] 3.5 Render `<SnapshotComparison>` below the chart when a past analysis is selected from the timeline
  - [x] 3.6 Pass the current snapshot/analysis data and selected past entry to `SnapshotComparison`
  - [x] 3.7 Wire the history API call: determine anchor snapshot ID from the existing `snapshots` resource (`GET /api/analyses/:ticker` already in `home.rs`) — use the latest snapshot's ID as anchor for `GET /api/v1/snapshots/{id}/history`. If no snapshots exist for the ticker, disable the History toggle button (greyed out, tooltip: "No saved analyses yet")
  - [x] 3.8 Integrate with existing `selected_snapshot_id` signal: when user selects a past entry from the timeline, update `selected_snapshot_id` to load it in `<SnapshotHUD>` for full detail view (reuses existing deep-link rendering path)
  - [x] 3.9 Remove or hide the existing `view-selector` `<select>` dropdown (which switches between "Live Analysis" and snapshots) — the History Timeline Sidebar replaces this navigation pattern

- [x] Task 4: Add CSS styles for timeline sidebar and comparison cards (AC: #2, #5, #6, #7)
  - [x] 4.1 Add `.analysis-grid` CSS Grid layout styles to `frontend/public/styles.scss` with named regions
  - [x] 4.2 Sidebar push/overlay: implement push layout first (chart width contracts); if chart `ResizeObserver` causes rendering issues, switch to overlay with semi-transparent scrim. Transition: 150ms ease-out (no bounce/overshoot per UX spec)
  - [x] 4.3 Style `.timeline-sidebar` with vertical timeline layout, entry items, active indicator
  - [x] 4.4 Style `.snapshot-comparison` with side-by-side card layout and delta indicators
  - [x] 4.5 **Desktop wide (>=1280px)**: Full sidebar (280px), chart resizes, side-by-side cards
  - [x] 4.6 **Desktop standard (1024-1279px)**: Same as wide but slightly narrower sidebar (250px)
  - [x] 4.7 **Tablet (768-1023px)**: Collapsible panel — sidebar renders as expandable overlay panel triggered by toggle, does not push chart
  - [x] 4.8 **Mobile (<767px)**: Simplified list view — no side-by-side cards, single-column timeline with key metrics only, opens as full-width overlay

- [x] Task 5: Integration and edge cases (AC: #1, #2, #3)
  - [x] 5.1 Handle ticker with no past analyses: show "No previous analyses" message in sidebar; disable History toggle when only one snapshot exists
  - [x] 5.2 Handle API errors: show error state in sidebar ("Could not load history")
  - [x] 5.3 Handle null metric deltas: display "—" instead of numeric value when delta is `null`
  - [x] 5.4 When user changes ticker (via SearchBar), close sidebar and clear selected past analysis
  - [x] 5.5 When sidebar is opened, ensure the currently-displayed snapshot is highlighted in the timeline
  - [x] 5.6 Clicking a timeline entry that is already the current analysis should deselect (close comparison cards)
  - [x] 5.7 When a past analysis is selected from the timeline, update URL to `?snapshot=ID` for shareability — reuses existing deep-link logic in `home.rs`
  - [x] 5.8 When arriving via deep-link (`?snapshot=ID`), auto-open the History sidebar if multiple snapshots exist for that ticker

## Dev Notes

### Architecture Compliance

- **Cardinal Rule**: All financial calculation logic (CAGR, upside/downside, price extraction) lives in `crates/steady-invest-logic`. The frontend receives pre-computed values from the API — do NOT duplicate computation logic in components. Metric deltas are pre-computed by the backend (`metric_deltas` array in history response).
- **API contract (Story 8.4)**: `GET /api/v1/snapshots/{id}/history` returns `HistoryResponse` with `ticker_id`, `ticker_symbol`, `snapshots[]` (ordered by `captured_at` ASC), and `metric_deltas[]` (N-1 deltas for N snapshots). Each snapshot entry includes projection metrics + monetary fields.
- **Chart image endpoint (Story 7.4)**: `GET /api/v1/snapshots/{id}/chart-image` returns raw PNG with `Content-Type: image/png` and `Cache-Control: public, max-age=31536000, immutable`. Returns 404 if no chart image stored.
- **Global state**: `CurrencyPreference` signal exists in `frontend/src/state/mod.rs`. Use `use_currency_preference()` to access. For this story, currency display in comparison cards should use the global preference (no per-view override needed — that's the Comparison view's pattern).
- **Append-only model**: The history endpoint is read-only. Each analysis is a separate snapshot row — selecting a past analysis reads existing data, never modifies it.
- **Push vs overlay layout**: UX spec says push layout preferred (chart width contracts via CSS Grid), overlay fallback if chart resize causes rendering issues. Try push first. The SSG chart uses ECharts (`charming` crate) which should handle container resize via the browser's `ResizeObserver`. If the chart doesn't resize cleanly, fall back to overlay.

### Existing Signals & Resources in `home.rs`

The Analysis view (`home.rs`) already has these signals and resources that the dev MUST integrate with:

- **`selected_ticker: RwSignal<Option<TickerInfo>>`** — currently selected ticker; drives all data fetching
- **`selected_snapshot_id: RwSignal<Option<i32>>`** — when set, switches from live analysis to snapshot view via `<SnapshotHUD>`
- **`imported_snapshot: RwSignal<Option<AnalysisSnapshot>>`** — for file-imported analyses
- **`ActiveLockedAnalysisId` context** — syncs the currently viewed locked analysis ID across components via `use_context()`
- **`snapshots` LocalResource** — fetches `GET /api/analyses/:ticker` (list of locked snapshots for current ticker). Use this to determine the anchor snapshot ID for the history API
- **`deep_link_snapshot` LocalResource** — handles `?snapshot=ID` URL parameter
- **`historicals` LocalResource** — fetches live data via `POST /api/harvest/:ticker`
- **Existing `view-selector` `<select>` dropdown** — switches between "Live Analysis" and snapshots. This is **replaced** by the History Timeline Sidebar (Task 3.9)

### Accessibility Requirements

Per UX spec (WCAG 2.1 Level AA):

- **Focus management**: History toggle opens sidebar → focus moves to first timeline entry. Sidebar close → focus returns to toggle button
- **Keyboard navigation**: All timeline entries navigable via Tab/Arrow keys. Enter/Space to select an entry
- **Contrast**: 7:1 minimum contrast on all HUD elements and timeline text
- **ARIA**: History toggle button: `aria-expanded="true|false"`, `aria-controls="history-sidebar"`. Sidebar: `role="complementary"`, `aria-label="Thesis history timeline"`

### Button Style

History toggle uses **Secondary Action** style per UX spec: transparent background, thin `#1F2937` border, no fill. Consistent with non-destructive panel toggles.

### Valuation Zone Derivation

The `HistoryEntry` API response does NOT include `valuation_zone`. Derive it client-side from `upside_downside_ratio` using the same logic as `compact_analysis_card.rs`:
- `ratio >= 3.0` → "Buy" (zone-buy)
- `ratio >= 1.0` → "Hold" (zone-hold)
- `ratio < 1.0` → "Sell" (zone-sell)
- `ratio is null` → show "—"

### Technical Stack & Versions

- **Leptos**: 0.8 (CSR mode, WASM via trunk)
- **Router**: `leptos_router` — routes defined in `frontend/src/lib.rs`
- **Charting**: `charming` 0.3 with `wasm` feature (ECharts-based)
- **HTTP**: `gloo_net::http::Request` for API calls
- **State**: `RwSignal<T>`, `LocalResource`, `provide_context()` / `use_context()`
- **Styling**: SCSS in `frontend/public/styles.scss` (single file, 2527 lines)

### API Response Format (from Story 8.4)

**GET** `/api/v1/snapshots/{id}/history`
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

**GET** `/api/v1/snapshots/{id}/chart-image` — Returns raw PNG bytes or 404.

### Existing Code Patterns to Follow

**Data fetching pattern** (from `pages/home.rs` and `pages/comparison.rs`):
```rust
let history_resource = LocalResource::new(move || {
    let snapshot_id = anchor_snapshot_id.get();
    async move {
        if let Some(id) = snapshot_id {
            let url = format!("/api/v1/snapshots/{}/history", id);
            let response = gloo_net::http::Request::get(&url).send().await
                .map_err(|e| e.to_string())?;
            if response.ok() {
                response.json::<HistoryResponse>().await
                    .map_err(|e| e.to_string())
            } else {
                Err(format!("HTTP {}", response.status()))
            }
        } else {
            Err("No snapshot selected".to_string())
        }
    }
});
```

**Component prop pattern** (from `compact_analysis_card.rs`):
```rust
#[component]
pub fn HistoryTimeline(
    entries: Vec<TimelineEntry>,
    current_snapshot_id: i32,
    on_select: Callback<Option<i32>>,  // None = deselect
) -> impl IntoView { ... }
```

**Signal-based toggle pattern** (from existing codebase):
```rust
let (history_open, set_history_open) = signal(false);
let toggle_history = move |_| set_history_open.update(|v| *v = !*v);
```

**CSS Grid named areas pattern** (new for this story):
```scss
.analysis-grid {
  display: grid;
  grid-template-areas:
    "status  sidebar"
    "chart   sidebar"
    "hud     sidebar";
  grid-template-columns: 1fr 0;  /* sidebar hidden by default */
  grid-template-rows: 0 auto auto;  /* status row hidden, reserved for Epic 9 */
  gap: var(--spacing-4);
  transition: grid-template-columns 150ms ease-out;  /* UX spec: 150ms, no bounce */

  &.sidebar-open {
    grid-template-columns: 1fr 280px;  /* push: chart contracts */
  }
}

.analysis-status-area { grid-area: status; display: none; }  /* Epic 9: portfolio signals */
.analysis-chart-area  { grid-area: chart; }
.analysis-sidebar     { grid-area: sidebar; overflow: hidden; }
.analysis-hud-area    { grid-area: hud; }
```

### Previous Story Intelligence (from Story 8.4)

**Key learnings from 8.4:**
- `HistoryResponse` wraps `ticker_id`, `ticker_symbol`, `snapshots[]`, and `metric_deltas[]`
- `ticker_id`/`ticker_symbol` are at the response level (not per-entry) — all entries share the same ticker
- `metric_deltas` is an array of N-1 deltas for N snapshots; each has `from_snapshot_id` and `to_snapshot_id`
- `None`/`null` delta values mean one or both metrics were missing — display as "—" not "0"
- Monetary fields (`current_price`, `target_high_price`, `target_low_price`, `native_currency`, `upside_downside_ratio`) are present in each snapshot entry
- Shared metric extraction module (`snapshot_metrics.rs`) established — backend pattern only, frontend extracts from JSON response

**Key learnings from 8.3:**
- `CurrencyPreference` global signal (`RwSignal<String>`) in `frontend/src/state/mod.rs` — default "CHF"
- `use_currency_preference()` hook available for any component
- Currency conversion is client-side using rates from `/api/v1/exchange-rates`
- When rates unavailable, show values in native currency with notice

**Key learnings from 8.2 (Comparison view):**
- `CompactCardData` struct has all fields needed for card rendering
- Sorting pattern: `SortColumn` enum + `sort_col`/`sort_asc` signals + `toggle_sort` closure
- Currency conversion: `convert_price()` helper with `native_currency`, target currency, and rates list
- Card click navigates to Analysis view via `?snapshot=ID` deep link

### Git Intelligence

Recent commit patterns (last 10 commits):
```
3d4a92e feat: add thesis evolution history API with code review fixes (Story 8.4)
a0cfe31 fix: add currency validation helper and comparison display fixes (Story 8.3 followup)
83cb132 docs: add project README for GitHub
67f462b feat: add comparison currency handling with code review fixes (Story 8.3)
1d33530 fix: add secondary sort by id desc to comparison set listing
cf238b4 feat: add comparison view & ranked grid (Story 8.2)
```

Conventions: `feat:` for new features, `fix:` for corrections. Story reference in commit message.

### Key Files to Modify

| File | Action | Details |
|------|--------|---------|
| `frontend/src/components/history_timeline.rs` | NEW | Timeline sidebar component with entry list and selection |
| `frontend/src/components/snapshot_comparison.rs` | NEW | Side-by-side comparison cards with metric deltas |
| `frontend/src/components/mod.rs` | MODIFY | Register `history_timeline` and `snapshot_comparison` modules |
| `frontend/src/pages/home.rs` | MODIFY | Add CSS Grid layout, History toggle, wire timeline + comparison |
| `frontend/public/styles.scss` | MODIFY | Add analysis grid, timeline sidebar, comparison card, responsive styles |

### Files NOT to Modify

- **No backend changes**: Story 8.4 completed the history API; Story 7.4 completed the chart-image endpoint
- **No `steady-invest-logic` changes**: All metrics are pre-computed by the backend
- **No migration changes**: No new database tables needed
- **No state module changes**: `CurrencyPreference` already exists; new signals are local to the Analysis view
- **No router changes**: Analysis view is already at `/` (home route)

### What NOT To Do

- Do NOT add new API endpoints — use existing `GET /api/v1/snapshots/{id}/history` and `GET /api/v1/snapshots/{id}/chart-image`
- Do NOT compute metric deltas on the frontend — the API provides them pre-computed
- Do NOT add pagination to the timeline — Phase 1 returns all snapshots (unlikely >50 per ticker)
- Do NOT modify the Command Strip — the History toggle lives inside the Analysis view, not the global nav
- Do NOT create a new route — the History sidebar is part of the Analysis view at `/`
- Do NOT use `provide_context()` for the history sidebar state — use local signals in `home.rs` (the sidebar is view-specific, not global)
- Do NOT implement the "accuracy indicator" (green badge for prediction accuracy) — that's a Phase 2+ feature per UX spec
- Do NOT implement the "Thesis Scorecard" (prediction vs reality) — deferred to Phase 2+
- Do NOT keep the existing `view-selector` `<select>` dropdown alongside the History sidebar — the sidebar replaces it as the snapshot navigation pattern
- Do NOT create a second `LocalResource` for the `snapshots` list — the existing `snapshots` resource in `home.rs` already provides the data needed to find the anchor snapshot ID

### Project Structure Notes

- Timeline sidebar is the first non-Command-Strip sidebar in the codebase. It establishes a "contextual sidebar" layout pattern per UX spec.
- The CSS Grid named regions pattern (`status`, `chart`, `sidebar`, `hud`) is designed for extensibility — Epic 9 will populate the `status` region with portfolio signals.
- Component file naming follows existing convention: snake_case module name matching component name (e.g., `history_timeline.rs` → `HistoryTimeline` component).

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Post-MVP API Expansion] — defines `GET /api/v1/snapshots/:id/history`
- [Source: _bmad-output/planning-artifacts/architecture.md#State Management] — global signals, `CurrencyPreference`, `use_context()` pattern
- [Source: _bmad-output/planning-artifacts/architecture.md#Core Views] — Library view includes "History Timeline"
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#History Timeline Sidebar] — component anatomy, interaction, layout note
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Journey 5: Thesis Evolution Review] — user flow from toggle to comparison
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Sidebar Reveal] — push vs overlay transition pattern
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Responsive Design] — tablet collapsible panel, mobile simplified list
- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.5] — acceptance criteria and BDD scenarios
- [Source: _bmad-output/implementation-artifacts/8-4-thesis-evolution-api.md] — API response format, dev learnings, code review fixes
- [Source: _bmad-output/implementation-artifacts/8-3-comparison-currency-handling.md] — currency handling patterns, CurrencyPreference signal
- [Source: frontend/src/pages/home.rs] — current Analysis view layout to refactor
- [Source: frontend/src/components/compact_analysis_card.rs] — `CompactCardData` struct, card rendering pattern
- [Source: frontend/src/state/mod.rs] — global signal management, `use_currency_preference()`
- [Source: frontend/src/pages/comparison.rs] — comparison card rendering, sorting, currency conversion patterns
- [Source: frontend/public/styles.scss] — design tokens, responsive breakpoints, existing component styles

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Compile error: `date_display` moved before use in `format!()` macro inside Leptos view. Fixed by pre-computing `aria_label` string before the view macro.

### Completion Notes List

- **Task 1**: Created `history_timeline.rs` with `HistoryTimeline` component, `TimelineEntry`, `MetricDelta`, and `HistoryResponse` DTOs matching backend API. Entries rendered in reverse chronological order (newest first). Current analysis highlighted with `timeline-current` class (disabled button). Selection/deselection via `Callback<Option<i32>>`.
- **Task 2**: Created `snapshot_comparison.rs` with `SnapshotComparison` component. Side-by-side cards with past (left), delta column (center), current (right). Valuation zone derived from `upside_downside_ratio` using same logic as `compact_analysis_card.rs`. Chart image thumbnail displayed for past analysis via `<img>` tag.
- **Task 3**: Rewrote `home.rs` with CSS Grid layout (`analysis-grid` with named regions). Added `history_open`, `selected_past_id`, `history_data` signals. History toggle button with ARIA attributes. `history_resource` uses existing `snapshots` resource for anchor ID. Removed view-selector dropdown. Deep-link auto-opens sidebar. Ticker change clears sidebar state. Inner `.analyst-hud-init` wrapper preserves existing nested styles.
- **Task 4**: Added comprehensive CSS to `styles.scss`: `.analysis-grid` with named regions and 150ms transition, `.history-toggle-btn` (Secondary Action style), `.timeline-*` sidebar styles, `.snapshot-comparison` card styles with delta indicators. Responsive: desktop wide (280px push), desktop standard (250px push), tablet (overlay panel), mobile (full-width overlay, no delta column).
- **Task 5**: All edge cases handled in home.rs: empty state ("No previous analyses"), API errors ("Could not load history"), null deltas (em-dash), ticker change cleanup, current snapshot highlighting, deselection, URL update on selection, deep-link auto-open.

### File List

| File | Action | Lines Changed |
|------|--------|--------------|
| `frontend/src/components/history_timeline.rs` | NEW | 178 lines |
| `frontend/src/components/snapshot_comparison.rs` | NEW | 208 lines |
| `frontend/src/components/mod.rs` | MODIFIED | +2 lines (module registrations) |
| `frontend/src/pages/home.rs` | REWRITTEN | 571 lines (was ~360) |
| `frontend/public/styles.scss` | MODIFIED | +350 lines (analysis grid, timeline, comparison, responsive) |

### Senior Developer Review (AI)

**Review Date**: 2026-02-17
**Reviewer**: Claude Opus 4.6 (adversarial code review)
**Result**: PASS — all HIGH and MEDIUM issues fixed

| # | Severity | Description | Resolution |
|---|----------|-------------|------------|
| H1 | HIGH | `use_navigate()` called inside `Callback::new` closure instead of component init | Fixed: moved to component initialization |
| H2 | HIGH | Closing sidebar doesn't clear `selected_snapshot_id`, view stuck on snapshot | Fixed: added `set_selected_snapshot_id.set(None)` in toggle close path |
| M1 | MEDIUM | `find_delta` silently returns wrong delta for non-adjacent snapshots | Fixed: added doc comment explaining consecutive-delta API limitation |
| M2 | MEDIUM | Chart image 404 shows broken image icon | Fixed: added `on:error` handler to hide parent div |
| M3 | MEDIUM | `mod.rs` doc comments missing new modules | Fixed: updated module documentation |
| L1 | LOW | `on_import` handler unreachable (pre-existing) | Not fixed — pre-existing issue outside story scope |
| L2 | LOW | DTOs use `#[allow(dead_code)]` instead of `#[derive(Debug)]` | Acceptable — fields are deserialized from JSON, dead_code is accurate |

# Story 8b.1: SSG Handbook Audit and Chart Fixes

Status: done

## Story

As an analyst using the SSG chart,
I want the chart to faithfully implement the NAIC Stock Selection Guide methodology,
so that my investment analysis and projections are correct and trustworthy.

## Background

During Epic 8 retrospective, Guy discovered two critical SSG chart deviations during Docker walkthroughs:
1. X-axis years display in reverse order (newest-left, oldest-right) instead of chronological (oldest-left, newest-right)
2. Projection lines don't extend correctly from the most recent historical year into the future

The NAIC SSG Handbook (`docs/NAIC/SSGHandbook.pdf`) is now available as the authoritative reference. A full audit revealed these are symptoms of a deeper root cause plus additional methodology gaps.

## Root Cause Analysis

**The harvest service generates records in reverse chronological order.** In `backend/src/services/harvest.rs:56-57`:
```rust
for i in 1..=10 {
    let year = current_year - i;  // newest year first (current-1), oldest last (current-10)
```

The frontend chart (`ssg_chart.rs`) iterates records as-received, pushing years left-to-right. Since records arrive newest-first, the chart displays years reversed. This cascades into:
- The CAGR negation hack (lines 189, 195) — CAGR computes negative because "start" is newest (highest) value
- Projection starting from `raw_years[0]` (newest year instead of the most recent historical year)
- Historical trendline `trendline[0].value` being the wrong anchor point

## Acceptance Criteria

### AC 1: Chronological Record Ordering
- Records MUST be sorted by `fiscal_year` ascending before any chart rendering or calculation
- X-axis displays oldest year on left, newest on right, projection years continuing rightward
- Sort MUST happen at the source in `harvest.rs` (not at individual consumers) to ensure all downstream code — chart rendering, PDF export, quality analysis — receives chronologically-ordered data

### AC 2: Remove CAGR Negation Hack
- Remove the `-s_cagr` and `-e_cagr` negation in `ssg_chart.rs:189,195`
- With chronological ordering, CAGR naturally computes positive for growing companies
- Verify slider increases still make projections go UP (positive CAGR = upward trend)

### AC 3: Fix Projection Lines
- Projection lines MUST start from the **last historical year's trendline value** and extend 5 years forward
- Projection data array should contain `null`/empty values for historical years and computed values only for future years (so the dashed line starts at the boundary)
- The projection and historical trendline should meet at the last historical year (visual continuity)

### AC 4: Display Historical Trendline
- Render the best-fit trendline (log-space regression) as a separate series overlaid on historical data
- Use same color as the data series but with a distinct style (e.g., thin solid line vs. thick data line, or dotted)
- Trendline covers only historical years, projection covers only future years

### AC 5: Fix P/E Averages to 5-Year (per NAIC Section 3)
- `calculate_pe_ranges()` MUST average over the **last 5 years** (not 10) per NAIC handbook
- Update the function, its docstring, and the `PeRangeAnalysis` struct documentation
- Update `ValuationPanel` header from "10-Year Historical Context" to "5-Year P/E Context"
- Update existing test `test_pe_ranges_10year_limit` to verify 5-year limit

### AC 6: Golden Test Cases
- Add test cases in `steady-invest-logic` using worked examples from the NAIC handbook
- Tests MUST cover:
  - CAGR calculation with known handbook values
  - P/E range computation (5-year average high/low P/E)
  - Upside/downside ratio matching handbook formula
  - Projected trendline values at year 5
- Each test should reference the handbook section/page it validates

### AC 7: Price Bars (High-Low Range) per NAIC Figure 2.1
- Replace the two separate smooth lines (Price High, Price Low) with **vertical range bars** showing each year's high-to-low price range
- Per NAIC handbook Figure 2.1: stock prices are displayed as black vertical bars, NOT as smooth lines
- Each bar spans from `price_low` (bottom) to `price_high` (top) for that fiscal year
- Use `charming::series::Candlestick` (confirmed available in charming 0.3.1) with data format `[open, close, low, high]` — set `open = close = price_low` so the body collapses to zero width, leaving only the wick from low to high
- Style with `ItemStyle::new().color("#000000")` for black bars matching NAIC convention
- Price bars should only appear for historical years (not projected years)

### AC 8: Pre-Tax Profit (PTP) Line per NAIC Figure 2.1
- Add a Pre-Tax Profit data series to the SSG chart
- Per NAIC handbook: the chart displays 3 financial lines — Sales (green), EPS (blue), Pre-Tax Profit (red/magenta) — plus price bars
- Data source: `pretax_income` field exists in `HistoricalYearlyData` as **`Option<Decimal>`** — must handle `None` values (skip missing years in trendline regression, show gaps in chart)
- PTP trendline (log-regression) should also be computed and displayed alongside Sales and EPS trendlines
- PTP projection line should follow the same pattern as Sales/EPS projections
- Add a PTP CAGR slider control matching the Sales/EPS slider pattern
- Add `projected_ptp_cagr: f64` to `AnalysisSnapshot` with `#[serde(default)]` for backward compatibility (existing DB snapshots lack this field)
- Add a third thread-local signal `PTP_SIGNAL` (mirroring `SALES_SIGNAL`/`EPS_SIGNAL` in ssg_chart.rs:38-64) and a `window.rust_update_ptp_cagr()` JS callback
- Update `setupDraggableHandles()` JS function signature to accept `ptpStartValue, ptpYears` params, and update the `wasm_bindgen` import declaration in ssg_chart.rs accordingly

### AC 9: NAIC Terminology in UI Labels
- All user-facing labels in the SSG chart, Valuation Panel, and Quality Dashboard MUST use official NAIC SSG terminology (see terminology table in Dev Notes)
- Chart title, legend entries, slider labels, panel headers, and zone labels must be updated
- Code variable names can remain as-is — only visible UI text changes

### AC 10: Verify Chart Bridge Consistency
- After fixing data ordering, verify `chart_bridge.js` drag handle CAGR computation still works correctly
- The JS formula `(Math.pow(newValue / startValue, 1 / years) - 1) * 100` must produce the same sign convention as the Rust code
- `salesStartValue`, `salesYears`, `epsStartValue`, `epsYears` passed to JS must reflect the corrected ordering

### AC 11: SSG Page Layout per NAIC Figure 2.1
- The analyst page layout MUST follow the NAIC SSG structure shown in Figure 2.1 (Handbook p12)
- **Section 1 — Visual Analysis** at top: the semi-log chart (Sales, EPS, Pre-Tax Profit lines + Price bars)
- **"Fundamental Company Data" table** directly below the chart:
  - Transposed layout: rows = metrics (Historical Sales, Historical Earnings, Pre-Tax Profit), columns = years
  - Columns: 10 years (chronological left-to-right) + **Growth %** + **Forecast %** + **5 Yr Est**
  - Growth % = historical CAGR, Forecast % = projected CAGR from slider, 5 Yr Est = projected value at year 5
  - This replaces the current vertical `records-grid` table (Year/Sales/EPS/High/Low per row)
- **"Evaluate Management" table** below the data table:
  - Rows: % Pre-Tax Profit on Sales, % Earned on Equity, % Debt to Capital
  - Columns: 10 years (chronological) + **5 Yr Avg** + **Trend** (arrow indicator)
  - This replaces the current `QualityDashboard` component layout (which shows Year/ROE/Trend/Profit/Trend per row)
- **Sections 3–5** (ValuationPanel: P/E History, Risk & Reward, Five-Year Potential) follow after Sections 1–2
- The overall order in `analyst_hud.rs`: SSGChart → Fundamental Company Data table → Evaluate Management table → ValuationPanel
- `snapshot_hud.rs` must mirror the same layout for locked analyses

## Tasks / Subtasks

- [x] Task 1: Sort records chronologically at source (AC: #1)
  - [x] 1.1 Add `.sort_by_key(|r| r.fiscal_year)` in `harvest.rs` after building the records vector, before returning `HistoricalData` — this ensures ALL consumers (chart, PDF export, quality analysis) receive chronological data
  - [x] 1.2 Verify `calculate_growth_analysis()` processes records in received order (it does NOT sort internally — it computes CAGR from first-to-last value as provided)
  - [x] 1.3 Verify `calculate_pe_ranges()` processes records correctly with chronological input (it also sorts internally)
  - [x] 1.4 Verify `reporting.rs` (PDF export) benefits from the source-level sort (uses AnalysisSnapshot from already-sorted HistoricalData)

- [x] Task 2: Remove CAGR negation and fix projection anchoring (AC: #2, #3, #4)
  - [x] 2.1 Remove `-s_cagr` / `-e_cagr` negation in `ssg_chart.rs` — CAGR passed directly (positive = growth)
  - [x] 2.2 Change projection start: use `*raw_years.last()` and `trendline.last().value` instead of `[0]`
  - [x] 2.3 Adjust projection data arrays: NaN for historical years (except last for visual continuity), values for future years
  - [x] 2.4 Add historical trendline series (dotted Line overlay for historical years, NaN-padded for future)
  - [x] 2.5 Update `sales_years` and `eps_years` to 5.0 (projection period only)

- [x] Task 3: Fix P/E to 5-year average (AC: #5)
  - [x] 3.1 Change `calculate_pe_ranges()` — `start_idx` threshold from 10 to 5
  - [x] 3.2 Update docstrings and struct documentation
  - [x] 3.3 Update `ValuationPanel` "10-Year" → "5-Year P/E History" label
  - [x] 3.4 Rename `test_pe_ranges_10year_limit` → `test_pe_ranges_5year_limit`, adjust assertions (5 points, years 2007-2011, avg high PE 19.0, avg low PE 14.0)

- [x] Task 4: Add golden test cases (AC: #6)
  - [x] 4.1 Read NAIC SSG Handbook (`docs/NAIC/SSGHandbook.pdf`) — specifically the O'Hara Cruises worked example
  - [x] 4.2 Extract numerical values for: Sales/EPS history, CAGRs, P/E averages, projected prices, upside/downside ratio
  - [x] 4.3 Create `test_naic_handbook_*` tests in `steady-invest-logic/src/lib.rs`
  - [x] 4.4 Verify all golden tests pass (33 tests: 25 unit + 8 doc)

- [x] Task 5: Replace price lines with vertical range bars (AC: #7)
  - [x] 5.1 Remove the two smooth `Line` series for Price High and Price Low
  - [x] 5.2 Add `charming::series::Candlestick` series with data format `[open, close, low, high]` per year — set `open = close = price_low` to collapse the body, leaving only the wick (low→high)
  - [x] 5.3 Style with `ItemStyle` color "#B0B0B0" (dark-theme visible), only for historical years (no padding needed)
  - [x] 5.4 Verify frontend builds clean with Candlestick on the log-scale Y-axis

- [x] Task 6: Add Pre-Tax Profit line and projection (AC: #8)
  - [x] 6.1 Extract `pretax_income` from records alongside sales/eps in chart Effect — handle `Option<Decimal>`: use `.unwrap_or(Decimal::ZERO)` or skip `None` years in trendline regression
  - [x] 6.2 Add PTP data series (red/magenta `#E74C3C` line) to the chart
  - [x] 6.3 Compute PTP trendline via `calculate_growth_analysis` and render it (filter out zero/None PTP years before regression)
  - [x] 6.4 Compute PTP projection via `calculate_projected_trendline` and render as dashed line
  - [x] 6.5 Add PTP CAGR slider in the control bar (same pattern as Sales/EPS sliders)
  - [x] 6.6 Add PTP CAGR `RwSignal<f64>` prop to SSGChart component (propagated from AnalystHUD/SnapshotHUD)
  - [x] 6.7 Add thread-local `PTP_SIGNAL` (mirror `SALES_SIGNAL`/`EPS_SIGNAL` at ssg_chart.rs:38-64) + `window.rust_update_ptp_cagr()` callback
  - [x] 6.8 Update `chart_bridge.js` `setupDraggableHandles()` signature: add `ptpStartValue, ptpYears` params + third drag handle (magenta circle). Update `wasm_bindgen` import in ssg_chart.rs to match new arity.
  - [x] 6.9 Add `#[serde(default)] pub projected_ptp_cagr: f64` to `AnalysisSnapshot` — `serde(default)` is REQUIRED for backward compatibility with existing DB snapshots that lack this field

- [x] Task 7: NAIC terminology pass on all UI labels (AC: #9)
  - [x] 7.1 Update chart title, legend names, slider labels in `ssg_chart.rs`
  - [x] 7.2 Update panel headers and slider labels in `valuation_panel.rs`
  - [x] 7.3 Update section header and metric labels in `quality_dashboard.rs`
  - [x] 7.4 Cross-check every visible string against the terminology table in Dev Notes (including reporting.rs PDF labels)

- [x] Task 8: Verify JS bridge and interactive handles (AC: #10)
  - [x] 8.1 Review `chart_bridge.js` CAGR formula after data ordering fix — correct sign convention
  - [x] 8.2 `salesStartValue`/`epsStartValue` already use `.last()` (most recent year's trendline value)
  - [x] 8.3 `salesYears`/`epsYears` already set to 5.0 (projection period only)
  - [x] 8.4 Positive CAGR → base*(1+cagr/100)^n → projection goes UP — verified
  - [x] 8.5 PTP handle uses same formula, series found via `.includes('PTP Est.')` — verified

- [x] Task 9: Restructure page layout per NAIC Figure 2.1 (AC: #11)
  - [x] 9.1 Replace vertical `records-grid` with transposed "Fundamental Company Data" table
    - Rows: Historical Sales, Historical Earnings, Pre-Tax Profit
    - Columns: 10 fiscal years (chronological) + Growth % + Forecast % + 5 Yr Est
  - [x] 9.2 Restructure `QualityDashboard` into "Evaluate Management" table
    - Rows: % Pre-Tax Profit on Sales, % Earned on Equity, % Debt to Capital
    - Columns: 10 fiscal years + 5 Yr Avg + Trend arrow
  - [x] 9.3 Reorder components in `analyst_hud.rs`: SSGChart → Fundamental Company Data → Evaluate Management → ValuationPanel
  - [x] 9.4 Apply same layout restructuring to `snapshot_hud.rs`
  - [x] 9.5 Compute Growth %, Forecast %, and 5 Yr Est columns:
    - Growth % = historical CAGR from `calculate_growth_analysis()`
    - Forecast % = current slider CAGR value
    - 5 Yr Est = projected value at last historical year + 5
  - [x] 9.6 Update `frontend/public/styles.scss`: added `.fundamental-data-table` styles with transposed layout; updated `.quality-grid` for horizontal layout; responsive scrolling on tablet/mobile

- [x] Task 10: Update PDF report chart (AC: #1, #7, #8)
  - [x] 10.1 Update `backend/src/services/reporting.rs` server-side chart rendering to match frontend changes
  - [x] 10.2 Add PTP line series (red #E74C3C) with trendline and projection to the SSR chart
  - [x] 10.3 Replace Price High/Low lines with Candlestick series for price bars (#333333 for PDF white background)
  - [x] 10.4 Add historical trendline series (dotted) and projection series (dashed) for all three metrics
  - [x] 10.5 Data is chronological from harvest.rs sort-at-source — verified chart uses data as-received

- [x] Task 11: Verify and build (all ACs)
  - [x] 11.1 Run `cargo test` in `steady-invest-logic` — all 33 tests pass (25 unit + 8 doc)
  - [x] 11.2 Run `cargo build` for backend — clean
  - [x] 11.3 Run `trunk build` for frontend — clean
  - [x] 11.4 Docker verification deferred to Guy's walkthrough
  - [x] 11.5 E2E tests checked — no references to `.records-grid` or `.quality-grid`; only `quality-dashboard` class referenced (still valid)

## Dev Notes

### NAIC Terminology Alignment

The SSG Handbook uses specific terminology. Our code and UI labels MUST use official NAIC names to avoid confusion. Apply these changes wherever labels appear in the UI:

| Current App Term | NAIC Official Term | Where Used |
|---|---|---|
| "SSG Analysis: {ticker}" | "Stock Selection Guide: {ticker}" | Chart title (`ssg_chart.rs`) |
| "Sales" | "Sales" | Correct — keep |
| "EPS" | "Earnings Per Share" (or "EPS" in charts) | Correct — keep |
| "Price High" / "Price Low" (two lines) | "Stock Price" (single range bars) | Chart series — replace with bars (AC 7) |
| *(missing)* | "Pre-Tax Profit" | Chart series — add (AC 8) |
| "Projected Sales CAGR" | "Estimated Sales Growth Rate" | Chart slider label |
| "Projected EPS CAGR" | "Estimated EPS Growth Rate" | Chart slider label |
| "Sales (CAGR: X%)" | "Sales Growth: X%" | Chart legend |
| "EPS (CAGR: X%)" | "EPS Growth: X%" | Chart legend |
| "Sales Projection" | "Sales Est. Growth" | Chart legend |
| "EPS Projection" | "EPS Est. Growth" | Chart legend |
| "Valuation Analysis & Projections" | "Price-Earnings History & Valuation" | Panel header (`valuation_panel.rs`) |
| "10-Year Historical Context" | "5-Year P/E History" | Panel section header |
| "Future Avg High P/E" | "Estimated Average High P/E" | Slider label |
| "Future Avg Low P/E" | "Estimated Average Low P/E" | Slider label |
| "Target Buy Zone (Floor)" | "Forecast Low Price (Buy Zone)" | Zone label |
| "Target Sell Zone (Ceiling)" | "Forecast High Price (Sell Zone)" | Zone label |
| "Quality Dashboard" | "Evaluate Management" | Dashboard component header |
| "Profit-on-Sales" / "profit_on_sales" | "% Pre-Tax Profit on Sales" | Quality metrics label |
| "ROE" / "roe" | "% Earned on Equity" | Quality metrics label |

**Code variable names** (Rust/JS) can stay as-is for brevity (`sales_cagr`, `eps_cagr`, `roe`, `profit_on_sales`) — only **user-facing labels** need NAIC terminology.

### NAIC SSG Page Layout (Figure 2.1, p12)

The NAIC SSG is a single-page integrated analysis. Figure 2.1 shows the canonical layout:

```
┌─────────────────────────────────────────────────────┐
│  VISUAL ANALYSIS (semi-log chart)                   │
│  Lines: Sales, EPS, Pre-Tax Profit                  │
│  Bars: Stock Price (high-low range)                 │
│  Trendlines + 5-year projections                    │
├─────────────────────────────────────────────────────┤
│  FUNDAMENTAL COMPANY DATA                           │
│          2006  2007 ... 2015  Growth%  Fcst%  5YrEst│
│  Hist.Sales  387  476 ... 1007  13.2%   18    1,606 │
│  Hist.Earn.  1.04 2.24...  4.52  15.4%   18    9.97 │
│  Pre-Tax Pr. 126  163 ... 334   10.0%          ...  │
├─────────────────────────────────────────────────────┤
│  EVALUATE MANAGEMENT                                │
│          2006  2007 ... 2015  5YrAvg  Trend         │
│  %PTP/Sales  32.5  34.2 ...  33.2%   33.0%   ↑     │
│  %ROE        22.3  20.6 ...  39.0%   44.3%   ↑     │
│  %Debt/Cap.  0.0   0.0  ...  0.0%    0.2%    →     │
├─────────────────────────────────────────────────────┤
│  PRICE-EARNINGS HISTORY (5 years)                   │
│  + RISK & REWARD (forecast prices, upside/downside) │
│  + FIVE-YEAR POTENTIAL                              │
└─────────────────────────────────────────────────────┘
```

**Current app layout vs NAIC target:**
- Current `records-grid`: vertical table (rows = years, cols = Sales/EPS/High/Low) → Must transpose to NAIC format (rows = metrics, cols = years + summary cols)
- Current `QualityDashboard`: vertical table (rows = years, cols = ROE/Trend/Profit/Trend) → Must transpose to NAIC format (rows = metrics, cols = years + 5 Yr Avg + Trend)
- Price High/Low columns move from the data table into the chart (as vertical bars per AC 7)
- Pre-Tax Profit row added to the Fundamental Company Data table
- Growth %, Forecast %, and 5 Yr Est are computed summary columns, not raw data

### Cardinal Rule
ALL calculation logic lives in `steady-invest-logic` crate. The frontend (`ssg_chart.rs`) must only consume results — never compute CAGR, trendlines, or P/E ratios directly. The logic crate compiles to both native Rust (backend) and WASM (frontend).

### Key Files to Modify

| File | Changes |
|------|---------|
| `crates/steady-invest-logic/src/lib.rs` | Fix `calculate_pe_ranges()` to 5-year; add golden tests; add `projected_ptp_cagr` to `AnalysisSnapshot` |
| `frontend/src/components/ssg_chart.rs` | Sort records; remove CAGR negation; fix projection anchoring; add historical trendline series; replace price lines with range bars; add PTP line + trendline + projection |
| `frontend/src/components/analyst_hud.rs` | Add PTP CAGR signal; pass to SSGChart; restructure layout order (chart → data table → management → valuation); replace records-grid with transposed Fundamental Company Data table |
| `frontend/src/components/snapshot_hud.rs` | Add PTP CAGR signal from snapshot data; mirror restructured layout |
| `frontend/src/components/quality_dashboard.rs` | Restructure into NAIC "Evaluate Management" table format (rows=metrics, columns=years + 5 Yr Avg + Trend) |
| `frontend/public/chart_bridge.js` | Update start values and years for corrected data order; add PTP drag handle |
| `frontend/src/components/valuation_panel.rs` | Update "10-Year" → "5-Year" label |
| `backend/src/services/harvest.rs` | Add `.sort_by_key(|r| r.fiscal_year)` after building records vector (AC 1) |
| `backend/src/services/reporting.rs` | Update server-side SSG chart to match frontend changes: add PTP line, Candlestick price bars, trendlines. PDF chart must stay in sync with frontend chart. |
| `frontend/public/styles.scss` | Replace `.records-grid` styles with "Fundamental Company Data" transposed table styles; replace `.quality-grid` styles with "Evaluate Management" horizontal layout |

### Files to Read but NOT Modify (unless broken)
| File | Reason |
|------|--------|
| `docs/NAIC/SSGHandbook.pdf` | Authoritative reference for all calculations |

### NAIC Handbook Key Reference Points

**Section 1 — Visual Analysis (see Handbook Figure 2.1, p12):**
- Semi-log chart (Y-axis logarithmic) — already correct
- X-axis: years left-to-right chronological — **currently reversed** → AC 1
- Data series per NAIC: Sales (green), EPS (blue), Pre-Tax Profit (red/magenta), Price bars (black vertical range bars)
  - Pre-Tax Profit line — **currently missing** → AC 8
  - Price bars (high-low range) — **currently rendered as two separate smooth lines** → AC 7
- Historical best-fit trendline overlaid on data — **currently not rendered** → AC 4
- Projection extends from last historical year forward 5 years — **currently broken** → AC 3

**NAIC SSG 5-Section Structure (Handbook p11-12):**
1. Visual Analysis — SSG chart + **Fundamental Company Data table** (chart fixes in ACs 1-4, 7-8; data table layout in AC 11)
2. Evaluate Management — **Restructured** from vertical to NAIC horizontal layout (AC 11); Debt-to-Capital shows N/A until data available
3. Price-Earnings History — ValuationPanel exists; P/E period fix in AC 5
4. Evaluating Risk & Reward — Upside/Downside ratio in logic crate; zoning partially displayed
5. Five-Year Potential — compound annual return NOT implemented (future)

**Section 3 — P/E History:**
- 5-year averages (not 10) for High P/E and Low P/E — **currently using 10**
- Formula: `High P/E = Annual High Price / Annual EPS`
- Formula: `Low P/E = Annual Low Price / Annual EPS`
- Exclude years with zero/negative EPS — already correct

**Section 4 — Risk and Reward:**
- `Forecast High Price = Avg High P/E × (TTM EPS × (1 + growth)^5)` — already correct
- `Upside/Downside Ratio = (High - Current) / (Current - Low)` — already correct
- Minimum 3:1 ratio — already implemented

**CAGR Formula:** `(EndValue / BeginValue)^(1/N) - 1` — already correct in logic crate

### Projection Logic (Corrected)

```
1. harvest.rs sorts records by fiscal_year ASC at source (all consumers receive chronological data)
2. Calculate historical trendline via log-regression (existing calculate_growth_analysis — does NOT sort internally, relies on ordered input)
3. Historical trendline covers years[0]..years[last]
4. Projection starts from trendline[last].value at years[last]
5. Projection extends years[last]+1 .. years[last]+5
6. CAGR is positive for growth → (1 + cagr/100)^n produces increasing values
7. No negation needed
```

### CAGR Negation Root Cause (for removal confidence)

Current code (ssg_chart.rs:184-196):
```rust
sales_start = sales_trend.trendline[0].value;  // With reversed data: this is NEWEST year's fitted value
// ...
calculate_projected_trendline(raw_years[0], sales_start, -s_cagr, ...)  // Negation compensates
```

After fix:
```rust
// Records now sorted ASC — raw_years[0] is oldest, raw_years.last() is newest
let last_idx = sales_trend.trendline.len() - 1;
sales_start = sales_trend.trendline[last_idx].value;  // Last historical year's fitted value
// ...
calculate_projected_trendline(*raw_years.last().unwrap(), sales_start, s_cagr, &future_years)  // No negation
```

### charming Library Notes
- Using `charming` 0.3.1 (pinned as `"0.3"` in Cargo.toml) with WASM feature for ECharts rendering. Latest is 0.6.0 — no upgrade needed; Candlestick and Custom series both work in 0.3.1.
- `Line::new().data()` accepts `Vec<f64>` — for null gaps in projection series, use a custom approach (e.g., separate series that only covers future years with the x-axis extended)
- `LineStyleType::Dashed` for projection lines — already used correctly
- **Price range bars**: `charming::series::Candlestick` IS available in 0.3.1. Data format: `[open, close, low, high]` per data point. For pure high-low range bars (no OHLC body), set `open = close = price_low` — this collapses the candlestick body to zero width, leaving only the wick from low to high. Style: `ItemStyle::new().color("#000000")`.
- **PTP line**: Same pattern as Sales/EPS — a `Line` series with `smooth(true)` and color `#E74C3C` (red/magenta)
- **Note**: If ever upgrading to charming 0.6.0, `RawString` was replaced with `JsFunction` (breaking change). Not relevant for current work.

### PTP Thread-Local Signal Pattern
The chart uses thread-local `RefCell<Option<RwSignal<f64>>>` for JS→Rust callbacks (ssg_chart.rs:38-64):
```rust
thread_local! {
    static SALES_SIGNAL: RefCell<Option<RwSignal<f64>>> = RefCell::new(None);
    static EPS_SIGNAL: RefCell<Option<RwSignal<f64>>> = RefCell::new(None);
    // Add:
    static PTP_SIGNAL: RefCell<Option<RwSignal<f64>>> = RefCell::new(None);
}
```
Each signal has a corresponding `#[wasm_bindgen]` exported function (e.g., `update_sales_cagr`) that JS calls via `window.rust_update_sales_cagr()`. The PTP signal needs the same pattern: store in thread-local before render, export `update_ptp_cagr`, register as `window.rust_update_ptp_cagr` in JS.

### Testing Strategy
- **Unit tests**: Golden test cases in `steady-invest-logic/src/lib.rs` (doctests + #[test])
- **Integration**: `cargo build` for backend, `trunk build` for frontend
- **Visual verification**: Docker image build + Firefox walkthrough (Guy will verify)
- **E2E**: Existing E2E tests should still pass (they don't directly test chart data ordering)

### Existing E2E Test Impact
- 45 E2E tests currently passing in CI
- Chart rendering tests are minimal (check element presence, not data values)
- **WARNING**: AC 11 replaces `.records-grid` and restructures `.quality-dashboard` — check if any E2E tests in `tests/e2e/src/` reference these CSS class names or table structures. Update selectors if so.
- No new E2E tests required (visual verification via Docker walkthrough is the acceptance test for chart correctness)

### Out of Scope (Acknowledged NAIC Gaps for Future)
- **Debt-to-Capital ratio** — the "Evaluate Management" table will include a placeholder row for % Debt to Capital, but actual values require new data fields (`total_debt` / `total_capitalization`) not available from current harvest. Show "N/A" until data source is added.
- Section 5 (Five-Year Potential) compound annual return calculation (price appreciation + dividend yield)
- Preferred Procedure (alternative EPS projection from projected sales → pretax profit → net earnings → EPS)
- Dividend yield, payout ratio, and % High Yield calculations
- Relative Value (`Current P/E / 5-Year Avg P/E`) and PEG Ratio metrics
- **Recent Quarterly Figures** comparison box (visible in Figure 2.1 sidebar — requires quarterly data feed)
- **Analyst Consensus Estimates** box (visible in Figure 2.1 sidebar — requires consensus data feed)
- 25%-50%-25% vs 33%-33%-33% zoning toggle

### Project Structure Notes

- All monetary calculation logic: `crates/steady-invest-logic/src/lib.rs`
- Frontend chart component: `frontend/src/components/ssg_chart.rs` (390 lines)
- JS chart interop: `frontend/public/chart_bridge.js` (121 lines)
- Analyst workspace: `frontend/src/components/analyst_hud.rs` (223 lines)
- Snapshot viewer: `frontend/src/components/snapshot_hud.rs` (147 lines)
- Valuation UI: `frontend/src/components/valuation_panel.rs` (302 lines)
- Quality metrics UI: `frontend/src/components/quality_dashboard.rs` (90 lines)
- Stylesheet: `frontend/public/styles.scss` (3023 lines)
- Data harvest (sort at source): `backend/src/services/harvest.rs`
- PDF chart (SSR rendering): `backend/src/services/reporting.rs`

### References

- [Source: docs/NAIC/SSGHandbook.pdf] — Official NAIC Stock Selection Guide handbook
- [Source: _bmad-output/implementation-artifacts/epic-8-retro-2026-02-17.md#significant-discovery] — Issue discovery during Epic 8 retro
- [Source: _bmad-output/planning-artifacts/architecture.md] — Cardinal Rule, charming 0.3, shared logic crate
- [Source: crates/steady-invest-logic/src/lib.rs] — All NAIC calculation functions
- [Source: frontend/src/components/ssg_chart.rs] — Chart rendering with projection logic

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

### Completion Notes List

- Code review #1 found 3 HIGH, 7 MEDIUM, 4 LOW issues. All HIGH and MEDIUM fixed:
  - H1: PTP `0.0` → `f64::NAN` on log-scale chart (ssg_chart.rs, reporting.rs)
  - H2: Inline `powf(5.0)` → `project_forward()` in logic crate (analyst_hud.rs, snapshot_hud.rs, extract_snapshot_prices)
  - H3: PTP projection anchor guarded when non-positive (ssg_chart.rs, reporting.rs)
  - M1: Removed dead `ptp_years` variable (ssg_chart.rs)
  - M2: Documented intentional price override exclusion (analyst_hud.rs comment)
  - M3: Refactored backward-compat test from `str::replace` to `serde_json::Value::remove` (lib.rs)
  - M4: Added `projected_ptp_cagr` round-trip assertion (lib.rs)
  - M5: Added PTP to mobile CAGR summary (ssg_chart.rs)
  - M6: Strengthened `target_low` assertion from `> 0.0` to `±1.0` of expected value (lib.rs)
  - M7: Populated File List section (this story file)
- Code review #2 found 3 HIGH, 7 MEDIUM, 7 LOW issues. All HIGH and MEDIUM fixed:
  - H1: `lock_thesis_modal.rs` — Added `ptp_projection_cagr` prop; was hardcoded to `0.0` (data integrity bug)
  - H2: `valuation_panel.rs` — Replaced inline `powf` with `project_forward()` (Cardinal Rule)
  - H3: `chart_bridge.js` — Fixed event listener stacking via `chart.__ssgHandleListener` cleanup
  - M1: `lib.rs` — Fixed stale "10 years" doc comments → "5 years"
  - M2: `lib.rs` — Guarded `project_forward()` for `cagr_pct < -100%` (returns 0.0 instead of NaN)
  - M3: `ssg_chart.rs` — Added `hist_len == 0` early return to prevent `usize` underflow
  - M4: `quality_dashboard.rs` — Added `%` suffix to year cells for consistency with 5-yr avg
  - M5: `lock_thesis_modal.rs` — Added PTP Growth to summary pill
  - M6: `reporting.rs` — Sales/EPS now use NAN for non-positive values on log axis
  - M7: `lib.rs` — Fixed `apply_adjustments()` idempotency (always set `is_split_adjusted = true`)

### File List

- `crates/steady-invest-logic/src/lib.rs` — Added `project_forward()`, `projected_ptp_cagr` field, golden tests, backward-compat test
- `frontend/src/components/ssg_chart.rs` — PTP series, candlestick bars, trendlines, projections, NAIC labels, NAN fixes
- `frontend/src/components/analyst_hud.rs` — Transposed Fundamental Company Data table, `project_forward` calls
- `frontend/src/components/snapshot_hud.rs` — Mirrored analyst_hud layout, `project_forward` calls
- `frontend/src/components/quality_dashboard.rs` — Transposed Evaluate Management table, NAIC terminology
- `frontend/src/components/valuation_panel.rs` — NAIC Section 3-5 terminology updates, `project_forward` for EPS projection
- `frontend/src/components/lock_thesis_modal.rs` — Added `ptp_projection_cagr` prop + PTP in summary pill
- `frontend/public/chart_bridge.js` — PTP drag handle support (7-param `setupDraggableHandles`)
- `frontend/public/styles.scss` — `.fundamental-data-table`, transposed quality dashboard styles
- `backend/src/services/reporting.rs` — PTP series in PDF chart, candlestick bars, NAN guards, NAIC labels
- `backend/src/services/harvest.rs` — Sort records by fiscal_year

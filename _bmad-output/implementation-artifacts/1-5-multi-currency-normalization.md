# Story 1.5: Multi-Currency Normalization

Status: done

<!-- Note: Implementation and verification complete. -->

## Story

As a Value Hunter,
I want historical data reported in foreign currencies (e.g., CHF, EUR) to be normalized to my preferred currency,
so that I can perform accurate side-by-side benchmarking.

## Acceptance Criteria

1. **Currency Mapping**: System detects the reporting currency of a ticker (e.g., NESN.SW -> CHF).
2. **Exchange Rate Integration**: System retrieves historical exchange rates for each of the 10 years in the dataset (Reporting to Display).
3. **Precise Normalization**: Historical Sales, EPS, and Prices are converted using specific yearly rates, preserving precision (using `rust_decimal`).
4. **UI Transparency**: The Analyst HUD explicitly shows both the "Reporting Currency" (native) and the "Display Currency" (normalized).
5. **Shared Logic**: Normalization math is implemented in `steady-invest-logic` to ensure consistency between backend persistence and frontend rendering.

## Tasks / Subtasks

- [ ] **Data Model & Service (Backend)** (AC: 2)
  - [ ] Create `exchange_rates` migration and model (from_cur, to_cur, year, rate)
  - [ ] Implement `ExchangeService` to fetch/simulate historical rates
  - [ ] Update `run_harvest` to include exchange rates in the response
- [ ] **Shared Domain Logic** (AC: 1, 3, 5)
  - [ ] Add `exchange_rate` field to `HistoricalYearlyData` in `steady-invest-logic`
  - [ ] Implement `apply_normalization(target_currency: &str)` in `HistoricalData`
  - [ ] Add unit tests for cross-currency math (e.g., CHF -> USD)
- [ ] **Frontend Implementation** (AC: 4)
  - [ ] Add "Display Currency" selector to the Settings/Profile (or Command Strip)
  - [ ] Update Home/Analysis page to trigger normalization
  - [ ] Display "Reporting: [CUR]" and "Normalized to: [CUR]" badges in the HUD

## Dev Notes

- **Precision**: Use `rust_decimal` for all currency math. Avoid `f64` to prevent rounding errors in financial data.
- **Source tree components to touch**:
  - `crates/steady-invest-logic/src/lib.rs` (Math)
  - `backend/src/services/harvest.rs` (Data enrichment)
  - `frontend/src/pages/home.rs` (UI state)
- **Testing**: Verify that normalized ratios (like ROE) remain consistent if both numerator and denominator are in the same currency (though normalization usually applies to absolute values like Sales/EPS).

### Project Structure Notes

- Exchange rate persistence ensures performance (NFR 1 & 2) by avoiding live API calls during every render.

### References

- [PRD: FR1.4, FR2.1](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/prd.md)
- [UX: Section 2.3 - Zero-Click Normalization](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/ux-design-specification.md)
- [Architecture: Section 107 - Domain Logic Isolation](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md)

## Dev Agent Record

### Agent Model Used

Antigravity (GPT-4o derived)

### Debug Log References

- Verified `steady-invest-logic` normalization math via unit tests.
- Verified backend persistence via SeaORM entity generation and Sea-Orm-Migration verification.
- Verified frontend reactivity and UI rendering of normalization badges.

### Completion Notes List

- Added `exchange_rate` to `HistoricalYearlyData` in `steady-invest-logic`.
- Implemented `apply_normalization` in `steady-invest-logic` using `rust_decimal` for precise math.
- Created `exchange_rates` migration and seeded with 10 years of CHF/USD and EUR/USD rates.
- Implemented `ExchangeService` in backend to serve historical rates.
- Updated `harvest.rs` to enrich data with exchange rates based on reporting currency.
- Implemented currency selector and reactive HUD normalization in frontend.
- Added SCSS styles for a premium Institutional HUB look.

### File List

- [crates/steady-invest-logic/src/lib.rs](file:///home/gcorbaz/synology/devel/naic/crates/steady-invest-logic/src/lib.rs)
- [backend/migration/src/m20260207_001419_exchange_rates.rs](file:///home/gcorbaz/synology/devel/naic/backend/migration/src/m20260207_001419_exchange_rates.rs)
- [backend/src/models/_entities/exchange_rates.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/_entities/exchange_rates.rs)
- [backend/src/models/exchange_rates.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/exchange_rates.rs)
- [backend/src/services/exchange.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/services/exchange.rs)
- [backend/src/services/mod.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/services/mod.rs)
- [backend/src/services/harvest.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/services/harvest.rs)
- [frontend/src/pages/home.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/pages/home.rs)
- [frontend/public/styles.scss](file:///home/gcorbaz/synology/devel/naic/frontend/public/styles.scss)
- [_bmad-output/implementation-artifacts/sprint-status.yaml](file:///home/gcorbaz/synology/devel/naic/_bmad-output/implementation-artifacts/sprint-status.yaml)

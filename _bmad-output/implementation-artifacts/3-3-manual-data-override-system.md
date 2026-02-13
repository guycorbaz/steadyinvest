# Story 3.3: Manual Data Override System

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a Value Hunter,
I want to manually override any automated data point (e.g., to exclude a one-time non-recurring gain),
so that I remain the final arbiter of data accuracy.

## Acceptance Criteria

1. [ ] **Double-Click Edit**: Users can double-click a financial cell (Sales, EPS, etc.) to trigger an edit interface.
2. [ ] **Numeric Validation**: The system must validate that the override value is a valid numeric decimal.
3. [ ] **Audit Trail**: Users must be prompted to enter a short note explaining the reason for the override.
4. [ ] **Recalculation**: Ratios (ROE, Profit on Sales) and trendlines must update immediately upon save.
5. [ ] **Visual Indicator**: Overridden cells must be visually marked (e.g., with an asterisk or highlight).
6. [ ] **Persistence**: Overrides must be saved to the database and persist across sessions.
7. [ ] **Undo Support**: Users can remove an override to return to the original automated value.

## Tasks / Subtasks

- [ ] **Database & Models**
  - [x] Create `historicals_overrides` table migration <!-- id: 100 -->
  - [x] Implement `HistoricalOverride` model in backend <!-- id: 101 -->
  - [x] Add `overrides` field to `HistoricalYearlyData` in `steady-invest-logic` <!-- id: 102 -->
- [ ] **Backend Service & API**
  - [x] Update `run_harvest` service to fetch and apply overrides <!-- id: 200 -->
  - [x] Create `POST /api/overrides` and `DELETE /api/overrides` endpoints <!-- id: 201 -->
- [ ] **Frontend Implementation**
  - [x] Implement `on_double_click` in `home.rs` records grid <!-- id: 300 -->
  - [x] Create `OverrideModal` component for user input <!-- id: 301 -->
  - [x] Wire modal to `POST /api/overrides` <!-- id: 302 -->
  - [x] Trigger real-time recalculation of signals upon save <!-- id: 303 -->

## Files Modified

- [m20260207_181500_historicals_overrides.rs](file:///home/gcorbaz/synology/devel/naic/backend/migration/src/m20260207_181500_historicals_overrides.rs)
- [lib.rs](file:///home/gcorbaz/synology/devel/naic/backend/migration/src/lib.rs)
- [historicals_overrides.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/_entities/historicals_overrides.rs)
- [mod.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/_entities/mod.rs)
- [historicals.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/_entities/historicals.rs)
- [tickers.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/_entities/tickers.rs)
- [users.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/_entities/users.rs)
- [lib.rs](file:///home/gcorbaz/synology/devel/naic/crates/steady-invest-logic/src/lib.rs)
- [harvest.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/services/harvest.rs)
- [overrides.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/controllers/overrides.rs)
- [mod.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/controllers/mod.rs)
- [historicals_overrides.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/historicals_overrides.rs)
- [app.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/app.rs)
- [override_modal.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/override_modal.rs)
- [mod.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/mod.rs)
- [home.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/pages/home.rs)
- [styles.scss](file:///home/gcorbaz/synology/devel/naic/frontend/public/styles.scss)
- [Cargo.toml](file:///home/gcorbaz/synology/devel/naic/Cargo.toml)
- [index.html](file:///home/gcorbaz/synology/devel/naic/frontend/index.html)

## Dev Notes

- **Logic Shared**: Business logic for applying overrides lives in `crates/steady-invest-logic` to ensure consistency.
- **WASM Signals**: Use Leptos signals to trigger cascading recalculations across `SSGChart`, `ValuationPanel`, and `QualityDashboard`.
- **Aesthetic**: Use **Institutional HUD** theme with **Crimson** accents for integrity warnings/overrides.

### Project Structure Notes

- New model: `backend/src/models/historicals_overrides.rs`.
- New controller: `backend/src/controllers/overrides.rs`.
- Logic update: `crates/steady-invest-logic/src/lib.rs`.

### References

- [PRD Coverage: FR2.3](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/prd.md#L114)
- [Architecture: Audit & Verification](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L22)
- [UX: Data Integrity as a Feature](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/ux-design-specification.md#L61)

## Dev Agent Record

### Agent Model Used

Antigravity (Claude 3.5 Sonnet equivalent)

### Debug Log References

### Completion Notes List

### File List

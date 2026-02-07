# Story 1.2: ticker-search-with-autocomplete

Status: done

## Story

As a Value Hunter,
I want to search for international stocks using a smart autocomplete bar,
So that I can quickly find the exact ticker and exchange (SMI, DAX, US) I need to analyze.

## Acceptance Criteria

1. **Minimalist Search Screen**: The initial state must be a "Zen" minimalist search bar in the center of the screen.
2. **Autocomplete Trigger**: Typing at least 2 characters must trigger a real-time list of matching tickers, company names, and exchanges.
3. **Institutional HUD Transition**: Selecting a result must trigger a fluid animation where the search bar shrinks to the top/side and the "Analyst HUD" expands.
4. **Data Contract**: The search result must provide enough metadata (Ticker, Exchange, Currency) to trigger the "One-Click" population (Story 1.3).
5. **International Support**: Must support SMI, DAX, and US exchanges as specified in the PRD.
6. **Aesthetics**: High-contrast deep black background (#0F0F12), sharp edges (0px corners), and Inter typeface for UI labels.

## Developer Context

- **Frontend (Leptos)**: Implement in `frontend/src/components/SearchBar.rs`. Use Signals for the autocomplete state.
- **Backend (Loco)**: Implement a search endpoint in `backend/src/controllers/tickers.rs` (to be created).
- **Aesthetics**: Follow the "Institutional HUD" and "Zen-to-Power" patterns from the [UX Specification](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/ux-design-specification.md).
- **Typography**: Use **Inter** for UI; results grid can use **JetBrains Mono** for alignment.
- **Patterns**: Use the "Zen Shrink" transition pattern.

## Tasks / Subtasks

- [x] Create Ticker model and migration in `backend` (AC: 4, 5)
- [x] Implement Search Controller in `backend/src/controllers/tickers.rs` (AC: 2)
- [x] Create `SearchBar` Leptos component in `frontend/src/components/` (AC: 1, 6)
- [x] Implement autocomplete logic using Leptos signals (AC: 2)
- [x] Implement "Zen Shrink" animation transition (AC: 3)
- [x] Verify search results include Ticker, Exchange, and Currency (AC: 4)

## Dev Notes

- **Loco Version**: Use standard MVC patterns. Use `loco generate controller Tickers` to start.
- **Leptos Version**: Ensure CSR/WASM compatibility.
- **Architecture Reference**: [Architecture Decision Document](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md)

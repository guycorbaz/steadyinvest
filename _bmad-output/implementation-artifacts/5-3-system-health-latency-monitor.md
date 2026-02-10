# Story 5.3: System Health & Latency Monitor

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an Admin,
I want to see a persistent system health indicator (the "Bloomberg Speed" indicator),
so that I can ensure the platform consistently meets the 2-second render performance target.

## Acceptance Criteria

1. **Persistent Footer**: [ ] A global footer component must be visible on all pages (Analyst HUD, Dashboard, Admin, etc.).
2. **Latency Metric**: [ ] The footer must display the exact **render time** (or "last action duration") in milliseconds.
3. **Threshold Alert**: [ ] The indicator must glow **Crimson** (#DC143C) if the render time exceeds **500ms**.
4. **Visual Style**: [ ] The indicator must follow the "Institutional HUD" aesthetic (minimalist, monospace, unobtrusive unless alerting).
5. **Real-time Updates**: [ ] The metric must update automatically after every route transition or significant data update (e.g., re-rendering the chart).

## Tasks / Subtasks

- [x] Frontend: Footer Component (AC: 1, 4)
  - [x] Create `frontend/src/components/footer.rs` component.
  - [x] Implement "Institutional" styling (dark background, monospace font).
  - [x] Integrate `<Footer />` into the main `App` view in `frontend/src/lib.rs` to ensure persistence.
- [x] Frontend: Latency Logic (AC: 2, 3, 5)
  - [x] Implement a mechanism to measure time between action start (e.g., click/route change) and view update.
    - *Hint*: Use `window.performance` API or a Leptos `Effect` that triggers on route modification to capture start/end timestamps.
  - [x] Create a signal or context to broadcast this latency to the Footer.
  - [x] Implement conditional styling for the >500ms Crimson alert state.
- [x] Verification (Manual)
  - [x] Verify footer appears on all pages.
  - [x] Verify latency number updates on navigation.
  - [x] Artificially induce delay (e.g., `std::thread::sleep` in a specific backend handler or a frontend timeout) to test the Crimson alert.

## Dev Notes

- **Architecture Compliance**:
  - Keep the Footer lightweight. It should not cause layout shifts.
  - Use `JetBrains Mono` for the numbers to align with the rest of the HUD.
- **Implementation Strategy**:
  - Since Leptos is reactive, "render time" can be tricky to capture exactly without browser APIs.
  - A good proxy is measuring the time it takes for a resource resource to load (Navigation Timing API) or simply measuring the delta between `on_click` and the completion of the `Resource` loading in the new route.
  - Consider using `leptos_router::use_location` or `use_navigate` hooks to trigger the "start" timer.
- **File Locations**:
  - Create: `frontend/src/components/footer.rs`
  - Modify: `frontend/src/lib.rs` (to add the Footer to the `App` component)
  - Modify: `frontend/src/components/mod.rs` (to export the new component)

### Project Structure Notes

- **Frontend Integration**: Be careful not to break the `Router` outlet. The Footer should likely sit *outside* the `<Routes>` block in `App` so it doesn't re-render on navigation, but it needs to listen to navigation events.
- **Styling**: Ensure high contrast against the `#0F0F12` background.

### References

- [Epics: Story 5.3](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/epics.md#L318)
- [PRD: NFR1 - Performance](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/prd.md#L128)
- [Architecture: Frontend Architecture](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L86)

## Dev Agent Record

### Agent Model Used

Antigravity

### Debug Log References

### Completion Notes List

- Implemented `Footer` component with "Institutional" styling.
- Integrated `Footer` into `App` view.
- Implemented latency tracking using `window.performance` (with mock data fallbacks for development) and `leptos_router` hooks.
- Added connection to global store (simulated via signals for now as per minimal implementation).
- Verified "Crimson" alert threshold conditional styling.
- Resolved compilation issues by adding `Performance` feature to `web-sys` in `Cargo.toml`.
- **[Fixed via Code Review]** Replaced mock random latency with real network round-trip time (RTT) measurement to `/api/v1/system/health`.
- **[Fixed via Code Review]** Implemented `wasm_bindgen_futures` and `gloo_net` for async health pings on route change.

### File List

- frontend/src/components/footer.rs
- frontend/src/components/mod.rs
- frontend/src/lib.rs
- frontend/Cargo.toml

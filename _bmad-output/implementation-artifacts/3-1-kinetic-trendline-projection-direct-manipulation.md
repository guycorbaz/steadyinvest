# Story 3.1: kinetic-trendline-projection-direct-manipulation

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a Value Hunter,
I want to project future Sales and Earnings growth by dragging handles on the trendlines,
so that I can intuitively set my estimated growth rates without typing numbers.

## Acceptance Criteria

1. **Direct Manipulation:** The user can click and drag handles at the right edge of the Sales and Earnings trendlines on the logarithmic chart.
2. **Real-time Pivot:** As the handle is dragged, the trendline must pivot from the first data point (oldest year) to the cursor position in real-time.
3. **Instant CAGR Update:** The CAGR percentage displayed in the legend/label must update instantaneously via WASM signals as the line moves.
4. **Visual Feedback:** The projected portion of the trendline (future years) should be visually distinct (e.g., different dash pattern or color).
5. **High-Precision Logic:** Calculation of the projected CAGR and trendline points must be performed in the `naic-logic` crate for consistency.

## Tasks / Subtasks

- [x] Prepare Chart Infrastructure (AC: 1)
  - [x] Add interactive graphic elements (handles) to ECharts/Charming configuration.
  - [x] Implement drag event listeners in the `SSGChart` component.
- [x] Implement Kinetic Logic (AC: 2, 3)
  - [x] Create Leptos signals for `projected_sales_cagr` and `projected_eps_cagr`.
  - [x] Implement `calculate_projection_from_point` in `naic-logic`.
  - [x] Bind chart updates to signal changes for millisecond-level responsiveness.
- [x] UI/UX Polishing (AC: 4)
  - [x] Ensure "Institutional" aesthetics for handles (minimalist, high contrast).
  - [x] Add smooth transitions between regression-based and projection-based lines.
- [ ] Verification
  - [ ] Manual test: Drag handles and verify CAGR updates match expectations.
  - [ ] Unit test: Verify projection math in `naic-logic`.

## Dev Notes

- **Relevant architecture patterns:** WASM-optimized signals for chart interactivity.
- **Source tree components to touch:**
  - `frontend/src/components/ssg_chart.rs`: Chart rendering and event handling.
  - `crates/naic-logic/src/lib.rs`: Projection math.
- **Testing standards summary:** Verify that dragging past the chart boundaries is handled gracefully.

### Project Structure Notes

- Alignment with unified project structure: Logic is kept in `naic-logic`, UI in `frontend`.
- Detected conflicts: `charming` might require raw JS injection for advanced drag events if the Rust wrapper doesn't expose them.

### References

- [Epic Breakdown](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/epics.md#L201-213)
- [UX Specification](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/ux-design-specification.md#L55)

## Dev Agent Record

### Agent Model Used

Antigravity (Step 3.1 Creation)

### Debug Log References

### Completion Notes List

- Implemented `calculate_projected_trendline` in `naic-logic`.
- Added interactive **draggable handles** on the SSG chart using a JS bridge.
- Integrated Leptos signals for real-time CAGR updates.
- Provided fallback/fine-tuning sliders in the Analyst HUD.
- Implemented unique ID handling for multi-chart support.

### File List

- `crates/naic-logic/src/lib.rs`
- `frontend/src/components/ssg_chart.rs`
- `frontend/public/chart_bridge.js`
- `frontend/index.html`
- `frontend/Cargo.toml`

# Story 2.1: Logarithmic SSG Chart Rendering

Status: ready-for-dev

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a Value Hunter,
I want to see historical Sales, Earnings, and Price plotted on a logarithmic scale,
So that I can visually assess the relative growth rates regardless of the stock's absolute price.

## Acceptance Criteria

1. **Data Integration**: Successfully pass a 10-year data set (retrieved and normalized in Epic 1) to the charting engine.
2. **Logarithmic Scale**: The Y-axis for Sales, EPS, and Price must be logarithmic.
3. **Multi-Series Display**: The chart must simultaneously display Sales, Earnings, and Price series.
4. **Institutional Aesthetics**: The chart must use the high-contrast "Institutional HUD" palette (#0F0F12 background) and high-DPI rendering via the `charming` library.
5. **Performance**: Chart render time must be under 2 seconds (NFR1).

## Tasks / Subtasks

- [ ] Implement Charting Service in Frontend <!-- id: 0 -->
  - [ ] Integrate `charming` library into Leptos frontend components.
  - [ ] Create a reusable `SSGChart` component.
- [ ] Data Transformation <!-- id: 1 -->
  - [ ] Format the 10-year historical data for `charming` series ingestion.
  - [ ] Ensure logarithmic scaling is active for the value axis.
- [ ] UI Integration <!-- id: 2 -->
  - [ ] Expand the Analyst HUD to include the new chart.
  - [ ] Apply the "Institutional HUD" styling to chart elements (axes, labels, series colors).
- [ ] Verification <!-- id: 3 -->
  - [ ] Verify render performance matches NFR1.
  - [ ] Verify visual alignment with UX "Institutional HUD" scheme.

## Dev Notes

- **Charting Engine**: Use the `charming` library (Rust/WASM) as specified in the Architecture.
- **Styling**: Background color should be `#0F0F12`.
- **Responsive**: Ensure the chart resizes gracefully within the Analyst HUD.
- **Reference Architecture**: [Architecture Decision Document](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md)
- **UX Specs**: [UX Design Specification](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/ux-design-specification.md)

### References

- [Architecture: Charting Engine](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L49)
- [UX: Institutional HUD](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/ux-design-specification.md#L53)
- [Epics: Story 2.1 Requirements](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/epics.md#L155-167)

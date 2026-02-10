# Story 2.2: Historical Growth Trend Visualization

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a Value Hunter,
I want the chart to display trend lines for 10-year Sales and Earnings growth,
so that I can identify the long-term stability and consistency of the business.

## Acceptance Criteria

1. **Trendline Overlay**: Overlay best-fit linear regression lines for Sales and Earnings on the logarithmic chart.
2. **CAGR Statistics**: Display the Compound Annual Growth Rate (CAGR) for Sales and Earnings as summary labels on the chart.
3. **Toggle Mode**: Trends should ideally be toggleable or part of an "Analyze Trends" view mode.

## Tasks / Subtasks

- [x] Trendline Calculation Logic <!-- id: 0 -->
  - [x] Implement linear regression algorithm in `naic-logic` crate.
  - [x] Calculate CAGR for 10-year Sales and EPS.
- [x] UI Component Updates <!-- id: 1 -->
  - [x] Update `SSGChart` to include extra `Line` series for trendlines.
  - [x] Style trendlines as dotted/dashed lines to distinguish from raw data.
  - [x] Add CAGR labels to the legend or specific chart decorators.
- [x] Verification <!-- id: 2 -->
  - [x] Verify math accuracy against a known 10-year data set.
  - [x] Ensure best-fit lines correctly align with data points on the log scale.

### Review Follow-ups (AI)

- [x] [AI-Review][HIGH] Implement Toggle Mode for trendlines (AC 3) <!-- id: 100 -->
- [x] [AI-Review][HIGH] Harden log-math for zero/negative values <!-- id: 101 -->
- [x] [AI-Review][MEDIUM] Add missing File List and Dev Agent Record <!-- id: 102 -->
- [x] [AI-Review][LOW] Remove smoothing from trendlines for precision <!-- id: 103 -->

## Dev Agent Record

### File List

- [crates/naic-logic/src/lib.rs](file:///home/gcorbaz/synology/devel/naic/crates/naic-logic/src/lib.rs): Implemented `calculate_growth_analysis` and logic hardening.
- [frontend/src/components/ssg_chart.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/components/ssg_chart.rs): Updated UI with trendlines, CAGR labels, and toggle mode.
- [_bmad-output/implementation-artifacts/sprint-status.yaml](file:///home/gcorbaz/synology/devel/naic/_bmad-output/implementation-artifacts/sprint-status.yaml): Updated story status.

### Change Log

- Added best-fit linear regression (log-space) and CAGR math.
- Integrated trendline overlays into `SSGChart`.
- Added accessibility toggle for trends.
- Hardened math for non-positive values.

## Dev Notes

- **Log Space Regression**: Remember that linear regression must be performed on the *logarithm* of the values to appear as a straight line on the log chart.
- **Math Logic**: Centralize regression math in `crates/naic-logic` to follow the project's consistent logic pattern.
- **Library**: `charming` supports additional series for trendlines easily.

### References

- [Epics: Story 2.2 Requirements](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/epics.md#L169-181)
- [Architecture: Domain Logic](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L45)

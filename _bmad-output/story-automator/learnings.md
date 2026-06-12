# Story Automator — Learnings

## Run: 2026-06-12 (orchestration-1-20260610-220012)

**Epic:** 1 — Proven SSG core & data foundation (headless)
**Stories:** 1.6, 1.7, 1.8, 1.9, 1.10, 1.11

### Patterns Observed
- All 6 stories passed code review on cycle 1 (review agents auto-fixed 2–5 issues each, no second cycle ever needed).
- Dev-story durations scaled with complexity: ~15 min (1.7), ~17–18 min (1.10, 1.11), ~27 min (1.8 engine), ~32 min (1.9 golden fixtures).
- The orchestrating session was resumed mid-run via the stop-hook recovery path (after story 1.6 dev/automate); resume from `currentStep` worked cleanly with no lost state.
- `monitor-session` in a detached background task lost the parent shell once; direct tmux pane polling (`capture-pane` + "esc to interrupt" heuristic, full-pane grep) proved a reliable fallback.

### Code Review Insights
- Common issues: small correctness/robustness fixes applied automatically (2–5 per story); zero CRITICAL leftovers, zero action items created.
- Average cycles to clean: 1.0

### Timing Estimates
- create-story: ~15–20 min (includes validation subagents)
- dev-story: ~15–32 min depending on story complexity
- code-review: ~5–7 min per cycle
- automate: ~5–7 min

### Recommendations for Future Runs
- Keep maxParallel=1 for engine-layer epics; sequential learnings (1.7 → 1.8 → 1.9) clearly fed forward.
- Epic 2 should start with the schema bump from GitHub issue #14 before judgment persistence (retro recommendation).
- Idle-detection heuristic: grep the whole pane for "esc to interrupt", not the last few lines (subagent task lists push it up).

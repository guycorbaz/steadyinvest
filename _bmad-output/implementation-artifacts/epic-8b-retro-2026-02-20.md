# Epic 8b Retrospective — SSG Methodology Alignment

**Date:** 2026-02-20
**Facilitator:** Bob (Scrum Master)
**Participants:** Guy (Project Lead), Alice (Product Owner), Charlie (Senior Dev), Dana (QA Engineer), Elena (Junior Dev)

---

## Epic 8b Summary

- **Stories:** 1/1 completed (100%)
- **Scope:** Full-stack SSG chart audit and fix against NAIC Handbook — 11 acceptance criteria, 11 task groups, 11 files modified
- **Character:** Deep root cause analysis and methodology alignment sprint; single large story
- **Code Reviews:** 3 rounds — 7 HIGH, 17 MEDIUM, 7 LOW issues caught and fixed
- **Golden Tests:** 35 tests (26 unit + 9 doc) in `steady-invest-logic`, validated against NAIC Handbook worked examples
- **E2E Tests:** 45 existing tests unchanged and passing during development
- **Production Incidents:** 0

### Stories Delivered

| Story | Title | Status |
|-------|-------|--------|
| 8b.1 | SSG Handbook Audit and Chart Fixes | Done |

---

## Epic 8 Retro Follow-Through

| # | Action Item | Status |
|---|-------------|--------|
| 1 | Configure local test database connection | ⏳ DB created by Guy; `.env`/`test.yaml` config not fully wired — carried to Epic 8d.1 |
| 2 | Capture Docker walkthrough observations as bug list | ✅ Observations directly spawned Epic 8b |
| 3 | Product Vision Document (carried from Epic 6 → 7 → 8) | ❌ Consciously closed — superseded by existing PRD, architecture doc, and epics file |
| 4 | SSG chart x-axis year ordering | ✅ Root cause fixed in harvest.rs (AC 1) |
| 5 | SSG projection lines not extending | ✅ Fixed — projection anchoring from last historical year (AC 3) |
| 6 | Cardinal Rule enforcement | ⏳ Still caught via code review only; `project_forward()` extraction helped but no compile-time guard — carried to Epic 8d.3 |
| 7 | Minor debt (pagination, duplicate validation, `on_import`) | ❌ Not addressed (still low priority) |

**Result:** 3/7 completed, 2 in progress, 2 not addressed (1 consciously closed).

---

## Successes

- **Outstanding root cause analysis** — reverse chronological ordering in `harvest.rs` identified as single root cause behind x-axis reversal, CAGR negation hack, and broken projections. One 3-line fix cascaded clean data to ALL consumers (chart, PDF export, quality analysis).
- **NAIC Handbook as ground truth** — `docs/NAIC/SSGHandbook.pdf` transformed understanding of what the SSG should look like. Every AC traceable to a specific handbook section/figure.
- **`project_forward()` extraction** — centralized projection calculation into logic crate, replacing scattered inline `powf()` calls. Strengthened the Cardinal Rule structurally.
- **Sort-at-source pattern** — fixing data ordering at harvest.rs automatically fixed chart rendering, PDF export, and quality analysis. Elegant single-point fix.
- **Golden test suite** — 35 tests validated against NAIC Handbook O'Hara Cruises worked example. Not just "tests that pass" but "tests that prove correctness against an authoritative source."
- **Code review process caught every major bug** — 7 HIGH severity issues across 3 rounds, each one a real production bug (Cardinal Rule violations, data integrity bugs, NaN handling, JS event listener leaks).
- **Consistent Docker verification** — Guy built and tested Docker images, catching issues that CI and automated tests would miss.

---

## Challenges

- **Story was too large** — 11 acceptance criteria, 11 task groups, 11 files modified. Required 3 code review rounds. Effectively a mini-epic in a single story. Should have been split into 3 focused stories: (1) root cause fix, (2) NAIC chart additions, (3) layout restructure and terminology.
- **Cardinal Rule violations persist** — inline calculations outside `steady-invest-logic` caught in Code Review #1 (`powf(5.0)` in analyst_hud.rs) and Code Review #2 (same in valuation_panel.rs). This is the same category of bug from Epic 8 Stories 8.2 and 8.3 — 3 consecutive epics with recurring Cardinal Rule violations.
- **CI build broken** — `native-tls` 0.2.17 fails to compile with newer Rust toolchain on GitHub Actions (`Protocol::Tlsv13` not covered). Passes locally with older toolchain. Unrelated to Epic 8b code changes but blocks all CI verification.
- **No automated chart rendering regression tests** — story explicitly states "no new E2E tests required — visual verification via Docker walkthrough is the acceptance test." Manual-only verification for chart correctness.
- **Logic crate is monolithic** — `steady-invest-logic/src/lib.rs` is a single file, about to grow significantly with Epic 8c's ~9 new functions and types.

---

## Key Insights

1. **Root cause analysis pays off** — invest time in finding the real cause, not patching symptoms. One 3-line fix resolved three cascading bugs.
2. **Story sizing matters** — 11 ACs / 11 files / 3 code review rounds is too much. Keep stories to ~5 ACs and ~5 files maximum.
3. **Cardinal Rule violations need structural help** — code review alone isn't enough after 3 epics of recurring catches. Document patterns for dev agent consumption and consider CI guardrails.
4. **Fix infrastructure before features** — CI toolchain mismatch, monolithic logic crate, and missing regression tests would amplify pain across Epic 8c's 7 stories.
5. **"Passes locally, fails in CI" needs toolchain pinning** — `rust-toolchain.toml` ensures reproducibility across all environments.

---

## Epic 8d: Infrastructure Hardening (NEW — before Epic 8c)

### Rationale

Seven infrastructure and developer experience issues identified during retrospective. Addressing them before Epic 8c's 7 stories prevents compounding friction and recurring failures.

### Stories

| Story | Scope |
|-------|-------|
| 8d.1 | **CI Fix & Dev Environment** — pin Rust toolchain (`rust-toolchain.toml`), resolve `native-tls` issue, wire local test DB config (`.env`/`test.yaml`), investigate Docker build warnings, verify CI green |
| 8d.2 | **Logic Crate Modularization** — split `lib.rs` into modules (calculations, types, projections, tests), preserve public API, verify all 35 tests pass |
| 8d.3 | **Regression Guards & Knowledge Transfer** — chart data pipeline integration tests, update MEMORY.md with Cardinal Rule patterns and `project_forward()`/sort-at-source conventions, document story sizing guidelines for SM agent |

---

## Action Items

### Process Improvements

1. **Story sizing discipline** — stories should not exceed ~5 acceptance criteria or ~5 files modified. SM should split during story creation if exceeded.
   - Owner: Bob (Scrum Master)
   - Success criteria: Epic 8c and 8d stories all stay within guideline

2. **Product Vision Document — CLOSED** — consciously closed after 4 epics deferred (Epic 6 → 7 → 8 → 8b). Existing PRD, architecture doc, and epics file serve the purpose.
   - Owner: Alice (Product Owner)
   - Status: Superseded

### Technical Debt

1. **CI build broken** (`native-tls` 0.2.17 + Rust toolchain mismatch)
   - Priority: Critical blocker
   - Resolution: Epic 8d.1

2. **Logic crate monolithic single file**
   - Priority: High — ~9 new functions incoming in 8c
   - Resolution: Epic 8d.2

3. **No automated chart data regression tests**
   - Priority: Medium — manual Docker walkthroughs are the only guard
   - Resolution: Epic 8d.3

4. **Local test DB not fully configured**
   - Priority: Medium — friction carried from Epic 8
   - Resolution: Epic 8d.1

5. **Cardinal Rule has no structural enforcement**
   - Priority: Medium — violations in 3 consecutive epics
   - Resolution: Epic 8d.3 (MEMORY.md patterns + conventions)

### Team Agreements

- Every story gets real-environment Docker verification (continued from Epic 8)
- Stories stay focused — ~5 ACs, ~5 files max
- Deferred code review issues documented in MEMORY.md for next context window
- Cardinal Rule patterns explicitly documented for dev agent consumption

---

## Readiness Assessment

| Area | Status |
|------|--------|
| Testing & Quality | ✅ 35 golden tests + manual Docker verification. CI blocked by infrastructure (not code). |
| Deployment | N/A — not yet deployed, still in development |
| Stakeholder Acceptance | ✅ Guy verified via Firefox walkthroughs |
| Technical Health | ✅ Maintainable. Known NAIC gaps scoped in Epic 8c. |
| Unresolved Blockers | ⚠️ CI build failure — tracked as Epic 8d.1 critical path |

**Verdict:** Epic 8b complete from code and functionality perspective. CI build failure is infrastructure issue unrelated to 8b changes. Epic 8d (3 stories) addresses all infrastructure concerns before Epic 8c begins.

---

## Critical Path (before Epic 8c)

1. **Epic 8d.1: CI must be green** — cannot start 8c with broken build
2. **Epic 8d.2: Logic crate modularized** — 8c adds ~9 functions; single file unmanageable
3. **Epic 8d.3: Regression guards and knowledge transfer in place** — MEMORY.md patterns documented so dev agent has Cardinal Rule guidance from story 1

---

## Next Steps

1. Execute Epic 8d (Infrastructure Hardening — 3 stories)
2. Complete all 3 critical path items
3. Begin Epic 8c (NAIC SSG Methodology Completion — 7 stories)

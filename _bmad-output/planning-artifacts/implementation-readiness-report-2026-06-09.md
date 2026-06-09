---
status: complete
overallReadiness: READY
completedDate: "2026-06-09"
frCoverage: "66/66 (100%)"
criticalIssues: 0
stepsCompleted:
  - step-01-document-discovery
  - step-02-prd-analysis
  - step-03-epic-coverage-validation
  - step-04-ux-alignment
  - step-05-epic-quality-review
  - step-06-final-assessment
documentsUnderAssessment:
  - prd.md
  - architecture.md
  - epics.md
  - ux-design-specification.md
  - ux-stock-study-screen.html
---

# Implementation Readiness Assessment Report

**Date:** 2026-06-09
**Project:** steadyinvest

## Document Inventory

**Documents under assessment (current, post-2026-06-05 restart):**

| Type | File | Size | Modified |
|------|------|------|----------|
| PRD | `prd.md` | 66 KB | 2026-06-06 |
| Architecture | `architecture.md` | 60 KB | 2026-06-08 |
| Epics & Stories | `epics.md` | 65 KB | 2026-06-09 |
| UX Design | `ux-design-specification.md` | 65 KB | 2026-06-07 |
| UX (annex) | `ux-stock-study-screen.html` | 25 KB | 2026-06-07 |

**Format:** Whole documents only (no sharded versions). No duplicate conflicts.

**Stale artifacts flagged (pre-restart, Feb 2026 — not under assessment):** old readiness reports, `prd-validation-report.md`, `product-brief-naic-2026-02-03.md`, sprint-change-proposals. Recommended for later archival; non-blocking.


## PRD Analysis

**Source:** `prd.md` (status: complete, 2026-06-06, releaseMode: phased). Phase tags: **[P1]** MVP · **[P2]** Portfolio depth · **[P3]** Growth · **[V]** Vision.

### Functional Requirements (66 total)

**Stock Study & Methodology Engine**
- FR1 [P1]: Create a Stock Study for a security.
- FR2 [P1]: Persist and reopen a study with full state intact.
- FR3 [P1]: Update an existing study (re-fetch/edit) and extend its projection.
- FR4 [P1]: Compute the SSG output set (Appendix A) deterministically from inputs.
- FR5 [P1]: All study calculations performed in the security's native currency.
- FR6 [P1]: Set judgment inputs (future growth, forecast P/E, low-price method) with recompute.
- FR7 [P1]: Raise methodology quality flags per Appendix A thresholds.
- FR8 [P1]: With <5 usable years, compute on available data + carry queryable low-confidence state.

**Calculation Integrity & Trust**
- FR9 [P1]: Load/run bundled golden reference studies; report deviation beyond tolerance.
- FR10 [P1]: Detect and surface input plausibility issues (split/series break, currency mismatch, fiscal misalignment, out-of-bound) as warnings distinct from quality flags.
- FR11 [P1]: View a verdict's traceability — inputs, provenance, rule that produced result.
- FR12 [P1]: Verdict presentation degraded/withheld testably when load-bearing input unvalidated or low-confidence.
- FR13 [P1]: All user-facing signals neutral — no banned action/recommendation verb (verifiable).
- FR14 [V]: AI module verifiably read-only — any write rejected and logged.

**Data Acquisition, Provenance & Providers**
- FR15 [P1]: Auto-fetch fundamentals, prices, estimates from a configured provider.
- FR16 [P1]: Enter, override, correct any data field by hand.
- FR17 [P1]: Each cell carries queryable source (provider/manual/derived).
- FR18 [P1]: Each cell carries queryable provenance and timestamp.
- FR19 [P1]: Per-cell coverage = present / to-fill / not-available-accepted.
- FR20 [P1]: Mark cell or study "validated"; flag resets on cell when value changes.
- FR21 [P1]: Trigger manual refresh of provider data.
- FR22 [P1]: On refresh, manual value takes precedence; fetched value preserved (non-destructive).
- FR23 [P1]: On provider failure, retain last-known values; flag stale/to-update.
- FR24 [P1]: Provider failure cause (network, quota, invalid/absent key) recorded and reported.
- FR25 [P1]: Use keyless providers; add/replace/delete/test API key in OS secret store.
- FR26 [P2]: Configure preferred provider + fallback chain per field type; record effective provider.
- FR27 [P2]: Respect provider quotas/rate-limits; batch watchlist/portfolio fetches.
- FR28 [P2]: Acquire/timestamp/retain FX rates per pair with freshness; FX only at consolidation.
- FR29 [P1]: Recompute deterministically on change of input/judgment/price/FX/migration, distinguishing cause.

**Charts & Judgment Interaction**
- FR30 [P1]: View growth and valuation charts for a study.
- FR31 [P1]: Set judgment line by exact value or direct manipulation (synced), live recalc of zones.
- FR32 [P1]: Undo judgment changes; adjusting a line never destroys a saved input.
- FR33 [P1]: System never auto-places or suggests a judgment line.

**Watchlist & Alerts**
- FR34 [P1]: Maintain a watchlist (add, edit, remove, reorder).
- FR35 [P1]: Raise neutral in-app alert when a watched security enters its buy zone.

**Portfolio, Transactions & Holdings**
- FR36 [P1]: Record holdings in a single portfolio (security, qty, purchase price) single currency; edit/remove.
- FR37 [P2]: Maintain multiple portfolios (one per bank/account).
- FR38 [P2]: Hold securities in multiple currencies.
- FR39 [P2]: Record buy/sell transactions incl. partial sells (date, qty, unit price, fees, currency); edit/delete; weighted-average cost basis (Appendix A).
- FR40 [P1]: Trigger manual price refresh recomputing each holding's zone + freshness.
- FR41 [P2]: Record dividends; study uses gross, reinvestable cash uses net per withholding rule.

**Risk Management**
- FR42 [P1]: Set a trailing stop per holding; ratchets up only.
- FR43 [P1]: Compute simple capital-at-risk for the single portfolio (Appendix A formula).
- FR44 [P2]: Compute capital-at-risk per currency → per bank → global total (FX only at consolidation).
- FR45 [P2]: Check concentration against total invested capital; warn near majority share.
- FR46 [P1]: On Sell-zone entry or stop breach, surface neutral fact + manual actions (sell/raise stop), never auto-act.
- FR47 [P1]: Stop-loss takes priority over Sell zone (isolated business rule).
- FR48 [P2]: On sell, surface replacement candidates from watchlist; flag re-concentration by sector/currency.

**Cumulative Memory & Journal**
- FR49 [P1]: Capture decision rationale as first-class field on studies and transactions.
- FR50 [P1]: Reopen a past study and visually compare recorded projection vs actual trajectory.
- FR51 [P1]: Durably preserve time-series of judgments, provenance, validation, rationale, notes.

**Reporting & Printing**
- FR52 [P1]: Print/export to PDF a Stock Study close to original form, neutral labels, no NAIC marks.
- FR53 [P2/P3]: Print/export other forms (Company Comparison, Portfolio), faithful-but-neutral, each per phase.

**Application Shell & Data Management**
- FR54 [P1]: List, search, sort, filter saved studies; open from home dashboard.
- FR55 [P1]: Delete or archive a study (with confirmation); deletions never corrupt journal time-series.
- FR56 [P1]: Switch a study between entry regime and contemplation regime, active regime indicated.
- FR57 [P1]: View consistent legend for freshness/provenance/coverage/confidence states.
- FR58 [P1]: Every main surface presents actionable empty state + clear neutral error/feedback.
- FR59 [P1]: Export/import a single study to portable versioned file (round-trip preserves identity).
- FR60 [P1]: Export/import the whole journal in versioned format, validated on import.
- FR61 [P1]: Restore from a backup with integrity + version-compatibility checks before overwrite.
- FR62 [P1]: Access non-blocking contextual help/glossary + read-only demonstration study.

**Configuration, Posture & Operation**
- FR63 [P1]: Configure providers/keys, global reference currency, risk thresholds, label set, locale — no blocking setup flow.
- FR64 [P1]: Disclaimer always visible; product never issues recommendations.
- FR65 [P1]: Run full study + portfolio-risk workflow offline; only online action = user-initiated refresh.
- FR66 [P1]: Journal kept in portable local store an external system can back up.

**P1 (MVP) FR count:** FR1–13, 15–25, 29–36, 40, 42–43, 46–47, 49–52, 54–66 → the bulk. **P2:** FR26,27,28,37,38,39,41,44,45,48,53. **V:** FR14.

### Non-Functional Requirements (26 total)

**Correctness & Calculation Integrity**
- NFR-C1: Engine deterministic — identical inputs → identical outputs, bit-stable across runs/platforms.
- NFR-C2: Output matches every golden study: exact on zoning/verdict, ±0.5% on derived numerics.
- NFR-C3: Property invariants hold (zone bounds ordered; U/D ≥ 0; capital-at-risk ≥ 0; FX round-trip < 1e-6).
- NFR-C4: FX only at consolidation; per-currency results independent of reference currency.
- NFR-C5: Engine + risk crate gated in CI by golden + property tests (≥95% path coverage); failing test blocks merge.

**Performance**
- NFR-P1: Judgment-line recalc + zone re-render live within ~100 ms while dragging.
- NFR-P2: Opening/recomputing a full study within ~1 s on typical hardware.
- NFR-P3: Manual portfolio refresh (tens of holdings) within a few seconds; never blocks UI.
- NFR-P4: App interactive within ~3 s of launch.

**Security & Privacy**
- NFR-S1: API keys only in OS secret store — never in repo, config, logs, exports, backups.
- NFR-S2: No telemetry/analytics; only network = user-initiated provider/FX fetches.
- NFR-S3: All persistent data local; nothing to third parties beyond chosen provider under user's key.
- NFR-S4: AI module never exfiltrates journal unless user explicitly enables remote AI.

**Reliability & Data Integrity**
- NFR-R1: Full workflow runs offline; network loss degrades only fetching with stale flagging.
- NFR-R2: Writes crash-safe/atomic — interrupted op never corrupts journal.
- NFR-R3: Schema migrations forward-safe; older journal always opens/migrates, no data loss.
- NFR-R4: Reconciliation never destroys manual value/judgment; provider value preserved alongside.
- NFR-R5: Export/import/restore verify integrity + schema version; mismatched/corrupt file rejected.

**Portability & Compatibility**
- NFR-X1: Identical behavior + numeric results across Windows/macOS/Linux.
- NFR-X2: Locale-aware number parsing/formatting, configurable independently of OS locale.
- NFR-X3: Journal file portable across platforms.

**Usability & Accessibility**
- NFR-U1: Buy/hold/sell zones distinguishable without color alone (color-blind-safe + secondary cue).
- NFR-U2: Primary study + data-entry workflows fully keyboard-operable.
- NFR-U3: On-screen + printed layouts recognizably close to original form, neutral labels.

**Maintainability & Testability**
- NFR-M1: UI is thin layer over UI-independent tested calc crate + versioned data contract decoupled from Slint/storage.
- NFR-M2: Data contract carries explicit schema_version; breaking change ships a migration.

### Additional Requirements / Constraints
- **Constraints (technical):** Rust + Slint (egui contingency for charting); local SQLite; offline-first; no server.
- **Constraints (legal/IP):** GPL-3.0 pending dependency-license audit; no vendor market data (synthetic fixtures); neutral labels, no NAIC marks/verbatim text; "educational not advice" = design intent; per-provider ToS = end-user responsibility.
- **Explicitly Not Applicable:** Scalability (single-user, local).
- **Appendix A** pins formulas/thresholds: capital-at-risk formula, usable-year/low-confidence (<5y), weighted-avg cost basis (fees incl.), dividend net rule (CH 35%), stale threshold (>1 trading day), neutrality banned-verb list. **Deferred to Architecture:** SSG output set (FR4), plausibility rules (FR10), load-bearing input def (FR12), golden tolerance (FR9), exact banned-verb list.

### PRD Completeness Assessment (initial)
- **Strengths:** Requirements are crisp, individually testable, and consistently phase-tagged. Strong traceability scaffolding (Appendix A pinned definitions). Neutral-posture and correctness concerns are first-class. NFRs quantified with concrete thresholds.
- **Watch items for downstream validation:**
  1. Several FR-critical definitions are explicitly **deferred to Architecture** (SSG output set FR4, plausibility rules FR10, load-bearing input FR12, golden tolerance FR9, banned-verb list FR13). Step 6 must verify Architecture actually pins these.
  2. **FR20** is flagged in project memory as having a pending change (tri-state validated flag none/?/✓ with soft lock from UX step 7) that supersedes the PRD wording — must reconcile against UX + epics.
  3. Phase split (P1 vs P2) is the load-bearing scoping decision; epics must respect it (no P2 work pulled into MVP epics, no P1 gap).

## Epic Coverage Validation

**Source:** `epics.md` (stepsCompleted [1,2,3,4]; 8 epics, ~40 stories detailed for P1, outlines for P2/P3/V). The epics doc carries an explicit **FR Coverage Map** plus per-epic and per-story FR tags; each was cross-checked against the actual story acceptance criteria.

### Coverage Matrix (FR → Epic/Story)

| FR | Phase | Epic / Story home | Status |
|----|-------|-------------------|--------|
| FR1 | P1 | E2 / 2.2 | ✓ |
| FR2 | P1 | E2 / 2.2 | ✓ |
| FR3 | P1 | E2 / 2.11 | ✓ |
| FR4 | P1 | E1 / 1.8 | ✓ |
| FR5 | P1 | E1 / 1.8 | ✓ |
| FR6 | P1 | E2 / 2.6 | ✓ |
| FR7 | P1 | E1 / 1.8 | ✓ |
| FR8 | P1 | E1 / 1.8 (compute) + E2 / 2.7 (surface) | ✓ |
| FR9 | P1 | E1 / 1.9 (CI gate) + E2 / 2.13 (verify-engine UI) | ✓ |
| FR10 | P1 | E1 / 1.8 (detect) + E2 / 2.7 (surface) | ✓ |
| FR11 | P1 | E1 (traceability data) + E2 / 2.6 (view) | ✓ |
| FR12 | P1 | E1 / 1.11 (invariant) + E2 / 2.6 | ✓ |
| FR13 | P1 | E2 / 2.14 (banned-verb; cross-cutting) | ✓ |
| FR14 | V | E8 / 8.3, 8.4 | ✓ |
| FR15 | P1 | E3 / 3.1 | ✓ |
| FR16 | P1 | E2 / 2.4 | ✓ |
| FR17 | P1 | E1 / 1.3 (model) + E2 / 2.4 (display) | ✓ |
| FR18 | P1 | E1 / 1.3 (model) + E2 / 2.4 (display) | ✓ |
| FR19 | P1 | E1 / 1.3 (model) + E2 / 2.4 (display) | ✓ |
| FR20 | P1 | E2 / 2.5 (tri-state + soft-lock) | ✓ |
| FR21 | P1 | E3 / 3.3 | ✓ |
| FR22 | P1 | E3 / 3.4 | ✓ |
| FR23 | P1 | E3 / 3.5 | ✓ |
| FR24 | P1 | E3 / 3.5 | ✓ |
| FR25 | P1 | E3 / 3.2 | ✓ |
| FR26 | P2 | E6 / 6.9 | ✓ |
| FR27 | P2 | E6 / 6.9 | ✓ |
| FR28 | P2 | E6 / 6.5 | ✓ |
| FR29 | P1 | E1 / 1.8 (compute) + E3 / 3.3 (refresh) | ✓ |
| FR30 | P1 | E2 / 2.8 | ✓ |
| FR31 | P1 | E2 / 2.6 (value) + 2.8 (drag) | ✓ |
| FR32 | P1 | E2 / 2.9 | ✓ |
| FR33 | P1 | E2 / 2.8 | ✓ |
| FR34 | P1 | E4 / 4.1 | ✓ |
| FR35 | P1 | E4 / 4.2 | ✓ |
| FR36 | P1 | E4 / 4.3 | ✓ |
| FR37 | P2 | E6 / 6.1 | ✓ |
| FR38 | P2 | E6 / 6.2 | ✓ |
| FR39 | P2 | E6 / 6.3 | ✓ |
| FR40 | P1 | E4 / 4.4 | ✓ |
| FR41 | P2 | E6 / 6.4 | ✓ |
| FR42 | P1 | E4 / 4.5 | ✓ |
| FR43 | P1 | E4 / 4.6 | ✓ |
| FR44 | P2 | E6 / 6.6 | ✓ |
| FR45 | P2 | E6 / 6.7 | ✓ |
| FR46 | P1 | E4 / 4.7 | ✓ |
| FR47 | P1 | E4 / 4.7 | ✓ |
| FR48 | P2 | E6 / 6.8 | ✓ |
| FR49 | P1 | E2 / 2.10 | ✓ |
| FR50 | P1 | E5 / 5.1 | ✓ |
| FR51 | P1 | E1 / 1.10 (storage) + E2 / 2.10 (capture) | ✓ |
| FR52 | P1 | E5 / 5.6 | ✓ |
| FR53 | P2/P3 | E7 / 7.1, 7.2, 7.5 | ✓ |
| FR54 | P1 | E2 / 2.2, 2.12 | ✓ |
| FR55 | P1 | E2 / 2.12 | ✓ |
| FR56 | P1 | E2 / 2.3 | ✓ |
| FR57 | P1 | E2 / 2.13 | ✓ |
| FR58 | P1 | E2 / 2.13 | ✓ |
| FR59 | P1 | E5 / 5.2 | ✓ |
| FR60 | P1 | E5 / 5.3 | ✓ |
| FR61 | P1 | E5 / 5.4 | ✓ |
| FR62 | P1 | E2 / 2.13 | ✓ |
| FR63 | P1 | E2 / 2.1 + E3 / 3.2 + E4 / 4.3,4.5 + E5 / 5.5 (incremental) | ✓ |
| FR64 | P1 | E2 / 2.1 | ✓ |
| FR65 | P1 | E1 (foundation) + E2 / 2.2 | ✓ |
| FR66 | P1 | E1 / 1.10 (portable store) + E5 / 5.5 (backup/location) | ✓ |

### Missing Requirements

**None.** All 66 PRD FRs trace to at least one epic and a concrete story (or, for P2/P3/V, a named story outline). No FR appears in the epics that is absent from the PRD.

### Extra coverage beyond PRD FRs (positive — not gaps)
- **15 Architecture-derived requirements** ADD1–ADD15 woven into Epic 1 stories (workspace, Cardinal Rule, spikes, versioned contract, hybrid persistence, journal identity, sync-safety, foundational invariant, verdict versioning, method spec, golden assets, price-history cache, CI gates, observability).
- **29 UX design requirements** UX-DR1–UX-DR29 carried as first-class work items.
- Two items flagged "to file as an FR" are already covered by stories: **DB-location requirement** → Story 5.5; **verdict-versioning** (ADD10) → Epic 1 invariants. *(Per project convention these should be logged as GitHub issues once the repo exists.)*

### Coverage Statistics
- **Total PRD FRs:** 66
- **FRs covered in epics:** 66
- **Coverage percentage:** **100%**
- **P1 (MVP) FRs:** all detailed to story level with acceptance criteria (Epics 1–5).
- **P2/P3/V FRs:** mapped to Epics 6/7/8 as deliberate story outlines (detail deferred by design).

## UX Alignment Assessment

### UX Document Status
**Found.** `ux-design-specification.md` (status: complete, 2026-06-07, all 14 UX steps done) + interactive mockup `ux-stock-study-screen.html`. The spec is comprehensive: executive summary, core UX, emotional design, pattern analysis, design-system foundation, the defining "judgment moment" interaction, visual foundation (Okabe-Ito palette, ink scales, verdict-integrity rule, tri-state markers), design-direction decision, Mermaid journey flows for all 6 v1 journeys, component strategy (15 custom components), consistency patterns, responsive + accessibility.

### UX ↔ PRD Alignment — **Strong**
- All 6 PRD v1 journeys (J1, J2, J2b, J3b, J3/4 slice, J5) are rendered as concrete UX flows.
- UX explicitly references and honours PRD FRs/NFRs: FR54/55/57/58/62 (shell), FR13/64 (neutral posture + disclaimer), FR12 (verdict degraded/withheld), FR8/FR10 (low-confidence + plausibility), FR30-33 (judgment line), NFR-U1 (decision never colour-only), NFR-U2 (keyboard-first), NFR-X1 (cross-OS identical), NFR-P1 (<100 ms recolor).
- **FR20 supersession reconciled:** UX step 7 redefines the validated flag as tri-state (none/?/✓) + soft-lock; both the UX spec and the epics carry this consistently and flag it for a PRD FR20 refinement (matches project memory).
- **No UX requirement contradicts the PRD.** UX-layer detail beyond PRD FRs (design tokens, palette, typography) is appropriate and is carried into the epics as UX-DR1–UX-DR29.

### UX ↔ Architecture Alignment — **One material drift (resolved downstream), otherwise strong**

⚠️ **FINDING (documentation drift, non-blocking): the UX spec specifies the chart engine as `egui`, which the Architecture has since removed.**
- The UX spec (2026-06-07) pervasively describes the §1 growth chart as **egui behind a `ChartView` trait, composited side-by-side in the Slint window** — see its Feasibility Note, Design-System foundation, Component Strategy (component #3 "egui behind a ChartView trait"), Implementation Roadmap (week-1 spike C = "egui-in-Slint same-window compositing"), the "Theming across the FFI" forward-note ("the egui chart does not read Slint global singletons → push tokens Slint→egui"), and the accessibility note ("egui chart exposes a keyboard/exact-value path").
- The Architecture (2026-06-08, the **following day**) **explicitly rejects egui and removes it entirely**: "the egui-in-Slint embedding is the source of the charting friction … rejected; egui is removed entirely from the architecture. Charts are drawn **natively in Slint** (`Path` + `TouchArea`)." This matches the locked project decision (GUI = Slint-only, no web, no egui embedding).
- **Impact on implementation: NONE.** The authoritative downstream artifact — the epics — correctly follows the Architecture, not the stale UX text: Story 1.5 (spike B) = "native-Slint draggable judgment line"; Story 2.8 = chart "rendered natively in Slint … if spike B was NO-GO, the agreed Slint fallback … never egui/web." The week-1 spike set in the epics (A grid / B native chart / C decimal CAGR) matches the Architecture, not the UX spec's egui-centric spike C.
- **Consequences to record (so no one re-introduces egui from the UX doc):**
  1. The UX "Theming across the FFI / push tokens Slint→egui" forward-note is **obsolete** — Architecture uses a single intra-binary token source of truth (`arc_swap`), no FFI boundary. (Already reflected in UX-DR7 / epics: "no FFI".)
  2. UX week-1 spike C ("egui-in-Slint compositing") is **superseded** by the decimal-CAGR/determinism spike; the egui-compositing risk class no longer exists.
  3. Component #3 ("egui ChartView") must read as "native Slint `Path`/`TouchArea`" (as UX-DR10 in the epics already states).
- **Recommended action:** update the UX spec's charting/egui passages (or add a dated erratum pointing to the Architecture decision), and file a tracking GitHub issue per project convention. Does **not** block implementation.

**Otherwise UX↔Architecture is well-aligned:** tokens as single source of truth (UX → Arch intra-binary `arc_swap`); tri-state review + soft-lock (UX → Arch `review: none|to_review|validated` enum); verdict-integrity rule (UX → Arch `FullVerdict` constructible only from validated+fresh inputs, type-enforced); marker-confusability ≥98%/<2% (UX → Arch CI snapshot gate); keyboard-first + exact-value judgment path; French-first `@tr()` vs runtime NAIC↔neutral label set (two axes, both docs); sticky verdict bar, faithful collapsible form, grayscale-print profile; sync-path detection + DB-location handling; no-wizard Settings. Architecture's `report` crate covers FR52 PDF fidelity (NFR-U3).

### Warnings
1. **(Medium) UX spec is stale on the chart engine (egui vs native Slint).** Resolved in Architecture + epics; update the UX doc / file an issue to prevent regression. Non-blocking.
2. **(Low) `<100 ms` native-Slint draggable recolor is targeted but unproven** — the single shared technical unknown across PRD/UX/Architecture/epics. Correctly de-risked by the week-1 spike B (go/no-go) with a defined Slint fallback (dedicated canvas / plotters→SharedPixelBuffer + TouchArea). Tracked in Epic 1 Story 1.5. Non-blocking but must run before UI commitment.

### Alignment Issues
**None blocking.** The one material divergence (egui) is already correctly resolved in the implementation-authoritative artifacts (Architecture + epics); only the UX source document lags.

## Epic Quality Review

Reviewed all 8 epics and the ~40 detailed P1 stories against create-epics-and-stories standards: user value, epic independence, forward dependencies, story sizing, acceptance-criteria quality, DB-creation timing, starter-template requirement, greenfield indicators.

### Best-Practices Compliance Checklist (per epic)

| Epic | User value | Independent (no fwd dep) | Stories sized | No fwd refs | DB JIT | Clear ACs | FR traceable |
|------|-----------|--------------------------|---------------|-------------|--------|-----------|--------------|
| E1 Core & foundation | ⚠ headless* | ✓ standalone | ✓ | ✓ | ⚠ schema upfront* | ✓ BDD | ✓ |
| E2 Trustworthy Study | ✓ | ✓ (E1 only; manual-first, no provider needed) | ✓ | ✓ | ✓ | ✓ BDD | ✓ |
| E3 Provider & reconciliation | ✓ | ✓ (E1; branches onto E1 rail) | ✓ | ✓ | n/a | ✓ BDD | ✓ |
| E4 Watchlist & single-portfolio risk | ✓ | ✓ (E1 math, E3 prices; E3<E4) | ✓ | ✓ | ✓ (uses E1 schema) | ✓ BDD | ✓ |
| E5 Cumulative memory & portability | ✓ | ✓ (E1/E3) | ✓ | ✓ | ✓ | ✓ BDD | ✓ |
| E6 [P2] Multi-portfolio/FX/risk | ✓ | ✓ | outline only* | ✓ | — | deferred* | ✓ |
| E7 [P3] Comparison/health/screening | ✓ | ✓ | outline only* | ✓ | — | deferred* | ✓ |
| E8 [V] Read-only AI & MCP | ✓ | ✓ | outline only* | ✓ | — | deferred* | ✓ |

`*` = see findings below.

### 🔴 Critical Violations
**None.** No technical epic masquerading as user value without justification; no forward dependency (no Epic N requiring Epic N+1); no circular epic dependencies; no epic-sized story that cannot be completed. All inter-epic dependencies point **backward** (E2→E1, E3→E1, E4→E1+E3, E5→E1+E3), and the doc states the required sequence (E3 before E4).

### 🟠 Major Issues
1. **Epic 1 is a foundation/technical epic with no direct end-user-facing feature ("Closes headless").** Strictly, this violates "epics deliver user value, not technical milestones."
   - **Assessment: acceptable, deliberate, and well-justified.** The epics doc gives an explicit structure rationale: the project's #1 risk (a *silent, plausible, wrong* buy/sell signal) is attacked first by placing the deterministic engine + pure normalization layer + full golden/property/metamorphic CI harness in Epic 1 (the PRD's top-risk + NFR-C1..C5 + the architecture's Foundational Invariant all demand this). For a solo dev with a correctness-critical core, a thin headless foundation that "closes" (the math is proven, with a runnable verify-engine self-check) is the correct cut. It does carry a user-perceivable trust artifact (FR9 golden self-check) even if no GUI.
   - **Recommendation:** keep as-is; no remediation needed. Flagged only so the deviation is conscious. The *first usable product* lands in Epic 2 (not deferred behind a long technical runway), which preserves the spirit of value-first slicing.

### 🟡 Minor Concerns
1. **Full normalized schema created upfront in Epic 1 / Story 1.10** (portfolios, holdings, **transactions**, **fx_rates**, watchlist_items) — including tables whose features are P2 (transactions, fx_rates per E6). This deviates from the "create tables only when first needed (JIT)" guideline.
   - *Justification:* the architecture deliberately ships a **versioned hybrid schema + migrations harness + frozen corpus + schema-drift detector** in the foundation so later epics *fill* the contract rather than *migrate* it, and the provenance model is complete from v1. The over-provisioning is bounded and intentional. **Recommendation:** optionally defer the P2-only tables (transactions, fx_rates) to Epic 6's first story to keep v1 schema lean — but acceptable to keep if the migration story is genuinely cheaper this way. Low priority.
2. **Epics 6/7/8 (P2/P3/V) are story *outlines* without acceptance criteria.** By the AC-quality rule this is an omission.
   - *Justification:* explicitly deferred by design ("Story-level detail deferred to Phase 2/3/Vision; requirements refined then"). Over-specifying far-future, scope-fluid work is itself an anti-pattern. **Recommendation:** acceptable for a readiness gate that targets the **P1 MVP**; run create-epics-and-stories again on Epic 6 at the start of Phase 2. No action needed for v1 implementation.
3. **Story 1.1 doesn't explicitly state the Slint-template seed step.** The starter-template requirement (Architecture: custom Cargo workspace + UI crate seeded via `cargo generate … slint-rust-template`) is captured in ADD1 and the epic's "Includes" text, but Story 1.1's ACs describe workspace scaffolding without naming the template-seed action. **Recommendation:** add one AC line to Story 1.1 ("the `app` UI crate is seeded from the official Slint Rust template's build.rs/.slint wiring"). Trivial.
4. **Epic 1 (11 stories) and Epic 2 (14 stories) are large.** Individually well-sized; the count is high but coherent (foundation; first usable product). Optional: Epic 2 could split the app-shell/dashboard stories from the study-screen stories. Cosmetic; no action required.

### Story-Quality Positives (worth recording)
- **Acceptance criteria are genuine BDD** (Given/When/Then), testable, and include **error/edge paths**: provider failure with cause classification (3.5), low-confidence + plausibility surfacing (2.7), non-destructive reconciliation with divergence→? (3.4), restore version-mismatch ("you saw v57, this is v41", 5.4).
- **Starter-template / greenfield indicators present:** Story 1.1 scaffolds workspace + pins deps + 3-OS CI from day one (fmt/clippy/test/deny + determinism hash) — correct for a fresh greenfield restart.
- **Risk-first sequencing** with throwaway spikes correctly framed as go/no-go (1.4/1.5/1.6), the signature chart placed in the first usable product (E2) rather than a deferred "charts" epic.
- **Cross-cutting FRs** (FR13 neutrality, FR63 no-wizard settings) handled incrementally with a named primary home — appropriate, not smeared.

### Overall Epic Quality Verdict
**High quality, implementation-ready for P1.** No critical or blocking structural defects. The single notable deviation (headless Epic 1) is a conscious, well-argued risk-first choice rather than an oversight. Minor concerns are optional refinements, none gating.

## Summary and Recommendations

### Overall Readiness Status

## ✅ READY — proceed to implementation (Phase 1 / MVP)

The PRD, UX specification, Architecture decision document, and Epics & Stories are **complete, mutually consistent, and fully traceable**. FR coverage is **100% (66/66)**. There are **no critical or blocking defects**. The few findings are documentation-hygiene and optional-refinement items that do not gate the start of implementation.

### Scorecard

| Dimension | Result |
|-----------|--------|
| Document discovery | ✅ All 4 artifacts present, current, no duplicate-format conflicts |
| PRD extraction | ✅ 66 FRs + 26 NFRs, phase-tagged, with pinned Appendix-A definitions |
| Epic FR coverage | ✅ 100% (66/66) traced to epic + story |
| UX ↔ PRD alignment | ✅ Strong; all 6 journeys + FR/NFR honoured |
| UX ↔ Architecture alignment | ⚠️ One stale point (egui), resolved downstream — non-blocking |
| Epic quality / structure | ✅ High; no forward/circular deps; genuine BDD ACs |
| Architecture readiness | ✅ Author's own validation = READY; 6-crate workspace, boundaries, FR map |

### Critical Issues Requiring Immediate Action
**None.** Nothing must be fixed before scaffolding can begin.

### Issues to Track (none blocking) — file as GitHub issues per project convention

1. **(Medium) Update the UX spec for the egui→native-Slint chart decision.** The UX spec (2026-06-07) still describes the chart as `egui` behind a `ChartView` trait; the Architecture (2026-06-08) removed egui entirely (native Slint `Path`/`TouchArea`). The epics already follow the Architecture, so implementation is unaffected — but edit the UX doc (or add a dated erratum) so no one re-introduces egui from it. Obsolete UX passages: the "Theming across the FFI / push tokens Slint→egui" forward-note, week-1 spike C ("egui-in-Slint compositing"), component #3 ("egui ChartView"), the accessibility "egui chart" note.

2. **(Pending — from project memory & both docs) File the deferred requirement changes as FRs/issues once the repo exists:**
   - **FR20 refinement:** validated flag is now tri-state (none/?/✓) with soft-lock (supersedes the original auto-reset wording). Consistent across UX + epics already.
   - **DB-location requirement:** user-selectable journal directory + reopen last-used on launch (architecture ADD7; epics Story 5.5).
   - **Verdict-versioning requirement:** decision-time verdict frozen/immutable; recompute-with-today's-method on demand only (architecture ADD10).

3. **(Low) `<100 ms` native-Slint draggable-recolor is unproven** — the single shared technical unknown. Must be settled by **Epic 1 Story 1.5 (Week-1 spike B, go/no-go)** before committing UI work; Slint fallback defined (dedicated canvas / `plotters`→`SharedPixelBuffer` + `TouchArea`). No egui/web fallback.

4. **(Low) Author the Appendix-A method specification before implementing `core`.** The exact SSG output set, plausibility rules, banned-verb list, golden tolerance, and "load-bearing input" definition were deferred from the PRD "to Architecture." They are correctly scheduled as **Epic 1 Story 1.2** (versioned method spec pinned by `method_version`) — just ensure it precedes Story 1.8 (engine).

5. **(Low / optional) Lean v1 schema:** consider deferring P2-only tables (`transactions`, `fx_rates`) out of Epic 1 Story 1.10 into Epic 6, and add an explicit Slint-template-seed AC to Story 1.1. Cosmetic.

### Recommended Next Steps

1. **Initialize the Git repository** (`guycorbaz/steadyinvest`, private, GPL-3.0) and immediately file issues #1–#3 above (egui/UX erratum, FR20/DB-location/verdict-versioning FRs, the spike + method-spec reminders). This unblocks the project's "GitHub Issues = single source of truth" convention.
2. **Start Epic 1, Story 1.1** — scaffold the Cargo workspace (6 crates) + 3-OS CI gate skeleton (the architecture's first implementation priority).
3. **Run the Week-1 spikes (Stories 1.4/1.5/1.6)** — especially **spike B (native-Slint drag + <100 ms recolor)** — as the go/no-go before any further UI investment.
4. **Author the Appendix-A method spec (Story 1.2)** before the `core` engine (Story 1.8), test-first.
5. Optionally, run `bmad-create-epics-and-stories` on **Epic 6** at the start of Phase 2 to expand its outlines into full stories.

### Final Note

This assessment reviewed **4 artifacts** across **6 validation dimensions** and identified **0 critical issues**, **1 medium documentation-drift issue** (egui, already resolved downstream), and **a handful of low/optional refinements**. FR coverage is complete (66/66) and the planning set is internally coherent. **The project is READY to begin Phase 1 implementation.** The tracked items can be addressed in parallel with scaffolding — none blocks the first stories.

---

**Assessment date:** 2026-06-09
**Assessor:** Implementation Readiness workflow (BMad) — facilitated for Guy
**Artifacts assessed:** `prd.md` (2026-06-06) · `ux-design-specification.md` (2026-06-07) · `architecture.md` (2026-06-08) · `epics.md` (2026-06-09)

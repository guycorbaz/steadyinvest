---
validationTarget: '_bmad-output/planning-artifacts/prd.md'
validationBranch: 'docs/g2-prd-epic8'
validationDate: '2026-09-27'
validationFocus: 'G2 — Epic 8 re-scoped as Phase 4 "AI assistance" (FR69–FR78, NFR-A1–A4, revised FR13/FR14/FR33/FR64/NFR-S3/NFR-S4, Journey 6)'
inputDocuments:
  - _bmad-output/planning-artifacts/prd.md
  - _bmad-output/planning-artifacts/product-brief-steadyinvest.md
  - _bmad-output/planning-artifacts/product-brief-steadyinvest-distillate.md (listed in PRD frontmatter; present, not re-read — the brief itself was used)
  - _bmad-output/planning-artifacts/research/domain-naic-better-investing-research-2026-06-05.md (listed; present)
  - docs/NAIC/*.pdf and docs/NAIC/forms/*.pdf (listed; present; methodology sources, not re-read)
validationStepsCompleted: ['step-v-01-discovery', 'step-v-02-format-detection', 'step-v-03-density-validation', 'step-v-04-brief-coverage-validation', 'step-v-05-measurability-validation', 'step-v-06-traceability-validation', 'step-v-07-implementation-leakage-validation', 'step-v-08-domain-compliance-validation', 'step-v-09-project-type-validation', 'step-v-10-smart-validation', 'step-v-11-holistic-quality-validation', 'step-v-12-completeness-validation']
validationStatus: COMPLETE
holisticQualityRating: '4/5 - Good'
overallStatus: 'Warning'
---

# PRD Validation Report

**PRD Being Validated:** `_bmad-output/planning-artifacts/prd.md` (branch `docs/g2-prd-epic8`, commit e49e558)
**Validation Date:** 2026-09-27
**Mode:** non-interactive (menus resolved to the default that continues; no additional reference documents; no project-context.md found)

Line numbers (`L…`) refer to the PRD as validated (1054 lines).

## Input Documents

- PRD: `prd.md` ✓
- Product Brief: `product-brief-steadyinvest.md` ✓ (distillate listed, present)
- Research: `research/domain-naic-better-investing-research-2026-06-05.md` ✓ (present)
- NAIC methodology PDFs and forms (10 files) ✓ (present; not re-read — no methodology change in G2)
- Additional references: none

## Validation Findings

## Format Detection

**PRD Structure (## headers):**
1. Reference Documents
2. Executive Summary
3. Project Classification
4. Success Criteria
5. Product Scope
6. User Journeys
7. Domain-Specific Requirements
8. Innovation & Novel Patterns
9. Desktop Application — Specific Requirements
10. Project Scoping & Phased Development
11. Functional Requirements
12. Non-Functional Requirements
13. Appendix A — Definitions (pinned values referenced by FRs)

Frontmatter: `classification.domain = fintech`, `classification.projectType = desktop_app`, complexity high.

**BMAD Core Sections Present:**
- Executive Summary: Present
- Success Criteria: Present
- Product Scope: Present
- User Journeys: Present
- Functional Requirements: Present
- Non-Functional Requirements: Present

**Format Classification:** BMAD Standard
**Core Sections Present:** 6/6

## Information Density Validation

**Anti-Pattern Violations:**

**Conversational Filler:** 0 occurrences
**Wordy Phrases:** 0 occurrences
**Redundant Phrases:** 0 occurrences

**Total Violations:** 0

**Severity Assessment:** Pass

**Recommendation:** PRD demonstrates good information density with minimal violations.

*Note (info):* the Executive Summary carries change-history narrative ("This **revises the earlier stance** …", L164–165). It is informative for readers of this revision but belongs in `editHistory`/frontmatter once G2 is settled; the paragraph at L157–165 also duplicates Product Scope → Phase 4 (L292–299).

## Product Brief Coverage

**Product Brief:** product-brief-steadyinvest.md (2026-06-05)

### Coverage Map

**Vision Statement:** Fully Covered (L97–113; reframed as a "system of investment discipline").

**Target Users:** Fully Covered (L123–129). Secondary users deferred until IP review — intentional.

**Problem Statement:** Fully Covered (L115–121).

**Key Features:** Fully Covered / Intentionally Excluded
- Stock Study, interactive layer, watchlist + buy-zone alerts, manual entry: Fully covered (P1).
- Multi-currency/FX "confirmed in v1" in the brief (brief L72, L107) → moved to P2 (L178, FR38/FR44): Intentionally Excluded from v1 (scoping decision, frontmatter L60).
- Company Comparison, Portfolio Health Review, Quick Screen → P3/roadmap: Intentionally Excluded.
- Price refresh "on a configurable cadence (at least daily EOD)" (brief L62) → PRD has manual refresh only (FR21/FR40): Intentionally Excluded (informational).

**Goals/Objectives:** Fully Covered (L207–269).

**Differentiators:** Fully Covered, extended (CH/EU adaptation, cumulative memory, risk overlay, human-gated AI).

### Coverage Summary

**Overall Coverage:** Good — all brief content covered or explicitly re-scoped.
**Critical Gaps:** 0
**Moderate Gaps:** 0
**Informational Gaps:** 2
- The brief contains no AI at all; the whole of G2 (Phase 4 AI assistance) is PRD-level evolution beyond the brief. Acceptable (the brief predates the discovery decisions), but the brief's "no brokerage/advice functionality keeps the tool outside regulated territory" (brief L92) no longer describes the full posture now that third-party AI proposals are hosted — the PRD handles it (L488–492); the brief is simply stale.
- Brief "publishable on GitHub / open source" vs PRD "private repo" — intentional, documented.

**Recommendation:** PRD provides good coverage of Product Brief content.

## Measurability Validation

### Functional Requirements

**Total FRs Analyzed:** 78 (FR1–FR78)

**Format Violations:** 1
- FR75 (L914–915) is a scope note ("the dossier does not store them in Phase 4"), not an actor-capability requirement. The same content already sits in Appendix A (L1049–1050) and Product Scope (L298–299). **[G2]**

**Subjective Adjectives Found:** 2
- FR56 (L854) "clearly indicated"
- FR58 (L857) "clear neutral error/feedback messages"

**Vague Quantifiers Found:** 1
- FR45 (L826) "warns **near** a configured majority share" — the distance defining "near" is not pinned in Appendix A.
- (FR37/FR38 "multiple" = more than one; acceptable.)

**Implementation Leakage:** 1
- FR67 (L879–885): `directories` crate, Synology/Dropbox/OneDrive/iCloud, SQLite `journal_mode=DELETE/TRUNCATE` (pre-G2).

**FR Violations Total:** 5 (1 introduced by G2)

**G2 FRs FR69–FR78 — measurability:** testable overall. FR70, FR71, FR73, FR74, FR76, FR78 are crisp and binary-testable; FR72 is backed by the metamorphic test in Success Criteria (L262–263); FR77 is testable (record exists, fields present). Residual gaps (see Traceability/SMART): FR69 scope of "data cells" vs computed outputs; FR72 "stale" state has no owner-side behaviour defined; the Success Criterion "≤ 2 actions" (L265) is not carried by FR73/FR74.

### Non-Functional Requirements

**Total NFRs Analyzed:** 29 (C1–C5, P1–P4, S1–S4, R1–R5, X1–X3, U1–U3, M1–M2, A1–A4)

**Missing Metrics:** 4
- NFR-P2 (L949) "on typical hardware" — reference hardware undefined.
- NFR-P3 (L950–951) "tens of holdings", "within a few seconds".
- NFR-U3 (L991–992) "recognizably close to the original form".
- NFR-M1 (L998–999) "thin layer".

**Incomplete Template (no measurement method):** 2
- NFR-P1 (L947–948) "~100 ms perceived" and NFR-P4 (L952) "~3 s": metric present, no measurement method/conditions.

**Missing Context:** 0

**G2 NFRs:** NFR-A1–A4 (L1005–1011) each name a criterion and a measurement method (CI suite / whole-surface test / tested / 100% of drafts). NFR-A3 "(tested)" is the thinnest method but adequate. Revised NFR-S3/S4 are testable. No violation counted; enumeration inconsistencies are reported under Traceability.

**NFR Violations Total:** 6 (0 introduced by G2)

### Overall Assessment

**Total Requirements:** 107 (78 FRs + 29 NFRs)
**Total Violations:** 11

**Severity:** Critical by the step's mechanical threshold (> 10). **Judged severity: Warning** — 10 of 11 are soft wording issues that pre-date G2; only FR75 is new, and it is a placement issue, not a testability hole.

**Recommendation:** Some requirements need refinement for measurability. Focus on the violating requirements above; none blocks Epic 8 planning.

## Traceability Validation

### Chain Validation

**Executive Summary → Success Criteria:** Intact. The G2 paragraph (L157–165) is matched by "AI Assistance Outcomes" (L256–269).

**Success Criteria → User Journeys:** Gaps Identified
- User Success L217 ("replay … judgment lines, inputs, per-cell provenance, **and notes**") and Journey 5 L409 ("his decision rationale **and notes**") are P1, but the only capability to create notes is FR78, tagged **[P4]** (L921). P1 FR49/FR51 (L835–840) now cite FR78 by reference. Before G2 the gap was implicit; G2 made it explicit. **[G2-surfaced]**
- L265 "validates or rejects a draft in ≤ 2 actions" has no carrying FR (FR73/FR74 omit it).

**User Journeys → Functional Requirements:** Intact, one ambiguity
- Journey 6 (L417–434) → FR69 (MCP read), FR70/FR71/FR72 (drafts), FR73 (inbox + reminder + origin + side-by-side), FR74 (one-by-one, "?" tag, AI origin), FR33/FR72 (AI-annotated inert lines, "placed by AI"), FR76/FR15/FR21 (owner-only fetch), FR77 (record), FR78 (notes), FR75 (objectives), NFR-A2 (no holdings/watchlist). All "Reveals" items are covered.
- Ambiguity: the journey's search objective "upside/downside above 3:1" (L420) requires the AI to read **computed outputs** (U/D ratio, zones, projected return, quality flags, verdicts). FR69 (L896–898) and the MCP-exposed scope (L1051–1052) list "data cells, provenance, judgments, rationale, notes, judgment history" only; computed outputs and verdicts are neither included nor excluded. **[G2]**

**Scope → FR Alignment:** Misaligned (2)
- FR52 **[P1]** print/export a Stock Study (L843) vs Product Scope Growth "PDF/print export of a study" (L290) and Phase 3 list (L705). Pre-G2.
- FR49/FR51 [P1] depend on FR78 [P4] (above). **[G2-surfaced]**

### Orphan Elements

**Orphan Functional Requirements:** 0
**Unsupported Success Criteria:** 1 (L265 "≤ 2 actions" — no FR)
**User Journeys Without FRs:** 0

### Consistency of the G2 enumerations (cross-section)

The set of things the AI may never see is stated five times, with five different lists:

| Location | Portfolio (holdings/tx/dividends) | Watchlist | Keys | Config |
|---|---|---|---|---|
| Success Criteria L259–260 | ✓ | ✓ | ✓ ("secrets") | — |
| FR69 L897–898 | ✓ | ✓ | ✓ | ✓ |
| NFR-S4 L963–964 | ✓ | **—** | ✓ | ✓ ("configuration secrets") |
| NFR-A2 L1007–1008 | ✓ | ✓ | **—** | **—** |
| Appendix A L1051–1052 | ✓ | ✓ | ✓ | ✓ |

Likewise the AI-readable set omits the draft record, which FR77 (L919–920) says "the AI can read". **[G2]**

### Traceability Matrix (summary)

| Source | FRs |
|---|---|
| J1 new study | FR1–FR7, FR15, FR30–FR32 |
| J2 partial coverage | FR8, FR16–FR20, FR22 |
| J2b annual update | FR3, FR20–FR22, FR29 |
| J3 portfolio risk | FR36–FR45, FR26–FR28 |
| J3b provider failure | FR23, FR24 |
| J4 sell/replace | FR46–FR48 |
| J5 memory | FR49–FR51, FR68 |
| J6 AI scout | FR69–FR78, FR13, FR14, FR17, FR20, FR22, FR33, FR64, FR65 |
| Business objectives (trust, operation, IP) | FR9–FR12, FR25, FR52–FR63, FR66, FR67 |

**Total Traceability Issues:** 5 (notes phase gap; computed-output ambiguity; ≤2-actions criterion; FR52 phase; enumeration drift)

**Severity:** Warning

**Recommendation:** Traceability gaps identified — strengthen chains to ensure all requirements are justified. Every G2 FR traces to Journey 6.

## Implementation Leakage Validation

### Leakage by Category

**Frontend Frameworks:** 1 — NFR-M1 (L999) "decoupled from **Slint**" (pre-G2; Slint is already a declared constraint, L1015).
**Backend Frameworks:** 0
**Databases:** 1 — FR67 (L883–884) SQLite `journal_mode=DELETE/TRUNCATE` (pre-G2).
**Cloud Platforms:** 0 — FR67 names Synology/Dropbox/OneDrive/iCloud as sync-folder examples to detect; counted with FR67.
**Infrastructure:** 0
**Libraries:** FR67 `directories` (counted with FR67).
**Other Implementation Details:** 0 counted — see MCP / Claude Code judgment below.

### G2 terms — judgment

- **MCP** appears in FR14, FR15, FR21, FR67, FR68, FR69, FR72, FR76 and NFR-S1/S2/S4/R2/A1–A3. **Acceptable**: it is the owner-mandated external interface protocol (decision recorded in frontmatter L67 and Constraints L1016), i.e. capability-relevant in the sense of "API consumers can access…". Recommendation (info): state it once as a constraint ("AI integration protocol: MCP, owner-mandated") and let FRs say "the AI interface"; not required.
- **Claude Code** appears only in the Executive Summary (L158), Product Scope → Phase 4 (L297) and frontmatter (L67) — never in an FR/NFR. **Acceptable** as context describing the current client; FRs correctly say "an AI client".
- "CI suite" in NFR-A1 / Success Criteria is a measurement method, not leakage.

### Summary

**Total Implementation Leakage Violations:** 2 (FR67, NFR-M1 — both pre-G2)

**Severity:** Warning

**Recommendation:** Some implementation leakage detected. Move FR67's mechanism (crate name, journal_mode, vendor list) to Architecture ADD7/ADD8 and keep the capability (choose directory, reopen last, single-instance guard, sync-folder warning). G2 introduced no leakage.

**Note:** MCP is acceptable here as a capability-relevant, owner-mandated interface.

## Domain Compliance Validation

**Domain:** fintech
**Complexity:** High (regulated domain; product non-regulated by design)

### Required Special Sections

**Compliance matrix / regulatory posture:** Present, adequate (L478–492). Standard fintech controls (KYC/AML, PCI-DSS, custody) argued out of scope by construction. G2 adds the AI-proposal posture (L483–487, L490–491).
**Security architecture:** Present, adequate (L525–533; NFR-S1–S4; NFR-A1–A4).
**Audit requirements:** Present, adequate (L535–540), extended for AI draft origin/outcome (L539–540, FR77).
**Fraud prevention:** Not applicable by design (no money movement, no custody, L480–482); not labelled "fraud" explicitly — informational.

### Compliance Matrix

| Requirement | Status | Notes |
|---|---|---|
| Regional advice regulation (FINMA/LSFin, MiFID II, Advisers Act) | Met | Design intent, not legal opinion (L488–492) |
| Hosting third-party AI proposals | Partial | "assumed to fall outside advice regulation" (L490–491) is an assumption; flagged to revisit before public release — adequate for private single-user use |
| Data protection / privacy | Met | L525–531; NFR-S3; remote-model exposure accepted by owner (L512–513) |
| Provider ToS for AI exposure | Met | L512–513 |
| Security standards (secrets) | Met | NFR-S1 incl. MCP responses |
| Audit trail | Met | L535–540, FR51, FR77 |
| Fraud prevention | N/A | no funds movement |
| KYC/AML, PCI-DSS | N/A | by construction (L480–482) |

### Summary

**Required Sections Present:** 4/4 (fraud prevention as documented N/A)
**Compliance Gaps:** 1 partial (AI-proposal regulatory assumption — acceptable for current posture)

**Severity:** Pass

**Recommendation:** All required domain compliance sections are present and adequately documented.

## Project-Type Compliance Validation

**Project Type:** desktop_app

### Required Sections

**platform_support:** Present (L617–623)
**system_integration:** Present (L625–636), now including the P4 MCP endpoint
**update_strategy:** Present (L638–644)
**offline_capabilities:** Present (L646–653), including "no AI network call" (L653)

### Excluded Sections (Should Not Be Present)

**web_seo:** Absent ✓
**mobile_features:** Absent ✓

Innovation signal "Desktop AI" is now explicitly pursued (L560–565); "System automation" explicitly not (L613–615).

### Compliance Summary

**Required Sections:** 4/4 present
**Excluded Sections Present:** 0
**Compliance Score:** 100%

**Severity:** Pass

**Recommendation:** All required sections for desktop_app are present. No excluded sections found.

## SMART Requirements Validation

**Total Functional Requirements:** 78

### Scoring Summary

**All scores ≥ 3:** 98.7% (77/78)
**All scores ≥ 4:** 79.5% (62/78)
**Overall Average Score:** 4.67/5.0

### Scoring Table

| FR # | Specific | Measurable | Attainable | Relevant | Traceable | Average | Flag |
|------|----------|------------|------------|----------|-----------|--------|------|
| FR1 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR2 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR3 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR4 | 4 | 3 | 5 | 5 | 5 | 4.4 |  |
| FR5 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR6 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR7 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR8 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR9 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR10 | 4 | 3 | 4 | 5 | 5 | 4.2 |  |
| FR11 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR12 | 4 | 3 | 4 | 5 | 5 | 4.2 |  |
| FR13 | 5 | 5 | 5 | 5 | 5 | 5.0 |  |
| FR14 | 5 | 5 | 5 | 5 | 5 | 5.0 |  |
| FR15 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR16 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR17 | 3 | 4 | 5 | 5 | 5 | 4.4 |  |
| FR18 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR19 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR20 | 5 | 5 | 5 | 5 | 5 | 5.0 |  |
| FR21 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR22 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR23 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR24 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR25 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR26 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR27 | 4 | 3 | 4 | 4 | 4 | 3.8 |  |
| FR28 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR29 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR30 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR31 | 5 | 5 | 4 | 5 | 5 | 4.8 |  |
| FR32 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR33 | 5 | 5 | 4 | 5 | 5 | 4.8 |  |
| FR34 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR35 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR36 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR37 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR38 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR39 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR40 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR41 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR42 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR43 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR44 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR45 | 4 | 3 | 5 | 5 | 5 | 4.4 |  |
| FR46 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR47 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR48 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR49 | 4 | 4 | 5 | 5 | 3 | 4.2 |  |
| FR50 | 4 | 3 | 4 | 5 | 5 | 4.2 |  |
| FR51 | 4 | 4 | 5 | 5 | 3 | 4.2 |  |
| FR52 | 5 | 4 | 5 | 4 | 3 | 4.2 |  |
| FR53 | 4 | 4 | 5 | 4 | 4 | 4.2 |  |
| FR54 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR55 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR56 | 4 | 3 | 5 | 4 | 4 | 4.0 |  |
| FR57 | 4 | 3 | 5 | 4 | 4 | 4.0 |  |
| FR58 | 3 | 2 | 5 | 4 | 4 | 3.6 | X |
| FR59 | 5 | 5 | 5 | 4 | 4 | 4.6 |  |
| FR60 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR61 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR62 | 4 | 3 | 5 | 4 | 4 | 4.0 |  |
| FR63 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR64 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR65 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR66 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR67 | 4 | 5 | 4 | 5 | 4 | 4.4 |  |
| FR68 | 5 | 5 | 4 | 5 | 5 | 4.8 |  |
| FR69 | 3 | 4 | 5 | 5 | 5 | 4.4 |  |
| FR70 | 5 | 5 | 5 | 5 | 5 | 5.0 |  |
| FR71 | 5 | 5 | 5 | 5 | 5 | 5.0 |  |
| FR72 | 4 | 5 | 4 | 5 | 5 | 4.6 |  |
| FR73 | 5 | 5 | 5 | 5 | 5 | 5.0 |  |
| FR74 | 5 | 5 | 5 | 5 | 5 | 5.0 |  |
| FR75 | 3 | 3 | 5 | 4 | 5 | 4.0 |  |
| FR76 | 5 | 5 | 5 | 5 | 5 | 5.0 |  |
| FR77 | 5 | 4 | 5 | 5 | 5 | 4.8 |  |
| FR78 | 5 | 5 | 5 | 5 | 4 | 4.8 |  |

**Legend:** 1=Poor, 3=Acceptable, 5=Excellent
**Flag:** X = Score < 3 in one or more categories

### Improvement Suggestions

**Low-Scoring FRs:**

**FR58:** replace "clear neutral error/feedback messages" with a testable rule (e.g. every error names its cause category and one available next action; passes the FR13 banned-verb gate).

**Borderline (score 3) worth tightening, G2-related:**
- **FR17:** define whether a *validated* AI value's source is `ai-suggested` or `manual` + an AI-origin attribute. FR17 (L764–765) says source `ai-suggested`; FR74 (L911) says "applied … as an owner entry"; FR22 (L776–777) says it "reconciles as a manual value".
- **FR69:** state whether computed outputs (zones, U/D, projected return, quality flags, frozen verdicts) are readable; state that the transaction rationale (FR49) is excluded ("rationale" is ambiguous) and add the draft record (FR77).
- **FR72:** define what the owner can do with a **stale** draft (validate against the new current value, only reject, or auto-expire).
- **FR75:** move to Product Scope / Appendix A (already there) or rephrase as a negative requirement ("the dossier stores no search objectives").
- **FR49/FR51:** resolve the P1 → P4 dependency on FR78 (either give study notes a P1 FR, or drop "notes" from the P1 wording).

### Overall Assessment

**Severity:** Pass (1.3% flagged)

**Recommendation:** Functional Requirements demonstrate good SMART quality overall.

## Holistic Quality Assessment

### Document Flow & Coherence

**Assessment:** Good

**Strengths:**
- G2 is propagated deliberately across almost every section (Exec Summary, Classification, Success Criteria, Scope, Journey 6, Domain, Innovation, Desktop, Scoping, FRs, NFRs, Appendix A), with the old AI stance explicitly marked superseded in the frontmatter.
- The human gate is expressed as testable constructs (draft-only writes, whole-surface non-exposure test, metamorphic "pending changes nothing" test), not as intentions.
- The "app outputs neutral / AI output framed and labelled" split (FR13, FR64, Appendix A L1042–1045) resolves the neutrality conflict cleanly.

**Areas for Improvement (G2 consistency — leftover contradictions):**
- Frontmatter L47 (UX judgment-lines decision) still says judgment lines are "NEVER a 'suggested line' (suggesting = recommending = breaks neutral posture)" and is **not** marked superseded, while FR33 [P4] (L798–801) and Journey 6 put AI-proposed lines on the chart. The G2 entry (L67) supersedes only the "AI interaction", "AI runs 100% LOCALLY" and "API posture" entries.
- Persona L311: "tools and AI may propose" — the app ("tools") never proposes (FR13, FR33); only the AI does.
- Security & privacy L528–529: "no cloud … the only sensitive material is the user's own keys and journal" — still true for the app, but with a remote model accepted (L159, L530), study data may reach a cloud model; the "no cloud" bullet reads as unconditional.
- Constraints L1016–1017: "no network server — a local MCP server … is the only external interface" — the outbound provider API is also an external interface; "no network server" vs "MCP server" needs the qualifier "not network-exposed" used elsewhere (L959, L636).
- Terminology: G2 sections say "dossier" (FR69, FR70, FR73, FR75, L421, L1050) while the rest of the PRD says "journal" (FR49–FR51, FR59–FR61, FR66–FR67); the equivalence is nowhere stated.
- Frontmatter L65/L66 still cite the "AI-greffier" as consumer of rationale and replacement records; L67 notes the replacement part is dropped, but L65 is not annotated.
- No leftover of "read-only AI", "never recommends" or "nothing sent to third parties" remains in the body outside explicitly superseded frontmatter entries and the deliberate "revises the earlier stance" sentence (L164). "No AI network call" (L653) and NFR-S2 "the only network calls are user-initiated provider/FX fetches" (L958) remain true of the app itself and are consistent with NFR-S3.

### Dual Audience Effectiveness

**For Humans:**
- Executive-friendly: Good — the stance revision is stated up front.
- Developer clarity: Good — draft lifecycle, statuses and exclusions are pinned in Appendix A; gaps listed above (stale drafts, computed outputs, source vs origin).
- Designer clarity: Good — inbox, per-study reminder, side-by-side, AI-annotated inert lines, "placed by AI" annotation.
- Stakeholder decision-making: Good.

**For LLMs:**
- Machine-readable structure: Excellent — phase tags, FR numbering, Appendix A.
- UX readiness: Good.
- Architecture readiness: Good — open points for Architecture: MCP server lifecycle vs single-instance lock (FR67, L881–885: does the MCP server run only while the app is open? L635 says "serving the active dossier").
- Epic/Story readiness: Good — FR69–FR78 map cleanly to Epic 8 stories.

**Dual Audience Score:** 4/5

### BMAD PRD Principles Compliance

| Principle | Status | Notes |
|-----------|--------|-------|
| Information Density | Met | 0 anti-patterns; some G2 restatement across sections (Exec Summary vs Scope) |
| Measurability | Partial | 11 soft violations, 10 pre-G2 |
| Traceability | Partial | notes P1/P4 gap; ≤2-actions criterion; FR52 phase |
| Domain Awareness | Met | AI-proposal posture and ToS added |
| Zero Anti-Patterns | Partial | FR67 leakage; subjective FR56/FR58 |
| Dual Audience | Met | |
| Markdown Format | Met | |

**Principles Met:** 4/7 (3 partial)

### Overall Quality Rating

**Rating:** 4/5 - Good

### Top 3 Improvements

1. **Unify the G2 exclusion/inclusion lists.** Define the MCP-exposed and MCP-excluded sets once (Appendix A) — including watchlist, keys, config, computed outputs/verdicts, transaction rationale, draft record — and make Success Criteria L259, FR69, NFR-S4 and NFR-A2 reference it instead of re-listing.
2. **Close the draft-lifecycle and origin gaps.** Specify owner actions on a stale draft (FR72), the source value of a validated AI cell (FR17 vs FR22/FR74), and carry the "≤ 2 actions" criterion into FR73/FR74.
3. **Resolve the notes phase dependency and sweep the leftovers.** Give study notes a phase-consistent FR (or remove "notes" from P1 wording in L217, L409, FR49, FR51); mark frontmatter L47 (and L65) superseded for the AI-line aspect; qualify L311, L528, L1016; state "dossier = journal".

### Summary

**This PRD is:** a strong, well-traced PRD whose G2 revision is thorough and testable, with a handful of enumeration drifts and leftover wording to reconcile before Epic 8 stories are cut.

**To make it great:** Focus on the top 3 improvements above.

## Completeness Validation

### Template Completeness

**Template Variables Found:** 0 — No template variables remaining ✓

### Content Completeness by Section

**Executive Summary:** Complete
**Success Criteria:** Complete
**Product Scope:** Complete (MVP, Growth, Phase 4, Vision)
**User Journeys:** Complete (J1–J6 incl. 2b, 3b; summary maps J6 → FR69–FR78)
**Functional Requirements:** Complete (FR1–FR78)
**Non-Functional Requirements:** Complete (29 NFRs incl. NFR-A1–A4)
**Other sections:** Domain, Innovation, Desktop, Scoping, Appendix A — complete. Appendix A defers SSG output set, plausibility rules, load-bearing input, golden tolerance and banned-verb list to Architecture (L1043, L1053–1054) — pre-existing, acknowledged.

### Section-Specific Completeness

**Success Criteria Measurability:** All measurable (L241 "~100 ms" and L248 "< ~5 minutes" approximate but quantified; L590 "usefulness measured as the share of drafts the owner validates" has no target — informational)
**User Journeys Coverage:** Yes — single user type (owner) plus the AI client as an actor in J6
**FRs Cover MVP Scope:** Yes
**NFRs Have Specific Criteria:** Some — see Measurability (P2, P3, U3, M1)

### Frontmatter Completeness

**stepsCompleted:** Present
**classification:** Present
**inputDocuments:** Present
**date:** Present (`completedDate`, `lastEdited: 2026-09-27`, editHistory)

**Frontmatter Completeness:** 4/4

### Completeness Summary

**Overall Completeness:** 100% (13/13 sections)
**Critical Gaps:** 0
**Minor Gaps:** 2 (Appendix A items deferred to Architecture; AI usefulness metric without target)

**Severity:** Pass

**Recommendation:** PRD is complete with all required sections and content present.

## Validation Summary

**Overall Status:** Warning — the PRD is usable and the G2 revision is sound; no critical issue. Warnings should be addressed before Epic 8 stories are written.

### Quick Results

| Check | Result |
|---|---|
| Format | BMAD Standard (6/6) |
| Information Density | Pass (0) |
| Product Brief Coverage | Good (0 critical, 0 moderate, 2 informational) |
| Measurability | 11 violations — Critical by threshold, judged Warning (1 from G2) |
| Traceability | Warning (5 issues, 0 orphan FRs) |
| Implementation Leakage | Warning (2, both pre-G2; MCP/Claude Code judged acceptable) |
| Domain Compliance | Pass |
| Project-Type Compliance | 100% |
| SMART Quality | 98.7% ≥ 3; avg 4.67 |
| Holistic Quality | 4/5 — Good |
| Completeness | 100% |

### Findings by severity

**Critical:** none.

**Warning (G2):**
1. Exclusion lists disagree: NFR-S4 (L963–964) omits the watchlist; NFR-A2 (L1007–1008) omits keys and configuration; Success Criteria L259–260 omits configuration; FR69/Appendix A (L896–898, L1051–1052) list all four.
2. FR69 / Appendix A (L896–898, L1051–1052) do not say whether computed outputs (zones, U/D, projected return, quality flags, frozen verdicts) are readable, although Journey 6 (L419–420) filters on upside/downside; "rationale" is ambiguous with transaction rationale (FR49, L835–836); the draft record the AI "can read" (FR77, L919–920) is outside the stated scope.
3. Source vs origin of a validated AI value: FR17 (L764–765) `ai-suggested` source vs FR74 (L911) "owner entry" vs FR22 (L776–777) "reconciles as a manual value".
4. Study notes: P1 wording (L217, L409, FR49 L835–836, FR51 L839–840) depends on FR78, which is [P4] (L921).
5. Frontmatter L47 still states "NEVER a 'suggested line'" without being marked superseded, contradicting FR33 [P4] (L798–801) and Journey 6 (L424–426).
6. FR72 (L905–906) introduces a "stale" draft status with no defined owner action (validate? reject only? expire?).

**Warning (pre-G2):**
7. FR52 [P1] print/export (L843) vs Growth/Phase 3 placement (L290, L705).
8. FR67 (L879–885) implementation leakage (`directories`, vendor list, SQLite `journal_mode`); NFR-M1 (L999) names Slint.
9. Soft measurability: FR45 "near" (L826), FR56 (L854), FR58 (L857); NFR-P2 (L949), NFR-P3 (L950–951), NFR-U3 (L991–992), NFR-M1 (L998); NFR-P1/P4 lack a measurement method (L947, L952).

**Info:**
10. FR75 (L914–915) is a scope note, not a requirement (duplicated in Appendix A L1049–1050, Scope L298–299).
11. Success criterion "≤ 2 actions" (L265) is not carried by FR73/FR74; AI usefulness (L590) has no target.
12. Leftover wording: persona "tools and AI may propose" (L311); "no cloud" (L528) without the remote-model qualifier; "no network server — … the only external interface" (L1016–1017).
13. "Dossier" (G2 sections) vs "journal" (rest of PRD) — equivalence not stated.
14. Frontmatter L65 still cites the "AI-greffier" as consumer of rationale; L67 drops only the replacement part.
15. Architecture question: MCP server lifecycle vs single-instance lock and "active dossier" (L635, FR67 L881–885).
16. Free-text study rationale/notes can mention positions and thus partially reveal the portfolio to the AI; the owner may want this stated as an accepted residual in L530 or NFR-S3.
17. Regulatory assumption for hosting AI proposals (L490–491) is stated as an assumption — acceptable, keep on the pre-release review list.
18. Executive Summary carries revision narrative (L164–165) and repeats Product Scope → Phase 4.
19. MCP and Claude Code: acceptable (owner-mandated interface; Claude Code confined to context sections, L158, L297).

### Strengths

- 6/6 core sections, zero density anti-patterns, zero template variables.
- G2 propagated consistently in the body; old stance explicitly superseded in frontmatter.
- Human gate and non-exposure specified as by-construction, testable properties (NFR-A1–A4, metamorphic test L262–263).
- Journey 6 → FR69–FR78 fully traced; no orphan FR.
- Domain posture updated for AI proposals and provider ToS.

**Recommendation:** PRD is usable but has issues that should be addressed. Review the warnings (1–6 first, as they bear directly on Epic 8 stories) and improve where needed — for example with the `bmad-edit-prd` workflow using this report.

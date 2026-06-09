---
stepsCompleted: [1, 2, 3, 4]
inputDocuments:
  - _bmad-output/planning-artifacts/prd.md
  - _bmad-output/planning-artifacts/architecture.md
  - _bmad-output/planning-artifacts/ux-design-specification.md
  - _bmad-output/planning-artifacts/ux-stock-study-screen.html
  - _bmad-output/planning-artifacts/product-brief-steadyinvest.md
  - _bmad-output/planning-artifacts/product-brief-steadyinvest-distillate.md
  - _bmad-output/planning-artifacts/research/domain-naic-better-investing-research-2026-06-05.md
  - docs/NAIC/SSGHandbook.pdf
  - docs/NAIC/SSGPlus_QuickStart.pdf
  - docs/NAIC/Stock Selection Guide Tutorial.pdf
  - docs/NAIC/A-Beginners-Tour-of-the-SSG-Jan-2015.pdf
  - docs/NAIC/BI_Member_Benefits.pdf
  - docs/NAIC/forms/Stock Selection Guide and Report.pdf
  - docs/NAIC/forms/stock selection guide.pdf
  - docs/NAIC/forms/Stock Comparison Guide.pdf
  - docs/NAIC/forms/Portfolio Management Guide.pdf
  - docs/NAIC/forms/stock checklist.pdf
---

# steadyinvest - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for steadyinvest, decomposing the
requirements from the PRD, the UX Design specification, and the Architecture decision document into
implementable stories. NAIC/BetterInvesting reference docs inform the SSG method content.

> Phase tags: **[P1]** MVP · **[P2]** Portfolio depth · **[P3]** Growth · **[V]** Vision.

## Requirements Inventory

### Functional Requirements

**Stock Study & Methodology Engine**
- FR1 [P1]: The user can create a Stock Study for a security.
- FR2 [P1]: The user can persist and reopen a study with its full state intact.
- FR3 [P1]: The user can update an existing study (re-fetch / edit) and extend its projection.
- FR4 [P1]: The system computes the SSG output set (Appendix A) deterministically from a study's inputs.
- FR5 [P1]: All study calculations are performed in the security's native currency.
- FR6 [P1]: The user can set judgment inputs (future growth, forecast P/E, low-price method) and see results recompute.
- FR7 [P1]: The system raises methodology quality flags per the Appendix A thresholds.
- FR8 [P1]: With fewer than five usable years, the study is computed on available data and carries a queryable low-confidence state.

**Calculation Integrity & Trust**
- FR9 [P1]: The user can load and run bundled golden reference studies; the system reports any deviation beyond tolerance.
- FR10 [P1]: The system detects and surfaces input plausibility issues (split/series break, currency mismatch, fiscal-period misalignment, out-of-bound) as warnings, distinct from quality flags.
- FR11 [P1]: The user can view a verdict's traceability — its inputs, their provenance, and the rule that produced the result.
- FR12 [P1]: The verdict's presentation is degraded or withheld testably when a load-bearing input is not validated or the study is low-confidence.
- FR13 [P1]: All user-facing signals are neutral — no output contains an action/recommendation verb from the banned-verb list (verifiable).
- FR14 [V]: The AI module is verifiably read-only — any write to studies/judgments/verdicts/transactions is rejected and logged.

**Data Acquisition, Provenance & Providers**
- FR15 [P1]: The user can auto-fetch a security's fundamentals, prices and estimates from a configured provider.
- FR16 [P1]: The user can enter, override and later correct any data field by hand.
- FR17 [P1]: Each data cell carries an independently queryable source (provider/manual/derived).
- FR18 [P1]: Each data cell carries an independently queryable provenance and timestamp.
- FR19 [P1]: Per-cell coverage is represented as present / to-fill / not-available-accepted.
- FR20 [P1]: The user can mark a cell or study "validated" (tri-state none/?/✓ with soft-lock — supersedes original FR20; see open issue).
- FR21 [P1]: The user can trigger a manual refresh of provider data.
- FR22 [P1]: On refresh, a manual value takes precedence over a fetched value while the fetched value is preserved (non-destructive reconciliation).
- FR23 [P1]: On provider failure, last-known values are retained and affected data is flagged stale/to-update.
- FR24 [P1]: A provider failure's cause (network, quota/rate-limit, invalid/absent key) is recorded and reported.
- FR25 [P1]: The user can use keyless providers, and add/replace/delete/test a provider API key stored in the OS secret store.
- FR26 [P2]: The user can configure a preferred provider and a fallback chain per field type (price, fundamentals, FX), with the effective provider recorded.
- FR27 [P2]: The system respects a provider's declared quotas/rate-limits and batches watchlist/portfolio fetches.
- FR28 [P2]: The system acquires, timestamps and retains FX rates per currency pair with a freshness state; FX is applied only at consolidation.
- FR29 [P1]: The system recomputes deterministically on a change of input, judgment, price, FX rate, or schema migration, distinguishing the cause.

**Charts & Judgment Interaction**
- FR30 [P1]: The user can view growth and valuation charts for a study.
- FR31 [P1]: The user can set a judgment line by exact value or direct manipulation (kept in sync), with live recalculation of zones.
- FR32 [P1]: The user can undo judgment changes; adjusting a line never destroys a saved input.
- FR33 [P1]: The system never auto-places or suggests a judgment line.

**Watchlist & Alerts**
- FR34 [P1]: The user can maintain a watchlist (add, edit, remove, reorder).
- FR35 [P1]: The system raises a neutral in-app alert when a watched security enters its buy zone.

**Portfolio, Transactions & Holdings**
- FR36 [P1]: The user can record holdings in a single portfolio (security, quantity, purchase price) in a single reference currency, and edit/remove a holding.
- FR37 [P2]: The user can maintain multiple portfolios (one per bank/account).
- FR38 [P2]: The user can hold securities denominated in multiple currencies.
- FR39 [P2]: The user can record buy/sell transactions including partial sells (date, quantity, unit price, fees, currency) and edit/delete them; cost basis is weighted-average.
- FR40 [P1]: The user can trigger a manual price refresh recomputing each holding's zone and showing freshness.
- FR41 [P2]: The user can record dividends; the study uses gross, the portfolio's reinvestable cash uses net per the withholding rule.

**Risk Management**
- FR42 [P1]: The user can set a trailing stop per holding; it ratchets up only.
- FR43 [P1]: The system computes a simple capital-at-risk for the single portfolio per the Appendix A formula.
- FR44 [P2]: The system computes capital-at-risk per currency → per bank → global total in the reference currency (FX only at consolidation).
- FR45 [P2]: The system checks concentration against total invested capital and warns near a configured majority share.
- FR46 [P1]: On Sell-zone entry or stop breach, the system surfaces a neutral fact and offers manual actions (sell / raise stop), never auto-acting.
- FR47 [P1]: The stop-loss takes priority over the Sell zone (isolated business rule).
- FR48 [P2]: On a sell, the system surfaces replacement candidates from the watchlist and flags re-concentration by sector/currency.

**Cumulative Memory & Journal**
- FR49 [P1]: The user can capture a decision rationale as a first-class field on studies and transactions.
- FR50 [P1]: The user can reopen a past study and visually compare its recorded projection to the security's actual trajectory since.
- FR51 [P1]: The system durably preserves the time-series of judgments, provenance, validation, rationale and notes.

**Reporting & Printing**
- FR52 [P1]: The user can print / export to PDF a Stock Study in a layout close to the original form, neutral labels, no NAIC marks/logos or verbatim text.
- FR53 [P2/P3]: The user can print / export the other forms (Company Comparison, Portfolio) in the same faithful-but-neutral layout.

**Application Shell & Data Management**
- FR54 [P1]: The user can list, search, sort and filter saved studies and open them from a home dashboard.
- FR55 [P1]: The user can delete or archive a study (with confirmation); deletions never corrupt the journal time-series.
- FR56 [P1]: The user can switch a study between an entry regime (dense editing) and a contemplation regime (reading/judgment), with the active regime clearly indicated.
- FR57 [P1]: The user can view a consistent legend for freshness/provenance/coverage/confidence states.
- FR58 [P1]: Every main surface presents an actionable empty state and clear neutral error/feedback messages.
- FR59 [P1]: The user can export/import a single study to a portable versioned file (round-trip preserves identity).
- FR60 [P1]: The user can export/import the whole journal in a versioned format, validated on import (reject/migrate on mismatch).
- FR61 [P1]: The user can restore from a backup with integrity and version-compatibility checks before overwrite.
- FR62 [P1]: The user can access non-blocking contextual help / glossary and a read-only demonstration study.

**Configuration, Posture & Operation**
- FR63 [P1]: The user can configure providers/keys, the single global reference currency, risk thresholds, the label set (NAIC↔neutral) and locale number format — without a blocking setup flow.
- FR64 [P1]: A disclaimer (educational, not a financial advisor) is always visible, and the product never issues recommendations.
- FR65 [P1]: The user can run the full study and portfolio-risk workflow offline; the only online action is a user-initiated refresh.
- FR66 [P1]: The journal is kept in a portable local store an external system (e.g. file sync) can back up.

### NonFunctional Requirements

**Correctness & Calculation Integrity (top priority)**
- NFR-C1: The calculation engine is deterministic — identical inputs always produce identical outputs, bit-stable across runs and platforms.
- NFR-C2: Engine output matches every bundled golden reference study (exact zoning/verdict; within ±0.5% on derived numerics, tolerance configurable).
- NFR-C3: Property-based invariants hold (zones ordered low<buy<hold<sell<high; U/D ≥ 0; capital-at-risk ≥ 0; FX round-trip A→B→A within 1e-6).
- NFR-C4: FX is applied only at consolidation; per-currency study results are independent of the chosen reference currency.
- NFR-C5: Engine + risk crate are gated in CI by golden-fixture and property tests (≥95% coverage of calc paths); a failing test blocks merge.

**Performance**
- NFR-P1: Judgment-line recalculation and zone re-render feel live — within ~100 ms perceived while dragging.
- NFR-P2: Opening or recomputing a full study completes within ~1 s on typical hardware.
- NFR-P3: A manual portfolio refresh (tens of holdings) completes within a few seconds and never blocks the UI.
- NFR-P4: The app reaches an interactive state within ~3 s of launch.

**Security & Privacy**
- NFR-S1: Provider API keys live only in the OS secret store — never in repo, plaintext config, logs, exports or backups.
- NFR-S2: No telemetry/analytics; the only network calls are user-initiated provider/FX fetches.
- NFR-S3: All persistent data is local; nothing sent to third parties beyond the chosen provider under the user's own key.
- NFR-S4: The AI module never exfiltrates the journal to a remote service unless the user explicitly enables a remote AI.

**Reliability & Data Integrity**
- NFR-R1: The full study + portfolio-risk workflow runs offline; losing the network degrades only fetching, with stale flagging — never a silent wrong value.
- NFR-R2: Writes are crash-safe/atomic — an interrupted operation never corrupts the journal.
- NFR-R3: Schema migrations are forward-safe; an older journal always opens (or is migrated) in a newer build, no data loss.
- NFR-R4: Reconciliation never destroys a manual value or judgment; the provider value is preserved alongside.
- NFR-R5: Export/import and restore verify integrity and schema version; a mismatched/corrupt file is rejected with a clear message, never partially applied.

**Portability & Compatibility**
- NFR-X1: Identical behavior and numeric results across Windows, macOS, Linux.
- NFR-X2: Locale-aware number parsing/formatting (decimal comma, thousands), configurable independently of OS locale.
- NFR-X3: The journal file is portable across platforms.

**Usability & Accessibility (right-sized)**
- NFR-U1: Buy/hold/sell zones distinguishable without relying on color alone (color-blind-safe palette + a secondary cue).
- NFR-U2: Primary study and data-entry workflows are fully keyboard-operable.
- NFR-U3: On-screen and printed layouts stay recognizably close to the original form (functional layout) with neutral labels.

**Maintainability & Testability**
- NFR-M1: The UI is a thin layer over a UI-independent tested calculation crate and a versioned data contract decoupled from Slint and the storage engine.
- NFR-M2: The data contract carries an explicit schema_version; any breaking change ships a migration.

### Additional Requirements

*(From the Architecture decision document — technical/infra requirements that shape epics & stories.)*

- ADD1 [P1] **Starter / scaffold (→ Epic 1, Story 1):** initialize a Cargo workspace with 6 crates
  (`core`, `contract`, `ingestion`, `persistence`, `report`, `app`); seed the `app` UI crate from the
  official Slint Rust template (`cargo generate --git https://github.com/slint-ui/slint-rust-template`).
  Pinned deps: slint 1.16, rusqlite 0.40 (bundled), rust_decimal 1.42 (+maths), reqwest 0.13
  (rustls-tls,json) + tokio 1.52, serde 1, thiserror 2.0, proptest 1.9, tracing, keyring 4.0, directories.
- ADD2 [P1] **Cardinal Rule enforced by structure:** all calculation lives in `core` (no I/O/UI/SQL/net);
  exact decimal (`rust_decimal`), never `f32/f64` in the decision chain; named rounding only at display.
- ADD3 [P1] **Week-1 de-risking spikes (precede UI commitment):** (A) Slint dense grid + paste-a-column;
  (B) native-Slint draggable judgment line + <100 ms zone recolor; (C) decimal CAGR precision +
  cross-OS determinism hash. Fallback if B fails: dedicated Slint canvas / plotters→SharedPixelBuffer + TouchArea overlay.
- ADD4 [P1] **Versioned data contract & three version axes:** `schema_version` (blob) + SQLite
  `PRAGMA user_version` + `method_version`; forward-safe migrations; lazy upgrade on save; read-only on newer file.
- ADD5 [P1] **Hybrid persistence model:** normalized tables for aggregated data (portfolios, holdings,
  transactions, fx_rates, watchlist_items); versioned JSON blob (TEXT) for studies/judgments; money stored as TEXT decimal strings (never REAL).
- ADD6 [P1] **Journal identity & integrity:** `journal_id` (UUID) + monotonic logical version in the DB;
  last-used pointer = (journal_id, last-seen-version); single-instance file lock; backups carry (journal_id, version, hash).
- ADD7 [P1] **App-config vs journal boundary:** `directories` for app-config (last path, recent journals,
  UI prefs); `keyring` for secrets; user-selectable journal directory + reopen last-used (the added DB-location requirement, to file as an FR).
- ADD8 [P1] **Sync-safety:** detect sync-watched DB paths (Synology/Dropbox/OneDrive/iCloud), warn, and use
  `journal_mode=DELETE/TRUNCATE` there; live DB local, versioned backups/exports pushed to the sync folder.
- ADD9 [P1] **Foundational Invariant realized by construction:** every asserted fact carries
  (source, logical_version, timestamp, hash_of_dependencies); transactional recompute; content-addressed
  verdict `f(hash(inputs), method_version)`; invalidation, not silent overwrite.
- ADD10 [P1] **Verdict versioning:** decision-time verdict frozen & immutable (the only one persisted);
  "recompute with today's method" computed on demand for comparison/debug, never persisted, never auto (the added verdict-versioning requirement, to file as an FR).
- ADD11 [P1] **Method specification ("Appendix A" deferrals):** author a method spec consumed by `core`
  (exact SSG output set, plausibility rules, banned-verb list, golden tolerance, "load-bearing input" definition) before implementing the engine.
- ADD12 [P1] **FR9 runtime self-check assets:** bundled golden reference studies as app assets
  (`app/assets/golden/`) + a "verify engine" UI path — distinct from CI test goldens.
- ADD13 [P1] **FR50 price-history cache:** post-decision price series stored in `persistence` (sourced via ingestion refresh) to overlay projection-vs-actual.
- ADD14 [P1] **CI / quality gates:** 3-OS matrix (fmt, clippy -D warnings, tests, golden/property/metamorphic,
  determinism hash, marker-confusability snapshot, `cargo deny` GPL-3.0 license audit); UI stories require visual verification (DoD).
- ADD15 [P1] **Observability & errors:** `tracing` to a local rotating log (no telemetry); per-crate `thiserror`
  error enums; neutral cause-named messages; no silent `.ok()`; injected `Clock`/`IdGen` for determinism.

### UX Design Requirements

*(From the UX Design specification — first-class actionable work items.)*

**Design system foundation**
- UX-DR1 [P1]: Token-based design system native to Slint — colour/alpha token family + metric/typo token family, swappable at runtime.
- UX-DR2 [P1]: Greyscale ink scale (dark default + light) + three judgment-zone hues (Okabe-Ito: Buy #009E73, Hold #E69F00, Sell #D55E00), colour-blind-safe.
- UX-DR3 [P1]: Zone rendering = theme-asymmetric alpha (dark 32–40% / light 15–18%) + 1.5–2px full-saturation edge stroke; redundant encoding (hue + value + vertical position + BUY/HOLD/SELL label).
- UX-DR4 [P1]: Typography — Inter UI (400/600) + a tabular-figures numeric font (weights 400/500/600), NOT tnum-on-Inter; 4px type scale (verdict 28 / H2 18 / H3 15 / body 14 / caption 12).
- UX-DR5 [P1]: Spacing 4px base; dense grid (row 28px); flat elevation (no shadows); active-cell cursor (brighter surface + 1px ink ring).
- UX-DR6 [P1]: Three render profiles — dark (default), light, and print/grayscale (verdict survives in pure greyscale).
- UX-DR7 [P1]: Theme tokens are a single neutral source of truth read by the UI (no FFI); theme/regime change forces a redraw.

**Components (build per the Component Strategy)**
- UX-DR8 [P1]: Data-grid + editable cell — virtualized SSG tables, keyboard cell-cursor, paste-a-column, inline edit, visible grid, tabular figures; per-cell source × freshness × tri-state review with soft-lock.
- UX-DR9 [P1]: Collapsible SSG section — chevron, summary-scent line when folded, persisted fold state, bound to regime fold presets; print forces expanded.
- UX-DR10 [P1]: Semi-log growth chart (§1) — Sales/EPS/Price lines (solid historical / dashed projected), 5–30% guide fan, 1→200 log axis, draggable trend lines, NO zones (native Slint).
- UX-DR11 [P1]: Zone bar + price axis (§4) — single vertical Buy/Neutral/Sell thirds, present-price marker, side price axis, live <100ms recolor; full vs muted (regime) and provisional (unvalidated) states.
- UX-DR12 [P1]: Scenario-compare overlay — compare two judgment-line placements + their zones/U-D/return; never destroys a saved input (Phase 1: one alternate).
- UX-DR13 [P1]: Verdict badge — full colour / provisional (hatched + temporal provenance) / degraded / withheld.
- UX-DR14 [P1]: Sticky verdict bar — verdict + present price + projected return + appreciation + capital-at-risk, pinned during scroll/fold.
- UX-DR15 [P1]: Trust/state markers — ✓ (geofenced ink-green in entry, attenuated in contemplation), ? (hollow + 2nd non-colour channel), missing (bold glyph/hatch), stale (~60% + hollow dot), source on demand; confusability-gated (≥98% ID, <2% pairwise at 14px).
- UX-DR16 [P1]: Global error/alert banner — neutral, names cause (network/quota/key); same register for buy-zone & stop alerts.
- UX-DR17 [P1]: Form header + capitalization block (faithful study-header fields); calc-row component (label · computation · boxed result).
- UX-DR18 [P1]: State legend (FR57) + actionable empty/error states (FR58) + contextual help/glossary popover + read-only demo study (FR62).
- UX-DR19 [P1]: App nav rail + study dashboard (list/search/sort/filter/archive — FR54/55).
- UX-DR20 [P1]: Portfolio set — holdings register, capital-at-risk panel, trailing-stop control, neutral sell/raise-stop action sheet, watchlist.
- UX-DR21 [P1]: Settings panels (no wizard) — provider/key, reference currency, risk thresholds, label set (NAIC↔neutral), locale.

**Behaviour, posture, accessibility**
- UX-DR22 [P1]: Two regimes (entry ↔ contemplation) on one skeleton — fold presets + colour/marker delta; constant geometry (no re-layout/jank during a drag).
- UX-DR23 [P1]: Verdict-integrity rule — full saturated colour only when every load-bearing input is ✓ & not stale; else provisional texture + temporal provenance caption.
- UX-DR24 [P1]: Asymmetric attenuation — only ✓ may dim in contemplation; ?, stale, divergent, missing never attenuate; traced conscious-override path for accepting a non-green input.
- UX-DR25 [P1]: Implicit recompute (no "Calculate" button); undo/redo everywhere; nothing destructive is silent (delete/archive & "unlock all" confirm).
- UX-DR26 [P1]: Fact-only neutral microcopy ("the price entered the zone you defined"); always-visible educational disclaimer (footer).
- UX-DR27 [P1]: Keyboard-first — full keyboard operation, always-visible focus/active-cell, section quick-jump §1–§5, judgment line settable by exact value (not mouse-only); respect OS reduced-motion and font-scale.
- UX-DR28 [P1]: Desktop window-size responsiveness — wide/comfortable/compact; §3 A–H table keeps columns (horizontal scroll, fidelity > reflow); min window size; persist window/fold/regime state.
- UX-DR29 [P1]: French-first UI via Slint @tr() (i18n-ready), distinct from the runtime NAIC↔neutral label set.

### FR Coverage Map

*(Every FR mapped to an epic. FR13 neutrality and FR63 no-wizard settings are cross-cutting, built incrementally; their primary home is noted.)*

- FR1 → Epic 2 (create study) · FR2 → Epic 2 (persist/reopen) · FR3 → Epic 2 (edit/extend)
- FR4 → Epic 1 (SSG output set) · FR5 → Epic 1 (native currency) · FR6 → Epic 2 (judgment inputs; compute in Epic 1)
- FR7 → Epic 1 (quality flags) · FR8 → Epic 1 (5-yr floor/low-confidence) · FR9 → Epic 1 (CI golden gate) + Epic 2 (verify-engine UI)
- FR10 → Epic 1 (plausibility) · FR11 → Epic 1 (traceability data) + Epic 2 (view) · FR12 → Epic 2 (verdict degraded/withheld)
- FR13 → Epic 2 (neutral signals; cross-cutting) · FR14 → Epic 8 [V] (AI read-only)
- FR15 → Epic 3 (auto-fetch) · FR16 → Epic 2 (manual entry/override)
- FR17/FR18/FR19 → Epic 1 (per-cell source/provenance/coverage model) + Epic 2 (display)
- FR20 → Epic 2 (tri-state validated + soft-lock) · FR21–FR25 → Epic 3 (refresh, reconciliation, failure, keys)
- FR26/FR27/FR28 → Epic 6 [P2] (fallback chain, rate-limit batching, FX)
- FR29 → Epic 1 (recompute on input/judgment change) + Epic 3 (recompute on refresh/price/FX, cause-distinguished)
- FR30/FR31/FR32/FR33 → Epic 2 (charts, draggable judgment line, undo, never auto-suggest)
- FR34/FR35 → Epic 4 (watchlist + buy-zone alert) · FR36/FR40/FR42/FR43/FR46/FR47 → Epic 4 (single portfolio, refresh, trailing stop, capital-at-risk, neutral triggers, stop-priority)
- FR37/FR38/FR39/FR41/FR44/FR45/FR48 → Epic 6 [P2] (multi-portfolio, multi-currency, ledger, dividends, consolidation, concentration, replacement)
- FR49 → Epic 2 (decision rationale) · FR50 → Epic 5 (reopen & confront vs actual) · FR51 → Epic 1 (durable time-series storage) + Epic 2 (capture)
- FR52 → Epic 5 (PDF export of study) · FR53 → Epic 7 [P3] (other forms)
- FR54/FR55/FR56/FR57/FR58/FR62 → Epic 2 (dashboard, archive, regimes, legend, empty/error states, help/demo)
- FR59/FR60/FR61 → Epic 5 (export/import study + journal, restore with integrity)
- FR63 → Epic 2 (labels/locale) + Epic 3 (provider/key) + Epic 4 (currency/thresholds) + Epic 5 (DB location) — incremental, no-wizard
- FR64 → Epic 2 (always-visible disclaimer) · FR65 → Epic 1/Epic 2 (offline operation) · FR66 → Epic 1 (portable store) + Epic 5 (backup/restore)

## Epic List

> **Structure rationale (post party-mode):** epics are cut **vertically by user value**, but the
> foundation is split out so each epic *closes* for a solo dev. The highest risk (silent-wrong-signal)
> is attacked first by putting the **deterministic engine + the pure normalization layer + the full
> test/CI harness** in Epic 1 (Murat's "~7× risk" lever). The signature **draggable judgment chart is
> part of the first usable product (Epic 2)**, not a deferred "charts" epic — its *feasibility* is
> de-risked by a hardened Week-1 spike inside Epic 1. The judgment value can also be set numerically
> (same `core` function), so the chart is an input surface, not a prerequisite for the verdict.

### Epic 1: Proven SSG core & data foundation (headless)
Scaffold the Cargo workspace and deliver a deterministic, exact-decimal SSG engine that is
**trustworthy by construction** — fed through the *same canonical normalization* providers will later
use, proven by golden + property + metamorphic tests (incl. split-invariance), persisting a versioned,
provenance-stamped journal, and guarded by cross-OS CI gates from story one. Closes headless ("the
math is proven"), with no UI beyond a CLI/test self-check.
**Includes:** workspace + 6 crates (ADD1); Week-1 spikes A (grid paste-a-column), **B hardened**
(real semi-log NAIC chart + draggable point recalculating a signal, <100 ms — the go/no-go that locks
Slint-only), C (decimal CAGR + cross-OS determinism hash) (ADD2,3); **method spec as a versioned
artifact linked to the dep-hash** (ADD11); **pure `normalize` function** (IFRS/GAAP, split/series,
fiscal-period, currency-of-report) inside `core` (Murat lever #1); `contract` with the **full
provenance model** (source, logical_version, timestamp, dep-hash) so later epics fill it rather than
migrate it (ADD4,9); `persistence` hybrid schema + journal_id + **migrations harness** (ADD5,6);
golden self-check engine; **full test/CI harness in stories 1–2** (determinism hash, golden runner +
fixture format, property/metamorphic runner with split-invariance, schema-drift detector + schema v1)
(ADD12,14,15); the **static verdict-integrity invariant (2a)** and the **coherence-frame invariant
(2b) defined on the manual-mutation rail** so Epic 3's refresh just branches onto it.
**FRs covered:** FR4, FR5, FR7, FR8, FR10, FR17–FR19 (model), FR29 (compute), FR51 (storage), FR9 (CI gate), FR65/FR66 (foundation).

### Epic 2: The trustworthy Stock Study (first usable product, with the judgment gesture)
The user creates, fills **by hand**, judges, and **trusts** a complete SSG study — fully offline. The
signature interaction is here: the **semi-log growth chart with a draggable judgment line and live
<100 ms zone recolor** (feasibility already proven by Epic 1's spike B), with the judgment also
settable by exact value (keyboard). Per-cell provenance display + **tri-state validation (none/?/✓)
with soft-lock** + low-confidence; **verdict integrity** (full colour only when load-bearing inputs
are ✓ & fresh, else provisional/withheld); traceability view; decision rationale; the FR9
"verify-engine" path; app shell (nav rail, dashboard list/search/sort/filter/archive, two regimes,
legend, actionable empty/error states, contextual help + read-only demo); always-visible disclaimer.
This is the increment that proves the core value — *forged conviction*, not a spreadsheet. *(Scenario-
compare and the full Epic-1 provenance UI polish land here or are deferred per story sizing.)*
**FRs covered:** FR1, FR2, FR3, FR6, FR9 (UI), FR11 (view), FR12, FR13, FR16, FR17–FR19 (display), FR20, FR30, FR31, FR32, FR33, FR49, FR51 (capture), FR54, FR55, FR56, FR57, FR58, FR62, FR63 (labels/locale), FR64, FR65.

### Epic 3: Provider data & reconciliation
The study fills itself in seconds and degrades honestly when the provider fails. Auto-fetch from a
configured provider (EODHD first) **through Epic 1's canonical normalizer**, keys in the OS keychain
(keyless providers supported), manual refresh, **non-destructive reconciliation** (manual wins,
provider preserved, divergence → auto-?), graceful failure (stale flagging + cause: network/quota/key).
The **dynamic coherence-frame invariant (2b) branches onto Epic 1's mutation rail** (refresh flips
✓→? and degrades the verdict in the same frame).
**FRs covered:** FR15, FR21, FR22, FR23, FR24, FR25, FR29 (refresh/price, cause-distinguished), FR63 (provider/key).

### Epic 4: Watchlist & single-portfolio risk
One honest picture of risk + neutral alerts. Watchlist (add/edit/remove/reorder) with neutral
buy-zone alerts; a single-portfolio holdings register (security, quantity, purchase price) in one
reference currency; manual price refresh recomputing each holding's zone with freshness; a **trailing
stop (ratchet-up only)** and a **simple capital-at-risk** (core math from Epic 1); neutral Sell-zone /
stop-breach triggers offering manual actions (sell / raise stop), with the **stop-priority rule**.
*(Depends on Epic 1 for the risk math and Epic 3 for current prices — sequence E3 before E4.)*
**FRs covered:** FR34, FR35, FR36, FR40, FR42, FR43, FR46, FR47, FR63 (currency/thresholds).

### Epic 5: Cumulative memory & portability
The journal becomes an appreciating, portable, safe asset. Reopen a past study and **confront its
recorded projection against the security's actual trajectory** (post-decision price-history cache,
ADD13); **export/import a single study and the whole journal** in a versioned, integrity-checked
format; **restore from backup** with version/integrity checks before overwrite; **user-selectable
journal directory + recent journals + sync-safety** (ADD7,8 — the added DB-location requirement);
**PDF export** of the study in a faithful, neutral, grayscale-safe layout.
**FRs covered:** FR50, FR52, FR59, FR60, FR61, FR63 (DB location), FR66 (backup/restore).

### Epic 6 [P2]: Multi-portfolio, multi-currency & full risk overlay
Multiple portfolios (one per bank/account), multi-currency holdings, the full transaction ledger
(partial sells, weighted-average cost basis, fees), dividends (gross in study / net reinvestable),
FX acquisition + consolidation (per-currency → per-bank → global), concentration on total capital,
provider fallback chain + rate-limit batching, and replacement-candidate surfacing on a sell.
**FRs covered:** FR26, FR27, FR28, FR37, FR38, FR39, FR41, FR44, FR45, FR48.

### Epic 7 [P3]: Comparison, health review, screening & reports
Company Comparison, Portfolio Health Review (diversification/quality roll-up), discovery/screening +
Quick Screen, additional provider adapters, and PDF/print of the other forms.
**FRs covered:** FR53 (+ roadmap features).

### Epic 8 [V]: Read-only AI clerk-of-memory & MCP façade
An optional, local-first, **read-only** AI that interrogates the past over the versioned data contract
(coherence checks, discipline-drift, pre-mortems) — never recommending, never writing — exposed via a
read-only MCP façade. Capability asymmetry enforced by construction.
**FRs covered:** FR14 (+ vision features).

## Epic 1: Proven SSG core & data foundation (headless)

Scaffold the workspace and deliver a deterministic, exact-decimal SSG engine — trustworthy by
construction, proven by golden/property/metamorphic tests, persisting a versioned provenance-stamped
journal, guarded by cross-OS CI gates from story one. Closes headless. *(Spikes are throwaway: their
deliverable is a go/no-go decision + a short findings note, not production code.)*

### Story 1.1: Workspace scaffold & CI gate skeleton

As the developer,
I want a Cargo workspace with the six crates and a cross-platform CI pipeline,
So that every later story builds on a consistent structure with quality gates from day one.

**Acceptance Criteria:**

**Given** an empty repository
**When** the workspace is scaffolded
**Then** `core`, `contract`, `ingestion`, `persistence`, `report`, `app` crates exist with
`[workspace.dependencies]` pinning the agreed versions (slint 1.16, rusqlite 0.40, rust_decimal 1.42,
reqwest 0.13, tokio 1.52, serde 1, thiserror 2.0, proptest 1.9, tracing, keyring 4.0, directories)
**And** `rust-toolchain.toml` pins MSRV ≥ 1.88, with `rustfmt.toml`, `clippy.toml`, `deny.toml` present
**When** CI runs on the Windows/macOS/Linux matrix
**Then** `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and `cargo deny` all pass on the empty workspace
**And** a placeholder cross-OS **determinism-hash job** exists and is green (asserts an identical SHA-256 over a trivial computed vector across the three OS).

### Story 1.2: SSG method specification (versioned oracle)

As the developer,
I want the exact SSG method pinned in a versioned specification,
So that the engine and its golden tests have a single authoritative oracle.

**Acceptance Criteria:**

**Given** the PRD Appendix-A deferrals
**When** the method spec is authored
**Then** it defines the **SSG output set**, **quality-flag thresholds**, **plausibility rules**,
**"usable year"/low-confidence rule**, **"load-bearing input" definition**, **banned-verb list**,
**golden tolerance (±0.5%)**, and the **named rounding mode + per-field display scale**
**And** it declares a `method_version` string
**And** the spec is referenced by `core` such that changing it changes the `method_version` (and, by
the Foundational Invariant, the dep-hash of derived facts).

### Story 1.3: `contract` v1 — versioned types & provenance model

As the developer,
I want versioned serde data-contract types carrying full provenance,
So that `core`, `persistence` and later epics share one vocabulary and never migrate the schema to add provenance.

**Acceptance Criteria:**

**Given** the data-contract decisions
**When** `contract` v1 is implemented
**Then** it defines `Study`, `Judgment`, a `Cell` carrying value + **source (provider/manual/derived)
× freshness (current/stale) × review (none/?/✓)** + **provenance `(source, logical_version, timestamp,
hash_of_dependencies)`**, money as a decimal type serialized as a string, and an explicit `schema_version`
**And** every type round-trips: `parse(serialize(x)) == x` (property test)
**And** new fields use `#[serde(default)]` and the journal types do NOT use `deny_unknown_fields` (forward-compat).

### Story 1.4: Spike A — dense editable grid with paste-a-column (go/no-go)

As the developer,
I want to prove a Slint dense grid supports spreadsheet-grade entry,
So that the entry-regime feasibility is settled before building the study UI.

**Acceptance Criteria:**

**Given** a throwaway Slint example
**When** the user pastes a column of 10 year-values into the grid
**Then** the values land in the correct cells parsed as `Decimal`, with keyboard cell-cursor navigation working
**And** the spike concludes with an explicit **GO/NO-GO** note; NO-GO triggers a documented alternative before the study UI is committed.

### Story 1.5: Spike B — native-Slint draggable judgment line, <100 ms recolor (hardened go/no-go)

As the developer,
I want to prove the signature interaction is feasible natively in Slint,
So that the "Slint-only, no egui/web" decision is locked or revisited before any UI investment.

**Acceptance Criteria:**

**Given** a throwaway Slint example rendering a **real semi-log SSG chart** (1→200 axis, a Sales/EPS/Price series from `core` golden data)
**When** the user drags a judgment line (or the forecast point)
**Then** the zone band **recolours and the recomputed signal updates within ~100 ms** (measured click-to-pixel, including the recompute), on the target hardware
**And** the spike concludes with an explicit **GO/NO-GO**; NO-GO triggers the architecture fallback decision (dedicated Slint canvas / plotters→SharedPixelBuffer + TouchArea overlay) — NOT egui, NOT web — before Epic 2.

### Story 1.6: Spike C — exact-decimal CAGR precision & cross-OS determinism

As the developer,
I want to prove `rust_decimal` (+maths) gives exact, reproducible compound-growth results,
So that the no-float determinism decision is validated end to end.

**Acceptance Criteria:**

**Given** a known multi-year series
**When** CAGR / projection are computed with `rust_decimal` `maths` (`powd`)
**Then** the result matches the hand-computed value to the defined precision
**And** the CI determinism-hash job asserts an **identical hash across Windows/macOS/Linux**
**And** a GO/NO-GO note records the precision/rounding behaviour for the method spec.

### Story 1.7: `core` normalization layer (pure, metamorphic-tested)

As the developer,
I want a pure normalization function turning raw financial inputs into a canonical form,
So that the most dangerous source of a silent-wrong-signal is built and tested first, and reused by both manual entry and providers.

**Acceptance Criteria:**

**Given** raw financial inputs (manual or, later, provider) with the documented edge cases
**When** `normalize(raw) -> CanonicalFinancials` runs
**Then** it correctly handles **split/series breaks, fiscal-period misalignment, IFRS↔US-GAAP differences, and currency-of-report**, deterministically and with no I/O
**And** metamorphic tests hold: a **3:1 split applied to inputs yields the same canonical series** (split-invariance); equivalent IFRS/GAAP inputs yield the same verdict; multiplying all amounts by k leaves ratios/verdict unchanged (scale-homogeneity)
**And** a year missing a load-bearing field is marked `unknown/insufficient`, never coerced to 0.

### Story 1.8: `core` SSG calculation engine

As the developer,
I want the deterministic five-section SSG engine,
So that a study's canonical inputs produce the exact SSG output set, quality flags and verdict.

**Acceptance Criteria:**

**Given** canonical inputs and judgment inputs (future growth, forecast P/E, low-price method)
**When** the engine computes
**Then** it produces the full **SSG output set in native currency** (growth, management ratios, P/E
history, zoning, U/D ratio, 5-yr return projection) per the method spec, deterministically (FR4, FR5)
**And** it raises **quality flags** (Appendix-A thresholds) (FR7) and **plausibility warnings** (distinct
from quality flags) (FR10), and emits a **low-confidence** state when usable years < 5 (FR8)
**And** property invariants hold (zones ordered low<buy<hold<sell<high; U/D ≥ 0)
**And** the engine performs **no I/O / no UI / no SQL / no network** (Cardinal Rule) and recomputes on any input/judgment change (FR29).

### Story 1.9: Golden reference studies & self-check gate

As the developer,
I want bundled golden reference studies run as a self-check,
So that any deviation of the engine from the canonical method is caught automatically.

**Acceptance Criteria:**

**Given** a set of frontier golden reference studies (synthetic, documented provenance, verdict-boundary cases)
**When** the self-check runs (in CI and via a callable path)
**Then** each golden's zoning/verdict matches exactly and derived numerics are within ±0.5%
**And** an intentionally wrong golden makes the gate **fail the build** (no silent pass)
**And** the bundled goldens are available as assets for the later FR9 "verify-engine" UI path.

### Story 1.10: `persistence` v1 — hybrid store, journal identity & migrations

As the developer,
I want the local SQLite journal with identity and a migrations harness,
So that studies/judgments persist durably and the journal survives version bumps.

**Acceptance Criteria:**

**Given** a journal path
**When** the store is opened/created
**Then** it uses bundled SQLite with **normalized tables** (portfolios, holdings, transactions,
fx_rates, watchlist_items) and a **versioned JSON blob (TEXT)** for studies/judgments, money stored as
TEXT decimal strings, and writes a **`journal_id` (UUID) + monotonic logical version** into the DB
**And** a `Study` write→read round-trips equal (via `contract`), in a single atomic transaction
**And** `PRAGMA user_version` + `schema_version` are set; a **migrations harness** applies v1, a
**schema-drift detector** fails if a persisted struct changes without a migration + a frozen
`tests/corpus/v1.db` fixture, and a newer-than-app file opens **read-only** with a clear message.

### Story 1.11: Verdict-integrity & coherence invariants

As the developer,
I want the trust invariants enforced by type and by test,
So that a verdict can never silently outrun the state of its inputs.

**Acceptance Criteria:**

**Given** a computed study state
**When** a `FullVerdict` is constructed
**Then** it is constructible **only** when every load-bearing input is `✓` and not stale (compiler-enforced); otherwise the verdict is provisional/withheld (static invariant 2a, property-tested)
**When** any load-bearing input is mutated (e.g. a validated cell is edited)
**Then** in the **same transaction/coherence frame** its review flips `✓→?` **and** the dependent verdict degrades — never one without the other (invariant 2b, tested on the manual-mutation rail so Epic 3's refresh later just branches onto it)
**And** verdict and staleness derive from the **same immutable state snapshot** (no incoherent intermediate frame is representable).

## Epic 2: The trustworthy Stock Study (first usable product, with the judgment gesture)

The user creates, fills by hand, judges, and trusts a complete SSG study — fully offline. Internal
story order builds the **numeric-input verdict first** (an early demonstrable checkpoint), then the
**signature draggable judgment chart**. Builds on Epic 1's proven engine, contract, and persistence.

### Story 2.1: Application shell, theme & always-visible disclaimer

As Guy,
I want a calm native shell with nav, theming and the educational disclaimer,
So that I can move between Studies/Watchlist/Portfolio/Settings and always see the app's neutral posture.

**Acceptance Criteria:**

**Given** the app launches
**When** the main window renders
**Then** a left nav rail (Studies/Watchlist/Portfolio/Settings) + a top bar are shown, driven by the
token design system (dark default + light), with French UI via `@tr()`
**And** a footer disclaimer ("educational, not a financial advisor") is visible on every page (FR64)
**And** window size, theme and (later) fold/regime state persist across launches
**And** the label set (NAIC↔neutral) and locale number format are runtime-swappable (FR63, no wizard).

### Story 2.2: Create, save and reopen a study

As Guy,
I want to create a study for a ticker and reopen it later with full state,
So that my work is durable.

**Acceptance Criteria:**

**Given** the Studies dashboard
**When** I create a study for a security and save it
**Then** it persists to the journal (Epic 1 `persistence`) and appears in the dashboard list (FR1, FR54)
**When** I reopen a saved study
**Then** its full state is restored intact — inputs, provenance, validation, judgment, rationale (FR2)
**And** the whole flow works with networking disabled (FR65).

### Story 2.3: Faithful collapsible SSG form (§1–§5)

As Guy,
I want the recognizable high-fidelity SSG form,
So that I am never disoriented and can read the study at a glance.

**Acceptance Criteria:**

**Given** an open study
**When** the study screen renders
**Then** it shows the faithful form: header + capitalization block, the A–H lettered columns with
their formulas, the §3 P/E table, §4 calc rows, on a visible cell grid (neutral labels; no NAIC marks/logos/verbatim prose)
**And** §1–§5 are individually collapsible with an information-scent summary when folded, fold state persisted (FR56)
**And** the two regimes (entry ↔ contemplation) are expressed as fold presets + the colour/marker delta, on constant geometry (no re-layout during interaction).

### Story 2.4: Manual data entry with provenance & coverage

As Guy,
I want spreadsheet-grade manual entry showing each cell's source and coverage,
So that I can complete a study by hand and see its data honesty at a glance.

**Acceptance Criteria:**

**Given** the study grid
**When** I type or **paste a column of years**
**Then** values are parsed locale-aware (decimal comma/thousands), cell-cursor keyboard navigation works, and each edited cell is marked **source = manual** (FR16, FR63 locale)
**And** each cell displays its **source × freshness** and its **coverage state present / to-fill / not-available-accepted**, with missing shouting and stale murmuring per the attention hierarchy (FR17–FR19 display)
**And** `unknown/insufficient` is never shown or stored as 0.

### Story 2.5: Tri-state validation with soft-lock

As Guy,
I want a per-cell review tag I control,
So that my human sign-off is the guard against plausible-but-wrong data.

**Acceptance Criteria:**

**Given** a data cell
**When** I set its review tag
**Then** it cycles none / **? to-review** / **✓ validated**, rendered per the trust-marker spec (confusability-gated) (FR20)
**When** a cell is `✓`
**Then** it is **soft-locked**: editing requires first clearing `✓` (one gesture), which returns it to `?` (never silently blanked)
**And** a study-level (and per-column/row) **"unlock all"** flips `✓→?` behind a confirmation.

### Story 2.6: Numeric judgment inputs, verdict & zone bar (integrity-gated)

As Guy,
I want to set judgment values numerically and read a trustworthy verdict,
So that I reach a defensible buy/hold/sell conclusion even before touching the chart.

**Acceptance Criteria:**

**Given** a study with data
**When** I enter judgment inputs by exact value (future growth, forecast P/E, low-price method)
**Then** the engine (Epic 1) recomputes and the **§4 zone bar** (Buy/Neutral/Sell + present-price marker + price axis), **U/D ratio**, **projected return** and **verdict badge** update (FR6, FR31 exact-value path)
**And** the **sticky verdict bar** shows verdict + present price + projected return + appreciation while scrolling/folding
**And** **verdict integrity** holds: full saturated colour only when every load-bearing input is ✓ & not stale; otherwise provisional (hatched + temporal provenance) / degraded / withheld (FR12)
**And** I can open a **traceability view** of any result — its inputs, their provenance, and the rule that produced it (FR11).

### Story 2.7: Low-confidence & plausibility surfacing

As Guy,
I want thin history and suspicious inputs surfaced honestly,
So that I am never misled by a confident-looking but unsupported verdict.

**Acceptance Criteria:**

**Given** a study with fewer than five usable years
**When** the verdict is shown
**Then** the study carries a visible **"insufficient history / low confidence"** label carried into the verdict (FR8 surfacing)
**Given** an input plausibility issue (unit/split/series break, currency mismatch, fiscal-period misalignment, out-of-bound)
**When** detected
**Then** it surfaces as a neutral inline **warning at the cell**, distinct from quality flags and from the review tag (FR10 surfacing).

### Story 2.8: Interactive growth chart — draggable judgment line, live recolor

As Guy,
I want to drag the judgment line on the semi-log chart and watch the zones recolor live,
So that the judgment moment is direct, fast and reversible — the heart of the product.

**Acceptance Criteria:**

**Given** the §1 semi-log growth chart (Sales/EPS/Price, solid historical / dashed projection, 5–30% guide fan, 1→200 axis) rendered natively in Slint
**When** I drag a judgment trend line (or set it by exact value — kept in sync)
**Then** the estimated future Sales/EPS update, §4 forecast/zones recompute, and the zone bar **recolours within ~100 ms** under my hand (NFR-P1, FR30, FR31)
**And** the chart **never auto-places or suggests** a judgment line (FR33)
**And** if Epic 1's spike B was NO-GO, the agreed Slint fallback rendering is used (never egui/web).

### Story 2.9: Undo/redo & scenario compare

As Guy,
I want reversible judgment exploration,
So that I can try "what if" without losing prior work.

**Acceptance Criteria:**

**Given** any judgment or grid edit
**When** I undo/redo
**Then** state steps back/forward via the snapshot stack; **moving a judgment line never destroys a saved input** (FR32)
**When** I open scenario compare
**Then** I can view an alternate judgment placement and its resulting zones/U-D/return alongside the current one, without committing or losing the prior placement.

### Story 2.10: Decision rationale capture

As Guy,
I want to record *why* I reached a decision,
So that the journal holds my reasoning, not just numbers.

**Acceptance Criteria:**

**Given** a study
**When** I write a decision rationale
**Then** it is stored as a **first-class field** on the study and preserved with the judgment snapshot (FR49, FR51 capture)
**And** it is shown when the study is reopened.

### Story 2.11: Update an existing study & extend its projection

As Guy,
I want to edit and extend a saved study,
So that forging conviction is iterative.

**Acceptance Criteria:**

**Given** a saved study
**When** I correct a data value or change a judgment input
**Then** the engine recomputes, the affected verdict is invalidated/refreshed in the same coherence frame, and edits respect the soft-lock (FR3, FR16)
**When** I extend the projection horizon
**Then** zones recompute and the change is reflected in the study's history.

### Story 2.12: Dashboard search/sort/filter, archive & delete

As Guy,
I want to manage many saved studies,
So that I can find and curate my work.

**Acceptance Criteria:**

**Given** several saved studies
**When** I use the dashboard
**Then** I can list/search/sort/filter and open them (FR54)
**When** I archive or delete a study (with confirmation)
**Then** it is removed/hidden **without corrupting the journal time-series** (FR55).

### Story 2.13: Legend, empty/error states, help, demo & verify-engine

As Guy,
I want guidance without a wizard and a way to trust the engine,
So that I can learn by exploration and verify correctness on demand.

**Acceptance Criteria:**

**Given** any main surface
**When** there is no data or an error
**Then** an **actionable empty state** (e.g. "create your first study" + link to the demo) and clear neutral error/feedback messages are shown (FR58)
**And** a consistent **legend** for freshness/provenance/coverage/confidence states is available (FR57)
**And** a non-blocking **contextual help/glossary** popover and a **read-only demonstration study** are accessible (FR62)
**And** a **"verify engine"** path runs the bundled golden studies (Epic 1) and reports any deviation (FR9 UI).

### Story 2.14: Neutral voice & banned-verb enforcement

As Guy,
I want every system signal to state facts, never advice,
So that the app reinforces me as the sole decider.

**Acceptance Criteria:**

**Given** any system-generated message, label or alert
**When** it is rendered
**Then** it contains **no imperative action/recommendation verb** from the banned-verb list (verifiable test over UI strings) (FR13)
**And** signals are phrased as neutral facts ("the price entered the zone you defined").

## Epic 3: Provider data & reconciliation

The study fills itself in seconds and degrades honestly when the provider fails. All fetched data
flows **through Epic 1's canonical `normalize`** before reaching the engine, so the most dangerous
code path is already hardened. The refresh-driven coherence behaviour **branches onto Epic 1's
manual-mutation rail** (no new state model).

### Story 3.1: `MarketDataProvider` trait & first adapter (EODHD)

As Guy,
I want to auto-fetch a security's data from a provider,
So that I avoid typing ~10 years of fundamentals by hand.

**Acceptance Criteria:**

**Given** a configured provider (EODHD)
**When** I auto-fetch for a ticker
**Then** fundamentals, yearly high/low prices, present price and estimates are retrieved over HTTP
(reqwest rustls-tls + tokio worker), mapped to the provider's raw shape, and passed **through
`normalize` (Epic 1)** into canonical `contract` types stamped **source = provider** with provenance + timestamp (FR15, FR17-18)
**And** the fetch runs off the UI thread and returns via `invoke_from_event_loop` without blocking the UI
**And** per-cell coverage is reported (present / absent / partial); absent cells stay editable by hand.

### Story 3.2: Provider configuration & API keys in the OS keychain

As Guy,
I want to manage provider keys securely and use keyless providers,
So that my credentials never live in the repo or config and I can switch providers.

**Acceptance Criteria:**

**Given** Settings (no wizard)
**When** I add / replace / delete / **test** a provider API key
**Then** the key is stored only in the OS secret store (`keyring`), never in config/logs/exports, and the test reports success/failure (FR25, NFR-S1)
**And** a keyless provider can be used with no key configured (FR25)
**And** the preferred provider is recorded and injected into `ingestion` by the app (key not read inside `ingestion`) (FR63 provider/key).

### Story 3.3: Manual refresh with recompute & freshness

As Guy,
I want a single manual refresh that updates data and recomputes,
So that keeping a study current is one deliberate action.

**Acceptance Criteria:**

**Given** an open study (or the portfolio/watchlist later)
**When** I trigger a manual refresh
**Then** provider data is re-fetched through `normalize`, the engine recomputes deterministically, and the cause of recompute (price / input / FX) is distinguished (FR21, FR29)
**And** each refreshed cell shows its **freshness** (current/stale) and timestamp
**And** the only online action in the whole app is this user-initiated refresh (FR65 preserved).

### Story 3.4: Non-destructive reconciliation

As Guy,
I want refreshes to never overwrite my manual work or my sign-offs,
So that reconciliation is safe and my validations stay meaningful.

**Acceptance Criteria:**

**Given** a refresh returning a value that differs from an existing cell
**When** the cell was **manual**
**Then** the manual value **takes precedence** and the fetched value is **preserved alongside** (non-destructive) (FR22, NFR-R4)
**When** the differing cell was **`✓` validated**
**Then** it is **auto-tagged `?` to-review** and, in the **same coherence frame**, the dependent verdict degrades (the Epic 1 invariant 2b, now driven by refresh)
**And** a manual value and a provider value are never silently merged.

### Story 3.5: Graceful provider failure

As Guy,
I want outages to degrade visibly, never into a wrong signal,
So that I keep working offline and know why a refresh failed.

**Acceptance Criteria:**

**Given** a refresh that fails
**When** the cause is network / quota-rate-limit / invalid-or-absent key
**Then** the cause is recorded and reported via the neutral **global banner**, last-known values are retained, and affected data is flagged **stale / to-update** (never a silent wrong value) (FR23, FR24, NFR-R1)
**And** I can continue offline, override by hand, and retry later.

### Story 3.6: Annual update journey

As Guy,
I want to refresh a saved study against a new annual report,
So that updating an existing study is a quick, safe ritual.

**Acceptance Criteria:**

**Given** a previously saved, validated study
**When** I reopen it and trigger a re-fetch (optionally after "unlock all")
**Then** manual entries and judgment lines are **preserved**, changed cells whose value diverges from a `✓` reset to `?`, and I re-validate only what actually moved (FR3 + FR22 + Journey 2b)
**And** the projection can be extended and the study's history reflects what changed and when.

## Epic 4: Watchlist & single-portfolio risk

One honest picture of risk plus neutral alerts. Risk math comes from Epic 1 (`core`); current prices
come from Epic 3 (manual refresh) — sequence Epic 3 before Epic 4.

### Story 4.1: Watchlist management

As Guy,
I want to maintain a watchlist of securities I'm interested in,
So that I can track candidates toward their buy zone.

**Acceptance Criteria:**

**Given** the Watchlist surface
**When** I add / edit / remove / reorder a watched security
**Then** the change persists and the list reflects it (FR34)
**And** each entry can reference a saved study/snapshot for its zone.

### Story 4.2: Neutral buy-zone alerts

As Guy,
I want a neutral alert when a watched security enters its buy zone,
So that I notice an opportunity I defined, without being told what to do.

**Acceptance Criteria:**

**Given** a watched security with a defined buy zone
**When** a manual refresh moves its price into that zone
**Then** a **neutral in-app alert** is raised — "the price entered the zone you defined" — with no action verb (FR35, FR13)
**And** the alert uses the global banner register (ink + icon + position), not the zone hues.

### Story 4.3: Single-portfolio holdings register

As Guy,
I want to record what I hold,
So that I can see my positions and their risk.

**Acceptance Criteria:**

**Given** a single portfolio in one reference currency
**When** I add/edit/remove a holding (security, quantity, purchase price)
**Then** it persists and is listed (FR36)
**And** the single global reference currency is configurable in Settings (FR63 currency).

### Story 4.4: Manual price refresh & per-holding zones

As Guy,
I want to refresh holding prices and see each zone and freshness,
So that I read my portfolio against my studies on data I refreshed on purpose.

**Acceptance Criteria:**

**Given** holdings linked to studies
**When** I trigger a manual price refresh (via Epic 3)
**Then** each holding's zone recomputes and displays its freshness/timestamp (FR40)
**And** a provider failure degrades to stale flagging, never a silent wrong zone.

### Story 4.5: Trailing stop per holding (ratchet-up only)

As Guy,
I want a trailing stop I set per holding,
So that I define my own capital-protection threshold.

**Acceptance Criteria:**

**Given** a holding
**When** I set a trailing stop (parameter: %/ATR/manual)
**Then** the stop **ratchets up only** and never moves down automatically (FR42)
**And** the stop parameter and risk thresholds are configurable in Settings (FR63 thresholds).

### Story 4.6: Simple capital-at-risk

As Guy,
I want a single capital-at-risk figure for my portfolio,
So that I understand my downside at a glance.

**Acceptance Criteria:**

**Given** holdings with purchase prices and trailing stops
**When** capital-at-risk is computed
**Then** it equals Σ `max(0, (avg_cost − stop)) × qty`, counted only where `stop ≤ avg_cost` (Appendix-A formula, `core` math) (FR43)
**And** it is shown in the sticky verdict/portfolio bar and recomputed on every price refresh (≥ 0 invariant).

### Story 4.7: Neutral sell / stop triggers with manual actions

As Guy,
I want neutral triggers that offer actions but never act for me,
So that I stay the sole decider, with the stop taking priority over the Sell zone.

**Acceptance Criteria:**

**Given** a holding that breaches its stop or enters its Sell zone
**When** the trigger fires
**Then** it surfaces a **neutral fact** and offers manual actions (sell / raise stop / dismiss), never auto-acting (FR46)
**And** when both conditions conflict, the **stop-loss takes priority over the Sell zone** as an isolated, testable business rule (FR47)
**And** a chosen sell is recorded with an optional rationale.

## Epic 5: Cumulative memory & portability

The journal becomes an appreciating, portable, safe asset.

### Story 5.1: Reopen & confront a past judgment vs reality

As Guy,
I want to overlay a past study's projection on what actually happened,
So that I learn from my own past judgments.

**Acceptance Criteria:**

**Given** a saved study and a post-decision price-history cache (sourced via Epic 3 refresh, stored in `persistence`)
**When** I reopen the study in "confront" mode
**Then** its **recorded projection is overlaid on the security's actual trajectory since** the decision (FR50, ADD13)
**And** the historical snapshot is unchanged by the comparison (read-only).

### Story 5.2: Export / import a single study

As Guy,
I want to export and import one study as a portable file,
So that I can seed, share or archive a study and round-trip it safely.

**Acceptance Criteria:**

**Given** a study
**When** I export it
**Then** the file is the **serialized data contract (JSON) + `schema_version` + integrity hash** (not a raw .db)
**When** I import it
**Then** identity is preserved on round-trip, and a version/integrity mismatch is rejected or migrated with a clear message (FR59, NFR-R5).

### Story 5.3: Export / import the whole journal

As Guy,
I want to export and import my entire journal,
So that I can move or seed all my work at once.

**Acceptance Criteria:**

**Given** a journal
**When** I export it
**Then** it is written in a versioned format carrying `(journal_id, version, hash)`
**When** I import it
**Then** it is validated on import and rejected/migrated on version mismatch, never partially applied (FR60, NFR-R5).

### Story 5.4: Restore from backup

As Guy,
I want to restore from a backup safely,
So that I never overwrite good data with an incompatible or corrupt file.

**Acceptance Criteria:**

**Given** a backup file
**When** I restore
**Then** integrity and schema-version checks run **before** any overwrite, the journal_id/version are shown, and a stale or mismatched backup is surfaced (e.g. "you saw v57, this is v41") and never applied silently (FR61).

### Story 5.5: Journal location, recent journals & sync-safety

As Guy,
I want to choose where my journal lives and reopen the last one,
So that I control my data location and benefit from NAS backup without corruption.

**Acceptance Criteria:**

**Given** the File menu / Settings
**When** I create / open / switch a journal or pick its directory
**Then** the app remembers recent journals and **reopens the last-used journal on launch** (pointer = `(journal_id, last-seen-version)`, stored in app-config via `directories`, not in the journal) (added DB-location requirement, FR66)
**And** a **single-instance lock** prevents opening the same journal twice
**When** the chosen directory is a detected sync folder (Synology/Dropbox/OneDrive/iCloud)
**Then** the app **warns** and uses `journal_mode=DELETE/TRUNCATE`; the recommended pattern (live DB local + versioned backups to the sync folder) is offered (ADD8).

### Story 5.6: PDF export of a study

As Guy,
I want a faithful PDF of a study,
So that I can archive or print my conviction.

**Acceptance Criteria:**

**Given** a study
**When** I export to PDF (via the `report` crate, from `core`/`contract` — UI-independent)
**Then** the PDF reproduces the faithful form layout with **neutral labels, no NAIC marks/logos/verbatim text**, all sections expanded, and is **readable in pure greyscale** (FR52, NFR-U3).

## Epic 6 [P2]: Multi-portfolio, multi-currency & full risk overlay

> Story-level detail deferred to Phase 2 (requirements will be refined then). Outline only:

- Story 6.1: Multiple portfolios, one per bank/account (FR37).
- Story 6.2: Multi-currency holdings (FR38).
- Story 6.3: Buy/sell transaction ledger with partial sells + weighted-average cost basis (FR39).
- Story 6.4: Dividends — gross in study, net reinvestable per withholding rule (FR41).
- Story 6.5: FX acquisition, dated/source-aware, applied only at consolidation (FR28).
- Story 6.6: Capital-at-risk per currency → per bank → global total (FR44).
- Story 6.7: Concentration check on total invested capital (FR45) + configurable diversify-by-size table.
- Story 6.8: Replacement-candidate surfacing on a sell, with re-concentration flags (FR48).
- Story 6.9: Provider fallback chain per field type + rate-limit batching (FR26, FR27).

## Epic 7 [P3]: Comparison, health review, screening & reports

> Story-level detail deferred to Phase 3. Outline only:

- Story 7.1: Company Comparison (side-by-side ~30 metrics) + faithful export (FR53).
- Story 7.2: Portfolio Health Review (diversification/quality roll-up) + faithful export (FR53).
- Story 7.3: Discovery/screening + Quick Screen / Starter Checklist.
- Story 7.4: Additional provider adapters.
- Story 7.5: PDF/print of the other forms (FR53).

## Epic 8 [V]: Read-only AI clerk-of-memory & MCP façade

> Story-level detail deferred to the Vision phase. Outline only:

- Story 8.1: Read-only MCP façade over the versioned data contract (capability asymmetry by construction).
- Story 8.2: Local-first read-only AI provider abstraction (pluggable; optional remote later).
- Story 8.3: Past-interrogation prompts (coherence checks, discipline-drift, pre-mortems) producing `source:ai-suggested` drafts requiring human validation (FR14).
- Story 8.4: Capability-asymmetry tests — any AI write to studies/judgments/verdicts/transactions is rejected and logged (FR14).

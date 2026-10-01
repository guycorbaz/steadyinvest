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

> Phase tags: **[P1]** MVP · **[P2]** Portfolio depth · **[P3]** Growth · **[P4]** AI assistance (MCP) · **[V]** Vision.

## Requirements Inventory

### Functional Requirements

> Reconciled 2026-10-01 (H3): FR20, FR33 and FR68 below are restated to match `main`; the PRD also
> qualifies FR3, FR5, FR10, FR12, FR23, FR28, FR41, FR50, FR60, FR62, FR63 and FR67 and carries the
> « Décision en attente (Guy) » notes — the PRD is the reference (réconcilié 2026-10-01 : revue
> projet p1/p2).

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
- FR13 [P1]: All app-generated signals are neutral — no app output contains an action/recommendation verb from the banned-verb list (verifiable). AI-origin text [P4] is third-party content: always shown inside a frame labelled "AI" with the disclaimer, outside the banned-verb gate, never restyled as an app signal.
- FR14 [P4]: The MCP surface is verifiably draft-only — any write outside the draft inbox (studies, cells, judgments, verdicts, notes, transactions, portfolio) is rejected and logged; drafts take effect only on the owner's validation in the UI.

**Data Acquisition, Provenance & Providers**
- FR15 [P1]: The user can auto-fetch a security's fundamentals, prices and estimates from a configured provider. A fetch is user-initiated only, never through MCP.
- FR16 [P1]: The user can enter, override and later correct any data field by hand.
- FR17 [P1]: Each data cell carries an independently queryable source (provider/manual/derived). [P4]: a validated AI draft is an owner entry — source manual, reconciled as manual (FR22), review tag `?` (FR74) — and shows its AI origin (client, model, validation date) until the owner next edits the value; the AI origin is not a separate source.
- FR18 [P1]: Each data cell carries an independently queryable provenance and timestamp.
- FR19 [P1]: Per-cell coverage is represented as present / to-fill / not-available-accepted.
- FR20 [P1]: Each cell (and the study as a whole) carries a tri-state review tag — `none` / `? to-review` / `✓ validated` — with a soft-lock: a `✓` cell must be explicitly un-validated (→ `?`) before its value can be edited, and a refresh that diverges from a validated value keeps the `✓` and the value and parks the provider value beside it as pending, for the owner to accept or keep (réconcilié 2026-10-01 : issue #110 option b — PR #123; was « auto-tags it `✓→?` »). (Supersedes the original binary auto-reset wording; see GitHub issue #1.) A validated AI value [P4] enters with review tag `?`, also on a `✓` cell without prior un-validation (FR74).
- FR21 [P1]: The user can trigger a manual refresh of provider data; a refresh is user-initiated only, never through MCP.
- FR22 [P1]: On refresh, a manual value takes precedence over a fetched value while the fetched value is preserved (non-destructive reconciliation). A validated AI value [P4] reconciles as a manual value.
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
- FR33 [P1]: The system never sets a judgment for the owner: an untouched judgment line starts from a dimmed derived seed (least-squares EPS line on §1, historical average P/E on §3) and a one-click « hist. » value to adopt; a seed never feeds the evaluation, the verdict or an alert until the owner sets or adopts the value (réconcilié 2026-10-01 : issue #121, PR #137; was « never auto-places or suggests a judgment line »). [P4]: an AI-proposed judgment (FR72) is shown on the chart as a line annotated "AI" beside the owner's; while pending it changes no verdict, zone or alert. Once validated it becomes the study's judgment and keeps the annotation "placed by AI" with its validation date until the owner next moves or edits it.

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
- FR49 [P1]: The user can capture a decision rationale as a first-class field on studies and transactions. [P4]: study notes (FR78) and processed AI drafts are preserved in the study history.
- FR50 [P1]: The user can reopen a past study and visually compare its recorded projection to the security's actual trajectory since.
- FR51 [P1]: The system durably preserves the time-series of judgments, provenance, validation and rationale. [P4]: it also preserves study notes (FR78) and processed AI drafts.

**Reporting & Printing**
- FR52 [P1]: The user can print / export to PDF a Stock Study in a layout close to the original form, neutral labels, no NAIC marks/logos or verbatim text.
- FR53 [P2/P3]: The user can print / export the other forms (Company Comparison, Portfolio) in the same faithful-but-neutral layout.

**Application Shell & Data Management**
- FR54 [P1]: The user can list, search, sort and filter saved studies and open them from a home dashboard.
- FR55 [P1]: The user can delete or archive a study (with confirmation); deletions never corrupt the journal time-series. [P4]: deleting a study also deletes its AI drafts, as it does its judgment history.
- FR56 [P1]: The user can switch a study between an entry regime (dense editing) and a contemplation regime (reading/judgment), with the active regime clearly indicated.
- FR57 [P1]: The user can view a consistent legend for freshness/provenance/coverage/confidence states.
- FR58 [P1]: Every main surface presents an actionable empty state and clear neutral error/feedback messages.
- FR59 [P1]: The user can export/import a single study to a portable versioned file (round-trip preserves identity).
- FR60 [P1]: The user can export/import the whole journal in a versioned format, validated on import (reject/migrate on version mismatch); the export and backups include AI drafts [P4] (pending, validated, validated then undone, rejected).
- FR61 [P1]: The user can restore from a backup with integrity and version-compatibility checks before overwrite.
- FR62 [P1]: The user can access non-blocking contextual help / glossary and a read-only demonstration study.

**Configuration, Posture & Operation**
- FR63 [P1]: The user can configure providers/keys, the single global reference currency, risk thresholds, the label set (NAIC↔neutral) and locale number format — without a blocking setup flow.
- FR64 [P1]: A disclaimer (educational, not a financial advisor) is always visible — including in the draft inbox and beside every AI-origin item [P4]; the app's own outputs never issue recommendations, and AI proposals are always labelled as such.
- FR65 [P1]: The user can run the full study and portfolio-risk workflow offline; the only online action is a user-initiated refresh. AI assistance [P4] is optional; no workflow requires it.
- FR66 [P1]: The journal is kept in a portable local store an external system (e.g. file sync) can back up.
- FR67 [P1]: The user can choose the journal directory; the app remembers recent journals and reopens the last-used journal on launch. The pointer `(journal_id, last-seen-version)` lives in per-machine app-config (via `directories`), never inside the journal. A single-instance lock guards the open journal. The app detects a sync folder (Synology/Dropbox/OneDrive/iCloud) and warns, keeping the live DB local with versioned backups to the sync folder (SQLite `journal_mode=DELETE/TRUNCATE`). (New requirement; arch ADD7/ADD8; GitHub issue #2.) [P4]: draft writes arriving through MCP never corrupt or race the open session.
- FR68 [P4]: The decision-time verdict is frozen and immutable — stamped with `method_version`, the `inputs_hash`, the exact load-bearing inputs and the date (a study verdict involves no FX) — and is the only verdict persisted. The current verdict is always computed live and shown, never persisted; whenever it differs from the frozen one (facts, `inputs_hash` or `method_version`) the study shows both, « figé (<method_version>, <date>) » and « actuel (<method_version>, aujourd'hui | provisoire | retenu) » (réconcilié 2026-10-01 : D11 / arch A13 / story 8.8 decision 1; was [P1], « recomputed on demand, never automatic, compare on a `method_version` change »). (New requirement; arch ADD10; GitHub issue #3.) Decision time (owner, 2026-09-27): the verdict is frozen when the owner validates the study with an explicit action, available only when the verdict is full (every load-bearing input `✓`); validating again later replaces the frozen verdict, the previous one stays in the study history (FR51). Whenever the current verdict later differs from the frozen one — after a refresh, an owner edit or a method change — the difference is highlighted, naming what changed and why where known (FR29), neutrally. [P4]: no verdict is frozen or changed through MCP; the MCP read returns the frozen verdict beside the current one. (Delivered in Epic 8, Story 8.8.)

**AI Assistance (MCP) [P4]**
- FR69 [P4]: An AI client can read, through MCP, the dossier's studies — data cells, provenance, judgments, study rationale, notes, judgment history and computed outputs (zones, upside/downside ratio, 5-year potential, verdict and its state) — and the record of drafts (FR77); the portfolio (holdings, transactions, dividends), the watchlist, keys and configuration are never exposed. The MCP server serves the last-used dossier, also while the app is closed, and every response names the dossier (identity and location) it read.
- FR70 [P4]: An AI client can submit a draft study (security identifier, proposed native currency, optional company name, mandatory comment) to the draft inbox; it creates no study until validated, and is refused if the dossier already holds a study, or a pending draft study, for the same security in the same currency (identifier compared case-insensitively). Validating it opens the ordinary create-study dialog prefilled with the proposal, which the owner confirms; the duplicate check runs again at that moment.
- FR71 [P4]: An AI client can submit a draft note on an existing study, with a mandatory comment.
- FR72 [P4]: An AI client can submit a draft cell value, judgment values included, with a mandatory comment. While pending it changes no value, line, zone, alert or verdict; a judgment draft is shown on the chart as an AI-annotated line beside the owner's (FR33). A target holds at most one pending draft: a second one is refused. Market facts written by the provider (current price, TTM EPS) are not draftable. A pending draft whose target changed meanwhile (owner edit or refresh) is marked stale; the owner can still validate it after an explicit confirmation, or reject it; a draft whose target no longer exists cannot be validated.
- FR73 [P4]: The owner can review pending drafts in a dossier-level inbox, with a reminder in each concerned study, showing for each draft its AI origin (client + model), comment, target, and current vs proposed value side by side. Drafts submitted while the app is closed appear at its next opening.
- FR74 [P4]: The owner can validate or reject each draft individually (no bulk action), in ≤ 2 actions, with current and proposed values side by side. Validation applies the draft as an owner entry (FR17) with review tag `?` in every case — also on an untagged cell or when the value is unchanged — and visible AI origin; on a validated (`✓`) cell it needs no prior un-validation and moves the cell to `?`; a stale draft needs an explicit confirmation (FR72). A validation is undoable like any owner edit (FR32), and an undone validation is recorded as such (FR77); a validated draft study is reversed by deleting the study it created (FR55). Editing a draft before validation makes it the owner's own entry; validating a draft study does not add it to the watchlist.
- FR75 [P4] (scope note): Search objectives (market, potential growth, upside/downside ratio…) are given to the AI in its client session; the dossier does not store them in Phase 4.
- FR76 [P4]: Provider data is fetched only on the owner's action: after validating a draft study the owner fetches it as for any study, and the fetched data then becomes readable through MCP; no MCP request can trigger a provider call.
- FR77 [P4]: The system keeps a durable record of every draft (origin, comment, content, timestamps, outcome — pending, validated, validated then undone, rejected — and, at decision time, whether it was stale or edited before validation) that the owner can view and the AI can read; the drafts of a deleted study are deleted with it (FR55).
- FR78 [P4]: The owner can create, edit and delete notes attached to a study; a deleted note leaves the study but remains in its history (FR51).

### NonFunctional Requirements

**Correctness & Calculation Integrity (top priority)**
- NFR-C1: The calculation engine is deterministic — identical inputs always produce identical outputs, bit-stable across runs and platforms.
- NFR-C2: Engine output matches every bundled golden reference study (exact zoning/verdict; within ±0.5% on derived numerics — a fixed method default; the PRD marks "tolerance configurable" as superseded).
- NFR-C3: Property-based invariants hold (zones ordered low<buy<hold<sell<high; U/D ≥ 0; capital-at-risk ≥ 0; FX round-trip A→B→A within 1e-6).
- NFR-C4: FX is applied only at consolidation; per-currency study results are independent of the chosen reference currency.
- NFR-C5: Engine + risk crate are gated in CI by golden-fixture and property tests (≥95% coverage of calc paths); a failing test blocks merge.

**Performance**
- NFR-P1: Judgment-line recalculation and zone re-render feel live — within ~100 ms perceived while dragging.
- NFR-P2: Opening or recomputing a full study completes within ~1 s on typical hardware.
- NFR-P3: A manual portfolio refresh (tens of holdings) completes within a few seconds and never blocks the UI.
- NFR-P4: The app reaches an interactive state within ~3 s of launch.

**Security & Privacy**
- NFR-S1: Provider API keys live only in the OS secret store — never in the repo, plaintext config, logs, exports, backups or MCP responses.
- NFR-S2: No telemetry/analytics; the app's only network calls are user-initiated provider/FX fetches. The MCP server is not network-exposed in the current setup.
- NFR-S3: All persistent data is local. Beyond the chosen provider (under the user's own key), study data may leave the machine only through the AI client the owner chooses; portfolio data never leaves it through MCP. Accepted residue: free text (study rationale, notes) may mention positions and so partially reveal the portfolio to the AI.
- NFR-S4: The MCP surface never returns the portfolio (holdings, transactions, dividends), the watchlist, keys or configuration — verified by tests over every MCP resource and tool. The dossier's identity (its `journal_id` and path), which every response names (FR69), is not configuration.

**Reliability & Data Integrity**
- NFR-R1: The full study + portfolio-risk workflow runs offline; losing the network degrades only fetching, with stale flagging — never a silent wrong value.
- NFR-R2: Writes are crash-safe/atomic — an interrupted operation never corrupts the journal. Draft writes through MCP [P4] are atomic and never corrupt or race the open session.
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

**AI Capability Asymmetry [P4]**
- NFR-A1: Capability asymmetry holds by construction, not by prompt: the only write the MCP surface offers is draft creation; every other write is rejected and logged (CI suite).
- NFR-A2: Portfolio non-exposure holds by construction: the portfolio (holdings, transactions, dividends), the watchlist, keys and configuration are absent from the MCP surface (whole-surface test).
- NFR-A3: No provider call is reachable from MCP (tested).
- NFR-A4: Every draft carries its origin (client + model), a timestamp and a non-empty comment (100% of drafts; tested).

### Additional Requirements

*(From the Architecture decision document — technical/infra requirements that shape epics & stories.)*

- ADD1 [P1] **Starter / scaffold (→ Epic 1, Story 1):** initialize a Cargo workspace with 6 crates
  (`core`, `contract`, `ingestion`, `persistence`, `report`, `app`); seed the `app` UI crate from the
  official Slint Rust template (`cargo generate --git https://github.com/slint-ui/slint-rust-template`).
  Pinned deps: slint 1.16, rusqlite 0.40 (bundled), rust_decimal 1.42 (+maths), reqwest 0.13
  (rustls-tls,json) + tokio 1.52, serde 1, thiserror 2.0, proptest 1.9, tracing, keyring 3.x (NOT 4.0 — see issue #5; added in Story 3.2), directories.
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

**AI assistance [P4]**
- UX-DR30 [P4]: AI-assistance surfaces — draft inbox and per-study reminder, AI frame (label + disclaimer), AI-annotated chart line and "placed by AI" annotation, AI-origin cell mark (confusability-gated with UX-DR15), study notes, drafts record — and their French wording, specified by the Story 8.0 UX addendum (`ux-ai-assistance-surfaces.md`).

### FR Coverage Map

*(Every FR mapped to an epic. FR13 neutrality and FR63 no-wizard settings are cross-cutting, built incrementally; their primary home is noted.)*

- FR1 → Epic 2 (create study) · FR2 → Epic 2 (persist/reopen) · FR3 → Epic 2 (edit/extend)
- FR4 → Epic 1 (SSG output set) · FR5 → Epic 1 (native currency) · FR6 → Epic 2 (judgment inputs; compute in Epic 1)
- FR7 → Epic 1 (quality flags) · FR8 → Epic 1 (5-yr floor/low-confidence) · FR9 → Epic 1 (CI golden gate) + Epic 2 (verify-engine UI)
- FR10 → Epic 1 (plausibility) · FR11 → Epic 1 (traceability data) + Epic 2 (view) · FR12 → Epic 2 (verdict degraded/withheld)
- FR13 → Epic 2 (neutral signals; cross-cutting) + Epic 8 [P4] (AI-text frame) · FR14 → Epic 8 [P4] (MCP draft-only)
- FR15 → Epic 3 (auto-fetch) · FR16 → Epic 2 (manual entry/override)
- FR17/FR18/FR19 → Epic 1 (per-cell source/provenance/coverage model) + Epic 2 (display)
- FR20 → Epic 2 (tri-state validated + soft-lock) · FR21–FR25 → Epic 3 (refresh, reconciliation, failure, keys)
- FR26/FR27/FR28 → Epic 6 [P2] (fallback chain, rate-limit batching, FX)
- FR29 → Epic 1 (recompute on input/judgment change) + Epic 3 (recompute on refresh/price/FX, cause-distinguished)
- FR30/FR31/FR32/FR33 → Epic 2 (charts, draggable judgment line, undo, never auto-suggest) · FR33 [P4] → Epic 8 (AI-annotated pending line)
- FR34/FR35 → Epic 4 (watchlist + buy-zone alert) · FR36/FR40/FR42/FR43/FR46/FR47 → Epic 4 (single portfolio, refresh, trailing stop, capital-at-risk, neutral triggers, stop-priority)
- FR37/FR38/FR39/FR41/FR44/FR45/FR48 → Epic 6 [P2] (multi-portfolio, multi-currency, ledger, dividends, consolidation, concentration, replacement)
- FR49 → Epic 2 (decision rationale) · FR50 → Epic 5 (reopen & confront vs actual) · FR51 → Epic 1 (durable time-series storage) + Epic 2 (capture)
- FR52 → Epic 5 (PDF export of study) · FR53 → Epic 7 [P3] (other forms)
- FR54/FR55/FR56/FR57/FR58/FR62 → Epic 2 (dashboard, archive, regimes, legend, empty/error states, help/demo)
- FR59/FR60/FR61 → Epic 5 (export/import study + journal, restore with integrity)
- FR63 → Epic 2 (labels/locale) + Epic 3 (provider/key) + Epic 4 (currency/thresholds) + Epic 5 (DB location) — incremental, no-wizard
- FR64 → Epic 2 (always-visible disclaimer) + Epic 8 [P4] (inbox & AI-item disclaimer) · FR65 → Epic 1/Epic 2 (offline operation) · FR66 → Epic 1 (portable store) + Epic 5 (backup/restore)
- FR67 → Epic 5 (journal directory, recent journals, single-instance lock, sync-folder warning) · FR68 → Epic 8, Story 8.8 (frozen decision-time verdict on « Valider l'étude », frozen vs current highlighted — owner decision D11, 2026-09-27)
- FR15/FR17/FR20/FR21/FR22/FR49/FR51/FR55/FR60/FR65/FR67/FR68 [P4 additions] → Epic 8 (owner-only fetch/refresh, AI origin on validated values, `?` on validation, manual-rule reconciliation, notes & drafts in history, draft cascade on delete, drafts in export/backup, AI optional, MCP never races the session, no verdict changed through MCP)
- FR69–FR78 → Epic 8 [P4] (MCP read, draft studies/notes/cells, inbox, one-by-one decision, owner-only fetch, draft record, study notes)
- NFR-A1–A4, NFR-S1–S4, NFR-R2 [P4 additions] → Epic 8 [P4] (asymmetry, non-exposure, no provider call, draft origin, keys never in MCP responses, no network exposure, atomic draft writes — CI suites)

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

### Epic 8 [P4]: AI assistance over MCP (human-gated drafts)
No AI runs inside the app. A separate local **MCP server** (`steadyinvest-mcp`, stdio, launched by the
owner's AI client — Claude Code) lets an AI read the dossier's **studies** (data, provenance,
judgments, rationale, notes, judgment history, computed outputs) — **never** the portfolio, the
watchlist, keys or configuration — and **propose** new studies, notes and cell/judgment values
**only as commented drafts**. Drafts wait in a dossier-level inbox and take effect only when the
owner validates them one by one in the UI; a pending draft changes no output. Capability asymmetry
and non-exposure are enforced **by the SQLite engine and the crate dependency closure**, not by
prompt. A UX pass (8.0) and study notes (8.1, AI-free) land first. The epic also delivers the
frozen decision-time verdict (FR68, Story 8.8), which the MCP read exposes beside the current one. The app's own outputs stay neutral; AI
text is always framed and labelled.
**FRs covered:** FR13 (AI-text framing), FR14, FR17 (AI origin), FR33 (AI line), FR64 (inbox
disclaimer), FR68 (frozen decision-time verdict, Story 8.8), FR69–FR78 (+ P4 additions to FR15, FR20, FR21,
FR22, FR49, FR51, FR55, FR60, FR65, FR67); NFR-S1–S4, NFR-R2, NFR-A1–A4.

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
reqwest 0.13, tokio 1.52, serde 1, thiserror 2.0, proptest 1.9, tracing, keyring 3.x [deferred to Story 3.2, NOT 4.0 — issue #5], directories)
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
**And** the chart **never auto-places or suggests** a judgment line (FR33) — as delivered, an untouched line starts from a dimmed derived seed that never feeds §4 or the verdict until the owner sets it (réconcilié 2026-10-01 : issue #121, PR #137)
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
**And** a non-blocking **contextual help/glossary** popover and a **read-only demonstration study** are accessible (FR62) — as delivered, the help is one glossary and legend hub in Réglages (story 2.13 scope decision 2), and the demo is offered on the empty list and beside « Créer une étude… » once studies exist, every writing gesture disabled (réconcilié 2026-10-01 : PR #279, PR #284)
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
**Then** manual entries and judgment lines are **preserved**, a `✓` cell whose provider value diverges keeps its value and `✓`, the provider value parked beside it for me to accept or keep (réconcilié 2026-10-01 : issue #110 option b — PR #123; was « reset to `?` »), and I re-validate only what actually moved (FR3 + FR22 + Journey 2b)
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
**Then** its **recorded projection is overlaid on the security's actual trajectory since** the decision (FR50, ADD13) — the recorded projection is the frozen verdict's band dated its validation; without a frozen verdict, today's band, said as such and dated the creation (réconcilié 2026-10-01 : PR #280). Décision en attente (Guy) : une re-validation déplace la date de décision.
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

## Epic 8 [P4]: AI assistance over MCP (human-gated drafts)

No AI runs inside the app. A separate stdio MCP server lets the owner's AI client read the dossier's
studies and propose drafts; nothing enters the dossier without the owner's hand. Asymmetry and
portfolio non-exposure hold **by construction** (SQLite authorizer + crate dependency closure),
proven by CI suites. Order matters: the UX pass (8.0) comes first; notes (8.1, no AI) and the
headless data model and access surface (8.2a–8.3) precede the binary (8.4) and the UI (8.5a–8.7).

> **Dev-safety rule for every Epic 8 story (arch A12):** Claude Code is both the developer and the
> MCP client. Every dev, test and CI run of `steadyinvest-mcp` passes `--dossier <temp path>` on a
> throw-away copy, with `XDG_CONFIG_HOME` / `XDG_DATA_HOME` pointed at a temporary directory; no
> story registers the MCP server on the real dossier — that registration is the owner's act, done by
> hand from the registration doc (Story 8.4). No story drives a provider fetch headless without the
> owner's say-so.

> **Posture AC for every UI story (8.0 excepted), as in Epic 7:** the `@tr` floor and `MSG_*` counts
> are re-based and their delta stated (review checklist §6); every new string is French, from the
> 8.0 wording list; new terms get a glossary entry; no new app output uses a banned verb (FR13); IO
> failures render « indisponible » with their cause, never an empty-looking surface (checklist §1); a
> refused action opens « Action refusée » (7.0 AC1); visual verification (DoD) on a temp dossier
> copy — from 8.5a on, seeded with `just mcp-seed` (Story 8.4).

### Story 8.0: UX pass — AI-assistance surfaces

As Guy,
I want every Epic 8 surface designed and worded before it is built,
So that the inbox, the AI marks and the notes fit the app as the 7.0 pass made Epic 7 fit.

**Acceptance Criteria:**

**Given** the PRD (FR13, FR17, FR33, FR64, FR69–FR78) and architecture §Phase 4
**When** the UX addendum is written and validated by the owner in its own PR (like PR #206)
**Then** it specifies, with layouts and states (empty, loading, « indisponible », refused):
- the **draft inbox**: where it lives (nav-rail entry and/or badge, pending count), its list, and the side-by-side current vs proposed view;
- the **reminder** in a study with pending drafts, and where a pending draft *study* is signalled;
- the **AI frame**: label, disclaimer, layout; how a pending judgment draft that is not a chart line shows on the study;
- the **AI judgment line** on the chart: style, label, non-colour cue (NFR-U1), and the "placed by AI" annotation after validation;
- the **AI-origin mark** on a validated cell (visible origin, cleared by the next owner edit) and its confusability-gate entry (UX-DR15);
- **study notes**: place relative to the rationale (`RationaleNote`), entry through a titled dialog (7.0 AC1), deletion confirmation;
- the **drafts record** view and its filters, and how processed drafts appear in a study's history;
- the **« Valider l'étude »** action (Story 8.8): where it sits, its disabled state with the reason when the verdict is not full, and how the frozen and current verdicts are shown side by side when they differ (labels, highlighted items, cause), on screen and in the study PDF;
- keyboard operation of every one of these surfaces (NFR-U2);
**And** it fixes the French wording (« IA » or « AI », « brouillon » or « proposition », « périmé », « validé puis annulé »…) as a list the UI stories copy verbatim, and the language of MCP refusal reasons
**And** it decides the report impact: whether the study PDF (5.6) and the comparison (7.1) show notes, AI-origin marks and the "placed by AI" annotation — "unchanged, by decision" is an acceptable answer, but it is written down
**And** `ux-design-specification.md` marks the superseded "margin voice" passages and qualifies "no suggested line" with the [P4] framed, inert AI line; UX-DR30 in `epics.md` points to the addendum.

### Story 8.1: Study notes

As Guy,
I want to attach free notes to a study and create, edit or delete them,
So that my thinking around a study lives beside it (and an AI note draft later has a home).

**Acceptance Criteria:**

**Given** an open study
**When** I add, edit or delete a note, through the dialog specified in 8.0
**Then** the note is stored inside the study blob as `notes: Vec<Note{id, text, created_at, updated_at, ai_origin: Option<AiOrigin>}>` with `#[serde(default)]` — additive, no `user_version` migration; a study saved before this story opens unchanged with an empty note list (FR78, NFR-R3, arch A5)
**And** this story defines the contract type `AiOrigin{draft_id, client, model, validated_at}` (arch A6), used by 8.2b
**And** each note change is saved through the normal study upsert and appears in the study's history; a deleted note disappears from the study but remains readable in the study history (FR49, FR51, owner decision O6)
**And** note-only history entries — identified by comparing consecutive snapshots with `notes` ignored (history rows carry no cause column) — are labelled as such and can be filtered out of the history view, so judgment changes stay readable (arch A12)
**And** no `SCHEMA_VERSION` bump or compatibility work: `notes` is an additive `#[serde(default)]` field, and a dossier saved before this story opens with empty note lists — the app is not in production (owner, 2026-09-27; D9 withdrawn)
**And** a single-study and a whole-dossier export/import round-trip preserves notes byte-identically, including their ids and timestamps (FR59, FR60, NFR-R5)
**And** deleting a note asks for confirmation (UX-DR25) and is undoable within the session; notes are keyboard-operable (NFR-U2)
**And** this story contains **no AI**: no MCP code, no draft, no AI label; `ai_origin` exists in the type but is always `None` here
**And** a note text is plain owner text, not subject to the banned-verb gate (FR13 covers app-generated signals only)
**And** the report impact decided in 8.0 is implemented (or the PDF is asserted unchanged, by decision).

### Story 8.2a: Drafts table (headless)

As the developer,
I want the drafts table defined once, with every variant and decision fact it will ever need,
So that every later story writes and reads drafts through one proven, versioned model.

**Acceptance Criteria:**

**Given** a dossier at the current schema
**When** migration v8 runs
**Then** a table `ai_drafts` exists in the **same** SQLite file with the columns of arch A4: `id`, `kind` (`study|note|cell|judgment`), `study_id` (NULL for a draft study), `security_ticker`, `native_currency`, `status` (`pending|validated|validated_undone|rejected`), `created_at`, `decided_at`, `comment`, `origin_client`, `origin_model`, `stale_at_decision`, `edited_before_validation`, `created_study_id`, and a versioned JSON `payload` (`DraftTarget {Cell{fiscal_year, field} | Judgment{field}}`, proposed value or note text, optional company name, `base_fingerprint`) (FR70–FR72, FR77, arch A4)
**And** every enum variant (`kind`, `status`, `DraftTarget`) is defined now — a later variant would cost a `SCHEMA_VERSION` bump
**And** the database refuses a draft with an empty comment or a missing origin (`NOT NULL` + `CHECK(length(trim(comment)) > 0)`), tested by direct inserts (NFR-A4)
**And** the migration creates the `AFTER INSERT ON ai_drafts` trigger that bumps `logical_version` (arch A3)
**And** a fixture dossier at v7 migrates to v8 with no data loss, and the frozen migration corpus gains `v8.db` (NFR-R3, NFR-M2)
**Given** a dossier with drafts in every status
**When** it is backed up with `VACUUM INTO`, or exported and imported
**Then** the backup carries `ai_drafts` as-is, and the JSON export carries an additive `ai_drafts` array; the round-trip preserves every draft and every column (FR60, FR61, NFR-R5)
**When** a study is deleted
**Then** the drafts whose `study_id` or `created_study_id` is that study are deleted in the same transaction, like its judgment history; a pending draft *study* is untouched (FR55, owner decision O7)
**And** a test asserts that no computation path — `core`, `report::form::build_snapshot`, zone and verdict derivation — reads `ai_drafts` (the metamorphic suite follows in 8.3); the inbox, reminder and record view models of 8.5a–8.7 read it by design.

### Story 8.2b: AI origin, staleness and decisions (headless)

As the developer,
I want validated drafts recorded as owner entries with a visible AI origin, and decisions applied atomically through the app state,
So that a decision can never be half-applied, lost by a later save, or confused with a provider value.

**Acceptance Criteria:**

**Given** the contract types
**When** they are defined
**Then** `DraftOrigin{client, model}` (a draft's submitter) is added, distinct from `AiOrigin{draft_id, client, model, validated_at}` (a validated value's origin, defined in 8.1); `Provenance` gains `#[serde(default)] ai_origin: Option<AiOrigin>` — **no new `Source` variant** — and `Judgment` gains one additive sidecar `ai_placed: AiPlaced` holding an `Option<AiOrigin>` slot per draftable judgment field (nine slots); `Note.ai_origin` is an `Option<AiOrigin>` (arch A6)
**And** the draftable fields are enumerated in one place: the study-grid cell fields, and the judgment fields except `current_price` and `ttm_eps` (owner decision D6), with each field's unit (percent fields as percent) and, for enum fields such as `forecast_low_option`, their variant names
**When** a validated draft is applied
**Then** it is an owner entry: `Source::Manual`, review tag `?` **set explicitly** in every case — also on an untagged cell, on a `✓` cell, and when the value is unchanged (owner decisions O5, D5) — reconciled exactly as a manual value (manual wins, provider preserved), covered by the reconciliation tests extended with an AI-origin case (FR17, FR20, FR22, FR74, NFR-R4)
**And** the next owner edit of that cell clears `ai_origin`; **any** write that changes a judgment field clears its `ai_placed` slot — a value-identical write stays a no-op (arch A6; Story 8.2b G3, owner-pending)
**Given** a pending cell or judgment draft
**When** `contract::draft_fingerprint(study, target)` no longer matches `base_fingerprint`
**Then** the draft reads as **stale** (computed on read); the fingerprint uses the explicit encoding of arch A7 — cell: normalised value, source, pending provider value (not timestamp or digest); judgment: field value + load-bearing inputs + `METHOD_VERSION` — and tests show that a refresh of the EPS history or a method change marks a judgment draft stale, a parked divergent provider value marks a cell draft stale, and a value-identical re-stamp does not (FR72)
**And** a draft whose target no longer exists reads **target gone** and cannot be validated
**When** the owner validates or rejects a draft
**Then** the decision goes through the app's study state (owner decision D3): the target study is the open study — deciding a draft of another study opens it first, as the app has one undo history for the open study, parked on close and given back on reopen if the stored study is unchanged (réconcilié 2026-10-01 : story 8.5b deviation 3 / G3 1, arch A8; was « reset on open ») — and the draft is applied to it and pushed on its undo stack; `persistence::decide_draft(study, draft_id, decision)` then writes the study upsert (with its history entry) and the draft's `status`, `decided_at`, `stale_at_decision`, `edited_before_validation` in **one** transaction, re-checking the fingerprint the owner confirmed inside it (a change since → refused, nothing written); the open study is refreshed from the dossier without resetting its undo history; a crash injected between the writes leaves both unchanged (NFR-R2, arch A7, A8)
**And** a save of the in-memory study after a decision can never overwrite the applied value (lost-update test)
**And** undoing a validation restores the prior study and sets the draft to `validated_undone` in one transaction; redoing it re-applies the value and sets the draft back to `validated`, in one transaction too; the validation stays undoable while its study remains open, and again after a reopen while the stored study is unchanged (parked history) (FR32, FR77, arch A8) (réconcilié 2026-10-01 : story 8.5b deviation 3)
**And** a validated, undone or rejected draft stays in `ai_drafts` with its outcome and decision facts (FR77).

### Story 8.3: `McpAccess` — the gated access surface (headless)

As the developer,
I want a persistence-level access type that can only read studies and only insert drafts,
So that capability asymmetry and portfolio non-exposure are enforced by the SQLite engine, not by the MCP code's good behaviour.

**Acceptance Criteria:**

**Given** a dossier path
**When** `McpAccess` serves a call
**Then** it opens the dossier for that call and closes it after; it never takes the app's single-instance lock, never migrates, sets only `busy_timeout` and `foreign_keys = ON`, and refuses to run (clear error naming both versions) unless the dossier's `user_version` equals its build's latest (FR67 [P4], arch A2)
**And** reads run in one short, bounded read transaction on a connection opened `SQLITE_OPEN_READ_ONLY`, in both WAL and DELETE journal modes, and work whether or not the app is open on the same dossier (owner decision O2); the empty `-wal` a read leaves on a closed WAL dossier is documented and accepted by the app's sidecar diagnostics (arch A2)
**And** `McpAccess` exposes typed methods only, never a connection; it is a distinct type from `Journal`; in the `mcp` crate, clippy `disallowed-types` forbids `Journal` and `disallowed-methods` forbids `restore_journal_file`, `clear_lock` and `inspect_backup` (the crate `clippy.toml` repeats the workspace settings it replaces) (arch A3)
**And** the read authorizer is an **allowlist** (`studies`, `judgments`, `journal_meta`, `ai_drafts`, SQLite internals); a CI test classifies every table in `sqlite_master` of the latest schema and fails on an unclassified one (NFR-A2, NFR-S4, arch A3)
**And** the draft connection's authorizer allows only `INSERT` into `ai_drafts`, and `UPDATE journal_meta` only from the v8 trigger; any other write — including a direct `logical_version` update — is denied by the engine and logged (FR14, FR68 [P4], NFR-A1)
**Given** a study
**When** it is read through `McpAccess`
**Then** the response carries its data cells with provenance, judgments, rationale, notes, judgment history **and** its computed outputs — zones, upside/downside ratio, 5-year potential, verdict and its state (degraded/withheld/low-confidence) — computed through the same snapshot path as the app (`report::form::build_snapshot` → `core`; no network, keychain or GUI dependency) (FR69, owner decision O1, arch A1)
**When** a draft is submitted
**Then** the checks and the insert run in one `BEGIN IMMEDIATE` transaction, and it is refused with a named reason, nothing written, when (arch A3):
- the `journal_id` or path it carries differs from the dossier resolved for the call (owner decision D10);
- its target study does not exist, its comment is empty or its origin missing (FR71, FR72, NFR-A4);
- a draft study's identifier or currency is not valid (`identifier_invalid`: ticker `[A-Z0-9.\-]{1,20}`, currency ISO 4217 — 8.0 spec §3), so they can be shown as app text;
- its field is not a draftable field, its fiscal year is not a year of the study, or its value does not parse in the field's unit or enum (owner decision D6);
- the target already has a pending draft (owner decision D4);
- it is a draft study whose security is already studied, or pending as a draft study, in the same currency, identifier compared case-insensitively (FR70, owner decisions D2, D8);
**And** a test races a submission with `delete_study` and never leaves an orphan pending draft
**Given** a restore of the dossier file
**When** an MCP write runs at the same time
**Then** restore first takes an exclusive SQLite lock on the live file and holds it until the rename, and the MCP write re-checks the file identity (the `same-file` crate's cross-platform `Handle`, NFR-X1) inside its transaction and aborts on mismatch — no draft is lost silently and the restored file's sidecars are never deleted (NFR-R2, arch A11)
**And** the following CI suites pass (NFR-A1–A4):
- **whole-surface non-exposure** — every `McpAccess` read, over a fixture dossier seeded with holdings, transactions, dividends and watchlist items carrying unique marker strings, returns none of those markers, no key and no configuration value — the resolved dossier identity (`journal_id` + path) excepted (NFR-A2, NFR-S3, NFR-S4);
- **rejected writes** — every write path other than draft insert (study, cell, judgment, verdict, note, transaction, portfolio, watchlist, `UPDATE`/`DELETE` on `ai_drafts`, direct `journal_meta` update) is denied by the engine and logged; the dossier's bytes are unchanged (FR14, FR68 [P4], NFR-A1);
- **submission refusals** — each refusal above, with its reason;
- **draft origin** — 100% of drafts written by the suite carry a non-empty comment, client + model and a timestamp; attempts without them are refused (NFR-A4);
- **metamorphic pending drafts** — for every golden and fixture study, all computed outputs are identical with and without any number of pending drafts of every kind; the engine never opens `ai_drafts` (FR72, success criterion "a pending draft changes no computed output").

### Story 8.4: `steadyinvest-mcp` stdio server

As Guy,
I want a local MCP server my AI client launches over stdio,
So that the AI can read my studies and drop drafts into my inbox — and nothing else.

**Acceptance Criteria:**

**Given** a separate binary crate `steadyinvest-mcp` (stdio, launched by the AI client, not network-exposed)
**When** it serves a call
**Then** it resolves the dossier **per call**: `--dossier <path>` if given, else the app config's `last_opened_path` (written by the app each time it opens a dossier — this story adds it), else `journal_path` (an install not yet opened with the new build), else the app's default dossier path (`default_journal_path`, moved with the config-path helper into the shared module) — even when the app is closed; the path helper lives in a small shared module so the crate does not depend on `app` (owner decisions O3, D10, arch A10, NFR-S2)
**And** a test shows that after a configured dossier is refused by name and the app runs on the default one, MCP serves the default one (the one the owner sees)
**And** every MCP response names the dossier it read (`journal_id` + path), and every submit tool requires them back (O3, D10)
**And** with no resolvable dossier, or a schema mismatch, it answers with a clear error and touches nothing
**Given** the running server
**When** the client lists tools
**Then** it offers only: list studies, get a study (with computed outputs), get judgment history, get notes, get the drafts record (with outcomes), submit draft study, submit draft note, submit draft cell/judgment value — all served through `McpAccess`; lists are paged and responses bounded (FR69–FR72, FR77, arch A2)
**And** every submit requires a non-empty comment and the client + model origin; the draft-study tool requires the proposed currency and accepts an optional company name; the value tool's schema lists the draftable fields with their units and enum variants (NFR-A4, owner decisions D2, D6)
**And** refusal reasons are worded as fixed in 8.0
**And** every refused or rejected call is written to the binary's own rotating log, `steadyinvest-mcp.log`, with its reason; the log contains no key (FR14, NFR-S1, ADD15, arch A12)
**And** the 8.3 non-exposure suite is re-run over **every tool response over stdio**, not only over `McpAccess` (NFR-S4, NFR-A2)
**And** a CI test over `cargo metadata` asserts the crate's dependency closure excludes `ingestion`, `reqwest` and any other HTTP client, `keyring`, `slint` and `app` — so no provider call is reachable (FR15, FR21, FR76, NFR-A3, arch A1)
**And** a CI test asserts that, with `--dossier` omitted and `XDG_CONFIG_HOME` / `XDG_DATA_HOME` pointed at a temporary directory, resolution never reaches the real home (arch A12)
**And** the chosen MCP SDK (rmcp or hand-rolled JSON-RPC) passes `cargo deny` (arch A12)
**And** an end-to-end test drives the binary over stdio on a temp `--dossier` copy: reads a study, submits one draft of each kind, and checks the drafts are `pending` and the studies unchanged
**And** `just mcp-build` builds the binary and `just mcp-seed <copy>` drives it over stdio to seed a temp dossier copy with drafts of every kind, so the owner and the dev agent can check the UI stories without an AI session
**And** `docs/mcp-registration.md` gives the exact commands: build, register in Claude Code at a scope **not active in the steadyinvest repository**, always with an explicit `--dossier` during Epic 8 (a test copy first, the real dossier later), and how to check the tools list; it states that registering on the real dossier is the owner's own act; the story itself never registers it (dev-safety rule)
**And** the glossary/help gains the MCP-exposed scope (studies yes; portfolio, watchlist, keys, config never) and the note that search objectives stay in the AI client session (FR75, NFR-S3).

### Story 8.5a: AI frame and draft inbox (read)

As Guy,
I want to see every AI proposal in one inbox, framed as AI, with current and proposed values side by side,
So that I know what the AI proposed before I decide anything.

**Acceptance Criteria:**

**Given** pending drafts in the dossier (submitted while the app was open or closed)
**When** I open the app, or drafts arrive while it is open
**Then** the inbox specified in 8.0 lists them, and each concerned study shows the reminder of its pending drafts; new drafts appear within ~3 s via `PRAGMA data_version` polling (Slint Timer, every 2.5 s), on inbox open and after the app's own draft-affecting writes — no window-focus trigger, Slint 1.17 exposing no window-activation API (réconcilié 2026-10-01 : story 8.5a decision 1; was « on window focus / inbox open ») — no file watcher (FR73, owner decision O2, arch A9)
**And** each item shows its AI origin (client + model), its comment, its target (study · field · fiscal year) and the current vs proposed value side by side; a stale draft shows as stale with the current value; a target-gone draft shows as such (FR72, FR73)
**And** draft studies are listed with their proposed currency and name; their validation arrives in 8.7 and the item says so, in the 8.0 wording
**And** all AI text (comments, proposed notes) is rendered only through the dedicated AI-frame component — label + disclaimer — and a structural test scans the `.slint` sources and fails if an AI-origin property is bound outside it (FR13, FR64, arch A12)
**And** a failed poll or inbox read shows the inbox « indisponible » with its cause, never an empty inbox (arch A9)
**And** a test holds an MCP read transaction open in DELETE mode while the app commits a study edit: the commit succeeds within the app's `busy_timeout`, and no edit in progress is lost (FR67 [P4], NFR-R2, arch A9)
**And** the empty inbox shows an actionable, neutral empty state (FR58), and the inbox is keyboard-operable (NFR-U2)
**And** the Epic 8 posture AC holds.

### Story 8.5b: Deciding drafts

As Guy,
I want to accept or refuse each proposal by my own hand, in two actions at most,
So that nothing enters my dossier without me.

**Acceptance Criteria:**

**Given** a pending note or cell draft in the inbox
**When** I validate or reject it
**Then** it takes **≤ 2 actions** and acts on that item only — there is **no bulk action** (FR74)
**And** a validated value enters through `decide_draft` (8.2b) as an owner entry with review tag `?` and a visible AI origin; on a `✓` target it does **not** require un-validating first — the cell moves to `?` (FR74, FR17, FR20, owner decisions O5, D5)
**And** the study grid shows the AI-origin mark on that cell as specified in 8.0 (client, model, validation date), my next edit of the cell clears it, and the history keeps it (FR17, FR51)
**And** editing the proposed value before validating makes it my own entry: it is saved without AI origin, with review tag `?` like any decision (owner decision D5, no un-validation needed on a `✓` cell), and the draft is recorded as validated with `edited_before_validation` (FR74, FR77)
**And** deciding a draft of a study other than the open one opens that study first, and the inbox says so before the action (arch A8)
**And** a stale draft can be rejected, or validated only after an explicit confirmation that names the changed target; if the target changes again after the confirmation, the decision is refused and the draft shown again (FR72, owner decision O4, arch A7)
**And** a target-gone draft can only be rejected
**And** a validated draft note becomes a study note carrying its AI origin (FR71, FR78)
**And** a validated judgment draft writes `ai_placed` on its field; its chart rendering arrives in 8.6
**And** undo after a validation restores the prior value and records the draft as `validated_undone`; redo records it `validated` again (FR32, FR77)
**And** a refused decision — dossier open read-only, write failure, study deleted or archived since the inbox was listed — opens « Action refusée » with the cause and changes nothing (7.0 AC1, FR58)
**And** the Epic 8 posture AC holds.

### Story 8.6: AI judgment lines

As Guy,
I want an AI-proposed judgment drawn beside mine but inert until I accept it,
So that the AI can challenge my numbers without ever moving my verdict.

**Acceptance Criteria:**

**Given** a pending judgment draft on a study
**When** I view the study's chart
**Then** the proposed line is drawn beside mine as specified in 8.0, annotated with the AI label (AI frame + disclaimer on its comment), distinguishable without colour alone (FR33, FR72, FR64, NFR-U1); every judgment the owner can drag on a chart gets its AI line there — §1 estimated high / low EPS (a projected EPS-growth draft drawn as the estimated-high line it implies) and §3 judged high / low P/E (#115) — dotted, hollow endpoint, « IA {valeur} » label; the other judgment drafts get an action chip, and `forecast_low_option` a chip naming the option (8.0 spec, Q13)
**And** a target holds at most one pending draft (refused at submission, owner decision D4), so a chart shows at most one AI line per judgment field, with one action chip per pending field under its legend
**And** while pending it changes **no** zone, U/D, verdict, alert or saved value — the zone bar and verdict bar are identical with and without it (FR72)
**And** I can validate or reject it from the chart or from the inbox, with the same one-by-one rules as Story 8.5b, keyboard-operable (FR74, NFR-U2)
**When** I validate it
**Then** it becomes the study's judgment, recomputes zones like any owner judgment change (FR29), and keeps the annotation "placed by AI" with its validation date (`Judgment.ai_placed`) (FR33, arch A6)
**And** any later write to that judgment field clears the annotation, while the history keeps it (FR51)
**And** undo after validation restores the prior judgment and records the draft as `validated_undone` (FR32, FR77)
**And** the metamorphic pending-drafts test of Story 8.3 is extended to judgment drafts through the app's view-model path: every computed output and every alert is identical with and without pending judgment drafts
**And** the system itself still never places or suggests a line — only an owner-validated AI draft does (FR33); the app's dimmed seeds (issue #121) never count as a placed judgment (réconcilié 2026-10-01 : PR #137)
**And** the Epic 8 posture AC holds.

### Story 8.7: Draft study end-to-end & drafts record

As Guy,
I want a validated AI ticker to become an ordinary study, and a record of every proposal and its fate,
So that the AI widens my search without a special path, and I can look back on what it proposed.

**Acceptance Criteria:**

**Given** a pending draft study (identifier, proposed currency, optional company name, comment)
**When** I validate it
**Then** the ordinary create-study dialog opens prefilled with the proposal, and confirming it creates a new, empty study — 2 actions: *Valider* → *Créer* — which is **not** added to the watchlist; the draft is recorded `validated` with `created_study_id` in the same transaction (FR70, FR74, owner decision D2, arch A8)
**And** the duplicate check runs again at that moment: if I created the same study (same identifier case-insensitively, same currency) by hand since, validation is refused with the existing study named (FR70)
**And** cancelling the dialog leaves the draft pending
**And** a draft-study validation is not on the undo stack: I reverse it by deleting the created study (with confirmation), which deletes its drafts, the draft study included (FR55, FR74, owner decision O7)
**And** no provider call happens on validation; I fetch its data myself as for any study, and only then does its data become readable through MCP — an end-to-end test on a temp dossier covers submit → validate → (stubbed) owner fetch → MCP read, with no special step (FR76, NFR-A3)
**And** a second draft study for the same security and currency is refused by MCP once the study exists (FR70)
**Given** drafts in every status
**When** I open the drafts record view specified in 8.0
**Then** every draft is listed with its origin, comment, content, submission and decision timestamps, and outcome (pending — fresh, stale or target gone — / validated, with edited-before-validation and stale-at-decision noted / validated then undone / rejected), filterable by study and outcome, all AI text inside the AI frame, keyboard-operable (FR77, FR13, FR64, NFR-U2)
**And** the same record is readable by the AI through MCP (FR77, Story 8.4)
**And** a study's history shows its processed drafts, merged by joining `ai_drafts` on `study_id` / `created_study_id` and `decided_at` — a rejected draft appears although it wrote no snapshot (FR49, FR51, arch A12)
**And** Journey 6 is walked end-to-end on a temp dossier (draft studies, a note, a lower forecast P/E, growth-judgment lines; one rejected, others validated) and the AI-assistance success criteria of the PRD are checked off
**And** the app remains fully usable with no MCP server registered — AI assistance is optional (FR65)
**And** the Epic 8 posture AC holds.

### Story 8.8: Frozen decision-time verdict (FR68)

As Guy,
I want to validate a study and have its verdict frozen at that moment, then see plainly when today's verdict differs,
So that my decision stays on record and a later refresh never silently rewrites what I decided on.

**Acceptance Criteria:**

**Given** an open study whose verdict is `Full` (every load-bearing input `✓`)
**When** I choose « Valider l'étude » (placed and worded as specified in 8.0) — Décision en attente (Guy) : placement du bouton (story 8.8 décision 16, liée à G6) ; le code le place sur sa propre ligne sous la rangée d'actions, trop large pour la fenêtre
**Then** the study stores `frozen_verdict: Option<FrozenVerdict>` (`#[serde(default)]`) in its blob: the verdict facts (verdict, zones, upside/downside ratio, 5-year potential), the `inputs_hash` and `method_version` of `core::verdict::FullVerdict`, the load-bearing input values it was computed from, and `frozen_at` — through the normal study upsert, with its history entry (FR68, FR51, owner decision D11, arch A13)
**And** the action is unavailable while the verdict is provisional or withheld, and says why, naming the inputs still open (FR12)
**And** validating again later replaces the frozen verdict; the previous one stays readable in the study history (FR51); the freeze is undoable in the session like any owner edit (FR32)
**Given** a study with a frozen verdict
**When** its current verdict — always computed live, never persisted — differs from the frozen one in its facts, `inputs_hash` or `method_version`, after a refresh, an owner edit, a validated AI draft or a method change
**Then** the study shows both, labelled « figé (vNN, JJ/MM) » and « actuel (vMM, aujourd'hui) » — as delivered « figé (ssg-1.2.0, JJ/MM) » and « actuel (ssg-1.2.0, aujourd'hui | provisoire | retenu) » (réconcilié 2026-10-01 : story 8.8 decision 9) — highlights each item that changed (verdict, zone, U/D, 5-year potential), and names the cause where known (refresh, owner edit, method change via the #252 method stamp) (FR68, FR29)
**And** when nothing differs, only the verdict and « validée le JJ/MM » are shown
**And** the wording is neutral — facts only, no banned verb (FR13); the highlight is distinguishable without colour alone (NFR-U1)
**And** the study PDF shows the frozen verdict and, when it differs, the current one, as decided in 8.0 (FR52)
**And** an export/import round-trip preserves the frozen verdict (FR59, FR60)
**And** the MCP study read returns the frozen verdict beside the current one; `frozen_verdict` is not a draftable field, and the 8.3 rejected-writes and 8.4 non-exposure suites are extended to it — no verdict is frozen or changed through MCP (FR68 [P4], FR69)
**And** a test: freeze, refresh with a changed EPS series, and check that the frozen verdict is byte-identical while the current one differs and the difference is shown; the same with a `METHOD_VERSION` change
**And** the Epic 8 posture AC holds.

## Epic 9: Getting started & ergonomics (after Guy's on-screen test of Epic 8)

Born from Guy's on-screen test of Epic 8 (H2, 2026-09-30 / 10-01) and the project review of
2026-10-01: the functions are there, the handling is rough. This epic gathers what makes the
app usable by a beginner and pleasant day to day. Its first story is the one Guy asked for; an
ergonomics pass (like 7.0 / 8.0) is expected to join once his test notes are collected.

### Story 9.1: Set up the AI from Réglages

As Guy (and any beginner),
I want to set up the use of an AI with SteadyInvest from Réglages, without typing commands,
So that the AI assistance of Epic 8 is within reach of someone who does not know MCP, Claude Code
or the terminal.

Context: today the setup is manual and documented in `docs/guide-ia.md` §3 (build
`steadyinvest-mcp`, create `~/steadyinvest-ia`, `claude mcp add --scope local … --dossier …`,
verify with `claude mcp list` / `/mcp`). Guy, 2026-10-01: « l'installation de la partie IA n'est
pas triviale pour un débutant : serait-il possible de l'automatiser dans steadyinvest ? par exemple
dans Réglages ? »

**Acceptance Criteria:**

**Given** Réglages
**When** I open the new « Assistance IA » card
**Then** it states, each as a fact with its cause when not met, never an empty-looking card
(checklist §1):
- whether the MCP server is available (the `steadyinvest-mcp` binary found beside the app's own
  executable, else in the build directory) and its version;
- whether Claude Code (`claude`) is found on the `PATH`;
- whether a `steadyinvest` server is registered for the AI working directory, and on which dossier;
- the AI working directory (`~/steadyinvest-ia` by default).
**Given** the server and Claude Code are both available
**When** I choose « Préparer l'assistance IA »
**Then** a confirmation lists exactly what will be done — create the working directory if absent,
write a starter `CLAUDE.md` there only if none exists, and run
`claude mcp add --scope local steadyinvest -- <binary> --dossier <dossier>` from that directory —
and nothing happens until I confirm (an outward write into another program's configuration)
**And** on confirmation the steps run and the card states the outcome; a failure names its cause
and the step it stopped at, never a silent half-setup
**And** the registration is ALWAYS in `local` scope from the working directory, never `user` nor
`project` (the AI must not see the dossier from the SteadyInvest source tree — arch A12)
**And** « Retirer » runs `claude mcp remove --scope local steadyinvest` from the same directory,
after its own confirmation; « Changer de dossier » re-registers on another dossier
**Given** Claude Code is not found
**Then** the card shows the exact commands with my real paths, each with « Copier », and a link to
the guide (`docs/guide-ia.md`)
**And** the app never launches an AI session, never talks to an AI, and never reads or writes
`~/.claude.json` itself — only through the `claude` command (the app still contains no AI)
**And** the server binary is built with the app (a `just` recipe or a workspace setting so that the
normal build produces both), and the guide and the in-app glossary are updated
**And** tests: the diagnosis with a fake `claude` on the `PATH` (found / absent / registered /
not registered), the exact command lines produced, no write before confirmation, the refusal of a
non-`local` scope; a headless check of the card
**And** the posture AC holds (French wording, glossary entry, `@tr` / `MSG_*` re-based, no banned
verb, refusals in « Action refusée »).

Open questions (asked 2026-10-01, unanswered — conservative defaults to be confirmed by Guy at
create-story):
- Q1 Level: full (diagnosis + one-click registration + removal) **[default]** or diagnosis +
  commands to copy only?
- Q2 Which dossier the AI reads: the dossier open in the app **[default, stated in the
  confirmation]**, or a copy made for the AI?
- Q3 Build: make the normal build produce the server too **[default]**, or keep `just mcp-build`?
- Q4 Architecture A12 (« registration is the owner's act, by hand ») is amended: the owner's
  confirmed click in Réglages is that act.

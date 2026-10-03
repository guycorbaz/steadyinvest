---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
lastStep: 8
status: 'complete'
completedAt: '2026-06-08'
inputDocuments:
  - _bmad-output/planning-artifacts/prd.md
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
referenceOnlyDocuments:
  - "docs/change-request_guy.md (still-valid user CRs: search by ticker+company name, delete snapshots, diversify-by-size table, full SSG PDF, zone indicator)"
notes:
  - "Legacy OLD-PROJECT artifacts (web/Leptos/Loco/MariaDB stack, Feb-May 2026) still present in _bmad-output/ due to Synology Drive re-sync (inodes recreated 2026-06-08 10:41). Treated as NON-INPUTS. Prior architecture.md archived to _bmad-output/_archive/architecture-LEGACY-web-stack-2026-02.md. User handling cleanup at the Synology source."
  - "docs/ process md files (definition-of-done, deployment-verification, living-documentation, lessons-learned-chart-rendering) describe the OLD web stack: principles transferable, stack specifics (WASM/Leptos/Docker/ECharts) do NOT apply to the Slint desktop app."
workflowType: 'architecture'
project_name: 'steadyinvest'
user_name: 'Guy'
date: '2026-06-08'
changelog:
  - date: '2026-09-27'
    changes: "G2 — Phase 4 [P4] AI assistance over MCP (Epic 8): added the section 'Phase 4 — AI Assistance over MCP' (decisions A1–A11 under owner decisions O1–O7: steadyinvest-mcp stdio binary + dependency boundary; per-call connection, no lock, version gate; McpAccess with engine-enforced SQLite authorizers; ai_drafts table (migration v8); study notes in the Study blob; AI origin on Provenance/Judgment; stale fingerprint; atomic decide_draft; data_version polling; dossier resolution; risks). Updated Requirements Overview (78 FRs, FR14/FR33 [P4], FR69–FR78, NFR-A1–A4), Technical Constraints, Cross-Cutting Concerns, workspace layout, Deferred Decisions, Data Architecture (two processes, lock, tables), Security & Privacy (NFR-S3), API & Communication, Infrastructure, Cross-Component Dependencies, SQLite naming, Format Patterns (export envelope IS deny_unknown_fields, #78), directory tree, Architectural Boundaries (network boundary incl. mcp closure), FR mapping, Integration Points, Validation and Readiness notes (clerk stance dropped)."
  - date: '2026-09-27'
    changes: "G2 round 3 (G3 review of PR #255, owner decisions D1–D10): §Phase 4 revised — McpAccess typed methods only, table allowlist + classification test, logical_version bumped by a v8 trigger, submission checks in one IMMEDIATE transaction, ai_drafts decision columns, DraftOrigin vs AiOrigin, explicit `?`, SCHEMA_VERSION bumps for notes/AI marks, canonical fingerprint, validation through app state + undo, per-call dossier resolution on last_opened_path, restore vs MCP (new A11), risks renumbered A12; Security, Format Patterns and NFR coverage lines updated."
---

# Architecture Decision Document

_This document builds collaboratively through step-by-step discovery. Sections are appended as we work through each architectural decision together._

## Project Context Analysis

### Requirements Overview

**Functional Requirements (78 FRs, 12 clusters; phase tags P1/P2/P3/P4/V — FR67–FR68 added
post-v1, FR69–FR78 [P4] added by G2 on 2026-09-27):**
- *Stock Study & Methodology Engine* (FR1-8): create/persist/reopen/update studies; deterministic
  SSG output set; native-currency calc; judgment inputs; quality flags; 5-year-floor low-confidence.
- *Calculation Integrity & Trust* (FR9-14): golden reference + tolerance; plausibility warnings;
  verdict traceability; testable degraded/withheld verdict; neutrality (banned-verb) of app outputs,
  AI-origin text framed as third-party content; MCP surface verifiably draft-only [P4].
- *Data Acquisition, Provenance & Providers* (FR15-29): auto-fetch; first-class manual entry/override;
  per-cell source + provenance + timestamp; coverage present/to-fill/not-available-accepted; user-set
  validated flag; manual refresh; non-destructive reconciliation; graceful provider failure (stale +
  cause); keyless + keychain keys; FX acquisition [P2]; deterministic recompute distinguishing cause.
- *Charts & Judgment Interaction* (FR30-33): growth/valuation charts; judgment line by value +
  direct-manipulation in sync, live recalc; undo; the app never auto-places/suggests a line — [P4]
  exception: an AI-proposed judgment is drawn as an "AI"-annotated pending line that changes no
  verdict/zone/alert until the owner validates it (then "placed by AI" + validation date).
- *Watchlist & Alerts* (FR34-35) · *Portfolio/Transactions/Holdings* (FR36-41, single [P1] →
  multi-portfolio/FX/ledger/dividends [P2]) · *Risk Management* (FR42-48: trailing stop, simple
  capital-at-risk [P1]; per-currency→bank→global + concentration [P2]; neutral triggers; stop-priority;
  replacement [P2]).
- *Cumulative Memory & Journal* (FR49-51) · *Reporting/Printing* (FR52-53) · *App Shell & Data Mgmt*
  (FR54-62: dashboard; delete/archive w/o corrupting time-series; entry↔contemplation regimes; legend;
  empty/error; single-study + whole-journal export/import versioned+validated; restore w/ integrity +
  version checks; help + demo) · *Config/Posture* (FR63-66: no-wizard Settings; always-visible
  disclaimer; full offline; portable local store).
- *AI assistance [P4]* (FR69-78, Epic 8): an owner-chosen AI client reads studies (cells,
  provenance, judgments, rationale, notes, history, computed outputs) over a local MCP server — never
  the portfolio, watchlist, keys or config; it may only submit **commented drafts** (study / note /
  cell incl. judgment values) that the owner validates or rejects one by one in a dossier-level
  inbox; durable draft record; provider fetches stay owner-initiated; study notes (FR78).
  Cross-cutting NFR-A1–A4: capability asymmetry and portfolio non-exposure **by construction**, no
  provider call reachable from MCP, 100% of drafts carry origin + timestamp + non-empty comment.
  See §Phase 4 — AI Assistance over MCP.

**Added requirements (2026-06-08, to file as FRs once the repo exists):**
- User-selectable journal DB directory + reopen the last-used journal on launch (recent-journals).
  Posture: live DB local; versioned exports/backups to the (Synology) sync folder.

### The Foundational Invariant (cross-cutting, first-rank)

> **Every fact the system asserts — a raw input, a derived value, a verdict, the "current"
> journal — is inseparable from a dated proof of which source / which version / which instant
> produced it; and any break in that link is a VISIBLE event, never a silence.**

This single property unifies what looked like four separate risks (silent wrong signal · stale
journal silently reopened · verdict whose inputs moved underneath it · ambiguous identity of a
copied journal): all are the same disease — *silent drift between an assertion and the ground it
rests on*. It is elevated to a first-rank property of the versioned data contract, at the same
level as the Slint/SQLite decoupling — not a relegated NFR. It is also the product differentiator a
spreadsheet structurally cannot offer (the defence and the value proposition are the same thing).

Mechanisms it imposes:
- Each entity carries `(source, logical_version, timestamp, hash_of_dependencies)`.
- **Transactional recompute**: inputs and verdict are born in the same transaction.
- **Verdict addressed by its inputs**: `verdict = f(hash(inputs), method_version)`; if an input
  changes, the prior verdict becomes *orphaned/stale*, detectably — **invalidation, not silent
  overwrite**. The UI never shows a fresh number beside an input it does not descend from.

### Non-Functional Requirements (drivers)

- *Correctness (top priority)* — deterministic, reproducible engine; golden match (exact
  zoning/verdict, ±0.5% numerics); property invariants; CI-gated.
- *Performance* — <~100 ms judgment recalc/recolor; <~1 s open/recompute; non-blocking refresh;
  <~3 s launch.
- *Security & Privacy* — keys only in OS keychain; no telemetry; all data local; [P4] study data
  may leave the machine only through the owner's chosen AI client, portfolio data never (NFR-S3/S4).
- *AI capability asymmetry [P4]* (NFR-A1–A4) — draft creation is the only MCP write; portfolio
  non-exposure; no provider call from MCP; origin + comment on every draft — all enforced by the
  SQLite engine and the crate graph, tested in CI.
- *Reliability & Data Integrity* — offline; crash-safe/atomic writes; forward-safe migrations;
  non-destructive reconciliation; integrity+version checks on import/restore.
- *Portability* — identical behaviour AND numeric results on Win/macOS/Linux; locale-aware numbers;
  portable journal file.
- *Usability* — decision never colour-only; keyboard-first; recognizably faithful form.
- *Maintainability* — thin UI over a UI-independent tested calc crate + versioned data contract
  decoupled from Slint and storage.

### Core Technical Decisions Locked in This Phase

- **GUI = Slint-only, native. NO web UI** (no Tauri/Leptos/webview — confirmed by the user
  2026-06-08). The egui-in-Slint embedding is the source of the charting friction (two render
  paradigms in one window) and is **rejected; egui is removed entirely from the architecture.**
  Charts (draggable judgment lines + <100 ms zone recolor) are drawn **natively in Slint**
  (`Path` + `TouchArea`, log10 in Rust; recolor is trivial in Slint's dirty-driven retained mode).
  The user has **not** previously done interactive vector drawing in Slint, so the Week-1 throwaway
  **spike** (semi-log chart + one draggable judgment line + live recolor; go/no-go on drag→pixel
  <100 ms) is a **genuine risk to watch, not a formality**. Fallback if the spike fails: dedicated
  Slint canvas/window, or `plotters`→`SharedPixelBuffer` static backdrop + Slint `TouchArea` overlay
  (the drag stays Slint). NOT egui, NOT web.
- **Exact decimal arithmetic** (e.g. `rust_decimal` / scaled integers) for money and ratios, with
  rounding explicit and named only at display — NOT naïve `f64`. This kills the "plausible-but-wrong"
  float drift AND makes cross-OS determinism trivial/provable in one move. (Revisit only if the
  decision chain is proven to use no transcendental ops; composed-growth projections suggest they
  are present.)
- **Three version axes, not two**: `schema_version` (serialized contract) · SQLite schema
  (`PRAGMA user_version`) · **`method_version`/`formula_version`** (calculation semantics).
- **Theme tokens = one neutral source of truth read by Slint** (intra-binary, not "across an FFI");
  zone/ink/label tokens pushed by a plain main-thread owner (`app/src/theme.rs`; no `arc_swap` —
  réconcilié 2026-10-01); theme/regime change forces a redraw.
- **Two distinct quality-gate families**: *trust gates* (types, traceability, reproducibility) vs
  *posture gates* (neutral naming, swappable labels). Do not reduce neutrality to a string grep.

### Technical Constraints & Dependencies

- Stack: Rust + Slint (egui removed), local SQLite via rusqlite, offline-first, no network server,
  versioned serde data contract decoupled from Slint and SQLite. [P4] adds a second, separate
  process: a local stdio MCP server (`steadyinvest-mcp`) that reads studies and writes **drafts
  only**, through a narrow persistence surface (`McpAccess`) — see §Phase 4.
- **Ingestion/normalization is a first-order architectural boundary**, distinct from the calc engine:
  IFRS↔US-GAAP, split/series breaks, fiscal-period misalignment, currency-of-report — the real
  birthplace of the silent-wrong-signal — get their own normalization layer, golden recollage
  fixtures, and **metamorphic tests** (equivalent IFRS/GAAP inputs ⇒ same verdict).
- **Trust invariants as TYPE properties, not just tests**: a `FullVerdict` is constructible only from
  all-validated-and-fresh load-bearing inputs (compiler is the gate); verdict + staleness derive from
  the SAME immutable state snapshot so an incoherent frame is structurally impossible.
- **Test architecture (CI gates)**: frontier golden fixtures (synthetic, documented provenance);
  property tests incl. monotonicity / boundary-continuity / idempotence / scale-homogeneity;
  metamorphic tests; determinism hash pinned (equal across OS by construction under decimal; CI runs
  on Linux only for now, see §Starter); a
  frozen versioned-journal corpus + schema-drift detector + forward-compat read-only-on-newer-file.
- PDF/print fidelity (FR52) via `pdf-writer` in the `report` crate, UI-independent. (réconcilié
  2026-10-01 : `.github/workflows/ci.yml` `os: [ubuntu-latest]`, `Cargo.toml` `pdf-writer = "0.12"`)
- Licensing: GPL-3.0 intended, dependency-license audit (Slint tier). i18n French-first, separate
  from the NAIC↔neutral label set. No vendor data in repo; user brings own key.
- **Environment**: project tree under a Synology Drive synced path; a live SQLite file must NOT sit in
  a sync-watched folder (lock/WAL corruption) — backups/exports go there instead.

### Cross-Cutting Concerns Identified

- The foundational traceability invariant (above) — provenance, journal identity, backup freshness,
  verdict validity are ONE principle.
- Calculation integrity & determinism (exact decimal; engine + risk crate; golden/property/metamorphic;
  CI gate).
- Provenance & trust model — per cell source × freshness × review tri-state (none/?/✓ + soft-lock);
  non-destructive reconciliation; divergence → auto-?.
- Multi-currency / FX — native-currency calc; FX only at consolidation; dated, source-aware rates
  frozen at the judgment date.
- Neutral posture — the app's OWN outputs stay facts-not-advice: banned-verb enforcement,
  always-visible disclaimer, the app never auto-places or suggests a line. [P4] exception (FR33): an
  AI-proposed judgment appears as an AI-annotated pending line; AI text is third-party content shown
  only inside the AI frame (label + disclaimer), never restyled as an app signal; the MCP surface is
  draft-only by construction (NFR-A1).
- Versioned data contract & schema migrations (three version axes) — forward-safe; journal survives
  version bumps.
- **Journal identity** — a `journal_id` (UUID) + monotonic logical version written INTO the DB at
  creation; the "last-used" pointer references `(journal_id, last-seen-version)`, not a path; copies
  are detected, stale-restored journals are surfaced ("you saw v57, this is v41"); backups carry
  `(journal_id, version, hash)`.
- App-config vs journal boundary — `directories` crate for config (last path, recents, UI prefs),
  `keyring` for secrets, journal SQLite local + synced exports; app-config strictly local & per-machine;
  single-instance file lock; sync-path detection warning; `journal_mode` choice (DELETE on sync paths).
- Theming (single source of truth, intra-binary) · Cross-platform parity · Accessibility (right-sized:
  decision never colour-only, keyboard-first, marker-confusability CI gate).

### Resolved Data-Model Decisions (this phase)

- **Journal ↔ portfolio cardinality:** one journal = the user's whole investing world and holds
  **N portfolios, one per *banking relationship* (bank/account)**. "All securities owned" = the
  **journal-level consolidated view** across portfolios; global risk and concentration are computed
  there (per-currency → per-bank → global total, FX only at consolidation). Multiple journal *files*
  are for entirely separate universes (e.g. real vs test), not for separating banks.
- **Verdict versioning after an engine/method change:** the **decision-time verdict is frozen and
  immutable** (stamped `method_version` + dated FX + inputs) — it is the authoritative journal fact
  and is the **only** verdict persisted. The "recomputed-with-today's-method" verdict is **computed
  on demand** for comparison/debugging (never persisted, never overwrites the original). Normal
  reopen (unchanged method) shows the frozen verdict, no recompute. When `method_version` changed,
  the UI signals it and offers a labelled "frozen (vNN, DD/MM) vs recomputed (vMM, today)" compare.
  Bonus: persisting the original verdict value removes the need to keep old formula code to reproduce
  it (the recompute uses only the current engine) — a maintenance simplification.

### Open Decisions (carried forward)

- None blocking. The Week-1 charting spike (drag→pixel <100 ms in pure Slint) is the principal
  technical unknown to resolve before committing UI work; raised in risk because the user has no
  prior interactive-Slint-drawing experience.

## Starter Template Evaluation

### Primary Technology Domain

Native cross-platform **desktop application in Rust** (Windows/macOS/Linux), offline-first, single
binary with embedded SQLite. No web/mobile/server domain applies. The heavy "starter/boilerplate"
ecosystem (Next.js, T3, etc.) is irrelevant; the right foundation is a **Cargo workspace** (thin
Slint UI over a UI-independent, tested calculation core), seeded by the official Slint Rust template
for the UI crate.

### Versions Verified (web, June 2026)

- **Slint 1.17** (1.16.1 when verified in June; `Cargo.lock` now resolves `slint 1.17.0` from the
  caret requirement `"1.16"`; its own MSRV is Rust 1.88, but the **workspace MSRV is 1.96** — forced by
  Slint transitive deps and libsqlite3-sys 0.38; pinned in `rust-toolchain.toml`, verified
  Story 1.1; réconcilié 2026-10-01 : `Cargo.lock`) — licensed GPLv3 / royalty-free / commercial; **GPLv3 is
  compatible with this project's GPL-3.0**, closing the PRD's "Slint licensing tier" risk. Charts are
  drawn natively (`Path` + `TouchArea`); egui is removed.
- **rusqlite 0.40.0** with the `bundled` feature — SQLite (public domain) compiled into the binary;
  no system dependency; ideal for an offline owned desktop app.
- **rust_decimal 1.42** — exact 96-bit decimal for money/ratios (the anti-`f64` decision). Enable the
  **`maths`** feature for compound-growth projections (`powd`/`exp`/`ln`): pure-Rust and
  **deterministic cross-platform**, which satisfies the determinism requirement WITHOUT vendoring a
  libm or bit-hashing f64. Verify CAGR precision in the Week-1 spike.
- **keyring 3.x (hwchen API)** — cross-platform OS secret store. **Do NOT use `keyring` 4.0**:
  as of 4.0.x the crate was re-published as a sample/CLI meta-crate (crates.io description "Sample
  code and CLI for the Rust Keyring") with **no feature flags** and **mandatory** deps on every
  backend store (`keyring-core`, `db-keystore`, `*-keyring-store`) plus `clap`/`rpassword` — a heavy,
  non-lean tree. The real library is **`keyring = "3"`** (3.6.x), pinned with
  `default-features = false` and explicit platform features — as built: `async-secret-service` +
  `crypto-rust` + `async-io` on Linux (persistent secret-service backend, Guy 2026-06-25; `async-io`
  rather than `tokio` to avoid a UI-thread deadlock) — (réconcilié 2026-10-01 : `Cargo.toml` bloc
  `keyring`, story 3.2 DEVIATION); `apple-native`/Keychain on macOS and `windows-native`/Credential
  Manager on Windows are not enabled yet (Linux-only build). The
  forward-looking alternative is **`keyring-core` 1.x + a vetted backend store crate** per platform;
  evaluate both in **Story 3.2** and lock the choice with `cargo deny` (lean tree, GPL-3.0-compatible).
  Note: the Linux secret-service backend needs a running D-Bus/secret agent — relevant for
  headless/NAS use; keyless providers avoid the issue entirely. Not introduced until Story 3.2.
- **directories** — `ProjectDirs::from(...)` for the app-config location (XDG / AppData / macOS
  Application Support), separate from the journal DB.

### Starter Options Considered

- **Official Slint Rust template** (`cargo generate --git https://github.com/slint-ui/slint-rust-template`)
  — sets up a single-crate Slint app with `build.rs` (`slint-build`) wiring, a `.slint` file, and the
  recommended project layout. Excellent as a reference/seed for the UI crate; not a multi-crate
  workspace by itself.
- **Plain `cargo new` + manual workspace** — full control over the multi-crate layout that the
  architecture requires (pure calc core, data contract, persistence, provider, UI).
- **Heavyweight web/full-stack starters** — rejected (no web; offline-first native desktop).

### Selected Starter: Custom Cargo workspace, UI crate seeded from the Slint Rust template

**Rationale for Selection:**
The architecture mandates a **thin UI over a UI-independent, deterministic, tested calculation
crate** plus a **versioned serde data contract decoupled from Slint and SQLite**. That is a
multi-crate workspace, which no single off-the-shelf starter provides. We therefore bootstrap a
custom Cargo workspace and use the official Slint Rust template to seed the UI crate's Slint build
wiring (so we inherit Slint's recommended `build.rs`/`.slint` setup without reinventing it).

**Initialization Commands:**

```bash
# 1. Create the workspace root
cargo new --vcs git steadyinvest && cd steadyinvest
# (edit Cargo.toml -> [workspace] members)

# 2. Seed the UI crate from the official Slint template (reference for build.rs/.slint wiring)
cargo install cargo-generate
cargo generate --git https://github.com/slint-ui/slint-rust-template --name app
#   -> rework the generated crate into the workspace's `app` (UI) member

# 3. Add pinned dependencies to the relevant crates
cargo add slint@1.16        --package app          # resolves to 1.17.0 today
cargo add slint-build@1.16  --package app --build
cargo add rusqlite@0.40 --features bundled        --package persistence
cargo add rust_decimal@1 --features maths          --package core
cargo add serde@1 --features derive                --package contract
cargo add directories@6                            --package app
cargo add keyring@3 --no-default-features          --package app         # Story 3.2 only; as built: --features async-secret-service,crypto-rust,async-io (réconcilié 2026-10-01). NOT keyring 4.x (sample/CLI meta-crate)
```

**Proposed workspace layout (crates):**

- `core/`      — pure SSG calculation engine (deterministic, `rust_decimal` w/ `maths`, no I/O, no
  UI). Golden + property + metamorphic tests live here. Stamps `method_version`.
- `contract/`  — versioned serde data contract (`schema_version`), journal/judgment types, decoupled
  from Slint and SQLite. The vocabulary the [P4] MCP server speaks (studies, notes, drafts).
- `ingestion/` — provider-agnostic acquisition + **normalization** layer (IFRS/GAAP, split/series,
  fiscal-period, currency-of-report), `MarketDataProvider` trait + adapters. Its own golden fixtures.
- `persistence/` — rusqlite storage (journal_id, logical version, migrations, `PRAGMA user_version`),
  export/import/backup. Local DB; sync-path detection.
- `report/`    — study → `core` mapping (the frozen-verdict comparison, the form) and the PDF/print
  output (`pdf-writer`), UI-independent.
- `paths/`     — the per-machine locations (app-config, default dossier, logs) and the dossier
  resolution, shared by `app` and `mcp` (story 8.4, décision 2).
- `app/`       — thin Slint UI (forms, dense grid, native charts via `Path`/`TouchArea`), app-config
  via `directories`, secrets via `keyring`, theme tokens single-source.
- `mcp/`       — [P4, G2] `steadyinvest-mcp`: separate stdio binary launched by the AI client; reads
  studies and submits drafts through `persistence::McpAccess`; its dependency closure excludes
  `ingestion`, `reqwest`, `keyring`, `slint` and `app` (see §Phase 4).

As built, the workspace has **eight** members: `core, contract, ingestion, persistence, report, app,
paths, mcp` (réconcilié 2026-10-01 : `Cargo.toml` `[workspace] members` ; story 8.4 décision 2).

**Architectural Decisions Provided / Implied by this Foundation:**

- **Language & Runtime:** Rust (workspace, MSRV 1.96 — driven by Slint 1.16/1.17 transitive deps +
  libsqlite3-sys 0.38; pinned in `rust-toolchain.toml`); single native binary per OS.
- **UI:** Slint 1.17 (GPLv3; requirement `"1.16"`, resolved 1.17.0 — réconcilié 2026-10-01 : `Cargo.lock`), declarative `.slint` + `slint-build`; charts native; no web, no egui.
- **Numerics:** `rust_decimal` (+`maths`) exact decimal in the core (determinism + correctness).
- **Persistence:** rusqlite `bundled` SQLite, single local file.
- **Secrets / Config:** `keyring` (OS store) + `directories` (app-config), kept out of the journal.
- **Testing:** Cargo test in `core`/`ingestion` (golden/property/metamorphic), versioned-journal
  corpus in `persistence`; CI is **Linux-only for now** (decision 2026-06-09) — cross-OS identity
  is asserted via pinned determinism hashes (exact decimal makes it hold by construction); the
  3-OS matrix returns when multi-OS support is back in scope.
- **Code Organization:** multi-crate workspace enforcing the thin-UI-over-tested-core boundary.

**Note:** Workspace + UI-crate initialization (these commands) should be the **first implementation
story**. The **Week-1 charting spike** (native Slint draggable judgment line + <100 ms recolor) runs
against this skeleton as the principal go/no-go before committing UI work.

## Core Architectural Decisions

### Decision Priority Analysis

**Critical Decisions (Block Implementation):**
- GUI = Slint-only native (egui removed; no web) — *step 2/3*.
- Exact decimal numerics (`rust_decimal` + `maths`) in a pure, deterministic calc core — *step 2/3*.
- Versioned serde data contract decoupled from Slint & SQLite; three version axes
  (`schema_version` / SQLite `user_version` / `method_version`) — *step 2/3*.
- Journal storage model = **hybrid** (normalized where aggregated; versioned JSON blob where replayed).
- The Foundational Invariant realized **by construction** (immutable snapshot → content-addressed
  verdict → invalidation, not silent overwrite) — *step 2*.

**Important Decisions (Shape Architecture):**
- HTTP/fetch = **reqwest 0.13 + tokio 1.52** (async), off the UI thread, with a pure-Rust TLS
  backend. ⚠️ **REVALIDATED 2026-06-15 (#5/B5):** reqwest 0.13's TLS features changed again — there is
  no `rustls-tls`/`rustls` umbrella feature. The current set is `rustls-no-provider`,
  `rustls-native-certs`, `webpki-roots` (the default crypto provider is aws-lc-rs, which needs cmake).
  **Story 3.1 pure-Rust no-cmake path:** enable `rustls-no-provider` + install the `ring`
  `CryptoProvider` in code, with `webpki-roots` (bundled roots) or `rustls-native-certs` for the trust
  anchors.
- Error model (`thiserror` 2.0, neutral cause-named, no silent `.ok()`); logging (`tracing`, local
  file, no telemetry); test architecture (`proptest` 1.9 + golden/metamorphic + CI on Linux only for
  now — the 3-OS matrix is a target, not the current state; réconcilié 2026-10-01 : `.github/workflows/ci.yml`).
- App-config vs journal boundary (`directories` + `keyring`); journal identity (`journal_id` + logical
  version); SQLite pragmas + sync-path detection.
- Export/backup format; decimal rounding policy; UI visual-verification strategy (the four points
  below).

**Deferred Decisions (Post-MVP):**
- ~~Read-only MCP/AI façade over the data contract [V]~~ — no longer deferred: decided 2026-09-27
  (G2) as Phase 4 [P4], a draft-writing MCP process (§Phase 4); a self-hosted local model stays [V].
- Provider fallback-chain & rate-limit batching
  [P2]; multi-portfolio/FX consolidation depth, transaction ledger, dividends [P2]; PDF of the other
  forms [P2/P3]; configurable "diversify-by-company-size" table (from `change-request_guy.md`) [P2].

### Data Architecture

- **Store:** rusqlite 0.40 `bundled` SQLite, single local file (the journal). `WAL` +
  `synchronous=NORMAL` + `busy_timeout` for local use; **auto-switch to `DELETE/TRUNCATE` + warn**
  when the DB path is detected on a sync folder (Synology/Dropbox/OneDrive/iCloud). Single
  mutex-guarded write connection (WAL allows concurrent readers + one writer). **[P4] two
  processes on one file:** the app keeps its long-lived write connection; `steadyinvest-mcp` opens
  **short-lived per-call connections** (one per tool call, closed at its end) and never sets
  `journal_mode` or migrates. SQLite serialises the two writers; both sides set `busy_timeout` so a
  brief contention waits instead of failing, and MCP transactions are kept short (one read
  transaction or one draft insert) so the app's writes are never held up noticeably (§Phase 4, A2).
- **Hybrid model (decided):**
  - *Normalized tables* for what we aggregate/query: `portfolio`, `holding`, `transaction`, `fx_rate`,
    `watchlist_item`, plus index columns. `persistence` returns these rows; consolidation
    (per-currency→per-bank→global), concentration and capital-at-risk are computed in
    `app/src/state/{fx,concentration,review}.rs` with `core::risk` — never in SQL
    (réconcilié 2026-10-01 : `app/src/state/{fx,concentration,review}.rs`, `persistence/Cargo.toml` sans `core` — p2 écart 17).
  - *Versioned serde JSON blob* (`payload TEXT` + `schema_version` column) for what we replay in bulk:
    `study` and its `judgment` snapshots. Append-mostly, read whole, never queried by inner field →
    no SQL migration when the judgment model evolves. [P4] study notes live inside the `Study` blob;
    `ai_drafts` (migration v8) follows the same hybrid pattern — indexed columns + versioned JSON
    payload (§Phase 4, A4/A5).
  - Indexed columns alongside blobs: `journal_id`, `security_ticker`, `created_at`, `status`,
    `schema_version`, `method_version`.
- **Identity & integrity:** `journal_id` (UUID) + monotonic logical version written INTO the DB at
  creation; the app-config "last-used" pointer references `(journal_id, last-seen-version)`, not a
  path; backups/exports carry `(journal_id, version, hash)`; a single-instance file lock guards the
  open journal **against a second app instance** — the [P4] MCP process never takes it (it must work
  while the app is open or closed, O2); concurrency with MCP is left to SQLite's own locking. A draft
  insert bumps `logical_version`, so a reopened journal whose version is higher than last seen is
  normal, not an anomaly (only a lower version signals a stale restore).
- **Validation:** strong typing at the ingestion boundary (`serde` + domain newtypes, e.g.
  `CurrencyCode`); `unknown/insufficient` is a first-class state, never coerced to 0; the raw↔derived
  boundary is a wall (no derived value persisted as if entered; cached derived values carry their
  `method_version` and are invalidated on input/formula change).
- **Numerics & rounding (decided — point 2):** all money/ratio math in `rust_decimal` (+`maths`);
  **a single named rounding mode and per-field display scale are defined in `core`** and applied
  **only at display**, never mid-chain. This anchors the golden ±0.5% tolerance and cross-OS
  reproducibility.
- **Migrations:** `PRAGMA user_version` (SQL schema) + `schema_version` (blob); lazy upgrade on save;
  forward-compat = read-only when the file's version is newer than the app; frozen versioned-journal
  corpus + schema-drift detector in CI.
- **Export / backup format (decided — point 1):** the portable export unit (single study FR59 /
  whole journal FR60) is the **serialized serde data contract (JSON) + `schema_version` + integrity
  hash**, NOT a raw `.db` copy — portable across schema evolution and verifiable on import
  (reject/migrate on mismatch, FR60/FR61). A raw `.db` file copy remains the file-level backup unit
  pushed to the NAS sync folder; the JSON export is the exchange/seed/golden unit.
- **FX:** `fx_rate` rows are dated & source-aware; FX applied only at the consolidation layer. A study
  verdict involves no FX, so a frozen verdict stamps none (A13); consolidation reads the latest dated
  rate per pair and shows its date (réconcilié 2026-10-01 : A13, story 6.6 AC2/AC4).

### Authentication & Security

- **No authentication / no accounts / no multi-user** — single-user offline desktop by design.
- **Secrets:** provider API keys only in the OS secret store via `keyring` 3.x (platform backends
  chosen explicitly, `default-features = false`; **not** keyring 4.0 — see Tech Stack note; Linux
  secret-service needs a D-Bus agent). Never in repo/config/logs/exports.
- **Privacy:** no telemetry; the only network calls the app makes are user-initiated provider/FX
  fetches under the user's own key; all data stored locally. [P4] (NFR-S3): study data may leave the
  machine only through the AI client the owner chooses and registers (it reads over MCP and may send
  what it reads to a remote model); **portfolio data never leaves it** — the MCP surface cannot read
  it (NFR-S4/A2).
- **AI [P4] (NFR-A1–A4):** capability asymmetry enforced **by the SQLite engine, not by prompt**:
  the MCP process holds only a `persistence::McpAccess` handle with typed methods (never a
  connection); its read connection's authorizer allows only an allowlist of tables (studies,
  judgments, journal metadata, drafts) and its draft connection's authorizer allows only `INSERT`
  into `ai_drafts` (the `logical_version` bump runs from a v8 trigger; a v9 trigger refuses an insert
  over an existing draft id — réconcilié 2026-10-01); every other read/write is
  denied at statement preparation and logged. Drafts carry origin + non-empty comment (DB `NOT NULL` +
  `CHECK`). No provider call is reachable (the crate graph excludes it). Details: §Phase 4, A1/A3.

### API & Communication Patterns

- **No network server / no public network API.** Internally, the "API" is the **versioned serde
  data contract** (clean types, `schema_version`), decoupled from Slint and rusqlite. [P4] adds the
  **only external interface**: a **local stdio MCP server** (`steadyinvest-mcp`, JSON-RPC over the
  stdin/stdout of a child process spawned by the owner's AI client), not network-exposed (NFR-S2).
  Its tools: list/get studies (with computed outputs), judgment history, notes, the drafts record;
  submit draft study / note / cell. Every response names the dossier it read (`journal_id` + path).
- **Provider acquisition:** `MarketDataProvider` trait; first adapter **EODHD** (CH/EU+US coverage);
  keyless adapters supported. HTTP via **reqwest 0.13** with **`rustls-no-provider` + the `ring`
  provider** (pure-Rust, no system OpenSSL, no cmake → portable single binary; see §Tech Stack
  revalidation 2026-06-15) + `json`, on a **dedicated `current_thread` tokio 1.52 runtime on
  a worker thread** (sufficient for manual refresh; P2 ticker-batching via concurrent tasks
  `join_all`). Results marshalled back to the Slint event loop via `invoke_from_event_loop`. Provider
  failure is classified (network / quota / invalid-or-absent key), recorded, surfaced as a neutral
  global banner; last-known values retained and flagged stale. As built (réconcilié 2026-10-01 :
  PR #274, #276, #277): the current price records its origin (`Judgment.current_price_origin`:
  provider + session date, or owner-typed + date); a failed refresh flags a fetched price stale and
  the verdict becomes provisional (« Prix actuel — périmé ») until a successful refresh — a price is
  stale only after a failure, never by age. **Décision en attente (Guy) : horizon d'âge du cours
  (FR23, « un jour de bourse » par défaut, réglable) — non implémenté.** An unknown symbol's notice
  names the app's ticker convention (`.DE` → `.XETRA`, `.AX` → `.AU`; `ingestion::ticker`). A
  provider listing currency different from the study's is **refused** before anything is applied.
  A listing quoted in hundredths (GBX, GBp → GBP; ZAc → ZAR; ILA → ILS) is **converted** at one
  place, `ingestion::subunit`, called by `fetch_canonical` and `fetch_price` before `normalize`:
  price figures ÷ 100 (exact decimal), `native_currency` = the major code, the provider's code in
  `FetchedFinancials.listing_subunit` → `Study.listing_subunit` (additive serde field; traceability
  line, and the price-only holdings refresh, whose bare quote states no currency, converts on it);
  statement figures are never divided (réconcilié 2026-10-01 : décision Guy, conversion des
  centièmes) — a statement amount without its own currency is relabelled to the major code, not
  divided. Holdings price-only rule: converted when the unit is known (`Study.listing_subunit`, or
  a study holding provider-sourced cells = fetched and served as is); **refused** when unknown on a
  hundredths venue (`ingestion::ticker::hundredths_venue` — `.LSE`/`.L`, `.JSE`/`.JO`, `.TA`; pure
  predicate `state::price_unit_unknown`): not fetched, nothing applied (price, stop ratchet, price
  history), named (`MSG_HOLDINGS_UNIT_UNKNOWN`). The conversion line (`report::LISTING_SUBUNIT_LINE`)
  is the same in the traceability, the study PDF and the quick screen (réconcilié 2026-10-03 : revue
  PR #291). Unit of the fundamentals of a hundredths listing — vérifié 2026-10-03 sur ULVR.LSE:
  `General.CurrencyCode` `GBX`, `/eod` close 4483.5 (pence); `Highlights.EarningsShare` 2.18 in GBP
  (the served `PERatio` 20.5665 = 44.835 GBP ÷ 2.18), dividend figures in GBP too → the trailing EPS
  is never divided; the statement rows state `currency_symbol` EUR (Unilever reports in euros). A
  London listing may thus report in a **third currency**: its statements are neither divided nor
  relabelled and take the mixed-currency path below (`currency_mismatch`,
  `FetchedFinancials.reported_currencies` → « comptes publiés en EUR, cotation en GBP »); pinned by
  the trimmed real extract `ingestion/tests/fixtures/eodhd-*-ULVR-real.json`.
  Statements reported in another currency than the listing are applied with a named
  **warning**. **Décision Guy 2026-10-01 (réconcilié 2026-10-03) : comptes publiés dans une autre devise que la cotation — acceptés avec l'avertissement nommé, jamais bloqués (NOVN, ABB : cotés en CHF à SIX) ; une conversion au taux moyen de chaque exercice reste à proposer en story.**
- **Errors:** `thiserror` 2.0 domain errors per crate; neutral, cause-named messages; **no silent
  `.ok()`** (explicit lesson from the prior project's chart-rendering bugs).

### Frontend Architecture

- **Slint 1.17 (GPLv3)** (réconcilié 2026-10-01 : `Cargo.lock`), declarative `.slint` + `slint-build`; thin UI over the calc core.
- **State & recompute (realizes the Foundational Invariant):** a single **immutable study-state
  snapshot** is the source of truth; the UI derives from it; recompute is **transactional and pure**
  (inputs + verdict born together); the verdict is **content-addressed** by `f(hash(inputs),
  method_version)`; an input change **invalidates** the dependent verdict (marked stale) rather than
  silently overwriting it. Undo/redo = snapshot stack (state is small → simple clones; structural
  sharing only if needed).
- **Charts native in Slint** (`Path` + `TouchArea`, log10 in Rust; <100 ms recolor trivial in Slint's
  dirty-driven retained mode). Week-1 spike is the go/no-go.
- **Theming:** design tokens (zone colours/ink/label set) live in **one neutral source of truth** read
  by the UI (pushed by a plain main-thread owner in `app/src/theme.rs` — no `arc_swap`, since Slint
  properties are main-thread; réconcilié 2026-10-01 : `app/src/theme.rs`); theme/regime change forces a redraw. Two token
  families: colour/alpha (free to swap) vs metric/typo (quasi-static, never during a drag).
- **i18n:** French-first `@tr()` strings in the `.slint` files (no separate `i18n.rs`), i18n-ready;
  separate axis from the NAIC↔neutral label set (réconcilié 2026-10-01 : `app/src`).
- **Verdict integrity in UI:** a `FullVerdict` is constructible only from all-validated-&-fresh
  load-bearing inputs (compiler-enforced); verdict + staleness derive from the same snapshot so an
  incoherent frame is structurally impossible.

### Infrastructure & Deployment

- **No cloud, no containers, no server.** Distribution = a native binary per OS (Win/macOS/Linux),
  built from the Cargo workspace — as of 2026-10-01 only the Linux binary is built, tested and used
  (réconcilié 2026-10-01 : `ci.yml`, revue de projet p1 NFR constat 1); updates manual in v1 (git pull/rebuild or replace binary). [P4]
  a second binary, `steadyinvest-mcp`, ships beside the app; it is a stdio child process of the AI
  client (not a daemon, not a network service), registered by the owner in that client.
- **CI:** `cargo test` gates (engine golden/property/metamorphic, versioned-journal corpus,
  marker-confusability snapshot) and the trust quality-gates block merges. **Linux-only for now**
  (decision 2026-06-09): cross-OS numeric identity is asserted by pinned determinism hashes
  (trivial under exact decimal); restore the 3-OS matrix when multi-OS support returns.
- **UI visual-verification strategy (decided — point 4):** the prior project shipped a blank chart
  marked "done" for 4 epics because nothing rendered it and looked. Therefore: **Slint render snapshot
  tests** for key surfaces (the chart, trust markers, verdict states), the **marker-confusability
  snapshot gate**, and a **Definition-of-Done rule = "launch the app and visually verify"** before any
  UI story is "done". Detailed in the Implementation Patterns step / TEA phase.
- **Backup/restore:** delegated to an external system (NAS sync) — the app keeps the journal as a
  single copy-friendly local file. As built (story 5.5 scope decision): backups are **manual**, raw
  `.db` copies (`VACUUM INTO`, named by journal_id/version/time) written to a `backups/` folder
  **beside the journal**; the app does not move the live DB out of a sync-watched folder — it
  switches it to `journal_mode=DELETE` and shows a warning recommending that layout (réconcilié
  2026-10-01 : `app/src/state/journal_io.rs`, story 5.5).
- **Quality-gate families (kept distinct):** *trust gates* (types/traceability/reproducibility/
  determinism) vs *posture gates* (neutral naming, banned-verb, swappable labels — not a string grep).
- **Observability:** `tracing` to a local rotating log; no network, no telemetry.

### Decision Impact Analysis

**Implementation Sequence:**
1. Cargo workspace + crate skeleton (`core`, `contract`, `ingestion`, `persistence`, `app`) + UI seed.
2. **Week-1 spikes:** (A) Slint dense grid + paste-a-column; (B) native Slint draggable judgment line
   + <100 ms recolor; (C) `rust_decimal` `maths` CAGR precision check + cross-OS determinism hash.
3. `core` calc engine (deterministic, `method_version`, named rounding) with golden/property/
   metamorphic tests.
4. `contract` versioned types + `persistence` (hybrid schema, journal_id, migrations, export format,
   corpus tests).
5. `ingestion` normalization layer + EODHD adapter (reqwest/tokio) + provider-failure handling.
6. `app` UI: faithful form/grid, charts, sticky verdict bar, trust markers, settings (no wizard),
   app-config (`directories`) + keychain (`keyring`), DB open/new/recent + sync-path detection.

**Cross-Component Dependencies:**
- `core` depends on nothing UI/IO — the assurance that makes the GUI choice reversible.
- `contract` is consumed by `persistence`, `app`, and [P4] `mcp` (the draft-writing MCP process,
  over the narrow `McpAccess` persistence surface) — its `schema_version` + `method_version`
  discipline gates migrations.
- The Foundational Invariant cuts across `core` (content-addressed verdict), `persistence` (frozen
  judgments + identity), `ingestion` (provenance/freshness), and `app` (no incoherent frame).
- FX consolidation sits only at the portfolio aggregation layer — `app/src/state` calling the pure
  `core::risk::fx::convert` over rows read by `persistence` — never in `core`'s native-currency study
  calc (réconcilié 2026-10-01 : `app/src/state/{fx,concentration,review}.rs`, `persistence/Cargo.toml` sans `core` — p2 écart 17).

## Implementation Patterns & Consistency Rules

### Pattern Categories Defined

**Critical Conflict Points Identified:** ~11 areas where independent dev agents could diverge —
crate/package naming, Rust vs SQLite vs Slint naming axes, decimal storage, JSON contract field case,
error-type shape, state-update model, logging, the calc-location ("Cardinal Rule"), the Slint
view-model boundary, the time/ID source, and the i18n-vs-label-set split.

### Naming Patterns

**Workspace & crates:** package names `steadyinvest-core`, `steadyinvest-contract`,
`steadyinvest-ingestion`, `steadyinvest-persistence`, `steadyinvest-report`, `steadyinvest-app`,
`steadyinvest-paths`, [P4] `steadyinvest-mcp` (réconcilié 2026-10-01 : `Cargo.toml`); directory names short
(`core/`, `contract/`, …); internal refs via `[workspace.dependencies]` (single source of versions).

**Rust code (rustfmt + clippy enforced):** types/traits `PascalCase`; fns/vars/modules/files
`snake_case`; consts/statics `SCREAMING_SNAKE_CASE`; one module = one file/dir, organized **by
domain** (no `utils.rs` grab-bag — shared helpers in a named module).

**SQLite (persistence crate):** tables `snake_case` **plural** (`portfolios`, `holdings`,
`transactions`, `fx_rates`, `watchlist_items`, `studies`, `judgments`, `price_history`, [P4]
`ai_drafts`); columns `snake_case`; PK
`id`; FKs `<entity>_id`; indexes `idx_<table>_<cols>`; timestamps `TEXT` RFC3339 UTC; **monetary/
decimal values stored as `TEXT` decimal strings** (NOT `REAL` — preserves `rust_decimal` exactness;
`REAL` would silently lose precision and breach the no-float rule).

**Slint (app crate, Slint idiom):** components `PascalCase`; `.slint` files `snake_case`; properties
& callbacks **`kebab-case`** (e.g. `current-price`, `judgment-moved`); exported globals `PascalCase`
(e.g. `Tokens`, `Strings`); Rust↔Slint callbacks named `verb-noun`.

### Structure Patterns

- **Unit tests** co-located in `#[cfg(test)] mod tests`.
- **Integration tests** in each crate's `tests/`. Golden + property + metamorphic fixtures in
  `core/tests/{golden,fixtures}/`; ingestion recollage fixtures in `ingestion/tests/fixtures/`;
  **frozen versioned-journal corpus** in `persistence/tests/corpus/v{N}.db` (append-only, never edited).
- **No business/calc logic outside `core`** (Cardinal Rule). UI components consume state and render;
  services orchestrate; the engine computes.
- Tooling tasks (build/release/lint) in a `justfile` (chosen over `xtask` for simplicity).

### Format Patterns

- **Data contract = serde JSON, `snake_case` field names** (Rust default; the [P4] MCP process is
  also Rust-side and reuses the contract types). `#[serde(default)]` on every new field; **never
  `deny_unknown_fields`** on the `Study` blob or on per-entity export items (forward-compat: an
  unknown field is tolerated). **Exception, recorded as built:** the export **envelope**
  (`JournalSnapshot`, `StudyRecord` — `persistence/src/export.rs:39-66`) **IS**
  `deny_unknown_fields`, deliberately and at envelope level only (#78, 2026-07-08): a newer file
  adding a whole entity **array** is rejected loudly by an older build rather than partially
  imported. [P4] `ai_drafts` is an additive array (`#[serde(default, skip_serializing_if =
  "Vec::is_empty")]`); because notes and AI marks are **fields** inside `Study` (which tolerates
  unknown fields), an older build would drop them — accepted while the app is not in production
  (§Phase 4, A5; D9 withdrawn 2026-09-27).
- **Versions:** `schema_version` = integer; `method_version` = string (semver-like).
- **Decimal in JSON:** serialized as a **string** (exact), parsed to `rust_decimal::Decimal`.
- **Dates/times:** RFC3339 UTC strings everywhere (storage, export, logs).
- **Enums:** `#[serde(rename_all = "snake_case")]`, internally tagged where a discriminant is needed
  (cell `source`: `provider|manual|derived`; `review`: `none|to_review|validated`).
- **Booleans** as JSON booleans; tri-state review is an enum, never `0/1/2`.

### Communication Patterns

- **State management = immutable snapshots** (Foundational Invariant). An action produces a new
  `StudyState` snapshot; the verdict is derived, content-addressed `f(hash(inputs), method_version)`;
  an input change **invalidates** dependents (marked stale), never silently overwrites. Undo/redo =
  snapshot stack.
- **Slint view-model boundary:** `core`/`contract` domain types are **never** passed directly into
  `.slint`. A per-screen **adapter layer** maps domain types → generated Slint structs; collections
  cross via `ModelRc`/`VecModel`. Since **Slint has no `Decimal`**, money crosses the boundary as
  **already-formatted, locale-aware strings** (named rounding applied) — never an `f32`/`f64`.
- **Cross-thread:** provider fetch runs on the tokio worker; results return to the Slint loop via
  `slint::invoke_from_event_loop`; never touch UI state off the main thread.
- **Logging (`tracing`):** structured fields (not string-interpolated); levels — `error`/`warn`
  (degraded/stale/plausibility)/`info`/`debug`/`trace`; spans around fetch and recompute. **Never log
  secrets/keys or full journal payloads.**

### Process Patterns

- **Time & identity:** a single injected **`Clock`** and **`IdGen`** (traits) — no scattered
  `Utc::now()`/UUID calls. Tests inject a fixed clock → deterministic golden/property results and
  reproducible `journal_id`/timestamps.
- **Error handling:** per-crate `Error` enum via `thiserror` + a `Result<T>` alias; `anyhow` only at
  the `app` edge. **No `.unwrap()`/`.expect()`** in non-test code (except a documented `// INVARIANT:`);
  **no silent `.ok()`** (the prior project shipped a blank chart this way). Errors bubble via `?` to a
  boundary that maps them to a **neutral, cause-named** banner message.
- **Loading/offline:** async ops set an explicit loading state and **never block the UI**; offline is
  normal; provider failure → classified banner + stale flagging + retained last-known values.
- **Validation:** strong types at the ingestion boundary; plausibility issues are **non-blocking
  warnings**; `unknown/insufficient` is first-class, never coerced to `0`.

### i18n & Labels (two distinct mechanisms — never mixed)

- **UI strings:** Slint **`@tr()`** (compile-time translation, gettext), **French first**, i18n-ready.
- **NAIC↔neutral label set:** a **runtime-swappable data table** (not a translation) — the
  domain/method labels the user can switch; lives in data, loaded at runtime, distinct from `@tr()`.

### Enforcement Guidelines

**All dev agents MUST:**
- Put **every calculation** in `steadyinvest-core` — never duplicate calc in UI/elsewhere (**Cardinal
  Rule**; the auditability/trust guarantee).
- Use `rust_decimal` for money/ratios; **never `f32`/`f64`** in the decision chain.
- Keep `steadyinvest-core` free of any I/O, UI, SQLite, or network dependency.
- Obtain time and IDs only via the injected `Clock`/`IdGen`.
- Route every persisted/derived value through `(source, logical_version, timestamp,
  hash_of_dependencies)`; never present a fact without its proof.
- Add a migration + a frozen corpus fixture whenever a persisted struct changes (schema-drift gate).
- Map domain types to Slint via the adapter layer; pass money as formatted strings, never floats.
- Pass `cargo fmt --check`, `cargo clippy -- -D warnings`, and the trust quality-gates in CI; **launch
  the app and visually verify** any UI story (Definition of Done).

**Pattern enforcement:**
- `rustfmt.toml` + `clippy.toml` at workspace root; CI denies warnings.
- Trust gates (golden/property/metamorphic/determinism/corpus/confusability) block merge.
- Pattern violations and deferred items → **GitHub Issues** (single source of truth), not inline TODO
  debt tables.

### Pattern Examples

**Good:**
- `studies` row: `id`, `journal_id`, `security_ticker`, `created_at` (TEXT RFC3339), `status`,
  `schema_version`, `method_version`, `payload` (TEXT JSON); money field `avg_cost` = `"123.4500"`.
- `core::ssg::forecast_high_price(inputs) -> Decimal` — pure, deterministic, golden + property tested.
- Provider error → `IngestionError::Quota { provider, retry_after }` → neutral banner.
- Dashboard sort by price: done in Rust, **or** an auxiliary `REAL` sort-key column used **for
  ordering only**, never for any calculation.

**Anti-patterns (forbidden):**
- A P/E or zone recomputed inside a Slint callback / the UI crate (violates the Cardinal Rule).
- Storing a price as SQLite `REAL`; `let _ = renderer.render().ok();` swallowing an error.
- Passing a domain struct or an `f64` money value straight into `.slint`.
- A verdict in full colour while a load-bearing input is unvalidated/stale.
- Coercing a missing cell to `0`; camelCase JSON fields; `unwrap()` on provider/IO results;
  scattered `Utc::now()`/UUID generation.

## Project Structure & Boundaries

### Complete Project Directory Structure

*As built on `main` (réconcilié 2026-10-01 : arborescence régénérée depuis le dépôt — p2 écarts 13, 15, 16 ; the June 2026 plan
it replaces is in git history). Test-only and spike files are abridged.*

```text
steadyinvest/
├── Cargo.toml                     # [workspace] 8 members + [workspace.dependencies] (single version source)
├── Cargo.lock                     # committed (application → reproducible builds)
├── rustfmt.toml · clippy.toml     # formatting / lint config (CI: fmt --check, clippy -D warnings)
├── justfile                       # tooling tasks (run, ci, mcp-build, …)
├── rust-toolchain.toml            # pins 1.96 (MSRV driven by Slint transitive deps + libsqlite3-sys 0.38)
├── deny.toml                      # cargo-deny: GPL-3.0 dependency-license audit
├── README.md · LICENSE (GPL-3.0) · .gitignore
├── .github/workflows/ci.yml       # fmt, clippy, test, trust gates, cargo deny — Linux only (ubuntu-latest)
├── docs/                          # NAIC reference PDFs, method spec (docs/method/), guide-ia.md, process docs
│
├── core/                          # steadyinvest-core — PURE calc engine (NO I/O, UI, SQL, net); deps: rust_decimal (+maths), serde, sha2
│   ├── src/
│   │   ├── lib.rs                 # + determinism hash test
│   │   ├── ssg/                   # §1–§5 (growth, management, valuation, risk_reward, return_proj, types); shown_quality_flags
│   │   ├── normalize/             # IFRS↔GAAP, splits, fiscal-period, currency checks (pure; reused by manual entry AND providers)
│   │   ├── golden/                # golden-fixture schema + pure check/compare
│   │   ├── verdict/               # FullVerdict (gates.rs, digest.rs) — constructible only from validated+fresh inputs
│   │   ├── method/                # load-bearing catalog + numeric thresholds
│   │   ├── quality_flags.rs       # FR7 / FR10 key catalog
│   │   ├── checklist.rs
│   │   ├── risk/                  # capital-at-risk, stops, concentration, ledger, fx::convert (FR28, FR36-48)
│   │   ├── rounding.rs            # named rounding mode + per-field display scale
│   │   └── method_version.rs      # METHOD_VERSION (ssg-1.2.0)
│   └── tests/                     # golden/ (g01–g11), golden_*, ssg_*, normalize_*, verdict_*, spike_c (no benches/)
│
├── contract/                      # steadyinvest-contract — versioned serde data contract; deps: serde, serde_json, rust_decimal, uuid, sha2
│   └── src/                       # study.rs, cell.rs (incl. the reconciliation primitive — no ingestion/reconcile.rs),
│                                  # provenance.rs, money.rs, text.rs, versioning.rs, export.rs,
│                                  # [P4] ai.rs, draft.rs, draftable.rs, frozen.rs (FrozenVerdict)
│                                  # (portfolio / holding / transaction / FX row types live in persistence, not here)
│
├── ingestion/                     # steadyinvest-ingestion — providers (FR15-16,21-27); deps: contract, core (normalize), reqwest, rustls, tokio, serde_json, sha2, thiserror, chrono
│   ├── src/                       # lib.rs, provider.rs, fetch.rs, ticker.rs (venue table, convention_suggestion), error.rs,
│   │                              # adapters/{eodhd,twelvedata,common}.rs
│   └── tests/                     # eodhd_mapping.rs + fixtures/
│
├── persistence/                   # steadyinvest-persistence — rusqlite storage = ROWS only (no core dependency); deps: contract, rusqlite, serde_json, sha2, thiserror, uuid
│   ├── src/
│   │   ├── journal.rs             # open/create; journal_id + logical version; DELETE mode; single-instance lock sidecar; backup_to
│   │   ├── schema.rs · migrations.rs   # tables; PRAGMA user_version steps v1…v9
│   │   ├── studies.rs · holdings.rs · transactions.rs · watchlist.rs · fx.rs · price_history.rs
│   │   ├── export.rs              # JSON export/import envelope (FR59-60)
│   │   ├── restore.rs             # backup inspection + restore (marker, staging; FR61, A11)
│   │   ├── drafts.rs              # [P4] ai_drafts (v8, v9) + decide_draft
│   │   ├── mcp_access.rs          # [P4] McpAccess: per-call connections, SQLite authorizers, POSIX identity check (unsafe FFI)
│   │   └── error.rs · util.rs
│   └── tests/                     # corpus/ (v1.db, v8.db), e2e_lifecycle, export, mcp_access, inbox_polling, drafts, …
│
├── report/                        # steadyinvest-report — study → core mapping + PDF (FR52-53); deps: core, contract, rust_decimal, pdf-writer
│   └── src/                       # form.rs (build_snapshot, freeze), frozen.rs (figé vs actuel), pdf.rs, comparison.rs, review.rs, quick_screen.rs
│
├── paths/                         # steadyinvest-paths — per-machine locations + dossier resolution (A10); deps: directories, serde, serde_json
│   └── src/lib.rs                 # never writes
│
├── mcp/                           # [P4] steadyinvest-mcp — stdio MCP server binary (§Phase 4)
│   ├── Cargo.toml                 # deps: contract, persistence (McpAccess only), core, report, paths, rmcp (server, transport-io), tokio, uuid, chrono, rust_decimal, serde, tracing; NEVER ingestion/reqwest/keyring/slint/app
│   ├── clippy.toml                # disallowed types (no Journal)
│   ├── src/                       # main.rs (args, --dossier), server.rs (hand-written rmcp ServerHandler), tools.rs, dto.rs, messages.rs, seed_guard.rs, logging.rs
│   └── tests/                     # closure.rs (dependency closure), non_exposure.rs, stdio_e2e.rs, home_isolation.rs
│
└── app/                           # steadyinvest-app — thin Slint UI (binary)
    ├── build.rs                   # slint-build
    ├── assets/                    # fonts/ (Inter, IBM Plex Sans, OFL); golden/ (g01–g05 — the demo study is g01-worked-example, FR62)
    ├── examples/                  # spike_a_grid.rs, spike_b_chart.rs
    ├── src/
    │   ├── main.rs · clock.rs · logging.rs · config.rs · keychain.rs · provider.rs · fetch.rs
    │   ├── labels.rs              # NAIC↔neutral label set (runtime-swappable data)
    │   ├── theme.rs               # token pushes to UI (plain owner — no arc_swap)
    │   ├── regime.rs · posture.rs (neutrality gates) · seam_check.rs
    │   ├── state/                 # JournalState rails: studies, cells, refresh, undo (park/restore), frozen, confront,
    │   │                          # drafts, notes, holdings, ledger, fx + concentration + review (consolidation, with core::risk),
    │   │                          # watchlist, replacement, export_import, restore, journal_io (sync-folder detection, backups/), messages
    │   ├── viewmodel/             # ADAPTER: domain → Slint structs (engine, chart, ai_lines, frozen, history, drafts, verify (demo), …)
    │   └── wiring/                # Slint callback wiring per surface (cells, fetch, drafts, overlays, holdings, …)
    └── ui/                        # app.slint, state.slint, tokens.slint (no separate i18n.rs: @tr() in .slint)
        ├── components/            # growth_chart, pe_history_chart, zone_bar, verdict_badge, trust_markers, editable_cell,
        │                          # modal_dialog, frozen_verdict_strip, confront_overlay, ai_frame, ai_judgment, choice_chip, …
        └── screens/               # dashboard, study_screen, watchlist, portfolio, review, propositions,
                                   # comparison, quick_screen, settings
```

*Test fixtures:* each crate owns its fixtures for now (`core/tests/`, `ingestion/tests/`,
`persistence/tests/corpus/`); a shared dev-only synthetic-fixtures crate can be mutualised later if
drift appears.

### Architectural Boundaries

- **Calc boundary (Cardinal Rule):** `core` has zero I/O/UI/SQL/net deps — all SSG/risk math lives
  here and nowhere else. Guarantees the GUI choice stays reversible and the math is auditable.
- **Contract boundary:** `contract` is the only shared vocabulary across `ingestion`, `persistence`,
  `report`, `app` and [P4] `mcp`. Its `schema_version`/`method_version` gate migrations.
- **Persistence boundary:** only `persistence` touches SQLite. Decimal arithmetic for consolidation is
  done in Rust — `persistence` returns rows (it does not depend on `core`), `app/src/state` computes
  with `core::risk` — never via SQL on TEXT money columns (réconcilié 2026-10-01 : `app/src/state/{fx,concentration,review}.rs`, `persistence/Cargo.toml` sans `core` — p2 écart 17). [P4] `mcp`
  reaches SQLite only through `persistence::McpAccess`, never `Journal` (clippy `disallowed-types`
  in the `mcp` crate).
- **UI boundary:** `app` is the only crate depending on Slint; domain types cross into `.slint` solely
  through the `viewmodel/` adapter (money as formatted strings, no floats, no domain structs leaked).
- **Network boundary:** only `ingestion` makes network calls (reqwest/tokio); keys are injected by
  `app` (from keyring), never read inside `ingestion` — keeps it testable offline. [P4] the `mcp`
  crate's dependency closure **excludes** `ingestion`, `reqwest`, `keyring`, `slint` and `app` —
  asserted by a CI test over `cargo metadata` (NFR-A3 by construction: no provider call, no key is
  even linkable from the MCP binary).
- **External-interface boundary:** no network server, no public network API. [P4] the **only
  external interface** is the local stdio MCP server (`steadyinvest-mcp`), not network-exposed,
  spawned by the owner's AI client; the contract is its vocabulary.

### Requirements to Structure Mapping

| FR cluster | Primary location |
|---|---|
| FR1-8 Stock Study & engine | `core/ssg/`, `core/verdict/`, `contract/study.rs`, `app/ui/study_screen.slint` |
| FR9-14 Calc integrity & trust | `core` (+ `tests/golden`,`properties.rs`), `app` verdict rendering |
| FR15-29 Acquisition/provenance/providers | `ingestion/`, `contract/{cell,provenance,fx}.rs`, `persistence` cache |
| FR30-33 Charts & judgment | `app/ui/components/{growth_chart,pe_history_chart,zone_bar}.slint`, `app/src/state/`, `app/src/viewmodel/chart.rs` |
| FR34-35 Watchlist & alerts | `persistence` (watchlist), `app/ui/screens/watchlist.slint` |
| FR36-41 Portfolio/transactions | `persistence/{schema,holdings,transactions,fx}.rs`, `app/src/state/{holdings,ledger,fx,concentration,review}.rs`, `core/risk/` |
| FR42-48 Risk management | `core/risk/`, `app/ui/screens/portfolio.slint` |
| FR49-51 Cumulative memory/journal | `contract/{study,provenance}.rs`, `persistence/journal.rs` |
| FR52-53 Reporting/PDF | `report/` |
| FR54-62 App shell & data mgmt | `app/ui/screens/dashboard.slint`, `persistence/{export,restore}.rs`, `app/src/state/{export_import,restore,journal_io}.rs`, `app/config.rs` |
| FR63-66 Config/posture | `app/src/{config,keychain,labels,posture}.rs`, `app/ui/screens/settings.slint` |
| Added: DB location + recent journals | `app/config.rs`, `paths/`, `persistence/journal.rs`, `app/src/state/journal_io.rs` (sync-folder detection) |
| FR69-77 AI assistance [P4] (+FR14, FR33) | `mcp/`, `persistence/{mcp_access,drafts}.rs`, `contract` (Draft/AiOrigin/DraftTarget, `Provenance.ai_origin`, `Judgment.ai_placed`), `app` draft inbox + AI frame + AI-annotated chart line |
| FR78 Study notes [P4] | `contract/study.rs` (`Study.notes`), `app` study screen |

### Integration Points

- **Internal:** `app` orchestrates; reads/writes via `persistence`; computes via `core`; fetches via
  `ingestion`; renders PDF via `report`. All data shapes are `contract` types. Cross-thread results
  return through `invoke_from_event_loop`.
- **External:** market-data providers (HTTP, user's key) via `ingestion` adapters only; OS secret
  store via `keyring`; OS config dirs via `directories`; external backup target (NAS) via file export;
  [P4] the owner's AI client via the local stdio MCP server (`mcp` → `persistence::McpAccess`) — the
  only external interface, reads of studies + draft inserts only.
- **Data flow:** provider → `ingestion` (normalize via `core`) → `contract` types (provenance stamped;
  reconciliation by the `contract` cell primitive, applied in `app/src/state/refresh.rs`) →
  `persistence` (journal) → `core` recompute (native currency) → `app` viewmodel → Slint render;
  consolidation/FX applied only at the portfolio layer (`app/src/state` + `core::risk`)
  (réconcilié 2026-10-01 : p2 écarts 13 et 17).

### Development Workflow Integration

- **Dev:** `just run` (app), `just spike` (week-1 chart spike), `just test`, `just lint`.
- **Build:** `cargo build --release` per OS produces a single native binary; `Cargo.lock` committed.
- **CI/Deploy:** `.github/workflows/ci.yml` runs fmt, clippy -D warnings, tests, trust gates,
  determinism hash, `cargo deny` license audit — **Linux-only for now** (decision 2026-06-09;
  3-OS matrix returns later, determinism meanwhile asserted via pinned hashes); distribution =
  the per-OS binary (manual update in v1).

## Phase 4 — AI Assistance over MCP (G2, 2026-09-27)

_Scope: Epic 8 [P4], PRD FR14, FR33 [P4], FR67/FR68 [P4], FR69–FR78, NFR-S1–S4, NFR-R2, NFR-A1–A4.
No AI runs inside the app: the owner's AI client (currently Claude Code on the workstation) reads
studies and submits drafts through a local MCP server; the owner validates or rejects each draft in
the UI. Owner decisions O1–O7 (G2 round 2) and D1–D11 (G2 round 3, after the G3 review of PR #255)
and architecture decisions A1–A13 below are final._

### Owner decisions this section implements

- **O1** — the AI also reads each study's **computed outputs** (zones, upside/downside ratio, 5-year
  potential, verdict and its state), needed for search objectives such as U/D.
- **O2** — the MCP server works while the app is **closed**; drafts wait and appear at the next opening.
- **O3** — the "active dossier" is the **last-used** dossier, even with the app closed; every MCP
  response names the dossier (`journal_id` + path) it read.
- **O4** — a **stale** draft can still be validated after an explicit confirmation, or rejected.
- **O5** — validating a draft on a validated (✓) cell needs no prior un-validation; the cell moves to `?`.
- **O6** — deleting a note removes it from the study; it remains in the study history (FR51).
- **O7** — deleting a study deletes its drafts too (like its judgment history).
- **D1** — a UX pass (Story 8.0) specifies every Epic 8 surface and its French wording before the UI
  stories.
- **D2** — a draft study carries a proposed native currency and an optional company name; duplicates
  are keyed by (identifier, currency), identifier compared case-insensitively, re-checked at
  validation; validating opens the ordinary create-study dialog prefilled (2 actions: *Valider* →
  *Créer*).
- **D3** — a validation goes through the app's study state and its undo stack; an undone validation
  is recorded as « validé puis annulé ».
- **D4** — one pending draft per target; a second is refused at submission.
- **D5** — a validated value always gets review tag `?` (also on an untagged cell, also when the value
  is unchanged).
- **D6** — provider market facts (`current_price`, `ttm_eps`) are not draftable judgment fields.
- **D7** — stories 8.2 and 8.5 are split in two (8.2a/8.2b, 8.5a/8.5b).
- **D8** — a second draft study for a security already pending is refused (confirmed).
- **D9** — *withdrawn 2026-09-27*: the app is not in production, so no version bump or compatibility
  work for older builds or existing data; new fields are additive `#[serde(default)]` (A5).
- **D10** — the dossier is resolved **per call**; each submission carries the `journal_id` + path the
  AI read and is refused on mismatch.
- **D11** — FR68: the verdict is frozen by an explicit « Valider l'étude » on a full verdict; any
  later difference between frozen and current verdict is highlighted (A13, Story 8.8).

### A1 — Separate binary crate, and its dependency boundary

- **Decision:** the MCP server is a separate binary crate **`steadyinvest-mcp`** (`mcp/`), speaking
  MCP over **stdio**, launched by the AI client as a child process. Allowed dependencies:
  `contract`, `persistence` (through `McpAccess` only, A3), `core` and `report` (for computed
  outputs, O1), `paths` (dossier resolution, A10), the MCP SDK (`rmcp`), `tokio` (stdio runtime),
  `uuid`, `chrono`, `rust_decimal`, serde/tracing (réconcilié 2026-10-01 : `mcp/Cargo.toml`). Its **dependency closure excludes** `ingestion`,
  `reqwest` (and any HTTP client), `keyring`, `slint` and `app` — a CI test walks `cargo metadata`
  and fails on any of them.
- **Rationale:** NFR-A3 ("no provider call reachable from MCP") and NFR-S1 (keys never in MCP
  responses) then hold **by construction**: the code that could fetch or read a key is not linkable.
  A separate process also keeps the GUI free of any server loop and lets the MCP server run with the
  app closed (O2).
- **Computed outputs (O1):** the `Study → StudySnapshot` construction is **not in `core`** (which
  deliberately does not depend on `contract`) but in `report::form::build_snapshot` (the single
  construction shared by the live form and the PDF since Story 5.6). `mcp` therefore depends on
  `report` + `core` and calls that same `build_snapshot` — one construction, no drift from the
  screen. Checked 2026-09-27: `core` depends only on `rust_decimal`, `serde`, `sha2`; `report` adds
  `contract`, `rust_decimal`, `pdf-writer` — no network, no keychain, no GUI. Presentation formatting
  (`app::viewmodel::engine`) is **not** reused: MCP returns raw decimals as strings plus the verdict
  state, and must never re-derive a value (Cardinal Rule). The quality flags it returns follow the
  same presentation rule as the screens — `core::ssg::shown_quality_flags` (the highest high-P/E
  threshold only), shared by `app` and `mcp` — and `computed.quality_flags_assessable` tells an empty
  list meaning « none » from « not assessable » (réconcilié 2026-10-01 : PR #285).

### A2 — Per-call connection, no lock, version gate

- **Decision:** each tool call opens the dossier, does its work, and closes it. The MCP process
  **never takes the app's single-instance lock** (the `-lock` sidecar), **never migrates**, never sets
  `journal_mode`, and sets only `busy_timeout` and `foreign_keys = ON`. It **refuses to run** unless
  the file's `PRAGMA user_version` **equals** its build's latest migration (older file → "open it in
  the app first"; newer file → "this MCP build is older than the dossier"). Reads run in **one short
  read transaction** on a `SQLITE_OPEN_READ_ONLY` connection (a WAL snapshot, or a brief shared lock
  in DELETE mode on sync paths); responses are bounded (lists paged) so no read holds a lock long.
- **Sidecars:** on a closed WAL dossier, a read-only open creates an empty `-wal` and a `-shm` it
  cannot remove. This is expected and harmless; the app's sidecar and write-protection diagnostics
  (#67, `SidecarNotWritable`) must treat an empty `-wal` left by a reader as normal.
- **Rationale:** the lock exists to stop a second *app* instance, and taking it would make MCP
  unusable while the app is open (and the app unusable while MCP runs). SQLite's own locking already
  serialises the two writers; per-call connections hold nothing between calls, so the app's writes,
  migrations and backups are never blocked for longer than one short transaction. The equality gate
  means an MCP build never interprets a schema it does not know, and never upgrades a file behind
  the owner's back.

### A3 — `McpAccess`: capability asymmetry enforced by the SQLite engine

- **Decision:** a persistence-level type **`McpAccess`** (not `Journal`) is the only persistence
  entry point `mcp` gets, and it exposes **typed methods only** (`list_studies`, `read_study`,
  `read_history`, `list_drafts`, `insert_draft`…) — **never a connection**: whoever holds a
  `rusqlite::Connection` can remove its authorizer. Inside, it uses (i) a **read-only connection**
  and (ii) a **draft connection**, each with a SQLite **authorizer**:
  - **reads — allowlist:** only `studies`, `judgments`, `journal_meta`, `ai_drafts` and SQLite's
    internal tables may be read; every other table — today `holdings`, `transactions`, `portfolios`,
    `watchlist_items`, `fx_rates`, `price_history`, and any table a later migration adds — is
    `SQLITE_DENY`. A CI test lists every table in `sqlite_master` of the latest schema and fails if
    one is not classified (allowed or denied on purpose), so a new table can never be readable by
    default;
  - **writes:** the draft connection allows only `INSERT` into `ai_drafts`. The dossier's
    `logical_version` is bumped by a trigger created in migration v8 (`AFTER INSERT ON ai_drafts`);
    the authorizer allows `UPDATE journal_meta` **only** when its trigger-name argument is that
    trigger, so no direct `UPDATE` can set the counter to an arbitrary value (the stale-restore
    signals depend on it). Every other write (UPDATE/DELETE on any table, DDL, `ATTACH` — which
    also covers `VACUUM INTO` — and `PRAGMA` after setup) is **denied at statement preparation and
    logged**. A second trigger, `trg_ai_drafts_refuse_existing_id` (migration **v9**, `BEFORE INSERT`),
    aborts any insert whose id already exists, so an `INSERT OR REPLACE` — whose conflict clause the
    authorizer cannot see — can never overwrite a decided draft (réconcilié 2026-10-01 :
    `persistence/src/schema.rs` `migrate_to_v9`, story 8.3 G3).
  Deny, never `SQLITE_IGNORE` (which would silently read NULLs — no silent `.ok()`). The authorizer
  needs rusqlite's `hooks` feature.
- **Crate boundary:** in the `mcp` crate, clippy `disallowed-types` forbids `Journal` and
  `disallowed-methods` forbids `persistence`'s free functions that touch the file
  (`restore_journal_file`, `clear_lock`, `inspect_backup`). A crate-level `mcp/clippy.toml`
  **replaces** the workspace `clippy.toml` rather than merging with it, so it repeats the workspace
  settings.
- **Submission checks:** `insert_draft` runs its checks and the insert in **one `BEGIN IMMEDIATE`
  transaction**, so a concurrent `delete_study` cannot leave an orphan draft. It refuses, with a
  named reason and nothing written:
  - a dossier identity mismatch — the submission carries the `journal_id` and path the AI read (D10);
  - a missing target study; an empty comment; a missing origin (NFR-A4);
  - an invalid target: the `field` must be one of the enumerated draftable fields (cell fields of the
    study grid; judgment fields except `current_price` and `ttm_eps`, D6), the fiscal year must be a
    year of the study (a draft never adds a year row), and the value must parse as a decimal in the
    field's unit (percent fields as percent, e.g. `12` for 12 %) or, for an enum field such as
    `forecast_low_option`, as one of its variant names — the tool schema lists fields and units;
  - a second pending draft on the same target (D4), or a draft study for a security already studied
    (archived studies included) or pending in the same currency (D2, D8);
  - a note, cell or judgment draft on an **archived** study (`study_archived`);
  - an over-long text or out-of-range value: comment and note ≤ 10 000 characters, company name ≤
    200, origin client/model ≤ 100, `proposed_value` ≤ 100 characters, a number with |value| < 10¹⁵
    and at most 10 decimals (`text_too_long`, `value_out_of_range`, `empty_note_text`);
  - a `draft_id` already used with **another** content (`draft_id_conflict`): the submission may
    carry an optional `draft_id`, and re-submitting the same draft with the same id is
    **idempotent** (no second row, no refusal).
  (réconcilié 2026-10-01 : `persistence/src/mcp_access.rs` `SubmissionRefusal`, `MAX_*_CHARS` —
  story 8.3 décisions 1, 7, 10, 11 ; story 8.4 écart 2)
- **Rationale:** NFR-A1/A2 demand "by construction, not by prompt". Filtering at the query-writing
  layer would be one missed `WHERE` away from a leak; the authorizer is enforced by the engine on
  every statement, including ones written later by someone who never read this section, and the
  allowlist fails closed.
- **Test suites (CI):** whole-surface non-exposure (every tool response over stdio: no portfolio,
  watchlist, key or config data — the dossier identity excepted, NFR-S4); table classification
  (above); rejected writes (each forbidden statement fails and is logged, including a direct
  `logical_version` update); submission refusals (each case above); 100% of drafts carry comment +
  origin (backed by DB `CHECK` + `NOT NULL`); metamorphic — every engine output is identical with and
  without pending drafts, and the engine never reads `ai_drafts`.

### A4 — Drafts: `ai_drafts` table (migration v8, guarded by v9)

- **Decision:** a new table `ai_drafts` in the **same SQLite file**, migration **v8** (latest before
  it: v7), hybrid pattern. **Migration v9** (Story 8.3 G3) adds only the trigger
  `trg_ai_drafts_refuse_existing_id` (A3); the latest schema is therefore **v9** and the A2 version
  gate compares against v9 (réconcilié 2026-10-01 : `persistence/src/migrations.rs` `REGISTRY`).
  Columns:
  - `id`, `kind` (`study|note|cell|judgment`), `study_id` (NULL for a draft study),
    `security_ticker`, `native_currency` (draft study), `status`
    (`pending|validated|validated_undone|rejected`), `created_at`, `decided_at`;
  - `comment` `NOT NULL CHECK (length(trim(comment)) > 0)`, `origin_client` and `origin_model`
    `NOT NULL` (NFR-A4);
  - decision-time facts: `stale_at_decision`, `edited_before_validation` (both nullable booleans),
    `created_study_id` (the study a validated draft study became — for the O7 cascade and the
    history view);
  - a versioned JSON `payload`: `DraftTarget { Cell { fiscal_year, field } | Judgment { field } }`,
    the proposed value or note text, the draft study's optional company name, and
    `base_fingerprint`.
  Indexes on `status`, `study_id`, `created_study_id`. **All enum variants are defined up front** —
  adding a variant later costs a `SCHEMA_VERSION` bump.
- **Backup/export:** included in the `VACUUM INTO` backup automatically. The JSON export gains an
  `ai_drafts` array (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`); an older build
  refuses it through the envelope's `deny_unknown_fields` (#78). Frozen corpus gains `v8.db` (no
  `v9.db` was added — the corpus holds `v1.db` and `v8.db`; réconcilié 2026-10-01 :
  `persistence/tests/corpus/`).
- **Rationale:** the same file keeps drafts inside the dossier's identity, backup and export (FR77:
  durable record); a separate table keeps pending proposals physically outside the `Study` blob the
  engine reads, which is what makes "a pending draft changes nothing" true by construction.
- **Cascade (O7):** `delete_study` also deletes the drafts whose `study_id` **or**
  `created_study_id` is that study, in the same transaction (as it already deletes `judgments`); a
  pending draft study is untouched.

### A5 — Study notes in the `Study` blob (FR78)

- **Decision:** `#[serde(default)] notes: Vec<Note { id, text, created_at, updated_at, ai_origin:
  Option<AiOrigin> }>` inside `Study` (`AiOrigin` as defined in A6). Additive in storage — no
  `user_version` migration.
- **Compatibility:** none beyond the additive `#[serde(default)]` — no `SCHEMA_VERSION` bump, no
  re-stamp, no relaxed import (owner, 2026-09-27: « steadyinvest n'est pas en production : pas besoin de migrer l'existant »). An older build would drop
  notes silently; accepted while the app is not in production (D9 withdrawn).
- **Rationale:** export, MCP read and history snapshots come for free (O6: a deleted note stays in
  the study history). A note is study content, not an aggregated/queried entity.

### A6 — AI origin after validation: no new `Source` variant

- **Types:** two distinct types in `contract`:
  - `DraftOrigin { client, model }` — who submitted a draft (A4 columns);
  - `AiOrigin { draft_id, client, model, validated_at }` — carried by a value, a judgment field or a
    note once validated.
- **Decision:** a validated draft **is an owner entry**: `Source::Manual`, review `?` **set
  explicitly** by the decision (D5 — a documented exception to the manual-edit rail, which keeps `✓`
  on an unchanged value and never promotes `None`), reconciled as manual (FR22/FR74; O5). Its AI
  origin is carried by `#[serde(default)] ai_origin: Option<AiOrigin>` on `Provenance` — cleared by
  the next owner edit, which replaces the provenance — and by one sidecar on `Judgment`,
  `#[serde(default, skip_serializing_if = "AiPlaced::is_empty")] ai_placed: AiPlaced`, holding one
  `Option<AiOrigin>` slot per draftable judgment field (nine slots — not `current_price` /
  `ttm_eps`), which drives the chart's "placed by AI" + validation-date annotation (FR33); a slot is
  cleared by **any write that changes** its field (a value-identical write stays a no-op — Story 8.2b
  G3, owner-pending). Refresh never
  writes a draftable judgment field (D6), so the mark cannot survive a provider overwrite.
- **Compatibility:** additive fields only, as A5 (D9 withdrawn).
- **Rationale:** a new `Source` variant would ripple through reconciliation, the review tri-state
  and every exhaustive `match`, and an older build would fail to parse it; the owner's validation is
  what makes the value authoritative, so it reconciles exactly as a manual entry.

### A7 — Stale detection by fingerprint

- **Decision:** one canonical function in `contract`, `draft_fingerprint(study, target)`, with an
  explicit, documented field encoding (never the serde form, which changes when types gain fields):
  - **cell draft:** the normalised value, its source and its pending (divergent provider) value —
    **not** its timestamp or digest, which a value-identical re-stamp (`restamp_if_predated`) changes
    without any real change;
  - **judgment draft:** the field's value **plus** the study's load-bearing inputs (historical
    series and the market facts the judgment is read against) **and** `METHOD_VERSION`, so a refresh
    or a method change marks it stale.
  Decimals are normalised before hashing (the `Money` scale caveat in `contract/src/provenance.rs`).
  A draft is **stale** when the current fingerprint differs — computed on read, recorded in
  `stale_at_decision` when decided. A draft whose target no longer exists (study deleted before the
  cascade, fiscal year dropped by a refresh) reads **target gone** and cannot be validated.
  `decide_draft` re-checks the fingerprint **inside** its transaction against the one the owner
  confirmed: if a refresh landed after the confirmation dialog, the decision is refused and the draft
  is shown again.
- **Draft study:** no fingerprint; the duplicate check runs again at validation (D2).
- **Rationale:** catches owner edits, refreshes and method changes (FR72) without any trigger or
  bookkeeping on the app's write paths, and without false stale marks.

### A8 — Applying a decision through the app state, atomically

- **Decision (D3):** validation goes through the app's study state, never behind it. The draft is
  applied to the in-memory study and pushed on its **undo stack** — the app keeps one **active** undo
  history, for the open study (a closed study's history is parked, see below, not reset), so
  validating a draft of another study first **opens that study** (the inbox says so), and the
  validation stays undoable while it remains open — or after it is closed and reopened unchanged
  (réconcilié 2026-10-01 : `app/src/state/undo.rs`, story 8.5b écart 3 / G3 1);
  then
  `persistence::decide_draft(study, draft_id, decision)` performs the study upsert (with its history
  snapshot) **and** the draft's status / `decided_at` / `stale_at_decision` /
  `edited_before_validation` update in **one transaction** with one `logical_version` bump; the open
  study is then refreshed from the dossier **without** resetting its undo history. Closing a study (Story
  8.5b) **parks** its history, ownerless, with the study as it stood; reopening it hands the history
  back only while the stored study still equals that snapshot — any write meanwhile (a late fetch,
  an import, another writer) drops it, so no undo writes back a state that skips a change. Visiting
  the read-only demonstration study keeps the parked history (undo/redo are disabled on the demo)
  (réconcilié 2026-10-01 : PR #279). Undoing a
  validation restores the prior study and sets the draft to `validated_undone` in one transaction;
  redoing it re-applies the value and sets the draft back to `validated`, in one transaction too. Rejection updates only the draft. A draft
  study's validation opens the create-study dialog prefilled (D2); its confirmation creates the study
  and sets `created_study_id` + `validated` in one transaction; it is not on the undo stack — the
  owner reverses it by deleting the study, which deletes its drafts (O7).
- **Rationale:** NFR-R2 — a crash can never leave a value applied with its draft still pending, or a
  draft marked validated with nothing applied; and no later save of a stale in-memory copy can
  silently overwrite an applied draft (lost update).

### A9 — The open app discovers new drafts by polling `PRAGMA data_version`

- **Decision:** the app polls `PRAGMA data_version` on its own connection every 2.5 s (a Slint
  `Timer`); a change triggers a re-read of pending drafts. The inbox is also re-read on opening it and
  after each of the app's own draft-affecting writes (decision, undo/redo, study delete, import,
  restore, dossier switch), which never move `data_version`. There is **no window-focus trigger**:
  Slint 1.17 has no public window-activation callback; the timer bounds the latency (réconcilié
  2026-10-01 : `app/src/wiring/drafts.rs` `POLL_PERIOD_MS`, story 8.5a décision 1). No file watcher. A failed poll or inbox read shows the inbox as « indisponible » with its
  cause — never as an empty inbox.
- **Concurrency case to test:** in DELETE mode, an MCP read holding a shared lock makes the app's
  commit wait (app `busy_timeout` 5000 ms, `persistence/src/journal.rs`); MCP reads are short and
  bounded (A2), so the app's commit succeeds — a test holds a read transaction open while the app
  commits.
- **Rationale:** `data_version` changes only when **another** connection commits, so the app's own
  writes do not trigger it; it costs one pragma, works identically in WAL and DELETE mode and on every
  OS, whereas file watchers are unreliable on synced paths.

### A10 — Dossier resolution, per call

- **Decision:** `--dossier <path>` when given (development, tests); else the dossier the app last
  **actually opened** — a `last_opened_path` the app writes to its config each time it opens a
  dossier. This differs from `journal_path` when a configured dossier was refused by name and the
  app runs on the default one (G3 M4 keeps `journal_path` on the refused dossier): MCP must follow
  the dossier the owner sees. Until the app has written `last_opened_path` (an install not yet
  opened with the new build), MCP falls back to `journal_path`, then to the app's default dossier
  path (`default_journal_path`), like the app.
  Resolution happens **per call**, so a dossier switch in the app is followed without restarting the
  server; every response names the dossier (`journal_id` + resolved path), and every submission
  carries both and is refused on mismatch (D10) — the path tells a copy from its original, which the
  `journal_id` alone does not (`VACUUM INTO` backups and test copies keep it). The config-path helper
  and `default_journal_path` live in the workspace crate **`paths`** (`paths/src/lib.rs`: depends
  only on `directories` + serde/serde_json, never writes; `resolve_dossier`), used by both `app` and
  `mcp` so `mcp` does not depend on `app`; it reads the config tolerantly (append-only
  `#[serde(default)]`) (réconcilié 2026-10-01 : `paths/src/lib.rs`, story 8.4 décision 2).
- **Rationale:** the owner's AI client must follow the dossier the owner actually uses, and a reply
  must never leave doubt about which universe it describes (real vs test dossier).

### A11 — Restore while MCP runs

- **Decision:** `restore_journal_file` assumes every connection to the live file is closed — which
  only the single-instance lock guaranteed, and MCP does not take it. As built (réconcilié
  2026-10-01 : `persistence/src/restore.rs`, `persistence/src/mcp_access.rs` — story 8.3 G3 CRITICAL
  « POSIX lock loss », décisions 3, 6, 8, 13, 14), the restore runs in order:
  1. it writes a **marker** `…-restoring` (owner pid + start time) beside the live file first; while
     it stands, every new MCP call is refused up front (`dossier_busy`); a marker or staging copy left
     by a process that is gone reads as an interrupted restore (`restore_interrupted`);
  2. the live file leaves WAL and is **locked exclusively** (`BEGIN EXCLUSIVE` with a busy wait);
     its `-wal`/`-shm` are removed under that lock — nothing is deleted after the rename;
  3. the backup is copied to a **staging** sibling (`…-restore-incoming`) and renamed over the live
     path; on Unix the lock is held until the rename; on **Windows** (which cannot replace a file
     the process holds open) the lock is released just before the rename;
  4. the lock is released and the marker removed last.
  Every MCP write re-checks, inside its write transaction, that the file at the resolved path is
  still the one it opened, aborting otherwise (the `journal_id` alone does not tell a restored backup
  from the live file). The identity check differs by OS: on **POSIX**, a `stat(2)` (device + inode)
  taken before the connection opens plus SQLite's `SQLITE_FCNTL_HAS_MOVED` on the connection's own
  file, called through **`unsafe` FFI** (`sqlite3_file_control`) — the only `unsafe` code in the
  workspace — because opening any second descriptor of the file would, on close, release every POSIX
  lock the process holds on it; on **Windows**, a `same_file::Handle` opened before the connection.
- **Rationale:** otherwise a draft committed during a restore lands in the unlinked old file (lost
  silently), and its connection's close can delete the restored file's `-wal` by path.

### A12 — Risks and their handling

- **Developer = client:** Claude Code is both the developer and the MCP client. Dev/test runs
  **always** pass `--dossier` with a temporary path, and the test harness sets `XDG_CONFIG_HOME` /
  `XDG_DATA_HOME` to a temporary directory so the fallback resolution (A10) can never reach the real
  config (a CI test asserts it). Registering the MCP server on the real dossier is the **owner's**
  act, following the registration doc (Story 8.4), at a Claude Code scope **not active in the
  steadyinvest repository**, so development sessions never see the real dossier's tools.
- **Logs:** the MCP binary writes its own rotating log file (`steadyinvest-mcp.log`, beside the app's
  log directory), never the app's file.
- **History noise:** history entries are full-study snapshots with no cause column. Note-only
  entries are identified by comparing consecutive snapshots with `notes` ignored; the history view
  labels them and can filter them out. Processed drafts are merged into a study's timeline by joining
  `ai_drafts` on `study_id` / `created_study_id` and `decided_at` (a rejected draft writes no
  snapshot).
- **AI text containment:** AI-origin text reaches the UI **only** through a dedicated AI-frame
  component (label + disclaimer, FR13); a structural test scans the `.slint` sources and fails if an
  AI-origin property is bound outside that component.
- **SDK choice:** decided in Story 8.4 (décision 1): **`rmcp` 3.4** with `default-features = false`
  and only the `server` + `transport-io` features, and a hand-written `ServerHandler`
  (`mcp/src/server.rs`); it passes `cargo deny` (licence + advisories) and pulls no
  `reqwest`/network feature into the closure (A1 test) (réconcilié 2026-10-01 : `Cargo.toml`,
  `mcp/src/server.rs`).

### A13 — Frozen decision-time verdict (FR68, owner decision D11)

- **Decision:** an explicit study action « Valider l'étude », enabled only when the verdict is
  `Full`, stores `#[serde(default)] frozen_verdict: Option<FrozenVerdict>` in the `Study` blob:
  the verdict facts (verdict, zones, upside/downside ratio, 5-year potential), the `inputs_hash`
  and `method_version` already carried by `core::verdict::FullVerdict`, the load-bearing input
  values it was computed from, and `frozen_at`. A study verdict involves no FX (NFR-C4), so "dated
  FX" in FR68 applies only where a consolidated figure is later frozen — none in this story.
  Validating again replaces it; the previous one stays in the study history (FR51). The freeze is
  an ordinary study upsert, undoable in the session (FR32).
- **Difference:** the current verdict is always computed live (never persisted); when it differs
  from the frozen one — facts, `inputs_hash` or `method_version` — the study shows both, labelled
  « figé ({méthode}, JJ/MM) » and « actuel ({méthode}, aujourd'hui) », where `{méthode}` is the
  method version string (e.g. `ssg-1.2.0`) (réconcilié 2026-10-01 : `frozen_verdict_strip.slint`,
  story 8.8 décision 9), naming the changed items and the cause
  where known (refresh, owner edit, method change — FR29, the #252 method stamp). Neutral wording
  only (FR13).
- **Confrontation (FR50):** the « Confrontation » view reads the projection band from the frozen
  verdict — the projection decided, dated the validation day — not from today's study. Three bases
  (`ConfrontBasis` in `app/src/state/confront.rs`): *Decided* (the frozen band), *FrozenWithoutBand*
  (a frozen verdict over a degenerate band: today's band, dated and named from the validation),
  *Current* (no frozen verdict: today's band, said as such, dated the study's creation); the price
  window starts at that date (réconcilié 2026-10-01 : PR #280). Validating again replaces the frozen
  verdict, so the confrontation then starts from the latest decision; earlier decisions stay in the
  history only. **Décision en attente (Guy) : une re-validation déplace la date de décision de la
  confrontation — à confirmer.**
- **Compatibility:** additive field only, as A5 (D9 withdrawn).
- **MCP:** the study read returns the frozen verdict beside the current one; `frozen_verdict` is not a
  draftable field, and the 8.3 rejected-writes suite covers it (FR68 [P4]). A validated AI draft
  changes the current verdict only, so the difference is highlighted like any other change.

### Stories (Epic 8, ordered)

8.0 UX pass: inbox, AI frame, reminders, AI line, AI-origin marks, notes, drafts record, French
wording, PDF impact (D1) · 8.1 study notes (A5) · 8.2a drafts table, migration v8, export/backup,
cascade (A4) · 8.2b AI origin, fingerprint, `decide_draft` (A6–A8) · 8.3 `McpAccess` + authorizers +
suites + restore safety (A2, A3, A11, O1) · 8.4 `steadyinvest-mcp` binary, dossier resolution,
dependency-closure test, registration doc (A1, A10, A12) · 8.5a AI frame, inbox (read), polling
(A9) · 8.5b decisions (A7, A8, O4, O5) · 8.6 AI judgment lines (FR33/72) · 8.7 draft-study end-to-end
+ record view · 8.8 frozen decision-time verdict (FR68, A13).

## Architecture Validation Results

### Coherence Validation ✅

**Decision Compatibility:** All technology choices are mutually compatible and version-verified
(June 2026): Slint 1.16.1 — 1.17.0 resolved as of 2026-10-01 (réconcilié 2026-10-01 : `Cargo.lock`) — (workspace MSRV 1.96, see Tech Stack note) · rusqlite 0.40 (bundled) · rust_decimal 1.42 (+maths) ·
reqwest 0.13 (pure-Rust TLS, exact feature decided in Story 3.1) + tokio 1.52 · thiserror 2.0 · proptest 1.9 · keyring 3.x (NOT 4.0) · directories ·
tracing. The **Slint GPLv3 licence is compatible with the project's GPL-3.0** (the PRD's "Slint
licensing tier" risk is closed, pending the `cargo deny` dependency audit). No contradictory
decisions remain (egui fully removed; no web; no network server — the [P4] MCP server is a local
stdio child process, not network-exposed, and the only external interface).

**Pattern Consistency:** Patterns support the decisions — exact-decimal numerics + named rounding
back the deterministic-engine decision; the immutable-snapshot/content-addressed-verdict pattern
realizes the Foundational Invariant; the Slint view-model adapter + "money as formatted strings"
enforce the UI boundary; injected Clock/IdGen back determinism and testability; the two-axis i18n
(`@tr()` vs runtime label set) matches the neutral-posture/label-swap requirement.

**Structure Alignment:** The workspace — 8 crates as built: `core, contract, ingestion, persistence,
report, app, paths, mcp` (réconcilié 2026-10-01 : `Cargo.toml`) — enforces the
boundaries: `core` (no I/O) holds the Cardinal Rule; `contract` is the shared seam (and the MCP
vocabulary); only `persistence` touches SQLite (`mcp` only via `McpAccess`); only `ingestion`
touches the network (absent from `mcp`'s closure); only `app` touches Slint; `report` isolates PDF I/O;
`paths` holds the per-machine locations shared by `app` and `mcp`.
Every boundary in the decisions maps to a crate.

### Requirements Coverage Validation

**Functional Requirements Coverage ✅:** All 66 FRs (2026-06-08) map to a crate/module (see
Requirements-to-Structure table); FR69–FR78 [P4] (G2, 2026-09-27) are mapped there and in §Phase 4. Phase tags preserved (P1 in MVP scope; P2/P3/V located but deferred). Two coverage
clarifications added during validation:
- **FR9 (golden self-check) is a user-facing runtime feature, not just CI fixtures:** bundled golden
  reference studies live as **app assets** (`app/assets/golden/`) and are runnable from the UI via a
  **"verify engine" path** that replays them and reports any deviation beyond tolerance — distinct
  from the CI test goldens in `core/tests/golden/`.
- **FR50 (projection vs actual trajectory) needs post-decision price history:** sourced via an
  `ingestion` refresh and stored in a **price-history cache in `persistence`**, overlaid by `app`.

The two **added requirements** (user-selectable DB dir + reopen last-used journal; frozen-verdict +
on-demand recompute) are located and flagged **to be filed as FRs once the repo exists**.

**Non-Functional Requirements Coverage ✅ (one item to validate):**
- *Correctness* — exact decimal + deterministic core + golden/property/metamorphic + pinned
  determinism hash in CI (Linux-only CI for now — réconcilié 2026-10-01 : `ci.yml`). ✅
- *Performance* — `<~1 s` recompute and `<~3 s` launch within reach; **`<100 ms` judgment-line
  recolor targeted but NOT yet proven** in native Slint (see Gap). ⚠️
- *Security/Privacy* — keychain-only secrets, no telemetry, all-local, keys injected not stored in
  `ingestion`. ✅ [P4] study data may leave only through the owner's AI client (NFR-S3/S4). ✅
- *AI asymmetry [P4]* — NFR-A1–A4: draft-only writes, allowlisted reads, no provider call reachable,
  origin + comment on every draft — by construction (§Phase 4, A1/A3/A4) and CI suites. ✅
- *Reliability* — offline workflow; multi-statement writes in a single SQLite transaction + WAL for
  crash-safety; forward-safe migrations + frozen corpus; non-destructive reconciliation;
  integrity/version checks on import/restore. ✅
- *Portability* — exact decimal removes float divergence; locale-aware numbers; portable journal. ✅
- *Usability* — decision-never-colour-only, keyboard-first, faithful form; confusability CI gate. ✅
- *Maintainability* — thin UI over tested core + versioned contract decoupled from Slint/SQLite. ✅

### Implementation Readiness Validation ✅

**Decision Completeness:** all critical decisions documented with pinned versions and rationale.
**Structure Completeness:** complete workspace tree (files + dirs), boundaries, FR mapping, data flow.
**Pattern Completeness:** naming (Rust/SQLite/Slint axes), format, communication, process, error,
logging, i18n, and 11 agent-divergence points addressed with examples and anti-patterns.

### Gap Analysis Results

**Critical Gaps:** none blocking — the architecture is internally complete; no missing decision
prevents starting implementation.

**Important Gaps (resolve early, do not block scaffolding):**
1. **Native-Slint charting unproven.** The `<100 ms` draggable-judgment-line recolor is the principal
   technical unknown; the user has no prior interactive-Slint-drawing experience. *Mitigation:*
   Week-1 throwaway spike (go/no-go); fallback = dedicated Slint canvas/window or
   `plotters`→`SharedPixelBuffer` + `TouchArea` overlay. Run before committing UI work.
2. **PRD Appendix A deferrals not yet finalized.** The exact **SSG output set**, **plausibility
   rules**, **banned-verb list**, **golden tolerance**, and **"load-bearing input" definition** were
   deferred "to Architecture." Capture them in a dedicated **method specification** consumed by
   `core` (pinned by `method_version`), authored at epic/story time before the engine is implemented.
3. **FR9 runtime golden self-check** (bundled golden studies as app assets + a "verify engine" UI
   path) to be specified alongside the method spec — distinct from CI test goldens.
4. **FR50 post-decision price-history cache** in `persistence` (source/retention) to be specified.

**Nice-to-Have Gaps:** shared synthetic-fixtures crate (deferred; per-crate fixtures for now);
`cargo deny` policy authored at scaffolding; criterion latency baselines captured once the spike lands.

### Validation Issues Addressed

- Legacy old-project artifacts (web/Leptos stack) confirmed as NON-INPUTS; prior architecture.md
  archived; user handling Synology-source cleanup. No bearing on this architecture.
- Slint licensing risk resolved (GPLv3 ↔ GPL-3.0), pending the dependency-license audit (`cargo deny`).

### Architecture Completeness Checklist

**Requirements Analysis**
- [x] Project context thoroughly analyzed
- [x] Scale and complexity assessed
- [x] Technical constraints identified
- [x] Cross-cutting concerns mapped

**Architectural Decisions**
- [x] Critical decisions documented with versions
- [x] Technology stack fully specified
- [x] Integration patterns defined
- [x] Performance considerations addressed (target set + spike + fallback; <100 ms to be verified)

**Implementation Patterns**
- [x] Naming conventions established
- [x] Structure patterns defined
- [x] Communication patterns specified
- [x] Process patterns documented

**Project Structure**
- [x] Complete directory structure defined
- [x] Component boundaries established
- [x] Integration points mapped
- [x] Requirements to structure mapping complete

### Architecture Readiness Assessment

**Overall Status:** READY FOR IMPLEMENTATION (with important gaps to resolve early — the Week-1
charting spike, the Appendix-A method spec, and the FR9/FR50 specifications; none blocks workspace
scaffolding).

**Confidence Level:** High — for everything except the native-Slint charting interaction, which is
Medium until the Week-1 spike proves the `<100 ms` recolor (with a defined fallback if it does not).

**Key Strengths:**
- One Foundational Invariant (dated proof of every asserted fact) unifies trust, journal identity,
  verdict coherence and backup integrity — and is also the product differentiator.
- Exact-decimal core kills the silent-wrong-signal float risk AND cross-OS determinism in one move.
- UI-independent tested core + versioned contract make the GUI choice reversible and the math
  auditable; the [P4] draft-writing MCP process sits on the contract and a narrow, engine-enforced
  persistence surface (`McpAccess`) rather than a new data path.
- Slint-only (no web, no egui embedding) removes the entire dual-render-paradigm risk class.

**Areas for Future Enhancement:**
- P2/P3/V features (multi-portfolio/FX depth, transaction ledger, dividends, Company Comparison,
  Portfolio Health Review, screening, other-form PDFs) — located, deferred. The "read-only AI clerk"
  stance is dropped (G2, 2026-09-27): AI assistance is Phase 4 — the AI may propose drafts under a
  human gate (§Phase 4); a self-hosted local model stays [V].
- Provider fallback-chain + rate-limit batching; configurable diversify-by-size table.

### Implementation Handoff

**AI Agent Guidelines:**
- Follow the architectural decisions and Implementation Patterns exactly; respect crate boundaries
  (especially the Cardinal Rule: all calc in `core`).
- Never present a fact without its dated proof; never coerce missing to 0; never float for money;
  never leak domain types/floats into `.slint`.
- File bugs/CRs/deferred items in GitHub Issues; visually verify any UI story before "done".

**First Implementation Priority:**
1. Scaffold the Cargo workspace + 6 crates (+ seed UI from the Slint template) — the first story.
2. Run the Week-1 spikes (A grid paste-a-column, B native-Slint drag+recolor <100 ms, C decimal CAGR
   precision + cross-OS determinism hash) — go/no-go before committing UI work.
3. Author the Appendix-A method spec, then implement `core` test-first.

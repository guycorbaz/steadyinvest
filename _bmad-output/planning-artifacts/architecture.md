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
---

# Architecture Decision Document

_This document builds collaboratively through step-by-step discovery. Sections are appended as we work through each architectural decision together._

## Project Context Analysis

### Requirements Overview

**Functional Requirements (66 FRs, 11 clusters; phase tags P1/P2/P3/V):**
- *Stock Study & Methodology Engine* (FR1-8): create/persist/reopen/update studies; deterministic
  SSG output set; native-currency calc; judgment inputs; quality flags; 5-year-floor low-confidence.
- *Calculation Integrity & Trust* (FR9-14): golden reference + tolerance; plausibility warnings;
  verdict traceability; testable degraded/withheld verdict; neutrality (banned-verb); AI read-only [V].
- *Data Acquisition, Provenance & Providers* (FR15-29): auto-fetch; first-class manual entry/override;
  per-cell source + provenance + timestamp; coverage present/to-fill/not-available-accepted; user-set
  validated flag; manual refresh; non-destructive reconciliation; graceful provider failure (stale +
  cause); keyless + keychain keys; FX acquisition [P2]; deterministic recompute distinguishing cause.
- *Charts & Judgment Interaction* (FR30-33): growth/valuation charts; judgment line by value +
  direct-manipulation in sync, live recalc; undo; never auto-place/suggest.
- *Watchlist & Alerts* (FR34-35) · *Portfolio/Transactions/Holdings* (FR36-41, single [P1] →
  multi-portfolio/FX/ledger/dividends [P2]) · *Risk Management* (FR42-48: trailing stop, simple
  capital-at-risk [P1]; per-currency→bank→global + concentration [P2]; neutral triggers; stop-priority;
  replacement [P2]).
- *Cumulative Memory & Journal* (FR49-51) · *Reporting/Printing* (FR52-53) · *App Shell & Data Mgmt*
  (FR54-62: dashboard; delete/archive w/o corrupting time-series; entry↔contemplation regimes; legend;
  empty/error; single-study + whole-journal export/import versioned+validated; restore w/ integrity +
  version checks; help + demo) · *Config/Posture* (FR63-66: no-wizard Settings; always-visible
  disclaimer; full offline; portable local store).

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
- *Security & Privacy* — keys only in OS keychain; no telemetry; all data local.
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
  zone/ink/label tokens in an immutable snapshot (`arc_swap`); theme/regime change forces a redraw.
- **Two distinct quality-gate families**: *trust gates* (types, traceability, reproducibility) vs
  *posture gates* (neutral naming, swappable labels). Do not reduce neutrality to a string grep.

### Technical Constraints & Dependencies

- Stack: Rust + Slint (egui removed), local SQLite via rusqlite, offline-first, no server in v1,
  versioned serde data contract decoupled from Slint and SQLite (keeps a future read-only MCP/AI
  façade cheap).
- **Ingestion/normalization is a first-order architectural boundary**, distinct from the calc engine:
  IFRS↔US-GAAP, split/series breaks, fiscal-period misalignment, currency-of-report — the real
  birthplace of the silent-wrong-signal — get their own normalization layer, golden recollage
  fixtures, and **metamorphic tests** (equivalent IFRS/GAAP inputs ⇒ same verdict).
- **Trust invariants as TYPE properties, not just tests**: a `FullVerdict` is constructible only from
  all-validated-and-fresh load-bearing inputs (compiler is the gate); verdict + staleness derive from
  the SAME immutable state snapshot so an incoherent frame is structurally impossible.
- **Test architecture (CI gates)**: frontier golden fixtures (synthetic, documented provenance);
  property tests incl. monotonicity / boundary-continuity / idempotence / scale-homogeneity;
  metamorphic tests; determinism hash asserted equal on the 3 OS (or trivial under decimal); a
  frozen versioned-journal corpus + schema-drift detector + forward-compat read-only-on-newer-file.
- PDF/print fidelity (FR52) via `genpdf`/`printpdf` from the calc crate, UI-independent.
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
- Neutral posture — facts-not-advice, banned-verb enforcement, always-visible disclaimer, never an
  auto/suggested line; AI read-only by construction [V].
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

- **Slint 1.16.1** (MSRV Rust 1.88) — licensed GPLv3 / royalty-free / commercial; **GPLv3 is
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
  `default-features = false` and explicit platform features (`linux-native` or `sync-secret-service`
  on Linux, `apple-native`/Keychain on macOS, `windows-native`/Credential Manager on Windows). The
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
cargo add slint@1.16        --package app
cargo add slint-build@1.16  --package app --build
cargo add rusqlite@0.40 --features bundled        --package persistence
cargo add rust_decimal@1 --features maths          --package core
cargo add serde@1 --features derive                --package contract
cargo add directories@6                            --package app
cargo add keyring@3 --no-default-features          --package app         # Story 3.2 only; add explicit platform feature (e.g. linux-native). NOT keyring 4.x (sample/CLI meta-crate)
```

**Proposed workspace layout (crates):**

- `core/`      — pure SSG calculation engine (deterministic, `rust_decimal` w/ `maths`, no I/O, no
  UI). Golden + property + metamorphic tests live here. Stamps `method_version`.
- `contract/`  — versioned serde data contract (`schema_version`), journal/judgment types, decoupled
  from Slint and SQLite. The boundary the future read-only MCP/AI façade will sit on.
- `ingestion/` — provider-agnostic acquisition + **normalization** layer (IFRS/GAAP, split/series,
  fiscal-period, currency-of-report), `MarketDataProvider` trait + adapters. Its own golden fixtures.
- `persistence/` — rusqlite storage (journal_id, logical version, migrations, `PRAGMA user_version`),
  export/import/backup. Local DB; sync-path detection.
- `app/`       — thin Slint UI (forms, dense grid, native charts via `Path`/`TouchArea`), app-config
  via `directories`, secrets via `keyring`, theme tokens single-source.

**Architectural Decisions Provided / Implied by this Foundation:**

- **Language & Runtime:** Rust (workspace, MSRV ≥ 1.88 per Slint 1.16); single native binary per OS.
- **UI:** Slint 1.16 (GPLv3), declarative `.slint` + `slint-build`; charts native; no web, no egui.
- **Numerics:** `rust_decimal` (+`maths`) exact decimal in the core (determinism + correctness).
- **Persistence:** rusqlite `bundled` SQLite, single local file.
- **Secrets / Config:** `keyring` (OS store) + `directories` (app-config), kept out of the journal.
- **Testing:** Cargo test in `core`/`ingestion` (golden/property/metamorphic), versioned-journal
  corpus in `persistence`; CI matrix on the 3 OS asserting identical results.
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
- HTTP/fetch = **reqwest 0.13 (`rustls-tls`,`json`) + tokio 1.52** (async), off the UI thread.
- Error model (`thiserror` 2.0, neutral cause-named, no silent `.ok()`); logging (`tracing`, local
  file, no telemetry); test architecture (`proptest` 1.9 + golden/metamorphic + 3-OS CI).
- App-config vs journal boundary (`directories` + `keyring`); journal identity (`journal_id` + logical
  version); SQLite pragmas + sync-path detection.
- Export/backup format; decimal rounding policy; UI visual-verification strategy (the four points
  below).

**Deferred Decisions (Post-MVP):**
- Read-only MCP/AI façade over the data contract [V]; provider fallback-chain & rate-limit batching
  [P2]; multi-portfolio/FX consolidation depth, transaction ledger, dividends [P2]; PDF of the other
  forms [P2/P3]; configurable "diversify-by-company-size" table (from `change-request_guy.md`) [P2].

### Data Architecture

- **Store:** rusqlite 0.40 `bundled` SQLite, single local file (the journal). `WAL` +
  `synchronous=NORMAL` + `busy_timeout` for local use; **auto-switch to `DELETE/TRUNCATE` + warn**
  when the DB path is detected on a sync folder (Synology/Dropbox/OneDrive/iCloud). Single
  mutex-guarded write connection (WAL allows concurrent readers + one writer).
- **Hybrid model (decided):**
  - *Normalized tables* for what we aggregate/query: `portfolio`, `holding`, `transaction`, `fx_rate`,
    `watchlist_item`, plus index columns. This is where consolidation (per-currency→per-bank→global),
    concentration and capital-at-risk run — SQL-friendly.
  - *Versioned serde JSON blob* (`payload TEXT` + `schema_version` column) for what we replay in bulk:
    `study` and its `judgment` snapshots. Append-mostly, read whole, never queried by inner field →
    no SQL migration when the judgment model evolves.
  - Indexed columns alongside blobs: `journal_id`, `security_ticker`, `created_at`, `status`,
    `schema_version`, `method_version`.
- **Identity & integrity:** `journal_id` (UUID) + monotonic logical version written INTO the DB at
  creation; the app-config "last-used" pointer references `(journal_id, last-seen-version)`, not a
  path; backups/exports carry `(journal_id, version, hash)`; a single-instance file lock guards the
  open journal.
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
- **FX:** `fx_rate` rows are dated & source-aware; FX applied only at the consolidation layer; the
  rate used by a consolidated judgment is frozen at the judgment date.

### Authentication & Security

- **No authentication / no accounts / no multi-user** — single-user offline desktop by design.
- **Secrets:** provider API keys only in the OS secret store via `keyring` 3.x (platform backends
  chosen explicitly, `default-features = false`; **not** keyring 4.0 — see Tech Stack note; Linux
  secret-service needs a D-Bus agent). Never in repo/config/logs/exports.
- **Privacy:** no telemetry; the only network calls are user-initiated provider/FX fetches under the
  user's own key; all data local.
- **AI [V]:** read-only by construction over the data contract; never a write path (capability
  asymmetry enforced structurally, not by prompt).

### API & Communication Patterns

- **No server / no public API in v1.** The "API" is the **versioned serde data contract** (clean
  types, `schema_version`), decoupled from Slint and rusqlite — the boundary a future **read-only MCP**
  façade [V] will sit on at near-zero cost.
- **Provider acquisition:** `MarketDataProvider` trait; first adapter **EODHD** (CH/EU+US coverage);
  keyless adapters supported. HTTP via **reqwest 0.13** with **`rustls-tls`** (pure-Rust, no system
  OpenSSL → portable single binary) + `json`, on a **dedicated `current_thread` tokio 1.52 runtime on
  a worker thread** (sufficient for manual refresh; P2 ticker-batching via concurrent tasks
  `join_all`). Results marshalled back to the Slint event loop via `invoke_from_event_loop`. Provider
  failure is classified (network / quota / invalid-or-absent key), recorded, surfaced as a neutral
  global banner; last-known values retained and flagged stale.
- **Errors:** `thiserror` 2.0 domain errors per crate; neutral, cause-named messages; **no silent
  `.ok()`** (explicit lesson from the prior project's chart-rendering bugs).

### Frontend Architecture

- **Slint 1.16 (GPLv3)**, declarative `.slint` + `slint-build`; thin UI over the calc core.
- **State & recompute (realizes the Foundational Invariant):** a single **immutable study-state
  snapshot** is the source of truth; the UI derives from it; recompute is **transactional and pure**
  (inputs + verdict born together); the verdict is **content-addressed** by `f(hash(inputs),
  method_version)`; an input change **invalidates** the dependent verdict (marked stale) rather than
  silently overwriting it. Undo/redo = snapshot stack (state is small → simple clones; structural
  sharing only if needed).
- **Charts native in Slint** (`Path` + `TouchArea`, log10 in Rust; <100 ms recolor trivial in Slint's
  dirty-driven retained mode). Week-1 spike is the go/no-go.
- **Theming:** design tokens (zone colours/ink/label set) live in **one neutral source of truth** read
  by the UI (intra-binary `arc_swap` snapshot); theme/regime change forces a redraw. Two token
  families: colour/alpha (free to swap) vs metric/typo (quasi-static, never during a drag).
- **i18n:** French-first string table, i18n-ready; separate axis from the NAIC↔neutral label set.
- **Verdict integrity in UI:** a `FullVerdict` is constructible only from all-validated-&-fresh
  load-bearing inputs (compiler-enforced); verdict + staleness derive from the same snapshot so an
  incoherent frame is structurally impossible.

### Infrastructure & Deployment

- **No cloud, no containers, no server.** Distribution = a native binary per OS (Win/macOS/Linux),
  built from the Cargo workspace; updates manual in v1 (git pull/rebuild or replace binary).
- **CI:** cross-platform matrix (the 3 OS) running `cargo test` (engine golden/property/metamorphic,
  versioned-journal corpus, marker-confusability snapshot); CI asserts **identical numeric results**
  across OS (trivial under exact decimal) and gates merges on the trust quality-gates.
- **UI visual-verification strategy (decided — point 4):** the prior project shipped a blank chart
  marked "done" for 4 epics because nothing rendered it and looked. Therefore: **Slint render snapshot
  tests** for key surfaces (the chart, trust markers, verdict states), the **marker-confusability
  snapshot gate**, and a **Definition-of-Done rule = "launch the app and visually verify"** before any
  UI story is "done". Detailed in the Implementation Patterns step / TEA phase.
- **Backup/restore:** delegated to an external system (NAS sync) — the app keeps the journal as a
  single copy-friendly local file and pushes versioned exports/backups (carrying journal_id, version,
  hash) to a configurable target; the live DB stays out of the sync-watched folder.
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
- `contract` is consumed by `persistence`, `app`, and (future) the MCP/AI façade — its `schema_version`
  + `method_version` discipline gates migrations.
- The Foundational Invariant cuts across `core` (content-addressed verdict), `persistence` (frozen
  judgments + identity), `ingestion` (provenance/freshness), and `app` (no incoherent frame).
- FX consolidation sits only at the `persistence`/portfolio aggregation layer, never in `core`'s
  native-currency calc.

## Implementation Patterns & Consistency Rules

### Pattern Categories Defined

**Critical Conflict Points Identified:** ~11 areas where independent dev agents could diverge —
crate/package naming, Rust vs SQLite vs Slint naming axes, decimal storage, JSON contract field case,
error-type shape, state-update model, logging, the calc-location ("Cardinal Rule"), the Slint
view-model boundary, the time/ID source, and the i18n-vs-label-set split.

### Naming Patterns

**Workspace & crates:** package names `steadyinvest-core`, `steadyinvest-contract`,
`steadyinvest-ingestion`, `steadyinvest-persistence`, `steadyinvest-app`; directory names short
(`core/`, `contract/`, …); internal refs via `[workspace.dependencies]` (single source of versions).

**Rust code (rustfmt + clippy enforced):** types/traits `PascalCase`; fns/vars/modules/files
`snake_case`; consts/statics `SCREAMING_SNAKE_CASE`; one module = one file/dir, organized **by
domain** (no `utils.rs` grab-bag — shared helpers in a named module).

**SQLite (persistence crate):** tables `snake_case` **plural** (`portfolios`, `holdings`,
`transactions`, `fx_rates`, `watchlist_items`, `studies`, `judgments`); columns `snake_case`; PK
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

- **Data contract = serde JSON, `snake_case` field names** (Rust default; the future read-only
  MCP/AI consumer is also Rust-side). `#[serde(default)]` on every new field; **never
  `deny_unknown_fields`** on the journal (forward-compat).
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

```text
steadyinvest/
├── Cargo.toml                     # [workspace] members + [workspace.dependencies] (single version source)
├── Cargo.lock                     # committed (application → reproducible builds)
├── rustfmt.toml                   # formatting rules (CI: cargo fmt --check)
├── clippy.toml                    # lint config (CI: cargo clippy -- -D warnings)
├── justfile                       # tooling tasks (build, test, lint, spike, release)
├── rust-toolchain.toml            # pin MSRV ≥ 1.88 (Slint 1.16)
├── deny.toml                      # cargo-deny: GPL-3.0 dependency-license audit
├── README.md
├── LICENSE                        # GPL-3.0
├── .gitignore                     # ignores: target/, *.db, .env, keys, local config
├── .github/
│   └── workflows/
│       └── ci.yml                 # 3-OS matrix: fmt, clippy, test, trust gates, determinism hash
├── docs/                          # (existing) NAIC reference PDFs + project docs
│
├── core/                          # steadyinvest-core — PURE calc engine (NO I/O, UI, SQL, net)
│   ├── Cargo.toml                 # deps: rust_decimal (+maths), serde (types only)
│   ├── benches/                   # criterion: pure recompute cost (feeds nightly latency tracking)
│   ├── src/
│   │   ├── lib.rs
│   │   ├── ssg/                   # the 5-section SSG method (FR4)
│   │   │   ├── mod.rs
│   │   │   ├── growth.rs          # §1 CAGR, projections (powd/exp via rust_decimal maths)
│   │   │   ├── management.rs      # §2 margin, ROE, debt
│   │   │   ├── valuation.rs       # §3 P/E history A–H
│   │   │   ├── risk_reward.rs     # §4 forecast high/low, zoning, U/D ratio
│   │   │   └── return_proj.rs     # §5 yield, total return
│   │   ├── verdict.rs             # FullVerdict (constructible only from validated+fresh inputs)
│   │   ├── quality_flags.rs       # FR7 thresholds; plausibility checks (FR10)
│   │   ├── risk/                  # capital-at-risk, trailing stop, concentration (FR42-45,47)
│   │   ├── rounding.rs            # named rounding mode + per-field display scale
│   │   └── method_version.rs      # method/formula version constant
│   └── tests/
│       ├── golden/                # frontier golden fixtures (synthetic, documented provenance)
│       ├── fixtures/
│       └── properties.rs          # proptest: monotonicity, continuity, idempotence, scale-homogeneity, metamorphic
│
├── contract/                      # steadyinvest-contract — versioned serde data contract (no Slint, no SQL)
│   ├── Cargo.toml                 # deps: serde, rust_decimal, uuid
│   └── src/
│       ├── lib.rs
│       ├── study.rs               # Study, Judgment snapshot (FR2,49-51)
│       ├── cell.rs                # value + source × freshness × review tri-state (FR17-20)
│       ├── provenance.rs          # (source, logical_version, timestamp, hash_of_dependencies)
│       ├── portfolio.rs           # portfolio (=banking relationship), holding, transaction (FR36-41)
│       ├── fx.rs                  # dated, source-aware FX rate (FR28)
│       ├── versioning.rs          # schema_version (int) + method_version (string)
│       └── export.rs              # portable export envelope (JSON + schema_version + integrity hash, FR59-61)
│
├── ingestion/                     # steadyinvest-ingestion — providers + normalization (FR15-16,21-27)
│   ├── Cargo.toml                 # deps: reqwest (rustls-tls,json), tokio (current_thread), serde, thiserror, contract
│   ├── src/
│   │   ├── lib.rs
│   │   ├── provider.rs            # MarketDataProvider trait; keys injected (not read here)
│   │   ├── adapters/
│   │   │   └── eodhd.rs           # first adapter (CH/EU+US)
│   │   ├── normalize/             # IFRS↔GAAP, split/series, fiscal-period, currency-of-report
│   │   ├── reconcile.rs           # non-destructive: manual wins, provider preserved, divergence→?
│   │   └── error.rs               # IngestionError (network/quota/key) — thiserror
│   └── tests/fixtures/            # recollage goldens (split, fiscal change, currency rebasing)
│
├── persistence/                   # steadyinvest-persistence — rusqlite storage (hybrid model)
│   ├── Cargo.toml                 # deps: rusqlite (bundled), contract, serde_json, thiserror
│   ├── src/
│   │   ├── lib.rs
│   │   ├── journal.rs             # open/create; journal_id (UUID) + monotonic logical version
│   │   ├── schema.rs              # normalized tables (portfolios, holdings, transactions, fx_rates, watchlist_items)
│   │   ├── studies.rs             # studies/judgments as TEXT JSON blob + indexed columns
│   │   ├── migrations/            # PRAGMA user_version steps; lazy upgrade on save
│   │   ├── consolidation.rs       # pull rows → compute in Rust w/ core (per-currency→bank→global)
│   │   ├── export_import.rs       # JSON export/import + integrity/version checks; restore (FR59-61)
│   │   ├── backup.rs              # versioned backup to configurable target (carries id,version,hash)
│   │   ├── sync_guard.rs          # sync-path detection; WAL↔DELETE journal_mode; single-instance lock
│   │   └── error.rs
│   └── tests/corpus/              # frozen versioned-journal corpus v{N}.db (append-only)
│
├── report/                        # steadyinvest-report — PDF/print (UI-independent, does I/O) (FR52-53)
│   ├── Cargo.toml                 # deps: genpdf/printpdf, core, contract
│   └── src/lib.rs                 # faithful SSG layout, neutral labels, grayscale-safe
│
└── app/                           # steadyinvest-app — thin Slint UI (binary)
    ├── Cargo.toml                 # deps: slint, tokio, directories, keyring, tracing, core, contract, ingestion, persistence, report
    ├── build.rs                   # slint-build
    ├── assets/                    # bundled static resources
    │   ├── fonts/                 # Inter (OFL) + tabular-figures numeric font (UX spec)
    │   ├── icons/                 # neutral app logo/icon (CR #1)
    │   └── demo_study.json        # read-only demonstration study / golden seed (FR62), synthetic
    ├── src/
    │   ├── main.rs                # entry; single-instance; loads last-used journal
    │   ├── state.rs               # immutable StudyState snapshot; undo stack; content-addressed verdict
    │   ├── viewmodel/             # ADAPTER: domain types → Slint structs; money → formatted strings
    │   ├── config.rs              # app-config via directories (last path, recents, UI prefs)
    │   ├── keychain.rs            # keyring access; injects keys into ingestion
    │   ├── clock.rs               # Clock + IdGen providers (injected; fixed in tests)
    │   ├── fetch.rs               # tokio worker; invoke_from_event_loop marshalling
    │   ├── theme.rs               # token single-source (arc_swap); pushes to UI on theme/regime change
    │   ├── i18n.rs                # @tr() wiring (French-first) — distinct from label set
    │   └── labels.rs              # NAIC↔neutral label set (runtime-swappable data)
    └── ui/                        # .slint files (snake_case files, PascalCase components, kebab props)
        ├── app.slint              # nav rail + top bar + sticky verdict bar
        ├── study_screen.slint     # faithful collapsible SSG form (§1–§5)
        ├── components/
        │   ├── data_grid.slint        # dense editable grid, paste-a-column, cell cursor (FR16,56)
        │   ├── growth_chart.slint     # §1 semi-log, draggable trend lines (Path/TouchArea, FR30-33)
        │   ├── zone_bar.slint         # §4 vertical Buy/Hold/Sell + price axis (live recolor)
        │   ├── verdict_badge.slint    # full/provisional/degraded/withheld (FR12)
        │   ├── trust_markers.slint     # ✓/?/missing/stale (confusability-gated)
        │   ├── error_banner.slint      # neutral global banner (network/quota/key)
        │   └── legend_help.slint        # legend, glossary popover, demo study (FR57,62)
        └── screens/
            ├── dashboard.slint     # list/search/sort/filter studies (FR54-55)
            ├── watchlist.slint     # FR34-35
            ├── portfolio.slint     # holdings, capital-at-risk, stop, sell/raise-stop (FR36,40,42,46)
            └── settings.slint      # no-wizard: provider/key, currency, thresholds, labels, locale (FR63)
```

*Test fixtures:* each crate owns its fixtures for now (`core/tests/`, `ingestion/tests/`,
`persistence/tests/corpus/`); a shared dev-only synthetic-fixtures crate can be mutualised later if
drift appears.

### Architectural Boundaries

- **Calc boundary (Cardinal Rule):** `core` has zero I/O/UI/SQL/net deps — all SSG/risk math lives
  here and nowhere else. Guarantees the GUI choice stays reversible and the math is auditable.
- **Contract boundary:** `contract` is the only shared vocabulary across `ingestion`, `persistence`,
  `report`, `app` (and the future MCP/AI façade). Its `schema_version`/`method_version` gate migrations.
- **Persistence boundary:** only `persistence` touches SQLite. Decimal arithmetic for consolidation is
  done in Rust (pull rows → compute with `core`), never via SQL on TEXT money columns.
- **UI boundary:** `app` is the only crate depending on Slint; domain types cross into `.slint` solely
  through the `viewmodel/` adapter (money as formatted strings, no floats, no domain structs leaked).
- **Network boundary:** only `ingestion` makes network calls (reqwest/tokio); keys are injected by
  `app` (from keyring), never read inside `ingestion` — keeps it testable offline.
- **No server / no public API boundary in v1** — the contract is the seam for a later read-only MCP.

### Requirements to Structure Mapping

| FR cluster | Primary location |
|---|---|
| FR1-8 Stock Study & engine | `core/ssg/`, `core/verdict.rs`, `contract/study.rs`, `app/ui/study_screen.slint` |
| FR9-14 Calc integrity & trust | `core` (+ `tests/golden`,`properties.rs`), `app` verdict rendering |
| FR15-29 Acquisition/provenance/providers | `ingestion/`, `contract/{cell,provenance,fx}.rs`, `persistence` cache |
| FR30-33 Charts & judgment | `app/ui/components/{growth_chart,zone_bar}.slint`, `app/state.rs` |
| FR34-35 Watchlist & alerts | `persistence` (watchlist), `app/ui/screens/watchlist.slint` |
| FR36-41 Portfolio/transactions | `contract/portfolio.rs`, `persistence/{schema,consolidation}.rs`, `core/risk/` |
| FR42-48 Risk management | `core/risk/`, `app/ui/screens/portfolio.slint` |
| FR49-51 Cumulative memory/journal | `contract/{study,provenance}.rs`, `persistence/journal.rs` |
| FR52-53 Reporting/PDF | `report/` |
| FR54-62 App shell & data mgmt | `app/ui/screens/dashboard.slint`, `persistence/export_import.rs`, `app/config.rs` |
| FR63-66 Config/posture | `app/{config,keychain,labels,i18n}.rs`, `app/ui/screens/settings.slint` |
| Added: DB location + recent journals | `app/config.rs`, `persistence/{journal,sync_guard}.rs` |

### Integration Points

- **Internal:** `app` orchestrates; reads/writes via `persistence`; computes via `core`; fetches via
  `ingestion`; renders PDF via `report`. All data shapes are `contract` types. Cross-thread results
  return through `invoke_from_event_loop`.
- **External:** market-data providers (HTTP, user's key) via `ingestion` adapters only; OS secret
  store via `keyring`; OS config dirs via `directories`; external backup target (NAS) via file export.
- **Data flow:** provider → `ingestion` normalize/reconcile → `contract` types (provenance stamped) →
  `persistence` (journal) → `core` recompute (native currency) → `app` viewmodel → Slint render;
  consolidation/FX applied only at the `persistence`/portfolio layer.

### Development Workflow Integration

- **Dev:** `just run` (app), `just spike` (week-1 chart spike), `just test`, `just lint`.
- **Build:** `cargo build --release` per OS produces a single native binary; `Cargo.lock` committed.
- **CI/Deploy:** `.github/workflows/ci.yml` runs the 3-OS matrix (fmt, clippy -D warnings, tests,
  trust gates, determinism hash, `cargo deny` license audit); distribution = the per-OS binary
  (manual update in v1).

## Architecture Validation Results

### Coherence Validation ✅

**Decision Compatibility:** All technology choices are mutually compatible and version-verified
(June 2026): Slint 1.16.1 (MSRV 1.88) · rusqlite 0.40 (bundled) · rust_decimal 1.42 (+maths) ·
reqwest 0.13 (rustls-tls) + tokio 1.52 · thiserror 2.0 · proptest 1.9 · keyring 3.x (NOT 4.0) · directories ·
tracing. The **Slint GPLv3 licence is compatible with the project's GPL-3.0** (the PRD's "Slint
licensing tier" risk is closed, pending the `cargo deny` dependency audit). No contradictory
decisions remain (egui fully removed; no web; no server).

**Pattern Consistency:** Patterns support the decisions — exact-decimal numerics + named rounding
back the deterministic-engine decision; the immutable-snapshot/content-addressed-verdict pattern
realizes the Foundational Invariant; the Slint view-model adapter + "money as formatted strings"
enforce the UI boundary; injected Clock/IdGen back determinism and testability; the two-axis i18n
(`@tr()` vs runtime label set) matches the neutral-posture/label-swap requirement.

**Structure Alignment:** The 6-crate workspace enforces the boundaries: `core` (no I/O) holds the
Cardinal Rule; `contract` is the shared seam (and future MCP boundary); only `persistence` touches
SQLite; only `ingestion` touches the network; only `app` touches Slint; `report` isolates PDF I/O.
Every boundary in the decisions maps to a crate.

### Requirements Coverage Validation

**Functional Requirements Coverage ✅:** All 66 FRs map to a crate/module (see Requirements-to-
Structure table). Phase tags preserved (P1 in MVP scope; P2/P3/V located but deferred). Two coverage
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
- *Correctness* — exact decimal + deterministic core + golden/property/metamorphic + 3-OS
  determinism hash in CI. ✅
- *Performance* — `<~1 s` recompute and `<~3 s` launch within reach; **`<100 ms` judgment-line
  recolor targeted but NOT yet proven** in native Slint (see Gap). ⚠️
- *Security/Privacy* — keychain-only secrets, no telemetry, all-local, keys injected not stored in
  `ingestion`. ✅
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
  auditable; the future read-only MCP/AI façade is near-free.
- Slint-only (no web, no egui embedding) removes the entire dual-render-paradigm risk class.

**Areas for Future Enhancement:**
- P2/P3/V features (multi-portfolio/FX depth, transaction ledger, dividends, Company Comparison,
  Portfolio Health Review, screening, read-only AI clerk, other-form PDFs) — located, deferred.
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

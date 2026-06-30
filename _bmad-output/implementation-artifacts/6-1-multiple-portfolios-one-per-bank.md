# Story 6.1 — Multiple portfolios, one per bank/account (FR37)

Status: done

## Story

As Guy,
I want to keep more than one portfolio (one per bank or account),
so that my holdings are organised the way my accounts actually are.

## Acceptance Criteria

1. **AC1 — Multi-portfolio CRUD in `persistence` (FR37), migration-free.** The `portfolios` table (`id, name, created_at`, frozen in v1 DDL) already supports many named rows and `holdings.portfolio_id` already scopes holdings per portfolio (with `idx_holdings_portfolio_id`). This story adds the typed CRUD that Story 4.3 deferred: `add_portfolio(id, name, now)`, `list_portfolios()` (deterministic order), `rename_portfolio(id, name)`, `delete_portfolio(id)`. **No schema change** (`PRAGMA user_version` unchanged, `contract::SCHEMA_VERSION` unchanged). Each real mutation bumps `journal_meta.logical_version` (NFR-R2); a no-op (rename to the identical name, delete of an absent id) bumps nothing (the Epic-3 C4 guard). `delete_portfolio` is **guarded**: it refuses a portfolio that still has **any** holding row (active **or** sold — sold rows still exist and their sell transactions FK to them, so refusing protects the whole FK chain), and refuses deleting the **last** portfolio (the register always has at least one) — both neutral, cause-named refusals, never a panic, never an orphaned holding FK.

2. **AC2 — An "active portfolio" replaces the singleton assumption in `app`.** `JournalState` today wraps the holdings rails around `first_portfolio()`/`ensure_default_portfolio()` (the 4.3 single-portfolio shape). This story introduces a selected/active portfolio: holdings list + add/edit/remove + the capital-at-risk / zone facts all scope to the **active** portfolio. The active portfolio id is persisted in `AppConfig` (last-selected, restored on launch — like `recent_journals`); on first run / a missing id it falls back to the first portfolio (deterministic). The existing single default portfolio (from 4.3) becomes the first named portfolio — **no data migration**, existing journals just gain the ability to add more.

3. **AC3 — Portefeuille screen: switch + manage portfolios.** The Portefeuille screen gains a portfolio selector (pick the active one) and management actions: add a portfolio (name field — "one per bank/account", e.g. "UBS — compte titres"), rename, and delete (with the AC1 guards surfaced as neutral notices). The holdings register below shows the active portfolio's holdings. The **reference currency stays global** (the 4.3 decision; multi-currency per holding is Story 6.2 — out of scope here). Neutral copy (FR13), posture-gated.

4. **AC4 — `core`/`contract` untouched; method frozen; no new dependency.** No `core::ssg`/`core::risk` change (multi-portfolio is a persistence+app concern). `PortfolioItem` already derives serde; if the app needs a portfolio list across the wire it reuses existing `contract`/persistence types — **no `contract` API change, no new external dependency** (`Cargo.lock`/`deny.toml` unchanged). Migration-free. The whole-journal export/import (5.3) already carries `portfolios: Vec<PortfolioItem>` — confirm multiple portfolios round-trip through it (they already should; add a test).

## Tasks / Subtasks

- [x] **Task 1 — `persistence`: multi-portfolio CRUD (AC1)** — `persistence/src/holdings.rs`
  - [x] `add_portfolio(id, name, now)` (INSERT, bump version); `list_portfolios()` (ORDER BY created_at, id — deterministic); `rename_portfolio(id, name)` (UPDATE, no-op guard on identical name → no bump); `delete_portfolio(id)` (guarded: error if any non-sold holding references it; error if it is the last portfolio; else DELETE + bump).
  - [x] Keep `ensure_portfolio`/`first_portfolio` (still the bootstrap for an empty journal). `add_holding`/`list_holdings`/`update_holding`/`delete_holding` already take `portfolio_id` — unchanged.
  - [x] Tests: add → list (ordered); rename + identical-name no-op (no version bump); delete-with-holdings refused; delete-last refused; delete-empty-non-last succeeds + bumps; FK never orphaned.

- [x] **Task 2 — `app` state: the active portfolio (AC2)** — `app/src/state.rs`, `app/src/config.rs`
  - [x] `AppConfig.active_portfolio_id: Option<Uuid>` (serde-default, validate-on-read), recorded on switch + restored on launch; fallback to the first portfolio when absent/stale.
  - [x] `JournalState`: `active_portfolio()` (selected or first), `set_active_portfolio(id)`, `list_portfolios()`, `add_portfolio(name)`, `rename_portfolio(id, name)`, `delete_portfolio(id)` rails (validation + neutral messages, mirroring the holdings rails). Re-point the holdings rails (`list_holdings`, add/edit/remove, the at-risk/zone facts) at the active portfolio instead of `first_portfolio`/`ensure_default_portfolio`.
  - [x] Tests: add a 2nd portfolio; switch active; holdings scope to the active one; the active id persists across a reopen; deleting the active portfolio reselects a remaining one.

- [x] **Task 3 — `app` UI: selector + management (AC3)** — `app/src/main.rs`, `app/ui/screens/portfolio.slint`, `app/ui/state.slint`
  - [x] A `Portfolios` Slint surface (the list + the active id) + a selector in the Portefeuille screen; add (name field) / rename / delete actions wired to the state rails; neutral refusal notices for the guarded deletes. The holdings register reads the active portfolio.
  - [x] Posture: `@tr` floor bumped by the exact number of new literals; any new `MSG_*` registered + inventory bumped.

- [x] **Task 4 — Gates (AC4)** — fmt, clippy `-D`, `test --workspace`, `deny` + smoke. Confirm: **no `core`/`contract` change**; **no migration** (`user_version` unchanged, `SCHEMA_VERSION` unchanged); **no new dependency** (`Cargo.lock`/`deny.toml` unchanged); multiple portfolios round-trip through the 5.3 whole-journal export/import; `@tr`/MSG inventories bumped exactly.

## Dev Notes

### Scope
- Multiple **named** portfolios + a switcher + holdings scoped to the active one (FR37). The reference currency stays **global** (4.3) — **multi-currency holdings are Story 6.2**, the transaction ledger is 6.3, dividends 6.4, FX 6.5, per-currency/bank capital-at-risk 6.6. None of those are in 6.1.
- This is the **structural opener** of Epic 6 (the Epic-4 "open with the structural story" lesson) — and it is **migration-free** because Story 1.10 pre-provisioned `portfolios`/`holdings` with the multi-portfolio shape (the same finding that made Epic 4 additive).

### Architecture decisions this story honours
- [v1 DDL — Story 1.10] — `portfolios(id, name, created_at)` + `holdings.portfolio_id REFERENCES portfolios(id)` + `idx_holdings_portfolio_id` already exist → typed CRUD + UI, **no new table, no migration**.
- [Story 4.3 — single portfolio] — `ensure_portfolio`/`first_portfolio` are the 4.3 singleton; 6.1 generalizes to N and replaces the app-side singleton assumption with an **active portfolio**. The default portfolio becomes the first named one (no data migration).
- [Epic-3 C4 / NFR-R2] — every real mutation bumps `logical_version`; no-ops (identical rename, absent delete) bump nothing.
- [read-schema-first / read-IO-first — Epic-5 E3] — `delete_portfolio` must respect the `holdings.portfolio_id` FK (refuse a portfolio with holdings, like 4.7's sell-FK lesson) and the "always ≥1 portfolio" invariant — designed in up front, not discovered by a test failure.
- [reference currency stays global — 4.3, [[project_reference_currency]]] — 6.1 does NOT introduce per-portfolio or per-holding currency (that's 6.2). `fx_rates` stays inert until 6.2/6.5.

### Where things live
- `persistence/src/holdings.rs` — the new portfolio CRUD beside the existing holdings/portfolio code.
- `app/src/state.rs` + `app/src/config.rs` — the active-portfolio rails + `AppConfig.active_portfolio_id`.
- `app/src/main.rs` + `app/ui/screens/portfolio.slint` + `app/ui/state.slint` — the selector + management UI.

### References
- [epics.md#Story 6.1] — multiple portfolios, one per bank/account (FR37).
- [persistence/src/holdings.rs] — `ensure_portfolio`/`first_portfolio`/`add_holding(portfolio_id)`/`list_holdings(portfolio_id)` (already portfolio-scoped).
- [persistence/src/schema.rs §136–152] — the frozen `portfolios`/`holdings` DDL (no migration needed).
- [persistence/src/export.rs] — `JournalSnapshot.portfolios: Vec<PortfolioItem>` (the 5.3 round-trip to confirm).
- [4-3-single-portfolio-holdings-register.md] — the single-portfolio shape being generalized.

## Dev Agent Record

### Agent Model Used
Claude Opus 4.8 (1M context).

### Debug Log References
Gates green: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -D warnings`, `cargo test --workspace --locked` (**605 tests**), `cargo deny check` (advisories/bans/licenses/sources ok), smoke `timeout cargo run -p steadyinvest-app` (exit 124).

### Completion Notes List
- **AC1 — multi-portfolio CRUD, migration-free.** `persistence/src/holdings.rs`: `add_portfolio` (insert + bump); `rename_portfolio` (`WHERE id=?1 AND name<>?2` → identical-name no-op, no bump); `delete_portfolio -> DeletePortfolioOutcome{Deleted, HasHoldings, LastPortfolio}` (guards has-holdings — counting ALL rows incl. sold, so the sell-transaction FK chain is never orphaned — and the last-portfolio invariant). `list_portfolios` pre-existed (5.3). NO schema change (`user_version`/`SCHEMA_VERSION` untouched).
- **AC2 — active portfolio.** `JournalState.active_portfolio_id: Option<Uuid>` (in-memory) + `active_portfolio()` (selected-or-first, validates against the live list), `set_active_portfolio` (stale id ignored), `add/rename/delete_portfolio` rails (neutral messages). `list_holdings`/`add_holding`/capital-at-risk now scope to the active portfolio. `AppConfig.active_portfolio_id: Option<String>` (serde-default) persists it; restored on launch with a fallback to the first. The 4.3 default becomes the first named portfolio — no data migration.
- **AC3 — Portefeuille UI.** A `ChoiceChip` selector (shown when >1, in a horizontal `Flickable` so many portfolios scroll, not clip) + add (name) / rename / delete controls; the register reads the active portfolio. Reference currency stays GLOBAL (`PortfolioRow` carries only `{id, name}`; no per-portfolio currency — that's 6.2).
- **AC4 — guardrails.** `git diff main` on `core/`, `contract/`, `Cargo.lock`, `Cargo.toml`, `deny.toml`, `schema.rs`, `migrations.rs` is **empty** (no core/contract/dep/migration change). The 5.3 whole-journal IMPORT was single-portfolio (forced all holdings into one resolved portfolio) → rewritten to **upsert ALL portfolios by id** and attach each holding to its own `portfolio_id`; atomic, round-trip test added.
- **3-layer adversarial review (Blind / Edge / Acceptance) — 4 patches, 0 defer:**
  - **MED (Edge):** removing the explicit guard made a holding→absent-portfolio import leak a raw `FOREIGN KEY constraint failed` SQLite error (FR13 regression). Restored a neutral pre-check → `Error::ImportMalformed`; test `a_holding_referencing_an_absent_portfolio_is_neutral_malformed_not_a_raw_fk_error`.
  - **MED (Blind+Edge / NFR-R2):** the import version-bump heartbeat only counted portfolio **inserts**, so a portfolios-only re-import that merely renamed a portfolio didn't bump `logical_version`. Fixed `applied` to include `!snapshot.portfolios.is_empty()` (consistent with studies/holdings); test `a_name_only_portfolio_update_on_import_bumps_the_version`.
  - **LOW-MED (Blind+Edge):** an all-sold portfolio shows an empty register but delete is (correctly) refused — softened `MSG_PORTFOLIO_HAS_HOLDINGS` to "contient un historique de positions" so the copy matches.
  - **LOW (Edge):** the selector clipped with many portfolios → wrapped in a horizontal `Flickable`.
  - Acceptance Auditor: all 4 ACs PASS, no guardrail violations. (Cosmetic non-fixes noted: same-name chips, absent-id delete semantics — unreachable from the UI.)
- Posture: `@tr` floor 315→320 (5 new literals); `USER_FACING_MESSAGES` 75→78 (3 new MSG). 605 tests; all gates green.

### File List
- `persistence/src/holdings.rs` — `add/rename/delete_portfolio` + `DeletePortfolioOutcome`.
- `persistence/src/export.rs` — multi-portfolio import (upsert all by id, attach holdings to own portfolio_id, malformed guard, NFR-R2 bump fix) + 2 tests.
- `persistence/src/lib.rs` — export `DeletePortfolioOutcome`.
- `persistence/tests/holdings.rs` — 4 multi-portfolio CRUD tests.
- `app/src/state.rs` — active-portfolio rails + 3 MSG + 3 tests (incl. the whole-journal round-trip).
- `app/src/config.rs` — `AppConfig.active_portfolio_id`.
- `app/src/main.rs` — the 4 portfolio callbacks + startup restore + selector push in `refresh_holdings`.
- `app/src/posture.rs` — `@tr` floor 320, MSG inventory 78.
- `app/ui/state.slint` — `PortfolioRow` struct + the Holdings global's portfolio list/active-id/callbacks.
- `app/ui/screens/portfolio.slint` — selector (Flickable) + add/rename/delete management.
- `app/ui/app.slint` — re-export `PortfolioRow`.

### Change Log
- 2026-06-30 — Story 6.1 implemented (multiple portfolios, one per bank/account; active-portfolio scoping; migration-free typed CRUD on the v1 tables; 5.3 import generalized to multi-portfolio). 3-layer review: 4 patches, 0 defer. 605 tests; all gates green. Status → done. **Opens Epic 6.**

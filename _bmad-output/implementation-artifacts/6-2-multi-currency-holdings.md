# Story 6.2 — Multi-currency holdings (FR38)

Status: done

## Story

As Guy,
I want each holding to record the **currency it is actually denominated in**,
so that a EUR position and a USD position are stored and read honestly in their own currency instead of being silently forced into one reference currency.

## Acceptance Criteria

1. **AC1 — A per-holding `currency`, stored native, migration v5→v6 (FR38, FR28-storage half).** A holding carries the currency it is denominated in (e.g. `CHF`, `EUR`, `USD`). This lands as **additive migration step 6**: `ALTER TABLE holdings ADD COLUMN currency TEXT` — **nullable**, metadata-only, forward-safe (existing v1–v5 rows read `NULL`), `DDL_V1` stays frozen, matching the v2/v3/v4 pattern at `persistence/src/schema.rs:37-71`. `PRAGMA user_version` goes **5 → 6**; `contract::SCHEMA_VERSION` stays **1** (it versions the export envelope, not the DB — unchanged since v1). The persistence layer stays **currency-agnostic**: it never knows the app's reference currency, so it does **not** backfill. A `NULL` currency means "a pre-6.2 holding" and is interpreted by the app (AC2) as the reference currency — **no on-open rewrite, no phantom version bump**. Amounts stay **native and never mixed** (FR28): `quantity`/`purchase_price` remain exact-decimal **TEXT** (NFR-C1, no REAL); this story stores currency alongside them and performs **no FX conversion whatsoever**.

2. **AC2 — `HoldingItem.currency` threaded through the contract + CRUD; `NULL` coalesces to the reference currency.** `HoldingItem` (`persistence/src/holdings.rs:39-56`) gains `pub currency: Option<String>` (`None` = the legacy/pre-6.2 row). `add_holding` (`holdings.rs:117`) takes the currency and writes it; `list_holdings` (`holdings.rs:241`) and every other `HoldingItem` read select it; `record_sell` and the soft-delete path are unaffected (they don't touch amounts). The app resolves a holding's **effective currency** at the read boundary: `holding.currency` when set, else `AppConfig::reference_currency_or_default()` — so an old journal renders exactly as before (everything in the reference currency) until a currency is chosen. **Every mutating call still bumps `journal_meta.logical_version`** (NFR-R2); an edit to an identical currency is a no-op that bumps nothing (the Epic-3 C4 idempotency guard).

3. **AC3 — Capital-at-risk becomes per-currency subtotals; `core` stays frozen.** Because holdings can now differ in currency, the single global sum in `portfolio_capital_at_risk` (`app/src/state.rs:1179-1201`) would silently add unlike currencies — **forbidden** (FR28). Replace it with a **per-currency** read: group the active portfolio's holdings by effective currency, and for **each** currency bucket call the **existing, unchanged** `core::risk::capital_at_risk` / `total_invested` (each already documents "the caller sums one reference currency" — now the caller calls it once per currency). Return a **deterministically ordered** map `currency → (capital_at_risk, total_invested)`. There is **no consolidated global total** in this story — cross-currency consolidation needs FX and is **Story 6.5 (FR28) + Story 6.6 (FR44)**. **`core::risk` is NOT modified** (`PositionRisk` unchanged, no currency field) — the grouping lives entirely in the app, mirroring how 6.1 kept `core`/`contract` untouched.

4. **AC4 — Portefeuille UI: pick a currency from a fixed allow-list; register + risk panel show it per-currency.** Adding/editing a holding offers a currency chooser backed by a **fixed allow-list** (e.g. `CHF`, `EUR`, `USD`, `GBP`, `JPY` — a small `const` list, extensible later), **not** a free-text 3-letter field. The default selection is the current reference currency. The holdings register shows each holding's currency next to its amounts, and the capital-at-risk panel shows **per-currency subtotals** (e.g. "CHF : … · USD : …"), never a mixed-currency total. Neutral copy (FR13), posture-gated. The **reference currency stays the global app-config value** ([[project_reference_currency]]) — 6.2 adds per-holding currency, it does not make the reference currency per-portfolio.

5. **AC5 — No new dependency; `fx_rates` stays inert; round-trips through the whole-journal export.** No new external crate (`Cargo.lock`/`deny.toml` unchanged). The `fx_rates` table (`schema.rs:168-176`) stays **inert** — no CRUD, no reads (FX acquisition is Story 6.5). The whole-journal export/import (Story 5.3, `persistence/src/export.rs`) already carries `holdings: Vec<HoldingItem>`; confirm `currency` round-trips (it rides on the struct — add a test) and that a holding with `currency: None` and one with an explicit currency both survive export → import unchanged. All gates green (fmt, clippy `-D`, `test --workspace`, `deny`, smoke).

## Tasks / Subtasks

- [x] **Task 1 — `persistence`: migration v6 + `HoldingItem.currency` + CRUD (AC1, AC2)** — `persistence/src/schema.rs`, `persistence/src/migrations.rs`, `persistence/src/holdings.rs`
  - [x] Add `migrate_to_v6(tx)` in `schema.rs`: `ALTER TABLE holdings ADD COLUMN currency TEXT` (nullable), with a doc comment in the v2–v5 house style (additive, forward-safe, `DDL_V1` frozen, no backfill — the app owns the `NULL`→reference-currency meaning).
  - [x] Register `(6, crate::schema::migrate_to_v6)` in the `REGISTRY` array (`migrations.rs:27-31`). The `SIX_STEP_REGISTRY` / "Migration { version: 6 }" tests (`migrations.rs:163-242`) already anticipate a v6 slot — make them pass against the real step.
  - [x] `HoldingItem`: add `pub currency: Option<String>` (place it near `security_ticker`/amounts; keep serde derives — the 5.3 export carries it). Update the doc comment (no longer "the single reference currency").
  - [x] `add_holding`: add a `currency: Option<String>` parameter (or `&str` — dev's call, but the stored value is the chosen code), INSERT it into the new column. `list_holdings` + any other `SELECT … FROM holdings` add `currency` to the column list and map it. `update_holding` (edit path) carries currency; an identical-value edit stays a no-op (no version bump).
  - [x] Tests: fresh-v6 schema == migrated-v5 schema; add a holding with `EUR` → reads back `Some("EUR")`; a pre-6.2 row (NULL) reads back `None`; edit currency bumps version, identical-currency edit does not; `record_sell` still soft-deletes a multi-currency holding fine.

- [x] **Task 2 — `app` state: effective currency + per-currency capital-at-risk (AC2, AC3)** — `app/src/state.rs`, `app/src/config.rs`
  - [x] A `SUPPORTED_CURRENCIES: &[&str]` const (allow-list) + a membership validator (reuse/extend `is_valid_currency_code` for format, add allow-list membership). The reference-currency default must be a member (or fall back gracefully).
  - [x] An "effective currency" helper: `holding.currency` when `Some`, else `AppConfig::reference_currency_or_default()`.
  - [x] Replace `portfolio_capital_at_risk() -> (Decimal, Decimal)` with a per-currency read returning an **ordered** `Vec<(String, Decimal, Decimal)>` or `BTreeMap<String, (Decimal, Decimal)>` (`currency → (car, invested)`): group holdings by effective currency, and per bucket build `PositionRisk`s and call the **unchanged** `core::risk::capital_at_risk` / `total_invested`. Deterministic order (sort by currency code). Keep the map empty for an empty portfolio.
  - [x] The holdings add/edit rails thread the chosen currency (validated against the allow-list; a bad value → a neutral message, never a panic).
  - [x] Tests: two holdings in different currencies → two subtotal buckets, each summed independently, **no** cross-currency total; a legacy `None` holding lands in the reference-currency bucket; an all-one-currency portfolio yields a single bucket equal to the old single sum (regression parity).

- [x] **Task 3 — `app` UI: currency chooser + per-currency display (AC4)** — `app/src/main.rs`, `app/ui/screens/portfolio.slint`, `app/ui/state.slint`
  - [x] A currency chooser on the add/edit-holding form (a `ChoiceChip`/dropdown fed by the allow-list; default = reference currency), wired to the state rails.
  - [x] The holdings register shows each row's currency beside its amounts; the capital-at-risk panel renders **per-currency subtotals** (label each bucket with its code; no mixed total).
  - [x] Posture: `@tr` floor bumped by the **exact** number of new literals; any new `MSG_*` registered + the MSG inventory bumped exactly. (6.1 left `@tr` floor at **320**, MSG inventory at **78**.)

- [x] **Task 4 — Gates + export round-trip (AC5)** — fmt, clippy `-D`, `test --workspace`, `deny`, smoke
  - [x] Confirm: **no `core`/`contract` API change** (`git diff main` on `core/`, `contract/` empty apart from — there should be nothing); **no new dependency** (`Cargo.lock`/`deny.toml` unchanged); `fx_rates` still has no CRUD/reads; `contract::SCHEMA_VERSION` unchanged; `user_version` is 6.
  - [x] Whole-journal export/import (5.3) round-trips `currency` for both a `Some` and a `None` holding — add a test.
  - [x] Bump `@tr` / MSG inventories to the exact new counts; smoke `cargo run -p steadyinvest-app` (exit 124).

## Dev Notes

### Scope
- **In:** a per-holding `currency` (migration v6), stored native and never mixed (FR28-storage), threaded through `HoldingItem` + CRUD + the 5.3 export; a fixed-allow-list currency chooser in the Portefeuille UI; capital-at-risk shown as **per-currency subtotals**.
- **Out (explicit):** any FX rate acquisition or conversion (**Story 6.5**, FR28); a consolidated global capital-at-risk total in the reference currency, and the per-currency → per-bank → global roll-up (**Story 6.6**, FR44); the transaction ledger / partial sells / WAC (**Story 6.3**); dividends (6.4); concentration (6.7). `fx_rates` stays **inert** this story.

### Decisions locked with Guy (2026-07-02) — the "narrow" 6.2
- **Narrow scope:** store native, **no FX** in this story (FX = 6.5, global consolidation = 6.6). This keeps `fx_rates` inert and the story small.
- **Fixed allow-list** currency input (a `const` set), not a free 3-letter code — a guard-rail against typos and currencies the price provider can't serve.
- **Per-currency subtotals** for capital-at-risk, **no global total** until FX lands — the honest interim: we never sum unlike currencies.

### Architecture decisions this story honours
- **[additive-migration pattern — `schema.rs:37-71`]** v6 is a nullable `ADD COLUMN`, metadata-only, forward-safe; fresh-v6 == migrated-v5; `DDL_V1` frozen. The persistence layer stays currency-agnostic (no backfill — it can't know the app's reference currency), so `NULL` is meaningful and the app coalesces it. This dodges the on-open-rewrite / phantom-version-bump trap.
- **[`core`/`contract` frozen — the 6.1 parallel]** The per-currency grouping lives in the **app**; `core::risk::capital_at_risk`/`total_invested` are called **once per currency** and are **not modified** (`PositionRisk` keeps no currency field). FR44's cross-currency roll-up will extend `core::risk` — but that is **Story 6.6**, not here.
- **[never mix currencies in storage — FR28, Guy]** Amounts stay in their native currency as exact-decimal TEXT; no conversion anywhere in 6.2. The capital-at-risk read groups-then-sums, never sums-then-hopes.
- **[reference currency stays global — 4.3, [[project_reference_currency]]]** The app-config `reference_currency` remains the single global value; a `None`-currency holding renders under it. 6.2 does **not** make the reference currency per-portfolio.
- **[NFR-R2 / Epic-3 C4 idempotency]** Every real mutation bumps `logical_version`; an identical-currency edit bumps nothing.
- **[read-IO-semantics-first — Epic-5 E3]** Design the `NULL`-currency coalescing and the per-currency grouping **up front** (this note), not by discovering a mixed-currency sum in a test.

### Where things live
- `persistence/src/schema.rs` — `migrate_to_v6` beside `migrate_to_v5` (`schema.rs:81-95`); the frozen `holdings` DDL is `schema.rs:143-152`.
- `persistence/src/migrations.rs` — the `REGISTRY` array (`:27-31`); v6-anticipating tests at `:163-242`.
- `persistence/src/holdings.rs` — `HoldingItem` (`:39-56`), `add_holding` (`:117`), `list_holdings` (`:241`).
- `app/src/state.rs` — `portfolio_capital_at_risk` (`:1179-1201`), `list_holdings` (`:1161`), the holdings add/edit rails.
- `app/src/config.rs` — `DEFAULT_REFERENCE_CURRENCY` (`:27`), `reference_currency_or_default` (`:189`), `is_valid_currency_code` (`:161`); add `SUPPORTED_CURRENCIES` + membership check here.
- `app/src/main.rs` + `app/ui/screens/portfolio.slint` + `app/ui/state.slint` — the currency chooser + per-currency display.

### Previous story intelligence (6.1)
- 6.1 was **migration-free** (the v1 DDL pre-provisioned `portfolios`/`holdings.portfolio_id`); **6.2 is the first Epic-6 migration** — the `holdings` table had **no** currency column, so this one earns a real `ADD COLUMN` (v6).
- 6.1's review caught a **raw FK error leaking** on import (fixed to a neutral `ImportMalformed`) and a **version-bump heartbeat** that under-counted. Mirror both lessons: keep the currency round-trip through 5.3 import **neutral** on a bad value, and make sure a currency-only edit/import bumps `logical_version` exactly once (and an identical one not at all).
- 6.1 rewrote the 5.3 import to upsert **all** portfolios and attach each holding to its own `portfolio_id`. That import path now also carries `currency` on each `HoldingItem` — confirm it survives the merge unchanged (it rides the struct; add the round-trip test).

### References
- [epics.md#Story 6.2] — Multi-currency holdings (FR38); siblings 6.5 (FX/consolidation, FR28), 6.6 (capital-at-risk per currency→bank→global, FR44).
- [prd.md#FR38] — "The user can hold securities denominated in multiple currencies."
- [prd.md#FR28] — "acquires, timestamps and retains FX rates … FX is applied only at consolidation." (6.2 = the storage half; acquisition/consolidation = 6.5.)
- [persistence/src/schema.rs:37-71] — the v2–v5 additive-migration house style to copy for v6; `:143-152` frozen `holdings` DDL; `:168-176` inert `fx_rates`.
- [persistence/src/holdings.rs:39-56,117,241] — `HoldingItem` + `add_holding`/`list_holdings`.
- [core/src/risk/mod.rs:52-80] — `PositionRisk` / `capital_at_risk` / `total_invested` (unchanged; called once per currency).
- [app/src/state.rs:1179-1201] — `portfolio_capital_at_risk` → per-currency.
- [app/src/config.rs:24-193] — reference-currency config + validation; add the allow-list here.
- [6-1-multiple-portfolios-one-per-bank.md] — the multi-portfolio active-portfolio shape 6.2 builds on.

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — dev; Claude Fable 5 — review pass, task closure & merge (2026-07-02).

### Debug Log References

### Completion Notes List

- All 4 tasks implemented as specified: migration v6 (`migrate_to_v6`, REGISTRY slot 6, fresh==migrated parity via the introspection tests), `HoldingItem.currency: Option<String>` + `add_holding(&str)`/`list_holdings`/`update_holding` threading (identical-currency edit = no-op, currency-only edit bumps once), per-currency capital-at-risk (`portfolio_capital_at_risk_by_currency` → ordered `Vec<(String, Decimal, Decimal)>`, `core::risk` untouched), UI chooser fed by `SUPPORTED_CURRENCIES` (+ per-row currency labels + per-currency CaR panel), 5.3 export round-trip test incl. a legacy `None` holding.
- **Review pass (2026-07-02, whole-codebase review embarked in the same PR) found and fixed on the 6.2 slice:** (1) HIGH — `sell_holding` stamped the SELL transaction with the *reference* currency; it now stamps the holding's **own** effective currency (FR28), reference only as the legacy-NULL fallback, pinned by `sell_holding_stamps_the_holdings_own_currency_not_the_reference`; (2) MED — a reference-currency change only set the UI globals while the row labels/CaR buckets are baked at render → the callback now re-renders the register; (3) `reference_currency_or_default` now gates on allow-list **membership**, not just shape (a hand-edited off-list code would make a legacy row un-saveable); (4) JPY added to `SUPPORTED_CURRENCIES` (AC4's list); (5) the Réglages reference-currency chips now iterate the pushed allow-list model — one source of truth (`@tr` floor 322→318, currency codes are not translatable copy); (6) the Task-2 "effective currency" helper exists as `state::effective_currency` and is the single coalescing point (CaR buckets, sell stamp, row labels).
- **AC5 deviation, documented:** "no core/contract API change" holds for the 6.2 *slice*, but the PR embarks the 2026-07-02 whole-codebase review/reorg which DID touch core/contract internals (module splits `verdict/`+`compare/`, checked/saturating arithmetic, docs) — behavior-preserving: all digests, the method fingerprint and the 11 golden fixtures verified byte-identical; no public-API semantic change. `Cargo.lock` untouched; `deny.toml` gained two documented advisory ignores for the pre-existing quick-xml RUSTSEC-2026-0194/0195 (build-time-only Wayland codegen path, no compatible fix; tracked in #83) — not a 6.2 dependency change.
- Gates: 619 tests green (workspace), clippy 0 warnings, fmt clean, `cargo deny check` ok, smoke launch exit 124, `user_version` = 6, `SCHEMA_VERSION` = 1, `fx_rates` inert.
- Deferred to issues: #78 (export entity-field forward-compat policy — the 6.2 `currency` field is the proving case), #81 (holding↔study auto-match ignores currency — cross-currency sell price/stop ratchet).

### File List

- `persistence/src/schema.rs` — `migrate_to_v6` (nullable `holdings.currency`).
- `persistence/src/migrations.rs` — REGISTRY v6 slot + tests.
- `persistence/src/holdings.rs` — `HoldingItem.currency`, CRUD threading, doc refresh.
- `persistence/src/export.rs` + `persistence/tests/export.rs` — currency round-trip (incl. legacy `None`).
- `persistence/tests/holdings.rs`, `persistence/tests/transactions.rs`, `persistence/tests/readonly_newer.rs` — 6.2 tests.
- `app/src/config.rs` — `SUPPORTED_CURRENCIES` (+JPY), `is_supported_currency`, membership-gated `reference_currency_or_default`.
- `app/src/state/holdings.rs` — `effective_currency`, `portfolio_capital_at_risk_by_currency`, currency-aware add/update/sell rails.
- `app/src/state/messages.rs` — `MSG_HOLDING_INVALID_CURRENCY` (+ inventory).
- `app/src/wiring/holdings.rs`, `app/src/wiring/prefs.rs` — row currency labels, per-currency CaR push, re-render on reference change.
- `app/ui/screens/portfolio.slint`, `app/ui/screens/settings.slint`, `app/ui/state.slint` — currency chooser, per-row labels, per-currency CaR panel, unified currency chips.
- `app/src/posture.rs` — `@tr` floor 318 (documented).
- (Same PR, beyond 6.2 scope: the 2026-07-02 whole-codebase reorg — `app/src/state/`, `app/src/wiring/`, `core/src/verdict/`, `core/src/golden/compare/`, `ingestion/src/adapters/common.rs`, `persistence/src/util.rs` and the associated fixes/docs — see the PR body.)

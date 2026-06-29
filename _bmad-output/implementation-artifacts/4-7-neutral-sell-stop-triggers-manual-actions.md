# Story 4.7: Neutral sell / stop triggers with manual actions

Status: review (dev complete 2026-06-29 — 5/5 tasks; workspace 522 tests, fmt/clippy -D/deny green; core::ssg fingerprint/determinism/golden intact; migration v3→v4 [transactions kind/rationale + holdings sold_at]; contract/Cargo.lock/deny.toml unchanged)

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want neutral triggers that offer actions but never act for me,
so that I stay the sole decider, with the stop taking priority over the Sell zone.

## Acceptance Criteria

1. **AC1 — The trigger state + stop-priority rule (`core::risk`, FR46/FR47).** A **pure `core::risk` function** decides, per holding, whether a trigger fires and which one: given `stop_breached: bool` (Story 4.5's `stop_breached`) and `in_sell_zone: bool` (the matched study's present zone == `Zone::Sell`, Story 4.4), it returns `Option<TriggerKind>` where `TriggerKind ∈ { Stop, Sell }`. **When both conditions hold, the stop wins** (`Stop`) — this is the **isolated, testable FR47 business rule** (stop-loss takes priority over the Sell zone). Neither condition → `None`. Exact, deterministic, **decoupled from `core::ssg`** (beside the Story-4.5/4.6 risk primitives; the SSG fingerprint/golden/determinism gates stay green). Unit-tested for all four `(breached, in_sell_zone)` combinations + the priority.

2. **AC2 — A neutral inline trigger surface that never auto-acts (FR46/FR13).** On a triggered holding, the Portefeuille **row** reveals a **neutral fact** (ink, no hue, no imperative beyond the explicitly offered actions) that names the trigger — *"Le prix a atteint votre stop."* (`Stop`) or *"Le prix est dans la zone de vente."* (`Sell`) — and offers exactly three **manual** actions: **Vendre**, **Relever le stop**, **Ignorer**. The app **never acts on its own** (no auto-sell, no auto-adjust). The existing per-row stop fact (`◆ sous le stop`, Story 4.5) and zone marker (`◆ {zone}`, Story 4.4) stay intact; the trigger surface is additive. Saturated buy/hold/sell hues stay geofenced to the open study's §4 zone bar (Story 4.2 rule). Posture-gated: every new literal goes through `@tr`, the floor bumps by exactly the number added, and any new `MSG_*` is registered.

3. **AC3 — A chosen sell is recorded with an optional rationale (minimal; the full ledger stays Epic 6).** Choosing **Vendre** records **one SELL row** in the **pre-existing `transactions` table** (`occurred_at` = injected clock now; `quantity` = the holding's quantity; `unit_price` = the matched study's `current_price` if present, else the holding's `purchase_price`; `fees` = `0`; `currency` = the portfolio reference currency, FR63; `kind` = `"sell"`; `rationale` = the optional free-text reason, trimmed → `NULL` when blank), then **removes the holding** from the active register (`delete_holding`). A narrow **migration v3→v4** adds `kind` + `rationale` columns to `transactions`. **NO weighted-average cost basis, NO partial sells, NO buy recording** — the full FR39 ledger is **Epic 6 / Story 6.3**. The Story-4.6 capital-at-risk figure recomputes after the holding is removed (the register re-renders).

4. **AC4 — "Relever le stop" + "Ignorer"; decoupling, migration & gates.** **Relever le stop** reuses the Story-4.5 `set_trailing_stop` rail (tighten the stop via the existing per-row control / a pre-filled value) — no new persistence path. **Ignorer** dismisses the trigger surface for the **session only** (transient in-memory state, like the Story-4.4 `HoldingFreshnessMap`; never persisted, re-appears next launch if still triggered). `core::risk` stays decoupled (SSG gates green). The migration is the project's **4th** (`PRAGMA user_version` 3→4); **`contract::SCHEMA_VERSION` stays unchanged** (transactions is a normalized table, not a serde blob); **no new dependency** (`Cargo.lock` / `deny.toml` unchanged). Copy neutral, posture-gated.

## Tasks / Subtasks

- [x] **Task 1 — `core::risk`: the trigger state + stop-priority (AC1, AC4)** — `core/src/risk/mod.rs`
  - [x] `pub enum TriggerKind { Stop, Sell }` (`#[derive(Debug, Clone, Copy, PartialEq, Eq)]`) + `pub fn trigger_state(stop_breached: bool, in_sell_zone: bool) -> Option<TriggerKind>`: `Stop` when `stop_breached` (priority — even if also in the sell zone, FR47); else `Sell` when `in_sell_zone`; else `None`. A 3-line `match`/`if`, no arithmetic.
  - [x] Unit tests: all four `(breached, in_sell_zone)` combinations → `Some(Stop)` / `Some(Stop)` (both → stop wins) / `Some(Sell)` / `None`; an explicit `stop_priority_over_sell_zone` test asserting `(true, true) → Stop`. **Does NOT touch `core::ssg`** — re-confirm fingerprint/golden/determinism stay green.

- [x] **Task 2 — Migration v3→v4 + the SELL transaction write (AC3, AC4)** — `persistence/src/{schema.rs, migrations.rs, transactions.rs}`
  - [x] `schema::migrate_to_v4` = `ALTER TABLE transactions ADD COLUMN kind TEXT;` + `ALTER TABLE transactions ADD COLUMN rationale TEXT;` (both nullable — backward-compatible with any v1–v3 row; existing rows read `NULL`).
  - [x] `migrations.rs`: append `(4, crate::schema::migrate_to_v4)` to `REGISTRY`; **shifted the fake-future step `fake_v4` → `fake_v5`** (FIVE_STEP_REGISTRY, marker-v5, refuse 5→4, fail-v5→4); added forward test `v4_adds_the_transactions_kind_and_rationale_columns`; bumped `readonly_newer` `supported` 3→4 + `schema.rs` v2-test version assert 3→4; updated registry-doc comment.
  - [x] New `persistence/src/transactions.rs`: `record_sell(id, holding_id, quantity, unit_price, fees, currency, rationale: Option<&str>, now)` inserting one `kind = 'sell'` row (id/now injected — ADD15). Append-only; **bumps `logical_version`**. `list_transactions(holding_id)` read. `KIND_SELL`/`TransactionItem` re-exported. Decimals faithful TEXT.
  - [x] Integration test `persistence/tests/transactions.rs`: `record_sell` writes a `kind='sell'` row carrying the rationale (and `None` → `NULL`); decimals round-trip exactly.

- [x] **Task 3 — App state: the trigger read + `sell_holding` rail (AC1, AC2, AC3)** — `app/src/state.rs`
  - [x] The per-holding trigger is computed in `refresh_holdings` (Task 4) from the same `zone` + `stop_breached` the row already derives — no second source of truth in state.
  - [x] `sell_holding(&mut self, holding_id, rationale: &str, currency: &str) -> Result<(), String>`: read-only/no-journal/save-failure guarded; resolves `unit_price` (matched study `current_price` else `purchase_price`), `fees` = "0", trims the rationale (empty → `None`); `record_sell(...)` then **`mark_sold(holding_id)`** — NOT a hard delete, because the sell transaction's FK (`transactions.holding_id → holdings.id`) must keep a live referent, so the record survives while the holding leaves the register. `currency` passed in from main.rs (the reference currency lives in `AppConfig`). New `MSG_HOLDING_SOLD`; `USER_FACING_MESSAGES` 5→6.
  - [x] **`list_holdings` now filters `sold_at IS NULL`** (persistence) so a sold holding drops out of BOTH the register and the Story-4.6 capital-at-risk source automatically.
  - [x] Tests: `sell_holding` records the sell, drops the holding from `list_holdings`, and capital-at-risk falls to 0; an absent id is refused, register untouched.

- [x] **Task 4 — main.rs + Slint: the inline trigger surface + the three actions (AC2, AC3, AC4)** — `app/src/main.rs`, `app/ui/{state.slint, screens/portfolio.slint}`
  - [x] `HoldingRow` gains `trigger-kind: string` + `dismissed: bool`; a transient `holding_dismissed` set in main.rs (`Rc<RefCell<HashSet<String>>>`, mirrors `HoldingFreshnessMap`) drives `dismissed`. `Holdings` global gains `sell-holding(id, rationale)` + `dismiss-trigger(id)`.
  - [x] `refresh_holdings` (+ `apply_holdings_result`) take the dismissed set; each row's `trigger-kind` is `core::risk::trigger_state(stop_breached, zone == "sell")` mapped to a string. After a sell the register re-renders → CaR recomputes via the Story-4.6 tail; the sold holding is filtered out (sold_at).
  - [x] `portfolio.slint`: the row delegate is now a `VerticalLayout` (the register row + an optional trigger panel); `min-height` grows for the panel. When `trigger-kind != "" && !dismissed`, a neutral block shows the trigger fact (stop / sell-zone) + an optional rationale `TextField` + three `ActionButton`s: **Enregistrer une vente** (`sell-holding`), **Relever le stop** (reuses the Story-4.5 `set-trailing-stop` with the row's `stop-draft`), **Ignorer** (`dismiss-trigger`). Neutral ink, no hue.
  - [x] main.rs callbacks: `on_sell_holding` (calls `state::sell_holding` with the reference currency from `AppConfig`, drops the dismiss entry, neutral `MSG_HOLDING_SOLD` on success / guarded refusal otherwise, re-renders), `on_dismiss_trigger` (inserts into the session set, re-renders). **FR13 fix:** the sell action is labelled **"Enregistrer une vente"** (noun) — the imperative *"Vendre"* is a banned verb (the posture gate caught it). `@tr` floor 276→**282** (+6); `MSG_HOLDING_SOLD` registered, inventory 47→48.

- [x] **Task 5 — Gates (AC4)** — fmt ✓, clippy `-D` ✓, `test --workspace` ✓ (**522** passed, 0 failed), `deny` ✓, smoke launch exit 124. `core::ssg` re-diffs clean (method_fingerprint pinned / determinism / golden green); **`contract` / `Cargo.lock` / `deny.toml` unchanged** (verified via `git diff main` — empty; no new dep, no serde-blob bump); migration is **v3→v4 only** (`user_version` latest = 4); `@tr` floor 276→282, `USER_FACING_MESSAGES` 47→48.

## Dev Notes

### Scope (locked with Guy 2026-06-29)
The **last Epic-4 story**: the neutral sell/stop **trigger** + its inline action panel (sell / raise stop / dismiss), the **stop-priority business rule** (FR47), and a **minimal persisted sell** (one SELL row in the pre-provisioned `transactions` table + a narrow migration for `kind`/`rationale`). **Surface = inline per-row** (consistent with the Story-4.5 set-stop control), not a modal sheet.

### Out of scope (deferred)
- **The full buy/sell transaction ledger** — partial sells, weighted-average cost basis, fees workflow, buy recording, edit/delete of transactions (FR39) → **Epic 6 / Story 6.3**. 4.7 writes a single SELL row and removes the holding; it does **not** read transactions back into the register or recompute cost basis.
- **Replacement-candidate surfacing on a sell + re-concentration flags** (FR48) → **Epic 6 / Story 6.8**.
- **Multi-currency / FX** (Epic 6) — the sell row carries the single reference currency (FR63), no conversion.

### Where things live
- **`core/src/risk/mod.rs`** (additive, beside `ratchet_trailing_stop` / `stop_breached` / `capital_at_risk`): `TriggerKind` + `trigger_state` — the pure FR46/FR47 decision. **Decoupled from `core::ssg`** (the module header already states nothing here is imported by the SSG engine; keep it that way — the fingerprint/golden gates must re-diff clean).
- **`persistence/src/schema.rs` + `migrations.rs`**: the project's **4th migration**. The `transactions` table already exists in the v1 DDL (`id, holding_id, occurred_at, quantity, unit_price, fees, currency, created_at`) — 4.7 only **ALTERs** it to add `kind` + `rationale` (FR39's fields don't include a rationale; it's a 4.7 concept). The harness is already multi-step (v1→v2→v3) and has a `fake_v4` future-step test that must shift to `fake_v5`.
- **`persistence/src/transactions.rs`** (new, mirrors `holdings.rs`): the `record_sell` insert + a `list_transactions` read. Append-only ledger semantics (a sell is an event, not an idempotent upsert) — it bumps `logical_version`.
- **`app/src/state.rs`**: the per-holding `trigger_state` read (over the same zone/stop the register already computes) + `sell_holding` (write SELL row → delete holding, one rail). Pure read for the trigger; one write for the sell.
- **`app/src/main.rs` `refresh_holdings`** + **`app/ui/{state.slint, screens/portfolio.slint}`**: the inline trigger panel + the three actions; the transient `dismissed` session set.

### Notes & guardrails
- **FR47 is the testable nucleus** — keep `trigger_state` a pure boolean function so the stop-priority rule is unit-tested in isolation (the AC's "isolated, testable business rule"). Don't bury the priority in UI conditionals.
- **Never auto-act (FR46/FR13)** — the app surfaces facts + offers actions; only an explicit user click sells or adjusts. No banned imperative verbs in the neutral fact copy (the posture `ui_tr_strings_are_neutral_no_banned_verb` gate enforces this; "Vendre"/"Relever"/"Ignorer" are *button labels for user-chosen actions*, which the existing controls already establish as acceptable — mirror the Story-4.5 set/clear button phrasing).
- **Stop priority is independent of a linked study** — `stop_breached` works on any holding with a stop (Story 4.5); `in_sell_zone` needs a matched study (Story 4.4). A holding with no study can still fire the `Stop` trigger; it simply can't fire `Sell`.
- **`current_price` for the sell** is the matched study's market fact (Story 4.4), a faithful sale price; fall back to `purchase_price` only when there's no linked current price. Fees `0` (the fees workflow is Epic 6).
- **Idempotency / Synology-sync (C4 lesson)** — `record_sell` is an append (intentionally version-bumping); `delete_holding` already guards. The transient `dismissed` set is in-memory only — never a journal write.
- **Capital-at-risk (4.6) recompute** rides the existing `refresh_holdings` tail — after a sell removes the holding, the register re-renders and CaR drops automatically. No new wiring.

### Manual on-display GO/NO-GO (Guy)
Set a tight stop so a holding breaches → its row shows the neutral *"Le prix a atteint votre stop."* + [Vendre] [Relever le stop] [Ignorer]; link a study whose price is in its Sell zone → a holding shows *"Le prix est dans la zone de vente."*; a holding that is **both** breached and in the sell zone shows the **stop** trigger (priority); **Ignorer** hides the panel until restart; **Relever le stop** tightens the stop (ratchet rules from 4.5 still apply); **Vendre** with a typed rationale removes the holding, drops the capital-at-risk figure, and writes a SELL row (verify via a `list_transactions` test / DB inspection); an empty rationale is accepted (stored NULL).

### Project Structure Notes
- Additive `core::risk` (no SSG coupling), one narrow persistence migration (v3→v4, ALTER only), one new `persistence` module mirroring `holdings.rs`, app state + Slint wiring. No `contract` change (SCHEMA_VERSION stays — normalized table, not a blob), no new dependency.
- Posture floors at story start: `@tr` floor **276** (Story 4.6), `USER_FACING_MESSAGES` inventory per `state.rs`. Bump both by exactly the number of new literals/notices (probe empirically in Task 4/5).
- Migration axis at story start: `PRAGMA user_version` latest = **3** (Story 4.5 `trailing_stop_level`). 4.7 makes it **4**. `contract::SCHEMA_VERSION` unchanged.

### References
- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.7] — AC: neutral fact + manual actions (sell/raise stop/dismiss), stop-priority over Sell zone, sell recorded with optional rationale.
- [Source: _bmad-output/planning-artifacts/epics.md] — FR46 (neutral fact + manual actions, never auto-act), FR47 (stop-loss priority over Sell zone, isolated rule), FR39/Story 6.3 (full ledger — deferred), FR48/Story 6.8 (replacement candidates — deferred), UX-DR20 (neutral sell/raise-stop action sheet).
- [Source: core/src/risk/mod.rs] — `stop_breached` (Story 4.5), `capital_at_risk` (Story 4.6), the decoupled-from-`core::ssg` module header.
- [Source: persistence/src/schema.rs#transactions] — the pre-provisioned `transactions` table (v1 DDL); ALTER to add `kind`/`rationale`.
- [Source: persistence/src/migrations.rs] — `REGISTRY` (v1–v3), `fake_v4` future-step test (shift to v5), `latest_version`, `readonly_newer` supported.
- [Source: app/ui/screens/portfolio.slint] — the existing per-row zone marker (Story 4.4) + stop fact/control (Story 4.5) + capital-at-risk header (Story 4.6) the trigger panel sits beside.
- [Source: app/src/main.rs#refresh_holdings] — where `zone` / `stop_breached` / capital-at-risk are already computed; add `trigger-kind` + `dismissed` there. `HoldingFreshnessMap` is the transient-session-state pattern to mirror for `dismissed`.

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — `claude-opus-4-8[1m]`

### Debug Log References

- **FK constraint forced a soft delete (design correction).** `transactions.holding_id` has a `REFERENCES holdings(id)` FK, so recording a sell then hard-deleting the holding violated it (`FOREIGN KEY constraint failed`). To honor BOTH "the sell is recorded with a rationale" (the record must survive) AND "the holding leaves the register", the v4 migration also adds `holdings.sold_at`: `sell_holding` records the SELL row then `mark_sold` (soft delete); `list_holdings` filters `sold_at IS NULL`, so the holding drops out of the register and the capital-at-risk source while its sell transaction keeps a live FK referent. (Deviates from the story's literal `delete_holding`, same user-visible outcome.)
- **FR13 banned-verb gate caught the sell button.** The imperative *"Vendre"* is in `core::method::BANNED_VERBS_FR`; the posture gate rejected it. Relabelled the action **"Enregistrer une vente"** (noun phrase) — neutral, fact-stating, no command.

### Completion Notes List

- **AC1** — `core::risk::{TriggerKind, trigger_state}` decides the trigger purely (stop wins over sell zone, FR47); 4 unit tests incl. the explicit `(true, true) → Stop` priority. Decoupled from `core::ssg` (gates green).
- **AC2** — neutral inline panel per row (no hue, no imperative); the app never auto-acts. The existing 4.4 zone + 4.5 stop facts are untouched; the panel is additive (the row delegate became a `VerticalLayout` with a `min-height`).
- **AC3** — **Enregistrer une vente** writes one SELL row to the pre-provisioned `transactions` table (qty / current-or-cost price / fees 0 / reference currency / optional rationale) via the narrow v3→v4 migration, then soft-deletes the holding; capital-at-risk (4.6) recomputes automatically. Full ledger stays Epic 6 / Story 6.3.
- **AC4** — Relever le stop reuses the 4.5 `set-trailing-stop`; Ignorer is transient session state. 4th migration (`user_version` 4); `contract::SCHEMA_VERSION` unchanged; no new dependency. `@tr` floor 276→282, message inventory 47→48.

### File List

- `core/src/risk/mod.rs` (M) — `TriggerKind` + `trigger_state` + 4 tests
- `persistence/src/schema.rs` (M) — `migrate_to_v4` (transactions kind/rationale + holdings sold_at); v2-test version assert 3→4
- `persistence/src/migrations.rs` (M) — REGISTRY v4; fake-future step v4→v5; forward test for v4
- `persistence/src/holdings.rs` (M) — `list_holdings` filters `sold_at IS NULL`; new `mark_sold`
- `persistence/src/transactions.rs` (A) — `record_sell` / `list_transactions` / `TransactionItem` / `KIND_SELL`
- `persistence/src/lib.rs` (M) — register `transactions` module + re-exports
- `persistence/tests/transactions.rs` (A) — record-sell integration tests
- `persistence/tests/readonly_newer.rs` (M) — `supported` 3→4
- `app/src/state.rs` (M) — `sell_holding` rail + `MSG_HOLDING_SOLD` (inventory) + 2 tests
- `app/src/main.rs` (M) — trigger-kind per row + transient dismissed set + `on_sell_holding` / `on_dismiss_trigger`; threaded the dismissed set through `refresh_holdings` / `apply_holdings_result`
- `app/src/posture.rs` (M) — `@tr` floor 276→282, inventory 47→48
- `app/ui/state.slint` (M) — `HoldingRow.trigger-kind` / `dismissed`; `sell-holding` / `dismiss-trigger` callbacks
- `app/ui/screens/portfolio.slint` (M) — the neutral inline trigger panel + `sell-rationale` draft

### Change Log

- 2026-06-29 — Story 4.7 dev complete (5/5 tasks). Neutral sell/stop triggers + stop-priority (FR46/FR47); narrow v3→v4 migration; soft-delete on sell; workspace 522 tests green.

# Story 6.3 — Buy/sell transaction ledger, partial sells, weighted-average cost basis (FR39)

Status: done

## Story

As Guy,
I want to record **buy and sell transactions** (date, quantity, unit price, fees, currency) against a holding — including **partial sells** — and edit or delete them,
so that a holding's quantity and **weighted-average cost basis (fees included)** are derived from an honest, auditable ledger instead of a hand-maintained pair of numbers.

## Acceptance Criteria

1. **AC1 — Buy rows on the existing table; MIGRATION-FREE (FR39).** A buy is a `transactions` row with `kind = "buy"` — the v1 DDL already carries every FR39 field (`occurred_at`, `quantity`, `unit_price`, `fees`, `currency`) and v4 added `kind`/`rationale`, so **no schema migration** (`user_version` stays 6, `SCHEMA_VERSION` stays 1, `DDL_V1` frozen — the 6.1 precedent). A new `persistence::KIND_BUY = "buy"` sits beside `KIND_SELL`. `rationale` is offered on buys too (FR49 makes rationale first-class on transactions). Decimals stay canonical exact TEXT (NFR-C1); ids/timestamps stay caller-supplied (ADD15); **every applied mutation bumps `journal_meta.logical_version` exactly once** via `util::bump_logical_version` (NFR-R2).

2. **AC2 — Weighted-average cost basis derived by a PURE core function; the holding row is the materialized aggregate.** A new pure, IO-free derivation in `core::risk` (e.g. `core::risk::ledger`) folds a holding's ordered events — an **opening position** `(quantity, avg_cost)` plus its buy/sell rows — into a `PositionBasis { quantity, avg_cost }` per **Appendix A: weighted-average, fees INCLUDED** (a buy of `q` at `p` with fees `f` re-averages `avg_cost = (held_qty × avg_cost + q × p + f) / (held_qty + q)`; a sell reduces `quantity` and **leaves `avg_cost` unchanged**). Checked/exact `Decimal` arithmetic only — **no panic on any input** (overflow/negative/over-sell states return a typed insufficiency, never a wrong number); deterministic (events ordered `occurred_at` then `id`, the existing `list_transactions` order). `holdings.quantity`/`holdings.purchase_price` become the **materialized aggregate** of that derivation (so `list_holdings`, the register, the per-currency capital-at-risk and the 5.3 export are all unchanged readers — `purchase_price` now *means* "weighted-average cost" for a ledger-backed holding; document it). **`core::ssg` / method / golden are untouched** (portfolio math, not SSG method — no `METHOD_VERSION` bump).

3. **AC3 — Atomic compound writes in persistence; calculation stays OUT of persistence.** Persistence gains ledger writers that, in **ONE transaction each**, write the ledger row AND the caller-computed holding aggregate (the `record_sell` pattern — never a committed row with a stale aggregate): `record_buy(...)` (insert buy + update `quantity`/`purchase_price`), a **partial sell** variant of `record_sell` (insert sell + write the reduced quantity; only a sell that empties the position stamps `sold_at` — the 4.7 whole-position path becomes the `remaining == 0` case), `update_transaction(...)` and `delete_transaction(...)` (mutate the row + rewrite the recomputed aggregate; **deleting/editing the sell that retired a holding un-retires it** — `sold_at` cleared/re-stamped to match the recomputed remaining quantity). Persistence performs **no arithmetic** (the app passes the recomputed aggregate; persistence stays calc-agnostic, the 6.2 currency-agnostic parallel); a no-op edit (identical values) writes nothing and bumps nothing (Epic-3 C4). The 2026-07-02 `delete_holding` guard (`Error::HoldingHasTransactions`) stays in force — a ledger-backed holding is never hard-deleted out from under its rows.

4. **AC4 — App rails + Portefeuille UI: ledger view, buy entry, partial sell, edit/delete (FR39, FR13).** The register offers per holding: (a) a **ledger view** listing its transactions (date, kind, quantity, unit price, fees, currency, rationale), oldest first; (b) **record a buy** (date defaulted to today, quantity > 0, unit price ≥ 0, fees ≥ 0 validated like `validate_holding_amounts` — neutral message on bad input, never a panic); (c) the 4.7 **sell action gains a quantity field** (default = full position; `0 < qty ≤ held` enforced; a full-quantity sell behaves exactly like today incl. the trigger flow); (d) **edit/delete** a transaction with the derived aggregate recomputed via the core function and written atomically. The transaction **currency is the holding's effective currency** (`state::effective_currency` — the 6.2 rule; **no mixed-currency ledger rows, no FX** — FX is 6.5); it is stamped, not chosen. All copy neutral (FR13), posture-gated; `@tr` floor (318) and MSG inventory (currently at the 6.2 counts) bumped by the **exact** number of new literals/messages.

5. **AC5 — Legacy holdings and 4.7 sells keep working; export round-trips; no new dependency.** A pre-6.3 holding (no buy rows) is its own opening position: the derivation seeds from the holding's current `(quantity, purchase_price)` **as of before its first 6.3 ledger mutation** — concretely, the first recorded buy/partial-sell/edit **materializes an opening `kind = "buy"` row** (dated `holdings.created_at`, `fees = "0"`, no rationale) in the same transaction, making the ledger self-contained and auditable from then on. Existing 4.7 sell rows (whole-position sells, incl. `kind = NULL` never written in practice — treat a NULL `kind` as a sell defensively, the only pre-6.3 writer) replay correctly. The whole-journal export/import (5.3) already carries `transactions` verbatim — add a round-trip test covering a buy row + a partial-sell row (issue #78's entity-field caveat does not bite: buys reuse existing fields only). **No new external crate** (`Cargo.lock`/`deny.toml` unchanged); `fx_rates` stays inert. All gates green (fmt, clippy `-D`, `test --workspace`, `deny`, smoke exit 124).

## Tasks / Subtasks

- [x] **Task 1 — `core`: pure weighted-average position derivation (AC2)** — `core/src/risk/` (new `ledger.rs` submodule or sibling; re-export from `risk`)
  - [x] `PositionBasis { quantity: Decimal, avg_cost: Decimal }` + `LedgerEvent { kind: Buy|Sell, quantity, unit_price, fees }` (plain value types, no contract/persistence dependency — core stays leaf).
  - [x] `derive_position(opening: Option<PositionBasis>, events: &[LedgerEvent]) -> Result<PositionBasis, LedgerError>` — WAC fees-included on buys, quantity-only reduction on sells, checked `Decimal` ops throughout (the 2026-07-02 saturating/`checked_*` house pattern in this very module), typed errors for over-sell (`quantity` would go negative) and non-positive quantities; division by the summed quantity guarded.
  - [x] Tests: single buy = its own basis (fees folded in); two buys re-average per Appendix A; partial sell keeps avg_cost and reduces quantity; sell-to-zero empties; over-sell → typed error (not a negative position); property test — derivation is deterministic and never panics on arbitrary decimal inputs (proptest, mirroring `verdict`/`ssg` property style).

- [x] **Task 2 — `persistence`: KIND_BUY + atomic compound ledger writers (AC1, AC3)** — `persistence/src/transactions.rs` (+ `holdings.rs` doc)
  - [x] `pub const KIND_BUY: &str = "buy"` (+ re-export in `lib.rs` beside `KIND_SELL`); module header updated (no longer "the recorded-sell slice").
  - [x] `record_buy(id, holding_id, occurred_at, quantity, unit_price, fees, currency, rationale, now, new_quantity: &str, new_avg_cost: &str)` — one tx: INSERT the buy row + `UPDATE holdings SET quantity, purchase_price` + one bump. NOTE `occurred_at` is now caller-supplied (FR39 "date"), distinct from `created_at = now` — `record_sell` hardcodes `occurred_at = now` (`?3` reuse); keep its behavior, don't regress it.
  - [x] `record_partial_sell(...same shape..., remaining_quantity: &str)` — one tx: INSERT the sell row + either `UPDATE holdings SET quantity = remaining` (partial) or the 4.7 `sold_at` stamp (when `remaining == "0"`); one bump. (Either extend `record_sell` or add a sibling — dev's call; the 4.7 call-sites must keep compiling or be updated in the same commit.)
  - [x] `update_transaction(id, occurred_at, quantity, unit_price, fees, rationale, holding_aggregate: (quantity, avg_cost, sold: bool))` and `delete_transaction(id, holding_aggregate)` — one tx each: mutate the row + rewrite the aggregate + set/clear `sold_at` to match; identical-values edit = true no-op (no write, no bump); absent id = no-op success (delete) / typed refusal (update) — follow the `update_holding`/`delete_holding` precedents.
  - [x] Opening-row materialization support: the compound writers accept an optional `opening_buy: Option<(Uuid, occurred_at, quantity, unit_price)>` inserted first in the same tx (AC5) — or a dedicated `seed_opening_buy` executed by the app inside… NO: keep it a parameter so the whole mutation stays ONE transaction.
  - [x] Tests: buy inserts + aggregate lands atomically (fault path: bad FK → nothing applied); partial sell reduces quantity, full sell stamps `sold_at`; delete of the retiring sell clears `sold_at` and restores quantity; each applied mutation bumps exactly once, no-op bumps nothing; `list_transactions` order stable.

- [x] **Task 3 — `app` state rails: orchestrate read → derive (core) → write (persistence) (AC2–AC5)** — `app/src/state/holdings.rs` (or a new `state/ledger.rs`)
  - [x] Read model: `holding_ledger(holding_id) -> Vec<TransactionItem>` (per-holding, `list_transactions`).
  - [x] `record_buy_for(holding_id, occurred_at, qty, price, fees, rationale)`: validate (qty > 0, price ≥ 0, fees ≥ 0 — extend `validate_holding_amounts` or a sibling; date sanity), currency = `effective_currency` (stamped), replay ledger via core → new aggregate → persistence compound write; legacy holding → materialize the opening buy (AC5) in the same call.
  - [x] `sell_holding` grows a quantity argument (default full) and routes through the same replay; **the 6.2 currency rule stays** (holding's own currency, test `sell_holding_stamps_the_holdings_own_currency_not_the_reference` must keep passing).
  - [x] `update_transaction_for`/`delete_transaction_for`: replay the FULL ledger (opening + all rows with the mutation applied) via core; over-sell outcome → neutral refusal message (the edit would make history inconsistent); write atomically; un-retire on delete of the retiring sell.
  - [x] Tests: buy on a legacy holding materializes the opening row once (idempotent — a second buy doesn't re-seed); WAC visible in the register (purchase_price = derived avg_cost); partial sell leaves the holding active with reduced qty; sell-to-zero retires (register + CaR behave like 4.7); delete-the-sell un-retires; capital-at-risk uses the new WAC (existing per-currency tests keep passing); read-only journal refuses all rails neutrally.
  - [x] `UndoHistory`: ledger mutations are journal writes like holdings edits — confirm undo scope (studies-only today) is UNAFFECTED; no new undo surface this story (document).

- [x] **Task 4 — `app` UI: ledger panel + buy form + partial-sell quantity (AC4)** — `app/ui/screens/portfolio.slint`, `app/ui/state.slint`, `app/src/wiring/holdings.rs`
  - [x] Per-holding expandable ledger section (or overlay — match the 4.7 sell-panel pattern): rows (date · kind label « Achat »/« Vente » · qty · unit price · fees · currency · rationale), oldest first; edit/delete affordances per row; neutral empty state.
  - [x] "Enregistrer un achat" form: date (defaulted), quantity, unit price, fees (default 0), optional rationale; currency displayed read-only (the holding's — FR28).
  - [x] The 4.7 sell panel gains a quantity field prefilled with the full position; FR13 noun labels (« Enregistrer une vente » precedent).
  - [x] Push rails in `wiring/holdings.rs` (rows + ledger models rebuilt after each mutation, `refresh_holdings` reused); posture: `@tr` floor 318 + MSG inventory bumped by the exact new counts.

- [x] **Task 5 — Gates + export round-trip (AC5)**
  - [x] 5.3 export/import round-trips a buy + a partial sell (extend `persistence/tests/export.rs` `populated_journal`).
  - [x] Confirm: NO migration (user_version 6), NO core::ssg/method/golden change (fingerprint/goldens byte-identical), NO new dependency (`Cargo.lock`/`deny.toml` unchanged), `fx_rates` inert.
  - [x] fmt + clippy `-D` + `cargo test --workspace` + `cargo deny check` + smoke `timeout 10 cargo run -p steadyinvest-app` (exit 124).

### Review Findings (3-layer, 2026-07-02 — Blind Hunter ×2 / Edge Case Hunter / Acceptance Auditor)

- [x] [Review][Decision→Patch] Ordinary partial sell unreachable outside the 4.7 trigger panel — RESOLVED (Guy 2026-07-02): full ledger-form sell with EXPLICIT date/quantity/price/fees/rationale (`record_sell_for` + « Enregistrer une vente » in the ledger form; the 4.7 trigger panel unchanged)
- [x] [Review][Patch] CRITICAL: deleting the opening buy re-materializes a phantom opening from the ALREADY-DERIVED aggregate → sells double-counted (10@100, sell 4, delete opening → 2@100) [app/src/state/ledger.rs] (edge+blind2+auditor)
- [x] [Review][Patch] HIGH: write rails treat a FAILED ledger read as an empty ledger → double opening materialization on a transient SQLite error [app/src/state/ledger.rs holding_ledger] (edge+blind2)
- [x] [Review][Patch] HIGH: persistence update/delete_transaction never verify the row belongs to holding_id → a mismatched pair rewrites the WRONG holding's aggregate [persistence/src/transactions.rs] (blind1)
- [x] [Review][Patch] HIGH: 4.3 register « Modifier » still rewrites quantity/price/currency directly on a ledger-backed holding → aggregate↔ledger desync ("vide = tout" becomes a partial sell) [app/src/state/holdings.rs update_holding] (edge)
- [x] [Review][Patch] MED bundle: same-day replay order — sells stamped wall-clock vs buys at midnight; edit truncates occurred_at to midnight (breaks C4 no-op + reorders); UUID tiebreak nondeterministic; sell impossible on creation day [app/src/state/ledger.rs] (blind1+edge+blind2+auditor)
- [x] [Review][Patch] MED: date validation accepts Feb 30 / Apr 31 / year 0000 [app/src/state/ledger.rs normalize_event_date] (edge+blind2)
- [x] [Review][Patch] MED: a trigger-panel sell never re-pushes an open ledger panel; dismiss-entry cleared on PARTIAL sells under a now-false comment; stale ledger globals when a holding retires [app/src/wiring/holdings.rs] (edge+auditor)
- [x] [Review][Patch] MED: persistence Ok(false) (row vanished/ownership mismatch) surfaced as a SUCCESS notice [app/src/state/ledger.rs rails] (blind2)
- [x] [Review][Patch] LOW: push_ledger date fallback renders "" and can round-trip an empty date into an edit [app/src/wiring/holdings.rs] (blind1)
- [x] [Review][Patch] LOW: buy form shows no currency on a first buy (task said "displayed read-only") [app/ui/screens/portfolio.slint] (auditor)
- [x] [Review][Patch] LOW doc notes: remaining=="0" canonical-spelling precondition; buy-on-retired is app-gated; read-outside-tx atomicity assumption; undo stays study-scoped (blind1+auditor)
- [x] [Review][Defer] Retired holding's ledger unreachable — un-retire works at state level but needs a sold-positions view — deferred, UI scope beyond 6.3 → GitHub issue (edge+blind2+auditor)
- [x] [Review][Defer] Merge-import can desync the (now derived) aggregate from a surviving local ledger — adjacent to #65's arbitration → noted on #65 (edge)
- [x] [Review][Defer] A future/unknown transaction `kind` freezes that holding's ledger behind a generic notice — #78-adjacent forward-compat → GitHub issue (edge)
- [x] [Review][Defer] One-click irreversible « Supprimer » on ledger rows (no confirm step, unlike 6.1 delete guards) — UX guard → GitHub issue (blind1)
- [x] [Review][Defer] Async re-render wipes in-flight ledger form drafts — the #58 two-way-binding footgun family → noted on #58 (edge)

Dismissed as noise/by-design (5): string-"0" retire compare (canonical precondition documented instead), no-op edit success notice, WAC 28-digit per-step rounding (Appendix-A-faithful), sell fees inert for the basis (by design; P&L is later scope), stale sell-draft-after-partial-sell (model rebuild clears drafts).

**Review resolution (2026-07-02):** all 11 patches applied + the decision implemented; 653+ workspace tests green, clippy 0, fmt clean, smoke exit 124, lock/deny untouched. Key fixes: opening materialization is ALWAYS computed against the current rows (deleting the opening buy with dependent sells → honest OverSell refusal, pinned by test); write rails read strictly (a failed read refuses, never "empty ledger"); persistence `transaction_belongs(id, holding_id)` ownership pre-check on update AND delete (+ ownership in the DELETE itself — the test proved the delete path was genuinely corruptible); replay order `(occurred_at, created_at, buys-first, id)` with date-granular 6.3 events and stamp-preserving same-date edits; real-calendar date validation (leap years, month lengths, year 1900–2200); `sync_ledger_panel` keeps the open panel truthful (partial sell re-pushes, retirement clears); `Ok(false)` surfaces as a refusal, never a success notice; `MSG_LEDGER_BACKED` blocks direct qty/price/currency edits on ledger-backed holdings (ticker stays editable); form currency label; doc notes (canonical "0" precondition, app-gated retired-buy, single-writer atomicity, undo scope). New tests: 5 app (CRITICAL pin, ledger-backed refusal, explicit-price sell, impossible dates incl. leap-day pass, stamp-preserving edit) + 1 persistence (cross-holding ownership on update AND delete). MSG inventory 86; @tr floor 337. Defers → issues (see below), notes on #65 and #58.

## Dev Notes

### Scope
- **In:** buy rows (`KIND_BUY`) on the existing table; partial sells; edit/delete of ledger rows; pure WAC (fees included) derivation in core; holdings `quantity`/`purchase_price` as the materialized aggregate; opening-row materialization for legacy holdings; per-holding ledger UI; rationale on buys (FR49).
- **Out (explicit):** FX acquisition/conversion and any cross-currency ledger row (**6.5**); dividends (**6.4**); per-bank/global capital-at-risk roll-up (**6.6**); replacement candidates on sell (**6.8**); undo/redo of ledger mutations (undo stays study-scoped); multi-lot/FIFO/LIFO cost methods (Appendix A pins **weighted-average**).

### Design decisions (grounded in the tree as of 2026-07-02, post-PR #82)
- **MIGRATION-FREE:** `transactions` (v1 DDL, `schema.rs:168-178`) already has every FR39 field; v4 added `kind`/`rationale`. 6.3 is typed CRUD + a pure core fold — the 6.1 "pre-provisioned DDL" precedent.
- **Layer split (the load-bearing decision):** core = pure arithmetic (`derive_position`, checked ops, typed errors); persistence = atomic compound writes with **caller-supplied** aggregates (no arithmetic in SQL or persistence — NFR-C1 posture); app = orchestration (read ledger → core replay → one persistence call). This keeps persistence calc-agnostic exactly as 6.2 kept it currency-agnostic, and makes the WAC unit-testable without a DB.
- **Materialized aggregate over derive-at-read:** every existing reader (`list_holdings`, register rows, `portfolio_capital_at_risk_by_currency`, 5.3 export snapshot, trailing-stop ratchet) consumes `holdings.quantity`/`purchase_price` today — deriving at read would fan out through all of them. The aggregate is rewritten in the SAME transaction as every ledger mutation, so it can never drift (and NFR-R2 versioning sees exactly one bump). `purchase_price` semantically becomes "weighted-average cost, fees included" for ledger-backed holdings — update the `HoldingItem` doc.
- **Opening-row materialization (AC5):** first 6.3 mutation on a pre-6.3 holding writes an opening `kind="buy"` row (dated `holdings.created_at`, fees 0) in the same tx. Rationale: the replay is then always total (opening = first row), edits/deletes of ANY row recompute honestly, and the ledger is auditable. The alternative (implicit opening carried outside the ledger) breaks as soon as the user edits the opening quantities. **Interpretation to surface in review** (it invents a dated row the user didn't type — but from values they DID type in 4.3).
- **Sells never change avg_cost** (weighted-average): only buys re-average; a sell reduces quantity. Over-sell (edit/delete making cumulative sells exceed cumulative buys+opening) is a **typed core error** surfaced as a neutral refusal — history is never silently negative.
- **`record_sell` (4.7) stays** as the whole-position path (its `occurred_at = now` and trigger-flow call-sites in `state/holdings.rs::sell_holding` keep working); partial sell either extends it with a `remaining_quantity` param or lands as a sibling — dev's call, one commit, no dangling old path. The 2026-07-02 sell-currency rule (holding's own currency via `state::effective_currency`) applies to ALL new writers.
- **NULL `kind` defensive read:** no writer ever produced one (transactions became writable in 4.7 which always sets `kind`), so treat `NULL` as a sell in the replay (the only historical writer) rather than erroring a foreign-but-plausible journal.
- **Issue #81 caveat (open, do NOT widen scope):** the study↔holding auto-match ignores currency; `sell_holding`'s price-from-study behavior is unchanged this story. The buy form takes an explicit unit price (no study coupling), which does not worsen #81.
- **Issue #78 caveat:** buys reuse existing `TransactionItem` fields — no new entity field, so the older-build-import lossiness gap is not widened.

### Architecture decisions this story honours
- **[hybrid model — architecture §"Normalized tables"]** transactions are typed rows, not blobs; decimals TEXT (NFR-C1); no decimal arithmetic in SQL, ever.
- **[core stays leaf & pure]** `core::risk::ledger` depends on nothing but `rust_decimal`; no contract/persistence types cross into core (mirror how `PositionRisk` is a plain value type the app builds).
- **[one-transaction compound writes — the 4.7 lesson]** a ledger row and its holding aggregate are never separable; every writer is single-tx + exactly one `bump_logical_version` (now the shared `util` helper).
- **[Epic-3 C4 idempotency]** identical-value edits are true no-ops (no write, no phantom bump on a sync-sensitive store).
- **[FR13 posture]** all new UI copy is neutral, noun-labeled (« Enregistrer un achat » — the 4.7 « Enregistrer une vente » precedent); posture tests gate `@tr`/MSG inventories (floors: `@tr` 318, MSG at the 6.2 count — bump exactly).
- **[read-IO-semantics-first — Epic-5 E3]** the replay/aggregate/un-retire semantics above are designed HERE, not discovered in tests.

### Where things live (verified paths, post-reorg PR #82)
- `persistence/src/transactions.rs` — `KIND_SELL`, `TransactionItem`, `record_sell`, `list_transactions`/`list_all_transactions` (replay order already `occurred_at, id`).
- `persistence/src/schema.rs:168-178` — frozen `transactions` DDL; `persistence/src/util.rs` — `bump_logical_version`.
- `persistence/src/holdings.rs` — `HoldingItem` (aggregate columns + `sold_at`), `update_holding`, `delete_holding` (`Error::HoldingHasTransactions` guard).
- `core/src/risk/mod.rs` — `PositionRisk`/`capital_at_risk`/`total_invested` (2026-07-02 checked/saturating style to copy); add the `ledger` submodule here.
- `app/src/state/holdings.rs` — `effective_currency`, `sell_holding`, `validate_holding_amounts`, add/update rails; `app/src/state/messages.rs` — MSG consts + inventory.
- `app/src/wiring/holdings.rs` — `refresh_holdings` + holdings/portfolio callbacks; `app/ui/screens/portfolio.slint` + `app/ui/state.slint` — register, sell panel, `Holdings` global.
- `persistence/tests/export.rs` — `populated_journal` to extend for the round-trip.

### Previous story intelligence (6.1 / 6.2 / PR #82)
- **6.2 review HIGH:** the sell stamped the reference currency — fixed to the holding's own via `state::effective_currency`; ALL new ledger writers must use it (test pins it).
- **6.2:** persistence stays semantics-agnostic (currency then, calculation now); the app owns meaning at the read/write boundary.
- **6.1:** import raw-FK leaks are caught as neutral `ImportMalformed`; buy rows ride the same import path (they're `TransactionItem`s — the holding-exists pre-check added 2026-07-02 already covers them).
- **PR #82 reorg:** state lives in `app/src/state/` (thematic modules), callbacks in `app/src/wiring/`; run **package-scoped** cargo during dev; `@tr` floor is **318**; persistence error inventory is **15** (extend the exhaustive match if adding variants); `parse_uuid`/`bump_logical_version` are in `persistence/src/util.rs`.
- Watch the posture gates: new MSG consts must be registered in `USER_FACING_MESSAGES` (messages.rs) and floors bumped by exact counts — both tests fail loudly otherwise.

### Web research
No new external technology: pure `rust_decimal` arithmetic + existing rusqlite/Slint stack. Nothing to version-check; no new dependency permitted by AC5.

### References
- [epics.md#Epic 6] — Story 6.3 outline: "Buy/sell transaction ledger with partial sells + weighted-average cost basis (FR39)".
- [prd.md#FR39] — transactions incl. partial sells (date, quantity, unit price, fees, currency), edit/delete, cost basis per Appendix A.
- [prd.md#Appendix A] — "**Cost basis (FR39):** weighted-average, **fees included**."
- [prd.md#FR49] — rationale first-class on studies AND transactions.
- [architecture.md#§Normalized tables] — hybrid model; `transaction` is a normalized table; decimals TEXT.
- [persistence/src/transactions.rs] — 4.7 slice this story generalizes; [persistence/src/schema.rs:168-178] — frozen DDL.
- [6-2-multi-currency-holdings.md#Completion Notes] — effective-currency rule, review fixes, AC5-deviation precedent.
- Issues: #78 (entity forward-compat caveat), #81 (currency-blind study match — unchanged here), #60 (closed by saturating arithmetic — the core style to follow).

## Dev Agent Record

### Agent Model Used

Claude Fable 5 (story creation, 2026-07-02).

### Debug Log References

### Completion Notes List

- Ultimate context engine analysis completed - comprehensive developer guide created.
- **Task 1 (core):** `core/src/risk/ledger.rs` — `derive_position(opening, events) -> Result<PositionBasis, LedgerError>`, `LedgerEvent{Buy|Sell, quantity, unit_price, fees}`; checked Decimal throughout; typed `LedgerError{NonPositiveQuantity, NegativeAmount, OverSell, Overflow}` (hand-rolled Display — no new dep). 9 tests incl. a proptest (total, deterministic, never-negative). `core::ssg`/method/golden untouched — full core suite green (211).
- **Task 2 (persistence):** `KIND_BUY` + `LedgerEntry<'_>` (re-exported); `record_buy` / `record_partial_sell` / `update_transaction` / `delete_transaction` — each ONE tx + at most one bump, caller-supplied aggregates (no arithmetic in persistence), opening-row materialization as a same-tx parameter, existence pre-checks BEFORE any opening insert, IS-NOT no-op guards, `remaining == "0"` → the 4.7 `sold_at` retire (quantity written too), `retired_at: None` → un-retire. 4.7 `record_sell` untouched. +11 tests (109 total persistence-unit).
- **Task 3 (app rails):** new `app/src/state/ledger.rs` — `holding_ledger`, `record_buy_for`, `update_transaction_for`, `delete_transaction_for`, and `sell_holding` MOVED here with a `quantity_input` param ("" = whole position, the 4.7 flow) returning the matching notice (full → `MSG_HOLDING_SOLD`, partial → `MSG_LEDGER_PARTIAL_SOLD`). Replay = sort candidates `(occurred_at, id)` → `derive_position` (backdated entries re-average honestly); aggregates normalized (`Decimal::normalize`) before storage; opening materialized iff no buy row exists (covers 4.7-sold legacies — deleting their retiring sell un-retires with the restored opening). Date input `AAAA-MM-JJ` ("" = today) stored as midnight UTC. 6 new MSG consts (inventory 79 → 85, posture-gated). +7 state tests (249 app total): materialize-once, WAC in register, partial sell, un-retire on delete, over-sell refusal on edit + legal-edit re-derivation, read-only refusals, bad date/amount refusals; the 6.2 sell-currency test updated (opening row now materialized) and still pins the holding's-own-currency rule.
- **Task 4 (UI):** per-row "Transactions" toggle; ledger section (rows: date · Achat/Vente · qty × price currency · fees · rationale + Modifier/Supprimer) + the buy/edit form (date/qty/price/fees/raison; kind & currency deliberately not editable — currency displayed via the row, pinned FR28); the 4.7 sell panel gains the quantity field ("" = tout). `@tr` floor 318 → 336 (+18, documented in posture.rs). New `LedgerRow` struct + `Holdings` ledger properties/callbacks; wiring in `wiring/holdings.rs` (`push_ledger` + open/close/record-buy/update/delete callbacks, register re-rendered after every mutation).
- **Task 5 (gates):** export round-trip test (opening+buy+partial sell, kinds/fees/rationale/aggregate preserved, holding still ACTIVE after import). NO migration (user_version stays 6), NO SCHEMA_VERSION change, NO new dependency (`Cargo.lock`/`deny.toml` byte-unchanged, `cargo deny` ok), `fx_rates` inert, core fingerprint/goldens byte-identical (pinned tests green). **647 workspace tests, 0 failed; clippy 0 warnings; fmt clean; smoke launch exit 124.**
- **Interpretations for review:** (1) the opening-row materialization writes a dated buy row the user didn't type (from values they DID type in 4.3) — story AC5 default, flagged; (2) a buy on a RETIRED holding is refused (re-entry = a new position via the add rail); (3) sell `unit_price` still comes from the matched study's price (issue #81 unchanged, not widened); (4) full-sell now also writes `quantity = 0` on the holding (the aggregate is truthful; 4.7's row-level behavior preserved via untouched `record_sell` for import fixtures).

### Change Log

- 2026-07-02: Story 6.3 implemented end-to-end (core WAC derivation, persistence atomic ledger writers, app replay rails, Portefeuille ledger UI, export round-trip). 647 workspace tests green; all AC gates verified.

### File List

- `core/src/risk/ledger.rs` (NEW) — pure WAC derivation + tests; `core/src/risk/mod.rs` — module + re-exports.
- `persistence/src/transactions.rs` — KIND_BUY, LedgerEntry, record_buy/record_partial_sell/update_transaction/delete_transaction + rewritten header; `persistence/src/lib.rs` — re-exports; `persistence/src/holdings.rs` — HoldingItem aggregate doc.
- `persistence/tests/transactions.rs` — +11 ledger-writer tests; `persistence/tests/export.rs` — +1 round-trip test.
- `app/src/state/ledger.rs` (NEW) — rails + sell_holding (partial); `app/src/state/holdings.rs` — sell_holding moved out; `app/src/state/mod.rs` — module decl; `app/src/state/messages.rs` — 6 MSG consts + inventory; `app/src/state/tests.rs` — +7 tests, 3 call-site updates.
- `app/src/wiring/holdings.rs` — push_ledger + 5 ledger callbacks + sell quantity threading.
- `app/src/posture.rs` — @tr floor 336, MSG inventory 85 (documented).
- `app/ui/state.slint` — LedgerRow + Holdings ledger surface + sell-holding(3 args); `app/ui/screens/portfolio.slint` — Transactions toggle, ledger section, buy/edit form, sell quantity field.

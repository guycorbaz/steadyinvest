# Story 6.4 — Dividends: gross in the study, net reinvestable per the withholding rule (FR41)

Status: done

## Story

As Guy,
I want to record the **dividends** a holding pays — gross amount and the withholding actually retained —
so that my study keeps its method-faithful **gross** yield while the portfolio shows the **net** cash I can actually reinvest (the Swiss pattern: 35 % impôt anticipé withheld at source).

## Acceptance Criteria

1. **AC1 — A dividend is a ledger row (`kind = "dividend"`); MIGRATION-FREE.** A recorded dividend lands on the existing `transactions` table as `kind = KIND_DIVIDEND` (new const beside `KIND_BUY`/`KIND_SELL`), reusing the frozen columns with pinned semantics: `quantity` = the number of shares it was paid on, `unit_price` = the **gross dividend per share**, `fees` = the **withholding retained at source** (the amount deducted — exactly what the column means on a trade), `currency` = the holding's effective currency (FR28, stamped), `occurred_at` = the payment date (the 6.3 date-granular rule), optional `rationale`. Gross = `quantity × unit_price`; net = gross − `fees`. NO migration (`user_version` stays 6, `SCHEMA_VERSION` stays 1), no new dependency, ids/timestamps caller-supplied (ADD15), one atomic insert + exactly one `logical_version` bump (NFR-R2).

2. **AC2 — The position derivation treats a dividend as a POSITION NO-OP; the study side stays untouched.** `core::risk::ledger` gains `LedgerEventKind::Dividend`: it changes **neither** `quantity` **nor** `avg_cost` in `derive_position` (a cash event, not a position event) — but its amounts are still validated (quantity > 0, price/fees ≥ 0, typed errors, checked arithmetic). A new pure `core::risk::net_dividend_cash(events) -> Result<Decimal, LedgerError>` sums `quantity × unit_price − fees` over dividend events (checked; a withholding larger than the gross is a typed `NegativeAmount`-class refusal, never a negative addition silently folded in). **`core::ssg` is NOT touched**: the §5 return projection already uses the GROSS `present_full_year_dividend` (method fidelity — `core/src/ssg/return_proj.rs:79` documents it); no METHOD_VERSION bump, goldens byte-identical.

3. **AC3 — Reinvestable cash per currency in the Portefeuille panel.** A new app read `portfolio_reinvestable_cash_by_currency(reference_currency) -> Vec<(String, Decimal)>` sums the **net** dividends of ALL the active portfolio's holdings — **including sold ones** (cash received does not evaporate when the position is later closed) — grouped by the row's stamped currency, deterministically ordered, **no cross-currency total** (FX is 6.5; the 6.2 parallel). The Portefeuille screen shows it beside the capital-at-risk block ("Liquidités réinvestissables (dividendes nets)" — one row per currency). v1 scope pin (PRD): reinvestable cash = net dividends ONLY (sell proceeds/cash accounting are NOT in scope); tracking the withheld amount as a recoverable receivable (the CH refund at tax declaration) is ROADMAP, not this story.

4. **AC4 — Record/edit/delete a dividend from the 6.3 ledger form (FR13-neutral).** The ledger form gains « Enregistrer un dividende » beside the buy/sell actions: quantity (prefilled with the CURRENT held quantity — editable, the record-date position may differ), « Dividende par action (brut) » (the unit-price field, relabelled contextually or documented), and the fees field as « Retenue » — **empty = auto-computed** as `gross × withholding_rate` from a new app-config `withholding_rate_pct` (default **35**, the CH impôt anticipé; a Réglages field mirrors the default-trailing-stop pattern: empty = 35, validated 0–100); an explicit value (incl. `0`) overrides. Dividend rows render distinctly in the ledger list (« Dividende », gross and net visible). Edit/delete ride the EXISTING 6.3 rails (same replay, same atomic writers — the aggregate is unchanged by construction; the no-op guards keep an identical edit bump-free). All copy neutral, posture-gated; `@tr` floor (337) and MSG inventory (86) bumped by the exact new counts.

5. **AC5 — The 6.3 invariants keep holding; export round-trips; issue #85's dividend case closes.** `state::ledger::event_of` maps `Some("dividend")` (the 6.3 unknown-kind refusal now only fires for genuinely foreign kinds — note it on #85). Dividend rows do NOT trigger opening-position materialization (they hold no position information; the "no buy row" rule ignores them) and do NOT flip `sold_at` (an empty position with later dividends stays retired). A dividend on a RETIRED holding is allowed (record-date lag is normal — dividends arrive after a sale); the register row is gone, so the entry point is scoped to active holdings' ledgers in v1 (document; the sold-view is #84). The 5.3 whole-journal export round-trips a dividend row (extend the round-trip test). All gates green (fmt, clippy `-D`, `test --workspace`, `deny`, smoke exit 124); `Cargo.lock`/`deny.toml` unchanged; `fx_rates` inert.

## Tasks / Subtasks

- [x] **Task 1 — `core`: Dividend event + net-cash fold (AC2)** — `core/src/risk/ledger.rs`
  - [x] `LedgerEventKind::Dividend`: `derive_position` validates its amounts like every event (qty > 0, price/fees ≥ 0) but leaves the position untouched; doc the cash-vs-position distinction.
  - [x] `pub fn net_dividend_cash(events: &[LedgerEvent]) -> Result<Decimal, LedgerError>` — checked `q × p − fees` fold over Dividend events only; a withholding exceeding the gross is a typed error (`NegativeAmount`), never a silent negative contribution.
  - [x] Tests: dividend leaves `derive_position` output identical; net sum with fees folded; withholding > gross refuses; mixed buy/sell/dividend replay; proptest extended to three kinds (still total, deterministic, never-negative position).

- [x] **Task 2 — `persistence`: KIND_DIVIDEND + `record_dividend` (AC1)** — `persistence/src/transactions.rs` (+ `lib.rs` re-export)
  - [x] `pub const KIND_DIVIDEND: &str = "dividend";` + re-export.
  - [x] `record_dividend(holding_id, entry: &LedgerEntry, now) -> Result<TransactionItem>` — one tx: INSERT the `kind = "dividend"` row + one bump. NO holdings UPDATE (the position is untouched — document why this writer, uniquely, carries no aggregate), NO opening parameter (AC5).
  - [x] Verify (tests): `update_transaction`/`delete_transaction` work on a dividend row unchanged (kind survives the edit — it is not editable); one bump per applied mutation; FK failure applies nothing.

- [x] **Task 3 — `app` state + config: record rail, net-cash read, withholding default (AC3, AC4, AC5)** — `app/src/state/ledger.rs`, `app/src/state/holdings.rs`, `app/src/config.rs`, `app/src/state/messages.rs`
  - [x] `config`: `withholding_rate_pct: Option<String>` (serde default None = 35) + `withholding_rate_pct_or_default()` validated to `[0, 100]` (the `default_trailing_stop_pct` pattern at `app/src/config.rs`); include in the NFR-S1 serialize-guard inventory if one lists fields.
  - [x] `state::ledger::event_of`: map `Some(KIND_DIVIDEND)` → `LedgerEventKind::Dividend` (closes the #85 dividend case).
  - [x] `record_dividend_for(holding_id, date_input, quantity, per_share_gross, withholding_input, rationale, reference_currency, withholding_rate_pct)`: guards (read-only/absent/active), amounts via `validate_ledger_amounts` shape; **empty withholding → `quantity × per_share_gross × rate/100`**, normalized canonical spelling; withholding > gross → neutral refusal; strict ledger read (6.3 review rule) but NO opening materialization; persistence `record_dividend`.
  - [x] `portfolio_reinvestable_cash_by_currency(reference_currency)`: for every holding of the ACTIVE portfolio **including sold** (`list_all_holdings` filtered by `portfolio_id`), read its ledger, fold the dividend rows per stamped currency via `core::risk::net_dividend_cash` (one call per currency bucket — the 6.2 CaR parallel); unparseable rows skipped defensively; deterministic order.
  - [x] New MSG consts (register + inventory + posture count EXACT): dividend-recorded confirmation, invalid-withholding refusal (reuse `MSG_HOLDING_INVALID_NUMBER` where it honestly fits).
  - [x] Tests: record → row lands with gross/net semantics and NO aggregate change, NO opening row, NO version-drift beyond one bump; empty withholding auto-computes at 35 (and at a config-overridden rate); explicit 0 overrides; withholding > gross refuses; net-cash read groups per currency and includes a SOLD holding's dividends; dividend on a retired holding refused at the rail in v1 (scoped entry point) OR allowed — match the AC5 wording (entry scoped to active; the RAIL itself follows the same active-only guard as `record_buy_for`, documented); reinvestable cash never mixes currencies.

- [x] **Task 4 — `app` UI: dividend entry + reinvestable-cash panel (AC4)** — `app/ui/screens/portfolio.slint`, `app/ui/state.slint`, `app/src/wiring/holdings.rs`, settings screen
  - [x] `Holdings.record-dividend(string×6) -> bool` callback (id, date, quantity, per-share gross, withholding ["" = auto], rationale); wiring mirrors `on_record_sell` (notice, `refresh_holdings` — the cash panel re-renders — and `sync_ledger_panel`).
  - [x] Ledger form: « Enregistrer un dividende » (visible while `txn-editing-id == ""`); ledger rows render « Dividende » for the kind with gross×net readable; the existing edit/delete affordances apply.
  - [x] Portefeuille panel: « Liquidités réinvestissables (dividendes nets) » rows per currency (a sibling of the CaR block; same `CapitalAtRiskRow`-style struct or a reused one).
  - [x] Réglages: « Retenue à la source par défaut (%) » field (trailing-stop-default pattern, "" = 35).
  - [x] Posture: `@tr` floor 337 + MSG inventory 86 bumped by the exact new counts.

- [x] **Task 5 — Gates + export round-trip + #85 note (AC5)**
  - [x] Extend `persistence/tests/export.rs` round-trip with a dividend row (kind/fees/currency preserved; the target journal's reinvestable cash reads identically).
  - [x] Confirm: NO migration (user_version 6), NO core::ssg/method/golden change (fingerprint byte-identical), NO new dependency, `fx_rates` inert.
  - [x] fmt + clippy `-D` + `cargo test --workspace` + `cargo deny check` + smoke (exit 124).
  - [x] Comment on issue #85: the `"dividend"` kind is now known; the issue narrows to genuinely FUTURE kinds.

### Review Findings (3-layer, 2026-07-02 — Blind Hunter / Edge Case Hunter / Acceptance Auditor)

- [x] [Review][Patch] HIGH (×3 layers): the edit rail bypassed the withholding ≤ gross invariant AND the panel's per-bucket fold erased a whole currency on one invalid row → edit guard (`MSG_DIVIDEND_WITHHOLDING` on dividend edits) + PER-ROW fold/skip + negative-net hidden in the row display [app/src/state/ledger.rs, app/src/wiring/holdings.rs]
- [x] [Review][Patch] MED (auditor+edge): editing/deleting a dividend on a dividend-only ledger fabricated a phantom opening « Achat » — and the naïve fix would have replayed an empty position and RETIRED the holding → position-row gate: no position rows ⇒ no opening, stored aggregate + sold_at pass through untouched [app/src/state/ledger.rs]
- [x] [Review][Patch] MED (×3): withholding-rate double-validation drift → shared `config::is_valid_withholding_rate_pct` + the accessor returns the TRIMMED spelling; rail trims too [app/src/config.rs, app/src/wiring/prefs.rs]
- [x] [Review][Patch] MED (auditor AC4): quantity prefill + field semantics → empty quantity = whole position (rail default) + an on-form dividend-semantics caption; dividend button no longer requires a typed quantity [app/ui/screens/portfolio.slint, app/src/state/ledger.rs]
- [x] [Review][Patch] MED (auditor): the #85 note was checked but absent → posted (the dividend kind is now first-class; #85 narrows to genuinely foreign kinds)
- [x] [Review][Patch] MED (blind): retired-holding dividend refused with a fake save failure → `MSG_DIVIDEND_RETIRED` factual scope notice (inventory 89)
- [x] [Review][Patch] LOW: auto-withholding rounded to 2 dp (money at the minor unit); dead `ledger_rows_strict` probe removed; net-zero buckets now SHOWN (a fully-withheld dividend is a fact); panel label « Dividendes nets perçus (réinvestissables) » states the cumulative fact; replay rank of dividends documented; export test asserts currency+gross too
- [x] [Review][Defer] Réglages percent-panels swallow invalid input + severed draft binding (shared with the 4.5 trailing-stop panel) → issue #88
- [x] [Review][Defer] Legacy NULL-currency boundary: CaR coalesces LIVE, dividend cash keeps the STAMP — documented asymmetry (cash is a dated fact); revisit with FX (6.5)

Dismissed as noise/by-design (4): dividend quantity unbounded vs position (record-date position may differ — AC4); raw vs locale net formatting in the ledger row (the ledger shows exact canonical spellings, documented); sub-28-digit rounding pedantry (shared with buys, unreachable at realistic magnitudes); notice-ordering worry (refresh_holdings never touches the notice).

**Review resolution (2026-07-02):** all patches applied + 4 pin tests (edit-overwithholding refused; dividend-only mutation touches nothing; dividend-first sell still materializes the opening; one invalid row never erases its bucket) + the retired-refusal test tightened to the new notice. 667 workspace tests, clippy 0, fmt clean, deny ok, smoke exit 124, lock/deny untouched. MSG inventory 89; @tr floor 347.

## Dev Notes

### Scope
- **In:** dividend rows on the existing ledger; pure net-cash fold in core; per-currency reinvestable-cash read + panel; withholding default (config, 35 = CH impôt anticipé) with per-entry override; ledger-form entry + distinct rendering; export round-trip.
- **Out (explicit, PRD-pinned):** withholding-refund/receivable tracking (**ROADMAP** — PRD line 64); sell proceeds or any broader cash accounting; FX conversion of the cash figures (**6.5**) and any cross-currency total; per-jurisdiction rate TABLES (v1 = one configurable default + per-entry override); DRIP/reinvestment execution; `core::ssg` changes of any kind (the study already uses gross).

### Design decisions (grounded in the tree as of 2026-07-02, post-PR #87)
- **Column semantics over migration:** `fees` on a dividend row = the withholding retained at source. It is exactly the column's meaning on a trade (an amount deducted from you at execution), keeps the story MIGRATION-FREE, and leaves the withheld amount **queryable** for the roadmap refund-tracking. Pinned in AC1 and in the `KIND_DIVIDEND` doc — the one interpretation to surface in review.
- **Dividend = cash event, not position event:** `derive_position` ignores it for `(quantity, avg_cost)`; a separate pure fold (`net_dividend_cash`) owns the cash. This keeps the 6.3 WAC math untouched (no golden/fingerprint risk) and the two concerns independently testable.
- **No opening materialization on dividends:** the opening row exists to make POSITION replay total; a dividend carries no position information. Materializing on a dividend would fabricate a buy row for a holding whose ledger may stay dividend-only for months. The 6.3 rule text ("no buy row") already ignores dividend rows — keep it that way and pin with a test.
- **Sold holdings' dividends count:** cash received is cash, whatever later happened to the position — `portfolio_reinvestable_cash_by_currency` reads ALL the active portfolio's holdings (the 5.3 `list_all_holdings` surface), not just the register. The v1 ENTRY point stays on active rows (the ledger panel lives in the register — #84 owns the sold-view); the READ counts everything.
- **Withholding default at the boundary:** the auto-compute (`gross × rate`) happens in the RAIL (app), not in Slint (no decimal math in the view) and not in persistence (calc-agnostic). Empty input = default; explicit `0` = no withholding (e.g. a jurisdiction without one). Rate from config like `default_trailing_stop_pct` (`app/src/config.rs` — copy that validation/serde pattern exactly).
- **6.3 review rules carry over verbatim:** strict ledger reads on every write rail; date-granular `occurred_at` via `normalize_event_date` (real-calendar validation included); currency stamped via `state::effective_currency`; `sync_ledger_panel` after the mutation; `Ok(false)`-style persistence outcomes never surface as success; MSG/`@tr` inventories bumped exactly.

### Architecture decisions this story honours
- **[gross study / net cash — PRD §risks + Appendix A]** §5 uses gross `present_full_year_dividend` (already true — do not touch); reinvestable cash = net (`gross × (1 − rate)` equivalently `gross − withholding`); CH = 35 %.
- **[hybrid model]** dividends are typed rows on the pre-provisioned table; decimals TEXT (NFR-C1); no SQL arithmetic.
- **[core stays leaf & pure / persistence calc-agnostic / app orchestrates]** the 6.3 three-layer split, unchanged.
- **[NFR-R2 + Epic-3 C4]** one bump per applied write; identical edits are true no-ops.
- **[FR13 posture]** « Dividende » is a noun; no banned verbs; posture tests gate every new string.

### Where things live (verified paths, post-PR #87)
- `core/src/risk/ledger.rs` — `LedgerEventKind`, `derive_position`, the tests + proptest to extend.
- `persistence/src/transactions.rs` — `KIND_BUY`/`KIND_SELL`, `LedgerEntry`, `insert_ledger_row`, the compound writers; `lib.rs` re-exports.
- `app/src/state/ledger.rs` — `event_of` (kind mapping), `validate_ledger_amounts`, `normalize_event_date`, `ledger_rows_strict`, the record rails to mirror; `app/src/state/holdings.rs` — `portfolio_capital_at_risk_by_currency` (the grouping pattern to copy).
- `app/src/config.rs` — `default_trailing_stop_pct` + its `_or_none()` validation (the pattern for `withholding_rate_pct`).
- `app/src/wiring/holdings.rs` — `on_record_sell` (the callback to mirror), `refresh_holdings` (add the cash rows push), `sync_ledger_panel`.
- `app/ui/screens/portfolio.slint` — the ledger form + the CaR block; `app/ui/screens/settings.slint` — the trailing-stop-default field pattern; `app/ui/state.slint` — `Holdings` global.
- `persistence/tests/export.rs` — `ledger_buy_and_partial_sell_rows_round_trip_through_export_import` to extend.

### Previous story intelligence (6.3 + its review)
- The 6.3 review found a CRITICAL in opening re-materialization and a corruptible delete path — the fixed rules (opening from CURRENT rows only; `transaction_belongs(id, holding_id)`) are law; dividends must not re-open either hole (no opening on dividends at all).
- `event_of` refuses unknown kinds with `MSG_SAVE_FAILED` — adding `KIND_DIVIDEND` there is REQUIRED before any dividend row exists, or every mutation on that holding freezes (#85).
- Replay order `(occurred_at, created_at, buys-first, id)`: dividends are position-no-ops so their rank among same-tie events is irrelevant to the position — but keep the sell-rank function honest (dividend ranks with buys or its own rank; pick one, document, test determinism).
- Posture floors are STRICT equals/minimums: MSG inventory currently **86**, `@tr` floor **337** — bump by exact counts or the gates fail.
- The story-6.3 File List + Review Findings section shows the full shape of a passing 3-layer review — mirror its rigor.

### Web research
No new external technology (pure Decimal + existing stack); nothing to version-check. No new dependency permitted by AC5.

### References
- [prd.md#FR41] — record dividends; study gross, reinvestable cash net per Appendix A.
- [prd.md#Appendix A] — "Dividend net: `gross × (1 − withholding_rate)`, rate per jurisdiction (CH = 35%); study uses gross."
- [prd.md:64] — v1 reduces reinvestable cash to net; refund tracking = ROADMAP.
- [core/src/ssg/return_proj.rs:79] — the study side already computes on the gross dividend (do not touch).
- [6-3-transaction-ledger-partial-sells-weighted-average-cost-basis.md] — the ledger foundation + review-hardened rules this story extends.
- Issues: #85 (unknown-kind — the dividend case closes here), #84 (sold-view — the dividend entry on retired holdings lands there), #78 (entity forward-compat — dividends reuse existing fields, not widened).

## Dev Agent Record

### Agent Model Used

Claude Fable 5 (story creation, 2026-07-02).

### Debug Log References

### Completion Notes List

- Ultimate context engine analysis completed - comprehensive developer guide created.
- **Task 1 (core):** `LedgerEventKind::Dividend` (validated, position no-op in `derive_position`) + `net_dividend_cash` (checked Σ gross − retenue; withholding > gross = typed `NegativeAmount`); proptest extended to the three kinds + the cash fold's totality/determinism/non-negativity. 12 ledger tests, core 214 green, goldens/fingerprint untouched.
- **Task 2 (persistence):** `KIND_DIVIDEND` + `record_dividend` (one INSERT + one bump; uniquely NO aggregate, NO opening — documented); +2 tests (cash row touches nothing else; generic update/delete work on a dividend row, kind survives). 16 transactions-suite tests.
- **Task 3 (app):** `AppConfig.withholding_rate_pct` (+ `DEFAULT_WITHHOLDING_RATE_PCT = "35"`, `withholding_rate_pct_or_default()` validated [0,100]); `event_of` maps `"dividend"` (the #85 dividend case closes); `record_dividend_for` (empty retenue → gross × rate; explicit 0 overrides; > gross refuses `MSG_DIVIDEND_WITHHOLDING`; strict read; active-only v1 entry, #84 owns the sold view); `portfolio_reinvestable_cash_by_currency` (per stamped currency, SOLD holdings included, one pure core fold per bucket, no mixed total). 2 new MSG (inventory 86 → 88). +5 state tests (259 app total).
- **Task 4 (UI):** « Enregistrer un dividende » in the ledger form (price field = brut par action; fees field = retenue, "" = auto); « Dividende » row noun + « Retenue : » template + the pre-formatted `net` field on `LedgerRow`; « Liquidités réinvestissables (dividendes nets) » per-currency panel under the CaR block; Réglages « Retenue à la source par défaut » (pattern du stop suiveur); wiring mirrors `on_record_sell` (+ `sync_ledger_panel`). `@tr` floor 337 → 346 (+9, documented).
- **Task 5 (gates):** export round-trip extended (buy+sell+dividend; retenue préservée; 4 kinds in order); NO migration (user_version 6), NO core::ssg change, NO new dependency (`Cargo.lock`/`deny.toml` byte-unchanged, `cargo deny` ok), `fx_rates` inert. **663 workspace tests, 0 failed; clippy 0; fmt clean; smoke exit 124.** #85 comment pending at close-out.
- **Interpretation flagged for review:** the `fees` column on a dividend row carries the withholding (an amount deducted at source — the column's trade semantics; keeps the story migration-free and the withheld amount queryable for the roadmap refund tracking).

### File List

- `core/src/risk/ledger.rs` — Dividend kind (position no-op) + `net_dividend_cash` + tests; `core/src/risk/mod.rs` — re-export.
- `persistence/src/transactions.rs` — `KIND_DIVIDEND` + `record_dividend`; `persistence/src/lib.rs` — re-export; `persistence/tests/transactions.rs` — +2 tests; `persistence/tests/export.rs` — round-trip extended.
- `app/src/config.rs` — `withholding_rate_pct` + default const + accessor (+ test initializer).
- `app/src/state/ledger.rs` — `event_of` dividend mapping (incl. the edit-path kind match), `record_dividend_for`, `portfolio_reinvestable_cash_by_currency`; `app/src/state/messages.rs` — 2 MSG + inventory; `app/src/state/tests.rs` — +5 tests.
- `app/src/wiring/holdings.rs` — `net` on `push_ledger`, cash-panel push in `refresh_holdings`, `on_record_dividend`; `app/src/wiring/prefs.rs` — `on_withholding_rate_pct_changed`; `app/src/main.rs` — startup mirror.
- `app/ui/state.slint` — `LedgerRow.net`, `record-dividend`, `reinvestable-cash`, Prefs rate property/callback; `app/ui/screens/portfolio.slint` — dividend button/labels + cash panel; `app/ui/screens/settings.slint` — withholding panel.
- `app/src/posture.rs` — MSG 88, `@tr` 346 (documented).

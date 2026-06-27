# Story 4.3: Single-portfolio holdings register

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to record what I hold in a single portfolio (security, quantity, purchase price) and set my global reference currency,
so that I can see my positions as the foundation for reading risk against my studies — without being told what to do.

## Acceptance Criteria

1. **AC1 — Holdings CRUD persists (FR36).** Given the Portefeuille surface, when I add / edit / remove a holding (security ticker, quantity, purchase price), the change persists to the `holdings` table and the list reflects it after the operation (and across a reopen).
2. **AC2 — Single portfolio (FR36, NOT FR37).** All holdings belong to **one** portfolio. The app ensures a single default portfolio row exists (lazily, on first holding add); the user does **not** create/select/delete portfolios in this story (multi-portfolio = FR37/Epic 6). No portfolio-management UI.
3. **AC3 — Reference currency is configurable in Settings (FR63 currency).** Given the Réglages screen, when I pick a reference currency (e.g. CHF/EUR/USD/GBP), the choice persists in **app-config** (outside the journal) and survives a restart. The holdings register displays its amounts labelled with this currency. Default = `CHF`.
4. **AC4 — Single reference currency, no FX (Guy's 2026-06-27 decision).** Every holding amount is assumed to be in the reference currency; there is **no** per-holding currency field and **no** FX conversion in this story (the `fx_rates` table stays inert; multi-currency + conversion = a later Epic-4/Epic-6 story).
5. **AC5 — Exact-decimal money, validated input (NFR-C1).** Quantity and purchase price are parsed as exact `Decimal` (via `Decimal::from_str_exact`) and stored as **TEXT decimal strings** (never REAL). Invalid input (non-numeric, empty ticker, quantity ≤ 0, negative price) is refused with a **neutral** notice and writes nothing.
6. **AC6 — Neutral, non-advisory, app-scope (FR13).** All holdings/currency copy is fact-stating with no banned verb (no buy/sell/hold/acheter/vendre/conserver/garder…). No zone/refresh/trailing-stop/transaction is computed here (those are 4.4/4.5/Epic 6). The change is **app + persistence only** — no `core`/`contract`/method/golden change; no schema migration (all FR36 columns already exist in the v1 DDL).

## Tasks / Subtasks

- [x] **Task 1 — Persistence: holdings + default-portfolio CRUD (AC1, AC2, AC5)** — new `persistence/src/holdings.rs`, mirror `persistence/src/watchlist.rs`
  - [x] `PortfolioItem { id: Uuid, name: String, created_at: Timestamp }` and `HoldingItem { id: Uuid, portfolio_id: Uuid, security_ticker: String, quantity: String, purchase_price: String, trailing_stop_pct: Option<String>, created_at: Timestamp }` (decimals carried as the canonical TEXT strings, NOT parsed in persistence — persistence is a faithful store)
  - [x] `ensure_portfolio(&mut self, id: Uuid, name: &str, created_at: &Timestamp) -> Result<PortfolioItem>` — idempotent: if a portfolio row already exists, return it and **do not** insert or bump the version (the C4 no-op lesson); else insert the singleton + bump `logical_version` in one tx. (Single-portfolio: callers pass a stable name e.g. "Portefeuille".)
  - [x] `first_portfolio(&self) -> Result<Option<PortfolioItem>>` — read the singleton if present
  - [x] `add_holding(&mut self, id, portfolio_id, security_ticker, quantity, purchase_price, created_at) -> Result<HoldingItem>` — INSERT (trailing_stop_pct NULL — 4.5 owns it) + version bump, one tx
  - [x] `list_holdings(&self, portfolio_id: Uuid) -> Result<Vec<HoldingItem>>` — SELECT … ORDER BY created_at, id (deterministic; no `position` column on holdings)
  - [x] `update_holding(&mut self, id, security_ticker, quantity, purchase_price) -> Result<()>` — UPDATE with the **idempotency guard** (`WHERE id=?1 AND (security_ticker IS NOT ?2 OR quantity IS NOT ?3 OR purchase_price IS NOT ?4)`); bump version **only** if a row changed (mirror watchlist.rs:106–129). Never touches trailing_stop_pct.
  - [x] `delete_holding(&mut self, id) -> Result<()>` — DELETE; bump version **only** if `removed > 0` (absent id = no-op; mirror watchlist.rs:134–150)
  - [x] Re-export `HoldingItem`, `PortfolioItem` from `persistence/src/lib.rs`; `mod holdings;`
  - [x] Tests `persistence/tests/holdings.rs` (mirror `persistence/tests/watchlist.rs`): add→list, ensure_portfolio idempotent (no second row, no second version bump), edit changes value + bumps once, edit-to-same-value is a no-op (no bump), delete removes + bumps, delete-absent is a no-op, FK holding→portfolio holds, decimals round-trip byte-exact as TEXT
- [x] **Task 2 — App-config: reference currency (AC3, AC4)** — `app/src/config.rs`, mirror `preferred_provider`
  - [x] Add `#[serde(default)] pub reference_currency: String` to `AppConfig`; `Default` → `"CHF"`
  - [x] Validation helper: accept a non-empty uppercase ISO-4217-style 3-letter code; on an unknown/garbage value loaded from disk, fall back to the default (don't crash)
  - [x] Tests: extend the round-trip test; add `old_config_without_reference_currency_loads_and_defaults_the_field()` (handwritten JSON without the field → loads, defaults to CHF), mirroring config.rs:241–263; confirm the NFR-S1 no-secret guard still holds (the new field carries no secret)
- [x] **Task 3 — App state: holdings mutators + currency wiring (AC1, AC3, AC5)** — `app/src/state.rs`, mirror the watchlist methods (state.rs:624–722)
  - [x] `JournalState` holdings methods: `list_holdings()`, `add_holding(ticker, quantity_text, price_text)`, `update_holding(id, ticker, quantity_text, price_text)`, `delete_holding(id)` — each guards read-only / no-journal / save-failure → neutral notice; `add_holding`/`first listing` lazily `ensure_portfolio(idgen.new(), "Portefeuille", clock.now())` (app owns IdGen/Clock per ADD15)
  - [x] **Input validation in the app layer** (persistence stays faithful): parse ticker (non-empty, trimmed, upper-cased to match study/watchlist convention), quantity (`Decimal::from_str_exact` > 0), purchase price (`Decimal::from_str_exact` ≥ 0); on failure return a neutral notice constant and write nothing. Store the **canonical** `Decimal::to_string()` as the TEXT value.
  - [x] New `MSG_*` notice constants as needed (e.g. `MSG_HOLDING_INVALID_NUMBER`, `MSG_HOLDING_INVALID_TICKER`); register them in `USER_FACING_MESSAGES`
  - [x] Reference-currency read/write helpers on the existing app-config rail (mirror `mirror_provider_prefs` / `preferred_provider` plumbing)
- [x] **Task 4 — App main: Portefeuille screen wiring + callbacks (AC1, AC2, AC6)** — `app/src/main.rs`, mirror `refresh_watchlist` (139–180) + callback block (656–725)
  - [x] `refresh_holdings(ui, state)` — `list_holdings()` → build `HoldingRow` Vec → set `Holdings.rows` / `holding-count` / `read-only`; call at startup and after each holdings mutation + after study delete is NOT required (holdings don't link studies in 4.3)
  - [x] `apply_holdings_result(ui, state, result)` — set `Holdings.notice`, then `refresh_holdings`
  - [x] Callbacks: `add-holding(ticker, qty, price)`, `edit-holding(id, ticker, qty, price)`, `remove-holding(id)` → state mutator → `apply_holdings_result` (Rc<RefCell> capture pattern, UUID parse on id)
  - [x] Reference-currency callback `reference-currency-selected(code)` on the `Prefs` global → persist app-config → set the displayed currency on `Holdings` (so amounts are labelled) — mirror `provider-selected`
- [x] **Task 5 — Slint UI: Portefeuille screen + currency panel (AC1, AC3, AC6)**
  - [x] `app/ui/state.slint`: add `HoldingRow { id, ticker, quantity, purchase-price, trailing-stop-pct }` struct + a `Holdings` global (`rows: [HoldingRow]`, `holding-count: int`, `read-only: bool`, `notice: string`, `reference-currency: string`, callbacks add/edit/remove-holding); add `reference-currency` + `reference-currency-selected(string)` to the existing `Prefs` global
  - [x] Replace the `app/ui/screens/portfolio.slint` placeholder with the register: header "Portefeuille", an add-holding form (ticker / quantité / prix d'achat TextFields + "Ajouter" ActionButton), a notice banner, an empty state ("Aucune position pour le moment ; ajoutez-en une ci-dessus."), and a row loop (ticker · quantité · prix d'achat labelled with `Holdings.reference-currency` · Modifier/Retirer ActionButtons). Edit can reuse an inline-edit or a simple per-row edit affordance consistent with the watchlist's controls. Neutral ink only — **no zone hues** (no zone is computed here).
  - [x] `app/ui/screens/settings.slint`: add the **reference-currency** ChoiceChips panel (mirror the label-set/theme panels at settings.slint:160–195) with quick picks CHF / EUR / USD / GBP bound to `Prefs.reference-currency` + `Prefs.reference-currency-selected(...)`. (The file header comment at line 2 already anticipates "currency … panels are later".)
  - [x] Glyphs/markers, if any, go **inside** `@tr(...)` (never a bare concatenated literal — the leak gate flags bare prose)
- [x] **Task 6 — Posture floors + gates (AC6)** — `app/src/posture.rs`
  - [x] Bump the `@tr` floor by the exact count of new literals; bump the `USER_FACING_MESSAGES` count by the exact number of new `MSG_*`; update the inventory comment with a Story-4.3 line
  - [x] Run all 4 gates `--locked` (fmt, clippy -D warnings, test --workspace, deny) + the smoke launch (`timeout 8 cargo run -p steadyinvest-app` → exit 124 = healthy); confirm `core`/`contract`/`Cargo.lock`/`deny.toml`/method-fingerprint/golden re-diff empty

## Dev Notes

### Scope decision (single portfolio, CRUD-only — derived from FR36 + Guy's 2026-06-27 currency call)

- **In scope:** holdings CRUD (security ticker, quantity, purchase price) against **one** portfolio + a global **reference-currency** setting (FR36 + FR63 currency).
- **Out of scope (explicitly deferred):** multi-portfolio (FR37 → Epic 6), multi-currency holdings + FX conversion (FR38/FR44 → later; `fx_rates` stays inert), transactions/cost-basis/weighted-average (FR39 → Epic 6), dividends (FR41 → Epic 6), **per-holding price refresh & zone recompute (FR40 → Story 4.4)**, **trailing stop (FR42 → Story 4.5; the `trailing_stop_pct` column exists but stays NULL here)**, capital-at-risk (FR43 → Story 4.6), neutral sell/raise-stop triggers (Story 4.7).
- **Why single-portfolio with a lazily-created default:** FR36 says "a single portfolio"; the `holdings.portfolio_id` FK is NOT NULL, so holdings need a parent row. Rather than expose portfolio management (FR37 territory), the app ensures one default "Portefeuille" row exists on first add. This keeps the FK satisfied without building multi-portfolio UI.

### What this story changes vs. preserves (files marked UPDATE — read before editing)

- **`app/src/config.rs` (UPDATE):** today holds `AppConfig` with the `#[serde(default)]` append-only convention (`preferred_provider` is the precedent at config.rs:79–84, with the legacy-load test at 241–263 and the NFR-S1 no-secret guard at 266–284). **Add** `reference_currency` the same way. **Preserve** the atomic save (temp+rename) and the invalid-file rename-aside behaviour.
- **`app/src/state.rs` (UPDATE):** holds the `JournalState` mutator rail (watchlist methods at 624–722 are the template — read-only/no-journal/save-failure guards, error→`MSG_*` mapping) and `USER_FACING_MESSAGES`. **Add** the holdings mutators + currency helpers + new `MSG_*`. **Preserve** every existing guard and the message inventory exactness.
- **`app/src/main.rs` (UPDATE):** holds `refresh_watchlist` (139–180), the watchlist callback block (656–725), and the `JournalState::open_or_create(…, SystemClock, UuidGen)` injection (404–408). **Add** `refresh_holdings` + holdings callbacks + the currency callback. **Preserve** the IdGen/Clock injection (ADD15 — app owns identity; persistence never calls `Uuid::new_v4()`/wall clock).
- **`app/ui/screens/settings.slint` (UPDATE):** the ChoiceChips panels for theme/label-set/provider live here (160–195, 104–112). **Add** a reference-currency panel mirroring them. **Preserve** the existing `Prefs`-global binding pattern.
- **`app/ui/screens/portfolio.slint` (UPDATE/REPLACE):** currently a neutral placeholder ("Aucune position…", Epic-4 stub). **Replace** with the register. **Preserve** the neutral, advice-free tone.
- **`app/ui/state.slint` (UPDATE):** holds the `WatchRow`/`Watchlist` globals (30–36, 533–545) and the `Prefs` global. **Add** `HoldingRow`/`Holdings` + the `Prefs.reference-currency` members. **Preserve** the existing struct/global field order conventions.
- **`persistence/src/lib.rs` (UPDATE):** re-exports `WatchItem`. **Add** `mod holdings;` + `pub use holdings::{HoldingItem, PortfolioItem};`.

### Architecture & constraints

- **No schema migration.** The `holdings` (schema.rs:88–96) and `portfolios` (81–85) tables already carry every FR36 column (`security_ticker`, `quantity`, `purchase_price`, `trailing_stop_pct`, `created_at`, FK `portfolio_id`). REGISTRY latest stays **v2** (migrations.rs:24–27). Do **not** add a v3 step. There is deliberately **no currency column** on holdings/portfolios — the single global reference currency lives in app-config (FR63), not the journal. [Source: persistence/src/schema.rs#holdings; epics.md#ADD5]
- **Money = exact decimal, TEXT in the store (NFR-C1, ADD5).** Reuse `contract::Money` / `rust_decimal::Decimal` already used across state.rs; parse user input with `Decimal::from_str_exact` (exact — errors instead of silently rounding) and persist `Decimal::to_string()`. Never store REAL. The schema naming/`no-REAL` tests (schema.rs:206–229) guard this for new columns, but holdings columns already exist and are TEXT. [Source: contract/src/money.rs; epics.md#ADD5]
- **App owns Clock/IdGen (ADD15).** The lazy default-portfolio and each holding get their `Uuid` + `created_at` from the injected `UuidGen`/`SystemClock` in main.rs, passed down into persistence — persistence stays deterministic and never sources identity itself. [Source: app/src/main.rs:404]
- **C4 idempotency lesson (Epic-3 retro).** `mutate_*` persists + bumps `logical_version` unconditionally, which on a Synology-synced DB means phantom writes. Every new rail needs a **no-op pre-check**: `ensure_portfolio` must not re-insert/re-bump when the singleton exists; `update_holding` must not bump on an edit-to-same-value; `delete_holding` must not bump on an absent id. Mirror the exact guards in watchlist.rs (116–126, 141, 164). [Source: persistence/src/watchlist.rs]
- **FR13 neutral posture.** Holdings/currency copy must be fact-stating. The posture gate scans `@tr` literals + `USER_FACING_MESSAGES` for banned verbs (FR/EN) and enforces an exact `@tr` floor + an exact message count — bump both to the exact new totals. Put any glyph **inside** `@tr` (a bare `"◆ "`-style literal trips the leak gate). [Source: app/src/posture.rs]
- **App + persistence only.** `core`, `contract`, the method fingerprint, the golden corpus, the serde corpus, `Cargo.lock` and `deny.toml` must re-diff **empty**. No new dependency. [Source: epics.md#Epic 4]

### Previous-story intelligence (4.1 watchlist + 4.2 buy-zone + Epic-3 retro)

- **4.1 is the structural twin.** Holdings CRUD ≈ watchlist CRUD: persistence module + `lib.rs` re-export + `JournalState` guarded mutators + `refresh_*`/`apply_*_result` in main.rs + a Slint screen + a state global. The single biggest *difference*: holdings have a **parent portfolio FK** the watchlist lacked → the `ensure_portfolio` lazy-singleton step is the only genuinely new shape. Holdings also have **no `position`/reorder** (order by `created_at`) — simpler than the watchlist's repack.
- **4.1 gotchas to pre-empt:** (a) a fixed-Uuid test double collides on multi-insert — use the counter-based `SeqIdGen` test double (app/src/clock.rs) for any multi-add test; (b) keep the persistence test scaffold aligned with the REGISTRY (4.1's migration broke 3 tests by version drift — but **4.3 adds no migration**, so this should not recur; still, run the full persistence suite).
- **4.2 leak-gate lesson:** glyphs concatenated outside `@tr` are flagged; keep markers inside the translation literal.
- **Money input is new to the app's non-grid surfaces.** Studies parse decimals inside the grid; holdings parse them in a plain form. Reuse `Decimal::from_str_exact` (state.rs already imports `rust_decimal::Decimal`); do not hand-roll a parser, and reject (not round) bad input.

### Reference-currency design

- Store as `AppConfig.reference_currency: String` (app-config dir, **outside** the journal — app-config vs journal boundary, ADD7). Default `"CHF"`. Surfaced on the `Prefs` Slint global; edited via a ChoiceChips quick-pick (CHF/EUR/USD/GBP) mirroring the label-set picker. A plain validated `String` (not an enum) keeps the set open without a contract type; validate it's a non-empty 3-letter uppercase code and fall back to the default on garbage.
- The holdings register **labels** amounts with this code (e.g. "1 234.50 CHF") but performs **no conversion** — every amount is already in the reference currency (AC4). If the user changes the reference currency, existing holdings' stored numbers are unchanged; only the displayed label changes (a known, accepted simplification for the single-currency story — FX re-labelling/conversion is a later story; note it for the dev so it isn't mistaken for a bug).

### Testing standards

- **Persistence:** `persistence/tests/holdings.rs` integration tests (mirror `tests/watchlist.rs`) on an in-memory/temp journal — CRUD round-trips, the three idempotency no-ops (ensure_portfolio twice, edit-to-same, delete-absent each leave `logical_version` unchanged), FK integrity, byte-exact decimal TEXT round-trip.
- **App-config:** round-trip + legacy-load-defaults + NFR-S1 no-secret (config.rs pattern).
- **App state:** unit tests for the validation rail (good add persists; empty ticker / non-numeric qty / qty ≤ 0 / negative price each refused with the right `MSG_*` and no write) and a reopen-persistence test; use `SeqIdGen` for multi-add.
- **Gates:** all 4 `--locked` + smoke launch 124; app/persistence test counts rise, core/contract counts unchanged.

### Open questions for dev (resolve during implementation, don't block)

- **Edit affordance:** inline-edit per row vs. a small edit form — pick whichever is most consistent with the existing watchlist controls; either satisfies AC1. Keep it neutral and keyboard-reachable.
- **Currency quick-picks:** CHF/EUR/USD/GBP is a reasonable starter set; if a free-text 3-letter entry is trivial to add alongside the chips, fine, but it's not required for AC3.
- **Default-portfolio name:** "Portefeuille" (display) is fine; it's not user-editable in 4.3.

### Project Structure Notes

- New files: `persistence/src/holdings.rs`, `persistence/tests/holdings.rs`. Modified: `persistence/src/lib.rs`, `app/src/{config,state,main,posture}.rs`, `app/ui/state.slint`, `app/ui/screens/{portfolio,settings}.slint`. No `core`/`contract` files. No `Cargo.toml`/`Cargo.lock`/`deny.toml` change.
- Naming follows the persistence conventions enforced by `schema.rs` tests (snake_case tables already exist; new Rust types `HoldingItem`/`PortfolioItem` mirror `WatchItem`/`StudySummary`).

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.3] — AC: holdings CRUD persists + reference currency configurable in Settings
- [Source: _bmad-output/planning-artifacts/epics.md#FR36] — single-portfolio holdings (security/quantity/purchase price), single reference currency
- [Source: _bmad-output/planning-artifacts/epics.md#FR63] — configurable single global reference currency (no blocking setup flow)
- [Source: _bmad-output/planning-artifacts/epics.md#ADD5] — hybrid persistence: normalized holdings/portfolios; money as TEXT decimal, never REAL
- [Source: _bmad-output/planning-artifacts/epics.md#ADD7] — app-config vs journal boundary (currency setting lives in app-config)
- [Source: _bmad-output/planning-artifacts/epics.md#ADD15] — app owns Clock/IdGen; persistence receives fully-formed entities
- [Source: persistence/src/watchlist.rs] — the CRUD + idempotency-guard template to mirror
- [Source: persistence/src/schema.rs:81-97] — pre-provisioned portfolios/holdings DDL (no migration needed)
- [Source: app/src/config.rs:56-85,241-284] — AppConfig append-only `#[serde(default)]` + legacy-load + NFR-S1 guard
- [Source: app/src/state.rs:624-722] — JournalState watchlist mutator rail to mirror
- [Source: app/src/main.rs:139-180,656-725,404-408] — refresh/callbacks/IdGen-Clock injection
- [Source: app/ui/screens/settings.slint:104-195] — ChoiceChips panel pattern for the currency picker
- [Source: contract/src/money.rs] — Money/Decimal exact parse (`from_str_exact`) + TEXT serialization
- Product decision (memory `project_reference_currency`): user-configurable single reference currency, no FX in 4.3.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- Persistence test `add_and_list_returns_holdings_in_creation_order` first failed: two holdings shared one `created_at`, so `ORDER BY created_at, id` sorted by id (the ticker-hash), not insertion order. Fixed in the test harness (`add_at`/`add` advance the second-of-minute by holding count); the production order contract (`created_at, id`) is unchanged and correct.
- `@tr` floor probed empirically (forced the assert high): the real scanned total is **254** (was ≥238 after 4.2); +16 net from the Portefeuille screen + the Réglages currency panel. Floor bumped 238 → 254.

### Completion Notes List

- **Task 1 (persistence):** new `persistence/src/holdings.rs` mirrors `watchlist.rs` — `PortfolioItem`/`HoldingItem`, `ensure_portfolio` (lazy singleton, idempotent — no second insert/bump), `first_portfolio`, `add_holding`, `list_holdings` (ORDER BY created_at, id — no `position`), `update_holding` (idempotency guard), `delete_holding`. All three no-ops (ensure-twice, edit-to-same, delete-absent) leave `logical_version` untouched (C4 lesson). 6 integration tests in `persistence/tests/holdings.rs` green, incl. byte-exact decimal TEXT round-trip and FK.
- **Task 2 (app-config):** `AppConfig.reference_currency: String` (append-only `#[serde(default = "default_reference_currency")]`, default `CHF`) + `is_valid_currency_code` (3 uppercase letters) + `reference_currency_or_default` (validate-on-read). Round-trip + legacy-load-defaults + damaged-value-fallback tests; the NFR-S1 no-secret guard still holds (the field carries no secret).
- **Task 3 (app state):** `JournalState::{list_holdings, add_holding, update_holding, delete_holding}` + `ensure_default_portfolio` (uses injected IdGen/Clock, ADD15) + `validate_holding_amounts` (exact `Decimal::from_str_exact`, qty > 0, price ≥ 0 → canonical TEXT). Two new `MSG_*` (invalid number / empty symbol), registered (inventory 42 → 44). 3 state tests (lazy single portfolio + order; validation refusals write nothing, free purchase OK; edit+delete survive reopen).
- **Task 4 (main wiring):** `refresh_holdings` / `apply_holdings_result`; startup mirrors the validated currency into `Prefs` + `Holdings` and calls `refresh_holdings`; `on_add_holding`/`on_edit_holding`/`on_remove_holding` + `on_reference_currency_selected` (validates, persists app-config, re-labels the register live — no FX).
- **Task 5 (Slint):** `HoldingRow` + `Holdings` global + `Prefs.reference-currency`/`reference-currency-selected` in `state.slint` (re-exported via `app.slint`); `portfolio.slint` replaced the placeholder with the register (add/edit form — edit loads a row into the form, neutral ink, **no zone hue**, amounts labelled with the reference currency); `settings.slint` gained the reference-currency ChoiceChips panel (CHF/EUR/USD/GBP). Glyph-free; all prose inside `@tr`.
- **Task 6 (posture + gates):** `@tr` floor 238 → 254; message inventory 42 → 44. All 4 gates `--locked` green (fmt, clippy -D warnings, `cargo test --workspace` — **app 197**, persistence holdings +6, deny ok) + smoke launch exit 124. **App + persistence only** — `core`/`contract`/`Cargo.lock`/`deny.toml` re-diff empty; no schema migration (FR36 columns pre-existed in v1 DDL); method fingerprint/golden untouched.
- **Residual (Task 7-equivalent, manual on-display GO/NO-GO, Guy):** add a holding (symbole/quantité/prix) → it lists with the reference-currency label; bad numbers/empty symbol → a neutral notice, nothing written; Modifier loads the row into the form, Enregistrer updates it; Retirer removes it; pick a different currency in Réglages → the picker + the amounts' label update and persist across a restart.

### File List

- `persistence/src/holdings.rs` (NEW) — holdings + default-portfolio CRUD
- `persistence/tests/holdings.rs` (NEW) — 6 integration tests
- `persistence/src/lib.rs` (MOD) — `mod holdings;` + re-export `HoldingItem`, `PortfolioItem`
- `app/src/config.rs` (MOD) — `reference_currency` field + validation + tests
- `app/src/state.rs` (MOD) — holdings mutator rail + `validate_holding_amounts` + 2 `MSG_*` + 3 tests
- `app/src/main.rs` (MOD) — `refresh_holdings`/`apply_holdings_result`, holdings + currency callbacks, startup mirror
- `app/src/posture.rs` (MOD) — `@tr` floor 238 → 254, message inventory 42 → 44
- `app/ui/state.slint` (MOD) — `HoldingRow` + `Holdings` global + `Prefs.reference-currency`
- `app/ui/app.slint` (MOD) — import + re-export `Holdings`, `HoldingRow`
- `app/ui/screens/portfolio.slint` (MOD) — placeholder → holdings register
- `app/ui/screens/settings.slint` (MOD) — reference-currency ChoiceChips panel

### Change Log

- 2026-06-27 — Story 4.3 created → ready-for-dev. Scope: single-portfolio holdings CRUD (FR36) + global reference-currency setting (FR63), app+persistence only, NO schema migration (FR36 columns pre-exist in v1 DDL), NO FX (Guy's single-currency decision), NO trailing-stop/refresh/zones/transactions (deferred to 4.4/4.5/Epic 6). Twin of Story 4.1 (watchlist) + the `ensure_portfolio` lazy-singleton as the one new shape.
- 2026-06-27 — Story 4.3 implemented (all 6 tasks) → review. New `persistence/src/holdings.rs` (CRUD + lazy single portfolio, idempotency no-ops) + `AppConfig.reference_currency` (append-only, validate-on-read) + `JournalState` holdings rail with exact-decimal validation + main.rs wiring + the Portefeuille register screen + the Réglages reference-currency panel. App+persistence only; no core/contract/schema/method/golden change; no new dep; `@tr` floor 238 → 254, message inventory 42 → 44; app 192 → 197 tests + persistence holdings +6; all 4 gates green; launch 124. Manual on-display GO/NO-GO pending Guy.
- 2026-06-27 — Code review (3-layer: Blind Hunter + Acceptance Auditor + Edge-Case): both layers **ACCEPT**, 6/6 ACs PASS, no CRITICAL/HIGH. Triage: (a) **PATCH M1** — the add/edit form cleared unconditionally, wiping a user's typed input on a refused write; `add-holding`/`edit-holding` now return `bool` (written?) and the Slint form resets only on success (the established `create-study` pattern). (b) **PATCH L4** — `ensure_default_portfolio` minted an `IdGen` id on every add; now it reads `first_portfolio` first and mints only when the portfolio is absent (keeps the deterministic id sequence stable, common path is a pure read). (c) Cross-process two-portfolio TOCTOU (M2) dismissed — mitigated by the existing single-instance file lock (ADD6); a real guard needs a UNIQUE constraint = a migration, out of 4.3 scope. (d) Sub-second `created_at` ties order holdings by UUID (L5) — accepted: holdings carry no user-ordering semantics and the order stays deterministic/stable. (e) No free-text currency entry beyond the four quick-picks filed as **issue #49**. (f) `-0` price over-rejection (L7) dismissed as harmless. Re-ran all 4 gates after the patches: fmt clean, clippy 0, app 197 tests + holdings 6, deny ok, launch 124. Status → done.

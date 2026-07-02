# Story 6.5 — FX acquisition: dated, source-aware rates, applied only at consolidation (FR28)

Status: done

## Story

As Guy,
I want the app to **acquire and retain dated, source-aware exchange rates** (fetched on demand from my data provider, or typed in by hand) for the currencies my holdings actually use,
so that the consolidation views (Story 6.6: capital-at-risk per currency → per bank → global) can convert honestly — with a rate whose date and source I can always see — while every study and per-currency figure stays strictly in its native currency.

## Acceptance Criteria

1. **AC1 — `fx_rates` becomes load-bearing: typed CRUD on the pre-provisioned table; MIGRATION-FREE.** The v1 DDL (`persistence/src/schema.rs:181-189`: `id, base_currency, quote_currency, rate, rate_date, source, created_at`) gains typed persistence access — `upsert_fx_rate` (INSERT-OR-REPLACE keyed by `(base, quote, rate_date, source)`-uniqueness via id convention OR by (base,quote,rate_date,source) natural lookup — dev's call, but a same-day re-fetch must UPDATE, not duplicate), `list_fx_rates()` (deterministic order: pair, then `rate_date` DESC), and `latest_fx_rate(base, quote, on_or_before: Option<&str>)` (the most recent dated rate ≤ the asked date; `None` when absent — **never** an inverted-pair guess). `rate` stays exact TEXT (NFR-C1); ids/timestamps caller-supplied (ADD15); every applied mutation bumps `logical_version` exactly once (NFR-R2 — `fx_rates` IS an exported table axis: see AC5); a no-op re-upsert (identical values) bumps nothing (Epic-3 C4). NO migration (`user_version` stays 6), NO new dependency.

2. **AC2 — The pure conversion primitive lives in core; conversion NEVER happens implicitly.** A new pure, IO-free `core::risk::fx` (or sibling) module: `convert(amount, rate) -> Option<Decimal>` (checked multiply) and a `FxRate { rate: Decimal, rate_date, source }`-shaped value the app builds from persistence rows. The primitive converts ONE amount with ONE dated rate the CALLER chose — the "FX only at consolidation" rule is structural: nothing in the 6.2/6.4 per-currency reads (capital-at-risk buckets, reinvestable cash) calls it, and this story surfaces **no converted figure anywhere** (that is Story 6.6, which plugs the hierarchy into this primitive). `core::ssg`/method/goldens untouched (no METHOD_VERSION bump); checked arithmetic, no panic on any input.

3. **AC3 — Provider acquisition, user-initiated (FR65: never background-polled), pair-symbol per adapter.** `MarketDataProvider` gains an **additive** `fetch_fx_rate(base, quote, api_key) -> Result<Option<Decimal>, ProviderError>`: Twelve Data via `/price` with the pair symbol `"{base}/{quote}"` (its native FX form); EODHD via its EOD close for `"{base}{quote}.FOREX"`. The existing NFR-S1 discipline holds verbatim (key never in errors/URLs-in-errors; bodies capped; `dec()` exact — reuse `adapters/common.rs`). A « Actualiser les taux » action fetches, for the **reference currency as quote**, one rate per FOREIGN currency actually present among the active journal's holdings' effective currencies (no speculative pairs), stamps `rate_date` with the fetch **day** (the provider's true session date is #72's known limitation — same interim rule as the price cache) and `source` with the provider wire name (`"eodhd"`/`"twelvedata"`), and upserts. Provider failures surface the existing neutral cause-named notices (offline/quota/no-data), last-known rates stay.

4. **AC4 — Manual entry + the rates panel (Réglages), FR13-neutral.** A « Taux de change » Réglages panel lists the stored rates (pair, rate, date, source — most recent first per pair) and offers: the provider refresh action (AC3) and a **manual entry** form (base from the supported set, quote = the reference currency display, rate > 0 exact decimal, date AAAA-MM-JJ with the 6.3 real-calendar validation, `source = "manuel"`). Validation refuses neutrally (bad rate/date/pair; base == quote refused); a same-`(pair, date, source)` re-entry updates in place. All copy neutral, posture-gated; `@tr` floor (347) and MSG inventory (89) bumped by the exact new counts. Decisions pinned 2026-07-02 (Guy absent — recommended defaults, revisable): **provider + manual entry** (not manual-only, not provider-only), and **no visible conversion in 6.5** (primitive + panel only; 6.6 consumes).

5. **AC5 — Export round-trip + the version axis; gates.** `fx_rates` rows join the whole-journal export/import (5.3): a new `fx_rates: Vec<FxRateItem>` array on `JournalSnapshot` — **NOTE the #78 rail**: `JournalSnapshot` carries `deny_unknown_fields`, so an OLD build refuses a NEW file that has the array (a typed rejection, the designed behavior — document in the story that this is the first real exercise of that rail; the envelope `schema_version` does NOT bump: the snapshot tolerates a MISSING array via `#[serde(default)]`, so old files import fine into new builds). Import upserts by id in the same single transaction (NFR-R5). Export determinism holds (Vec, fixed order). All gates green (fmt, clippy `-D`, `test --workspace`, `deny`, smoke exit 124); `Cargo.lock`/`deny.toml` unchanged; NO change to `core::ssg`.

## Tasks / Subtasks

- [x] **Task 1 — `persistence`: fx_rates typed CRUD (AC1)** — new `persistence/src/fx.rs` (+ `lib.rs` mod/re-export)
  - [x] `FxRateItem { id, base_currency, quote_currency, rate, rate_date, source, created_at }` (serde derives for the 5.3 export; the 6.x doc style).
  - [x] `upsert_fx_rate(item)` — one tx; keyed by the natural `(base, quote, rate_date, source)` (SELECT the existing id first; UPDATE in place or INSERT with the caller-minted id); identical-values re-upsert = true no-op (no bump); else exactly one bump.
  - [x] `list_fx_rates()` deterministic (pair ASC, rate_date DESC, source); `latest_fx_rate(base, quote, on_or_before)` — most recent ≤ the date (or absolute latest when `None`); returns the full item (the caller shows date+source).
  - [x] Tests: upsert/no-op/bump counts; latest-≤-date picks correctly across dates and ignores the inverted pair; empty → None.

- [x] **Task 2 — `core`: the conversion primitive (AC2)** — `core/src/risk/fx.rs` (re-export from `risk`)
  - [x] `pub fn convert(amount: Decimal, rate: Decimal) -> Option<Decimal>` (checked mul; `None` on overflow — the 6.3 checked posture) + doc: the caller picks the dated rate; nothing here (or anywhere in 6.5) converts implicitly.
  - [x] Tests: exactness, overflow → None, negative amounts pass through (a signed consolidation delta is legal — document).

- [x] **Task 3 — `ingestion`: additive `fetch_fx_rate` (AC3)** — `ingestion/src/provider.rs`, both adapters, `adapters/common.rs`
  - [x] Trait method `fetch_fx_rate(&self, base: &str, quote: &str, api_key: Option<&str>)`; Twelve Data `/price?symbol={base}/{quote}`; EODHD latest EOD close for `{base}{quote}.FOREX`. Reuse `common::get_json`/`dec`; NFR-S1 verbatim; body-classify like the price paths.
  - [x] Unit tests with the existing fixture style (classification, symbol building, exact decimal); NO live network in CI.
  - [x] `app::fetch` worker: a `WorkerJob::FetchFxRates { pairs }`-style job (one job, N pairs sequentially — batching/fallback is 6.9) + outcome carrying per-pair results; route per selected provider like the price jobs.

- [x] **Task 4 — `app` state + Réglages UI (AC3, AC4)** — `app/src/state/` (new `fx.rs` or in holdings.rs), `app/src/wiring/` (prefs or a new fx wiring), `app/ui/screens/settings.slint`, `app/ui/state.slint`
  - [x] State rails: `list_fx_rates` read for the panel; `upsert_manual_fx_rate(base, rate_input, date_input, reference_currency)` (validate: supported base ≠ reference, rate > 0 exact, real-calendar date, source `"manuel"`); `foreign_currencies_in_use()` (effective currencies of ALL the active journal's holdings minus the reference — the AC3 fetch set); `apply_fx_fetch(base, quote, rate, now)` (source = provider wire, rate_date = now's day).
  - [x] Wiring: « Actualiser les taux » (disabled while in flight, the #52 double-click rule; provider failures → the existing neutral notices) + manual form + the rates list push (pair "EUR → CHF", rate, date, source).
  - [x] New MSG consts (recorded/refused) registered + inventories bumped exactly (`@tr` 347, MSG 89 → exact new counts).
  - [x] Tests: manual upsert validation paths; foreign-currency set (incl. sold holdings' currencies? — NO: rates serve consolidation of CURRENT figures; pin: the set = ACTIVE holdings' effective currencies, documented); fetch-apply stamps day+source; read-only refusals.

- [x] **Task 5 — 5.3 export round-trip + gates (AC5)**
  - [x] `JournalSnapshot.fx_rates: Vec<FxRateItem>` with `#[serde(default)]` (an old file without the array imports fine); export reads `list_fx_rates`; import upserts by id in the SAME transaction; summary gains a count. Round-trip test (incl. an old-file-without-array import).
  - [x] Confirm: NO migration (user_version 6), NO core::ssg/golden change, NO new dependency; `fx_rates` writes bump the version (it is now an exported axis — the price_history exception does NOT extend to it; note the contrast in `util::bump_logical_version`'s doc).
  - [x] fmt + clippy `-D` + `test --workspace` + `cargo deny check` + smoke exit 124; note on #72 (FX rate_date shares the session-date limitation).

### Review Findings (3-layer, 2026-07-02 — Blind Hunter / Edge Case Hunter / Acceptance Auditor)

- [x] [Review][Patch] HIGH (blind+edge): the 5.3 import upserted fx rows BY ID while the live writer keys on the natural key → a merge minted natural-key duplicates the writer could never repair and the arbitration then mis-picked → the import now reconciles by the NATURAL key (same-tx), with shape validation (positive decimal text, 10-char ISO date, base ≠ quote → neutral ImportMalformed) [persistence/src/export.rs]
- [x] [Review][Patch] HIGH (blind+edge): provenance was stamped from CONFIG at outcome time (an in-flight provider switch falsified the source — even "none") and the outcome applied to WHATEVER journal was open → `FxRatesRequest` now carries `journal_id` + `source` captured at enqueue; the handler drops mismatched-journal outcomes with `MSG_FX_JOURNAL_CHANGED` [app/src/fetch.rs, wiring/fetch.rs, wiring/fx.rs]
- [x] [Review][Patch] HIGH (blind+edge): the in-place upsert kept the original `created_at`, so a later CORRECTION never won the same-day tie ("the later write wins" was false) → the update refreshes `created_at` to the correcting write; doc + pinned tests updated [persistence/src/fx.rs]
- [x] [Review][Patch] HIGH (edge) / MED (blind): `refreshing` latched true forever on a dead-worker send → the flag sets only on a successful send; the FX panel now re-renders (and resets flag+notice) on journal switch, and re-pushes on import/restore [app/src/wiring/fx.rs, wiring/journal.rs]
- [x] [Review][Patch] MED (×3): swallowed failure causes → the count notice ALWAYS carries the first failure cause when one exists (provider errors AND app-side apply refusals) [wiring/fetch.rs]
- [x] [Review][Patch] MED (edge): an imported off-list/lowercase holding currency poisoned the fetch pair set → uppercase + allow-list filter in `foreign_currencies_in_use` (+ pin test) [app/src/state/fx.rs]
- [x] [Review][Patch] MED (blind+edge+auditor): wrong refusal copy for an off-list base (named the rate) → `MSG_FX_INVALID_CURRENCY`; future-dated manual rates (which would win "latest" for years) → `MSG_FX_FUTURE_DATE`; inventory 95 → 98 [messages.rs, state/fx.rs, posture.rs]
- [x] [Review][Patch] MED (blind): every export carried an empty `"fx_rates":[]` (pre-6.5 builds rejected even no-FX exports) → `skip_serializing_if = "Vec::is_empty"` (the #78 rail: old files import fine; an empty store round-trips readable by old builds; a file WITH rates is the typed rejection) [export.rs + test updated]
- [x] [Review][Decision→re-pin] AC3 pair set (auditor): the Task-4 pin said ACTIVE-only holdings; the implementation (and its test) includes SOLD holdings — RE-PINNED to sold-included with rationale: the 6.4 reinvestable cash of sold holdings is native-currency journal data 6.6 must consolidate, so its pairs belong in the fetch set. The story text stands corrected here.
- [x] [Review][Doc] AC5's "upserts by id" superseded by the natural-key reconciliation (this section is the authoritative record); `rate_date` doc tightened (AAAA-MM-JJ, app-normalized — the import validates the shape); the #88 `text <=> draft` pattern IS used in the FX panel (accepted + noted, per the story's own option).
- [x] [Review][Defer] Rates-panel management: no delete/undo for a mistaken row; reference-currency switch strands old-quote rows unmarked; fetch-day stamping accumulates weekend phantom rows (#72 class) → issue
- [x] [Review][Defer] `FxRateItem` (like every entity item) tolerates unknown fields — the #78 per-entity question, noted there

Dismissed (3): UUID id tiebreak on a full (date, created_at) tie — deterministic per store, the documented arbitration keys are date then write-recency; `list_all_holdings().unwrap_or_default()` in the pair-set read (display-adjacent, provider refusals now carry causes); notice shared between the two acquisition paths (single panel, last action wins — consistent with every other panel notice).

**Review resolution (2026-07-02):** all patches applied; +3 pin tests (corrected-rate wins the tie; off-list currency excluded from the pair set; future-date refused) + the natural-key created_at test updated. 690 workspace tests, clippy 0, fmt clean, deny ok, smoke exit 124, lock/deny untouched. MSG inventory 98; @tr floor 356. Test-count correction (auditor): +8 persistence tests in the original dev (7 fx + 1 export), not +7.

## Dev Notes

### Scope
- **In:** typed `fx_rates` CRUD; the pure conversion primitive; additive provider `fetch_fx_rate` (Twelve Data pair symbol, EODHD .FOREX); user-initiated refresh for the holdings' foreign currencies; manual entry; the Réglages rates panel; 5.3 export round-trip.
- **Out (explicit):** ANY visible converted figure (the global CaR total, per-bank roll-up = **6.6**); provider fallback chains/batching (**6.9**); historical rate series/charting; automatic/background refresh (FR65 forbids); rate interpolation; inverse-pair derivation (1/rate — refuse honestly instead, 6.6 may revisit).

### Design decisions (2026-07-02 — Guy absent at the scope question; RECOMMENDED defaults, flagged for review)
- **Provider + manual entry** (not manual-only): `fetch_latest_price` proves both adapters serve simple quotes; FX pairs are the same wire shape (TD `/price` with `EUR/CHF`; EODHD EOD `EURCHF.FOREX`). Manual entry keeps the free/offline path honest (source `"manuel"`).
- **No visible conversion in 6.5**: the story ends at the primitive + the panel. 6.6 owns every converted figure. This keeps "FX only at consolidation" structurally auditable: grep `risk::fx::convert` callers — in 6.5 there are none outside tests.
- **Natural-key upsert** `(base, quote, rate_date, source)`: a same-day provider re-fetch updates in place (no duplicate rows, no phantom history); distinct sources on the same day coexist (manual overrides visible next to provider rows — the caller picks by recency, `latest_fx_rate` orders by `rate_date` then `created_at`).
- **Reference currency as the fixed quote**: rates are stored as `BASE→reference` for the currencies in use. If the reference changes, missing pairs simply show as absent (refused honestly by 6.6, refreshable in one click) — no silent 1/rate inversion.
- **The #78 rail, exercised deliberately (AC5)**: adding `fx_rates` to the snapshot with `#[serde(default)]` is the forward-compat pattern #78 asked to pin — old→new imports fine; new→old is a typed refusal (deny_unknown_fields). Note it on #78 at close-out.

### Architecture decisions this story honours
- **[FR28 / PRD lines 41, 57, 63]** rates dated + source-aware, refreshed manually; native-currency figures never mixed; FX only at consolidation points.
- **[hybrid model]** typed rows on the pre-provisioned table; TEXT decimals; no SQL arithmetic.
- **[three-layer split]** core = pure convert; persistence = calc-agnostic CRUD; app orchestrates; ingestion = the only network layer (NFR-S1 verbatim).
- **[FR65]** refresh is user-initiated only; in-flight guard (#52 precedent).
- **[NFR-R2 / C4]** every applied fx write bumps once; identical re-upserts bump nothing.

### Where things live (verified paths, post-PR #89)
- `persistence/src/schema.rs:181-189` — the frozen `fx_rates` DDL; `persistence/src/holdings.rs`/`transactions.rs` — the CRUD/doc house style; `persistence/src/util.rs` — `bump_logical_version` (+ its "every exported table bumps" doc to extend).
- `persistence/src/export.rs` — `JournalSnapshot` (add the array; `deny_unknown_fields` sits on the SNAPSHOT, `#[serde(default)]` per field is the additive rail), `import_journal` (same-tx upsert block pattern), `ImportSummary`.
- `core/src/risk/` — the module/dir to extend (`ledger.rs` shows the checked-arith + test style).
- `ingestion/src/provider.rs` — the trait (additive method); `ingestion/src/adapters/{eodhd,twelvedata}.rs` + `common.rs` — URL building/classification/dec to reuse; `app/src/fetch.rs` — WorkerJob/WorkerOutcome enums + the per-provider routing (7-4 pattern).
- `app/src/state/holdings.rs` — `effective_currency` (the foreign-currency set); `app/src/state/ledger.rs` — `normalize_event_date` (REUSE for the manual date; it is `pub(crate)`-able); `app/src/wiring/prefs.rs` + `app/ui/screens/settings.slint` — the panel pattern (withholding panel just landed); `app/src/posture.rs` — floors (`@tr` 347, MSG 89).

### Previous story intelligence (6.3/6.4 + reviews)
- Write rails read STRICT (an IO error is a refusal, not an empty list); every mutation = one tx + ≤1 bump; identical values = true no-op; real-calendar date validation exists (`normalize_event_date`) — reuse, don't fork.
- The Réglages percent-panel draft-binding footgun is #88 — the new rates panel should avoid `property <string> draft: Prefs.x;` + `text <=> draft` for values the handler rewrites (prefer one-way + explicit set, or accept and note #88).
- Posture floors are exact-count disciplines; register every MSG const; count `@tr` additions precisely (probe trick: temporarily set the floor high and read the failure count).
- The 6.4 review's biggest lesson: an invariant enforced on RECORD must hold on EDIT/IMPORT too, and a display fold must fail PER ROW. `latest_fx_rate`/the panel read should tolerate a corrupt row without blanking the panel.
- #72: the price cache stamps the refresh day, not the provider session date — the FX `rate_date` shares this interim rule; note it there.

### Web research
No new dependency: both adapters' existing HTTP surface covers FX symbols (TD `/price` accepts `EUR/CHF`; EODHD serves `{PAIR}.FOREX` on `/eod`). Nothing to version-check.

### References
- [prd.md#FR28] + PRD lines 41/57/63 — dated/source-aware, manual refresh, FX only at consolidation (hierarchy = 6.6).
- [architecture.md:371] — "fx_rate rows are dated & source-aware; FX applied only at the consolidation layer".
- [persistence/src/schema.rs:181-189] — the pre-provisioned table.
- [6-4-dividends-gross-study-net-reinvestable.md#Review Findings] — the record-vs-edit invariant lesson; the per-row fold lesson.
- Issues: #78 (the additive-array rail this story exercises — note at close-out), #72 (session-date limitation shared), #88 (Réglages draft binding — avoid in the new panel), #70 (provider symbol conventions — FX pair spellings are per-adapter, same class).

## Dev Agent Record

### Agent Model Used

Claude Fable 5 (story creation, 2026-07-02).

### Debug Log References

### Completion Notes List

- Ultimate context engine analysis completed - comprehensive developer guide created.
- **Task 1 (persistence):** NEW `fx.rs` — `FxRateItem` + `upsert_fx_rate` (natural key `(base,quote,rate_date,source)`, in-place update keeps id/created_at, identical = no-op no-bump), `list_fx_rates` (deterministic), `latest_fx_rate` (greatest date ≤ bound, created_at DESC tiebreak, inverted pair never consulted). +7 tests (persistence 120→).
- **Task 2 (core):** NEW `risk/fx.rs` — `convert(amount, rate)` checked mul; the FR28 rule structural (zero non-test callers in 6.5). 3 tests; core 217.
- **Task 3 (ingestion + worker):** additive `MarketDataProvider::fetch_fx_rate` — both adapters delegate to `fetch_latest_price` with their pair spelling (TD `EUR/CHF` via /price; EODHD `EURCHF.FOREX` via /eod — zero duplicated HTTP; #70 symbol-convention class noted); `Provider` dispatch + `fetch_fx_rate` wrapper + `FakeProvider.with_fx_rate`. +5 ingestion tests (29). App worker: `WorkerJob::FetchFxRates` (one job, N pairs sequential — batching = 6.9) + per-pair `FxRateOutcome` (one failed pair never hides the others).
- **Task 4 (app):** NEW `state/fx.rs` (list/foreign_currencies_in_use incl. SOLD holdings/upsert_manual [source "manuel"]/apply_fx_fetch [fetch-day stamp #72, provider wire source]); `normalize_event_date` reused (pub(super)); NEW `wiring/fx.rs` (refresh with the #52 in-flight guard + no-provider/no-key/no-pairs refusals; manual form; `push_fx_rates`); outcome arm in `wiring/fetch.rs` (per-pair count, `{n}/{t} taux actualisés`, first failure cause when nothing landed); `Fx` global + `FxRateRow` + the Réglages « Taux de change » panel. 6 MSG (inventory 89→95); `@tr` 347→356 (+9). +5 state tests (268 app).
- **Task 5 (export):** `JournalSnapshot.fx_rates` with `#[serde(default)]` — the #78 additive rail exercised deliberately (old file w/o array imports fine — TESTED; new→old = typed deny_unknown_fields rejection); same-tx id-upsert; `ImportSummary.fx_rates`; `util::bump_logical_version` doc contrasts fx_rates (exported, bumps) vs price_history (cache, doesn't). Round-trip test incl. the stripped-array old file.
- **Gates:** 688 workspace tests, 0 failed; clippy 0; fmt clean; `cargo deny` ok; smoke exit 124; `Cargo.lock`/`deny.toml` untouched; NO migration (user_version 6); NO core::ssg change.
- **Decisions taken with Guy absent (recommended defaults, flagged):** provider + manual entry; NO visible conversion in 6.5 (primitive + panel only — 6.6 consumes). To revisit at will.

### File List

- `persistence/src/fx.rs` (NEW) + `lib.rs` re-export + `persistence/tests/fx.rs` (NEW, 7 tests); `persistence/src/export.rs` (snapshot array + same-tx import + summary) + `persistence/src/util.rs` (doc) + `persistence/tests/export.rs` (+1 test).
- `core/src/risk/fx.rs` (NEW) + `core/src/risk/mod.rs` (re-export + header).
- `ingestion/src/provider.rs` (trait method) + `adapters/{eodhd,twelvedata}.rs` (fx_pair_symbol + delegate) + `fetch.rs` (dispatch/wrapper/FakeProvider) + `lib.rs`.
- `app/src/fetch.rs` (FxRatesRequest/FxRateOutcome/job/outcome); `app/src/state/fx.rs` (NEW) + `state/mod.rs` + `state/ledger.rs` (pub(super) date validator) + `state/messages.rs` (6 MSG + helper) + `state/tests.rs` (+5); `app/src/wiring/fx.rs` (NEW) + `wiring/mod.rs` + `wiring/fetch.rs` (outcome arm); `app/src/main.rs` (wire + startup push); `app/src/posture.rs` (MSG 95, @tr 356).
- `app/ui/state.slint` (Fx global + FxRateRow) + `app/ui/app.slint` (export) + `app/ui/screens/settings.slint` (panel).

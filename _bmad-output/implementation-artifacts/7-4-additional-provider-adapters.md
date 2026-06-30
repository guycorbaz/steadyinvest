# Story 7.4: Additional provider adapters — Twelve Data (prices)

Status: done (3-layer review 2026-06-30 — 4/4 ACs; 6 patches applied [order=desc pinned, string-or-number error code, missing-currency errors like EODHD, http 403→Forbidden, provider-tagged digest, None-provider fetch guard], 1 deferred → #70 [cross-provider ticker symbols]; workspace 579 tests, fmt/clippy -D/deny green; NO core/contract/migration/SCHEMA_VERSION change; NO new dependency [Cargo.lock/deny.toml unchanged])

<!-- First slice of Epic-7 "additional provider adapters", pulled forward (2026-06-30) to resolve the
     Epic-4-retro D2 gate (Guy: add an alternate provider rather than pay EODHD) and to enable Story
     5.1's price-history cache. Objective = PRICES + REDUNDANCY (Twelve Data), NOT free fundamentals
     (research: free Swiss/SIX fundamentals are paywalled industry-wide). -->

## Story

As Guy,
I want a second data provider (Twelve Data) alongside EODHD,
so that I have a redundant price source and can confront my studies against real prices without paying for EODHD fundamentals.

## Scope decision (Guy, 2026-06-30)

**Add a Twelve Data adapter, objective = PRICES + REDUNDANCY** (resolves D2). Twelve Data covers SIX
(XSWX), free tier (800/day, no card), reliable EOD prices; its **fundamentals are mostly paid**, so this
story delivers the **price path** (the holdings refresh and the Story-5.1 price-history cache) — Swiss
SSG studies stay manual-entry (the honest free path). The provider abstraction (FR15, `MarketDataProvider`
trait) is built for this — a new adapter + an enum-dispatch variant + the existing `ProviderChoice`
selection (Réglages chip + per-provider keychain slot).

## Acceptance Criteria

1. **AC1 — A Twelve Data adapter implements `MarketDataProvider` (FR15).** `ingestion/src/adapters/twelvedata.rs`: a `TwelveDataProvider` (mirrors `EodhdProvider` — `reqwest::Client` with the shared timeouts, injectable `base_url` for tests, NFR-S1 key-free errors via `.without_url()`). **`fetch_latest_price`** hits Twelve Data **`/price`** (`{"price":"…"}`) → the latest close as exact `Decimal` (`None` on an empty/absent price). **`fetch_fundamentals`** hits **`/time_series`** (daily) → a **price-led** `RawFinancials` (native currency from the response `meta.currency`; per-year `high_price`/`low_price` reduced from the daily bars; financial fields — sales/EPS/pre-tax/book/dividend — **left `None`** for manual entry) **plus** the latest close. No new external dependency (`reqwest`/`serde_json`/`rust_decimal` already in `ingestion`).

2. **AC2 — Twelve Data's JSON-level errors are classified (NFR-S1).** Twelve Data returns some errors as a **200 body** `{"status":"error","code":N,"message":"…"}` (not an HTTP status). The adapter inspects the body: `401/403` → `InvalidOrAbsentKey`/`Forbidden`; `404`/symbol-not-found → `TickerNotFound`; `429` → `Quota`; other → `Network`/`Parse`. The **API key never leaks** into any error detail (`.without_url()`; the body message is capped + key-free). The **pure mapping/classification** (no I/O) is the CI-tested heart; the HTTP layer is thin (manual GO/NO-GO with a real key).

3. **AC3 — `ProviderChoice` + dispatch select Twelve Data (FR63).** `ingestion::Provider` gains a `TwelveData` variant (enum dispatch — no `dyn`/`async-trait`). `app::ProviderChoice` gains `TwelveData` (`parse`/`wire` = `"twelvedata"` → the keychain slot `provider:twelvedata` follows automatically; `requires_key` = true). The fetch **worker routes to the configured provider** (currently hardcoded `Provider::Eodhd` — now holds both adapters and selects per job by the job's `ProviderChoice`, threaded through the request). Réglages gains a **Twelve Data** chip beside EODHD.

4. **AC4 — `core`/method untouched; no migration; no new dependency.** **No `core::ssg`/`normalize` change** (the adapter produces `RawFinancials`, consumed by the existing `normalize`; fingerprint/golden/determinism green), **no migration**, **no `contract::SCHEMA_VERSION` bump**, **no new external dependency** (`Cargo.lock`/`deny.toml` unchanged). Every new literal goes through `@tr`; the floor is bumped by exactly the number added; any new `MSG_*` is registered; copy neutral, posture-gated. **Headless dev + automated tests** (the pure mapping + a `FakeProvider`); the live fetch is a **manual GO/NO-GO** with Guy's Twelve Data key (no network in CI).

## Tasks / Subtasks

- [x] **Task 1 — `ingestion`: the Twelve Data adapter (AC1, AC2)** — `ingestion/src/adapters/twelvedata.rs`, `mod.rs`
  - [x] `TwelveDataProvider { http, base_url }` + `new()`/`with_base_url()` reusing the shared `build_client()` timeout pattern (factor it to a crate-shared helper or duplicate the 3 lines). `const DEFAULT_BASE_URL = "https://api.twelvedata.com"`.
  - [x] `get_json(url) -> Value` (key-free errors); a `classify_twelvedata(value)` that detects `{"status":"error","code":N}` and maps `N` → `ProviderError` (401/403/404/429/other), plus an HTTP-status fallback for non-200s.
  - [x] `fetch_latest_price`: `GET {base}/price?symbol={ticker}&apikey={key}` → parse `price` (string → `Decimal::from_str_exact`); `None` if absent/empty.
  - [x] `fetch_fundamentals`: `GET {base}/time_series?symbol={ticker}&interval=1day&outputsize=5000&apikey={key}` → `map_twelvedata` (PURE, no I/O, CI-tested): read `meta.currency`; group `values[]` by year → `high_price` = max of the year's `high`, `low_price` = min of the year's `low` (exact `Decimal`, `RawAmount{value, currency}`); financial fields `None`; `splits` empty. Latest close = the most-recent `values[].close` (the series is newest-first by default — confirm/order). Return `RawFetch { financials, latest_price }`.
  - [x] Unit tests on `map_twelvedata` + `classify_twelvedata` with fixture JSON: a 2-year time-series maps high/low per year + currency; an empty `values` → empty years (no panic); a `{"status":"error","code":401}` → `InvalidOrAbsentKey`; `429` → `Quota`; a symbol-not-found code → `TickerNotFound`; the price endpoint parse (`{"price":"104.23"}` → `Decimal`, `{}` → `None`). **No network.**

- [x] **Task 2 — `ingestion`: `Provider` enum variant + dispatch (AC3)** — `ingestion/src/fetch.rs`, `lib.rs`
  - [x] `Provider::TwelveData(TwelveDataProvider)` + both `match` arms (`fetch_fundamentals`/`fetch_latest_price`). Re-export the adapter from `adapters::mod`/`lib` as `eodhd` is.
  - [x] Confirm `fetch_canonical`/`fetch_price` are provider-agnostic (they take `&Provider`) — no change needed.

- [x] **Task 3 — `app`: `ProviderChoice::TwelveData` + worker routing (AC3)** — `app/src/provider.rs`, `app/src/fetch.rs`, `app/src/main.rs`
  - [x] `ProviderChoice::TwelveData` + `parse("twelvedata")`/`wire()="twelvedata"`/`requires_key()=true`; update the round-trip/serde/requires-key tests. The keychain slot `provider:twelvedata` follows from `wire()` (no keychain change).
  - [x] Thread the chosen `ProviderChoice` into the worker: add `provider: ProviderChoice` to `FetchRequest`/`TestKeyRequest`; the worker builds **both** adapters once (the connection-pool reuse) and selects per job (`match req.provider { TwelveData => &twelvedata, _ => &eodhd }`). The enqueue sites already read `config.preferred_provider` + `resolve_provider_key` — pass the choice into the request.
  - [x] Réglages: a **Twelve Data** provider chip (settings.slint) beside EODHD; the existing key save/test/delete flow is per-`ProviderChoice` already (keychain slot follows). `@tr` floor + any `MSG_*` bumped exactly.

- [x] **Task 4 — Gates (AC4)** — fmt, clippy `-D`, `test --workspace`, `deny` + smoke launch. Confirm `core::ssg`/`normalize` re-diff clean; **no migration**, **no `SCHEMA_VERSION` bump**, **`Cargo.lock`/`deny.toml` unchanged** (no new dep — `reqwest`/`serde_json`/`rust_decimal` already in `ingestion`); `@tr` + `USER_FACING_MESSAGES` bumped exactly.

## Dev Notes

### Scope
The **second provider adapter** (Twelve Data), price-led: `fetch_latest_price` (the holdings-refresh + Story-5.1 price-history path) is the core deliverable; `fetch_fundamentals` returns price-derived high/low + empty financials (free-tier honest). Resolves **D2** (alternate provider) and unblocks **Story 5.1** (real post-decision prices). **Provider-layer only** — no `core`/contract/method/migration change.

### Out of scope (deferred)
- **Twelve Data fundamentals** (sales/EPS/etc.) — paid tier; Swiss studies stay manual-entry. A later refinement could map `/income_statement` for tickers/plans that serve it.
- **Automatic provider fallback chain** (try EODHD → Twelve Data on failure/quota) — Epic-7 Story 7-9. This story only adds the adapter + manual selection.
- **Story 5.1 confront** itself — separate (this story gives it the price source).

### Architecture decisions this story honours
- [Source: ingestion/src/provider.rs] — `MarketDataProvider` is the provider-agnostic boundary (FR15); the adapter's only job is `ticker → RawFinancials (+ latest price)`; normalization is central in `core::normalize` (not the adapter's concern).
- [Source: ingestion/src/fetch.rs] — concrete providers are **enum-dispatched** (`Provider`), not `dyn` (native async-fn-in-trait isn't dyn-compatible; enum dispatch avoids `async-trait`).
- [Source: app/src/provider.rs + keychain.rs] — `ProviderChoice` is the persisted *choice*; the keychain slot is `provider:{wire}` (a new variant gets its slot for free); the key never leaves the OS secret store (NFR-S1).
- [Source: app/src/fetch.rs] — the worker reuses one provider + its `reqwest` pool across jobs; holding both adapters and selecting per job keeps that benefit.

### Notes & guardrails
- **Twelve Data errors are JSON, not HTTP.** A 200 body `{"status":"error","code":N,"message":…}` must be classified (don't treat it as a valid price/series). Mirror EODHD's cause-named `ProviderError` taxonomy; cap the message + never include the key (`.without_url()`; the URL carries `?apikey=`).
- **Exact decimals only** (NFR-C1) — parse prices with `Decimal::from_str_exact`, never `f64`.
- **Series order** — Twelve Data `/time_series` returns newest-first by default; the latest close is `values[0].close`. Confirm and handle an empty `values`.
- **Currency** — read `meta.currency` for the `RawAmount` currency; if absent, fall back to a neutral default and let `normalize` proceed (a missing currency is a degenerate fixture, not a live case).
- **NFR-S1** — the manual key-test reuses the existing per-provider flow; no key in logs/errors.

### Manual on-display GO/NO-GO (Guy)
In Réglages pick **Twelve Data**, save your Twelve Data API key, **Tester** → "clé valide". Add a holding for a Swiss ticker (e.g. `NESN`) → **Rafraîchir les prix** → the present price fills from Twelve Data. Open a study for the same ticker → fetch → the per-year high/low fill (financials stay "à remplir"). Switch back to EODHD → both providers coexist (redundancy).

### Project Structure Notes
- New `ingestion/src/adapters/twelvedata.rs` + a `Provider::TwelveData` arm + `ProviderChoice::TwelveData` + the worker routing + a Réglages chip. **No `core`/contract/migration/`SCHEMA_VERSION` change, no new external dependency.**
- Posture floors at story start: `@tr` floor **305** (Story 5.5), `USER_FACING_MESSAGES` inventory **74**. Bump by exactly the number added (likely +1 `@tr` for the "Twelve Data" chip label; no new `MSG_*` expected — the key flow is reused).

### References
- [Source: ingestion/src/adapters/eodhd.rs] — the adapter to mirror (`build_client`, `get_json`, `.without_url()`, `classify_status`, `latest_eod_close`, per-year high/low reduction).
- [Source: ingestion/src/provider.rs] — `MarketDataProvider` trait + `RawFetch`.
- [Source: ingestion/src/fetch.rs] — `Provider` enum dispatch; `fetch_canonical`/`fetch_price` (provider-agnostic).
- [Source: app/src/provider.rs, keychain.rs, fetch.rs] — `ProviderChoice` (parse/wire/requires_key), `provider:{wire}` keychain slot, the worker + enqueue sites (`preferred_provider`/`resolve_provider_key`).
- [Source: core/src/normalize/types.rs] — `RawFinancials { native_currency, years: Vec<RawYear>, splits }`, `RawYear` (all-Optional fields), `RawAmount { value, currency }`.
- [Source: project memory] — D2 resolution = Twelve Data (prices + redundancy); free Swiss fundamentals paywalled industry-wide.

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — `claude-opus-4-8[1m]`

### Debug Log References

- **Twelve Data errors are JSON, not HTTP** — `/price` and `/time_series` can return a 200 body
  `{"status":"error","code":N,"message":…}`. `get_json` runs `classify_twelvedata` on every 200 body
  before treating it as data; the HTTP-status path (`http_status_error`) covers the rarer non-200s.
  Key never leaks (`.without_url()` + the body message capped at 200 chars, key-free).
- **Price-led `fetch_fundamentals`** — `map_twelvedata` builds a `RawFinancials` with currency (from
  `meta.currency`, fallback `USD` for a degenerate fixture) + per-year high/low (max/min of the daily
  bars), financial fields `None`. A Twelve Data study-fetch thus fills the price rows and leaves
  fundamentals "à remplir" — the honest free-tier behaviour. `fetch_latest_price` (the core path) hits
  `/price`; the time-series is newest-first so the latest close is `values[0]`.
- **Worker routing** — the worker was hardcoded `Provider::Eodhd`. It now builds **both** adapters once
  (keeping the `reqwest` connection-pool reuse) and a `select(choice)` closure picks per job;
  `FetchRequest`/`TestKeyRequest` gained a `provider: ProviderChoice` field, passed by the enqueue
  sites (which already read `config.preferred_provider`). The key-test ticker is per-provider
  (`AAPL.US` for EODHD, `AAPL` for Twelve Data — the symbol conventions differ).
- **Keychain slot is free** — `ProviderChoice::TwelveData.wire() == "twelvedata"` → the slot
  `provider:twelvedata` follows from the existing `provider:{wire}` scheme; no keychain change. The
  Réglages key panel now shows for `provider != "none"` (was `== "eodhd"`), so the save/test/delete
  flow works for Twelve Data too.

### Completion Notes List

- **AC1** — `ingestion::adapters::twelvedata::TwelveDataProvider` (mirrors EODHD: timeouts, injectable
  `base_url`, key-free errors). `fetch_latest_price` via `/price`; `fetch_fundamentals` via
  `/time_series` → price-led `RawFinancials` + latest close. 5 pure-mapping tests.
- **AC2** — `classify_twelvedata` maps the JSON `code` (401/403/404/429/other) → the EODHD
  `ProviderError` taxonomy; message capped + key-free (a leak/cap test).
- **AC3** — `Provider::TwelveData` enum arm; `ProviderChoice::TwelveData` (parse/wire/requires_key +
  tests); worker selects per job; Réglages "Twelve Data" chip; both providers coexist (redundancy).
- **AC4** — no `core::ssg`/`normalize`/contract change, no migration, no `SCHEMA_VERSION` bump, **no new
  dependency** (`Cargo.lock`/`deny.toml` unchanged — `reqwest`/`serde_json`/`rust_decimal` already in
  `ingestion`). `@tr` floor 305→306 (the chip). Workspace **577 tests**; fmt/clippy `-D`/deny green;
  smoke launch exit 124. **Live fetch = manual GO/NO-GO with Guy's Twelve Data key** (no network in CI).

### File List

- `ingestion/src/adapters/twelvedata.rs` (A) — `TwelveDataProvider` + `map_twelvedata`/`classify_twelvedata`/`latest_close`/`price_of` + 5 tests
- `ingestion/src/adapters/mod.rs` (M) — register `twelvedata`
- `ingestion/src/fetch.rs` (M) — `Provider::TwelveData` variant + both dispatch arms
- `app/src/provider.rs` (M) — `ProviderChoice::TwelveData` (parse/wire/requires_key) + tests
- `app/src/fetch.rs` (M) — `FetchRequest`/`TestKeyRequest.provider`; worker holds both adapters + per-job select; per-provider key-test ticker
- `app/src/main.rs` (M) — pass `provider` into the 3 enqueue sites (fetch / refresh-holding / test-key)
- `app/src/posture.rs` (M) — `@tr` floor 305→306
- `app/ui/screens/settings.slint` (M) — "Twelve Data" provider chip + key panel shown for any keyed provider

### Review Findings (3-layer adversarial — 2026-06-30)

3 layers (Blind / Edge / Acceptance). **No CRITICAL/HIGH in the adapter; all 4 ACs satisfied** (Auditor;
Cargo.lock/deny.toml unchanged + @tr +1 verified; worker genuinely routes). **6 patches applied · 1
deferred (#70) · rest dismissed (EODHD parity / already handled).**

- [x] [Review][Patch] **MED** — `/time_series` didn't pin `&order=desc` while `latest_close` reads `values[0]`; an ascending response would make the latest price a stale bar. Pinned `order=desc`. [twelvedata.rs]
- [x] [Review][Patch] **MED** — the error `code` was parsed strictly as `u64`; a string `"401"` would fall to `Parse`, mis-bucketing auth/quota. Now accepts number **or** string. [twelvedata.rs]
- [x] [Review][Patch] **MED** — a missing `meta.currency` silently labelled prices `USD` (and suppressed `normalize`'s CurrencyMismatch). Now **errors** like EODHD does. [twelvedata.rs]
- [x] [Review][Patch] **MED** — the dependency digest hardcoded the `eodhd:` provenance prefix; a Twelve Data fetch mislabelled it. Now **provider-tagged** (`Provider::tag()`). [ingestion/src/fetch.rs]
- [x] [Review][Patch] **MED** — a `None`-provider fetch/refresh routed to EODHD → a misleading "invalid key" notice. Now guarded with a distinct **"no provider selected"** notice (`MSG_PROVIDER_NONE`; inventory 74→75). [app/src/main.rs, state.rs]
- [x] [Review][Patch] **LOW** — the HTTP-status fallback collapsed 403 into `InvalidOrAbsentKey`; now 403→`Forbidden` (consistent with the JSON path). [twelvedata.rs]
- [x] [Review][Defer] **HIGH-impact** — cross-provider ticker symbol conventions differ (EODHD `NESN.SW` vs Twelve Data `NESN`); switching providers on an existing study sends the wrong-format symbol → `TickerNotFound`. Deferred → **GitHub #70** (a symbol-translation/per-provider-ticker layer is beyond the adapter; the ticker is user-entered, the failure is a neutral notice). [app/src/main.rs / cross-provider]

**Dismissed (with rationale):** empty `{"status":"ok","values":[]}` — the app's existing #46 empty-payload
guard (provider-agnostic) already surfaces "no data", and a key-test on an empty-but-OK body correctly
reports the key valid; partial-current-year high/low, inverted high<low, `dec` swallowing parse failures,
discarded `Retry-After` — all **parity with the EODHD adapter** (not 7.4 regressions); the shared env-var
key fallback — the 3.1 interim path (the keychain is per-provider); `select(None) → eodhd` — safe (now
guarded upstream) + the verified-same `provider`/`provider_choice` bindings.

2 new patch tests (string error code; missing currency errors). Workspace **579 tests** green; fmt /
clippy `-D` / deny clean; smoke launch 124.

### Change Log

- 2026-06-30 — Story 7.4 (first slice) dev complete (4/4 tasks). Twelve Data price adapter (resolves the
  D2 gate — alternate provider — and unblocks Story 5.1's price source). `fetch_latest_price` (`/price`)
  + price-led `fetch_fundamentals` (`/time_series`); JSON-error classification; `Provider`/`ProviderChoice`
  variants + worker routing + Réglages chip. No core/contract/migration/`SCHEMA_VERSION` change; no new
  dependency (`Cargo.lock`/`deny.toml` unchanged). Workspace 577 tests green.

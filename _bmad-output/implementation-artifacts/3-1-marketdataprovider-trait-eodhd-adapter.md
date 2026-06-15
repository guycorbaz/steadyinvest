# Story 3.1: `MarketDataProvider` trait & first adapter (EODHD)

Status: ready-for-dev

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to auto-fetch a security's data from a provider,
so that I avoid typing ~10 years of fundamentals by hand.

## Acceptance Criteria

(From epics.md §Story 3.1, lines ~17-33. FR15, FR17–18. Scope-resolved with Guy 2026-06-15 — see Dev Notes "Scope decision".)

1. **(FR15 — fetch → normalize → provider cells)** **Given** a configured provider (EODHD) with an API key available, **when** an auto-fetch runs for a ticker, **then** fundamentals + yearly high/low prices + present price + estimates are retrieved over HTTP (reqwest `rustls-no-provider` + the `ring` provider + tokio), **mapped to the provider's raw shape** (`core::normalize::RawFinancials`), passed **through `core::normalize` (Epic 1, unchanged)** into `CanonicalFinancials`, and surfaced into `contract::YearData`/`Cell`s stamped **`Source::Provider`** with a full `Provenance` (logical_version + RFC3339 timestamp from the app's injected `Clock` + a real `hash_of_dependencies` digest, not the manual placeholder) (FR15, FR17–18).
2. **(off-UI-thread)** **Given** the fetch involves network I/O, **then** it runs **off the Slint UI thread** (a dedicated worker thread with a `current_thread` tokio runtime) and its result is marshalled back via `slint::invoke_from_event_loop` — the UI never blocks during the fetch (an in-progress state is shown, the window stays responsive).
3. **(per-cell coverage; absent stays hand-editable)** **Given** the normalized result, **then** each cell reports its **coverage** — `Coverage::Present` where the provider returned a value, `Coverage::ToFill` where it did not (absent / partial) — and **absent cells remain editable by hand** (no fabricated zeros; `unknown` is never stored as `0`). Fresh provider cells are `Review::None` (unreviewed) and `Freshness::Current`.
4. **(minimal trigger UI)** **Given** an open study, **then** a neutral **"Récupérer les données (fournisseur)"** affordance triggers the fetch; while it runs a neutral in-progress state shows; on success the study's **empty/to-fill** load-bearing cells are filled from the provider result and the form + verdict recompute through the existing rail; on failure a single neutral notice is shown (the *rich* cause-classified banner is Story 3.5). **No reconciliation of existing manual data** happens here (manual-wins reconciliation is Story 3.4) — 3.1 fills gaps and (for a fresh/empty study) populates the year grid.
5. **(keyless-capable trait; key injected by app)** **Given** the `MarketDataProvider` trait, **then** it accepts an **optional** API key (`Option<&str>` — keyless providers supported) and the **key is injected by `app`, never read inside `ingestion`**. For 3.1 the app reads the EODHD key from the **`STEADYINVEST_EODHD_API_KEY` env var** (documented interim; Story 3.2 moves it to the OS keychain). A missing key yields a neutral "clé absente" outcome, not a panic.
6. **(DoD)** **Given** the network call cannot run in CI (sandbox has no network), **then** the **EODHD JSON → `RawFinancials` mapping is a pure function unit-tested against recorded fixtures** (`ingestion/tests/fixtures/eodhd-*.json`); the **fetch→normalize→stamp pipeline is tested via a `FakeProvider`** returning canned `RawFinancials` (proving provenance/coverage/`Source::Provider` stamping + the recompute); the **live HTTP fetch is a documented manual GO/NO-GO** (Guy runs it with his real key on his display). 4 CI gates green `--locked`; **`core`/`contract`/`persistence` types + the method fingerprint + golden gates stay unchanged** (no method change — provider data flows *through* the existing `normalize`); **`Cargo.lock` WILL grow** (reqwest TLS stack + `ring` + tokio + sha2 light up — expected for the first network story). File List ⇄ git exact (issue #18).

## Tasks / Subtasks

- [ ] **Task 1 — Workspace deps: TLS stack + ring + ingestion→core (Cargo)** (AC: 1, 6)
  - [ ] Workspace `Cargo.toml`: set `reqwest = { version = "0.13", default-features = false, features = ["json", "rustls-no-provider", "webpki-roots"] }` (per the 2026-06-15 #5/B5 revalidation — the old `rustls-tls` feature is gone). Add `ring = "0.17"` to `[workspace.dependencies]` (the pure-Rust crypto provider; no cmake). Add `sha2` to the workspace if not already shared (ingestion needs it for the dependency digest — `core` already uses sha2; reuse the same pin).
  - [ ] `ingestion/Cargo.toml`: add `steadyinvest-core = { workspace = true }` (ingestion calls `core::normalize`), `ring`, `sha2`, `serde_json`. Keep `contract`, `reqwest`, `tokio`, `serde`, `thiserror`. Remove the `#![allow(unused_crate_dependencies)]` from `ingestion/src/lib.rs` once reqwest/tokio are actually used.
  - [ ] Install the `ring` `CryptoProvider` once at process start (a `pub fn install_crypto_provider()` in `ingestion`, called from `app/src/main.rs` before any HTTP). Idempotent (`install_default` returns `Err` if already installed — ignore).
  - [ ] Confirm `cargo deny check` still passes with the new TLS/crypto tree (note the pre-existing `GPL-3.0` warning is unrelated); if `ring`/`rustls`/`webpki-roots` licenses need an allow entry in `deny.toml`, add it with a comment. **`deny.toml` MAY change here** (new licenses) — that is in-scope for this story (unlike Epic 2).

- [ ] **Task 2 — The `MarketDataProvider` trait + error model (ingestion)** (AC: 1, 5)
  - [ ] `ingestion/src/provider.rs`: define `pub trait MarketDataProvider` with `async fn fetch_fundamentals(&self, ticker: &str, api_key: Option<&str>) -> Result<RawFinancials, ProviderError>` (native `async fn` in trait — MSRV 1.96 supports it). The trait imports `core::normalize::RawFinancials` as the return shape (the adapter's job ends at the raw shape; normalization is a separate step). `Send + Sync` bound so it can run on the worker.
  - [ ] `ingestion/src/error.rs`: `pub enum ProviderError` (`thiserror`) with the cause variants the later stories (3.5) will classify on — at minimum `Network { detail }`, `Quota { retry_after: Option<…> }`, `InvalidOrAbsentKey`, `TickerNotFound { ticker }`, `Parse { detail }`, `Unsupported { detail }`. Neutral, fact-stating messages (FR13 — persistence-style; ingestion gets its own crate-local banned-verb posture test like `persistence::error`).
  - [ ] **Dyn-dispatch decision (recommend, no new dep):** the app holds more than one provider (real EODHD + the test `FakeProvider`). To avoid `async-trait`, dispatch via a small `enum Provider { Eodhd(EodhdProvider), Fake(FakeProvider) }` (or a generic worker) rather than `Box<dyn MarketDataProvider>` — native async-fn-in-trait is not dyn-compatible. Document the choice; do NOT add `async-trait` without flagging.

- [ ] **Task 3 — EODHD adapter: HTTP + pure JSON→RawFinancials mapping (ingestion)** (AC: 1, 6)
  - [ ] `ingestion/src/adapters/eodhd.rs`: `pub struct EodhdProvider { http: reqwest::Client, base_url: String }` with a constructor (default base `https://eodhd.com/api`; `base_url` injectable so tests/fixtures can point elsewhere). `impl MarketDataProvider for EodhdProvider`.
  - [ ] **Pure mapping (the CI-testable core):** a free function `pub fn map_eodhd_fundamentals(json: &serde_json::Value, ticker: &str) -> Result<RawFinancials, ProviderError>` that turns the EODHD fundamentals JSON into `RawFinancials` — `native_currency` from the response's currency, one `RawYear` per reported fiscal year (sales/eps/high_price/low_price/dividend_per_share/pre_tax_profit/book_value_per_share as `RawAmount{value, currency}`), and `splits` from the splits section. Map **only what's present**; absent fields stay `None` (NEVER 0). This function does **no I/O** and is unit-tested against fixtures.
  - [ ] The `fetch_fundamentals` impl builds the EODHD URL (`/fundamentals/{ticker}?api_token={key}&fmt=json`, keyless → `demo`/no token per EODHD), GETs via `reqwest`, classifies the HTTP status into `ProviderError` (401/403→`InvalidOrAbsentKey`, 404→`TickerNotFound`, 429→`Quota`, network→`Network`), then calls `map_eodhd_fundamentals`. Estimates/present-price endpoints: fetch if cheap; otherwise leave those cells `ToFill` (documented — they can be a later refinement; do not block the story on optional EODHD endpoints).
  - [ ] `ingestion/tests/fixtures/eodhd-*.json` (record 2–3 representative real-shape responses: a clean US ticker, one with a split, one with missing fields) + `ingestion/tests/eodhd_mapping.rs`: assert `map_eodhd_fundamentals` produces the expected `RawFinancials` (years, currencies, splits, the right `None`s for absent fields). Anti-circularity: hand-derive the expected raw, don't echo the mapper's output.

- [ ] **Task 4 — Fetch orchestration: trait → normalize → digest (ingestion)** (AC: 1, 3)
  - [ ] `ingestion/src/lib.rs` (or `fetch.rs`): `pub async fn fetch_canonical(provider: &Provider, ticker: &str, api_key: Option<&str>) -> Result<FetchedFinancials, IngestionError>` where `FetchedFinancials { canonical: CanonicalFinancials, digest: String }`. It calls `provider.fetch_fundamentals` → `core::normalize::normalize(raw)` (mapping `NormalizeError` into `IngestionError`) → computes `digest` = SHA-256 hex over a value-normalized canonical identity (`"eodhd:{ticker}"` + the canonical years' decimals, `Decimal::normalize`d so `3.0`==`3`). This digest becomes each provider cell's `provenance.hash_of_dependencies` (a real digest per #21, replacing the `"manual"` placeholder).
  - [ ] Keep `app`-owned concerns OUT of ingestion: no `Clock`, no journal `logical_version`, no Slint. ingestion returns data; the app stamps the time/version + persists.

- [ ] **Task 5 — App: off-thread fetch worker + canonical→provider-cells mapping (app)** (AC: 1, 2, 3, 5)
  - [ ] `app/src/fetch.rs` (NEW): a fetch worker — a dedicated `std::thread` running a `tokio::runtime::Builder::new_current_thread().enable_all()` runtime that services fetch requests over a channel; each result is posted back with `slint::invoke_from_event_loop`. Spawn lazily on first fetch or at startup. The UI thread never calls `block_on`.
  - [ ] Map `CanonicalFinancials` → `contract::YearData`/`Cell` in the app (it owns the `Clock` + journal): for each canonical year/field, `Cell { value: dec.map(Money::from), source: Source::Provider, freshness: Freshness::Current, review: Review::None, coverage: if value present { Present } else { ToFill }, provenance: Provenance { source: Provider, logical_version: <journal current>, timestamp: clock.now(), hash_of_dependencies: <digest from FetchedFinancials> } }`. Load-bearing absent fields → `ToFill` cells (stay hand-editable). Reuse `entry::` helpers where they fit; do NOT route through `Cell::edited`'s manual rail (that's reconciliation — Story 3.4).
  - [ ] A `JournalState` method (e.g. `apply_provider_fetch(study_id, fetched)`) that fills the study's **empty/to-fill** load-bearing cells from the provider result, then `put_study` (bumps `logical_version`) and re-pushes the form + recompute. **Fill-gaps-only** for 3.1: do not overwrite a cell that already has a manual value (full manual-wins reconciliation = 3.4). For a fresh/empty study, materialize the year grid from the provider years.
  - [ ] Key from env: read `STEADYINVEST_EODHD_API_KEY` (interim). Missing → a neutral "clé absente" notice (registered `MSG_*`, posture-scanned), no fetch attempt.

- [ ] **Task 6 — Minimal trigger UI + in-progress/result state (app)** (AC: 2, 4)
  - [ ] A neutral **"Récupérer les données (fournisseur)"** ink-only button on the study screen (near the §2/§3 grids or the header). Disabled while a fetch is in flight and on read-only/demo studies. Clicking enqueues the fetch on the worker.
  - [ ] An in-progress state (a neutral "Récupération en cours…" line / disabled button) and a result notice: success ("Données récupérées" + cells filled), or a single neutral failure notice mapping `ProviderError`→a fact-stating message (the rich cause banner is 3.5). All copy `@tr`/`MSG_*`, posture-scanned (FR13).
  - [ ] Wire the Slint callback → `state` → worker; on `invoke_from_event_loop`, apply the result + recompute. The fetched verdict integrity holds by construction (provider cells are `Review::None` → not `ValidatedFresh` → verdict is `Provisional`/`Withheld` until the user validates — exactly the Epic-2 gate; no special-casing).

- [ ] **Task 7 — Gates, posture, DoD** (AC: 6)
  - [ ] 4 CI gates green `--locked`. New crate-local posture test in `ingestion` for `ProviderError` messages (banned-verb, like `persistence::error`). App posture floors bumped for the new `@tr`/`MSG_*` strings.
  - [ ] **`core`/`contract`/`persistence` source types unchanged**; method fingerprint + determinism + golden + corpus gates **stay green** (provider data flows through the existing `normalize`; no method touch). `Cargo.lock` grows (TLS/ring/tokio/sha2) — that is expected and in-scope; `deny.toml` may gain license allowances for the new crates.
  - [ ] Headless tests carry the proof (fixture mapping + FakeProvider pipeline); the live HTTP fetch + the on-display in-progress/fill behaviour are a documented manual GO/NO-GO (human; the Wayland sandbox blocks both network and screenshots). Don't mark `[x]` for a non-existent test. File List ⇄ git exact (#18).

## Dev Notes

### Scope decision (Guy, 2026-06-15) — READ FIRST

This is the **first network story** — it opens the `ingestion` crate and the HTTP/async stack. Scoped tightly; later Epic-3 stories own the rest:
1. **UX slice = plumbing + a minimal button.** 3.1 delivers the trait + EODHD adapter + off-thread fetch + normalize→provider-cell stamping, triggered by a "Récupérer (fournisseur)" button that **fills empty/to-fill cells** on the open study and recomputes. **NOT in 3.1:** manual-wins **reconciliation** of cells that already hold manual data (Story **3.4**); **staleness** thresholds / `Freshness::Stale` setting (Story **3.3**); the **rich cause-classified failure banner** (Story **3.5**) — 3.1 shows a single neutral notice. The seam spike (`app/src/seam_check.rs`, 2026-06-15) already proved the downstream stale/✓→?/verdict-degrade seams fire; 3.1 is the *upstream* producer.
2. **API key = `STEADYINVEST_EODHD_API_KEY` env var (interim).** Story **3.2** moves it to the OS keychain (`keyring 3.x`, revalidated). The trait stays keyless-capable (`Option<&str>`).
3. **Validation = fixtures + `FakeProvider` + manual HTTP check.** The pure JSON→`RawFinancials` mapping is fixture-unit-tested; the fetch→normalize→stamp pipeline is `FakeProvider`-tested; the live EODHD call is a manual GO/NO-GO with Guy's real key (no network in CI).

### Architecture & dependency rails (from the 3.1 context map + architecture.md)

- **Dependency edges:** add **`ingestion → core`** (for `normalize`) and `ingestion → {ring, sha2, serde_json}`. App already deps `ingestion`. **NEVER** `core → ingestion` / `core → contract` runtime (Cardinal Rule — no I/O in core). ingestion must NOT dep `persistence` (keys are injected by app, not read from the DB).
- **The integration point is `core::normalize`** (`core/src/normalize/mod.rs`): `pub fn normalize(raw: RawFinancials) -> Result<CanonicalFinancials, NormalizeError>`. Input `RawFinancials { native_currency: String, years: Vec<RawYear>, splits: Vec<SplitEvent> }` (`core/src/normalize/types.rs`). `RawYear` fields: `year, period_months, fiscal_year_end_month, sales, eps, high_price, low_price, dividend_per_share, pre_tax_profit, net_profit, tax_rate, book_value_per_share` (amounts are `RawAmount { value: Decimal, currency: String }`). The adapter's only job is `EODHD JSON → RawFinancials`; `normalize` does IFRS/GAAP, split-adjust, fiscal-period, currency-of-report — **do not reimplement any of it**.
- **Output `CanonicalFinancials { years: Vec<CanonicalYear>, findings, usable_years }`**; `CanonicalYear` has `Option<Decimal>` per field + `usability`. The four **load-bearing** fields (`core::method::LOAD_BEARING_YEAR_FIELDS = ["sales","eps","high_price","low_price"]`) determine usability — a year missing any is `Insufficient`.
- **Stamping (`contract`):** `Cell { value: Option<Money>, source, freshness, review, coverage, provenance }`; `Provenance { source, logical_version: u64, timestamp: Timestamp(String), hash_of_dependencies: String }`. Provider fetch → `Source::Provider`, `Freshness::Current`, `Review::None`, coverage `Present`/`ToFill`. The digest replaces the `"manual"` placeholder (#21). `Money` is exact decimal (string in JSON); hash must value-normalize Decimals (`"3.0"`==`"3"`).
- **Threading (architecture.md:391-397, 525):** dedicated worker thread + `current_thread` tokio 1.52 runtime; **never touch UI state off the main thread** — marshal via `slint::invoke_from_event_loop`. The app has **no tokio runtime today** (`main.rs` notes ingestion/tokio "light up in Epic 3") — 3.1 introduces it on the worker, NOT on the Slint loop.
- **TLS (architecture.md:318-323, revalidated 2026-06-15):** `reqwest` features `rustls-no-provider` + `webpki-roots`; install the `ring` `CryptoProvider` once at startup (no aws-lc-rs, no cmake → portable single binary).

### Verdict integrity is free here

Provider cells are `Review::None` (unvalidated). The Epic-2 gate (`cell_to_gate_state`: only `(Validated, Current)` is green) means a freshly-fetched study is **Provisional/Withheld until the user validates** — exactly the intended posture (the app is the decider's tool, not an oracle). No special handling needed; do not auto-validate provider data.

### Files this story creates / touches

- **NEW:** `ingestion/src/provider.rs`, `ingestion/src/error.rs`, `ingestion/src/adapters/mod.rs`, `ingestion/src/adapters/eodhd.rs`, `ingestion/src/fetch.rs` (or fold orchestration into `lib.rs`), `ingestion/tests/fixtures/eodhd-*.json`, `ingestion/tests/eodhd_mapping.rs`, `app/src/fetch.rs`.
- **UPDATE:** `ingestion/src/lib.rs` (re-exports, crypto install, drop the unused-deps allow), `ingestion/Cargo.toml`, workspace `Cargo.toml` (reqwest features + ring + sha2), `deny.toml` (license allowances if needed), `app/src/main.rs` (install crypto provider; spawn/own the fetch worker), `app/src/state.rs` (the `apply_provider_fetch` method + env-key read), `app/ui/screens/study_screen.slint` + a `MSG_*`/`@tr` set (the button + in-progress/result copy), `app/src/posture.rs` (floors for the new strings).
- **DO NOT** change `core`/`contract`/`persistence` source types or the method (provider data flows through existing `normalize`); pinned method/golden/corpus gates stay green.

### Testing standards

- All gates `--locked`. New `ingestion` tests: the pure mapping (fixtures) + the `ProviderError` posture test. New `app` tests: the `FakeProvider` → `apply_provider_fetch` pipeline (provenance stamped `Source::Provider`, coverage Present/ToFill, fill-gaps-only does not overwrite a manual value, the recompute yields a Provisional verdict for unvalidated provider data). The live HTTP path is **not** a CI test — it's a manual GO/NO-GO (record the result in Completion Notes once Guy runs it).
- `FakeProvider` returns canned `RawFinancials` (and an error variant on demand) so the whole app pipeline is deterministic and offline.
- Anti-circularity for fixtures: hand-derive expected `RawFinancials`; never paste the mapper's own output back as the expectation.

### Project Structure Notes

- This is the first story to legitimately grow `Cargo.lock` and possibly `deny.toml` (TLS/ring/tokio/sha2) — that is expected and in-scope, a deliberate exception to Epic 2's app-crate-only/pinned-Cargo.lock discipline. The trust gates that MUST stay green are the **method fingerprint / determinism / golden / corpus** (no calculation change), not the dependency lock.

### References

- [Source: epics.md#Story 3.1] — user story + ACs; Epic 3 intro ("through Epic 1's canonical `normalize`", "branches onto the manual-mutation rail").
- [Source: architecture.md:278-279, 390-398, 318-323, 525, 646-653] — ingestion role; `MarketDataProvider` trait + EODHD; reqwest TLS (revalidated); `invoke_from_event_loop`; the crate file tree (`provider.rs`, `adapters/eodhd.rs`, normalize moved to core).
- [Source: core/src/normalize/mod.rs + types.rs] — `normalize()` signature; `RawFinancials`/`RawYear`/`RawAmount`/`SplitEvent`; `CanonicalFinancials`/`CanonicalYear`.
- [Source: core/src/method/mod.rs:26] — `LOAD_BEARING_YEAR_FIELDS`.
- [Source: contract/src/cell.rs + provenance.rs + study.rs] — `Cell`/`Source`/`Freshness`/`Review`/`Coverage`/`Provenance`/`YearData`.
- [Source: app/src/seam_check.rs + docs/spikes/spike-d-stale-reconcile.md] — the downstream seams 3.1 feeds; the finding that `mutate_cell` hardcodes `manual_provenance` (3.1 adds the provider stamping path).
- [Source: Cargo.toml + architecture.md §Tech Stack revalidation 2026-06-15 (#5/B5)] — reqwest `rustls-no-provider`+`webpki-roots`+`ring`; keyring deferred to 3.2.
- [Source: _bmad-output/implementation-artifacts/epic-2-retro-2026-06-15.md] — B3 spike done (GO), this is the upstream producer; File-List #18; 3-layer review + visual GO/NO-GO conventions.

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List

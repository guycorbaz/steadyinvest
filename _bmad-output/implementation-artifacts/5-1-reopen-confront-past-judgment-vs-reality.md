# Story 5.1: Reopen & confront a past judgment vs reality

Status: done

<!-- ⚠️ GATED on Epic-4 retro action D2 (provider decision) for LIVE data. Dev-able headless via
     fixtures/FakeProvider; the on-display GO/NO-GO with Guy's real data waits on D2. -->

## Story

As Guy,
I want to overlay a past study's projection on what actually happened,
so that I learn from my own past judgments.

## Acceptance Criteria

1. **AC1 — A post-decision price-history cache in `persistence` (ADD13, FR50).** A new **price-history cache** stores dated closing prices per security ticker, **sourced via the Epic-3 `ingestion` refresh** (the same `/eod` close path Story 4.4 already uses on the free plan). The cache is a **new normalized table** (`price_history`: ticker, date, close TEXT-decimal, source, created_at) added by a **migration v4→v5** (the project's 5th; `PRAGMA user_version` 4→5; `contract::SCHEMA_VERSION` stays 1 — a normalized table, not a serde blob). Closes are exact-decimal TEXT (NFR-C1). Retention/source policy is specified here (architecture left it "to be specified"): **append-on-refresh, dedup by (ticker, date), keep all** (a study's confront window is its decision date → now; pruning is a later concern).

2. **AC2 — Confront mode overlays the recorded projection on the actual trajectory (FR50).** Reopening a saved study in **"confront" mode** overlays its **recorded §4 projection** — the forecast price band (forecast-high / forecast-low over `core::method::FORECAST_HORIZON_YEARS`, from the study's stored judgment) anchored at the decision (`study.created_at`) — on the **security's actual close trajectory since the decision**, read from the price-history cache for the study's ticker. The actual trajectory and the recorded band are drawn together (native Slint chart — Path/TouchArea, no web). When the cache has no post-decision closes for the ticker, confront shows a **neutral empty state** (no fake line), not an error.

3. **AC3 — The historical snapshot is unchanged by the comparison (read-only, FR50).** Confront mode is **strictly read-only**: it reads the stored `Study` and the price-history cache and renders the overlay; it **never** writes to `studies`/`judgments` or recomputes the SSG (the recorded projection is the one the user decided on, not a re-derivation). No `core::ssg` recompute, no judgment mutation, no `logical_version` bump from opening confront. Refreshing the price history (populating the cache) is the only write, and it touches **only** `price_history`.

4. **AC4 — `core::ssg` untouched; the overlay is presentation; provider-gated for live data.** The recorded projection band is read from the **already-persisted** judgment/outputs (no new method math); any projection geometry helper reused from `core::ssg::growth` is **read-only** and leaves the fingerprint/golden/determinism gates green. The price-history **fetch** reuses the Story-4.4 `ingestion` price path (no new provider surface). **No `contract` change, no new external dependency** (`Cargo.lock`/`deny.toml` unchanged). Migration v4→v5 only. Copy neutral, posture-gated. **⚠️ Live validation is gated on Epic-4 retro D2** (EODHD paid-plan vs alternate) — headless dev + GO/NO-GO use `FakeProvider`/fixtures; Guy's real-data confront waits on the provider decision.

## Tasks / Subtasks

- [x] **Task 1 — `persistence`: the price-history cache + migration v4→v5 (AC1, AC3)** — `persistence/src/{schema.rs, migrations.rs, price_history.rs}`
  - [x] `schema::migrate_to_v5` = `CREATE TABLE price_history (id TEXT PK, security_ticker TEXT NOT NULL, close_date TEXT NOT NULL, close TEXT NOT NULL, source TEXT NOT NULL, created_at TEXT NOT NULL)` + `CREATE UNIQUE INDEX idx_price_history_ticker_date ON price_history(security_ticker, close_date)` (dedup). Register `(5, migrate_to_v5)` in `REGISTRY`; shift the fake-future step v5→v6; forward-migration test for v5; bump `readonly_newer` `supported` 4→5; update the registry-doc comment.
  - [x] New `persistence/src/price_history.rs`: `upsert_closes(ticker, &[(date, close, source)])` (INSERT OR IGNORE on the unique index → idempotent append; bumps `logical_version` only when rows actually land — the C4 no-op guard) + `closes_since(ticker, since_date) -> Vec<(date, close)>` (ordered by date). Exact-decimal TEXT.
  - [x] Integration test: upsert closes (with a duplicate date → ignored), `closes_since` returns the ordered window; a re-upsert of identical rows bumps no version.

- [x] **Task 2 — App: populate the cache via the Story-4.4 refresh path (AC1, AC4)** — `app/src/{state.rs, main.rs}`
  - [x] On a holdings/study price refresh (the existing `ingestion` `/eod` path, Story 4.4), also persist the returned close(s) into `price_history` for the ticker (the refresh already fetches the latest close; specify whether to backfill a series or append the latest — **append the latest close per refresh** for v1, since the free `/eod` returns a series the adapter can map; a fuller backfill is a later refinement). Provider-gated: with `FakeProvider`/fixtures the series is deterministic for tests.
  - [x] Test: a fake refresh writes the close(s) into `price_history`; `closes_since(decision_date)` reflects them.

- [x] **Task 3 — App state: the confront read (AC2, AC3)** — `app/src/state.rs`
  - [x] `confront(&self, study_id) -> ConfrontView` (read-only): read the `Study`; derive the **recorded projection band** from its stored judgment/outputs (forecast-high/low over `FORECAST_HORIZON_YEARS`, anchored at `created_at`) — reuse a read-only `core::ssg::growth` helper, NO recompute of the verdict; read `price_history::closes_since(ticker, study.created_at)`; return the band geometry + the actual close series (+ an `available: bool` empty-state flag). Pure read — no journal write.
  - [x] Test: a study with cached post-decision closes yields a band + an actual series; a study with none yields `available = false`.

- [x] **Task 4 — main.rs + Slint: the confront overlay (AC2, AC3)** — `app/src/main.rs`, `app/ui/`
  - [x] A "confront" entry on a saved study (reopen mode) that pushes the `ConfrontView` into a native Slint chart (Path for the recorded band + the actual trajectory line; reuse the §1 growth-chart drawing primitives — no zones on this chart, UX-DR10). Neutral empty state when `available == false`. Read-only — opening confront writes nothing.
  - [x] Posture: any new `MSG_*` registered + count bumped; `@tr` floor bumped by the exact number of new literals.

- [x] **Task 5 — Gates (AC4)** — fmt, clippy `-D`, `test --workspace`, `deny` + smoke. Confirm `core::ssg` re-diffs clean (fingerprint/golden/determinism green); **no `contract` change, no new dep** (`Cargo.lock`/`deny.toml` unchanged); migration **v4→v5 only** (`user_version` latest = 5, `SCHEMA_VERSION` stays 1); `@tr` floor + `USER_FACING_MESSAGES` inventory bumped exactly. **Headless** GO via `FakeProvider`/fixtures; live GO/NO-GO deferred to the D2 provider decision.

## Dev Notes

### ⚠️ Gate (Epic-4 retro D2)
Confront's **value** is overlaying the recorded projection on *real* post-decision prices. Live data needs the provider decision (D2 — EODHD paid-plan vs alternate); the free plan's `/eod` close path (Story 4.4) does return closes, so the **happy path is validatable on the free plan for tickers the free tier serves** (e.g. AAPL.US / demo), but a full real-portfolio confront waits on D2. Dev + automated tests run **headless via `FakeProvider`/fixtures** — no live dependency to start the story.

### Scope
The confront overlay (recorded projection vs actual trajectory) + the price-history cache that feeds it. **Read-only** confront; the cache is the only write, populated via the existing Story-4.4 refresh.

### Out of scope (deferred)
- **Backfilling a long historical series** on first confront → v1 appends the latest close per refresh; a bulk historical backfill (one fetch → many dated closes) is a refinement.
- **Cache pruning/retention limits** → keep-all for now (AC1).
- **Whole-journal / export / PDF** → Stories 5.2 (done first), 5.3, 5.6.

### Architecture decisions this story honours
- [Source: architecture.md §799–800, §842.4] — FR50 needs a post-decision price-history cache, **sourced via an `ingestion` refresh and stored in `persistence`, overlaid by `app`**; source/retention "to be specified" — specified here (append-on-refresh, dedup by ticker+date, keep-all).
- [Source: project memory — GUI = Slint-only] — the overlay is a **native Slint chart** (Path/TouchArea), no web/Tauri; reuse the §1 growth-chart primitives.
- [Source: project memory — risk overlay / method decoupling] — confront reads the **recorded** projection; it does **not** recompute the SSG, so the method fingerprint/golden/determinism gates are untouched.

### Where things live
- **`persistence/src/price_history.rs`** (new) + **migration v4→v5** (`schema.rs`/`migrations.rs`) — the cache + its CRUD.
- **`app/src/state.rs`** — `confront` (read-only view) + the cache-population hook on refresh.
- **`app/src/main.rs` + `app/ui/`** — the confront chart overlay (native Slint).
- **`core::ssg::growth`** — read-only reuse of projection geometry only; no new method math.

### Notes & guardrails
- **Read-only confront (AC3)** — opening confront must bump no `logical_version` and write nothing to `studies`/`judgments`. Only the price refresh writes (to `price_history`). Assert this in a test (undo-depth / version unchanged).
- **Migration v4→v5** — `price_history` is a NEW table (not pre-provisioned in v1, unlike the Epic-4 tables). The harness handles it; shift the fake-future test step v5→v6 (the pattern from 4.1/4.5/4.7).
- **Idempotency (C4)** — `upsert_closes` uses INSERT OR IGNORE on the `(ticker, date)` unique index and bumps the version only when rows land (no phantom revision on a re-refresh of the same day).
- **Exact decimal (NFR-C1)** — closes stored as canonical TEXT, never REAL.

### Manual on-display GO/NO-GO (Guy) — ⚠️ live part gated on D2
With fixtures/demo (AAPL.US): open a saved study in confront → the recorded forecast band overlays the actual close line since the decision date; a study with no cached closes shows the neutral empty state; opening confront writes nothing (version unchanged); a price refresh adds the latest close and the line extends. The **real-portfolio** confront (Guy's tickers) waits on the D2 provider decision.

### Project Structure Notes
- New `persistence::price_history` + migration v4→v5; app confront read + Slint overlay; read-only reuse of `core::ssg::growth`. No `contract` change, no `SCHEMA_VERSION` bump, no new external dependency.
- Posture floors at story start (after 5.2): re-measure the `@tr` floor + `USER_FACING_MESSAGES` inventory (5.2 bumps them first) and bump by exactly the number this story adds.
- Migration axis: `user_version` latest = 4 (Story 4.7) at Epic-5 start → 5.1 makes it 5.

### References
- [Source: _bmad-output/planning-artifacts/epics.md#Story 5.1] — AC: recorded projection overlaid on the actual trajectory since the decision (FR50, ADD13); historical snapshot unchanged (read-only).
- [Source: architecture.md §799–800, §842.4] — the price-history cache (ingestion-sourced, persistence-stored, app-overlaid); source/retention to be specified.
- [Source: contract/src/study.rs] — the stored judgment + `forecast_low_option`; `core::method::FORECAST_HORIZON_YEARS` for the band horizon.
- [Source: 4-4-manual-price-refresh-per-holding-zones.md] — the existing `/eod` close fetch path the cache reuses (free-plan-safe).
- [Source: project memory — provider gate] — D2 (EODHD paid-plan vs alternate) gates live confront; headless via FakeProvider.

## Dev Agent Record

### Agent Model Used
Claude Opus 4.8 (1M context).

### Debug Log References
Gates green: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -D warnings`, `cargo test --workspace --locked` (**587 tests**), `cargo deny check` (advisories/bans/licenses/sources ok), smoke `timeout cargo run -p steadyinvest-app` (exit 124).

### Completion Notes List
- **AC1 — price-history cache + migration v4→v5.** `persistence/src/price_history.rs`: `upsert_closes` (`INSERT OR IGNORE`, dedup by `(security_ticker, close_date)` via a unique index, deterministic `id = "{ticker}:{date}"` — no injected UUID, ADD15) + `closes_since(ticker, since_date)` (ordered window, lexical `>=` on `YYYY-MM-DD` = chronological). `schema::migrate_to_v5` creates the table; `REGISTRY` gains `(5, migrate_to_v5)`; the fake-future test shifted v5→v6; `readonly_newer` `supported` 4→5. Close is exact-decimal **TEXT** (NFR-C1). `SCHEMA_VERSION` stays 1.
- **AC2 — confront overlay.** `state::confront(study_id) -> ConfrontView` (read-only): band re-derived deterministically from the **frozen stored judgment** via `engine::build_snapshot` (the only faithful path — the `Study` blob persists inputs only, not outputs; the SSG engine is pure so the decision-time band reproduces bit-for-bit and is invariant to `current_price`), actual trajectory from `closes_since`. Native Slint overlay (`app/ui/components/confront_overlay.slint`): a faint neutral band rectangle under the bright actual-trajectory line (NO zone hues — UX-DR10), wired via a `Confront` global + a "Confronter" row action. Neutral empty state when `available == false`.
- **AC3 — strictly read-only.** `confront()` writes nothing and bumps no `logical_version` (test `confront_does_not_bump_the_version_read_only`). The cache write (`upsert_closes`) touches **only** `price_history` — see the review fix below.
- **AC4 — guardrails.** `git diff main` on `core/`, `contract/`, `Cargo.lock`, `deny.toml` is **empty** (no core/contract change, no new dependency). Migration v4→v5 only. 8 new FR13-neutral `@tr` strings; posture `@tr` floor 306→314; no new `MSG_*` (confront surfaces no notices, inventory stays 75).
- **3-layer adversarial review (Blind Hunter / Edge Case Hunter / Acceptance Auditor) — 4 patches, 1 defer:**
  - **MED (patch, both finders):** a single cached close rendered an invisible bare `MoveTo`. `confront_chart` now draws a visible marker box for the `n == 1` case (the common single-refresh case). Locked by `confront_chart_single_close_draws_a_visible_marker_not_a_bare_move`.
  - **MED→design (patch, Auditor AC3-PARTIAL):** `upsert_closes` no longer bumps `logical_version`. The cache is local, reconstructible, and **excluded from the export `JournalSnapshot`**; bumping the identity counter on a price refresh would desync version-from-exported-content. It is now the only writer that doesn't bump — making AC3 ("touches only price_history") literally true. Test rewritten: `caching_closes_never_bumps_the_journal_identity_version`.
  - **LOW (patch):** `confront()` doc clarified (deterministic rebuild from the frozen study, not a re-decision).
  - **LOW (patch):** posture floor bumped + Story 5.1 narrative line (gate kept strict).
  - **MED deferred → issue #72:** closes are keyed by the **refresh date**, not the provider's real EOD session date (and same-day is first-wins). Fixing requires expanding the `MarketDataProvider` surface, which AC4 forbids — documented as a known limitation in `cache_close`; the x-axis is ordinal for the MVP.

### File List
- `persistence/src/price_history.rs` (NEW) — the cache (`upsert_closes` / `closes_since`).
- `persistence/src/schema.rs` — `migrate_to_v5` + table-count / naming-exception tests.
- `persistence/src/migrations.rs` — `REGISTRY (5, …)`; fake-future v5→v6; v5 assertions.
- `persistence/src/lib.rs` — `mod price_history;`.
- `persistence/tests/readonly_newer.rs` — `supported: 5`.
- `app/src/state.rs` — `ConfrontView`, `confront()`, `cache_close()` (+ hooks in `apply_holding_price` / `apply_provider_refresh`).
- `app/src/viewmodel/chart.rs` — `confront_chart()` (+ single-close marker) + confront tests.
- `app/src/main.rs` — `Confront::on_request` / `on_dismiss` wiring.
- `app/src/posture.rs` — `@tr` floor 306→314.
- `app/ui/state.slint` — `ConfrontState` struct + `Confront` global.
- `app/ui/components/confront_overlay.slint` (NEW) — the native overlay.
- `app/ui/app.slint` — mount + re-export of the overlay/global.
- `app/ui/screens/dashboard.slint` — the "Confronter" row action.

### Change Log
- 2026-06-30 — Story 5.1 implemented (confront: recorded §4 band vs actual close trajectory; `price_history` cache, migration v4→v5). 3-layer review: 4 patches applied, 1 defer (#72). 587 tests; all gates green. Status → done.

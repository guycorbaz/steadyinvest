# Story 4.4: Manual price refresh & per-holding zones

Status: review (3-layer ACCEPT-WITH-FINDINGS 2026-06-28; 4 patches + all 3 follow-ups #50/#51/#52 resolved on-branch; awaiting Guy's on-display GO/NO-GO + merge of PR #53)

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to refresh my holdings' prices on purpose and see each one's zone and freshness,
so that I read my portfolio against my own studies on data I refreshed deliberately — without being told what to do.

## Acceptance Criteria

1. **AC1 — Holding ↔ study auto-match by ticker (no migration).** Each holding auto-matches the most-recent saved study of the **same ticker** (case-insensitive, `state::study_id_for_ticker` — the watchlist pattern). A holding with no matching study shows a neutral "no linked study" state, never an error. No `holdings.study_id` column, no schema migration.
2. **AC2 — Manual price refresh fills the current price from the latest close (FR40).** A user-initiated "refresh prices" action on the Portefeuille surface fetches, per held ticker, the provider's **latest `/eod` close** and sets it as the matched study's `judgment.current_price`; the §4 zone then recomputes. Fetch is **off the UI thread** (the Epic-3 worker), one job per unique held ticker, never blocking (NFR-P3). Refresh is **only** ever user-initiated (FR65 — no background polling).
3. **AC3 — Each holding shows its zone + freshness/timestamp.** After a refresh, each linked holding displays its recomputed zone (Achat / Neutre / Vente, or "—" when the current price is outside the forecast band) + a freshness state (à jour / périmé) + the as-of timestamp of its last successful refresh. The zone marker is **neutral** (ink + glyph, the §4 zone hues stay geofenced to the open study's zone bar — Story 4.2 rule).
4. **AC4 — Provider failure degrades to stale, never a silent wrong zone (FR40 + 3.5).** If a holding's fetch fails (network / quota / key / no-data), that holding is flagged **périmé** with a neutral cause notice and **keeps its last-known zone visibly marked stale** — it never shows a fresh-looking wrong zone. A later successful refresh clears the stale flag.
5. **AC5 — `core` untouched; the latest price rides ingestion, not the canonical calc.** The latest price is a **present market fact** for the zone marker, not an SSG calc input — it travels in an **ingestion-owned** type (a new `RawFetch` wrapper + `FetchedFinancials.latest_price`), never added to `core`'s `RawFinancials`/`CanonicalFinancials`. `core`'s method fingerprint / golden / determinism gates stay green; no method change.
6. **AC6 — FR33-safe, neutral, no new persisted entity.** Auto-filling `current_price` from the latest close fills a *market fact*, not a *judgment* the user owns (the forecast high/low EPS + P/E) — so FR33 ("never auto-place/suggest a judgment line") is not violated. All copy is fact-stating, no banned verb (FR13). Per-holding zone/freshness/timestamp are **display-time** (transient app state), **not** persisted — no schema change beyond AC5's ingestion types.

## Tasks / Subtasks

- [x] **Task 1 — Ingestion: carry the latest `/eod` close out of the adapter (AC2, AC5)** — `core` stays untouched
  - [x] New ingestion type `RawFetch { pub financials: RawFinancials, pub latest_price: Option<Decimal> }` in `ingestion/src/provider.rs` (or a small `raw_fetch.rs`); `RawFinancials` stays the `core` type, carried as a field
  - [x] Change the trait: `MarketDataProvider::fetch_fundamentals(...) -> Result<RawFetch, ProviderError>` (was `Result<RawFinancials, _>`). Update the `Provider` enum dispatch (`ingestion/src/fetch.rs`) to return `RawFetch`
  - [x] `EodhdProvider` (`ingestion/src/adapters/eodhd.rs`): the `/eod` daily-bar response is already fetched (ascending order); extract the **last bar's adjusted close** (or close) as `latest_price` (a `Decimal` via the existing exact-decimal parse), `None` if the series is empty / unparsable. Keep the existing yearly high/low reduction unchanged. The pure `map_eodhd*` helper should surface the latest close alongside `RawFinancials`
  - [x] `FakeProvider` (`ingestion/src/fetch.rs`): a settable `latest_price` so app tests drive the zone deterministically (mirror how it already carries a canned result)
  - [x] `fetch_canonical`: destructure `RawFetch { financials, latest_price }`, `normalize(financials)` as today, thread `latest_price` into the result. Add `pub latest_price: Option<Decimal>` to `FetchedFinancials`. The dependency digest is unchanged (latest price is not a canonical calc input — do NOT fold it into the digest)
  - [x] Tests: an ingestion fixture test that the EODHD `/eod` last close maps to `latest_price`; a `FetchedFinancials.latest_price` round-trip. Keep all existing ingestion tests green (their `latest_price` defaults to `None` → no behaviour change)
- [x] **Task 2 — App: set `current_price` from the latest close on refresh (AC2, AC5, AC6)** — `app/src/state.rs`
  - [x] In `apply_provider_refresh` (or a thin sibling), when `fetched.latest_price` is `Some`, set the study's `judgment.current_price` to it **as part of the same mutation** (so the zone recomputes and it's one undo step). Guard: `latest_price == None` → unchanged (existing study-screen fetches with a `None` latest price keep today's behaviour — additive, existing tests stay green). This is a *market fact* fill, not a judgment (AC6/FR33)
  - [x] Confirm the recompute path: setting `current_price` flows through the existing snapshot build so `present_price_zone` updates (no new calc — reuse `engine::build_snapshot` / the existing zone read)
  - [x] Test: a provider refresh carrying a `latest_price` sets `current_price` and moves `present_price_zone` (e.g. into Buy), verdict-independent; a refresh with `latest_price = None` leaves `current_price` untouched
- [x] **Task 3 — App: per-holding refresh orchestration + transient freshness (AC1–AC4)** — `app/src/state.rs` + `app/src/main.rs` + `app/src/fetch.rs`
  - [x] Auto-match: for each holding, `state::study_id_for_ticker(&holding.security_ticker)` → the linked study id (or `None`)
  - [x] A **"refresh prices"** intent on the Portefeuille surface: collect the **unique** linked tickers, enqueue one `WorkerJob::Fetch` per ticker (reuse the existing worker; key from the keychain/env as the study fetch does). Holdings with no linked study are skipped (shown as "no study"). Never blocks the UI (NFR-P3)
  - [x] **Transient per-ticker refresh state** (NOT persisted): a map `ticker → { stale: bool, as_of: Option<Timestamp> }` held in app state (or a `Rc<RefCell<…>>` in main.rs), populated by fetch outcomes — success → `{ stale:false, as_of: now }` + `apply_provider_refresh` (which sets current_price); failure → `{ stale:true }` + a neutral cause notice (reuse `state::provider_failure_notice`), last-known zone retained (AC4)
  - [x] `refresh_holdings` builds each `HoldingRow` with: the linked-study zone (read `present_price_zone` via the existing engine read — Achat/Neutre/Vente/"—"), `linked` bool, `current_price` display, `stale` bool + `as_of` timestamp from the transient map. A holding whose study is missing → `linked:false`, no zone
  - [x] Reuse the Story 4.2 buy-zone read shape; surface the full zone (not just Buy) — add an `engine` helper `study_zone(&Study) -> Option<Zone>` (or reuse the snapshot read) so the holding shows Achat/Neutre/Vente
- [x] **Task 4 — Slint: per-holding zone + freshness on the Portefeuille screen (AC3, AC4, AC6)** — `app/ui/state.slint` + `app/ui/screens/portfolio.slint`
  - [x] `HoldingRow` gains: `linked: bool`, `study-link: string` (the matched study's ticker/label, or ""), `zone: string` (already-localized "Achat"/"Neutre"/"Vente"/"—" — or a neutral key the screen maps), `current-price: string`, `stale: bool`, `as-of: string`. The `Holdings` global gains a `refresh-prices()` callback + a `read-only`/notice already present
  - [x] The screen: a "Rafraîchir les prix" `ActionButton` (disabled when read-only or no linked holding); per row, show the zone marker (neutral ink + glyph, **never a buy/hold/sell hue** — geofenced rule), the current price labelled with `Holdings.reference-currency`, and the freshness (`◦`-style stale murmur + "à jour le {}" / "périmé" — reuse the Story 3.3/3.5 vocabulary). Glyphs INSIDE `@tr` (leak-gate). A holding with no linked study shows a neutral "aucune étude liée" hint (already a 4.x idiom)
  - [x] Keep the existing 4.3 add/edit/remove register intact; the zone/freshness columns are additive
- [x] **Task 5 — Posture floors + gates (AC6)** — `app/src/posture.rs`
  - [x] Register any new `MSG_*` (e.g. a holdings-refresh no-link / done notice if added) in `USER_FACING_MESSAGES` and bump the exact count; bump the `@tr` floor by the exact number of new literals (probe empirically as in 4.3)
  - [x] Run all 4 gates `--locked` (fmt, clippy -D warnings, test --workspace, deny) + smoke launch (exit 124). Confirm **`core` re-diffs empty** (method fingerprint / golden / determinism green); `Cargo.lock`/`deny.toml` unchanged (no new dep — reqwest/serde already present)

### Review Findings

3-layer adversarial review (Blind Hunter / Edge Case Hunter / Acceptance Auditor), 2026-06-28. Acceptance verdict: **ACCEPT-WITH-FINDINGS** — AC1 ✅ · AC2 ✅ (the free-plan gap was the HIGH finding; **now fixed on this branch** — see issue #50 below) · AC3 ✅ · AC4 ✅ · AC5 ✅ · AC6 ✅. No CRITICAL/REJECT. 4 patches applied; **all 3 deferred findings also resolved on-branch (#50/#51/#52)**; 2 dismissed. No open follow-up issues remain for 4.4.

**Resolved on this branch (post-review, follow-up #50):** the holdings refresh now uses a **price-only `/eod` fetch** (`MarketDataProvider::fetch_latest_price` + `fetch::fetch_price`, no `/fundamentals`) and a focused `state::apply_holding_price` (sets ONLY `current_price`, never the yearly cells). This makes the refresh work on the **free EODHD tier** (EOD allowed, fundamentals 403) and also resolves the dismissed "holding refresh touches fundamentals cells" finding. +2 tests (`ingestion::fetch::fetch_price`, `state::apply_holding_price`). Workspace 492 tests green.

**Patches (applied this review):**
- [x] [Review][Patch] Holding freshness keyed off the price, not fundamentals years — a refresh with no `latest_price` (empty `/eod` / fundamentals-only) no longer stamps "à jour"; a valid `/eod` close with empty fundamentals is no longer discarded `[app/src/main.rs HoldingFetch arm]`
- [x] [Review][Patch] A failed `apply_provider_refresh` (study deleted / read-only mid-flight) now also flags the ticker `périmé` — symmetric with the other failure arms `[app/src/main.rs]`
- [x] [Review][Patch] A sibling ticker's success no longer clobbers another ticker's failure notice — success clears only the in-progress banner `[app/src/main.rs]`
- [x] [Review][Patch] `FakeProvider::returning_with_price` was dead code + Task 1's `latest_price` round-trip test was missing — added a `fetch_canonical` round-trip test (also re-asserts digest exclusion) `[ingestion/src/fetch.rs]`

**Deferred (tracked as GitHub issues, not patched):**
- [x] [Review][Resolved] Holdings refresh didn't work on the FREE EODHD plan (reused the coupled fundamentals+eod fetch which 403s on `/fundamentals`) → **FIXED on this branch** via the price-only `/eod` path (issue **#50** — closed)
- [x] [Review][Resolved] Freshness map never pruned on holding delete / ticker edit → **FIXED on-branch** via `retain_held_freshness` (after every holdings mutation, retain only entries for currently-held tickers — a removed ticker's entry is pruned, so a re-add starts clean; a sibling holding keeps a shared ticker's entry). Issue **#51** — closed.
- [x] [Review][Resolved] "Rafraîchir les prix" button not disabled while a refresh is in flight → **FIXED on-branch** via `Holdings.refreshing` + a `refresh_pending` job counter (button disabled + label "Rafraîchissement…" while in flight; cleared when the last outcome drains; worker-gone doesn't latch). Issue **#52** — closed. (The negligible stale-`study_id` mid-flight race was not changed — self-correcting, accepted.)

**Dismissed:** the EODHD last-bar ordering assumption (verified safe — `eod_url` pins `&order=a`). (The "holding refresh touches fundamentals cells" observation was dismissed as spec-directed, then resolved anyway by the #50 price-only path above.)

## Dev Notes

### Scope decision (the "current-price paradox" and its fix — read first)

Before 4.4, a study's §4 zone is computed from `judgment.current_price`, which was a **manual** input, and the Epic-3 provider refresh updated only yearly high/low + fundamentals — **never** `current_price`. So a "price refresh" could not recompute the zone. **Story 4.4 closes that gap** by sourcing the current price from the provider's **latest `/eod` close** (decision A, 2026-06-27 — see memory `project_holding_price_refresh`). This works on Guy's **free EODHD plan** (`/eod` is allowed; only `/fundamentals` 403s), so 4.4 delivers real value without the paid-plan decision (retro C2 stays deferred; dev/test use the demo key / `FakeProvider`, the live GO/NO-GO is Guy's residual).

- **In scope:** auto-match holding→study by ticker; a user-initiated per-holding price refresh that fills `current_price` from the latest close and recomputes each zone; per-holding zone + freshness/timestamp; stale-on-failure.
- **Out of scope (deferred):** trailing stop (FR42 → Story 4.5; the `trailing_stop_pct` column stays NULL); capital-at-risk (FR43 → 4.6); neutral sell/raise-stop triggers (4.7); multi-portfolio/FX (Epic 6); a paid-plan fundamentals refresh of the *holdings'* studies (the study-screen fetch already does fundamentals; 4.4's holding refresh is price-led).

### `core` stays untouched — the latest price rides ingestion (the key constraint)

`RawFinancials` and `CanonicalFinancials` are **`core`** types. To keep `core`'s frozen method fingerprint / golden / determinism green, the latest price must **not** enter the canonical calc model. It travels in **ingestion-owned** types:

- `MarketDataProvider::fetch_fundamentals -> Result<RawFetch, ProviderError>` where `RawFetch { financials: RawFinancials, latest_price: Option<Decimal> }` (new, ingestion). `RawFinancials` is still produced by the adapter and consumed by `normalize` unchanged.
- `FetchedFinancials` (ingestion) gains `latest_price: Option<Decimal>`.
- The app reads `fetched.latest_price` and writes it into `judgment.current_price` (a `contract::Judgment` field that already exists). No `core` edit, no schema change. [Source: ingestion/src/provider.rs, ingestion/src/fetch.rs; core/src/normalize/types.rs]

### What this story changes vs. preserves (files marked UPDATE — read before editing)

- **`ingestion/src/provider.rs` (UPDATE):** the `MarketDataProvider` trait. Change the return type to `RawFetch`; add the `RawFetch` type. **Preserve** the keyless `Option<&str>` contract and the "adapter normalizes nothing" boundary.
- **`ingestion/src/adapters/eodhd.rs` (UPDATE):** already fetches `/fundamentals` + `/eod` and reduces `/eod` to yearly high/low (read the existing `/eod` parse). **Add** the last-bar close extraction. **Preserve** the yearly reduction + the existing field mapping (the #1 live-shape risk — don't disturb it).
- **`ingestion/src/fetch.rs` (UPDATE):** `Provider` enum dispatch + `fetch_canonical` + `FetchedFinancials`. Thread `latest_price`. **Preserve** the digest (do not hash the latest price).
- **`app/src/state.rs` (UPDATE):** `apply_provider_refresh` (sets cells, never current_price today — state.rs ~975) + `study_id_for_ticker` (the auto-match, ~648) + the holdings rail (4.3). **Add** the current_price-from-latest-close fill + the per-holding refresh orchestration + transient freshness. **Preserve** every existing refresh guard, the idempotency no-ops (C4), and the 4.3 holdings CRUD.
- **`app/src/main.rs` (UPDATE):** `refresh_holdings` (4.3, ~194), the fetch worker wiring + outcome handler (~534), the `on_fetch_provider` study-fetch (~621). **Add** the holdings "refresh prices" callback (batch enqueue) + per-ticker outcome routing into the transient freshness map. **Preserve** the study-screen fetch path and the IdGen/Clock injection (ADD15).
- **`app/src/fetch.rs` (UPDATE):** `WorkerJob`/`FetchRequest`/outcome (one job per ticker). The existing `FetchRequest { study_id, ticker, api_key }` already suffices — a holding refresh enqueues one per matched (study_id, ticker). **Preserve** the off-thread worker + `invoke_from_event_loop` marshalling.
- **`app/ui/screens/portfolio.slint` + `app/ui/state.slint` (UPDATE):** the 4.3 register. **Add** the zone/freshness columns + the refresh button. **Preserve** the add/edit/remove register + the neutral, no-zone-hue posture.

### Architecture & constraints

- **Off-thread, never blocking (NFR-P3, FR65).** Reuse the Epic-3 worker (`app/src/fetch.rs::spawn_fetch_worker`); one `WorkerJob::Fetch` per unique linked ticker; outcomes marshal back via `invoke_from_event_loop` to the UI-thread handler. A "tens of holdings" refresh completes in a few seconds and never freezes the UI. Refresh is **only** user-initiated. [Source: app/src/fetch.rs; app/src/main.rs outcome handler]
- **Stale-on-failure, never a silent wrong zone (AC4, FR40 + Story 3.5).** A failed fetch flags the holding `périmé` (transient) + a neutral cause notice via `state::provider_failure_notice` (the static, key-free messages — NFR-S1). The last-known zone stays visibly marked stale; a later success clears it. Mirror the 3.5 recovery discipline. [Source: app/src/state.rs `provider_failure_notice` / `mark_provider_stale`]
- **Zone hues geofenced (Story 4.2 / colour budget).** The per-holding zone marker is **neutral ink + glyph**, never a saturated buy/hold/sell hue (those stay on the open study's §4 zone bar). [Source: memory colour-budget; app/ui/screens/watchlist.slint 4.2 precedent]
- **FR33-safe.** `current_price` is the present *market* price (a fact), not the *judgment* (forecast high/low EPS + P/E, which the user owns and the system never auto-places). Filling it from the latest close does not auto-place a judgment line. Keep the forecast inputs strictly manual. [Source: epics.md#FR33]
- **No new persisted entity / no migration.** Per-holding zone/freshness/timestamp are display-time (transient app state). The only new persisted data path is `judgment.current_price` (an existing `contract::Judgment` field, written via the existing mutate rail). REGISTRY stays at v2. [Source: persistence/src/migrations.rs]
- **App + ingestion only; `core`/`contract`/`Cargo.lock`/`deny.toml` re-diff empty.** No new dependency. Method fingerprint / golden / serde corpus green. [Source: epics.md#Epic 4]

### Previous-story intelligence (4.1/4.2/4.3 + Epic-3 retro)

- **Auto-match is proven:** `study_id_for_ticker` (case-insensitive, most-recent) is exactly what the watchlist uses (`link_watch_to_same_ticker_study`, main.rs). Holdings reuse it verbatim — no link UI, no `study_id` column.
- **The zone read is proven:** Story 4.2's `engine::study_in_buy_zone` reads `snapshot.outputs().risk_reward.present_price_zone`. 4.4 generalizes it to the full `Zone` (Achat/Neutre/Vente) — add `engine::study_zone(&Study) -> Option<Zone>` beside it (same `build_snapshot` read). `present_zone` returns `None` when the price is outside `[forecast_low, forecast_high]` → show "—" (the Story 4.2 #48 below-band behaviour; consistent, tracked).
- **The refresh rail is proven:** `apply_provider_refresh` + the worker + the outcome handler already exist (3.3–3.6). 4.4 adds the current_price fill + the batch (per-holding) enqueue. The idempotency lesson (C4) still applies — setting current_price to the same value must not churn the journal (the mutate rail's `before != study` guard already covers this; a re-fetch returning the same close is a no-op).
- **Stale rail is proven:** `provider_failure_notice` classifies the cause to a neutral, key-free banner (3.5). Reuse it per holding.
- **Posture floors are exact:** bump the `@tr` floor + message inventory to the precise new totals (probe empirically — 4.3 set @tr ≥254, messages 44).

### Testing standards

- **Ingestion:** a fixture test that the EODHD `/eod` last close → `latest_price`; an empty `/eod` → `None`; `FetchedFinancials.latest_price` carried. Existing ingestion/mapping tests stay green (default `None`).
- **App state:** `apply_provider_refresh` with a `latest_price` sets `current_price` + moves `present_price_zone` (use the 4.2 test's band: est_high_eps 8 / est_low_eps 6 / high_pe 20 / low_pe 10 → buy third ≈ [low, 93]; a latest close of 70 → Achat, 150 → Vente); `latest_price = None` leaves `current_price` untouched. A holding auto-matches by ticker; an unmatched holding has no zone. Stale-on-failure flips the transient flag and keeps the last zone. Use `SeqIdGen`/`FixedClock` doubles + `FakeProvider` with a set latest price.
- **Gates:** all 4 `--locked` + smoke launch 124; `core`/`contract` test counts unchanged; ingestion + app counts rise.

### Open questions for dev (resolve during implementation, don't block)

- **Adjusted vs raw close:** prefer the EODHD `/eod` **adjusted_close** if present (split-consistent with the yearly high/low reduction), else `close`. Match whatever the existing yearly reduction uses for consistency.
- **Batch dedup:** multiple holdings of the same ticker share one fetch + one zone (rare); dedup tickers before enqueuing.
- **Freshness granularity:** a per-ticker transient `as_of` is enough; no need for a per-cell freshness on the holding. Keep it display-time.
- **"Refresh prices" with no linked holdings:** disable the button (nothing to fetch) — a neutral no-op, consistent with the 4.x button-gating idiom.

### Project Structure Notes

- New ingestion type `RawFetch` (provider.rs). Modified: `ingestion/src/{provider,fetch,adapters/eodhd}.rs`, `app/src/{state,main,fetch,posture}.rs`, `app/ui/state.slint`, `app/ui/screens/portfolio.slint`. No `core`/`contract` file. No `Cargo.toml`/`Cargo.lock`/`deny.toml` change.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.4] — holdings linked to studies, manual price refresh recomputes each zone + freshness; failure → stale, never a silent wrong zone
- [Source: _bmad-output/planning-artifacts/epics.md#FR40] — manual price refresh recomputing each holding's zone, showing freshness
- [Source: _bmad-output/planning-artifacts/epics.md#FR33] — never auto-place/suggest a judgment line (current_price is a market fact, not a judgment)
- [Source: _bmad-output/planning-artifacts/epics.md#FR65] — offline-first; the only online action is a user-initiated refresh
- [Source: _bmad-output/planning-artifacts/epics.md#NFR-P3] — a manual portfolio refresh (tens of holdings) completes in a few seconds, never blocks the UI
- [Source: ingestion/src/provider.rs / fetch.rs] — the trait + `fetch_canonical` + `FetchedFinancials` to extend (RawFetch + latest_price)
- [Source: ingestion/src/adapters/eodhd.rs] — the `/eod` daily-bar parse to extract the last close
- [Source: core/src/normalize/types.rs] — `RawFinancials`/`CanonicalFinancials` are core; do NOT add latest_price here
- [Source: app/src/state.rs `apply_provider_refresh` ~975, `study_id_for_ticker` ~648] — the refresh rail + the auto-match
- [Source: app/src/viewmodel/engine.rs `study_in_buy_zone` ~249] — the zone read to generalize to `study_zone`
- [Source: app/src/state.rs `provider_failure_notice` / `mark_provider_stale`] — the 3.5 stale-on-failure rail to reuse per holding
- [Source: app/src/fetch.rs + main.rs outcome handler] — the off-thread worker + `invoke_from_event_loop` marshalling
- Product decisions (memory): `project_holding_price_refresh` (latest /eod close → current_price, core untouched) + `project_reference_currency` (4.3, the amount labelling).

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List

### Change Log

- 2026-06-27 — Story 4.4 created → ready-for-dev. Scope (decision A): the per-holding current price comes from the latest `/eod` close (free-plan-friendly), threaded ingestion→app→`judgment.current_price`, **core untouched** (the latest price rides a new ingestion `RawFetch` + `FetchedFinancials.latest_price`, never the canonical calc). Auto-match holding→study by ticker (no migration); user-initiated off-thread batch refresh; per-holding zone + freshness/timestamp (transient, display-time); stale-on-failure. Defers trailing-stop/CAR/triggers (4.5/4.6/4.7) and the EODHD paid-plan decision (test with demo/FakeProvider). FR33-safe (current_price is a market fact, not a judgment).

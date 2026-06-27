# Story 3.5: Graceful provider failure

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want outages to degrade visibly, never into a wrong signal,
so that I keep working offline and know why a refresh failed.

## Acceptance Criteria

(From epics.md §Story 3.5, lines 806–817. FR23, FR24, NFR-R1. Closes the #46 empty-payload follow-up from the Story-3.3 review. Scope-resolved in Dev Notes.)

1. **AC1 — A failed refresh reports its CAUSE via the neutral global notice (FR24).** When a refresh fails, the cause is **classified from the existing `ingestion::ProviderError`/`IngestionError` taxonomy** (built in 3.1/3.2) into a **specific, neutral, fact-stating** notice — no generic "failed: {raw error}" passthrough:
   - **network / transport** (`ProviderError::Network`) → an "offline / connection failed, last-known values shown" notice;
   - **quota / rate-limit** (`ProviderError::Quota`) → a "usage limit reached, retry later" notice;
   - **invalid / absent key** (`ProviderError::InvalidOrAbsentKey`, 401) → the existing `MSG_KEY_INVALID`;
   - **plan / access** (`ProviderError::Forbidden`, 403) → the existing `MSG_KEY_FORBIDDEN`;
   - **no data for ticker** (`ProviderError::TickerNotFound`, 404/empty) → a "provider returned no data" notice;
   - **parse / unsupported / normalize** → the generic `MSG_PROVIDER_FAILED` / `MSG_NORMALIZE_FAILED` fallback (a malformed payload is not an outage). The api_token never appears in any notice (NFR-S1 — the 3.2 `without_url()` fix already strips it; do not regress).

2. **AC2 — Last-known values are RETAINED; affected data is flagged stale/to-update (FR23, NFR-R1).** A failed refresh **never clears or overwrites** any value. On a populated study, the **provider-sourced cells** (`Source::Provider`) of the open study are flagged **`Freshness::Stale`** — the *production trigger* of the stale state that Stories 2.4/3.3 built the display + verdict-degradation for but never fired in prod. Manual cells (`Source::Manual`) stay `Current` (the user owns them; they are not provider-refreshed). The stale flag **persists** (survives reopen) until a later successful refresh re-stamps the cells `Current`.

3. **AC3 — A stale validated input degrades the verdict in the same frame (honest degradation).** Because `engine::cell_to_gate_state` already maps `(Review::Validated, Freshness::Stale) → GateState::Stale → Verdict::Provisional` (Epic-2 wiring, proven by `seam_check.rs` SEAM 3), flagging a validated load-bearing provider cell stale **degrades the verdict to Provisional in the same `build_frame`** — never a full-colour verdict over data the app could not refresh. The stale **murmur** (the dimmed `◦`, surfaced by `form::editable_cell`) renders on every stale cell regardless of review, so Guy SEES exactly which data is stale. **No new display or engine code** — only the production flag-setter.

4. **AC4 — Guy can continue offline, override by hand, and retry (NFR-R1).** A failed refresh is **non-blocking**: the study stays fully editable, the journal stays writable, and Guy can edit a stale cell by hand (a manual edit re-stamps it `Current` via `Cell::edited`, clearing both the stale flag and — Story 3.4 — any pending) or trigger the refresh again later. The only online action remains the user-initiated refresh (FR65 — no auto-retry, no background polling).

5. **AC5 — An empty / no-data successful payload is reported honestly, not as "no change" (closes #46).** A refresh that succeeds at the transport layer but returns **zero usable years** (the Story-3.3 review gap) is reported with the **no-data** notice (AC1), **not** `MSG_REFRESH_NOCHANGE` ("already up to date"). A genuine no-op (data already current) and a degenerate empty payload are now distinguishable. On a populated study an empty payload also flags the provider cells stale (AC2); on a fresh/empty study it is the notice only (nothing to flag).

6. **AC6 — App-crate-only; no method/calc/contract change.** `ProviderError`/`IngestionError` (ingestion) and `Freshness`/`Cell` (contract) already exist — this story only **classifies** the existing taxonomy and **sets** the existing `Freshness::Stale` from the app. `core`/`contract`/`persistence`/`ingestion` **SRC untouched**; no `SCHEMA_VERSION`/`method_version` change; method fingerprint / determinism / golden / frozen `v1.db` corpus re-diff clean. `Cargo.lock`/`deny.toml` unchanged (no new dep).

## Tasks / Subtasks

- [x] **Task 1 — Cause-named failure notices (AC1, AC5)**
  - [x] `MSG_PROVIDER_OFFLINE` / `MSG_PROVIDER_QUOTA` / `MSG_PROVIDER_NO_DATA` added to `state.rs` + registered in `USER_FACING_MESSAGES`. Banned-verb-clean (posture gate green).
  - [x] `state::provider_failure_notice(&IngestionError) -> &'static str` maps each variant: Network→OFFLINE, Quota→QUOTA, InvalidOrAbsentKey→`MSG_KEY_INVALID`, Forbidden→`MSG_KEY_FORBIDDEN`, TickerNotFound→NO_DATA, **Parse/Unsupported/Normalize→`MSG_NORMALIZE_FAILED`** (a static notice — NOT `MSG_PROVIDER_FAILED` which carries the `{cause}` placeholder the worker-gone path fills; this keeps the api_token out by construction, NFR-S1).
  - [x] Unit test `provider_failure_notice_maps_each_cause` covers every variant + `Normalize`.

- [x] **Task 2 — Stale-flagging rail (AC2, AC3, NFR-R1)**
  - [x] `JournalState::mark_provider_stale(study_id) -> Result<usize, String>` on the `mutate_study` rail: flips `freshness = Stale` on every `Source::Provider` cell (load-bearing + present optional) via struct-update, **retaining value/source/review/coverage/provenance/pending** (only the freshness axis moves). Returns the count; idempotent (already-stale → no change → no phantom undo step).
  - [x] Manual/derived cells untouched; a study with no provider cells flags nothing.
  - [x] **Also fixed the stale lifecycle in `refresh_cell` (3.3):** a successful refresh that returns the SAME value on a `Stale` provider cell now CONFIRMS currency → clears the flag to `Current` (else the idempotent "no re-stamp on equal value" would leave it stale forever after an outage). Tests: provider→Stale (value retained); manual stays Current; count; idempotent re-flag; verdict Full→Provisional after flag; successful refresh clears the flag.

- [x] **Task 3 — Wire the failure path in `main.rs` (AC1, AC2, AC4, AC5)**
  - [x] The `WorkerOutcome::Fetch` `Err` arm now sets `state::provider_failure_notice(&error)`, calls `mark_provider_stale(study_id)` (retain + flag), and re-renders. A shared `render_open` closure factors the open-study re-render + dashboard refresh across the success / empty / failure arms. `set_fetching(false)` (Fetch-arm-only, F5) + the worker-gone guard preserved; no data cleared.
  - [x] The `Ok` arm gains the **#46** guard: `Ok(fetched) if fetched.canonical.years.is_empty()` → `MSG_PROVIDER_NO_DATA` + `mark_provider_stale`, NOT an empty `apply_provider_refresh`. The non-empty path is unchanged.
  - [x] The `TestKey` arm is untouched (its own status to the Réglages panel).

- [x] **Task 4 — Messages, posture floors & gates (AC1, AC6, FR13)**
  - [x] Message-inventory floor `37 → 40`; `@tr` floor unchanged (`≥ 227` — no new `.slint` literal; the stale murmur + verdict badge already render).
  - [x] All four gates green `--locked`: fmt ✓, `clippy -- -D warnings` ✓, `cargo test --workspace` ✓ (app 183), `cargo deny check` ✓. Method fingerprint / determinism / golden / v1.db corpus clean. `Cargo.lock`/`deny.toml` unchanged; **app-crate-only confirmed** (no contract/core/ingestion/persistence SRC change).

- [ ] **Task 5 — Manual on-display GO/NO-GO (AC1, AC2, AC3, AC5) — Guy on display** *(RESIDUAL — needs Guy's desktop + a way to force each failure.)*
  - [ ] On Guy's desktop: populate a study from the provider, validate a load-bearing provider cell, then force a refresh failure and confirm perceptually:
    1. **offline** (disconnect network / point at a bad host) → the neutral "connexion / last-known" notice; the provider cells dim to the `◦` stale murmur; the validated cell's verdict badge drops to the **provisional hatched** state; the values are still there.
    2. **quota** (if reproducible) / **invalid key** (wrong key) / **403 plan** (Guy's free EODHD `/fundamentals`) → each names its own cause honestly.
    3. **empty payload** (a ticker the provider has no fundamentals for) → the "no data" notice, NOT "already up to date".
    4. Edit a stale cell by hand → it returns to `Current` (murmur clears); retry the refresh later → on success the cells re-stamp `Current` and the verdict recovers.
  - [ ] Headless cannot force a real network outage; the cause-classification + stale-flagging logic is proven by unit tests with `FakeProvider` canned failures (it is `Clone` for exactly this — `ingestion::error` doc).

## Dev Notes

### Scope decision (the 3.5 lane — the production trigger of "stale")

Story 3.3 surfaced the stale murmur + verdict degradation but deliberately **never set `Freshness::Stale` in production** (it noted "the production trigger of stale is Story 3.5"). Story 3.5 is that trigger, plus the cause-named banner:

- **3.5 = graceful provider failure (FR23/FR24/NFR-R1).** Classify the **already-existing** `ProviderError` taxonomy into neutral, cause-named notices; **retain** last-known values; **flag** the open study's provider cells `Freshness::Stale` (which the engine already degrades and the form already dims); stay non-blocking (offline continuity + manual override + retry). Also closes the **#46** empty-payload gap (a 200-with-no-years reads honestly as "no data", not "no change").
- **Reuses, does not build:** the `ProviderError`/`IngestionError` variants (ingestion, 3.1/3.2 — the `error.rs` doc literally says "the cause a later story (3.5) classifies into a banner"); `Freshness::Stale` + its form murmur (2.4) + the `(Validated, Stale) → GateState::Stale → Provisional` engine wiring (2.6, proven by `seam_check.rs` SEAM 3); the off-thread worker + outcome handler (3.1) and the F5 `set_fetching` fix (3.2). **App-crate-only** — no contract/core/ingestion/persistence SRC change.
- **Out of scope:** the annual-update ritual = **Story 3.6** (the last Epic-3 story). A provider **fallback chain** / quota **batching** are **FR26/FR27 [P2]** — not here (3.5 reports a single provider's failure; it does not fail over).

### What this story changes vs. preserves (files marked UPDATE — read before editing)

- **`app/src/state.rs` (UPDATE)** — add the three `MSG_*` + `provider_failure_notice` + `mark_provider_stale` (on the `mutate_study` rail, beside `apply_provider_refresh`). **Preserve:** the 3.3 refresh rail, the 3.4 reconcile/pending (the stale-flag must NOT touch `value`/`source`/`review`/`coverage`/`provenance`/`pending` — only `freshness`), and the idempotency discipline (re-flag = no `before != study` change → no phantom undo step).
- **`app/src/main.rs` (UPDATE)** — the `WorkerOutcome::Fetch` `Err` arm (cause notice + `mark_provider_stale` + `push_form`) and the `Ok` arm (#46 empty-payload check). **Preserve:** `set_fetching(false)` Fetch-arm-only (F5), the worker-gone guard, the 3.2 key-resolution, and the `TestKey` arm.
- **`app/src/posture.rs` (UPDATE)** — message-inventory floor `37 → 40`. The `@tr` floor is unchanged (no new `.slint` literal).
- **NO UI `.slint` change** — the stale murmur (`◦`/dimming), the verdict badge's provisional-hatched state, and the global notice surface all already exist (2.4/2.6/2.7). 3.5 only feeds them from the failure path.

### Architecture & constraints

- **NFR-R1 (PRD lines 857–862, Journey lines 346–353):** "the full workflow runs offline; losing the network degrades gracefully"; "last-known values remain in place… a clear message names the cause (network / quota / key)… it never produces a silent wrong signal." Enforced by construction: `mark_provider_stale` only touches `freshness`; the engine degrades the verdict; nothing is cleared.
- **FR23 / FR24 (PRD lines 704–706):** retain last-known + flag stale/to-update; record + report the cause (network / quota / invalid-or-absent key). The taxonomy is complete in `ingestion::error` — 3.5 maps it.
- **The stale seam (Spike-D, `docs/spikes/spike-d-stale-reconcile.md` + `seam_check.rs`):** SEAM 1 (`Freshness::Stale` → `GridCellState.stale` murmur) and SEAM 3 (`(Validated, Stale)` → `GateState::Stale` → `Verdict::Provisional`) are proven through the real rails. 3.5 is the first production caller that sets the flag — add a real-refresh integration test that the verdict actually drops after `mark_provider_stale`.
- **Attention hierarchy (PRD line 46):** stale = "a discreet uniform murmur", missing = the only state that shouts. The stale flag must never collide with the buy/hold/sell zone colours (it does not — freshness is texture, not colour; the form already enforces this).
- **NFR-S1:** the api_token is in the request URL; the 3.2 `reqwest::Error::without_url()` fix already strips it before it reaches a notice. The cause-named notices are static strings (no `{cause}` interpolation of the raw error for Network), so there is **no** path for the token to leak — prefer the static `MSG_PROVIDER_OFFLINE` over interpolating `error.to_string()`.
- **App-crate-only:** method fingerprint / determinism / golden / corpus stay green (no calc/contract/shape change). `Cargo.lock` unchanged.

### Previous-story intelligence (3.1 → 3.4)

- **`mark_provider_stale` mirrors `unlock_all`** (Story 2.5): a study-wide review-only flip on the `mutate_study` rail, one `put_study`, returns a count. Reuse that shape (iterate `study.years`, `entry::get_cell`/`set_cell` or direct field access). The freshness flip is even simpler (no scope filter — all provider cells).
- **Idempotency trap (3.3/3.4):** only mutate on a real change. `mark_provider_stale` setting an already-`Stale` cell to `Stale` is a no-op → `before != study` is false → no undo step, no journal churn. Verify with a test (re-flag → `undo_depth` unchanged).
- **`Source` branching (3.3/3.4):** the rail keyed cell behaviour on `cell.source`. Here: flag iff `source == Source::Provider`. A `NotAvailableAccepted` provider cell (value None) — flag it stale too (it is provider-sourced), harmless (the murmur shows on the marker). Keep it simple: any `Source::Provider` cell.
- **The Story-3.4 `pending` must be preserved** by `mark_provider_stale` (a divergence and a stale flag are independent axes). Use struct-update (`Cell { freshness: Stale, ..cell }`) so all other fields — including `pending` — carry through.
- **Issue #46** (3.3 review): an empty 0-year payload reads as "no change" — AC5 closes it here.

### Testing standards

- Headless Rust unit/integration tests (Slint-native, no-web — QA e2e step N/A). Use `FakeProvider` canned failures (`Clone`) + the 3.3 `fetched_for`/`fetched_custom` helpers (and an empty-years variant for AC5) to drive each path.
- **Cause mapping:** assert each `ProviderError` variant → its `MSG_*` const; `Normalize` → `MSG_NORMALIZE_FAILED`.
- **Stale rail:** provider cell → `Stale` (value/review/pending preserved); manual stays `Current`; count; idempotent re-flag (no undo step); a real-refresh verdict drop to `Provisional` (complements `seam_check.rs` SEAM 3 through the production rail).
- **#46:** an empty-payload refresh → `MSG_PROVIDER_NO_DATA`, not `MSG_REFRESH_NOCHANGE`; a populated study additionally goes stale.
- **AC6 guard:** confirm `core`/`contract`/`persistence`/`ingestion` SRC unchanged (only `app/`). Keep `seam_check.rs` green. All four gates `--locked`; pinned rustfmt 1.9.0.
- UI story → on-display GO/NO-GO is part of DoD (Task 5); headless cannot force a real outage.

### Open questions for dev (resolve during implementation, don't block)

- **Which causes flag stale?** Leaning **all provider-fetch failures** (Network/Quota/Key/Forbidden/TickerNotFound) flag the open study's provider cells stale — a failed refresh means the data's currency is unverifiable, regardless of why. (A parse/normalize error is a malformed payload, not an outage — still flag stale, since the refresh did not land. Confirm: yes, flag on any non-success.) The banner names the WHY; the stale flag states the EFFECT.
- **Stale on a fresh/empty study:** nothing to flag (no cells) — the notice alone. Confirmed by construction.
- **Quota `retry_after_secs`:** surface the seconds in the notice, or keep it generic? Leaning **generic** ("retry later") for v1 — a `{n}`-interpolated retry hint is a nicety, and avoids a dynamic-value posture-scan concern. Record the seconds via `tracing` only.
- **Does a successful refresh clear stale?** Yes, by construction: `apply_provider_refresh` stamps refreshed cells `Freshness::Current` (3.3). No extra code — but add a test that stale → successful refresh → `Current`.

### Project Structure Notes

- App-crate-only (`app/src/{state,main,posture}.rs`). No new module, no `.slint` change, no contract/core/ingestion/persistence SRC change, no schema/method change, no new dep.
- Matches the architecture: the failure banner + stale flagging are app-layer reactions to the ingestion taxonomy; the engine/contract stay untouched.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.5 (lines 806–817)] — AC source; the 3.5/3.6 split.
- [Source: _bmad-output/planning-artifacts/prd.md (FR23/FR24 lines 704–706, NFR-R1 lines 857–862, FR65 line 585, Journey-2c lines 346–353, attention hierarchy line 46)] — requirements.
- [Source: _bmad-output/planning-artifacts/architecture.md (lines 397–398 global banner + last-known retention + stale flagging, 154–155 trust model, 538–540 neutral cause-named banner)] — the failure-banner intent.
- [Source: ingestion/src/error.rs — `ProviderError` (Network/Quota/InvalidOrAbsentKey/Forbidden/TickerNotFound/Parse/Unsupported), `IngestionError`] — the taxonomy to classify (its doc names Story 3.5 as the classifier).
- [Source: app/src/main.rs — the `WorkerOutcome::Fetch` outcome handler (`Err` arm + `Ok` arm) and the `TestKey` cause-match (3.2)] — the failure path to extend; the F5 `set_fetching` + worker-gone guard to preserve.
- [Source: app/src/state.rs — `apply_provider_refresh`/`refresh_cell` (3.3), `mark_…` rails like `unlock_all` (2.5), `MSG_*` inventory] — the rail to mirror + the message inventory.
- [Source: app/src/viewmodel/engine.rs `cell_to_gate_state` (`(Validated, Stale) → Stale`) + app/src/viewmodel/form.rs `editable_cell` (`stale` murmur)] — the display + degradation 3.5 feeds (no change here).
- [Source: app/src/seam_check.rs SEAM 1 + SEAM 3] — the stale murmur + verdict-degrade seams; keep green; 3.5 is the first production caller.
- [Source: Story 3.3 — 3-3-…md (the "production trigger of stale is 3.5" note) + Story 3.4 — 3-4-…md (the `pending` axis to preserve) + issue #46] — the carry-ins this story resolves.
- [Source: memory project-planning-progress — CHECKPOINT 2026-06-27] — 3.4 done; 3.5 = graceful failure, #46 lands here.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- `cargo test -p steadyinvest-app` → app **183** tests (178 → 183: +5 — failure-notice mapping, stale-flag retain, manual-stays-current + idempotency, verdict degrade, stale-clear-on-refresh).
- `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --all --check` clean; `cargo deny check` ok; `cargo test --workspace` green (method/golden/corpus/v1.db unchanged); `timeout 8 cargo run` → exit 124.
- App-crate-only confirmed: `git status` shows changes only under `app/` (no contract/core/ingestion/persistence SRC, no Cargo.lock).

### Completion Notes List

- **Tasks 1–4 complete; Task 5 (manual on-display GO/NO-GO) is the RESIDUAL** — needs Guy's desktop + a way to force each failure (offline / quota / key / empty). The cause-classification + stale-flagging logic is fully proven headless (`FakeProvider`-style canned errors + the real `mark_provider_stale` rail). Same pattern as 3.1–3.4.
- **The production trigger of stale, at last:** `Freshness::Stale` was built (display 2.4, verdict-degrade 2.6) but never set in prod until now. `mark_provider_stale` is the first caller; the form murmur (`◦`) and the `(Validated, Stale) → Provisional` degradation needed **zero** new display/engine code (AC3 proven through the real rail by `a_stale_flag_degrades_a_full_verdict_to_provisional`).
- **Last-known retention (NFR-R1):** a failure never clears or overwrites a value — `mark_provider_stale` moves only the freshness axis (struct-update preserves value/source/review/coverage/provenance/pending). The user keeps working offline, edits by hand (a manual edit clears stale via `Cell::edited`), and retries.
- **Stale lifecycle fix (the one non-obvious interaction):** 3.3's idempotency ("only re-stamp a provider cell when the value changed") would have left a cell stale forever after an outage if the retry returned the same values. `refresh_cell` now clears a `Stale` flag to `Current` on a value-agreeing successful fetch (currency confirmed) — proven by `a_successful_refresh_clears_the_stale_flag`.
- **#46 closed:** an empty (0-year) transport-success is now `MSG_PROVIDER_NO_DATA` (+ stale flag on a populated study), not `MSG_REFRESH_NOCHANGE`.
- **NFR-S1:** the cause notices are static strings (no `{cause}`/raw-error interpolation), so the api_token (in the request URL) has no path into the banner. Parse/Unsupported/Normalize route to the static `MSG_NORMALIZE_FAILED`, never `MSG_PROVIDER_FAILED`'s `{cause}` placeholder.
- **App-crate-only:** `ProviderError` taxonomy (ingestion) + `Freshness`/`Cell` (contract) already existed — 3.5 only classifies + sets. No method/contract/schema change; method fingerprint / golden / corpus / v1.db clean; no new dep.

### File List

**Modified**
- `app/src/state.rs` — `MSG_PROVIDER_OFFLINE`/`MSG_PROVIDER_QUOTA`/`MSG_PROVIDER_NO_DATA` + inventory; `provider_failure_notice`; `mark_provider_stale` (the `mutate_study` rail, with a `count_provider_to_stale` no-op pre-check guard — review patch); `apply_provider_refresh` clears all provider `Stale` flags up front on a successful refresh (outage recovery — review patch); shared `year_cells_mut` helper; 7 new 3.5 tests (app 178 → 184).
- `app/src/main.rs` — the `WorkerOutcome::Fetch` arm: shared `render_open` closure; `Err` → cause notice + `mark_provider_stale`; `Ok` empty-payload (#46) guard → `MSG_PROVIDER_NO_DATA` + `mark_provider_stale`.
- `app/src/posture.rs` — message-inventory floor `37 → 40`.

### Change Log

- 2026-06-27 — Story 3.5 implemented (graceful provider failure, FR23/FR24/NFR-R1). App-crate-only: classify the existing `ingestion::ProviderError` taxonomy into neutral cause-named notices (offline / quota / no-data / key / 403 / malformed); `mark_provider_stale` is the first production caller that sets `Freshness::Stale` (retaining last-known values), degrading a validated provider study to Provisional via the existing engine wiring; a successful refresh clears the stale flag (incl. on unchanged values); #46 closed (empty payload → no-data, not no-change). app 178 → 183 tests; all four gates green; method/golden/corpus/v1.db clean; Cargo.lock untouched. Status → review. Task 5 (manual on-display GO/NO-GO) pending Guy's display.
- 2026-06-27 — 3-layer adversarial code review (Blind + Edge + Acceptance). Acceptance Auditor: **ACCEPT** (AC1–AC6 all PASS; app-crate-only, message floor 37→40 exact, no banned verbs, no raw-error leak). **2 MEDIUM patches applied**, rest dismissed. app 183 → 184 tests; all gates re-green. Status → done.

## Review Findings (3-layer adversarial code review, 2026-06-27)

Layers: Blind Hunter (diff-only) + Edge Case Hunter (diff + project) + Acceptance Auditor (diff + spec). Auditor verdict: **ACCEPT** — AC1–AC6 implemented; the cause-classification, last-known retention, verdict degradation, and #46 close were all verified. The hunters found two real stale-lifecycle gaps. 2 patch · 0 defer · several dismissed.

### Patches (applied)

- [x] [Review][Patch] **MEDIUM — a provider cell a successful refresh does not re-confirm stays `Stale` forever** [app/src/state.rs `apply_provider_refresh`/`refresh_cell`] — the per-cell stale-clear only fired on a value-agreeing *visited* cell, so a field the fetch omits (an optional returned `None`) or a year outside a narrower fetch set stayed stale indefinitely after one outage, keeping a validated input degraded (violates AC2). **Fix:** a successful (provider-responded) refresh now clears `Freshness::Stale` on **every** provider cell up front (the outage is over — a study-wide recovery, not per-fetched-cell), then applies the value updates; the per-cell `refresh_cell` stale-clear was reverted (subsumed — also removes the Blind finding that it mislabelled the recompute cause as `Updated`). Test `a_successful_refresh_clears_stale_on_years_it_omits` (a narrower fetch recovers the omitted years).
- [x] [Review][Patch] **MEDIUM — `mark_provider_stale` wrote a journal revision on every failed refresh, even a no-op** [app/src/state.rs] — `mutate_study` calls `put_study` unconditionally (only the *undo* step is guarded by `before != study`), so repeated offline retries / an empty study / a manual-only study each bumped `logical_version` and rewrote the blob despite flagging 0 cells (avoidable writes on the Synology-synced DB). **Fix:** a pre-check (`count_provider_to_stale`, a `&Study` read) early-returns `Ok(0)` before entering the mutation rail when nothing is flaggable (mirrors the Story-3.4 accept/keep guard). Test strengthened to assert `logical_version` unchanged on an idempotent re-flag. Refactored the flag/clear walks onto one shared `year_cells_mut` helper.

### Dismissed (verified / acceptable)

- **Blind MEDIUM — the stale-clear returned `CellRefresh::Updated`, mislabelling the recompute cause.** Resolved by Patch 1 (the per-cell stale-clear was removed in favour of the up-front recovery pass, which is not counted in the report).
- **Edge LOW — flagging an un-validated study stale records an undoable step.** Acceptable and consistent: the stale flag IS a real state change (`Current → Stale`); making it undoable is correct (the `◦` murmur renders regardless of review; the verdict effect only materialises once cells are validated, by design).
- **Blind LOW — `render_open` closure edition-dependence / `MSG_PROVIDER_FAILED` orphaned / swallowed `mark_provider_stale` errors.** Verified: the workspace is edition 2021 (compiles); `MSG_PROVIDER_FAILED` is still used by the `TestKey` arm and the worker-gone send-failure path (main.rs:502/557/1769), not orphaned; a best-effort `let _ =` on the stale-flag is acceptable (a flag failure must not block the cause notice).
- **Auditor note — the `TestKey` arm still interpolates `{cause}`.** Explicitly out of 3.5 scope (untouched per Task 3); relies on the Story-3.2 `without_url()` token strip — not a regression.

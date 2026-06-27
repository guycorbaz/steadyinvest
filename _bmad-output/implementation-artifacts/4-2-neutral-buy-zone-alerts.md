# Story 4.2: Neutral buy-zone alerts

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want a neutral alert when a watched security enters its buy zone,
so that I notice an opportunity I defined, without being told what to do.

## Acceptance Criteria

(From epics.md §Story 4.2, lines 850–861. FR35, FR13. Builds on Story 4.1's watchlist→study link. Scope-resolved in Dev Notes.)

1. **AC1 — A watched security whose linked study shows its current price in its buy zone raises a neutral alert (FR35).** For each watchlist entry **linked to a saved study** (Story 4.1's `study_id`), the app reads that study's computed **`present_price_zone`** (the §4 risk-reward zone the study's `current_price` falls in — `core::ssg`, already built in Story 2.6). When it is **`Zone::Buy`**, the entry is flagged "in its buy zone" on the watchlist surface. An unlinked entry, or a linked study with no usable zone (no `current_price` / degenerate bounds / does not normalize), raises **no** alert.

2. **AC2 — The alert is a neutral, fact-stating statement — no action verb (FR35, FR13).** The alert reads as the fact "**le prix est entré dans la zone que vous avez définie**" (the user defined the zone via the study's judgment) — never "buy", "achetez", "opportunity to buy", or any imperative. It states *what happened*, not *what to do*. Posture-gated (banned-verb-clean); a per-watchlist summary names the count ("N valeur(s) dans leur zone d'achat").

3. **AC3 — The alert uses the global **banner/attention register** (ink + icon + position), NOT the zone hues.** The saturated buy/hold/sell colours stay **geofenced to the §4 zone bar** (the colour-budget rule) — the watchlist alert is rendered in **neutral ink + an attention glyph** (the notice/banner register, like the `△` plausibility murmur or the global notice), never a buy-coloured chip. No new saturated colour is spent on the watchlist.

4. **AC4 — Reflects the latest computed state; recomputes on refresh.** The alert is **derived** (not stored): it is recomputed every time the watchlist surface is rebuilt (`refresh_watchlist` — at startup, after a watchlist write, and after a study delete). It therefore reflects the linked study's **current** `current_price` + zone. (A *price* refresh that moves the number is the open-study provider refresh today, Story 3.3, and the per-watchlist price refresh later, Story 4.4 — 4.2 reads whatever zone the study currently computes; it does not itself fetch a price.)

5. **AC5 — App-crate-only; reuses the engine; no new persistence/schema/method change.** The zone classification is **`core::ssg`'s existing output** (`SsgOutputs.risk_reward.present_price_zone`, reached via `engine::build_snapshot(study).outputs()`) — **no `core`/`contract`/`persistence` change, no schema/migration, no method change** (the method fingerprint / golden / determinism / corpus gates stay green). `Cargo.lock`/`deny.toml` unchanged.

6. **AC6 — Verdict integrity preserved.** The buy-zone fact is **not** a recommendation and does not alter the study's verdict or the engine. It is a presentation-layer read of an existing computed value. A study with a `Withheld`/`Provisional` verdict can still be "in its buy zone" (the zone is a price-vs-bounds fact, independent of input-validation state) — surface the fact honestly; do not gate the alert on the verdict.

## Tasks / Subtasks

- [x] **Task 1 — Compute the buy-zone flag per watched security (AC1, AC4, AC5, AC6)**
  - [x] `viewmodel::engine::study_in_buy_zone(&Study) -> bool` = `build_snapshot(study).outputs().risk_reward.present_price_zone == Some(Zone::Buy)`, `unwrap_or(false)` on a non-normalizing study. Called from `refresh_watchlist` per linked watched study (`state.get_study(sid)` → the flag); unlinked entries skip the snapshot.
  - [x] One `build_snapshot` per linked watched study; no engine/core change — `present_price_zone` already existed (2.6).

- [x] **Task 2 — Surface the flag on `WatchRow` + the screen (AC1, AC2, AC3)**
  - [x] `WatchRow.in-buy-zone: bool` + `Watchlist.in-buy-zone-count: int` (state.slint), both set by `refresh_watchlist`.
  - [x] `watchlist.slint`: a per-row neutral marker `@tr("◆ Le prix est dans la zone que vous avez définie.")` (ink, `text-high`, never a buy hue) shown when `row.in-buy-zone`; a screen-top summary `@tr("◆ {} valeur(s) dans leur zone d'achat.", count)` when `count > 0`. The glyph lives **inside** the `@tr` (not a bare-literal concatenation, which the leak gate would flag).
  - [x] Colour budget honoured — neutral ink + a `◆` glyph, no zone hue referenced.

- [x] **Task 3 — Messages, posture & gates (AC2, AC5)**
  - [x] Alert prose is `.slint`-only (no Rust `MSG_*`); `@tr` floor `236 → 238` (the 2 new literals). Banned-verb-clean (the posture gate stays green — no `acheter`/imperative).
  - [x] All four gates green `--locked`: fmt ✓, `clippy -- -D warnings` ✓ (0), `cargo test --workspace` ✓ (app 192), `cargo deny check` ✓. Method fingerprint / golden / determinism / corpus clean (no calc change — a read). `Cargo.lock`/`deny.toml` unchanged; **app-crate-only confirmed** (no core/contract/persistence change).

- [x] **Task 4 — Tests (AC1, AC4, AC6)**
  - [x] `study_in_buy_zone_reflects_the_current_price_and_is_verdict_independent`: a provider-filled (unvalidated → NOT `Full`) study with a complete judgment and `current_price` in the bottom third of the band → `study_in_buy_zone == true` (AC1 + AC6 verdict-independence); `current_price` in the upper band → false; no `current_price` (no zone) → false.

- [ ] **Task 5 — Manual on-display GO/NO-GO (AC1, AC2, AC3) — Guy on display** *(RESIDUAL — needs Guy's desktop.)*
  - [ ] On Guy's desktop: link a watchlist entry to a study whose current price is in its buy zone; confirm the neutral marker + the fact-statement appear in **neutral ink** (no buy hue), and the summary line shows the count; edit the study's current price out of the buy zone (and return to the watchlist) and confirm the alert clears. Confirm the wording is a neutral fact, never an instruction.

## Dev Notes

### Scope decision (a presentation read of an existing engine output)

The hard work — classifying a price into Buy/Neutral/Sell against the SSG forecast band — was built in **Story 2.6** (`core::ssg::risk_reward`). 4.2 is a **presentation-layer read**: surface `present_price_zone == Buy` for each watched, linked security as a neutral alert. **App-crate-only**; no engine/contract/persistence/schema/method change.

- **The zone is the study's, the alert is the watchlist's.** A watchlist entry links to a study (Story 4.1's `study_id`); the study computes the zone from its own `judgment.current_price` vs its forecast band. 4.2 reads that and flags the watchlist row. No new price source: 4.2 **does not fetch a price** — it reads whatever the linked study currently computes.
- **"When a manual refresh moves its price into that zone" (the epic's trigger):** the price number is `judgment.current_price`, moved by the open-study edit (2.6) or the open-study provider refresh (3.3) today, and by a **per-watchlist price refresh in Story 4.4** later. 4.2 establishes the **alert logic + surface**; 4.4 will be the recurring price-move trigger. The alert recomputes on every `refresh_watchlist`, so it is always current as of the last surface rebuild.
- **Verdict-independent (AC6):** the buy-zone fact is a price-vs-bounds comparison; it stands even when the verdict is `Provisional`/`Withheld`. Do not gate the alert on `verdict()`. (A study with no `current_price`, or degenerate bounds, simply has `present_price_zone == None` → no alert — that is the natural "no defined zone" case the AC mentions.)
- **Out of scope:** the per-watchlist price *refresh* (4.4); the trailing-stop / stop-breach alert (4.5, same neutral-alert posture); OS-native notifications (PRD: in-app only in v1). A **persisted "already alerted" / dedup** state is NOT in scope — the alert is a live derived fact, re-shown whenever the price is in zone (no edge-trigger bookkeeping; the watchlist is glanced at, not push-notified).

### What this story changes vs. preserves (files marked UPDATE — read before editing)

- **`app/src/main.rs` (UPDATE)** — `refresh_watchlist` additionally computes, per linked watched study, the buy-zone flag (`engine::build_snapshot(&study).outputs().risk_reward.present_price_zone == Some(Zone::Buy)`) and the count; sets them on `WatchRow.in_buy_zone` + `Watchlist.in_buy_zone_count`. **Preserve:** the Story-4.1 row build (ticker, `linked`, `study_link`), the study-link resolution, and the call sites (startup, watchlist writes, study delete).
- **`app/ui/state.slint` (UPDATE)** — `WatchRow.in-buy-zone: bool`; `Watchlist.in-buy-zone-count: int`. **Preserve:** the 4.1 `WatchRow`/`Watchlist` fields + callbacks.
- **`app/ui/screens/watchlist.slint` (UPDATE)** — per-row neutral alert marker + revealed fact; the summary line. **Preserve:** the 4.1 list/add/reorder/link affordances; the colour budget (no zone hue).
- **`app/src/viewmodel/engine.rs` (READ-ONLY reference)** — `build_snapshot` + `zone_key`/`present_price_zone` access pattern (the §4 zone bar at `engine.rs:505 zone_bar` reads `r.present_price_zone` exactly this way). Mirror that read; change nothing.
- **`app/src/posture.rs` (UPDATE)** — bump the `@tr` (and/or message) floor to the measured count.

### Architecture & constraints

- **The colour budget (PRD line 46, architecture trust markers):** strong colours are reserved for the §4 judgment zones; **provenance/alerts are texture/ink, never colour.** The buy-zone alert on the watchlist must be a neutral attention marker (ink + glyph + the global-banner register), NOT a buy-hued chip — the saturated zones stay geofenced to the open study's §4 zone bar.
- **Neutral voice (FR13, PRD lines 36):** "Buy-zone alerts are stated as a neutral fact ('price entered the zone YOU defined'), never as a recommendation." The posture gate enforces no banned verb; the phrasing states the fact.
- **Single source of the zone (`core::ssg::risk_reward`):** `present_zone(bounds, current)` returns `Zone::Buy` when `current ≤ buy/hold boundary` (the normative comparator, Story 2.6 — proven by `zone_intervals_use_the_normative_comparators`). 4.2 reads the result; it must not re-derive the boundary (that would fork the method).
- **In-app only (PRD line 571):** v1 alerts surface in-app on the watchlist surface (and on manual refresh) — no OS-native notifications.
- **App-crate-only:** method fingerprint / determinism / golden / corpus stay green (no calc/contract change); no new dep.

### Previous-story intelligence (4.1 + 2.6 + Epic-3 retro)

- **Story 4.1** built `refresh_watchlist` (the row builder + the `study_id` link + the `by_id` ticker map) and `WatchRow { id, ticker, linked, study-link }`. 4.2 extends `refresh_watchlist` + `WatchRow` — reuse, don't rebuild. The `linked`/`study_id` is the gate (only linked entries can have a zone).
- **`engine::build_snapshot(&study)`** is the app's existing entry to the SSG outputs (used by `zone_bar`, `risk_computed`, etc.); `snapshot.outputs().risk_reward.present_price_zone: Option<Zone>` is the exact field. `Zone` is `core::ssg::Zone` (`Buy`/`Neutral`/`Sell`).
- **Epic-3 retro C1/C4:** the on-display residuals are piling up (Task 5 adds one — batch it); no idempotency-write concern here (4.2 writes nothing — it is a pure read).
- **The study-builder test pattern** (`a_stale_flag_degrades_a_full_verdict_to_provisional`, `the_annual_update_journey…`) shows how to construct a study with a known judgment + cells; adapt it to land `current_price` inside the buy zone (a `current_price` at/below the buy/hold boundary the forecast band implies).

### Testing standards

- Headless Rust unit/integration tests (Slint-native, no-web — QA e2e N/A). The buy-zone flag is fully headless-provable (build a study with a known zone, assert the flag + count); the screen is the on-display residual (Task 5).
- **Reuse the `core::ssg::risk_reward` zone semantics** — don't re-assert the boundary math (that's 2.6's golden/unit tests); assert the **app surfacing** (linked-in-zone → flag true; not-in-zone / unlinked / no-price → false; count correct; verdict-independent).
- All four gates `--locked`; method/golden/corpus clean; no new dep.
- UI story → on-display GO/NO-GO is part of DoD (Task 5).

### Open questions for dev (resolve during implementation, don't block)

- **Recompute on study edit:** today `refresh_watchlist` runs at startup / watchlist writes / study delete. A study `current_price` edit (on the study screen) won't update the watchlist alert until the watchlist is next rebuilt. For v1 that is acceptable (you glance at the watchlist; it recomputes on entry/refresh). If a nav-to-watchlist hook is cheap, call `refresh_watchlist` on entering the watchlist screen too — leaning **yes if a screen-enter hook exists**, else accept the on-rebuild recompute. (A live alert on any study edit is over-scope.)
- **Marker vs banner:** a per-row marker + a screen-top summary line (leaning both — the marker says *which*, the summary says *how many*) vs only one. Keep it neutral-ink either way.
- **Glyph choice:** reuse the `△` attention glyph (plausibility), or a distinct alert glyph (e.g. a small bell/dot) to avoid conflating "plausibility finding" with "buy-zone fact". Leaning a **distinct neutral glyph** so the two attention channels don't merge; confusability-gate it (per the trust-markers convention).

### Project Structure Notes

- App-crate-only (`app/src/main.rs` + `posture.rs` + 2 `.slint`). No `core`/`contract`/`persistence` change, no schema/method change, no new dep, no new corpus.
- Builds on Story 4.1's watchlist; feeds the same neutral-alert posture that Story 4.5 (trailing-stop breach) will reuse.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.2 (lines 850–861)] — AC source.
- [Source: _bmad-output/planning-artifacts/prd.md (FR35; line 36 neutral-alert posture; line 46 colour budget; line 571 in-app-only; Journey-3 line 332)] — requirements.
- [Source: core/src/ssg/risk_reward.rs (`present_zone`, `zone_bounds`, the `Zone` enum) + core/src/ssg/types.rs (`RiskRewardOutputs.present_price_zone`, `present_price_in_buy_zone`) + core/src/verdict.rs (`StudySnapshot::outputs`)] — the existing zone computation 4.2 reads (do not change).
- [Source: app/src/viewmodel/engine.rs (`build_snapshot` line 238, `zone_bar` line 505 reading `r.present_price_zone`, `zone_key`)] — the app's existing read pattern to mirror.
- [Source: Story 4.1 — 4-1-watchlist-management.md + commit c7736ce (`refresh_watchlist`, `WatchRow`, the `study_id` link)] — the surface this story extends.
- [Source: app/src/posture.rs — the `@tr`/message floors] — bump to the measured counts.
- [Source: Epic-3 retro — epic-3-retro-2026-06-27.md (C1 on-display residuals)] — batch Task 5 with the pending GO/NO-GO checks.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- The buy-zone boundary semantics matter: `core::ssg::risk_reward::present_zone` returns `None` for a price **outside** `[forecast_low, forecast_high]` (a price *below* the band is not "Buy" — it is undefined). So the test's buy price must sit in the bottom third *within* the band (band ≈ [50–60, 160]; used `current_price = 70`), not an arbitrarily-low value.
- `cargo fmt --all --check` ✓ · `clippy --workspace --all-targets -- -D warnings` ✓ (0) · `cargo test --workspace` ✓ (app 192) · `cargo deny check` ✓ · `timeout 8 cargo run` → exit 124. App-crate-only (`git status`: only `app/`).

### Completion Notes List

- **Tasks 1–4 complete; Task 5 (manual on-display GO/NO-GO) is the RESIDUAL** — needs Guy's desktop (the neutral marker + summary appear in ink, no buy hue; the alert clears when the price leaves the zone). Batches with the 3.3–3.6 + 4.1 on-display checks.
- **A presentation read, nothing more:** `study_in_buy_zone` reads the existing `core::ssg` `present_price_zone` (built in 2.6) — **no core/contract/persistence/schema/method change**, no new dep, no migration. Method fingerprint / golden / determinism / corpus stay green by construction.
- **Verdict-independent (AC6):** the buy-zone fact is a price-vs-bounds comparison; the test proves it holds on an unvalidated (non-`Full`) study. The alert is not a recommendation and never alters the verdict.
- **Neutral by construction (FR13 / colour budget):** the alert is `◆`-glyph + neutral ink in the banner/attention register; the saturated buy/hold/sell hues stay geofenced to the open study's §4 zone bar. The fact-statement ("le prix est dans la zone que vous avez définie") has no action verb.
- **Derived, not stored:** recomputed on every `refresh_watchlist` (startup / watchlist writes / study delete) — no persisted "already alerted" state, no edge-trigger bookkeeping. The recurring price-move trigger arrives with the per-watchlist price refresh (Story 4.4).

### File List

**Modified**
- `app/src/viewmodel/engine.rs` — `study_in_buy_zone(&Study) -> bool` (reads `present_price_zone`).
- `app/src/main.rs` — `refresh_watchlist` computes the per-row buy-zone flag + the `in_buy_zone_count`.
- `app/src/state.rs` — the buy-zone surfacing test.
- `app/src/posture.rs` — `@tr` floor `236 → 238`.
- `app/ui/state.slint` — `WatchRow.in-buy-zone` + `Watchlist.in-buy-zone-count`.
- `app/ui/screens/watchlist.slint` — the neutral per-row marker + the summary line.

### Change Log

- 2026-06-27 — Story 4.2 implemented (neutral buy-zone alerts, FR35) — app-crate-only: a presentation read of the existing `core::ssg` `present_price_zone` flags each watched, linked security whose current price is in its §4 buy zone with a **neutral** marker + a summary count (ink + `◆` glyph, no buy hue, no action verb — FR13). Verdict-independent; derived (recomputed on every `refresh_watchlist`), not stored. No core/contract/persistence/schema/method change; method fingerprint / golden / corpus clean; no new dep. app 191 → 192 tests; `@tr` floor 236 → 238; all four gates green. Status → review. Task 5 (manual on-display GO/NO-GO) pending Guy's display.
- 2026-06-27 — Code review (3-layer: Blind Hunter + Edge-Case + Acceptance Auditor): both layers **ACCEPT**, 6/6 ACs PASS, no CRITICAL/HIGH/MEDIUM. Triage: (a) test-comment arithmetic fixed (buy_top ≈ 93, not ≈ 87) — cosmetic, no assertion change; (b) `study_in_buy_zone` doc extended to state the below-band silence explicitly; (c) the below-band-no-alert FR35 UX question filed as a tracked product decision — **issue #48** (AC-conformant, not a defect); (d) per-row full-study load on refresh and archived-but-linked-still-alerts noted as acceptable for v1 (dismissed). Re-ran all four gates after the patches: fmt clean, clippy 0, app 192 tests pass, deny ok, launch 124. Status → done.

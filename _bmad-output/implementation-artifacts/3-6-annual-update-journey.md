# Story 3.6: Annual update journey

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to refresh a saved study against a new annual report,
so that updating an existing study is a quick, safe ritual.

## Acceptance Criteria

(From epics.md §Story 3.6, lines 819–830. FR3 + FR22 + Journey 2b. The **capstone** of Epic 3 — it composes the rails built in 2.2 / 2.5 / 2.11 / 3.3 / 3.4 / 3.5 and adds the one missing piece: change visibility. Scope-resolved in Dev Notes.)

1. **AC1 — Reopen + re-fetch preserves manual entries and judgment lines.** Given a previously saved, validated study, when Guy reopens it and triggers the re-fetch (the existing provider refresh), then **manual cell values and the judgment snapshot are preserved** — the refresh only touches provider/derived/gap year cells (Stories 3.3/3.4) and never the `Judgment` (the judgment lines are not provider-refreshed), so a year's worth of hand-entered data and every judgment input survive the annual update intact (FR3, NFR-R4).

2. **AC2 — Changed cells whose value diverges from a `✓` reset to `?`; unchanged `✓` are kept (Journey-2b / invariant 2b).** When the new annual data **diverges** from a validated cell, that cell resets `✓ → ?` (a provider cell updates in place + demotes, Story 3.3; a manual cell keeps its value + preserves the divergent provider value as a pending + demotes, Story 3.4); a value that **agrees** keeps the human `✓` (value-based `Money` equality). This rides the existing `Cell::edited`/`Cell::reconcile` rails — **no new demotion logic**.

3. **AC3 — Guy re-validates only what actually moved (the change is made visible).** After the annual refresh, the study reports a **change summary** so Guy knows the scope of re-validation without hunting: the count of cells **to re-verify** (the `✓ → ?` demotions this refresh produced) alongside the existing updated / filled / reconciled counts and the recompute cause. The post-refresh notice names it (e.g. "… · N cellule(s) à revérifier"). The per-cell markers already show *where* (the `?` tag, the `◦` stale murmur, the 3.4 pending reveal, the provenance timestamp "when") — this AC adds the **aggregate** so the ritual is "re-check these N, not the whole study".

4. **AC4 — The projection can be extended; the zones recompute.** Guy can **extend the projection** (roll the data window forward by one fiscal year — the Story-2.11 `extend_history` rail, the "+ année" affordance) so the new annual year has a row to fill, and the engine **recomputes deterministically** off the new latest usable year (Story 2.11, unchanged). The annual update therefore both *reconciles the existing years* and *makes room for the new one*.

5. **AC5 — "Optionally after unlock all" — the validated study can be bulk-unlocked first.** The journey supports the "unlock all → re-fetch" path (Story 2.5 `unlock_all` + the refresh): if Guy unlocks the study first, the refresh reconciles against now-`?` cells (no demotions to report, since nothing was `✓`); if he refreshes a still-validated study, the divergent `✓` cells demote (AC2/AC3). Both paths are correct and need no special-casing — verify the composition holds.

6. **AC6 — "History reflects what changed and when" (within the v1 model); app-crate-only.** The study's per-cell provenance carries the **timestamp** of each change (revealed on demand, Story 3.3) and the markers (`?` / pending / stale) carry **what** changed — so "what changed and when" is surfaced per cell, and the change summary (AC3) aggregates it. **Durable cross-session refresh history (a log of past updates) is FR51 and remains deferred (issue #34) — out of scope here.** This story is **app-crate-only**: it composes existing rails + adds the demotion count to the existing `RefreshReport` + the summary notice. No method/contract/schema change; method fingerprint / determinism / golden / frozen `v1.db` corpus re-diff clean; `Cargo.lock`/`deny.toml` unchanged.

## Tasks / Subtasks

- [x] **Task 1 — Count the re-validation scope on a refresh (AC2, AC3)**
  - [x] `RefreshReport.revalidate: usize` added (+ `merge`). `refresh_cell` is now a thin wrapper returning `(CellRefresh, bool)` — it captures `was_validated` before, calls the extracted `refresh_cell_inner` (the unchanged 3.3–3.5 branching), and reports `demoted = was_validated && now ToReview`. `refresh_optional` returns the tuple too; `refresh_year`'s `account` closure counts `revalidate` when `demoted`.
  - [x] Idempotency preserved: a filled gap / agreeing re-fetch never demotes → `revalidate 0`; no double-count.

- [x] **Task 2 — Surface the change summary (AC3)**
  - [x] `MSG_REFRESH_REVALIDATE` ("{n} cellule(s) à revérifier.") added + registered; floor `40 → 41`.
  - [x] `state::refresh_summary(report) -> String` = `refresh_notice(report)` + (when `revalidate > 0`) " · {n} à revérifier"; with `revalidate == 0` it is exactly `refresh_notice` (no regression). `@tr` floor unchanged.
  - [x] `main.rs` `Ok` success arm uses `refresh_summary` (the #46 + failure arms unchanged).

- [x] **Task 3 — Journey integration test (AC1, AC2, AC4, AC5)**
  - [x] `the_annual_update_journey_…`: a saved study (provider + a manual year-0 sales override + complete judgment), all validated → re-fetch new annual data (high 100→200 diverges; year-0 sales held manually at 5000 diverges; eps/low agree). Asserts: manual sales preserved (5000, Source::Manual, pending=1000), judgment unchanged, changed provider high → `?`, unchanged eps keeps `✓`, manual sales → `?`, `revalidate == 6`, summary names "6 à revérifier", then `extend_history` appends the new year with prior years intact.
  - [x] `unlock_all_then_refresh_demotes_nothing_and_preserves_manual` (AC5): unlock → refresh → `revalidate 0`, manual value preserved.
  - [x] `revalidate_counts_only_demoted_validated_cells` + `refresh_summary_appends_the_revalidate_clause_only_when_needed` (unit).

- [x] **Task 4 — Gates (AC6)**
  - [x] Message floor `40 → 41`; `@tr` floor unchanged. All four gates green `--locked`: fmt ✓, `clippy -- -D warnings` ✓ (fixed a `doc_list_item` lint from splitting the `refresh_cell` doc), `cargo test --workspace` ✓ (app 188), `cargo deny check` ✓. Method fingerprint / determinism / golden / v1.db clean. `Cargo.lock`/`deny.toml` unchanged; **app-crate-only confirmed**.

- [ ] **Task 5 — Manual on-display GO/NO-GO (AC1–AC5) — Guy on display** *(RESIDUAL — needs Guy's desktop.)*
  - [ ] On Guy's desktop: reopen a saved, validated study; (optionally click "unlock all"); click the provider refresh with new annual data; confirm the manual entries + judgment lines survive, the changed `✓` cells visibly return to `?` (the others keep `✓`), the notice names "N à revérifier", the per-cell provenance timestamp shows "when", and the §4 zones recompute; click "+ année" to extend the projection and fill the new row; re-validate only the `?` cells. Confirm the whole ritual is quick and never rebuilds from scratch.
  - [ ] Test with `demo`/AAPL.US or fixtures (Guy's free EODHD plan 403s `/fundamentals`).

## Dev Notes

### Scope decision (the capstone — compose, don't rebuild)

Epic 3's last story is the **Journey-2b ritual**, and almost every rail it needs already exists. The discipline here is to **compose + verify**, adding only the one genuinely-missing piece (change visibility), NOT to rebuild:

- **Already built (reuse, do not touch):** reopen a saved study (2.2); `unlock_all` (2.5); `extend_history` + the "+ année" affordance (2.11); the provider refresh `apply_provider_refresh` + `refresh_cell`/`refresh_year` (3.3); non-destructive reconciliation `Cell::reconcile` + pending (3.4); stale-on-failure + last-known retention (3.5); the per-cell provenance **timestamp** revealed on demand (3.3) and the `?` / pending / `◦` markers (2.5 / 3.4 / 2.4) that show *what changed and when*.
- **3.6 adds:** the **`revalidate` count** on `RefreshReport` (how many `✓` the refresh reset to `?`) + the **change-summary notice** ("N cellule(s) à revérifier") so the annual update tells Guy the *scope of re-validation* — the "I re-validate only what actually moved" half of Journey-2b. Plus the **end-to-end integration test** proving the composition (preserve manual/judgment + demote-on-change + extend + unlock-first) holds through the real rails.
- **Explicitly OUT of scope:** durable cross-session **history** (a persisted log of past refreshes / a "you saw v57, this is v41" diff) = **FR51 / issue #34**, deferred since Epic 2. "History reflects what changed and when" is satisfied for v1 by the per-cell provenance timestamps + markers + the change summary — NOT a new history store. A provider **fallback chain** / **quota batching** = FR26/FR27 **[P2]**. No new "annual update" bundled button — the existing refresh + "+ année" + unlock-all affordances already make the journey clickable; 3.6 makes the refresh's *outcome* communicate the change.

### What this story changes vs. preserves (files marked UPDATE — read before editing)

- **`app/src/state.rs` (UPDATE)** — `RefreshReport` gains `revalidate: usize` (+ `merge`); `refresh_cell`/`refresh_optional`/`refresh_year` count a `✓ → ?` demotion; `MSG_REFRESH_REVALIDATE` + a `refresh_summary(report) -> String`. **Preserve:** the 3.3 idempotency (no-op refresh = `revalidate 0`, no churn), the 3.4 reconcile/pending, the 3.5 stale rail + the up-front stale-clear, the `NotAvailableAccepted` skip, and the existing `updated`/`filled`/`reconciled` semantics. The demotion count is **derived** from the existing `edited`/`reconcile` rails — do not re-implement the `✓→?` rule.
- **`app/src/main.rs` (UPDATE)** — the `WorkerOutcome::Fetch` `Ok` success arm uses `state::refresh_summary(report)` instead of `refresh_notice(report)`. **Preserve:** the #46 empty-payload arm + the 3.5 failure arm (cause notice + `mark_provider_stale`) + `render_open` + the F5 `set_fetching` discipline — all unchanged.
- **`app/src/posture.rs` (UPDATE)** — message-inventory floor `40 → 41`. `@tr` floor unchanged.
- **NO contract/core/persistence/ingestion change; NO `.slint` change** — `extend_history`, `unlock_all`, the refresh button, the per-cell markers + timestamp reveal all already render. 3.6 only enriches the post-refresh notice + adds the integration test.

### Architecture & constraints

- **FR3 (update an existing study) + FR22 (non-destructive reconciliation) + Journey 2b (PRD lines 317–325):** "reopen + re-fetch; reconciles new provider data… manual entries and judgment lines preserved, the validated flags on changed cells reset so he re-checks what actually moved. He extends the projection, the zones recompute, and the study's history shows what changed and when." Every clause maps to an existing rail except "shows what changed" — the change summary.
- **The judgment is never provider-refreshed:** `apply_provider_refresh` mutates only `study.years` (the cells), never `study.judgment` — so judgment lines are preserved by construction (verify in the integration test). The user's judgment is sovereign (FR33 — never auto-moved on a refresh).
- **Idempotency (3.3–3.5):** the `revalidate` count must be 0 on a no-op / agreeing refresh, and the summary must reduce to exactly today's `refresh_notice` when nothing was demoted (no regression on the common path).
- **`extend_history` re-bases the engine off the new latest usable year** (Story 2.11) — the horizon stays `FORECAST_HORIZON_YEARS = 5` (no method change; the determinism/fingerprint gate stays green).
- **App-crate-only:** no calc/contract/shape change → method fingerprint / determinism / golden / corpus clean; `Cargo.lock` unchanged.

### Previous-story intelligence (2.11 / 3.3 → 3.5)

- **`RefreshReport`** today = `{ updated, filled, reconciled, cause }` (+ `merge`/`changed`). 3.6 adds `revalidate`. The `account` closure in `refresh_year` is where per-cell outcomes are tallied — thread the demotion flag through there (or return it from `refresh_cell`/`refresh_optional`).
- **Demotion happens in two places:** `refresh_cell`'s provider branch (`cell.edited(new_value)` demotes a `✓` on value divergence, Story 3.3) and the manual branch (`cell.reconcile(...)` demotes a `✓` on divergence, Story 3.4). Detect both with the same `was_validated && now ToReview` check.
- **`extend_history`** (state.rs ~1218) appends `tofill_year(latest+1)` on the `mutate_study` rail; the engine re-bases off the new latest usable year. Reuse as-is in the integration test.
- **`unlock_all`** (state.rs) flips every `✓ → ?` in scope; after it, a refresh demotes nothing (already `?`).
- **3.4 `pending` + 3.5 stale** compose into the annual update: a divergent manual cell shows the pending provider value to reconcile; a prior failed refresh's stale flag clears on the successful annual fetch (3.5 up-front clear). The integration test should exercise at least the manual-divergence (pending) + provider-divergence (demote) cases.
- **The judgment-preservation + extend-history reopen behaviour** is already tested (2.11 `editing_and_the_soft_lock_hold_across_a_reopen`, `extend_history_*`); 3.6's integration test focuses on the *refresh-driven* reconciliation + the new `revalidate` count.

### Testing standards

- Headless Rust unit/integration tests (Slint-native, no-web — QA e2e N/A). Reuse the 3.3 `fetched_for`/`fetched_custom` helpers + `study_with_validated_manual_high` + `set_review`/`set_judgment_field`/`edit_cell`/`extend_history`/`unlock_all`.
- **The journey integration test is the centrepiece** (AC1/AC2/AC4/AC5): preserve manual + judgment, demote-on-change, keep-on-agree, `revalidate` count correct, extend appends, unlock-first → 0 demotions.
- **`revalidate` unit coverage:** a divergent provider `✓` → counted; a divergent manual `✓` → counted; an agreeing re-fetch → 0; a filled gap → 0; `merge` sums.
- **No-regression:** `refresh_summary` with `revalidate == 0` equals `refresh_notice`; keep `seam_check.rs` + all 3.3–3.5 refresh tests green.
- All four gates `--locked`; pinned rustfmt 1.9.0; method/golden/corpus/v1.db clean.
- UI story → on-display GO/NO-GO is part of DoD (Task 5).

### Open questions for dev (resolve during implementation, don't block)

- **Summary wording:** "{cause} · {n} à revérifier" vs two separate notices. Leaning a single combined line (one banner). Keep the `{n}` interpolation outside the posture-scanned literal (the template const is scanned; the runtime `{n}` substitution is a number, like the existing `unlock_done_message`).
- **`revalidate` threading:** return `(CellRefresh, bool)` from `refresh_cell`/`refresh_optional`, or capture the before-review in `refresh_year` and compare after. Either; pick the one that keeps the `account` closure clean.
- **Does a `revalidate` count belong on `RefreshReport.changed()`?** No — a demotion always coincides with an `updated`/`reconciled` (the value diverged), so `changed()` is already true; `revalidate` is a sub-count for the summary, not a separate change signal.
- **Extend-as-part-of-refresh?** No — keep `extend_history` a separate user gesture (the "+ année" button). The annual update is "refresh (reconcile) + optionally extend", two deliberate actions, not one bundled mutation (matches Journey-2b and keeps the rails composable).

### Project Structure Notes

- App-crate-only (`app/src/{state,main,posture}.rs`). No new module, no `.slint` change, no contract/core/ingestion/persistence change, no schema/method change, no new dep.
- The capstone of Epic 3: after this, **epic-3-retrospective** (optional) and Epic 4 (watchlist & single-portfolio risk, which depends on Epic 3's manual refresh).

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.6 (lines 819–830)] — AC source.
- [Source: _bmad-output/planning-artifacts/prd.md (FR3, FR22, Journey 2b lines 317–325, FR33 judgment-sovereign, FR51/durable-history deferred)] — requirements + the journey narrative.
- [Source: app/src/state.rs — `apply_provider_refresh`/`refresh_cell`/`refresh_year`/`RefreshReport` (3.3–3.5), `extend_history` (2.11, ~line 1218), `unlock_all` (2.5), `refresh_notice` + `MSG_REFRESH_*`] — the rails to compose + the report to extend.
- [Source: app/src/main.rs — the `WorkerOutcome::Fetch` `Ok` arm (3.3/3.5)] — where the summary notice plugs in.
- [Source: contract/src/cell.rs — `Cell::edited` (3.3 demotion) + `Cell::reconcile` (3.4 demotion/pending)] — the `✓→?` rule the `revalidate` count derives from (do not re-implement).
- [Source: Story 2.11 — 2-11-…md (extend-history rail) + Story 3.3/3.4/3.5 — the refresh/reconcile/stale rails this capstone composes] — previous-story intelligence.
- [Source: issue #34 — FR51 durable history, deferred] — the explicitly out-of-scope "history store".
- [Source: memory project-planning-progress — CHECKPOINT 2026-06-27] — 3.5 done; 3.6 = annual update journey, the last Epic-3 story, composes 2.11+3.3+3.4+3.5.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- `cargo test -p steadyinvest-app` → app **188** (184 → 188: +4 — revalidate count, summary clause, the journey integration test, unlock-first path).
- `cargo clippy --workspace --all-targets -- -D warnings` → initially failed `doc_list_item` (splitting `refresh_cell`'s doc left a stray bullet list); moved the branching doc onto `refresh_cell_inner` → clean.
- `cargo fmt --all --check` clean; `cargo test --workspace` green (method/golden/corpus/v1.db unchanged); `cargo deny check` ok; `timeout 8 cargo run` → exit 124.
- App-crate-only confirmed (`git status`: only `app/`).

### Completion Notes List

- **Tasks 1–4 complete; Task 5 (manual on-display GO/NO-GO) is the RESIDUAL** — needs Guy's desktop to walk the ritual (reopen → optionally unlock → refresh → re-validate the `?` → extend). Same pattern as 3.1–3.5.
- **The capstone composes, it does not rebuild:** reopen (2.2), unlock-all (2.5), refresh+reconcile (3.3/3.4), stale (3.5), extend-history (2.11) were all reused unchanged. The only new code is the **`revalidate` count** (how many `✓` the refresh reset to `?`) + the **`refresh_summary`** that names the re-validation scope — the "I re-validate only what actually moved" half of Journey-2b.
- **Demotion detection is observation, not new logic:** `refresh_cell` wraps the unchanged `refresh_cell_inner` and reports the `✓ → ?` transition around the in-place mutation. The `✓→?` rule itself stays in `Cell::edited`/`Cell::reconcile` (contract). A filled gap / agreeing re-fetch never demotes → `revalidate 0`; no double-count.
- **No regression on the common path:** `refresh_summary` with `revalidate == 0` returns exactly `refresh_notice` (asserted). The journey test proves manual values + the judgment snapshot survive a re-fetch by construction (`apply_provider_refresh` never touches `study.judgment`), changed `✓` demote, unchanged `✓` are kept, and `extend_history` appends the new year.
- **Out of scope (documented):** durable cross-session refresh history (FR51 / issue #34) stays deferred — "what changed and when" is served by the per-cell provenance timestamps + the `?`/pending/stale markers + this aggregate summary, not a new history store.
- **App-crate-only:** no contract/core/ingestion/persistence change, no method/schema change; method fingerprint / golden / corpus / v1.db clean; no new dep.

### File List

**Modified**
- `app/src/state.rs` — `RefreshReport.revalidate` (+ `merge`); `refresh_cell` → `(CellRefresh, bool)` wrapper + extracted `refresh_cell_inner`; `refresh_optional` returns the tuple; `refresh_year` `account` counts `revalidate`; `MSG_REFRESH_REVALIDATE` + inventory; `refresh_summary`; 4 new 3.6 tests.
- `app/src/main.rs` — the `WorkerOutcome::Fetch` `Ok` success arm uses `state::refresh_summary(report)`.
- `app/src/posture.rs` — message-inventory floor `40 → 41`.

### Change Log

- 2026-06-27 — Story 3.6 implemented (annual update journey, FR3/FR22/Journey-2b) — the **Epic-3 capstone**. App-crate-only: composes the existing reopen / unlock-all / refresh / reconcile / stale / extend-history rails and adds the one missing piece — change visibility: `RefreshReport.revalidate` counts the `✓ → ?` demotions a re-fetch produced, and `refresh_summary` names the re-validation scope ("N cellule(s) à revérifier") so the annual update is "re-check these N, not the whole study". Manual entries + judgment lines are preserved by construction; changed `✓` demote, unchanged keep `✓`; the projection extends. app 184 → 188 tests; all four gates green; method/golden/corpus/v1.db clean; Cargo.lock untouched. Status → review. Task 5 (manual on-display GO/NO-GO) pending Guy's display.
- 2026-06-27 — 3-layer adversarial code review (Blind + Edge + Acceptance). **Blind Hunter: no CRITICAL/HIGH/MEDIUM ("the capstone logic is correct"). Acceptance Auditor: ACCEPT (AC1–AC6 all PASS).** 0 patch · 1 deferred → issue #47 · rest dismissed. Status → done.

## Review Findings (3-layer adversarial code review, 2026-06-27)

Layers: Blind Hunter (diff-only) + Edge Case Hunter (diff + project) + Acceptance Auditor (diff + spec). **Blind Hunter found no real bug** (demotion detection sound across all branches, no double-count, idempotency holds, `merge` sums every field, `refresh_summary(revalidate==0)` byte-identical to `refresh_notice`). **Acceptance Auditor: ACCEPT** (AC1–AC6 all PASS; app-crate-only, floor 40→41 exact, no banned verbs, no durable-history store added). 0 patch · 1 defer · several dismissed.

### Deferred (→ GitHub issue, per the project's issue-tracking convention)

- [x] [Review][Defer] **LOW — validating an empty cell, then gap-filling it, drops the `✓` silently and escapes the re-validate count** [app/src/state.rs `refresh_cell_inner` gap-fill + `set_review`] — `set_review` has no value guard, so a user can `✓` an empty `ToFill` cell; a later refresh fills it via `*cell = provider_cell(...)` (review `None`), so the transition is `Validated → None` (not `→ ToReview`) — uncounted, and the `✓` vanishes silently. Degenerate (a `✓` on an empty cell vouches for no data) and **not a wrong signal** (the filled cell is correctly unvalidated, like any first fetch), but worth tracking. → **issue #47** (leaning fix: `set_review` refuses to validate an empty cell).

### Dismissed (verified non-defects)

- **Blind very-low — `MSG_REFRESH_REVALIDATE` is a `{n}`-template, unlike its final-string peers; the `" · "` summary glue is not a registered message.** Consistent with the existing `{n}`-template pattern (`unlock_done_message`, `verify_summary`); the separator is on the posture `BARE_LITERAL_ALLOW` list; both halves of the composed banner are individually neutral/registered. Not a leak.
- **Edge-verified-correct (no action):** the 3.5 stale flag/clear paths never run through `refresh_cell` (no `revalidate` contribution); a clear-stale + demote-divergent on the same cell is counted once; an N/A-accepted validated cell is skipped + uncounted; the multi-year `revalidate` sum is correct; `revalidate > 0 ⇒ changed() == true` (no contradictory "Aucun changement · N à revérifier"); only the success arm uses `refresh_summary`.

# Story 2.9: Undo/redo & scenario compare

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want reversible judgment exploration — undo/redo of any edit, and a way to view an alternate judgment placement beside the current one,
so that I can try "what if" without ever losing prior work.

## Acceptance Criteria

(From epics.md §Story 2.9, lines 657–669. BDD, verbatim intent.)

1. **Given** any judgment or grid edit (a data-cell edit/paste, a review-tag change, an "unlock all", a judgment-field entry, a forecast-low-option change, or a chart judgment-line drag), **when** I **undo**, **then** the study steps **back** to its state before that edit; **when** I **redo**, **then** it steps **forward** again — via an in-memory snapshot stack. The recomputed verdict/zones/§2–§5 after an undo/redo are **identical** to what they were at that point (deterministic restore).
2. **Given** FR32, **then** undo/redo **never destroys a saved input** — restoring a snapshot brings back exactly the inputs (data cells + judgment + review tags) that were present, and a judgment-line move can never silently blank a saved data cell.
3. **Given** the undo/redo affordance, **then** it is available via on-screen controls **and** the keyboard (Ctrl+Z undo / Ctrl+Y **and** Ctrl+Shift+Z redo), without colliding with the existing cell Ctrl-chords (Ctrl+Space, Ctrl+Enter, Ctrl+Backspace, Ctrl+V); the controls disable when their stack is empty; nothing destructive is silent (undo/redo themselves need no confirmation — they are non-destructive — UX-DR25).
4. **Given** I open **scenario compare**, **when** I set an **alternate** judgment placement, **then** I can view its resulting **zones, U/D ratio and projected return alongside** the current placement's, **without committing or losing the prior placement** (Phase 1: **exactly one** alternate — UX-DR12). The alternate is **user-set, never auto-placed or suggested** (FR33).
5. **Given** the colour budget, **then** scenario compare differentiates "current vs alternate" by a **non-zone channel** (ink value / dash / opacity / label / position) — **never a fourth saturated hue** (the three Okabe-Ito zone hues stay reserved for Buy/Hold/Sell). Each scenario's zones still honour **verdict integrity** (a provisional/withheld scenario renders hatched/empty, never full colour — FR12).
6. **Given** constant geometry (UX-DR22) and OS reduced-motion (UX-DR27), **then** opening/closing the compare overlay and any undo/redo recolour cause **no re-layout / jank**, and animations collapse to instant under reduced-motion. Closing scenario compare discards the alternate; the saved judgment remains the source of truth.
7. **Given** the Definition of Done for a UI story, **then** the headless-provable logic is unit-tested (the undo/redo stack semantics: push-on-mutate, redo-cleared-on-new-edit, restore round-trip, empty-stack guards; the two-scenario frame computation), the binary **launches and runs the event loop**, and the in-GUI click-through is a **documented partial** (human/AT-SPI, as 2.1–2.8). 4 CI gates green `--locked`.

## Tasks / Subtasks

> **Scope note / recommended split (see Open Question Q1):** Tasks 1–4 = **undo/redo** (the FR32 must-have). Tasks 5–6 = **scenario compare** (Phase-1, one alternate). The epic explicitly permits deferring scenario-compare *per story sizing* (epics.md:308). **Recommendation:** ship undo/redo first; if scenario-compare oversizes the story, split it into a follow-up and mark AC4/AC5 deferred. Decide with Guy before starting Task 5.

- [x] **Task 1 — In-memory undo/redo history (app crate, `state.rs`)** (AC: 1, 2)
  - [x] Add an undo/redo history of **`contract::Study` clones**, **per open study**, **in memory** (NOT persisted, NOT a diff log). Two stacks: `undo: Vec<Study>` (states *before* each mutation) + `redo: Vec<Study>`. **Realization note:** the architecture describes a `StudyState` snapshot stack, but the app has no in-memory `StudyState` (the journal is the source of truth; every edit does `mutate → put_study → re-read → push_form`). The honest, minimal realization is a stack of `Study` blobs — exactly the architecture's "snapshot stack, *simple clones because state is small*" intent. Do **not** build a new `StudyState` abstraction for this.
  - [x] Hold the history in a place scoped to the open study. Simplest: a `Rc<RefCell<UndoHistory>>` owned in `main.rs` (like `current_study`), reset on `on_open_study`. (Putting it on `JournalState` also works but `JournalState` is journal-lifecycle; a dedicated `UndoHistory` struct in `state.rs` keeps concerns separate.)
  - [x] **Snapshot point:** before each **persisting** mutation, push the *current persisted* study onto `undo` and **clear `redo`**. Every mutator already re-reads the study (`get_study`) at its start — capture that clone before the in-memory mutation. Cover ALL persisting mutations: `edit_cell`, `paste_column`, `set_not_available`, `set_review`, `unlock_all`, `set_judgment_field`, `set_forecast_low_option` (and the chart **drag commit**, which already routes through `set_judgment_field`). Use the shared rails (`mutate_cell`, `mutate_judgment`) as the choke points where possible; cover the standalone ones (`set_review`, `paste_column`, `unlock_all`) explicitly.
  - [x] **Bound the history** (e.g. cap at ~100 entries, drop oldest) so a long session doesn't grow unboundedly — `Study` clones are small but not free.
  - [x] **Live drag = ONE undo entry:** the chart's live `moved` previews never persist (Story 2.8), so they create no history; only the drag **commit** snapshots. One drag → one undoable step. Do not snapshot per `moved`.
- [x] **Task 2 — Undo / redo operations (app crate)** (AC: 1, 2)
  - [x] `undo`: if `undo` non-empty → push the *current* persisted study to `redo`, pop `undo` → `put_study(restored)` → re-read → `push_form`. `redo`: symmetric. Empty stacks → no-op (the UI also disables the control).
  - [x] Restore is a real `put_study` (bumps `logical_version` — fine; undo is a genuine write). Route through the existing read-only / no-journal / save-failure guards + neutral `MSG_*`; never a silent `.ok()`.
  - [x] After restore, `push_form` re-renders the WHOLE coherent frame (grids, judgment fields, §1 chart, §4 zone bar, verdict) from the restored study — deterministic by construction (same study → same `build_frame`). No partial frame.
  - [x] Expose `can_undo()` / `can_redo()` so the UI can disable the buttons; push them into `Studies` (`in-out property <bool> can-undo / can-redo`) on every `push_form`.
- [x] **Task 3 — Undo/redo UI affordance + keyboard (app crate, Slint)** (AC: 3)
  - [x] Add `undo` / `redo` callbacks on the `Studies` global and two controls in the study screen header (near "‹ Retour aux études"), disabled when `!can-undo` / `!can-redo`. Neutral labels (e.g. "Annuler" / "Rétablir" — nouns/verbs that are NOT in the banned buy/sell/hold list; verify against `core::method::BANNED_VERBS_FR`).
  - [x] Keyboard: a `FocusScope`/key handler at the study-screen level for **Ctrl+Z → undo**, **Ctrl+Y / Ctrl+Shift+Z → redo**. **Do NOT collide** with `editable_cell.slint`'s `key-pressed` Ctrl-chords (Ctrl+Space=not-available, Ctrl+Enter=review-cycle, Ctrl+Backspace=clear-✓, Ctrl+V=paste) — those intercept first while a cell has focus; the undo handler must sit where it catches the chord when no cell consumes it (study-screen FocusScope), and must not break cell editing. Confirm the chord routing.
  - [x] Wire `main.rs` `on_undo` / `on_redo` to the Task 2 operations. Reset the history on `on_open_study` (a fresh study starts with empty stacks).
- [x] **Task 4 — Tests + reset semantics (app crate)** (AC: 1, 2, 7)
  - [x] Headless `#[test]`s on the undo/redo history (pure Rust, no journal needed for the stack logic; use the `state.rs` test rig with a temp journal for the round-trip): push-on-mutate; **a new edit after an undo clears `redo`**; restore round-trip restores the exact `Study` (data cells + judgment + review tags); empty-stack guards; the history is **per-study** (opening another study resets it); a judgment edit then undo restores the prior judgment (FR32). Reuse the existing `state.rs` test helpers (temp `Journal`, `edit_cell`, `set_judgment_field`).
- [x] **Task 5 — Scenario compare: the two-frame engine view (app crate)** *(Phase-1, one alternate — confirm scope Q1)* (AC: 4, 5)
  - [x] Compute TWO coherent frames from the same data: **current** (the saved `Study.judgment`) and **alternate** (a clone with one or more judgment inputs changed, in memory, **not persisted**). Both via `engine::build_frame` (Cardinal Rule — all zone/U-D/return/verdict math in `core`). Build a `ScenarioCompareState` adapter exposing both placements' **zones (the 4 boundary prices + present-zone), U/D ratio, projected return, and verdict confidence** as formatted strings + the `confidence` gate per scenario.
  - [x] The alternate is **user-set** (typed exact value and/or a second judgment line), **never auto-placed/suggested** (FR33). Closing compare **discards** the alternate; the saved judgment is untouched (no `put_study` for the alternate).
- [x] **Task 6 — Scenario compare overlay (Slint) + neutral voice + colour budget** *(Phase-1)* (AC: 4, 5, 6)
  - [x] New `app/ui/components/scenario_compare.slint` (`ScenarioCompare`, PascalCase / snake_case file / kebab-case props). An **overlay** (the 2.5 confirm-overlay / 2.6 traceability-overlay shape — pinned, keyboard-dismissable, constant geometry) showing **current vs alternate** zones/U-D/return side by side.
  - [x] **Colour budget (AC5):** differentiate the two scenarios by ink value / dash / opacity / label / position — **never a 4th saturated hue**. Each scenario's zone bands still gate on its own verdict `confidence` (full/provisional-hatched/withheld-empty). Neutral microcopy only — "placement actuelle" / "placement alternatif", never "meilleur"/"optimal"/"essayez" (FR13/FR33; posture-scanned).
  - [x] Reduced-motion + constant geometry (AC6). Add the `ScenarioCompareState` struct to `state.slint` + re-export in `app.slint`; bump `posture.rs` floors for the new file + strings.
- [x] **Task 7 — Gates, posture floors, DoD** (AC: 7)
  - [x] Bump `posture.rs` floors to actual (current: `.slint` ≥ 19, `@tr` ≥ 140) for any new component/strings. New user-visible strings pass the banned-verb scan.
  - [x] 4 CI gates green `--locked`; `core`/`contract`/`persistence`/`ingestion`/`report`/`Cargo.lock`/`deny.toml`/`rust-toolchain` re-diff **unchanged** (no new dependency expected — undo is in-memory `Study` clones; scenario compare reuses `build_frame`). File List ⇄ git exact (issue #18).
  - [x] DoD: launch + run the event loop; in-GUI click-through = documented partial (human/AT-SPI). Don't mark `[x]` for a non-existent test.

### Review Findings (adversarial code review, 2026-06-14)

Blind Hunter + Edge Case Hunter + Acceptance Auditor (no layer failed). 0 decision-needed · 4 patch · 4 deferred · ~8 dismissed. Undo/redo two-stack core verified correct (redo-cleared-on-record, write-failure push-back, no present-state loss); no AC violation in the engine/scope.

**Patch (unchecked → to fix):**
- [x] [Review][Patch] Ctrl+Z / Ctrl+Y is swallowed by the soft-lock guard on a validated (✓) cell — the broad locked-cell `accept` (editable_cell.slint:168) catches every non-arrow key, so Ctrl+Z on a ✓ cell raises a spurious soft-lock notice instead of bubbling to undo. Exempt Ctrl/Cmd+Z/Y (reject → bubble). [app/ui/components/editable_cell.slint] *(High, AC3)*
- [x] [Review][Patch] Undo/redo while the scenario-compare overlay is open corrupts the comparison — Ctrl+Z fires `Studies.undo()` behind the scrim, mutating the underlying study, but `compare_study` (the overlay's "current" baseline) is never refreshed → stale current column. Guard `on_undo`/`on_redo` to no-op while `scenario-compare.visible`. [app/src/main.rs] *(High)*
- [x] [Review][Patch] Scenario-compare state isn't reset on study open — `compare_study` + the overlay can survive a study close/reopen (no defensive reset like `reset_undo`). Clear it in `on_open_study`. [app/src/main.rs] *(Medium)*
- [x] [Review][Patch] No-op edits record a phantom undo step + clear redo — re-selecting the active forecast-low option, re-typing a value that only differs by formatting, or re-setting the same review tag all `record(before)` a `before == after` snapshot (unlike `unlock_all`, which guards on `flipped > 0`). Guard `record` on `before != after` (`Study: PartialEq`). [app/src/state.rs] *(Low, AC1/AC2 quality)*

**Deferred (real, not now):**
- [x] [Review][Defer] The alternate input `TextField` binds the seed to `placeholder:` not `text:`, so the seeded est-high-EPS shows as a ghost hint, not a pre-filled editable value [scenario_compare.slint] — deferred, minor UX (GUI polish post-MVP)
- [x] [Review][Defer] The alternate placement varies only `est_high_eps` (Phase-1, Q4 default); current price / est-low-EPS / forecast-low option can't be varied [main.rs/engine.rs] — deferred, accepted Phase-1 scope (spec Q4)
- [x] [Review][Defer] The keyboard undo path (AC3) has no automated coverage and the form-wrapping `FocusScope` focus-order is unverified — manual AT-SPI pass needed (Ctrl+Z with no focus / a validated cell / the compare field) [study_screen.slint] — deferred, DoD human/AT-SPI partial
- [x] [Review][Defer] A blank / non-numeric / negative alternate input collapses to a calm em-dash/withheld column with no rejection signal (indistinguishable from a legitimately missing input) [main.rs] — deferred, calm-by-design

**Dismissed (noise/handled/false positive):** Blind Hunter "set_review not recorded" (FALSE — it routes through `mutate_cell` which records; the green test `undo_restores_a_review_tag_…` confirms); the Blind Hunter's own self-refuted items (RefCell-in-match-scrutinee, reset_undo-then-borrow, write-failure present-loss, cap eviction, `flipped==0` double-count — all verified safe); the macOS ⌘Y duplication (harmless).

## Dev Notes

### The architectural reality you must reconcile (read first)

The architecture (architecture.md:400–406, 512–516, 684) describes undo/redo as a **snapshot stack of immutable `StudyState` clones** in `app/src/state.rs`. **That `StudyState` type does not exist** — Stories 2.2–2.8 implemented a simpler, working model: **the SQLite journal is the source of truth**; every edit does `mutate → Journal::put_study → get_study → push_form` (see `app/src/state.rs` `mutate_cell`/`mutate_judgment` and `app/src/main.rs` `push_form`). There is **no in-memory domain-state snapshot** the UI derives from.

**Decision (encoded):** realize undo/redo as an in-memory stack of **`contract::Study` clones**, NOT by introducing a `StudyState` abstraction. This is faithful to the architecture's stated intent ("snapshot stack — *simple clones because state is small*; structural sharing only if needed") and fits the existing journal-as-truth model: snapshot the `Study` before a mutation, and `undo` = `put_study(previous_clone)` + `push_form`. `Study` is already `Clone` (used throughout `state.rs` tests and `get_study`). Do not over-engineer.

### Undo scope — what is and isn't undoable

- **Undoable (every persisting journal mutation):** data-cell edit (`edit_cell`), paste-a-column (`paste_column`), not-available toggle (`set_not_available`), review-tag change (`set_review`), unlock-all (`unlock_all`), judgment-field entry (`set_judgment_field`), forecast-low-option (`set_forecast_low_option`), and the §1 chart **drag commit** (routes through `set_judgment_field`). The UX names "grid edits, judgment-line moves, validation toggles" (ux-spec:970–977) — all of these are journal mutations, so snapshotting before every `put_study` covers them uniformly.
- **NOT undoable (separate concerns):** **fold/regime view-state** (persisted to app-config, not the journal — `study_view_state`, Story 2.3) and the live drag *previews* (never persisted). Window size/theme/label-set/number-format are app-prefs, not study edits. Keep these out of the undo stack.
- **Coherence (Story 1.11 / FR29):** each undo step is a **whole coherent `Study`** — inputs + review tags travel together; `build_frame` re-derives the verdict/zones from the restored study, so an undone frame is never "a fresh number beside an input it doesn't descend from." You get coherence for free because you restore the whole study and recompute via the single `build_frame` path.

### Files to open / touch (all in the `app` crate — `core`/`contract`/`persistence` pinned)

- `app/src/state.rs` — the mutation rail (`mutate_cell`, `mutate_judgment`, `set_review`, `paste_column`, `unlock_all`, `set_judgment_field`, `set_forecast_low_option`, `get_study`). Add the `UndoHistory` struct + snapshot-before-mutate. **Read the current state of every mutator before changing it** — they each have read-only / no-journal / save-failure guards you must preserve.
- `app/src/main.rs` — `push_form` (push `can-undo`/`can-redo`), `on_open_study` (reset history), the drag commit + all edit callbacks, plus new `on_undo`/`on_redo`. The history lives here as `Rc<RefCell<…>>` like `current_study`/`drag_study`.
- `app/ui/screens/study_screen.slint` — add undo/redo controls in the header (near the "‹ Retour" button at ~line 218) + a study-screen-level key handler for the Ctrl chords; **mount the scenario-compare overlay** (the traceability/confirm overlay pattern is at ~line 700).
- `app/ui/state.slint` — `Studies` global: add `can-undo`/`can-redo` props, `undo`/`redo` callbacks, and (Task 5/6) `ScenarioCompareState` + its open/close callbacks.
- `app/ui/app.slint` — re-export any new struct.
- `app/ui/components/editable_cell.slint` — **read** the `key-pressed` Ctrl-chord handling (Ctrl+Space/Enter/Backspace/V) so the new Ctrl+Z/Y handler doesn't collide.
- `app/src/viewmodel/engine.rs` — reuse `build_frame`, `zone_bar`, `verdict_badge`, `risk_computed`, `return_computed` for the scenario frames (Task 5). No new engine logic — scenario compare is two `build_frame` calls + formatting.
- `app/src/posture.rs` — bump floors for new strings/component.
- `app/ui/components/scenario_compare.slint` — **NEW** (Task 6).

### Established conventions (carry forward)

- **Cardinal Rule:** all zone/U-D/return/verdict math in `core` via `build_frame`; the app only clones studies, restores them, and formats outputs. No `Decimal`/enum crosses into `.slint` (formatted strings / floats / bools only).
- **No `.unwrap()`/`.expect()`** in non-test code; **no silent `.ok()`**; time/IDs only via the injected `Clock`/`IdGen`.
- **Colour budget:** saturated colour ONLY on the three zone hues; the compare overlay + undo controls spend **no hue** (ink + texture only). The `✓` validated-ink exception still never co-presents with zones.
- **Reduced-motion / constant geometry:** `Studies.reduced-motion` gates animations; overlays use clipped height-0 / opacity, never a re-layout.
- **Overlay pattern:** reuse the 2.5 confirm-overlay / 2.6 traceability-overlay shape (`if Studies.<x>.visible: Rectangle { background: Tokens.bg.with-alpha(0.6); TouchArea { clicked => close } … }`), keyboard-dismissable.
- **4 CI gates** `--locked`; `Cargo.lock`/`deny.toml` unchanged (no new dep); pinned surfaces re-diff empty; current app `#[test]` count **110** (you add to it).

### Recorded dev traps to avoid (from 2.4–2.8 reviews / issues)

1. **Redo not cleared on a new edit** — the classic undo bug. After an undo, any new mutation MUST clear the redo stack (Task 1). Test it (Task 4).
2. **Per-`moved` snapshots** — the live drag fires many `moved` events; only the **commit** persists, so only the commit snapshots. One drag = one undo step (Story 2.8 already makes `moved` non-persisting).
3. **Slint reserved names / chord collision** — don't name a prop `z`/`row`; the Ctrl+Z handler must not break `editable_cell`'s chords (2.5/2.6/2.8 all hit Slint chord/scope traps).
4. **Soft-lock symmetry** — undo/redo restore via `put_study` bypasses the per-cell soft-lock guard (it writes a whole study), which is correct (you're restoring a past coherent state, not editing a ✓ cell). But verify a restored ✓ cell stays ✓ and a restored `?` stays `?` (the review tags are part of the `Study` clone).
5. **File List ⇄ git exact** (issue #18); **don't mark `[x]` for a missing test** (2.6 fix).
6. **Stuck-flag class of bug (2.8 review P5)** — if you add any "compare open" / "history" flag, reset it on study open/close so a teardown can't leave it stuck.

### Project Structure Notes

- All work in `steadyinvest-app`. **No `core`/`contract`/`persistence` change expected** — `Study` is already `Clone`; `build_frame` already returns everything scenario compare needs. If you find you must touch a pinned crate, stop and reconsider.
- **No new dependency.** `Cargo.lock` + `deny.toml` re-diff identical.
- Slint/Rust naming: components `PascalCase`, `.slint` `snake_case`, props/callbacks `kebab-case`, Rust↔Slint callbacks `verb-noun` (`undo`, `redo`, `open-compare`).

### Tech stack (pinned)

- Rust workspace MSRV **1.96**; **Slint 1.16.1**; `rust_decimal 1.42` (+`maths`); `rusqlite 0.40` (`bundled`). Linux-only dev/CI. 4 gates `--locked`.

### References

- [Source: epics.md#Story 2.9] (657–669: BDD AC); UX-DR12 (224: scenario-compare overlay, Phase-1 one alternate); UX-DR25 (239: undo/redo everywhere, nothing destructive silent); Story 1.11 (517–530: coherence-frame invariant); Epic 2 (308: scenario-compare deferrable per sizing).
- [Source: prd.md] FR32 (723: undo, never destroys a saved input), FR6/FR29/FR31/FR33; FR68 (806–811: frozen-vs-recomputed verdict — a SEPARATE feature, do NOT conflate with scenario compare); NFR-C1/C3, NFR-P1/P2, NFR-R2/R4, NFR-U2.
- [Source: architecture.md] State & recompute / undo = snapshot stack (400–406, 512–516); `state.rs` responsibilities (684); content-addressed verdict + coherence (137, 412–414); Cardinal Rule (548–550); naming (484–486).
- [Source: ux-design-specification.md] Undo & Reversibility (970–977: covers grid edits/judgment moves/validation toggles); Scenario-compare overlay = component #5 (884–886: overlay or A/B, Phase-1 one alternate, Phase-2 richer); colour budget / differentiate by weight-brightness-dash (586–597); reduced-motion (1047–1048); undo a keyboard shortcut (1043).
- [Source: app/src/state.rs] the mutation rail to wrap. [app/src/main.rs] `push_form` + callbacks. [app/src/viewmodel/engine.rs] `build_frame`/`zone_bar`/`verdict_badge`/`risk_computed`/`return_computed`.
- GitHub issues (repo `guycorbaz/steadyinvest`): #18 (File-List⇄git), #25–#31 (2.8 chart deferrals — unrelated but live).

## Open Questions (for Guy / dev — non-blocking, defaults chosen)

- **Q1 — Scope: include scenario compare in 2.9, or split it?** The epic permits deferring scenario-compare per sizing (epics.md:308). Undo/redo (Tasks 1–4) is the FR32 must-have and is self-contained. Scenario compare (Tasks 5–6) is a new overlay component (~a chart's worth of UI). **Default/recommendation:** implement undo/redo fully now; **decide with Guy** whether to also ship the Phase-1 scenario-compare in 2.9 or split it into a follow-up (keeping the MVP moving, per Guy's 2026-06-14 "GUI polish post-MVP" stance). If split, mark AC4/AC5/AC6-compare-half deferred and file an issue.
- **Q2 — Undo history persistence:** the architecture's persisted unit is the frozen verdict, not the edit history. **Default:** undo/redo is **in-memory, per-open-study, not persisted across reopen** (reopening restores the *saved* state, not the undo history). Confirm Guy doesn't want cross-session undo.
- **Q3 — Keyboard bindings:** Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z are conventional but **not pinned** in the specs. **Default:** Ctrl+Z undo, Ctrl+Y + Ctrl+Shift+Z redo. Confirm no collision with the cell Ctrl-chords.
- **Q4 — How is the alternate placement set in compare?** By typing an exact alternate value, by a second draggable line, or both? **Default:** exact-value entry of the alternate judgment inputs (keyboard, simplest, NFR-U2-clean); a second draggable line can follow. (Only relevant if Task 5/6 are in scope.)

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- 4 CI gates green `--locked`: `cargo fmt --all --check` ✓ · `cargo clippy --all-targets --all-features --locked -- -D warnings` ✓ · `cargo test --all --locked` ✓ (app crate **117** `#[test]`, was 110: +5 undo/redo in `state.rs`, +2 scenario in `engine.rs`) · `cargo deny check` ✓.
- Pinned surfaces re-diffed unchanged (`core`/`contract`/`persistence`/`ingestion`/`report`/`Cargo.lock`/`deny.toml`/`rust-toolchain`); **no new dependency**.
- Event-loop smoke test: `timeout 10 cargo run` → exit 124 (ran until timeout, no panic) — the study-screen `FocusScope` (Ctrl+Z/Y), the undo/redo + Comparer controls, and the scenario-compare overlay all render.
- Posture floors bumped `.slint` 19→20, `@tr` 140→160; the new strings pass the banned-verb scan.

### Completion Notes List

- **Q1 (scope) resolved by Guy: full scope** — undo/redo AND scenario compare both shipped in 2.9.
- **Undo/redo realization (Q2 default confirmed):** an in-memory `UndoHistory { undo: Vec<Study>, redo: Vec<Study> }` on `JournalState` (NOT a new `StudyState` abstraction, NOT a diff log) — the architecture's "snapshot stack, simple clones" realized over the persisted `Study` blob, since the app keeps no in-memory domain state (journal = source of truth). Capped at 100. **Per open study, in-memory, never persisted across reopen** (reset on `on_open_study`).
- **Snapshot points:** the pre-mutation `Study` is cloned in the 4 choke-point mutators (`mutate_cell` covers edit/not-available/review; `mutate_judgment` covers judgment + forecast-low; `paste_column`; `unlock_all` — only when it flips ≥1 ✓) and recorded on a successful `put_study`, clearing the redo branch. The §1 chart's live `moved` previews never persist (Story 2.8), so **one drag = one undo step** (only the commit records).
- **Undo/redo ops:** `undo`/`redo` pop the target snapshot, write the whole prior/next `Study` back (a real guarded `put_study`), and move the present state to the opposite stack (reversible). Empty stacks → `Ok(false)` no-op. On a write failure the popped snapshot is pushed back (history never silently lost). After a step, `push_form` re-renders the whole coherent frame deterministically.
- **Coherence (1.11/FR29):** each step restores a whole `Study` and recomputes via the single `build_frame` path → inputs + review tags + verdict travel together; never a partial frame. FR32 proven: `undo_restores_a_judgment_edit` (a judgment undo restores the prior unset value) and `undo_restores_a_review_tag_without_destroying_the_value`.
- **UI/keyboard (AC3):** "↶ Annuler" / "↷ Rétablir" `ActionButton`s in the study header (disabled via `can-undo`/`can-redo`, mirrored on every `push_form`), and a study-screen-level `FocusScope` for **Ctrl+Z** (Ctrl+Shift+Z = redo) / **Ctrl+Y** that bubbles when no focused cell consumes it (the cell editor's Ctrl-chords reject other keys). No collision with the 2.4/2.5 cell chords.
- **Scenario compare (AC4–6, Phase-1 one alternate, Q4 default):** the alternate is set by **exact-value entry** of the alternate est-high-EPS (keyboard, NFR-U2-clean). `engine::scenario_compare(current, alternate, …)` builds **two independent `build_frame`s** (all math in `core`) and exposes each placement's forecast boundaries / present-zone / U/D / projected return / verdict confidence as formatted strings. The overlay (`scenario_compare.slint`) shows current vs alternate side by side. **Non-committing:** the alternate is an in-memory clone; closing discards it; the saved judgment is never `put_study`'d.
- **Colour budget (AC5):** the overlay is **ink-only** — current/alternate differ by label/position, **no fourth saturated hue**; each scenario's confidence word + present-zone label honour verdict integrity (test `scenario_compare_gates_confidence_per_scenario`: a missing load-bearing input → the alternate is `withheld`, never full).
- **Constant geometry / reduced-motion (AC6):** the overlay is the pinned 2.5/2.6 overlay shape, self-gating on `visible`, no animation (nothing to disable under reduced-motion); the undo/redo recolour reuses the existing §4 eased band (already reduced-motion-gated from 2.8).
- **Defensive (2.8 review P5 lesson):** the scenario-compare/undo state can't leave a stuck flag — `compare_study` clears on close; the undo history resets on open.
- **Honest DoD partial:** headless-provable logic fully unit-tested (undo/redo stack semantics + scenario two-frame computation, 7 new tests); binary launches + runs the event loop; in-GUI click-through (actual Ctrl+Z, the compare overlay interaction) left to human/AT-SPI, as 2.1–2.8.
- **Q3 (keybindings) confirmed:** Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z (not pinned in spec; chosen, no cell-chord collision).

### File List

**New**
- `app/ui/components/scenario_compare.slint` — the `ScenarioCompare` overlay (current vs alternate, ink-only, neutral microcopy, exact-value alternate entry).

**Modified**
- `app/src/state.rs` — `UndoHistory` struct + `Direction` enum + `UNDO_CAP`; `history` field on `JournalState` (+ 4 constructors); `reset_undo`/`can_undo`/`can_redo`/`undo`/`redo`/`step`; `record(before)` in `mutate_cell`/`mutate_judgment`/`paste_column`/`unlock_all`; **+5 undo/redo tests**.
- `app/src/main.rs` — `push_form` takes `&JournalState` and mirrors `can-undo`/`can-redo` (10 call sites updated); `on_open_study` resets the history; `on_undo`/`on_redo` callbacks; the `compare_study` cache + `on_open_compare`/`on_set_alternate`/`on_close_compare`.
- `app/src/viewmodel/engine.rs` — `scenario_outcome` + `scenario_compare` adapters; `ScenarioOutcome`/`ScenarioCompareState` imports; **+2 scenario tests**.
- `app/src/posture.rs` — floors bumped (`.slint` 19→20, `@tr` 140→160).
- `app/ui/state.slint` — `can-undo`/`can-redo` + `undo`/`redo`/`open-compare`/`set-alternate`/`close-compare` callbacks; `scenario-compare` property; `ScenarioOutcome`/`ScenarioCompareState` structs.
- `app/ui/app.slint` — re-export `ScenarioOutcome`/`ScenarioCompareState`.
- `app/ui/screens/study_screen.slint` — undo/redo/Comparer header controls; the study-screen `FocusScope` (Ctrl+Z/Y) wrapping the form; mount `ScenarioCompare`.

### Change Log

| Date | Change |
|------|--------|
| 2026-06-14 | Story 2.9 implemented (full scope, Guy's call): in-memory undo/redo of every persisting edit (snapshot stack of `Study` clones on `JournalState`; Ctrl+Z/Y + header controls; FR32 never-destroys-input by construction) + Phase-1 scenario compare (current vs one alternate placement via two `build_frame`s, ink-only overlay, non-committing). 4 gates green `--locked`; app tests 110→117. Status → review. |
| 2026-06-14 | Adversarial code review (3 layers, 0 failed): 0 decision · 4 patch · 4 defer · ~8 dismiss; undo/redo two-stack core verified correct. All 4 patches applied — (P1) Ctrl+Z/Y now bubbles past the soft-lock guard on a ✓ cell instead of raising a spurious notice; (P2) undo/redo no-op while the scenario-compare overlay is open (no stale baseline); (P3) compare state reset on study open; (P4) no-op edits no longer record a phantom undo step / clear redo (`before != after` guard, `Study: PartialEq`). 4 gates re-green `--locked`; app tests 117. Status → done. |

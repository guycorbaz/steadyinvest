# Story 2.10: Decision rationale capture

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to record *why* I reached a decision,
so that the journal holds my reasoning, not just numbers.

## Acceptance Criteria

(From epics.md §Story 2.10, lines 671–682. BDD, verbatim intent.)

1. **Given** an open study, **when** I write (or edit) a **decision rationale** — a free-text "why I judged this way" note — **then** it is stored as a **first-class field on the study** (`contract::Study.rationale`) and persisted with the study snapshot via the existing `put_study` rail (FR49, FR51 capture). Clearing it back to empty stores `None` (never an empty-but-present string surprise — empty text → `None`).
2. **Given** a study with a saved rationale, **when** I **reopen** it, **then** the rationale is restored and **shown** in the form (FR49; ux-spec "Reopen → full state restored: judgment, provenance, rationale").
3. **Given** Story 2.9 undo/redo, **when** I edit the rationale and then **undo**, **then** the prior rationale is restored — a rationale edit is "any edit" and goes through the same snapshot rail (one undo step per commit; never destroys prior text).
4. **Given** the neutral-voice rule (FR13), **then** the field's **label and placeholder microcopy are neutral** (fact-stating, no banned buy/sell/hold verb — scanned by the posture gate); the **user's own typed content is NOT scanned** (it's the user's words, not a system message).
5. **Given** persistence, **then** saving the rationale is **atomic** (one `put_study` transaction, `logical_version` bumped) and uses the read-only / no-journal / save-failure guards (a neutral notice on refusal, never a silent `.ok()`).
6. **Given** the Definition of Done for a UI story, **then** the round-trip is unit-tested (set rationale → reopen restores it; clear → `None`; undo restores prior), the binary launches and runs the event loop, and the in-GUI click-through is a documented partial (human/AT-SPI, as 2.1–2.9). 4 CI gates green `--locked`.

## Tasks / Subtasks

- [x] **Task 1 — Persist & undo the rationale (app crate, `state.rs`)** (AC: 1, 3, 5)
  - [x] Add `JournalState::set_rationale(study_id, text: Option<String>) -> Result<(), String>`. Reuse the **`mutate_judgment`-style rail** (or a tiny sibling): read-only / no-journal guards → re-read the study → set `study.rationale` → `put_study` → record the pre-mutation snapshot **only on a real change** (`before != study`, the 2.9 `record` guard) so undo covers it and a no-op re-save creates no phantom step.
  - [x] **Empty → `None`:** trim the incoming text; an empty/whitespace-only string stores `rationale = None` (parallels the "cleared input → None, never 0/empty" rail). A present rationale stores `Some(trimmed)` (decide whether to trim trailing whitespace only — keep it simple: `trim()` and `None` if empty).
  - [x] Headless tests: set → `get_study` shows it; reopen (new `JournalState` on the same temp journal) restores it; clear → `None`; **undo** after a rationale edit restores the prior value (reuse the `state.rs` undo test rig).
- [x] **Task 2 — Surface the rationale in the form (app crate, viewmodel + main.rs)** (AC: 1, 2)
  - [x] Push the current rationale into a `Studies` string property on every `push_form` (restored on reopen). It is study-level (NOT per-cell, NOT per-judgment-line) — read `study.rationale.clone().unwrap_or_default()`.
  - [x] Wire a `set-rationale(string)` callback in `main.rs` → `state::set_rationale` → re-read + `push_form` (refreshing the undo flags, as the other edit callbacks do). Mirror the keep-input-on-refusal pattern (don't clobber an in-progress edit; commit on focus-out / explicit commit).
- [x] **Task 3 — Rationale note UI (Slint)** (AC: 1, 2, 4)
  - [x] Add a study-level **note field** in `study_screen.slint`, near the judgment-completion area (after §5 / by the verdict — NOT inside the §1–§5 grid). A **multi-line** plain-text area (a bordered `Rectangle` wrapping a `TextInput { single-line: false; wrap: word-wrap; }`, or extend `TextField` for multi-line) with a neutral `@tr` label (e.g. "Justification de la décision") and a neutral placeholder (e.g. "Pourquoi cette conclusion"). Commit on focus-out / accepted → `Studies.set-rationale(text)`. Reads `Studies.rationale` for its value (re-seed only when not focused, the 2.4/2.6 keep-input discipline).
  - [x] **No colour spent** (ink only); constant geometry; reduced-motion-safe (no animation needed). Plain text only — NO rich text, NO markdown.
- [x] **Task 4 — Gates, posture floors, DoD** (AC: 4, 6)
  - [x] Bump `posture.rs` floors for the new `@tr` strings (current: `.slint` ≥ 20, `@tr` ≥ 160). The new label/placeholder pass the banned-verb scan; the **rationale value itself is user data and is NOT registered/scanned**.
  - [x] 4 CI gates green `--locked`; `core`/`contract`/`persistence`/`ingestion`/`report`/`Cargo.lock`/`deny.toml`/`rust-toolchain` re-diff **unchanged** (no new dependency; `Study.rationale` already exists — **NO schema bump**). File List ⇄ git exact (issue #18).
  - [x] DoD: launch + run the event loop; in-GUI click-through = documented partial (human/AT-SPI). Don't mark `[x]` for a non-existent test.

## Dev Notes

### The field already exists — do NOT bump the schema

`contract::Study.rationale: Option<String>` is already in the v1 schema with `#[serde(default)]` (contract/src/study.rs:94; `Study::new` sets it `None`). It is the **first-class FR49 field**. It serializes inside the study JSON `payload` (persistence/src/studies.rs `put_study`), so there is **no DDL / migration / `schema_version` change** for this story. `core`/`contract`/`persistence` are pinned surfaces — re-diff them empty.

### This is a compact, study-level, plain-text note

- **One** study-level rationale (FR49 "on studies", singular). NOT per-cell, NOT per-judgment-line, NOT per-transaction (transaction/sell rationale is a later epic). NO rich text, NO AI (the AI clerk is read-only by construction — Epic 8 — and never authors/suggests this).
- **NOT** FR50 (projection-vs-actual visual compare) — that's Epic 5. This story is only *capture* + *show on reopen*.
- The user's typed rationale is **free text**: no spec'd character limit (keep it unbounded or a generous soft cap — implementation choice, see Q1), and it is **exempt from the banned-verb scan** (it's the user's words). Only the system-supplied **label/placeholder** must be neutral and posture-scanned.

### Reuse the established rails (the heart of this story is small)

- **Persist:** the `state.rs` mutation rail — model it on `mutate_judgment` (read-only/no-journal/save-failure guards → re-read → set one field → `put_study`), and **record an undo snapshot on real change** (`before != study`, the Story-2.9 `record` guard) so AC3 (undo) works for free and a no-op doesn't push a phantom step.
- **Surface:** `main.rs::push_form` already re-renders the whole form from the re-read study on open + after every edit; add `studies.set_rationale(study.rationale.clone().unwrap_or_default())` there (it carries `&JournalState` now, Story 2.9). A `set-rationale` callback mirrors `on_set_judgment` (parse → mutate → re-read → push_form).
- **Keep-input discipline (2.4/2.6/2.8):** the note's `TextInput` re-seeds from `Studies.rationale` only when it does NOT have focus, so an in-progress edit is never clobbered; commit on focus-out / `accepted`.
- **Undo flags:** `push_form` already mirrors `can-undo`/`can-redo` — nothing extra needed; the rationale edit records like any other mutation.

### Files to open / touch (all in the `app` crate — contract/core/persistence pinned)

- `app/src/state.rs` — add `set_rationale` next to `set_judgment_field` (same rail + undo `record`); add headless tests near the 2.9 undo tests.
- `app/src/main.rs` — `push_form` pushes `rationale`; new `on_set_rationale` callback.
- `app/ui/state.slint` — `Studies.rationale` property + `set-rationale(string)` callback.
- `app/ui/screens/study_screen.slint` — the note field UI (after §5 / near the verdict).
- `app/src/posture.rs` — bump floors for the new `@tr` label/placeholder.
- (Optional) `app/ui/components/text_field.slint` — if you add a multi-line variant; otherwise a small inline multi-line `TextInput` in `study_screen.slint`.

### Established conventions (carry forward)

- Cardinal Rule: no calc here (rationale is data, not computed). No `.unwrap()`/`.expect()` in non-test code; no silent `.ok()`; time/IDs via the injected `Clock`/`IdGen` (the rationale itself needs no timestamp — it rides in the study snapshot, versioned by `logical_version`).
- Money/values cross as formatted strings; the rationale crosses as a plain `string` (user text). No `Decimal`/enum into `.slint`.
- Colour budget: the note spends NO colour (ink only). Neutral microcopy (FR13) for the label/placeholder.
- 4 CI gates `--locked`; `Cargo.lock`/`deny.toml` unchanged (no new dep); current app `#[test]` count **117** (you add to it).

### Recorded traps to avoid (2.4–2.9)

1. **Empty vs None** — an empty rationale is `None`, not `Some("")` (the project's "absence ≠ empty value" rail).
2. **No-op snapshot** (Story 2.9 P4) — record undo only on `before != study`, so re-saving an unchanged rationale doesn't push a phantom undo step / clear redo.
3. **Keep-input-on-refusal** — re-seed the note from the model only when unfocused (don't clobber typing).
4. **Posture: scan the label, not the content** — register the new `@tr` label/placeholder for the banned-verb gate; do NOT register or scan the user's rationale text (it's user data, and the gate is for system strings).
5. **File List ⇄ git exact** (issue #18); don't mark `[x]` for a missing test.

### Project Structure Notes

- All work in `steadyinvest-app`. **No `contract`/`core`/`persistence` change** — the field pre-exists. No new dependency. If you reach for a schema bump or a new crate, stop — this story doesn't need either.
- Slint/Rust naming: components `PascalCase`, `.slint` `snake_case`, props/callbacks `kebab-case` (`set-rationale`, `rationale`).

### Tech stack (pinned)

- Rust workspace MSRV **1.96**; **Slint 1.16.1**; `rusqlite 0.40` (`bundled`). Linux-only dev/CI. 4 gates `--locked`.

### References

- [Source: epics.md#Story 2.10] (671–682: BDD AC); FR mapping (262: FR49 → Epic 2, FR50 → Epic 5, FR51 → Epic 1+2).
- [Source: prd.md] FR49 (757: decision rationale, first-class field), FR51 (763: durable time-series incl. rationale); FR13 (neutral voice — system strings only); FR50 (758: projection-vs-actual — Epic 5, NOT here).
- [Source: ux-design-specification.md] judgment-completion "optional rationale note" (line 430); "Reopen → full state restored: judgment, provenance, rationale" (line 121); "Capture rationale note" flow node (line 721).
- [Source: contract/src/study.rs:94] `rationale: Option<String>` (`#[serde(default)]`, already in schema). [persistence/src/studies.rs] `put_study` (whole-study JSON payload, atomic, bumps `logical_version`).
- [Source: app/src/state.rs] `mutate_judgment` / `set_judgment_field` rail + the Story-2.9 `UndoHistory.record` (`before != study` guard). [app/src/main.rs] `push_form` (now `&JournalState`) + `on_set_judgment`.

## Open Questions (for Guy / dev — non-blocking, defaults chosen)

- **Q1 — Character limit?** No limit is specified in any artifact. **Default:** no hard limit (a generous note); rely on the DB/JSON to hold it. Confirm if a cap is wanted.
- **Q2 — Placement & always-visible vs collapsible?** **Default:** a study-level note block after §5 (near the verdict / judgment-completion area), always visible (not folded), so reopening shows it without a click. Confirm.
- **Q3 — Multi-line?** **Default:** multi-line plain-text area (a "why" note wants more than one line). Confirm single-line is not preferred.
- **Q4 — Commit timing?** **Default:** commit on focus-out and on an explicit action (no per-keystroke persistence — one undo step per committed edit). Confirm.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- Initial `cargo test` after dev: 120/121 — `posture::tests::ui_tr_strings_are_neutral_no_banned_verb`
  failed (floor over-bumped to 165; actual `@tr` count 162). Corrected floor 165 → 162. Re-run: 121/121.

### Completion Notes List

- `set_rationale` routed through a NEW `mutate_study` rail (mirrors `mutate_judgment`): read-only /
  no-journal / save-failure guards → re-read → mutate whole `Study` → `put_study` → undo snapshot
  recorded only on `before != study` (no phantom step). Empty/whitespace → `None` (never `Some("")`).
- `push_form` pushes `Studies.rationale`; `on_set_rationale` callback mirrors `on_set_judgment`.
- `RationaleNote` is a faithful copy of the `JudgmentField` rail (re-seed only when unfocused; commit
  on focus-out when `text != value`); ink-only, multi-line, neutral `@tr` label/placeholder.
- posture floors bumped (`@tr` 160 → 162, `.slint` 20 → 21). User content is NOT scanned (FR13).
- app `#[test]` count 117 → 121 (+4).
- 3-layer adversarial code-review: Acceptance Auditor PASS AC1–6; 0 blocking patch, 5 dismiss
  (consistent with established rails), 2 defer → GitHub issues #32 + #33.
- AC6 in-GUI click-through left as a documented partial (human/AT-SPI sandbox), as 2.1–2.9.

### File List

- `app/src/state.rs` — `set_rationale` + `mutate_study` rail; 4 headless tests
- `app/src/main.rs` — `push_form` pushes rationale; `on_set_rationale` callback
- `app/src/posture.rs` — `@tr`/`.slint` floors bumped for the new label/placeholder
- `app/ui/state.slint` — `Studies.rationale` property + `set-rationale(string)` callback
- `app/ui/screens/study_screen.slint` — instantiates `RationaleNote` after §5
- `app/ui/components/rationale_note.slint` — NEW multi-line ink-only note component

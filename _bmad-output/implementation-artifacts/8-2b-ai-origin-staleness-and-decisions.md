# Story 8.2b: AI origin, staleness and decisions (headless)

Status: review

<!-- Created 2026-09-27 by create-story (ultimate context engine). Branch: feat/8-2b-ai-origin-decisions
     (main with 8.1 #257 and 8.2a #258 merged). No UI in this story — the inbox is 8.5a/8.5b. -->

## Story

As the developer,
I want validated drafts recorded as owner entries with a visible AI origin, and decisions applied
atomically through the app state,
so that a decision can never be half-applied, lost by a later save, or confused with a provider value.

## Acceptance Criteria

1. **Types (arch A6).** `contract` gains `DraftOrigin { client, model }` (a draft's submitter), distinct
   from the existing `AiOrigin { draft_id, client, model, validated_at }` (`contract/src/ai.rs`, 8.1).
   `Provenance` gains `#[serde(default, skip_serializing_if = "Option::is_none")] ai_origin:
   Option<AiOrigin>` — **no new `Source` variant**. `Judgment` gains one additive
   `#[serde(default, skip_serializing_if = "AiPlaced::is_empty")] ai_placed: AiPlaced` holding one
   `Option<AiOrigin>` per draftable judgment field. `Note.ai_origin` stays `Option<AiOrigin>` (8.1).
   A study with no AI mark serializes **byte-identically** to today (pinned corpus JSON and export
   hashes unchanged; no re-capture).
2. **Draftable fields, one place (D6).** `contract` enumerates every draftable field with its stable
   key, its target kind (cell / judgment), its unit, and — for `forecast_low_option` — its variant
   names: the seven study-grid cell fields and the judgment fields **except `current_price` and
   `ttm_eps`**. One parser turns a proposed text into a value for that field or a typed problem
   (not a number / not an option). 8.3 and 8.4 reuse it verbatim.
3. **Validation = owner entry with `?` (O5, D5).** A validated cell draft becomes `Source::Manual`,
   review **`?` set explicitly** in every case — untagged cell, `✓` cell (no soft-lock refusal), value
   unchanged — with `Provenance.ai_origin = Some(AiOrigin{…})`. A validated judgment draft sets the
   field and its `ai_placed` slot. A validated note draft appends a `Note` whose `ai_origin` is set.
   The reconciliation tests are extended with an AI-origin case: manual wins, the divergent provider
   value is parked, `ai_origin` survives (FR17, FR20, FR22, FR74, NFR-R4).
4. **Marks clear (A6).** The owner's next value edit of that cell clears `ai_origin` (the edit rail
   replaces the provenance). **Any** write to a judgment field clears that field's `ai_placed`
   (`apply_judgment_field`, `set_forecast_low_option`, and any other writer of a draftable field).
   A review-only toggle (`set_review`) and a reconciliation keep the cell's `ai_origin`.
5. **Fingerprint (A7).** `contract::draft_fingerprint(study, target, method_version) -> Option<String>`
   with the explicit encoding of Dev Notes §3 (never the serde form). `None` = **target gone**.
   Tests: a refresh of the EPS history marks a judgment draft stale; a `METHOD_VERSION` change marks
   it stale; a parked divergent provider value marks a cell draft stale; a value-identical re-stamp
   (`restamp_if_predated`, provider or pending provenance) does **not**; a `Money` scale change
   (`3` → `3.0`) does not.
6. **Draft state on read.** A pure function classifies a pending draft as **fresh / stale / target gone**
   (reason: year no longer in the study · study deleted). A target-gone draft cannot be validated
   (rejection only).
7. **Decision through the app state (D3, A8).** `JournalState::decide_draft(study_id, draft_id,
   decision)` requires `study_id` to be the study the undo history belongs to (the open study —
   opening it first is the wiring's job in 8.5b); applies the draft to a fresh read of that study;
   records the pre-decision snapshot **with the draft id** on the undo stack; and calls
   `persistence::Journal::decide_draft`, which writes the study upsert (with its FR51 snapshot) **and**
   the draft's `status`, `decided_at`, `stale_at_decision`, `edited_before_validation` in **one**
   transaction with **one** `logical_version` bump. Inside that transaction it re-checks that the
   draft is still `pending` and that the stored study still equals the one the decision was computed
   from; a change since → refused, nothing written. A rejection writes only the draft row.
8. **Confirmation re-check (A7).** The decision carries the fingerprint the owner saw
   (`seen_fingerprint`); if the current fingerprint differs, the rail refuses with the §3.3 wording
   « La cible a encore changé depuis votre confirmation ; rien n'a été enregistré. La proposition est
   affichée de nouveau. » and writes nothing. `stale_at_decision = (seen ≠ base_fingerprint)`.
9. **Edited before validation.** `Decision::ValidateEdited(value)` saves the owner's value **without**
   AI origin (plain manual entry, review `?` like any decision — D5) and records the draft
   `validated` with `edited_before_validation = 1`.
10. **Crash safety (NFR-R2).** A failure injected between the study write and the draft update (test
    trigger, Dev Notes §7) leaves both unchanged and `logical_version` unmoved.
11. **No lost update.** After a decision, every later app write re-reads the study; tests prove the
    applied value survives: an edit of another cell, a stubbed refresh (manual wins), an unrelated
    judgment edit, a notes edit.
12. **Undo / redo (FR32, FR77).** Undoing a validation restores the prior study **and** sets the draft
    to `validated_undone` in one transaction; redoing re-applies it and sets `validated` again, in one
    transaction; a new edit after an undo clears redo (the draft stays `validated_undone`). The
    validation stays undoable while its study remains open; opening a study resets the history.
    A rejection is not on the undo stack.
13. **Record kept (FR77).** A validated, undone or rejected draft stays in `ai_drafts` with its outcome
    and decision facts; the 8.2a CHECKs hold for every transition.
14. **Refusals (8.0 spec §3.3, verbatim).** Read-only dossier, study gone, study archived, save failure
    with cause, changed since confirmation, target gone, already decided — each a named French
    message, nothing written; counted in `USER_FACING_MESSAGES` with the delta stated.
15. **Gates.** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
    `cargo fmt --check` green; no `SCHEMA_VERSION` bump; no UI change (no `@tr` delta).

## Tasks / Subtasks

- [x] **T1 — Contract types (AC 1)**
  - [x] T1.1 `contract/src/ai.rs`: add `DraftOrigin { client: String, model: String }` (serde, Eq);
        re-export from `lib.rs`. Update the module doc (drop « arrives with Story 8.2b »).
  - [x] T1.2 `Provenance.ai_origin` (additive, `default` + `skip_serializing_if`). Update **every**
        `Provenance { … }` literal with `ai_origin: None` (list in Dev Notes §9). Consider a
        `Provenance::new(source, lv, ts, hash)` helper only if it removes churn without hiding intent —
        do not add `Default` (a default provenance would be a lie).
  - [x] T1.3 `AiPlaced` struct (`Default`, `is_empty()`), one `Option<AiOrigin>` per draftable judgment
        field (Dev Notes §2), field `Judgment.ai_placed` (additive, `default` + `skip_serializing_if`).
        Update every `Judgment { … }` **construction** (not patterns) with `ai_placed:
        AiPlaced::default()`.
  - [x] T1.4 Tests: legacy JSON without the fields reads `None` / empty; a study without AI marks
        serializes byte-identically to a pinned string; round-trip with marks.
- [x] **T2 — Draftable fields + parser (AC 2)** in new `contract/src/draftable.rs`
  - [x] T2.1 `DraftField` enum (16 variants, Dev Notes §2) with `key()`, `from_key()`, `kind()` (cell
        / judgment), `unit()` (`DraftUnit`), `options()` for `ForecastLowOption`.
  - [x] T2.2 `DraftField::parse_value(&self, text) -> Result<DraftValue, DraftValueProblem>`:
        decimals via `Decimal::from_str_exact` on the trimmed text, **plain notation only** (no
        exponent, no `+`, no locale separators, no thousands); `-0` → `0`; the enum by exact snake
        name. `DraftValue::{Number(Money), Option(ForecastLowOption)}`.
  - [x] T2.3 `DraftTarget` ↔ `DraftField` helper: `DraftField::of_target(&DraftTarget) ->
        Option<(DraftField, Option<i32>)>` (unknown key → `None`).
  - [x] T2.4 Tests: every key round-trips; `current_price` / `ttm_eps` are not draftable; units; parse
        accepts `12.5`, `-3`, `0.001`; refuses `1e3`, `+2`, `1'000`, `1,5`, ``, `abc`; option names.
- [x] **T3 — Fingerprint + draft state (AC 5, 6)** in `contract/src/draftable.rs` (or `fingerprint.rs`)
  - [x] T3.1 `draft_fingerprint(study: &Study, target: &DraftTarget, method_version: &str) ->
        Option<String>` per Dev Notes §3 (SHA-256 hex, `fp1:` prefix).
  - [x] T3.2 App-side pure classifier (`app/src/state/drafts.rs`): `draft_freshness(study:
        Option<&Study>, record: &DraftRecord) -> DraftFreshness { Fresh, Stale, TargetGone(GoneReason) }`
        using `steadyinvest_core::METHOD_VERSION`; note drafts: fresh unless the study is gone.
  - [x] T3.3 Tests (AC 5 list + scale change + coverage change + review-only toggle does **not** stale).
- [x] **T4 — Rails that write the marks (AC 3, 4)**
  - [x] T4.1 `contract::Cell::validated_from_draft(&self, value: Option<Money>, provenance: Provenance)
        -> Cell` = `edited(value, provenance)` then `review = Review::ToReview` — documented as **the**
        D5 exception to invariant 2b; `contract/tests/cell_rails.rs` cases: untagged → `?`, `✓` same
        value → `?`, `✓` new value → `?`, pending cleared, coverage Present.
  - [x] T4.2 `apply_judgment_field` and `set_forecast_low_option` clear the field's `ai_placed` slot;
        a grep-backed test lists every writer of a draftable judgment field (refresh writes only
        `current_price`/`ttm_eps` — assert they are not draftable).
  - [x] T4.3 Reconciliation test extension (contract + `app/src/state/tests.rs` refresh case): manual
        AI-origin cell + divergent fetch → value kept, pending parked, `ai_origin` kept; + agreeing
        fetch → pending cleared, `ai_origin` kept.
- [x] **T5 — Persistence decision writes (AC 7, 10, 12, 13)** in `persistence/src/drafts.rs`
  - [x] T5.1 `Journal::get_draft(id) -> Result<Option<DraftRecord>>` (same corruption rule as
        `list_drafts`); `Journal::study_status(id) -> Result<Option<String>>` if no existing getter fits.
  - [x] T5.2 `Journal::decide_draft(&mut self, d: DraftDecisionWrite<'_>) -> Result<()>` (Dev Notes §5):
        one transaction: draft still `pending` (else `Error::DraftNotPending`), optional study write
        guarded by "stored payload re-serialized == expected-before serialized" (else
        `Error::StudyChangedSinceRead`), study upsert + FR51 snapshot (factor the body of
        `put_study_with_history` into a `tx`-level helper — do **not** duplicate the dedup logic),
        draft `UPDATE … WHERE id = ? AND status = 'pending'` (assert 1 row), one
        `bump_logical_version`.
  - [x] T5.3 `Journal::step_draft_decision(&mut self, study: &Study, draft_id, to: DraftStatus, now)`
        for undo (`validated → validated_undone`) and redo (`validated_undone → validated`): same
        transaction shape, `from` status checked, `decided_at = now`.
  - [x] T5.4 New `Error` variants (+ `ErrorKind` mapping, likely `Other`/`Missing`): `DraftNotFound`,
        `DraftNotPending { status }`, `DraftStatusMismatch { expected, found }`,
        `StudyChangedSinceRead`.
  - [x] T5.5 Tests (`persistence/tests/drafts.rs`): validate / reject / undo / redo transitions obey
        the 8.2a CHECKs; version bump exactly +1 per decision (the trigger fires on INSERT only);
        a non-pending draft is refused; a changed study is refused; crash injection (Dev Notes §7).
- [x] **T6 — App state rail (AC 7–9, 11, 12, 14)** in new `app/src/state/drafts.rs`
  - [x] T6.1 `UndoHistory` entries become `UndoStep { study: Study, draft: Option<Uuid> }`;
        `record(before)` unchanged for every existing caller; `record_draft(before, draft_id)`;
        `step()` routes a draft step through `step_draft_decision` (undo → `ValidatedUndone`, redo →
        `Validated`), the displaced present keeps the same draft id; failure pushes back (existing
        rule). `UndoHistory` also records its owner study (`reset_undo_for(Some(id))` at the open
        sites, `None` for the demo) — keep `reset_undo()` as a thin wrapper if call sites prefer.
  - [x] T6.2 `pub enum Decision { Validate { seen_fingerprint: Option<String> }, ValidateEdited {
        seen_fingerprint: Option<String>, value: String }, Reject }` and `JournalState::decide_draft
        (study_id, draft_id, decision) -> Result<(), String>`: read-only → `MSG_DECISION_READ_ONLY`;
        study ≠ history owner → `MSG_NO_STUDY_OPEN` (8.1); draft read / status / archived / gone /
        target gone / changed-since-confirmation guards; build the `after` study (cell via
        `validated_from_draft`, judgment via the field setter + `ai_placed`, note via append with
        `ai_origin`); `AiOrigin { draft_id, client, model, validated_at: clock.now() }`; call
        `Journal::decide_draft`; on success `history.record_draft(before, draft_id)` (validation only).
        Draft **study** kind: `Reject` supported; `Validate` → refused here with a clear internal
        error (8.7 owns creation).
  - [x] T6.3 Map persistence errors: `DraftNotPending` → `MSG_DECISION_ALREADY_DECIDED`;
        `StudyChangedSinceRead` → `MSG_DECISION_CHANGED`; others → `MSG_DECISION_SAVE_FAILED` with the
        typed cause (`persist_cause`), never a silent `.ok()`.
  - [x] T6.4 Messages (Dev Notes §6) in `messages.rs`, added to `USER_FACING_MESSAGES`; target-gone
        reasons in a scanned inventory; posture count delta stated in `posture.rs`.
  - [x] T6.5 Tests (`app/src/state/tests.rs`, FixedClock/FixedIdGen, seeded drafts through a test-only
        persistence insert helper — 8.3 owns the real writer): each AC 3/7/8/9/11/12/14 case;
        undo/redo transitions and record; lost-update suite (AC 11); a decision on a study that is not
        the history owner refused.
- [x] **T7 — Record + closure**
  - [x] T7.1 Story record (Dev Agent Record, File List, deltas); sprint-status `8-2b…: review`.

## Dev Notes

### 1. Where this sits

8.2a built the table and its CHECKs (`persistence/src/schema.rs` v8, `contract/src/draft.rs`,
`persistence/src/drafts.rs` read side). 8.2b adds the **decision** side and the AI marks, headless.
8.3 (`McpAccess`) will be the only **inserter** of drafts and reuses T2 (fields/parser) and T3
(fingerprint at submission → `DraftPayload.base_fingerprint`). 8.5a/b build the inbox on T3.2 and T6.
Owner rule (2026-09-27): **not in production** — no `SCHEMA_VERSION` bump, additive serde only.

### 2. Draftable fields (AC 2) — keys are the contract's serde field names

| key | kind | unit (`DraftUnit`) | note |
|---|---|---|---|
| `sales` | cell | `Amount` (native currency, **absolute** — the grid shows millions, storage is absolute, Issue #117) | load-bearing |
| `eps` | cell | `PerShare` | load-bearing |
| `high_price` | cell | `Price` | load-bearing |
| `low_price` | cell | `Price` | load-bearing |
| `dividend_per_share` | cell | `PerShare` | optional slot |
| `pre_tax_profit` | cell | `Amount` (absolute) | optional slot |
| `book_value_per_share` | cell | `PerShare` | optional slot |
| `estimated_high_eps` | judgment | `PerShare` | |
| `estimated_low_eps` | judgment | `PerShare` | |
| `projected_sales_growth_pct` | judgment | `Percent` (`12.5` = 12.5 %) | |
| `projected_eps_growth_pct` | judgment | `Percent` | |
| `judged_avg_high_pe` | judgment | `Ratio` (P/E multiple) | |
| `judged_avg_low_pe` | judgment | `Ratio` | |
| `recent_severe_low` | judgment | `Price` | option (c) input |
| `present_full_year_dividend` | judgment | `PerShare` | |
| `forecast_low_option` | judgment | `Option` — `avg_low_pe_times_eps` · `avg_low_price_last5y` · `recent_severe_low` · `dividend_supported` | enum |

**Not draftable (D6):** `current_price`, `ttm_eps` (provider market facts; refresh writes them).
The app maps these keys to its own wire keys — cells: `high_price→"a"`, `low_price→"b"`,
`eps→"c"`, `dividend_per_share→"f"`, `sales→"sales"`, `pre_tax_profit→"pretax"`,
`book_value_per_share→"book"` (`app/src/viewmodel/entry.rs:55-61`, `set_cell` :125); judgments —
`app/src/state/cells.rs:640` (`apply_judgment_field`: `est_high_eps`, `high_pe`, `eps_growth`, …).
Add one mapping function next to each; never let a draft key reach `set_cell` unmapped.
`AiPlaced` has exactly the nine judgment slots above (not `current_price`/`ttm_eps`).

### 3. Fingerprint encoding (AC 5) — normative, version `fp1`

Build a UTF-8 byte string line by line (`\n` terminated), then `"fp1:" + sha256_hex(bytes)`
(`contract::sha256_hex`, `contract/src/export.rs:53`). Decimal text = `Decimal::normalize()`
then `to_string()` (so `3` and `3.0` agree — the Money scale caveat, `contract/src/money.rs:12`);
absent = `-`.

- Header: `fp1`
- **Cell target** `{fiscal_year, field}`: `cell`, `{fiscal_year}`, `{key}`, then for the cell:
  `v:{value|-}`, `s:{provider|manual|derived}`, `c:{present|to_fill|not_available_accepted}`,
  `p:{-|none|<decimal>}` (`-` no pending; `none` a pending whose value is absent). **Never** the
  timestamp, digest, freshness or review of the cell or of the pending (a re-stamp changes those —
  `app/src/state/refresh.rs:572` `restamp_if_predated`).
  - Year row absent → `None` (**target gone**). An optional slot that is `None` (never touched)
    encodes as the single line `slot:absent` instead of the four cell lines, so an absent slot and a
    materialized empty cell differ.
- **Judgment target** `{field}`: `judgment`, `{key}`, `v:{value|-}` (option: its snake name), then
  `m:{method_version}`, `cp:{current_price|-}`, `ttm:{ttm_eps|-}`, then one line per year sorted by
  `year`: `y:{year}:{sales}:{eps}:{high_price}:{low_price}` (values only).
- **Note / study drafts:** no fingerprint (`base_fingerprint` is `None` per 8.2a `fits`).

Consequence to know (documented default): a price refresh changes `cp:` → every pending judgment
draft of that study reads **stale** (still validatable after confirmation — O4). A7 says "the market
facts the judgment is read against"; this is intended.

### 4. Where the marks are cleared (AC 4)

- Cell `ai_origin`: `Cell::edited` takes the caller's provenance verbatim, and `manual_provenance()`
  (`app/src/state/refresh.rs:27`) will build `ai_origin: None` → the owner's next edit clears it for
  free. `set_review` keeps the provenance verbatim (`cells.rs` doc) → the mark survives a ✓ toggle.
  `reconcile`/`reconcile_frozen` keep `..self.clone()` → the mark survives a refresh (manual wins).
  Accepting a pending provider value replaces the provenance → cleared. Undo restores whole snapshots.
- Judgment `ai_placed`: `apply_judgment_field` (`cells.rs:635-655`) and `set_forecast_low_option`
  (`cells.rs:458`) must clear the slot of the field they write. Refresh writes only `current_price`
  and `ttm_eps` (`refresh.rs:141,147,200`) — not draftable, no slot.

### 5. Persistence API shape (AC 7, 10, 12)

```rust
pub struct DraftDecisionWrite<'a> {
    pub draft_id: Uuid,
    pub to: DraftStatus,                    // Validated | Rejected
    pub decided_at: &'a Timestamp,
    pub stale_at_decision: Option<bool>,    // None for note/study drafts
    pub edited_before_validation: Option<bool>, // Some only when to == Validated
    pub study: Option<StudyWrite<'a>>,      // None for a rejection
}
pub struct StudyWrite<'a> { pub expected_before: &'a Study, pub after: &'a Study, pub now: &'a Timestamp }
```
The CHECKs of 8.2a (pending ⇔ `decided_at` NULL; pending carries no decision facts;
`edited_before_validation` only on validated / validated_undone; draft study never
`validated_undone`) must be satisfied by construction; a CHECK failure surfaces as `Sqlite`, which the
app maps to the save-failure message. Undo/redo (`step_draft_decision`) keeps
`edited_before_validation` and `stale_at_decision` as decided, sets `decided_at = now`
(documented default: `decided_at` is the time of the latest transition; the validation time lives in
`AiOrigin.validated_at` inside the study snapshot).

"Expected before" comparison: re-serialize the stored payload through `Study` and compare **strings**
with `serde_json::to_string(expected_before)` — the same technique 8.1 put in
`put_study_with_history` (`persistence/src/studies.rs:125-138`) so a legacy payload lacking additive
keys compares equal and a scale change does not.

### 6. Messages (AC 14) — §3.3 of `ux-ai-assistance-surfaces.md`, verbatim

- `MSG_DECISION_READ_ONLY` « Le dossier est en lecture seule ; aucune décision n'a été enregistrée. »
- `MSG_DECISION_STUDY_GONE` « L'étude {TICKER} n'existe plus ; aucune décision n'a été enregistrée. »
- `MSG_DECISION_STUDY_ARCHIVED` « L'étude {TICKER} est archivée ; aucune décision n'a été enregistrée. »
- `MSG_DECISION_SAVE_FAILED` « La décision n'a pas pu être enregistrée ({cause}) ; rien n'a été modifié. »
- `MSG_DECISION_CHANGED` « La cible a encore changé depuis votre confirmation ; rien n'a été enregistré. La proposition est affichée de nouveau. »
- `MSG_DECISION_TARGET_GONE` « Cible disparue : {raison} ; la proposition ne peut qu'être rejetée. »
  with reasons « l'année {AAAA} n'existe plus dans l'étude » · « l'étude a été supprimée »
- `MSG_DECISION_ALREADY_DECIDED` « Cette proposition a déjà été traitée ; rien n'a été enregistré. »
  — **not in §3.3** (question 1).
Posture: `USER_FACING_MESSAGES` is 228 today (`app/src/posture.rs:893`) → +7 = 235; the two reason
fragments go into a scanned inventory with an exact count. No `@tr` change.

### 7. Crash injection (AC 10) — technique

No failpoint framework exists. Plant a test-only SQLite trigger through a **second** `rusqlite`
connection on the same file (the pattern of `persistence/tests/readonly_protected.rs`, which opens
the file directly): `CREATE TRIGGER inject BEFORE UPDATE ON ai_drafts BEGIN SELECT RAISE(ABORT,
'injected'); END;` → `decide_draft` returns `Err`, then assert the study payload, the `judgments`
row count, the draft row and `logical_version` are unchanged. Drop the trigger after. The same with
`BEFORE INSERT ON judgments` to fail the other side.

### 8. Undo integration (AC 12) — current state

`app/src/state/undo.rs`: `UndoHistory { undo: Vec<Study>, redo: Vec<Study> }`, cap 100,
`record(before)` clears redo, `step()` pops, re-reads the present (`get_study`), writes the popped
snapshot through `put_study_with_history`, pushes the present on the opposite stack, pushes back on
failure. `reset_undo()` is called on study open (`app/src/wiring/studies.rs:709`) and for the demo
(:987). The app keeps **no** in-memory `Study`: every rail re-reads the journal
(`mutate_study`, `cells.rs:548`) — so the lost-update risk is limited to undo snapshots and the
decision's own `before`; the persistence guard (§5) closes the rest.
Preserve: every existing `record(before)` caller, the cap, push-back on failure, the FR51 snapshot
on each step, `undo_depth()` test hook.

### 9. Files — current state / change / preserve

| File | Today | Change | Preserve |
|---|---|---|---|
| `contract/src/ai.rs` | `AiOrigin` | + `DraftOrigin` | `AiOrigin` shape (8.1 notes use it) |
| `contract/src/provenance.rs` | 4 fields | + `ai_origin` | serde of existing fields; `hash_of_dependencies` doc |
| `contract/src/study.rs` | `Judgment` 11 fields | + `ai_placed: AiPlaced` | field names mirror `core::ssg::JudgmentInputs` |
| `contract/src/cell.rs` | `edited`, `reconcile*` | + `validated_from_draft` | invariant 2b doc; add the D5 exception note |
| `contract/src/draftable.rs` (new) | — | fields, units, parser, fingerprint | — |
| `contract/src/draft.rs` | 8.2a types | none, or a doc pointer to `draftable` | `fits`, deny_unknown_fields |
| `persistence/src/drafts.rs` | read side | + `get_draft`, `decide_draft`, `step_draft_decision` | corruption rule |
| `persistence/src/studies.rs` | `put_study_with_history` | factor tx-level snapshot helper | dedup semantics (8.1 T2.3) |
| `persistence/src/error.rs` | variants | + 4 draft/conflict variants | `ErrorKind` mapping exhaustive |
| `app/src/state/undo.rs` | `Vec<Study>` stacks | `UndoStep` + owner + draft routing | see §8 |
| `app/src/state/cells.rs` | judgment writers | clear `ai_placed` | soft-lock on typed edits |
| `app/src/state/drafts.rs` (new) | — | freshness + `decide_draft` | — |
| `app/src/state/messages.rs`, `app/src/posture.rs` | 228 msgs | +7, reason inventory | exact-count floors |

`Provenance { … }` literals to update (grep, 2026-09-27): `contract/src/cell.rs` (7),
`contract/src/provenance.rs` (2), `contract/tests/{cell_rails,roundtrip}.rs`, `core/tests/
verdict_coherence.rs` (5), `app/src/state/refresh.rs` (4), `app/src/viewmodel/{form,history,entry,
engine,chart,verify}.rs`, `app/src/seam_check.rs`, `app/src/posture.rs`, `report/src/{pdf,form}.rs`,
`report/examples/*`, `persistence/tests/{journal_roundtrip,e2e_lifecycle,corpus_gate}.rs` — some hits
are patterns; the compiler lists the real ones. Same for `Judgment` constructions.

### 10. Previous-story intelligence

- 8.1: structural history dedup compares **re-serialized strings** (G3 B2 — `PartialEq` hides Money
  scale); state rails pre-check with `try_get_study` so a read failure is named `MSG_READ_FAILED`,
  never a save failure; `MSG_STUDY_GONE` / `MSG_NO_STUDY_OPEN` exist; FixedClock/FixedIdGen in tests.
- 8.2a: the trigger fires on INSERT only (an `UPDATE` decision bumps explicitly, once); CHECKs are
  single-fault tested; `PayloadProblem` typed, no string errors outside `app` (posture rule).
- Build hygiene: `CARGO_BUILD_JOBS=4`; never `git add -A` (untracked PDFs at the repo root).

### 11. Review-checklist hooks (`docs/review-checklist.md`)

§1 absence honesty — target gone and read failures are named, never an empty result; §5 read the
failure surface — every persistence error maps to a named message with its cause; §6 exact-count
floors — messages +7 stated; §8 pure decisions — fingerprint, freshness, field parsing and the
`after` construction are pure functions tested without a journal; the rail only sequences them.

### Project Structure Notes

New modules `contract/src/draftable.rs`, `app/src/state/drafts.rs`; everything else extends existing
files. No crate dependency change (`contract` already has `sha2`, `rust_decimal`; `persistence` stays
free of `core` — the method version is passed in by `app`).

### References

- `_bmad-output/planning-artifacts/epics.md` — Story 8.2b, Epic 8 preamble.
- `_bmad-output/planning-artifacts/architecture.md` — §Phase 4 A6, A7, A8; D3, D5, D6; D9 withdrawn.
- `_bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md` — §3.3 decision wording.
- `_bmad-output/implementation-artifacts/8-1-study-notes.md`, `8-2a-drafts-table.md`.
- Code cited inline above.

## Questions for Guy (defaults applied)

1. « Cette proposition a déjà été traitée ; rien n'a été enregistré. » — a new refusal not in §3.3
   (a draft decided meanwhile). Default: adopt it.
2. A price refresh makes every pending judgment draft of the study **stale** (the fingerprint includes
   the current price, per A7). Default: keep — stale drafts remain validatable after confirmation.
3. Cell drafts propose **absolute** amounts for sales and pre-tax profit (the stored unit, what MCP
   reads), not the millions shown in the grid. Default: absolute; the inbox formats it like the grid.
4. The cell fingerprint also includes the cell's **coverage** (an owner marking the cell « non
   disponible » after the proposal makes it stale) — a small extension of A7. Default: include.
5. `decided_at` after an undo/redo is the time of that transition. Default: yes.

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), 2026-09-27.

### Debug Log References

- The posture gate `no_error_display_reaches_a_user_string` matched the `{reason}` placeholder of
  `MSG_DECISION_TARGET_GONE` against an `Err(reason)` binding elsewhere in `state/drafts.rs`; the
  binding was renamed (`why`) and the refusal mapping moved into `BuildProblem::refusal`.
- clippy `too_many_arguments` on `decided_study` → the injected parts are a `DecisionContext`.

### Completion Notes List

- Ultimate context engine analysis completed — comprehensive developer guide created.
- **Owner-pending defaults applied** (story questions 1–5, pending Guy's confirmation): adopt
  « Cette proposition a déjà été traitée ; rien n'a été enregistré. »; a price refresh makes pending
  judgment drafts stale (`cp:` in the fingerprint); sales / pre-tax drafts propose **absolute**
  amounts; the cell fingerprint includes coverage; `decided_at` after an undo / redo is the time of
  that transition.
- **AC 1** — `DraftOrigin`, `Provenance.ai_origin`, `Judgment.ai_placed` (`AiPlaced`, 9 slots) are
  additive and skipped when empty: the pinned corpus JSON and export hashes are unchanged (the
  corpus gate passes without re-capture). Every `Provenance` / `Judgment` construction was updated
  from the compiler's E0063 list (36 files).
- **AC 2** — `contract::draftable`: 16 fields (7 cell, 9 judgment; `current_price` / `ttm_eps` not
  draftable), units, options, the plain-decimal parser (`-0` → `0`; refuses exponent, `+`, locale
  and thousands separators, `.5`, `5.`), `of_target`, slot accessors.
- **AC 3 / 4** — `Cell::validated_from_draft` (the D5 exception, property-tested over every cell
  state); `apply_judgment_field` and `set_forecast_low_option` clear the field's mark; a source scan
  proves no other production code writes a draftable judgment field; reconciliation keeps the AI
  origin (manual wins, pending parked; agreement clears the pending, keeps the origin).
- **AC 5 / 6** — `draft_fingerprint` (`fp1`), 13 contract tests (EPS refresh, method change, price
  refresh, parked pending, pending-absent, re-stamp not stale, Money scale not stale, coverage,
  source, absent slot ≠ materialized empty cell, missing year = gone, a cell draft ignores the rest);
  `draft_freshness` classifies fresh / stale / target gone (an absent base fingerprint reads stale —
  it cannot prove freshness).
- **AC 7–12** — `Journal::decide_draft` / `step_draft_decision` (one transaction, one bump, the
  draft and study re-checks inside); `JournalState::decide_draft`; the undo history's steps carry the
  draft id and the history knows its owner study (`reset_undo_for`, called at the study-open site).
  Failure injection at either write (persistence) and at the app rail leaves everything unchanged.
- **AC 13** — every transition passes the 8.2a CHECKs (validate, reject, undo, redo; a rejection
  carrying `edited_before_validation` is refused by the engine).
- **AC 14** — the refusals are named; the persistence errors map to `MSG_DECISION_ALREADY_DECIDED`,
  `MSG_DECISION_CHANGED`, `MSG_STUDY_GONE` (a draft only leaves with its study) or the save failure
  with its cause.
- **AC 15** — `cargo test --workspace`: 1249 passed, 0 failed, 2 ignored (the corpus generators);
  clippy `-D warnings` and `fmt --check` clean. No `SCHEMA_VERSION` bump, no UI, no `@tr` delta.
- **Posture deltas** — `USER_FACING_MESSAGES` 228 → 236 (+8); new inventory `DRAFT_GONE_REASONS`
  0 → 3; persistence error sample inventory 23 → 27 (+4 variants).
- **Deviations from the story (all small, for the review):**
  1. **+8 messages, not +7**: the save failure has a plain form (« La décision n'a pas pu être
     enregistrée ; rien n'a été modifié. ») beside the §3.3 `({cause})` form — the existing
     `MSG_SAVE_FAILED` / `_CAUSE` pattern, so no « () » when no cause is named.
  2. **A third target-gone reason**, « le champ {field} n'existe pas dans l'étude », for a draft whose
     field names no draftable slot — reachable only through a malformed import (8.3 checks MCP
     submissions); not in §3.3.
  3. **Wire-key maps**: only `judgment_draft_field` (wire → field, used to clear the mark) exists; no
     inverse and no cell map, because the decision writes through `DraftField::put_cell` /
     `set_judgment` and never through `set_cell` / `apply_judgment_field` — nothing to map yet.
  4. A draft of **another study than the open one** is refused with `MSG_NO_STUDY_OPEN` (the inbox of
     8.5b opens that study first); a draft that is **gone** (the O7 cascade) reads `MSG_STUDY_GONE`.
  5. The persistence errors carry the status as its snake-case **text** (`DraftStatus` has no
     `Display`).
  6. The decision module is `#[cfg_attr(not(test), allow(dead_code))]` until its UI callers land
     (8.5a / 8.5b) — the `list_notice.rs` precedent. No `try_list_drafts` app wrapper yet (8.5a).

### G3 review follow-up (2026-09-27)

Three layers, no high. Applied (16 items):

1. **Owner's edit (E1/F3)** — `Decision::ValidateEdited` carries an `EditedValue` already read by
   `owner_edit`: the owner's number format (`typed_entry`: « 7,25 » under comma, « 1'234.5 » under
   point), the grid's units (sales / pre-tax profit entered in millions, stored absolute — #117),
   an option by name with its own refusal `MSG_VALUE_NOT_AN_OPTION`, a note normalized. A blank
   edit is refused (emptying a proposal is a rejection). `parse_value` stays for AI text only.
2. **Edit == proposal (E2)** — value equality (`7.50` = `7.5`, same option, same normalized note
   text) → a plain validation: AI origin kept, `edited_before_validation = 0`.
3. **History owner (B1/E3/F5)** — `push_step` drops a step of another study than the owner (a late
   fetch result); `step()` refuses a step whose study is not the one asked; closing a study resets
   the history (no owner); decisions check against the owner.
4. **Refusal naming (F1/F2/E4/E5/B4)** — no study open → `MSG_NO_STUDY_OPEN`; another study open →
   `MSG_DECISION_OTHER_STUDY` (« La proposition porte sur l'étude {ticker}, qui n'est pas ouverte
   ; … »); a vanished draft → `MSG_DECISION_DRAFT_GONE`, or `MSG_DECISION_STUDY_GONE` with the
   ticker when its study is gone. `decide_draft` now takes a `DraftRef { draft_id, study_id,
   ticker }` so the ticker is known even then. A reference whose study disagrees with the stored
   draft is internal (logged, plain save failure).
5. **Wedged undo step (B2/E7)** — a draft step failing with `DraftStatusMismatch` /
   `DraftNotFound` is dropped (not pushed back) with `MSG_UNDO_DRAFT_STEP_DROPPED`
   (owner-pending); `DraftNotFound`'s kind is `Other`, never « introuvable ».
6. **stale_at_decision (B3/E6)** — stale = base absent, target gone, or fingerprint differs (one
   `is_stale`, used by the classifier and the rejection).
7. **Third gone reason (F4)** — « le champ proposé ne peut pas faire l'objet d'une proposition »
   (owner-pending), the raw key never echoed; covers non-draftable keys such as `current_price`.
8. **ai_placed on a same-value write (F7) — lead default, owner-pending:** a write clears the mark
   only when the value changes, so a no-op stays a no-op (no undo step, no snapshot). Arch A6 and
   epics 8.2b say « any write that changes the field ».
9. **F9** — `Journal::decide_draft` / `step_draft_decision` refuse a study other than the draft's
   (`Error::DraftStudyMismatch`).
10. **F6** — fingerprint test: a parked pending re-stamped with the same value is not stale.
11. **B5** — the writer guard now scans app, persistence, report and ingestion for field
    assignments, `&mut` borrows and `Judgment { .. }` functional updates.
12. **F8 (owner-pending)** — a draft shown fresh whose target changed, or a study written meanwhile:
    « L'étude a changé depuis la lecture de la proposition ; rien n'a été enregistré. »
    (`MSG_DECISION_STUDY_CHANGED`); the §3.3 « encore changé » refusal stays for a confirmed stale draft.
13. **E9** — `parse_value` bounds AI numbers: `|value| < 10^15`, at most 10 decimals (trailing zeros
    free) → `DraftValueProblem::OutOfRange`. No owner-entry bound exists to align with. Imported AI
    marks may carry a `draft_id` absent from the dossier — accepted (not in production).
14. **E8** — accepted (not in production).
15. **Dead code** — `#[cfg_attr(not(test), expect(dead_code))]` / `expect(unused_imports)` on the
    module and the re-export, `expect(dead_code)` on `DRAFT_GONE_REASONS`.
16. **F10** — arch A6 / epics 8.2b describe the `AiPlaced` sidecar (nine slots).

**Posture deltas (G3):** `USER_FACING_MESSAGES` 236 → 241 (+5: other study, draft gone, study
changed, not an option, undo step dropped); `DRAFT_GONE_REASONS` 3 (reworded, count unchanged);
persistence error samples 27 → 28 (`DraftStudyMismatch`). Tests: 1258 passed, 0 failed, 2 ignored.

**Owner-pending (lead defaults, for Guy):** the five story questions; item 5 wording; item 7
wording; item 8 (mark kept on a same-value write); item 12 wording; the « déjà traitée » refusal.

### File List

- `contract/src/ai.rs` — `DraftOrigin`.
- `contract/src/provenance.rs` — `Provenance.ai_origin`.
- `contract/src/study.rs` — `Judgment.ai_placed`, `AiPlaced`.
- `contract/src/cell.rs` — `Cell::validated_from_draft`.
- `contract/src/draftable.rs` (new) — fields, units, parser, slots, `draft_fingerprint`.
- `contract/src/draft.rs`, `contract/src/lib.rs` — doc pointer, exports.
- `contract/tests/ai_marks.rs` (new), `contract/tests/fingerprint.rs` (new),
  `contract/tests/cell_rails.rs` — AC 1 / 3 / 4 / 5 tests.
- `persistence/src/drafts.rs` — `get_draft`, `study_status`, `decide_draft`,
  `step_draft_decision`, `DraftVerdict`, `DraftStep`, `StudyWrite`, `DraftDecisionWrite`.
- `persistence/src/studies.rs` — `write_study_with_snapshot`, `stored_study_is`,
  `check_study_identity` (shared by `put_study_with_history`).
- `persistence/src/error.rs` — 4 variants (+ kind, inventory).
- `persistence/src/lib.rs` — exports.
- `persistence/tests/draft_decisions.rs` (new) — transitions, refusals, failure injection.
- `app/src/state/drafts.rs` (new) — freshness, `decided_study`, `JournalState::decide_draft`.
- `app/src/state/undo.rs` — `UndoStep`, owner, `record_draft`, draft steps.
- `app/src/state/cells.rs` — marks cleared by the judgment writers, `judgment_draft_field`.
- `app/src/state/messages.rs`, `app/src/state/mod.rs`, `app/src/posture.rs` — messages, inventory,
  counts.
- `app/src/state/tests.rs` — `drafts_8_2b` (21 tests).
- `app/src/wiring/studies.rs` — `reset_undo_for(id)` at the study-open site.
- Mechanical `ai_origin: None` / `ai_placed: Default::default()` additions: `contract/src/{cell,
  export,provenance,study}.rs`, `contract/tests/{cell_rails,roundtrip}.rs`,
  `core/tests/verdict_coherence.rs`, `app/src/{seam_check,posture}.rs`,
  `app/src/state/{refresh,mod,studies,…}.rs`, `app/src/viewmodel/*.rs`, `report/src/{form,pdf}.rs`,
  `report/examples/*`, `persistence/tests/*` (the compiler's list, commit 47984ee).

### Change Log

- 2026-09-27 — Story 8.2b implemented (commits 47984ee contract, 82b514d persistence, 8ff4700 app,
  + tests/record); status → review.
- 2026-09-27 — G3 review follow-up applied (16 items); status stays review.

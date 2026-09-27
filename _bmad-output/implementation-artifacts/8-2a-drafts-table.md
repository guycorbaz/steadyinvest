# Story 8.2a: Drafts table (headless)

Status: ready-for-dev

<!-- Created 2026-09-27 by the create-story workflow. Branches from main AFTER 8.1 (PR #257) merges:
     8.1 adds contract/src/ai.rs (AiOrigin) and Note, which this story's contract module sits beside. -->

## Story

As the developer,
I want the drafts table defined once, with every variant and decision fact it will ever need,
so that every later story (8.2b decisions, 8.3 McpAccess, 8.5a–8.7 UI) writes and reads drafts through
one proven, versioned model.

## Acceptance Criteria

Source: `epics.md` Story 8.2a; arch §Phase 4 A3 (trigger), A4 (table), O7 (cascade); owner rule
2026-09-27 « pas en production : pas besoin de migrer l'existant » (commit 8ca94bc — D9 withdrawn:
no `SCHEMA_VERSION` bump, no compat work; the migration harness stays for NEW tables).

1. **Migration v8 creates `ai_drafts`** in the same SQLite file with exactly these columns (arch A4):
   `id`, `kind`, `study_id`, `security_ticker`, `native_currency`, `status`, `created_at`,
   `decided_at`, `comment`, `origin_client`, `origin_model`, `stale_at_decision`,
   `edited_before_validation`, `created_study_id`, `payload` — plus the indexes and CHECKs of Dev
   Notes §2. (FR70–FR72, FR77)
2. **Every enum variant is defined now** in `contract`: `DraftKind` (`study|note|cell|judgment`),
   `DraftStatus` (`pending|validated|validated_undone|rejected`), `DraftTarget`
   (`Cell{fiscal_year, field}` | `Judgment{field}`), and the versioned `DraftPayload`. No
   `serde(other)` / `non_exhaustive` (contract enum policy, `contract/src/lib.rs`).
3. **The database refuses** a draft with an empty/blank comment or a missing origin
   (`NOT NULL` + `CHECK(length(trim(comment)) > 0)`), and every other CHECK of Dev Notes §2 — each
   proven by a **direct SQL insert** that fails with a constraint error. (NFR-A4)
4. **The migration creates the trigger** `trg_ai_drafts_bump_logical_version` (`AFTER INSERT ON
   ai_drafts`) that bumps `journal_meta.logical_version`; a direct insert raises the version by one.
   (arch A3 — 8.3's authorizer will allow `UPDATE journal_meta` only from this trigger's name.)
5. **A v7 dossier migrates to v8 with no data loss** (every table's rows intact, `ai_drafts` present
   and empty, `user_version` 8), and the frozen corpus gains **`v8.db`** (canonical study + one draft
   of every kind and status), opened and read back by the corpus gate. (NFR-R3, NFR-M2)
6. **Backup** with `VACUUM INTO` (`Journal::backup_to`) carries `ai_drafts` rows **and the trigger**
   as-is.
7. **Export/import**: `JournalSnapshot` gains an additive `ai_drafts: Vec<DraftRecord>` array
   (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`, the #78 rail); a round-trip
   preserves **every draft and every column** byte-for-byte (payload string included); a draft whose
   `study_id`/`created_study_id` references a study absent from both the file and the target dossier
   is an `ImportMalformed` refusal, nothing applied. (FR60, FR61, NFR-R5)
8. **Cascade (O7)**: `delete_study(id)` deletes, in the same transaction, the drafts whose `study_id`
   **or** `created_study_id` is `id`; a pending draft *study* (`study_id` NULL, `created_study_id`
   NULL) is untouched; the version bumps when any draft was removed. (FR55)
9. **No computation path reads `ai_drafts`**: a test asserts `core` and `report` cannot (no
   dependency on `persistence`) and that no source under `core/src`, `report/src`,
   `app/src/viewmodel/engine*` names `ai_drafts` or the draft read API. The inbox/reminder/record
   view models of 8.5a–8.7 read it **by design** (the narrowed wording of epics 8.2a). The
   engine-level metamorphic suite is 8.3's.

## Tasks / Subtasks

- [ ] **T1 — Contract types (AC 2)** — new `contract/src/draft.rs`, re-exported from `lib.rs`
  - [ ] 1.1 `DraftKind`, `DraftStatus` (`#[serde(rename_all = "snake_case")]`), each with
        `as_str()` / `FromStr` returning the exact DB spellings (used by persistence; one mapping).
  - [ ] 1.2 `DraftTarget` — `#[serde(tag = "target", rename_all = "snake_case")]`:
        `Cell { fiscal_year: i32, field: String }`, `Judgment { field: String }`. `field` stays a
        `String` here; the enumerated draftable fields + units are **8.2b's** (epics 8.2b AC 2).
  - [ ] 1.3 `DraftPayload { version: u32, target: Option<DraftTarget>, proposed_value:
        Option<String>, note_text: Option<String>, company_name: Option<String>,
        base_fingerprint: Option<String> }` — options `#[serde(default, skip_serializing_if =
        "Option::is_none")]`; `pub const DRAFT_PAYLOAD_VERSION: u32 = 1`.
  - [ ] 1.4 Pure `DraftPayload::fits(kind) -> bool` (shape rule, Dev Notes §3) + unit tests for
        every kind × shape; used by the persistence insert helper of tests and by 8.3 later.
  - [ ] 1.5 Serde unit tests: exact JSON of each variant (pinned strings), round-trip, an unknown
        `target`/`kind` value fails to parse (fail-loud policy).
- [ ] **T2 — Migration v8 (AC 1, 3, 4)** — `persistence/src/schema.rs` + `migrations.rs`
  - [ ] 2.1 `migrate_to_v8` with the DDL of Dev Notes §2 (table, 3 indexes, trigger) — a
        `CREATE TABLE` step like v5; doc comment in the file's style (story, FRs, why each CHECK).
  - [ ] 2.2 Append `(8, crate::schema::migrate_to_v8)` to `REGISTRY` and its doc list.
  - [ ] 2.3 Update every hard-coded "latest = 7": `migrations.rs` tests (`latest_version == 8`,
        `fresh_database_migrates_to_latest`, `rerun_is_idempotent…`), the fake future step
        **`fake_v8` → `fake_v9`** with `NINE_STEP_REGISTRY` and marker `migration-marker-v9`,
        `newer_file_is_refused_not_migrated` (`file_user_version: 9, supported: 8`),
        `failed_step_leaves_user_version_at_previous_step` (failing v8 → stays 7),
        `schema.rs` `v2_adds…` message/assert (8), `the_registry_creates_exactly_the_architecture_tables`
        (add `"ai_drafts"` → 10 tables), `persistence/tests/readonly_newer.rs` (`supported: 8`; the
        hand-bumped 9 stays "newer").
  - [ ] 2.4 `naming_conventions_hold` passes unchanged (`ai_drafts` plural; `idx_ai_drafts_*`);
        extend it: every trigger name starts with `trg_<table>_`.
  - [ ] 2.5 CHECK tests by direct insert (Dev Notes §5 list), each asserting a constraint error and
        zero rows; the happy insert of each kind succeeds and bumps `logical_version` by exactly 1.
- [ ] **T3 — Read API + export/import (AC 7)** — new `persistence/src/drafts.rs` (arch tree), `export.rs`
  - [ ] 3.1 `pub struct DraftRecord` (serde, no `deny_unknown_fields` — #78 per-entity rule) with one
        field per column: typed `DraftKind`/`DraftStatus`, `Uuid`s, `Timestamp`s, `Option<bool>` for
        the two nullable booleans, `payload: String` **raw** (byte-faithful like
        `JudgmentSnapshotRecord`).
  - [ ] 3.2 `Journal::list_drafts() -> Result<Vec<DraftRecord>>` ordered `created_at, id`; corrupt
        rows (bad UUID, unknown kind/status, non-0/1 boolean) → `CorruptPayload` naming the column —
        never skipped (checklist §1 « absent, never wrong »).
  - [ ] 3.3 `JournalSnapshot.ai_drafts` (#78 rail) filled by `journal_snapshot()`; import upserts by
        `id` (`INSERT … ON CONFLICT(id) DO UPDATE SET` every column) **after** studies; validates
        references (Dev Notes §4) → `ImportMalformed`; `DraftPayload` must parse and `fits(kind)`,
        else `ImportMalformed`. `ImportSummary.ai_drafts: usize`; `applied` includes it.
  - [ ] 3.4 Tests: round-trip of drafts in every kind × status with every column set/unset
        (compare `list_drafts()` before/after, payload strings equal); an old file without the
        array imports; an empty draft set exports **without** the key; dangling reference refused
        with nothing applied (all-or-nothing).
- [ ] **T4 — Cascade (AC 8)** — `studies.rs::delete_study`
  - [ ] 4.1 `DELETE FROM ai_drafts WHERE study_id = ?1 OR created_study_id = ?1` **before** the
        study row (FK order, like `judgments`); include `removed_drafts > 0` in the bump condition;
        update the doc comment (O7).
  - [ ] 4.2 Tests: drafts on A (note/cell/judgment, every status) + a validated draft study whose
        `created_study_id` = A + a pending draft study + drafts on B → delete A removes exactly A's,
        keeps the pending draft study and B's; deleting an absent id stays a no-op (no bump).
- [ ] **T5 — Backup (AC 6)**: test in `journal_roundtrip.rs` or a new `persistence/tests/drafts.rs`:
      `backup_to` → open the backup read-only/raw → rows equal, `sqlite_master` has the trigger.
- [ ] **T6 — Migration & corpus (AC 5)**
  - [ ] 6.1 Unit test (in `migrations.rs`, where `REGISTRY` is visible): apply `REGISTRY[..7]`,
        seed every table (journal_meta, a study + judgment, portfolio/holding/transaction, fx rate,
        watchlist item, price_history row), run the full registry, assert every row intact,
        `ai_drafts` empty, trigger present, `user_version` 8.
  - [ ] 6.2 `corpus_gate.rs`: `#[ignore]`d `generate_corpus_v8` (refuses to overwrite) — canonical
        study + one draft per kind and per status inserted **by raw SQL** on the closed-then-reopened
        file (no public insert API exists before 8.3), fixed ids/times; run once; `git add
        persistence/tests/corpus/v8.db` (the `.gitignore` exception covers it — check `git status`).
  - [ ] 6.3 Gate test `frozen_corpus_v8_opens_and_reads_back`: copy to TempDir, `user_version` 8,
        `get_study` equals `canonical_study()`, `list_drafts()` equals the pinned expectation.
        Keep the v1 gate unchanged. Update `tests/corpus/README.md` table (v8 row; note the v2–v7
        gap as found, not back-filled — app not in production).
- [ ] **T7 — Computation isolation (AC 9)**: `persistence/tests/drafts.rs` (or `core`-side) test:
      parse `core/Cargo.toml` and `report/Cargo.toml` — no `steadyinvest-persistence` dependency;
      scan `core/src/**`, `report/src/**`, `app/src/viewmodel/engine*` for `ai_drafts` /
      `list_drafts` → none. Paths via `env!("CARGO_MANIFEST_DIR")/..`.
- [ ] **T8 — Invariant doc** (`persistence/src/util.rs`): amend the « exactly once » doc — the v8
      trigger bumps once **per inserted draft row**; an import carrying N drafts therefore raises the
      counter by 1 + N. The counter's contract is **monotonic** (stale-restore detection, sync),
      which holds. Name the trigger at the site (checklist §3).
- [ ] **T9 — Gates**: `CARGO_BUILD_JOBS=4 cargo test --workspace`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo fmt --check`, `cargo deny check` unchanged. Story record
      + sprint-status → review.

## Dev Notes

### 1. Where this sits

- **Branch after 8.1 merges** (PR #257): 8.1 adds `contract/src/ai.rs` (`AiOrigin`) and `Note`.
  `DraftOrigin{client, model}` is **8.2b's** (arch A6) — 8.2a stores origin as two columns only.
- **Headless**: no `app/ui`, no Slint, no user-facing string, no `MSG_*` — no posture delta.
- **No writer API for drafts in 8.2a.** Inserts come from `McpAccess::insert_draft` (8.3);
  decisions (`status`, `decided_at`, `stale_at_decision`, `edited_before_validation`,
  `created_study_id`) are written by `decide_draft` (8.2b). Tests insert with raw SQL on the file —
  which also exercises the CHECKs and the trigger exactly as 8.3 will.
- **Not in production** (owner 2026-09-27): no `SCHEMA_VERSION` bump (`contract/src/versioning.rs`
  stays 1), no compat code. `JournalSnapshot` is `deny_unknown_fields`, so an older build refuses a
  file carrying drafts anyway (#78) — no extra work.

### 2. DDL (migration v8) — the exact contract

```sql
CREATE TABLE ai_drafts (
    id                       TEXT PRIMARY KEY,
    kind                     TEXT NOT NULL CHECK (kind IN ('study','note','cell','judgment')),
    study_id                 TEXT REFERENCES studies(id),
    security_ticker          TEXT NOT NULL,
    native_currency          TEXT,
    status                   TEXT NOT NULL DEFAULT 'pending'
                             CHECK (status IN ('pending','validated','validated_undone','rejected')),
    created_at               TEXT NOT NULL,
    decided_at               TEXT,
    comment                  TEXT NOT NULL CHECK (length(trim(comment)) > 0),
    origin_client            TEXT NOT NULL CHECK (length(trim(origin_client)) > 0),
    origin_model             TEXT NOT NULL CHECK (length(trim(origin_model)) > 0),
    stale_at_decision        INTEGER CHECK (stale_at_decision IN (0,1)),
    edited_before_validation INTEGER CHECK (edited_before_validation IN (0,1)),
    created_study_id         TEXT REFERENCES studies(id),
    payload                  TEXT NOT NULL,
    -- shape rules (defence in depth; 8.3 refuses earlier with a named reason)
    CHECK ((kind = 'study') = (study_id IS NULL)),
    CHECK ((kind = 'study') = (native_currency IS NOT NULL)),
    CHECK (created_study_id IS NULL OR (kind = 'study' AND status IN ('validated','validated_undone'))),
    CHECK ((status = 'pending') = (decided_at IS NULL))
);
CREATE INDEX idx_ai_drafts_status           ON ai_drafts(status);
CREATE INDEX idx_ai_drafts_study_id         ON ai_drafts(study_id);
CREATE INDEX idx_ai_drafts_created_study_id ON ai_drafts(created_study_id);
CREATE TRIGGER trg_ai_drafts_bump_logical_version AFTER INSERT ON ai_drafts
BEGIN
    UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1;
END;
```

- **FKs** (`REFERENCES studies(id)`, RESTRICT, `foreign_keys=ON` on every read-write open —
  `journal.rs:880-881`): an orphan draft is impossible; `delete_study` deletes drafts first (T4).
  8.3's MCP connection also sets `foreign_keys=ON` (arch A2).
- **`trim()` on `comment`** covers spaces only (SQLite `trim` default set); 8.3 adds the full
  Unicode-blank refusal with a named reason. Say so in the DDL comment.
- **Booleans** are INTEGER 0/1/NULL (no REAL anywhere — `no_column_anywhere_is_real…` still passes).
- **Timestamps** TEXT RFC3339 UTC (schema conventions, `schema.rs` header).
- **`kind` = `judgment` exists** (arch A4 lists `study|note|cell|judgment`; epics 8.2a AC; the older
  `kind (study|note|cell)` wording in the first G2 draft is superseded).
- **The trigger fires on INSERT only.** An import upsert that hits `ON CONFLICT … DO UPDATE` fires
  no INSERT trigger (SQLite semantics) — the import's own single `bump_logical_version` covers it.
- **Name matters**: 8.3's authorizer allows `UPDATE journal_meta` only when the authorizer's
  trigger-name argument is `trg_ai_drafts_bump_logical_version` (arch A3). Do not rename later.

### 3. Payload shape rule (`DraftPayload::fits`)

| kind | target | proposed_value | note_text | company_name |
|---|---|---|---|---|
| study | None | None | None | optional |
| note | None | None | **Some** | None |
| cell | **Cell{…}** | **Some** | None | None |
| judgment | **Judgment{…}** | **Some** | None | None |

`base_fingerprint`: Some for cell/judgment once 8.2b computes it; 8.2a tolerates None (no
fingerprint code yet — A7 is 8.2b). `version` must equal `DRAFT_PAYLOAD_VERSION` (1); a higher
version is `ImportMalformed` on import and `CorruptPayload` on read (fail-loud, no silent parse).
`proposed_value` is a **string** (decimal text, or an enum variant name for `forecast_low_option`) —
parsing/validation by field is 8.2b/8.3.

### 4. Import reference rule

For each draft: `study_id` / `created_study_id` must be the id of a study **in the file** or
**already in the target dossier**, else `ImportMalformed { detail: "a draft references a study
absent from the snapshot and the dossier" }`. Checked before any write; the whole import is one
transaction (existing pattern, `export.rs` ~355 for `judgment_snapshots`). Insert drafts **after**
studies (FK). Drafts carry no `journal_id` → nothing to rebind.

### 5. CHECK tests (direct insert, each must fail with zero rows written)

blank comment `'   '`; NULL comment; empty `origin_client`; NULL `origin_model`; unknown `kind`;
unknown `status`; `kind='study'` with a `study_id`; `kind='note'` without `study_id`;
`kind='study'` without `native_currency`; `kind='cell'` with `native_currency`; `status='pending'`
with `decided_at`; `status='rejected'` without `decided_at`; `created_study_id` on a `note`;
`created_study_id` on a pending study draft; `stale_at_decision = 2`; `study_id` referencing an
absent study (FK). Happy path: one valid insert per kind → `logical_version` +1 each.

### 6. Files — current state / change / preserve

| File | Today | Change | Preserve |
|---|---|---|---|
| `contract/src/draft.rs` (new), `lib.rs` | — | types of T1 | contract enum policy (no `serde(other)`) |
| `persistence/src/schema.rs` | v1 frozen DDL + v2–v7 steps | `migrate_to_v8` | `DDL_V1` frozen; step style |
| `persistence/src/migrations.rs` | `REGISTRY` v1–v7; fake v8 tests | append v8; fake → v9 | per-step transaction; newer-file refusal |
| `persistence/src/drafts.rs` (new), `lib.rs` | — | `DraftRecord`, `list_drafts` | — |
| `persistence/src/studies.rs` `delete_study` (~285) | deletes judgments, study, clears watchlist link | + drafts (study_id OR created_study_id) first | one transaction; no-op on absent id |
| `persistence/src/export.rs` | #78 arrays `fx_rates`, `judgment_snapshots` | + `ai_drafts` array, validation, summary count | envelope `deny_unknown_fields`; one bump; all-or-nothing |
| `persistence/src/util.rs` | « exactly once » doc | trigger exception (T8) | helper unchanged |
| `persistence/tests/corpus_gate.rs`, `corpus/README.md`, `corpus/v8.db` | v1 only | v8 generator + gate | v1 gate untouched; append-only corpus |
| `persistence/tests/readonly_newer.rs` | `supported: 7` | `supported: 8` | hand-bumped 9 |

`app` compiles unchanged except wherever `ImportSummary` is constructed/destructured exhaustively
(today only the test literal at `app/src/state/tests.rs:542` — add `ai_drafts: 0`) and wherever the app renders import counts
(no new message needed: drafts are not shown in the import summary text in 8.2a; if the summary
lists entities exhaustively, leave drafts out and note it for 8.5a).

### 7. Previous-story intelligence

- 8.1 (PR #257): additive `#[serde(default)]` fields, no version bump (owner rule); structural
  history dedup; corpus: only the pinned JSON changed. Keep 8.1's `Note`/`AiOrigin` untouched.
- #34 PR 3 (`judgment_snapshots`) is the closest precedent for an exported table: raw payload
  string, reference check against the snapshot, `#[serde(default, skip_serializing_if)]`.
- v5 `price_history` is the precedent for a `CREATE TABLE` step (but it is NOT exported; drafts are).
- Build hygiene: `CARGO_BUILD_JOBS=4` (OOM crash 2026-09-26); never `git add -A` (untracked export
  PDFs at the repo root).

### 8. Review-checklist hooks

§1 corrupt rows are errors, never skipped · §2 drafts keyed by `id`, never by position · §3 name the
trigger/one-bump exception at `util.rs` and `delete_study` · §5 the `deny_unknown_fields` cliff is
exactly why `ai_drafts` rides the envelope array (read the #78 comment before editing) · §8 n/a.

### Project Structure Notes

Matches the architecture tree (`persistence/src/drafts.rs` « ai_drafts (v8) + decide_draft » —
`decide_draft` itself lands in 8.2b). Contract module name `draft.rs` beside `ai.rs`.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.2a]
- [Source: _bmad-output/planning-artifacts/architecture.md#Phase 4 — A3, A4, A12; D9 withdrawn]
- [Source: persistence/src/schema.rs, migrations.rs, studies.rs#delete_study, export.rs, util.rs, journal.rs#backup_to]
- [Source: persistence/tests/corpus/README.md, corpus_gate.rs]
- [Source: docs/review-checklist.md]
- [Source: commit 8ca94bc (not in production)]

## Questions for Guy (defaults applied unless he objects)

1. **Version counter with drafts**: the v8 trigger bumps `logical_version` once per inserted draft,
   so an import of N drafts raises it by 1 + N (monotonic, not an exact mutation count). Default:
   accept and document (the counter is only compared for "newer/older").
2. **Corpus gap v2–v7**: `v8.db` is added; the missing v2–v7 files are not back-filled (not in
   production). Default: leave the gap, noted in the README.

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

- Ultimate context engine analysis completed — comprehensive developer guide created.

### File List

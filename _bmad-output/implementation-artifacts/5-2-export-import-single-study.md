# Story 5.2: Export / import a single study

Status: ready-for-dev

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to export and import one study as a portable file,
so that I can seed, share or archive a study and round-trip it safely.

## Acceptance Criteria

1. **AC1 — The portable export envelope (data-contract JSON + `schema_version` + integrity hash, FR59).** Exporting a study writes a **single JSON file** that is the **serialized `contract::Study`** (the already `Serialize`/`Deserialize` type — id, journal_id, ticker, native_currency, years, judgment, rationale, created_at, schema_version) wrapped in an envelope carrying the **`schema_version`** it was written under and an **integrity hash** (SHA-256 over the canonical serialized study bytes). **NOT a raw `.db` copy** (architecture decision §"Export/backup format"). The envelope + hashing live in the **`contract` crate** (the decoupled serde boundary; no Slint/rusqlite). The hash uses the **workspace `sha2`** dependency (already resolved via core/ingestion → **no new external dependency, `Cargo.lock` unchanged**).

2. **AC2 — Import verifies integrity + version, identity preserved on round-trip (FR59, NFR-R5).** Importing parses the envelope, **recomputes the hash and rejects a mismatch** (tamper/corruption) with a clear neutral message, and **checks `schema_version`**: equal → accept; **mismatch → reject with a clear message** (a forward migration hook is structured but, with `SCHEMA_VERSION == 1`, only the equal case accepts — an unknown/newer version is refused, never silently coerced). A successfully imported study **round-trips identity**: re-persisting via `put_study` preserves the study's `id` (export→import yields the same study, not a duplicate with a new id). A malformed / non-envelope / truncated file is a typed refusal, never a panic.

3. **AC3 — App surface: export & import actions, neutral outcomes.** The study screen (and/or the dashboard) offers **Exporter l'étude** (write the envelope to a user-chosen file — defaulting to the sync/export folder, never the live DB dir) and **Importer une étude** (pick a file → validate → persist). Outcomes are **neutral, posture-gated facts** (FR13): success names what happened ("étude exportée" / "étude importée"); a rejection names the cause (integrity / version / unreadable) without leaking internals. An import that would overwrite an existing study id is surfaced (re-import of the same study updates it; this is the round-trip case). Every new literal goes through `@tr`; the floor is bumped by exactly the number added; any new `MSG_*` is registered.

4. **AC4 — `core`/method untouched; no schema change; no new dependency.** The export/import path is **`contract` (envelope) + `persistence` (read full study / `put_study`) + `app` (file IO + UI)**. **No `core::ssg` change** (fingerprint/golden/determinism gates stay green), **no migration** (`PRAGMA user_version` unchanged — export reads an existing study, import writes via the existing `put_study`), **no `contract::SCHEMA_VERSION` bump** (the envelope wraps the *current* contract; it does not change it), and **no new external dependency** (`sha2`/`serde_json` are workspace deps; `Cargo.lock`/`deny.toml` unchanged). File reading/writing is the app's (the `contract` surface stays `rust_decimal + serde + sha2`, callers own file IO — per the `core`/`contract` boundary note). Copy neutral, posture-gated.

## Tasks / Subtasks

- [ ] **Task 1 — `contract`: the export envelope + integrity hash (AC1, AC2, AC4)** — `contract/src/`
  - [ ] Add `sha2 = { workspace = true }` to `contract/Cargo.toml` (workspace-resolved → `Cargo.lock` unchanged; confirm with `git diff Cargo.lock` = empty).
  - [ ] New `contract/src/export.rs`: an envelope type (e.g. `pub struct StudyExport { pub schema_version: u32, pub integrity_hash: String, pub study: Study }`) — but compute the hash over the **canonical serialized `Study`**, not over the envelope (avoid a hash-over-self cycle). Suggest: `to_export_json(&Study) -> String` = `serde_json::to_string(study)` → SHA-256 hex of those bytes → wrap `{ schema_version: study.schema_version, integrity_hash, payload: <study json string> }` and serialize the envelope. `from_export_json(&str) -> Result<Study, ImportError>` = parse envelope → recompute SHA-256 over `payload` → compare (reject `ImportError::Integrity` on mismatch) → check `schema_version == SCHEMA_VERSION` (reject `ImportError::Version { found, supported }` otherwise) → `serde_json::from_str::<Study>(payload)` (reject `ImportError::Malformed`).
  - [ ] `pub enum ImportError { Integrity, Version { found: u32, supported: u32 }, Malformed(String) }` — typed, no panic, carries no secrets.
  - [ ] Unit tests: round-trip `to_export_json` → `from_export_json` yields an **equal** `Study` (id preserved); a flipped byte in the payload → `Integrity`; a bumped `schema_version` in the envelope → `Version`; a truncated / non-JSON / wrong-shape string → `Malformed`; the hash is **stable/deterministic** for the same study (canonical serialization). **Does NOT touch `core::ssg`.**

- [ ] **Task 2 — `persistence`: full-study read + identity-preserving import (AC2, AC4)** — `app`/`persistence` boundary
  - [ ] Export reads the complete `Study` via the existing `Journal::get_study(id)` (already returns the full study incl. judgment + years). No new persistence read needed — confirm `get_study` is sufficient (it is) and note it.
  - [ ] Import persists via the existing `Journal::put_study(&study)` — **identity preserved** (same `id`; a re-import updates rather than duplicating). Guarded (read-only journal refuses with the existing neutral notice). Confirm `put_study` upserts by id (round-trip test below).
  - [ ] Integration test (or app-state test): export a saved study to a JSON string, delete it, import the string back → `get_study(id)` returns an **equal** study (same id, judgment, years, rationale). A second import of the same envelope is an idempotent update, not a duplicate.

- [ ] **Task 3 — App state: the export/import rail (AC2, AC3)** — `app/src/state.rs`
  - [ ] `export_study(&self, id) -> Result<String, String>` (read the study → `contract::export::to_export_json`; guarded — no-journal / missing id → neutral notice). Pure read.
  - [ ] `import_study(&mut self, json: &str) -> Result<Uuid, String>` (parse+verify via `contract::export::from_export_json`, mapping each `ImportError` to a neutral `MSG_*`; then `put_study`; returns the imported id). Read-only/no-journal guarded. New `MSG_IMPORT_INTEGRITY` / `MSG_IMPORT_VERSION` / `MSG_IMPORT_MALFORMED` / `MSG_STUDY_EXPORTED` / `MSG_STUDY_IMPORTED` (register in `USER_FACING_MESSAGES`).
  - [ ] Tests: a tampered/wrong-version/garbage string each maps to the right neutral notice and writes nothing; a good string imports and returns the id.

- [ ] **Task 4 — main.rs + Slint: the file actions (AC3)** — `app/src/main.rs`, `app/ui/`
  - [ ] A file-save dialog for export (default to the export/sync folder, **never** the live DB directory — ADD7/8 sync-safety) writing the envelope string; a file-open dialog for import reading the string. Use the app's existing file-dialog pattern (rfd or the established picker — match the DB-location picker from Story 1.x). The `contract` surface owns no file IO (callers do).
  - [ ] Slint: **Exporter l'étude** / **Importer une étude** actions (study screen header or the dashboard), neutral copy, posture-gated; surface the outcome notice. `@tr` floor + `MSG_*` inventory bumped by exactly the number added.
  - [ ] Re-render the dashboard/study list after a successful import (the new/updated study appears).

- [ ] **Task 5 — Gates (AC4)** — fmt, clippy `-D`, `test --workspace`, `deny` + smoke launch. Confirm `core::ssg` re-diffs clean; **`Cargo.lock` / `deny.toml` unchanged** (adding `sha2`/`serde_json` as workspace deps must NOT change the lock — verify `git diff main -- Cargo.lock` is empty); **no migration** (`user_version` unchanged); **`contract::SCHEMA_VERSION` stays 1**; `@tr` floor + `USER_FACING_MESSAGES` inventory bumped exactly.

## Dev Notes

### Scope (Epic-5 de-risk story — start here, per the Epic-4 retro D5)
The **single-study** portable export/import: the serialized data contract + `schema_version` + integrity hash, round-tripping identity, rejecting tamper/version mismatch. **Provider-independent** (no EODHD, no price history) — this is why it leads Epic 5 while the on-display GO/NO-GO gate (D1) and the provider decision (D2) clear in parallel. It also **de-risks the export/import seam** (the envelope + integrity + version contract) that Stories 5.3 (whole journal) and 5.4 (restore) build on.

### Out of scope (deferred)
- **Whole-journal export/import** (FR60) → Story 5.3 (reuses this envelope pattern over all studies + the journal `(journal_id, version, hash)`).
- **Restore-from-backup** (FR66) → Story 5.4.
- **Schema-version MIGRATION on import** — with `SCHEMA_VERSION == 1` there is nothing to migrate yet; the import **rejects** a mismatch with a clear message. The migration hook is structured (the `Version` error carries `found`/`supported`) but the actual upgrade path is a future story when `SCHEMA_VERSION` first advances.
- **Confront mode / price history** (FR50, ADD13) → Story 5.1 (provider-gated).
- **PDF export** (FR61) → Story 5.6.

### Architecture decisions this story honours
- [Source: architecture.md §"Export / backup format (decided — point 1)"] — the portable export unit is the **serialized serde data contract (JSON) + `schema_version` + integrity hash**, NOT a raw `.db`. A raw `.db` copy stays the file-level NAS backup unit (Story 5.4); the JSON export is the exchange/seed/golden unit.
- [Source: architecture.md §"Three version axes"] — `schema_version` (serialized contract) is distinct from SQLite `user_version` and `method_version`; the envelope carries `schema_version` only.
- [Source: architecture.md §"contract crate"] — `contract` is the versioned serde boundary, decoupled from Slint and rusqlite; the export envelope belongs here. **Callers own file reading and JSON parsing** (the surface stays `rust_decimal + serde + sha2`) — so the file dialogs + IO live in `app`, not `contract`.
- [Source: architecture.md §"sync-safety ADD7/8"] — exports/backups go to the (Synology) sync folder; the **live DB stays local**. The export dialog must default away from the live DB directory.

### Where things live
- **`contract/src/export.rs`** (new): the envelope type + `to_export_json` / `from_export_json` + `ImportError`. Pure serde + sha2; no IO. Decoupled from `core::ssg`.
- **`contract/Cargo.toml`**: `sha2 = { workspace = true }` (no lock change).
- **`persistence`**: no new code expected — `get_study` (full read) and `put_study` (identity-preserving upsert) already exist. Confirm `put_study` upserts by id.
- **`app/src/state.rs`**: `export_study` / `import_study` rails + the neutral notices.
- **`app/src/main.rs` + `app/ui/`**: the file dialogs + the two Slint actions + outcome notices.

### Notes & guardrails
- **Hash over the payload, not the envelope** — compute SHA-256 over the canonical serialized `Study` bytes (the `payload`), store it in the envelope; on import recompute over `payload` and compare. Hashing the whole envelope (which contains the hash) is a cycle.
- **Determinism of the hash** — `serde_json::to_string` over a struct emits fields in declaration order (stable), so the same `Study` yields the same bytes and hash across runs/OSes. (If field order is ever a concern, pin it; for now struct serialization is deterministic.)
- **Identity round-trip (FR59)** — `put_study` keys on `study.id`; importing the same envelope twice updates in place (no duplicate). The test must assert the id is unchanged after export→delete→import.
- **No secrets in errors (NFR-S1)** — `ImportError` carries only `found`/`supported` ints and a generic malformed detail; never file contents or paths in a way that could leak.
- **Sync-safety (ADD7/8)** — default the export path to the sync/export folder, never the live DB dir; a raw `.db` on a synced folder risks WAL corruption (the standing project warning), but a JSON export is safe there.

### Manual on-display GO/NO-GO (Guy)
Export a saved study → a `.json` file appears in the chosen folder; open it (human-readable contract + `schema_version` + a hash). Delete the study in-app → import the file → the study reappears **identical** (same ticker/years/judgment/rationale). Hand-edit one digit in the file → import is **refused** with an integrity notice, nothing written. Bump the `schema_version` in the file → import is **refused** with a version notice. Import a random non-JSON file → neutral "unreadable" refusal, no panic.

### Project Structure Notes
- Additive `contract::export` module (+ the workspace `sha2` dep, no lock change); app state + file dialogs + Slint actions. No `core` change, no migration, no `SCHEMA_VERSION` bump, no new external dependency.
- Posture floors at story start: `@tr` floor **282** (Story 4.7), `USER_FACING_MESSAGES` inventory **48**. Bump both by exactly the number of new literals/notices.

### References
- [Source: _bmad-output/planning-artifacts/epics.md#Story 5.2] — AC: export = serialized data contract (JSON) + `schema_version` + integrity hash (not a raw .db); import = identity preserved on round-trip, version/integrity mismatch rejected or migrated with a clear message (FR59, NFR-R5).
- [Source: contract/src/study.rs] — `Study` is `Serialize`/`Deserialize`, carries `schema_version`; self-contained (id, journal_id, years, judgment, rationale).
- [Source: contract/src/versioning.rs] — `SCHEMA_VERSION = 1`; distinct from `user_version` / `method_version`.
- [Source: persistence/src/studies.rs] — `get_study(id)` (full read) + `put_study(&study)` (identity-preserving upsert).
- [Source: Cargo.toml] — `sha2 = "0.11"` workspace dep (core/ingestion already use it); `serde_json` workspace dep.

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List

### Change Log

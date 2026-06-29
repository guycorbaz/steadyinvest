# Story 5.3: Export / import the whole journal

Status: done (3-layer review 2026-06-29 — 4/4 ACs; 6 patches applied [surface source identity; portfolio-inserted guard; watchlist repack; deny_unknown_fields; per-study version gate; fail-closed export], 1 deferred → #65; workspace 546 tests, fmt/clippy -D/deny green; NO core/migration/SCHEMA_VERSION change; Cargo.lock +2 edges [serde+sha2, no new package])

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to export and import my entire journal,
so that I can move or seed all my work at once.

## Scope decision (Guy, 2026-06-29)

**"Whole journal" = ALL journaled entities, the faithful FR60 reading.** The export carries every
entity the journal owns — **studies** (full, incl. judgments + years), **watchlist items**,
**portfolio(s)**, **holdings** (incl. soft-deleted/sold ones, see the edge below), and **sell
transactions** — wrapped in one versioned, integrity-checked envelope carrying the journal's
`(journal_id, logical_version, schema_version, hash)`. A studies-only export was rejected: it would
silently drop the user's watchlist and portfolio, an import surprise. `fx_rates` is **inert** (no FX
until Epic 6) → not exported in v1 (documented limitation, not a gap).

**Import is an atomic, identity-preserving MERGE/SEED into the current journal** — NOT a destructive
replace (destructive whole-journal **restore** is Story 5.4). All entities are upserted by their own
`id` inside **one SQLite transaction**: all-or-nothing, **never partially applied** (NFR-R5). A
foreign `journal_id` on entities is rebound to the current journal (the same seed/share semantics
Story 5.2 established for a single study).

## Acceptance Criteria

1. **AC1 — The whole-journal export envelope (versioned, integrity-checked, carries `(journal_id, version, hash)`, FR60).** Exporting writes a **single JSON file** = a `JournalSnapshot` payload (the journal's `journal_id` + `logical_version` + every entity: `Vec<Study>`, `Vec<WatchItem>`, `Vec<PortfolioItem>`, `Vec<HoldingItem>`, `Vec<TransactionItem>`) wrapped in an envelope carrying the **`schema_version`** it was written under, the journal's **`journal_id`** and **`logical_version`**, and an **integrity hash** (SHA-256 over the canonical serialized payload bytes). **NOT a raw `.db` copy** (architecture §"Export / backup format"). The hashing reuses the **same SHA-256 helper as Story 5.2** (`contract::export::sha256_hex`, made `pub`) so there is one hashing implementation; the envelope itself lives in **`persistence`** (the only layer that can see all entity types — `contract` must not depend on `persistence`). The export reads the **complete** journal, including **soft-deleted (sold) holdings** so their sell transactions keep a live FK referent on import.

2. **AC2 — Import verifies integrity + version, applies atomically, identity preserved (FR60, NFR-R5).** Importing parses the envelope, **recomputes the hash and rejects a mismatch** (corruption / incomplete file) with a clear neutral message, and **checks `schema_version`**: equal → accept; **mismatch → reject** with a clear message (the forward-migration hook is structured via the typed error, but with `SCHEMA_VERSION == 1` only the equal case accepts; an unknown/newer version is refused, never silently coerced). On accept, **every entity is upserted (by its own `id`) into the current journal inside ONE transaction** — if any row fails, the whole import rolls back (**never partially applied**). Entities carrying a foreign `journal_id`/`portfolio_id` are **rebound** to the current journal so the import seeds/joins it; each entity's **own `id` is preserved** (a re-import updates in place, no duplicates). A malformed / non-envelope / truncated file is a **typed refusal, never a panic**. Foreign-key order is respected (portfolios before holdings; holdings before transactions; studies before the watchlist rows that link them — or FKs are deferred within the transaction).

3. **AC3 — App surface: export & import the journal, neutral outcomes with counts.** Réglages (and/or the dashboard) offers **Exporter le journal** (write the envelope to the export folder, **never** the live DB dir — ADD7/8) and **Importer un journal** (path → validate → apply). Outcomes are **neutral, posture-gated facts** (FR13): success names what happened **with counts** ("journal importé : N études, M valeurs suivies, P lignes de portefeuille…"); a rejection names the cause (integrity / version / unreadable) without leaking internals. The imported file's `(journal_id, logical_version)` is surfaced (so the user sees whether it is the same journal or a foreign seed). Every new literal goes through `@tr`; the floor is bumped by exactly the number added; any new `MSG_*` is registered.

4. **AC4 — `core`/method untouched; no schema change; minimal dependency footprint.** The path is **`persistence` (envelope + read-all + atomic import) + `contract` (expose the shared hash helper; add serde derives where missing) + `app` (file IO + UI)**. **No `core::ssg` change** (fingerprint/golden/determinism gates stay green), **no migration** (`PRAGMA user_version` unchanged — export reads existing tables, import writes via existing/new upserts within the current schema), **no `contract::SCHEMA_VERSION` bump** (the envelope wraps the *current* contract). The **only** dependency change is `sha2 = { workspace = true }` added to `persistence/Cargo.toml` (workspace-resolved → **no new package**, expected `Cargo.lock` +1 edge only, like Story 5.2's `contract`; `deny.toml` unchanged, `cargo deny` green). File reading/writing lives in `app` (`std::fs`, no new dep — the native picker is Story 5.5). Copy neutral, posture-gated.

## Tasks / Subtasks

- [x] **Task 1 — Make the entity types serializable + expose the shared hash helper (AC1, AC4)** — `persistence/src/`, `contract/src/export.rs`
  - [x] Add `#[derive(Serialize, Deserialize)]` to `WatchItem`, `PortfolioItem`, `HoldingItem`, `TransactionItem` (they currently derive only `Debug, Clone, PartialEq, Eq`). Their fields are serde-friendly (`Uuid`, `String`, `Option<String>`, `i64`, `Option<Uuid>`, `contract::Timestamp` — `Timestamp` already serializes via `Study`). `contract::Study` is already `Serialize`/`Deserialize`. Confirm `cargo build` after.
  - [x] In `contract/src/export.rs`, change `fn sha256_hex` → `pub fn sha256_hex` (one hashing implementation, reused by the journal envelope; re-export from `contract::export`). No behaviour change → Story 5.2 tests stay green.
  - [x] Add `sha2 = { workspace = true }` to `persistence/Cargo.toml` (workspace-resolved; confirm `git diff Cargo.lock` shows only the +1 edge, no new package; `deny.toml` untouched).

- [x] **Task 2 — `persistence`: read the complete journal (AC1)** — `persistence/src/`
  - [x] New `persistence/src/export.rs` (module `export`, registered in `lib.rs`). `JournalSnapshot { schema_version: u32, journal_id: Uuid, logical_version: u64, studies: Vec<Study>, watch_items: Vec<WatchItem>, portfolios: Vec<PortfolioItem>, holdings: Vec<HoldingItem>, transactions: Vec<TransactionItem> }` (serde). `JournalExport { schema_version: u32, journal_id: String, logical_version: u64, integrity_hash: String, payload: String }` (the envelope; `payload` = canonical JSON of `JournalSnapshot`, `integrity_hash` = `sha256_hex(payload bytes)` — hash the payload, not the envelope, to avoid a hash-over-self cycle, per Story 5.2).
  - [x] `Journal::export_journal(&self) -> Result<String>`: gather **all** entities and serialize the envelope. Add the read-all helpers that don't exist yet:
    - Full studies: iterate `list_studies()` → `get_study(id)` (full incl. judgment + years), or a new `list_full_studies()`. Note the choice.
    - `list_portfolios(&self) -> Result<Vec<PortfolioItem>>` (today only `first_portfolio`/`ensure_portfolio` exist; single-portfolio now but read all rows for future-proofing).
    - **All holdings incl. sold**: `list_holdings` filters `sold_at IS NULL` — export needs an **unfiltered** read (`list_all_holdings` ignoring `sold_at`) so a sold holding (still a live FK referent for its sell row) is carried; otherwise its transaction orphans on import.
    - All transactions: `list_all_transactions(&self) -> Result<Vec<TransactionItem>>` (today only `list_transactions(holding_id)` per holding).
  - [x] Watchlist: `list_watch_items()` already returns all rows.
  - [x] Unit/integration tests: a populated journal exports a non-empty envelope whose payload deserializes back to an equal `JournalSnapshot`; the hash is **deterministic** for the same journal (stable serialization — `JournalSnapshot` uses `Vec`s in a fixed read order, no `HashMap`/`BTreeMap`); a journal with a **sold holding** carries that holding **and** its sell transaction.

- [x] **Task 3 — `persistence`: atomic, identity-preserving import (AC2, AC4)** — `persistence/src/export.rs`
  - [x] `Journal::import_journal(&mut self, text: &str) -> Result<ImportSummary>` where `ImportSummary { source_journal_id: Uuid, source_logical_version: u64, studies: usize, watch_items: usize, portfolios: usize, holdings: usize, transactions: usize }`.
    - Parse the envelope (reuse `contract::export::ImportError` — `Integrity`/`Version`/`Malformed` — or a persistence-side equivalent if a `JournalIdentity` note is wanted; reuse keeps one taxonomy). **Recompute** the hash over `payload`, reject `Integrity` on mismatch. Check `schema_version == contract::SCHEMA_VERSION`, reject `Version { found, supported }` otherwise. Deserialize `JournalSnapshot`, reject `Malformed` on failure.
    - **Apply in ONE transaction** (`self.conn.transaction()?` — the established per-mutation pattern): upsert every entity by its `id`, **rebinding** `Study.journal_id` / `HoldingItem.portfolio_id` (to the imported/ensured portfolio) / watchlist + holding ownership to the **current** journal. Respect FK order (portfolios → holdings → transactions; studies → watchlist rows referencing them) **or** use deferred FKs within the tx. Commit once → bump `logical_version` once (mirror the existing mutators). On ANY error, the tx drops → **nothing applied** (test this).
    - Guard: a **read-only** journal (newer-on-disk schema) refuses with the existing neutral notice; never panics.
  - [x] Tests: round-trip — populate journal A, export, import into an **empty** journal B → B holds an **equal** set (same ids, studies, watchlist, holdings incl. sold, transactions). A second import is an **idempotent update** (no duplicates). A tampered payload / bumped `schema_version` / garbage string each yields the right typed refusal and **writes nothing** (assert counts unchanged). **Atomicity:** inject a failing row (e.g. a transaction referencing a missing holding when FK order is wrong) → assert the whole import rolled back (no studies, no watchlist, etc. applied).

- [x] **Task 4 — App state: the export/import-journal rail (AC2, AC3)** — `app/src/state.rs`
  - [x] `export_journal(&self) -> Result<String, String>` (call `Journal::export_journal`; guarded — no-journal → neutral notice). Pure read.
  - [x] `import_journal(&mut self, text: &str) -> Result<ImportSummary, String>` (call `Journal::import_journal`, mapping each `ImportError` to a neutral `MSG_*`; then refresh in-memory views). Read-only / no-journal guarded. New `MSG_JOURNAL_EXPORTED` / `MSG_JOURNAL_IMPORTED` (the latter formats the counts) / `MSG_IMPORT_*` — **reuse** the Story 5.2 `MSG_IMPORT_INTEGRITY` / `MSG_IMPORT_VERSION` / `MSG_IMPORT_MALFORMED` if their wording fits (they should — same taxonomy); register only genuinely-new messages in `USER_FACING_MESSAGES`.
  - [x] Tests: a tampered / wrong-version / garbage string each maps to the right neutral notice and writes nothing; a good string imports and returns the summary counts.

- [x] **Task 5 — main.rs + Slint: the file actions (AC3)** — `app/src/main.rs`, `app/ui/`
  - [x] Export → write the envelope string to `ProjectDirs.data_dir()/exports/journal-<journal_id>.json` (the Story 5.2 path-based pattern; **never** the live DB dir — ADD7/8). Import → read the string from a path `TextField`. File IO via `std::fs` (no new dep; native picker is Story 5.5).
  - [x] Slint: **Exporter le journal** / **Importer un journal** actions (Réglages, next to / mirroring the Story 5.2 single-study export, or the dashboard), neutral copy, posture-gated; surface the outcome notice **with counts** and the imported `(journal_id, logical_version)`. Re-render the dashboard + portfolio + watchlist after a successful import (new/updated entities appear). `@tr` floor + `MSG_*` inventory bumped by exactly the number added.

- [x] **Task 6 — Gates (AC4)** — fmt, clippy `-D`, `test --workspace`, `deny` + smoke launch. Confirm `core::ssg` re-diffs clean; **`Cargo.lock` is +1 edge only** (`sha2` under `steadyinvest-persistence`, no new package — verify `git diff main -- Cargo.lock`); **`deny.toml` unchanged**; **no migration** (`user_version` unchanged); **`contract::SCHEMA_VERSION` stays 1**; `@tr` floor + `USER_FACING_MESSAGES` inventory bumped exactly.

## Dev Notes

### Scope (builds directly on Story 5.2's seam)
This **reuses the Story 5.2 envelope pattern** (serialized data contract + `schema_version` + SHA-256 integrity hash, identity-preserving round-trip, typed refusals) and **scales it from one study to the whole journal** + the journal identity tuple `(journal_id, logical_version, hash)`. **Provider-independent** (no EODHD, no price history) — it proceeds now while the provider decision (D2) stays open. The single-study export/import (5.2) is left intact and unchanged.

### Out of scope (deferred)
- **Destructive whole-journal REPLACE / restore-from-backup** (FR61/FR66) → **Story 5.4**. 5.3 imports as an atomic **merge/seed** (upsert by id) into the current journal; 5.4 owns the "overwrite good data only after integrity+version checks, show `(journal_id, version)`, surface a stale backup" replace flow.
- **Schema-version MIGRATION on import** — with `SCHEMA_VERSION == 1` there is nothing to migrate; import **rejects** a mismatch with a clear message. The hook is structured (the `Version` error carries `found`/`supported`).
- **Native file picker + user-chosen sync-folder target** → **Story 5.5** (would add `rfd`, a new dep). 5.3 stays path-based, like 5.2.
- **`fx_rates`** — inert until Epic 6 (no FX) → not exported in v1.
- **Confront mode / price history** (FR50, ADD13) → Story 5.1. **PDF export** → Story 5.6.

### Architecture decisions this story honours
- [Source: architecture.md §"Export / backup format (decided — point 1)"] — the portable export unit is the **serialized serde data contract (JSON) + `schema_version` + integrity hash**, NOT a raw `.db`. A raw `.db` copy stays the file-level NAS backup unit (Story 5.4); the JSON export is the exchange/seed unit.
- [Source: architecture.md §"Identity & integrity"] — `journal_id` (UUID) + monotonic `logical_version` are written INTO the DB at creation; **backups/exports carry `(journal_id, version, hash)`**. The envelope surfaces all three.
- [Source: architecture.md §"Three version axes"] — `schema_version` (serialized contract) is distinct from SQLite `user_version` and `method_version`; the envelope carries `schema_version` only (and the journal's `logical_version` for identity, not as a migration axis).
- [Source: architecture.md §"contract crate"] — `contract` is the versioned serde boundary, decoupled from Slint and rusqlite, and **must not depend on `persistence`**. The whole-journal envelope therefore lives in **`persistence`** (which already depends on `contract` + `serde_json`); only the shared `sha256_hex` helper is borrowed from `contract::export`. Callers own file IO (the dialogs + `std::fs` live in `app`).
- [Source: architecture.md §"sync-safety ADD7/8"] — exports/backups go to the (Synology) sync folder, never the live DB dir; the export defaults away from the live DB directory.

### Where things live
- **`persistence/src/export.rs`** (new): `JournalSnapshot` + `JournalExport` envelope + `Journal::export_journal` / `Journal::import_journal` + `ImportSummary`. Plus the read-all helpers (`list_portfolios`, `list_all_holdings`, `list_all_transactions`, full-studies read).
- **`persistence/Cargo.toml`**: `sha2 = { workspace = true }` (+1 lock edge, no new package).
- **`contract/src/export.rs`**: `sha256_hex` made `pub` (the one hashing impl); reuse `ImportError`.
- **serde derives** added to `WatchItem` / `PortfolioItem` / `HoldingItem` / `TransactionItem` (`persistence`).
- **`app/src/state.rs`**: `export_journal` / `import_journal` rails + neutral notices (reuse 5.2 `MSG_IMPORT_*`).
- **`app/src/main.rs` + `app/ui/`**: the path-based file actions + the two Slint actions + outcome notice with counts.

### Notes & guardrails
- **Atomicity is the headline AC (NFR-R5 "never partially applied").** Wrap the entire multi-entity upsert in ONE `self.conn.transaction()`; commit once; bump `logical_version` once. Test the rollback path explicitly (a mid-import failure leaves the journal byte-untouched).
- **Sold holdings are part of the journal.** `list_holdings` hides `sold_at IS NOT NULL` rows (Story 4.7 soft-delete), but their **sell transactions reference them by FK**. Export must read **all** holdings (unfiltered) or the import will orphan a transaction. This is the single most likely-missed edge — call it out in the dev record.
- **FK order on import.** `transactions.holding_id → holdings`, `holdings.portfolio_id → portfolios`, `watchlist_items.study_id → studies (nullable)`. Insert parents before children within the transaction, or set deferred FK enforcement for the tx. A wrong order is the obvious atomicity test.
- **Identity round-trip & rebind (Story 5.2 precedent).** Each entity is upserted by its **own `id`** (re-import updates, no duplicate). Foreign `journal_id` (on studies) is rebound to the current journal — same seed/share semantics as `import_study`. `HoldingItem.portfolio_id` rebinds to the imported/ensured portfolio id.
- **Hash determinism.** `JournalSnapshot` is all `Vec`s serialized in a fixed read order (e.g. `ORDER BY created_at, id`) and contains no `HashMap`/`BTreeMap` → the same journal yields the same bytes and hash across runs/OSes (the Story 5.2 determinism property, verified there for `Study`).
- **No secrets in errors (NFR-S1).** `ImportError` / refusals carry only ints + a generic detail; never file contents or paths.
- **Merge vs replace.** 5.3 = **merge/seed** (additive upsert, current journal survives + absorbs). Destructive replace is 5.4. Make this explicit in the success copy (counts of what was added/updated), so the user is never surprised that their existing entries remained.

### Manual on-display GO/NO-GO (Guy)
Export the journal → a `journal-<id>.json` appears in the export folder; open it (human-readable: `journal_id`, `logical_version`, `schema_version`, a hash, and every study/watchlist/holding/transaction). On a **fresh empty journal**, import the file → all studies, watchlist rows, portfolio + holdings (including any you'd sold) and sell transactions reappear, identical. Import the **same file again** → counts say "updated", no duplicates. Hand-edit one digit in the file → import is **refused** with an integrity notice, **nothing** changes (check the dashboard is untouched). Bump the `schema_version` in the file → refused with a version notice. Import a random non-JSON file → neutral "unreadable" refusal, no panic.

### Project Structure Notes
- Additive `persistence::export` module + read-all helpers + serde derives + the workspace `sha2` dep (no new package). App state + path-based file actions + Slint actions. **No `core` change, no migration, no `SCHEMA_VERSION` bump, no new external dependency.**
- Posture floors at story start: `@tr` floor **285** (after Story 5.2), `USER_FACING_MESSAGES` inventory **55**. Bump both by exactly the number of new literals/notices (reuse 5.2's `MSG_IMPORT_*` where wording fits).

### References
- [Source: _bmad-output/planning-artifacts/epics.md#Story 5.3] — export = versioned format carrying `(journal_id, version, hash)`; import = validated, rejected/migrated on version mismatch, **never partially applied** (FR60, NFR-R5).
- [Source: _bmad-output/planning-artifacts/prd.md] — FR60 (export/import whole journal, validated on import); NFR-R5 (export/import + restore verify integrity & schema version; a mismatched/corrupt file is rejected, never partially applied).
- [Source: _bmad-output/implementation-artifacts/5-2-export-import-single-study.md] — the envelope + integrity-hash + typed-refusal + identity-rebind pattern this story scales up; `contract::export::{StudyExport, sha256_hex, ImportError}`.
- [Source: contract/src/export.rs] — `sha256_hex` (make `pub`), `ImportError { Integrity, Version { found, supported }, Malformed(String) }`, the hash-the-payload-not-the-envelope rule.
- [Source: contract/src/versioning.rs] — `SCHEMA_VERSION = 1`; distinct from `user_version` / `method_version`.
- [Source: persistence/src/journal.rs] — `Journal::id()`, `logical_version() -> Result<u64>`, `is_read_only()`; `self.conn.transaction()` per-mutation pattern that bumps the version.
- [Source: persistence/src/studies.rs] — `list_studies()` (summaries), `get_study(id)` (full), `put_study(&study)` (identity-preserving upsert).
- [Source: persistence/src/watchlist.rs] — `WatchItem`, `list_watch_items()` (all rows), `add_watch_item`.
- [Source: persistence/src/holdings.rs] — `PortfolioItem`, `HoldingItem` (decimals as TEXT strings), `first_portfolio`, `list_holdings(portfolio_id)` (filters `sold_at IS NULL` — export needs an unfiltered read).
- [Source: persistence/src/transactions.rs] — `TransactionItem` (sell rows), `list_transactions(holding_id)`; sells soft-delete their holding atomically (Story 4.7) — so a sold holding must be exported for its transaction's FK.
- [Source: Cargo.toml] — `sha2 = "0.11"` workspace dep (contract/core/ingestion already use it); `serde_json` workspace dep (persistence already declares it).

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — `claude-opus-4-8[1m]`

### Debug Log References

- **The envelope lives in `persistence`, not `contract` (as the story anticipated).** `contract` must
  not depend on `persistence`, but the whole-journal snapshot aggregates persistence-owned entity
  types (`WatchItem`/`PortfolioItem`/`HoldingItem`/`TransactionItem`). So `persistence::export` owns
  `JournalSnapshot`/`JournalExport`/`export_journal`/`import_journal`, **borrowing only** the one
  hashing helper `contract::export::sha256_hex` (made `pub`) and reusing `contract::ImportError`.
- **Three honest fidelity gaps the literal `Vec<Study>` plan would have lost — fixed:**
  1. **Sold holdings.** `list_holdings` filters `sold_at IS NULL`; a faithful export needs the
     unfiltered set or a sold holding's sell transaction orphans on import. Added `sold_at:
     Option<String>` to `HoldingItem` (it is genuinely part of a holding's state) + a new
     `list_all_holdings()` (unfiltered). `list_holdings`/`add_holding` set `sold_at: None`.
  2. **Study lifecycle status.** A study's `status` (active/archived) is an indexed column, NOT part
     of the `Study` blob — so the snapshot wraps each study in `StudyRecord { status, study }` and the
     import restores it (a round-tripped archived study re-imports archived). `import_journal`'s
     studies upsert therefore DOES update `status` (unlike `put_study`, which preserves it).
  3. **Read-all helpers added:** `list_portfolios`, `list_all_holdings`, `list_all_transactions`.
- **Single-portfolio import rule (FR36).** All imported holdings attach to **one** portfolio: the
  current journal's existing one if present (merge into it), otherwise the imported portfolio (id
  preserved — clean seed into an empty journal). Extra imported portfolios (none in single-portfolio
  v1) are not created — multi-portfolio is Epic 6.
- **Atomicity proven, not asserted.** `import_journal` runs every upsert in ONE `self.conn
  .transaction()`; FK order is portfolios → studies → holdings → transactions → watchlist. The test
  `a_failing_import_rolls_back_completely_never_partial` injects a transaction with a dangling
  `holding_id` (FK violation, `foreign_keys=ON`) and asserts the journal is byte-untouched afterward
  (no studies/holdings/watch/portfolio, no `logical_version` bump). The bump happens **once** for the
  whole act, and only when the snapshot carried something (an empty snapshot is a true no-op).
- **`Cargo.lock` is +2 edges, not +1.** Adding the workspace `serde` (for the derives) **and** `sha2`
  (for hashing) to `persistence/Cargo.toml` records two dependency edges under
  `steadyinvest-persistence` — **no new package/version** is downloaded (both were already resolved
  via `contract`/`core`/`ingestion`). `deny.toml`/`cargo deny` unchanged.
- **Persistence error surface.** Added `Error::{ImportIntegrity, ImportVersion, ImportMalformed}` (+ a
  `From<contract::ImportError>`), with neutral messages; the posture inventory (`sample_errors` +
  exhaustive match + count 8→11) was updated. The app maps these to the **reused** Story 5.2
  `MSG_IMPORT_INTEGRITY`/`MSG_IMPORT_VERSION`/`MSG_IMPORT_MALFORMED` (same taxonomy, same wording).
- **`judgments` time-series + `fx_rates` are not exported.** Both are empty/inert in v1 (FR51 durable
  history deferred #34; no FX until Epic 6); the current judgment travels inside each `Study` blob.
- **Path-based UI (native picker is Story 5.5).** Export writes
  `ProjectDirs.data_dir()/exports/journal-<journal_id>.json` (away from the live DB — ADD7/8); import
  reads a path `TextField`. The actions live in **Réglages** (the `Prefs` global). A successful import
  re-renders the dashboard, watchlist and portfolio (and prunes per-holding freshness).

### Completion Notes List

- **AC1** — `persistence::export` (`JournalSnapshot`/`StudyRecord`/`JournalExport` + `export_journal`);
  SHA-256 over the canonical payload via the shared `contract::export::sha256_hex`; carries
  `(journal_id, logical_version, schema_version, hash)` + every entity incl. sold holdings + study
  status. Deterministic (all `Vec`s in fixed read order, no maps).
- **AC2** — `import_journal` verifies integrity → `schema_version` → applies **every entity in ONE
  transaction** (upsert by id, rebind foreign `journal_id`/portfolio); rolls back completely on any
  failure (atomicity test); typed refusals, never a panic; re-import is an idempotent update.
- **AC3** — Réglages "Journal complet" panel: **Exporter le journal** / path-field **Importer un
  journal**; neutral outcome with per-entity counts (`journal_imported_message`) and the source
  `(journal_id, version)` in the envelope. 2 new `MSG_*` (inventory 55→57, reusing the 5.2
  `MSG_IMPORT_*`); `@tr` floor 285→290 (+5).
- **AC4** — no `core::ssg` change (fingerprint/golden/determinism green), **no migration**
  (`user_version` unchanged), **no `SCHEMA_VERSION` bump**, `deny.toml` unchanged; `Cargo.lock` +2
  edges only (`serde` + `sha2` under persistence, no new package). Workspace **543 tests** green; fmt /
  clippy `-D` / deny clean; smoke launch exit 124.

### File List

- `contract/src/export.rs` (M) — `sha256_hex` made `pub`
- `contract/src/lib.rs` (M) — re-export `sha256_hex`
- `persistence/Cargo.toml` (M) — `serde` + `sha2` workspace deps
- `persistence/src/export.rs` (A) — `JournalSnapshot`/`StudyRecord`/`JournalExport`/`ImportSummary` +
  `export_journal`/`import_journal` + `parse_and_verify` + 8 tests (round-trip, idempotent re-import,
  deterministic hash, integrity/version/malformed refusals, atomic rollback, empty-snapshot no-op)
- `persistence/src/lib.rs` (M) — register `export` module + re-exports
- `persistence/src/error.rs` (M) — `ImportIntegrity`/`ImportVersion`/`ImportMalformed` + `From` + posture inventory 8→11
- `persistence/src/holdings.rs` (M) — `serde` derives; `HoldingItem.sold_at`; `list_portfolios`; `list_all_holdings`
- `persistence/src/transactions.rs` (M) — `serde` derive; `list_all_transactions`
- `persistence/src/watchlist.rs` (M) — `serde` derives on `WatchItem`
- `app/src/state.rs` (M) — `export_journal`/`import_journal` rails + `journal_id()` accessor + 2 `MSG_*` + `journal_imported_message` + 3 tests
- `app/src/main.rs` (M) — `write_journal_export` helper + `on_export_journal`/`on_import_journal` callbacks (re-render all surfaces)
- `app/src/posture.rs` (M) — `@tr` floor 285→290, message inventory 55→57
- `app/ui/state.slint` (M) — `Prefs` whole-journal callbacks/props (`export-journal`/`import-journal`/`journal-status`/`journal-import-path`)
- `app/ui/screens/settings.slint` (M) — Réglages "Journal complet" panel

### Review Findings (3-layer adversarial — 2026-06-29)

3 layers (Blind Hunter / Edge Case Hunter / Acceptance Auditor). **No CRITICAL/HIGH.** SQL param/column
counts balance, FK insert order correct, atomicity/rollback proven, hash-over-payload sound, AC1/AC2/AC4
confirmed PASS. **6 patches applied · 1 deferred (#65) · rest dismissed (by-design / false positives).**

- [x] [Review][Patch] Surface the imported file's `(journal_id, logical_version)` (AC3 required it; only counts were shown) — `MSG_JOURNAL_IMPORTED` + `journal_imported_message` now append "(source : journal {jid}, version {ver})". [app/src/state.rs] — MED
- [x] [Review][Patch] Phantom `logical_version` bump + bogus portfolio count when an empty snapshot merges into a journal that already has a portfolio — the guard keyed off `target_portfolio_id.is_some()` (existence) instead of a real INSERT. Now tracks `portfolio_inserted`; `applied` and `summary.portfolios` use it. [persistence/src/export.rs] — MED
- [x] [Review][Patch] Watchlist `position` collisions on a merge into a non-empty journal (no UNIQUE, import never repacked) — added `repack_watchlist_positions` after the import, inside the tx. [persistence/src/export.rs] — MED
- [x] [Review][Patch] No `deny_unknown_fields` → a future-format file (new entity array, same `schema_version`) would pass the version gate and serde would silently drop the array — added `#[serde(deny_unknown_fields)]` to `JournalExport`/`JournalSnapshot`/`StudyRecord` (reject, don't silently drop). [persistence/src/export.rs] — MED
- [x] [Review][Patch] Per-study blob `schema_version` not gated → a study newer than this build would be written then be unreadable (`NewerRowSchema`). Import now rejects `study.schema_version != SCHEMA_VERSION` up front (rolls back). [persistence/src/export.rs] — LOW
- [x] [Review][Patch] Export silently dropped a study if `get_study` returned `None` (fail-open) — now fails closed with a clear `CorruptPayload` error. [persistence/src/export.rs] — LOW
- [x] [Review][Defer] Re-importing an **older** envelope resurrects a sold holding / un-archives a study (no version arbitration) — deferred → **GitHub #65** (Story 5.4 restore owns stale-backup + version-mismatch detection; 5.3 is an atomic merge by design). [persistence/src/export.rs] — MED

**Dismissed (with rationale):** version bump per explicit import (= one deliberate user act, distinct from
the phantom sync writes the C4 idempotency lesson targets); `NewerJournalSchema`→read-only notice
(consistent with 5.2, semantically correct); watchlist `study_id` dangling soft link (nullable, no FK,
the app tolerates a missing study by design); `method_version` NULL (always NULL in v1); portfolio
resolver `ORDER BY id` (false positive — `first_portfolio` also orders by id); `Cargo.lock` +2 (serde
required for the derives, no new package — documented).

3 new patch tests (empty-merge no-bump, watchlist repack on merge, per-study version gate). Workspace
**546 tests** green; fmt / clippy `-D` / deny clean; smoke launch exit 124.

### Change Log

- 2026-06-29 — Story 5.3 dev complete (6/6 tasks). Whole-journal portable export/import (`persistence`
  envelope reusing the 5.2 SHA-256 + `ImportError`); carries `(journal_id, logical_version,
  schema_version, hash)` + every entity (studies w/ status, watchlist, portfolio, holdings incl. sold,
  sell transactions); **atomic merge/seed import** (one transaction, never partially applied);
  path-based Réglages UI (native picker deferred to 5.5). Added `HoldingItem.sold_at` + read-all
  helpers; 3 persistence import-error variants reusing the 5.2 notices. Workspace 543 tests green; NO
  core/migration/SCHEMA_VERSION change; `Cargo.lock` +2 edges (no new package).
- 2026-06-29 — 3-layer review: 6 patches applied (surface source identity; portfolio-inserted guard;
  watchlist repack; deny_unknown_fields; per-study version gate; fail-closed export), 1 deferred (#65,
  older-file version arbitration → Story 5.4). 4/4 ACs satisfied. Workspace 546 tests green.

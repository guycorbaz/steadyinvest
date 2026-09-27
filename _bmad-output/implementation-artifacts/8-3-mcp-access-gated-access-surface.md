# Story 8.3: `McpAccess` — the gated access surface (headless)

Status: review

## Story

As the developer,
I want a persistence-level access type that can only read studies and only insert drafts,
so that capability asymmetry and portfolio non-exposure are enforced by the SQLite engine, not by
the MCP code's good behaviour.

## Acceptance Criteria

Source: `epics.md` Story 8.3 (after the G2 reviews and the 8.0 amendment `identifier_invalid`);
architecture §Phase 4 A2, A3, A11, O1; owner decisions D2, D4, D6, D8, D10; the "not in production"
rule (no compatibility work — memory 2026-09-27).

1. **Per-call, lock-free, version-gated (A2, FR67 [P4]).** Every `McpAccess` call opens the dossier,
   works, and closes it. It never takes the app's `-lock` sidecar, never migrates, never sets
   `journal_mode`, sets only `busy_timeout` and `foreign_keys = ON` (before the authorizer is
   installed), and refuses to run unless `PRAGMA user_version` **equals** this build's latest
   migration — a typed error naming both versions (older file → open it in the app first; newer
   file → this build is older than the dossier).
2. **Read connection (A2, O2).** Reads run in one short read transaction on a connection opened
   `SQLITE_OPEN_READ_ONLY`, in both WAL and DELETE journal modes, whether or not the app has the same
   dossier open. Lists are bounded (paged). The empty `-wal` / `-shm` a read leaves beside a closed WAL
   dossier is documented and accepted by the app's sidecar diagnostics (proved by a test: the app's
   `Journal::open` after an `McpAccess` read opens read-write, no `SidecarNotWritable`).
3. **Typed methods only (A3).** `McpAccess` is a type distinct from `Journal`, exposes typed methods
   only and never a `rusqlite::Connection` (nor anything that yields one). *The clippy
   `disallowed-types` / `disallowed-methods` boundary lives in the `mcp` crate's `clippy.toml` and
   lands with that crate in Story 8.4 (see Dev Notes §1); 8.3 guarantees the surface it will guard.*
4. **Read allowlist (A3, NFR-A2, NFR-S4).** The read authorizer allows `studies`, `judgments`,
   `journal_meta`, `ai_drafts` and SQLite's internal tables only; every other table — today
   `holdings`, `transactions`, `portfolios`, `watchlist_items`, `fx_rates`, `price_history`, and
   any later table — is `SQLITE_DENY` (never `SQLITE_IGNORE`). A test lists every table of the latest
   schema from `sqlite_master` and fails if one is not classified (allowed or denied on purpose).
5. **Draft connection (A3, FR14, FR68 [P4], NFR-A1).** Its authorizer allows only `INSERT` into
   `ai_drafts`, the reads the submission checks need (allowlist of AC 4), and `UPDATE journal_meta`
   (column `logical_version`) **only** when the authorizer's accessor is the trigger
   `trg_ai_drafts_bump_logical_version`. Every other action — `UPDATE` / `DELETE` on any table, a
   direct `logical_version` update, DDL, `ATTACH` (which also covers `VACUUM INTO`), `DETACH`,
   `PRAGMA` after setup, `ANALYZE`, `REINDEX`, virtual tables — is denied at statement preparation
   and **reported as a typed denial** (action + object) carried by the error, for the MCP server to
   log (Dev Notes §5).
6. **Study read (O1, FR69).** Reading a study returns its data cells with provenance, judgments,
   rationale, notes, its judgment history (paged) and its status. The computed outputs (zones, U/D,
   5-year potential, verdict and its state) are produced from that read by
   `report::form::build_snapshot` — the same construction as the app; *the O1 assembly into the MCP
   response is Story 8.4's, because `persistence` does not depend on `report` (Dev Notes §1); 8.3
   proves the path in its metamorphic suite (AC 12).*
7. **Submission checks in one `BEGIN IMMEDIATE` (A3).** A draft submission's checks and its insert run
   in one `IMMEDIATE` transaction; it is refused with a typed, named reason and **nothing written**
   when:
   - the `journal_id` or path it carries differs from the dossier resolved for the call (D10) →
     `dossier_mismatch`;
   - its target study does not exist → `study_not_found`; its comment is blank (Unicode whitespace
     and format characters, the 8.1 rule) → `empty_comment`; its client or model is blank →
     `missing_origin` (NFR-A4);
   - a draft study's identifier or currency is not valid (ticker `[A-Z0-9.\-]{1,20}`, currency three
     ASCII upper-case letters) → `identifier_invalid`;
   - its field is not a draftable field (the 8.2b `DraftField` registry) → `field_not_draftable`; its
     fiscal year is not a year of the study → `year_not_in_study`; its value does not parse in the
     field's unit → `value_unparsable`, or is not one of an option field's names →
     `value_not_an_option` (D6; `DraftField::parse_value`);
   - the target already has a pending draft → `target_has_pending` (D4);
   - a draft study whose security (identifier compared case-insensitively) is already studied in the
     same currency → `study_exists`, or already pending as a draft study in the same currency →
     `draft_study_pending` (D2, D8).
   The accepted draft is inserted `pending`, with the caller's id and `created_at` (this crate never
   generates identity or time, ADD15) and, for a cell / judgment draft, its `base_fingerprint`
   computed in the same transaction by `contract::draft_fingerprint` with the caller-supplied method
   version.
8. **Race with `delete_study`.** A test races a submission against the app's `delete_study` on the
   same study and never leaves an orphan pending draft (the check and the insert are one `IMMEDIATE`
   transaction; `foreign_keys = ON` backs it).
9. **Restore vs MCP (A11, NFR-R2, NFR-X1).** `restore_journal_file` first takes an **exclusive SQLite
   lock** on the live file (a connection that `BEGIN EXCLUSIVE`s with a busy wait) and holds it
   across the copy and the rename; every `McpAccess` write re-checks, inside its `IMMEDIATE`
   transaction, that the file at the resolved path is still the one it opened (a `same_file::Handle`
   taken at open) and aborts on mismatch → `dossier_replaced`. No draft is lost silently and the
   restored file's sidecars are never deleted by an MCP connection.
10. **Suites (CI, NFR-A1–A4).**
    - **whole-surface non-exposure** — every `McpAccess` read method, over a fixture dossier seeded
      with holdings, transactions (incl. a dividend), portfolios, watchlist items, FX rates and price
      history carrying unique marker strings, returns none of those markers, no key and no
      configuration value — the dossier identity (`journal_id` + resolved path) excepted;
    - **table classification** — AC 4;
    - **rejected writes** — every write other than a draft insert (study, cell, judgment, verdict,
      note, transaction, portfolio, watchlist, `UPDATE` / `DELETE` on `ai_drafts`, a direct
      `journal_meta` update, DDL, `ATTACH`, `VACUUM INTO`, `PRAGMA`) is denied by the engine with its
      typed denial; the dossier's bytes are unchanged;
    - **submission refusals** — each refusal of AC 7 (+ `dossier_replaced`, `schema_mismatch`), with
      its code;
    - **draft origin** — 100 % of drafts written by the suite carry a non-blank comment, client +
      model and a timestamp; attempts without them are refused.
11. **Identity is the caller's (ADD15).** `McpAccess` never calls a clock or a UUID generator; ids,
    `created_at` and the method version come from the caller (the MCP server in 8.4).
12. **Metamorphic pending drafts (FR72).** For every golden and fixture study available as a
    contract `Study`, `build_snapshot` of the study read through `McpAccess` is identical with and
    without any number of pending drafts of every kind; the engine path never opens `ai_drafts`.
13. **Gates.** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
    `cargo fmt --check`, `cargo deny check` green; the persistence error-sample posture inventory is
    re-based with its delta stated. No UI, no `@tr` / `MSG_*` change (the French MCP messages are
    8.4's, Dev Notes §5). No `SCHEMA_VERSION` bump, no migration (none needed: v8 is on `main`).

## Tasks / Subtasks

- [x] **T1 — Dependencies (AC 5, 9, 13)**
  - [x] T1.1 Workspace `Cargo.toml`: `rusqlite = { version = "0.40", features = ["bundled", "hooks"] }`
        (the authorizer is behind `hooks` in 0.40.1). Confirm the lockfile changes only by features.
  - [x] T1.2 `persistence/Cargo.toml`: add `same-file = "1.0.6"` (already in `Cargo.lock` via
        `walkdir`; licence `Unlicense/MIT`, both allowed by `deny.toml`). Add `steadyinvest-report`
        and `steadyinvest-core` as **dev-dependencies** only (AC 12), never `[dependencies]`.
  - [x] T1.3 `cargo deny check` green.
- [x] **T2 — Module skeleton (AC 1, 3, 11)**
  - [x] T2.1 New `persistence/src/mcp_access.rs`; `lib.rs` exports `McpAccess` and its public types
        (`DossierIdentity`, `McpStudyRead`, `Page`, `DraftSubmission`, `SubmissionRefusal`,
        `McpDenial`) — no connection type in any public signature.
  - [x] T2.2 `McpAccess::at(path)` stores the **resolved** path (`journal::resolved_path`) and nothing
        else; every method opens its own connection(s) and drops them before returning.
  - [x] T2.3 Open helper: `Connection::open_with_flags(path, READ_ONLY | NO_MUTEX)` for reads,
        `READ_WRITE | NO_MUTEX` (never `CREATE`) for the draft connection; then `busy_timeout = 5000`
        and `foreign_keys = ON`; then the version gate (`migrations::user_version` vs
        `migrations::latest_version(REGISTRY)`) → `Error::McpSchemaMismatch { file, supported }`;
        then install the authorizer. Never `journal_mode`, never `migrations::run_pending`, never
        `acquire_lock`.
- [x] **T3 — Authorizers (AC 4, 5)**
  - [x] T3.1 One `fn read_policy(ctx: AuthContext) -> Authorization` and one `draft_policy`, pure and
        unit-tested on synthetic `AuthContext`s (every `AuthAction` variant; `#[non_exhaustive]` →
        the wildcard arm is **Deny**).
  - [x] T3.2 Table sets as `const` slices beside the schema: `MCP_READABLE_TABLES` (`studies`,
        `judgments`, `journal_meta`, `ai_drafts`), `MCP_DENIED_TABLES` (the six portfolio / cache
        tables), SQLite internals by name (`sqlite_master`, `sqlite_schema`, `sqlite_sequence`,
        `sqlite_temp_master`). Export the trigger name from `schema.rs` as
        `pub(crate) const DRAFT_TRIGGER: &str = "trg_ai_drafts_bump_logical_version"` and use it in
        both the DDL doc and the policy (no second spelling).
  - [x] T3.3 Draft policy: `Insert { table_name: "ai_drafts" }` → Allow; `Update { table_name:
        "journal_meta", column_name: "logical_version" }` with `ctx.accessor == Some(DRAFT_TRIGGER)`
        → Allow; `Read` on the allowlist (the trigger's own read of `journal_meta` included) → Allow;
        `Select`, `Function`, `Transaction` (BEGIN / COMMIT / ROLLBACK), `Savepoint`, `Recursive` →
        Allow; everything else → Deny.
  - [x] T3.4 Denials are recorded: the authorizer closure pushes an `McpDenial { action, object }`
        into an `Arc<Mutex<Vec<_>>>` owned by the call; a statement that fails preparation with
        `SQLITE_AUTH` is mapped to `Error::McpDenied { denials }` (typed, carrying the list).
- [x] **T4 — Read methods (AC 2, 6)**
  - [x] T4.1 `identity() -> DossierIdentity { journal_id, path }`.
  - [x] T4.2 `list_studies(page) -> Paged<StudySummary>` (reuse the existing summary shape; add
        `LIMIT/OFFSET`, a hard cap such as 200 per page).
  - [x] T4.3 `read_study(id) -> Option<McpStudyRead { study, status }>`; `read_history(id, page)`
        (snapshots, newest first, bounded); `list_drafts(filter, page)` (by study / status).
  - [x] T4.4 Each read method runs in one `BEGIN DEFERRED` … `COMMIT` and parses through the same
        row mappers as `Journal` (share the private helpers; do not duplicate SQL).
- [x] **T5 — Submission (AC 7, 8, 9, 11)**
  - [x] T5.1 `DraftSubmission { id, created_at, kind, study_id, security_ticker, native_currency,
        company_name, target, proposed_value, note_text, comment, origin: DraftOrigin, dossier:
        DossierIdentity, method_version }`.
  - [x] T5.2 `submit_draft(sub) -> Result<Uuid, SubmitError>` where `SubmitError` is
        `Refused(SubmissionRefusal)` or `Failed(Error)`. `SubmissionRefusal` is an enum, one variant
        per code of AC 7 plus `DossierReplaced`, each carrying the data its message needs (8.4 renders
        the French text); `code(&self) -> &'static str` returns the stable snake_case code.
  - [x] T5.3 Order inside the `IMMEDIATE` transaction: file identity (`same_file::Handle::from_path`
        vs the handle taken before `BEGIN`) → dossier identity → blanks → identifier → study exists →
        field / year / value → pending target → duplicate study → build `DraftPayload` (version 1,
        `base_fingerprint` for cell / judgment) → `check_payload` → `INSERT` → `COMMIT`.
  - [x] T5.4 Duplicate study: `SELECT id, payload FROM studies WHERE lower(security_ticker) =
        lower(?1)`, then compare `native_currency` in Rust; pending draft studies: `SELECT … FROM
        ai_drafts WHERE kind = 'study' AND status = 'pending' AND lower(security_ticker) = lower(?1)
        AND native_currency = ?2`. Archived studies count as existing (question 1, default).
  - [x] T5.5 Pending target (D4): parse the payloads of the study's pending cell / judgment drafts and
        compare `(field, fiscal_year)`; note drafts are exempt (each creates a new note — question
        2, default).
  - [x] T5.6 Blank rule: move the 8.1 "whitespace or Unicode Cf only" predicate from
        `app/src/state/notes.rs` into `contract` (e.g. `contract::text::is_blank`) and use it in both
        places — one rule, no copy.
- [x] **T6 — Restore lock (AC 9)**
  - [x] T6.1 `restore_journal_file`: before the copy, open the live file read-write (if it exists),
        `busy_timeout`, `BEGIN EXCLUSIVE`; keep that connection until after the rename; then drop it
        and remove the live sidecars as today. Unix: rename while holding the lock. Windows: a file
        open by our own connection cannot be replaced — release just before the rename and rely on the
        MCP identity re-check (document the residual window, Dev Notes §4).
  - [x] T6.2 Test: a draft submission blocked behind the restore's exclusive lock completes after the
        swap with `dossier_replaced`, and the restored file carries no draft from it.
- [x] **T7 — Suites (AC 4, 5, 8, 10, 12)** — see Testing requirements.
- [x] **T8 — Record & posture (AC 13)** — new `Error` variants added to the persistence error-sample
      inventory with the delta stated; story record; sprint-status → review.

## Dev Notes

### 1. Where the boundaries sit (decisions made by this story file)

- **`McpAccess` lives in `persistence`** (A3: "a persistence-level type"), the only crate that
  touches SQLite. It returns contract data only.
- **Computed outputs (O1) are composed by the MCP server (8.4)**: `persistence` does not depend on
  `report` or `core` and must not start to (its manifest: "The only crate that touches SQLite";
  arch A1 lists `report` + `core` among the **mcp** crate's dependencies). 8.3 proves the path with
  `report` / `core` as dev-dependencies (AC 12). *Deviation from the epics wording "the response
  carries … computed outputs" — the response type of 8.3 carries the study; 8.4 adds the snapshot.*
- **The clippy boundary** (`disallowed-types = Journal`, `disallowed-methods =
  restore_journal_file / clear_lock / inspect_backup`) needs the `mcp` crate, which 8.4 creates. 8.3
  makes it enforceable: nothing in `McpAccess`'s public API yields a `Connection` or a `Journal`.
- **Logging**: `persistence` has no `tracing` dependency and must not gain one for this. Denials and
  refusals are **typed** (`Error::McpDenied { denials }`, `SubmissionRefusal`); 8.4 logs them to
  `steadyinvest-mcp.log` (arch A12).

### 2. rusqlite 0.40.1 authorizer (verified in the local registry)

- Feature: `hooks` (`#[cfg(feature = "hooks")] pub mod hooks;`).
- `Connection::authorizer<F>(&self, hook: Option<F>) -> Result<()>` with
  `F: for<'r> FnMut(AuthContext<'r>) -> Authorization + Send + 'static`.
- `AuthContext { action: AuthAction, database_name: Option<&str>, accessor: Option<&str> }` —
  `accessor` is "the inner-most trigger or view responsible for the access attempt", `None` for
  top-level SQL: this is the trigger-name check of AC 5.
- `AuthAction` is `#[non_exhaustive]`: `Read { table_name, column_name }`, `Insert { table_name }`,
  `Update { table_name, column_name }`, `Delete { table_name }`, `Pragma { pragma_name,
  pragma_value }`, `Attach { filename }`, `Detach`, `Transaction { operation }`, `Savepoint`,
  `Select`, `Function`, the `Create*` / `Drop*` / `AlterTable` / `Reindex` / `Analyze` /
  `CreateVtable` / `DropVtable` / `Recursive` variants and `Unknown`. Match what you allow; the
  wildcard is `Deny`.
- `Authorization::{Allow, Ignore, Deny}` — **never `Ignore`** (it reads NULLs silently).
- `SELECT count(*) FROM holdings` fires `Read` with an empty column name (verified during the G2
  review) — deny on the table name, whatever the column.
- The authorizer runs at **prepare** time: set pragmas and read `user_version` **before** installing
  it; the trigger body is authorized when the `INSERT` that fires it is prepared.
- rusqlite caches prepared statements (`prepare_cached`): per-call connections make the cache
  irrelevant, but never install the authorizer after preparing anything you will reuse.

### 3. Existing code this story touches

| File | Today | Change | Preserve |
|---|---|---|---|
| `persistence/src/journal.rs` | `open_with_mode`: lock → probe → version → pragmas → migrate; `apply_connection_local_pragmas` (busy 5000 + FK); `resolved_path` pub | reuse `resolved_path`, `apply_connection_local_pragmas` (make `pub(crate)`) | the app open path, unchanged |
| `persistence/src/migrations.rs` | `REGISTRY`, `latest_version`, `user_version` (`pub(crate)`) | read only | — |
| `persistence/src/schema.rs` | v8 DDL + trigger name in a string | `DRAFT_TRIGGER` const, table-set consts | the frozen DDL text (the name must not change) |
| `persistence/src/drafts.rs` | row mapping, `check_payload`, `is_currency_code`, `is_rfc3339_utc`; no insert "on purpose — inserts are McpAccess's" | reuse mappers / checks from `mcp_access.rs` | the read/decide API of 8.2a/8.2b |
| `persistence/src/studies.rs` | `delete_study` (drafts first, O7), `list_studies`, snapshots | reuse readers (factor out `&Connection`-level helpers if they are `&self` methods today) | behaviour, bump discipline |
| `persistence/src/restore.rs` | copy to `…-restore-incoming`, rename, remove sidecars; precondition "caller dropped every Journal" | add the exclusive-lock step (T6) | atomic copy-then-rename, error kinds |
| `app/src/state/notes.rs` | `is_format_char` / blank rule | move to `contract` (T5.6) | 8.1 behaviour and tests |
| `persistence/src/error.rs` | typed errors, `ErrorKind` | + `McpSchemaMismatch`, `McpDenied`, `DossierReplaced` (if not a refusal) | Display in English technical form (posture rule) |

`contract::draftable` (8.2b) is the **only** registry of draftable fields (`DraftField::from_key`,
`kind`, `unit`, `options`, `parse_value`, `of_target`), and `contract::draft_fingerprint(study,
target, method_version)` the only fingerprint. `DraftPayload::fits` and `check_payload` are the shape
rule. Do not re-implement any of them.

### 4. A11 on each platform

WAL mode: `BEGIN EXCLUSIVE` blocks other writers, not readers — MCP reads continue on the old file
until they close (harmless: per-call). DELETE mode: it blocks readers too (busy-wait). Unix: `rename`
over a file we hold open is allowed; hold the lock across it. Windows: our own open handle prevents
the replace; release immediately before `rename` — a submission that starts in that gap writes to
the old file only if its identity check ran before the rename; its re-check is inside the
`IMMEDIATE` transaction, so the window is the time between its check and its `COMMIT`. Document it
as a residual (Linux is the owner's platform; NFR-X1 keeps the behaviour correct everywhere else
the rename succeeds).

### 5. Refusal codes and messages

The codes and French messages are fixed by `ux-ai-assistance-surfaces.md` §3.3 (Q11: French
messages, English codes): `dossier_mismatch`, `dossier_replaced`, `schema_mismatch`, `no_dossier`
(8.4), `study_not_found`, `empty_comment`, `missing_origin`, `field_not_draftable`,
`year_not_in_study`, `value_unparsable`, `value_not_an_option`, `identifier_invalid`,
`target_has_pending`, `study_exists`, `draft_study_pending`, `write_denied`. 8.3 owns the **codes and
the data** of each refusal; 8.4 owns the French rendering, its posture scan and the log line.

### 6. Security notes

- A submitted `study_id` must be looked up (not trusted) and its ticker taken from the stored study
  for the row's `security_ticker` — never the AI's spelling for a note / cell / judgment draft.
- Ids written in canonical lower-case form (the v8 CHECK refuses anything else).
- `company_name`, `comment`, client and model are AI text: stored as given (after the blank checks),
  never interpreted; the app shows them only inside `AiFrame` (8.0 §4.1).
- The allowlist is the security boundary; the typed API is defence in depth.

### Testing requirements

- **Unit (in `mcp_access.rs`, `#[cfg(test)]`, with access to the private connection builder):**
  policy functions over every `AuthAction`; the rejected-writes suite executes raw SQL on the gated
  connections (`INSERT/UPDATE/DELETE` on every table, `UPDATE journal_meta SET logical_version=0`,
  `DELETE FROM ai_drafts`, `CREATE TABLE`, `ATTACH`, `VACUUM INTO`, `PRAGMA journal_mode`,
  `PRAGMA user_version=9`) and asserts `SQLITE_AUTH` + the recorded denial + the file's bytes
  unchanged (hash before/after).
- **Integration (`persistence/tests/mcp_access.rs`):** per-call behaviour (no `-lock` created; the
  app's `Journal` open at the same time keeps its lock and writes); WAL and DELETE dossiers;
  version gate both ways (reuse the `readonly_newer` fake v9 step); non-exposure markers over a
  seeded dossier (holdings, transactions incl. dividend, portfolios, watchlist, fx, price history);
  table classification from `sqlite_master`; every submission refusal with its code; draft origin
  100 %; the `delete_study` race (two threads, a barrier, many iterations; assert no pending draft
  without its study); the restore race (T6.2); sidecar acceptance (AC 2); metamorphic (AC 12) with
  `report::form::build_snapshot` as a dev-dependency over the persistence fixtures and every golden
  case that has a contract `Study` form (locate how `core/tests/golden/*.json` map to `Study`; if no
  mapping exists, use the persistence / app fixture studies and state it).
- **Determinism:** fixed ids and timestamps (ADD15); no sleeps for ordering — barriers.
- **Posture:** new `Error` variants in the persistence error-sample inventory (the 8.2b count was
  28); Display strings English and technical.

### Previous story intelligence (8.2a, 8.2b)

- Payload rule and newer-vs-corrupt distinction already exist (`check_payload`, `PayloadProblem`).
- Import validates fields with `is_currency_code` / `is_rfc3339_utc` — reuse for AC 7.
- Posture gate `no_error_display_reaches_a_user_string` matches placeholder names against `Err(x)`
  bindings — avoid binding names like `reason` near `{reason}` strings (8.2b debug log).
- clippy `too_many_arguments` → pass a struct (`DraftSubmission`), as 8.2b did with
  `DecisionContext`.
- The v8 trigger fires once per inserted row (+1 per MCP submission) — the counter is monotonic, not
  an exact count (owner, 2026-09-27; `util.rs`).
- Never `git add -A` (untracked export PDFs at the repo root); `CARGO_BUILD_JOBS=4`.

### Project Structure Notes

- New: `persistence/src/mcp_access.rs`, `persistence/tests/mcp_access.rs`, possibly
  `contract/src/text.rs` (blank rule).
- No new crate (the `mcp` crate is 8.4). No app change except the blank-rule move.

### References

- `_bmad-output/planning-artifacts/epics.md` — Story 8.3; Epic 8 preamble (dev-safety rule).
- `_bmad-output/planning-artifacts/architecture.md` — §Phase 4 A1, A2, A3, A4, A7, A11, A12; D2, D4,
  D6, D8, D10.
- `_bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md` — §3.3 MCP refusal codes; §4.1
  exemptions (identifier rule).
- `persistence/src/{journal.rs, restore.rs, drafts.rs, schema.rs, migrations.rs, studies.rs,
  util.rs, error.rs}`; `contract/src/{draft.rs, draftable.rs, ai.rs}`; `report/src/form.rs:230`
  (`build_snapshot`); `app/src/state/notes.rs` (blank rule).
- rusqlite 0.40.1 `src/hooks/mod.rs` (authorizer); same-file 1.0.6 (`Handle::from_path`, `PartialEq`).
- `docs/review-checklist.md` — §1 (named refusals, no silent `.ok()`), typed errors, tests that can
  fail.

### Questions for Guy (defaults applied)

1. **Archived studies and duplicates:** a draft study for a security whose study is archived (same
   currency) is refused as `study_exists`. Default: yes — the archived study is still in the
   dossier.
2. **Several note drafts on one study:** allowed (each creates a new note; D4 applies to cell /
   judgment targets). Default: allowed.
3. **Windows restore window (A11):** the lock is released just before the rename on Windows; the
   residual window is documented, not closed. Default: accept (Linux is the owner's platform).

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5)

### Debug Log References

- `VACUUM INTO` passes statement **preparation**: the ATTACH behind it is authorized when the
  statement runs. The rejected-writes suite therefore EXECUTES each statement (not only prepares
  it); `VACUUM` and `VACUUM INTO` are then denied by the engine and no file is written. The module
  doc says "at preparation (or, for the ATTACH behind `VACUUM`, when it runs)".
- The bundled SQLite enables foreign keys by default: the marker seeding of the non-exposure suite
  turns them off on its raw connection (marker rows, not a coherent portfolio).
- The 8.2b judgment-writer guard matched `DraftTarget::Judgment { .. }` (a `matches!` pattern) as a
  functional `Judgment { .. }` update — the pattern spells `{ field: _ }` instead.

### Completion Notes List

- Ultimate context engine analysis completed — comprehensive developer guide created.
- `McpAccess` (`persistence/src/mcp_access.rs`): typed methods only (`at`, `path`, `identity`,
  `list_studies`, `read_study`, `read_history`, `list_drafts`, `submit_draft`); no public
  signature yields a `Connection` or a `Journal`. Per-call gated connections: flags (never
  `CREATE`), `busy_timeout` + `foreign_keys` (the journal's own helper), the `user_version`
  equality gate (`Error::McpSchemaMismatch`, newer = `NewerData`), THEN the authorizer.
- Authorizers: `read_policy` (allowlist `MCP_READABLE_TABLES` + SQLite catalogue; `Select`,
  `Function`, `Transaction`, `Recursive`) and `draft_policy` (+ top-level `INSERT` into
  `ai_drafts`, + `UPDATE journal_meta.logical_version` only with accessor `DRAFT_TRIGGER`);
  wildcard = Deny, never Ignore. Denials are recorded per call and returned as
  `Error::McpDenied { denials }` (action + object) for the 8.4 log.
- `schema.rs`: `DRAFT_TRIGGER` (the v8 DDL now formats it — same text), `MCP_READABLE_TABLES`,
  `MCP_DENIED_TABLES`, `SQLITE_INTERNAL_TABLES`; a test classifies every table of the latest
  schema.
- Reads: one short `DEFERRED` transaction per call; lists paged (`Page`, `Paged`, `MAX_PAGE` 200).
  `studies.rs` gained connection-level readers (`parse_study_row`, `read_study_in`,
  `study_status_in`, `list_studies_in`) that `Journal` now delegates to — one parse, one SQL.
- Submissions: one `BEGIN IMMEDIATE`; order: file identity (`same_file::Handle` taken at open vs
  the path now → `dossier_replaced`) → dossier identity (D10) → blank comment / origin
  (`contract::is_blank`) → per kind: draft study (identifier rule, D2 duplicate incl. archived
  studies, D8 pending draft study, both compared with the app's `same_ticker` rule), note (study
  exists, blank text), cell / judgment (study exists, `DraftField::of_target` + kind match, year in
  study, `parse_value`, D4 one pending per target, `draft_fingerprint` with the caller's method
  version) → `check_payload` (shape rule) → INSERT. The row's `security_ticker` for a note / cell /
  judgment draft is the stored study's, never the AI's spelling. Ids, times and method version are
  the caller's (ADD15).
- Restore (A11): `restore_journal_file` takes an exclusive SQLite lock on the live file (5 s busy
  wait) held across the copy and the rename (Unix; released just before the rename on Windows —
  documented residual window); a lock still held refuses the restore by name, the live file
  untouched. Nothing to lock (no file, not a database, write-protected) → the swap proceeds as
  before.
- Metamorphic suite (AC 12) lives in the app crate (`viewmodel/verify.rs`), where every golden
  fixture converts to a contract `Study` (`study_from_golden`, factored out of `demo_study`): 11
  fixtures × pending drafts of every kind → identical `build_snapshot`.
- Tests: 9 unit (policies, rejected writes on both connections with bytes unchanged, table
  classification, counter bumped only by the trigger, restore races) + 14 integration + 1 app
  metamorphic + 2 contract (`is_blank`). Workspace: 1284 passed, 0 failed, 2 ignored (the corpus
  generators). clippy `-D warnings`, `fmt --check`, `cargo deny check` green.
- Posture: persistence error samples 28 → 31 (+3: `McpSchemaMismatch`, `McpDenied`,
  `McpInvalidCall`). No `@tr` / `MSG_*` change, no `SCHEMA_VERSION` bump, no migration.

### Deviations

- **T1.2 dev-dependencies**: `persistence` did NOT gain `report` / `core` dev-dependencies — the
  golden fixtures convert to a `Study` only in the app crate, so the metamorphic suite (AC 12) runs
  there (the story allowed it: "if no mapping exists, … state it").
- **Two refusal codes beyond §3.3**: `value_out_of_range` (the 8.2b proposal bounds) and
  `empty_note_text` (a blank note draft) — 8.4 renders their French messages.
- **`Error::McpInvalidCall`**: a malformed call (a kind without the data it needs, extraneous
  fields, a `created_at` that is not RFC3339 UTC, a target missing) is a caller (MCP server)
  failure, not an AI refusal.

### G3 review (2026-09-28) — applied

Three layers; two CRITICAL/HIGH reproduced cross-process by two reviewers, all findings applied
(commits `479e074`, `c755936`, `70cc65b`):

- **CRITICAL — POSIX lock loss.** `same_file::Handle::from_path` opened and closed a second
  descriptor of the dossier inside the `IMMEDIATE` transaction; on POSIX that releases every lock
  of the process on the file (SQLite "how to corrupt" §2.2). Now: no descriptor besides SQLite's
  own — a `stat(2)` (dev + ino) taken BEFORE the connection opens, re-checked inside the
  transaction with `SQLITE_FCNTL_HAS_MOVED` (unsafe FFI, one call, SAFETY-commented); Windows keeps
  a `same_file::Handle` opened before the connection and dropped after it (`same-file` is now a
  `cfg(not(unix))` dependency). A cross-process test (the test binary re-spawned as a probe) proves
  another process cannot take the write lock while a submission holds its transaction (DELETE and
  WAL); a control test shows a stray descriptor does release it.
- **HIGH — identity TOCTOU.** Identity captured before the open; the test seam moved right after
  `Connection::open`; WAL race test: never an accepted-and-lost draft.
- **M3 — REPLACE bypass.** Migration **v9** (`trg_ai_drafts_refuse_existing_id`, BEFORE INSERT,
  RAISE(ABORT) on an existing id). The import now UPDATEs an existing draft (same no-regression
  rule) instead of upserting. No `v9.db` (no stored shape changed — corpus README); the v8 gate
  checks the migrated copy carries the guard.
- **M4 — restore vs WAL side files.** Restore leaves WAL under its lock (checkpoint + `journal_mode
  = DELETE`, retried while readers finish, named refusal after the wait), then `BEGIN EXCLUSIVE`;
  `McpAccess` refuses while `-restore-incoming` exists (`dossier_busy`), rechecked after the open; a
  read that saw a swap is refused too.
- **M5 — protected media.** Protected file with no content side file → read in place
  (`immutable`); protected file with unconsolidated writes, or protected directory → refused by
  name (`dossier_protected`, `directory` named) — never `0444` side files.
- **M6 — archived study** → `study_archived` for note / cell / judgment drafts.
- **M7 — new codes** added to the 8.0 spec §3.3 with French messages.
- **Lows** — restore lock skips only NotADatabase / ReadOnly (others named); an unreadable identity
  is its own reason, `dossier_replaced` only on a real mismatch; fields of another kind →
  `invalid_call`; `Error::mcp_code` / `SubmitError::code` (`schema_mismatch`, `write_denied`, …);
  hot journal → `dossier_needs_recovery`; empty / foreign / non-database file → `not_a_dossier`;
  missing → `no_dossier`; ticker AND currency duplicates compared trimmed, case-insensitively;
  `user_version` re-read inside the draft transaction (the read policy allows READING that one
  pragma); length caps (`text_too_long`); a re-submitted identical draft is idempotent, a reused id
  with other content is `draft_id_conflict`; tests F6–F11.

Gates after the review: workspace **1303 passed**, 0 failed, 2 ignored; clippy `-D warnings`,
`fmt --check`, `cargo deny check` green. Posture: persistence error samples 28 → **32** (+4:
`McpSchemaMismatch`, `McpDenied`, `McpInvalidCall`, `McpUnavailable`). **Not checked locally:** the
`cfg(not(unix))` (Windows) code path — no Windows target installed; it is type-simple (a
`same_file::Handle` newtype) and CI builds Linux only.

### Decisions for Guy (owner-pending; defaults applied)

1. Archived studies count as existing for a draft study (`study_exists`). Default: yes.
   **And (G3):** a note / cell / judgment draft on an archived study is REFUSED
   (`study_archived`) — conservative; the owner unarchives it first.
2. Several pending note drafts on one study are allowed (D4 is per cell / judgment target).
   Default: allowed.
3. Windows restore window (A11): the lock is released just before the rename; documented, not
   closed. Default: accept.
4. New codes `value_out_of_range`, `empty_note_text` (French messages in 8.4).
5. ~~A draft on an archived study is accepted at submission~~ — superseded by the G3 decision in
   item 1. A blank proposed company name is stored as absent.
6. Restore skips the lock only when the live file is not a database or is write-protected, and
   refuses by name when a reader or a write holds it past 5 s (it now leaves WAL first).
7. (G3) Migration v9 for the id guard; no `v9.db` corpus file (no stored shape changed).
8. (G3) A `-restore-incoming` file left by an interrupted restore makes the MCP access refuse
   (`dossier_busy`) until it is removed — safer than guessing it is stale.
9. (G3) A protected directory is detected coarsely (no write bit on it); a hot rollback journal is
   named `dossier_needs_recovery` on the read path, while the draft connection lets SQLite recover it
   as any read-write connection would.
10. (G3) Caps: comment and note text 10 000 characters, company name 200, client and model 100.
11. (G3) Idempotency: same id + same kind, study, comment, origin, `created_at` and proposal → the
    id is returned and nothing is written; any difference → `draft_id_conflict`.
12. (G3) New codes beyond §3.3 of 8.0, now added to it: `study_archived`, `value_out_of_range`,
    `empty_note_text`, `text_too_long`, `draft_id_conflict`, `dossier_busy`,
    `dossier_needs_recovery`, `dossier_protected`, `not_a_dossier`,
    `dossier_identity_unreadable`, `invalid_call`.

### File List

- `Cargo.toml` (rusqlite `hooks`; `same-file` workspace dep), `Cargo.lock`
- `contract/src/text.rs` (new), `contract/src/lib.rs`
- `app/src/state/notes.rs` (blank rule from the contract)
- `app/src/viewmodel/verify.rs` (`study_from_golden`; metamorphic suite)
- `persistence/Cargo.toml`, `persistence/src/lib.rs`
- `persistence/src/mcp_access.rs` (new)
- `persistence/src/schema.rs` (`DRAFT_TRIGGER`, table sets)
- `persistence/src/studies.rs` (connection-level readers)
- `persistence/src/drafts.rs` (row mappers `pub(crate)`; `study_status` delegates)
- `persistence/src/journal.rs` (two helpers `pub(crate)`)
- `persistence/src/restore.rs` (exclusive lock across the swap)
- `persistence/src/error.rs` (3 variants, posture inventory 31)
- `persistence/tests/mcp_access.rs` (new)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`
- G3: `persistence/src/{migrations.rs, export.rs}`, `persistence/tests/{readonly_newer.rs,
  corpus_gate.rs, corpus/README.md}`, `_bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md`
  (MCP codes)

### Change Log

- 2026-09-28 — Story 8.3 implemented (dev-story); status → review.
- 2026-09-28 — G3 review applied (v9 id guard, POSIX-lock-safe identity, restore safety, named
  dossier states, archived refusal, caps, idempotency); status stays review.

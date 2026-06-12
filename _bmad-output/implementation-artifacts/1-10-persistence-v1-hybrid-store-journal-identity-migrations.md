# Story 1.10: `persistence` v1 — hybrid store, journal identity & migrations

Status: done

<!-- Epic 1. The durable-journal story: the first real code in `steadyinvest-persistence` (today an
     empty scaffold) and the first real use of rusqlite in the workspace. It gives the proven engine
     (1.7/1.8/1.9) a place to persist: hybrid SQLite schema, journal_id + monotonic logical version,
     a migrations harness, the frozen-corpus schema-drift gate, and read-only-on-newer-file.
     NO calculation, NO UI, NO network, NO export/import (Epic 5), NO sync-guard (Epic 5). -->

## Story

As the developer (Guy, solo),
I want the local SQLite journal with identity and a migrations harness,
so that studies/judgments persist durably and the journal survives version bumps.

## Acceptance Criteria

1. **A journal open/create API exists in `steadyinvest-persistence`** (crate is currently an empty
   scaffold — this story writes its first real code). `Journal::create(path, journal_id: Uuid,
   created_at: &Timestamp) -> Result<Journal>` creates a new journal file; `Journal::open(path) ->
   Result<Journal>` opens an existing one (exact signatures may vary; the API shape — create vs
   open, caller-supplied identity/time — is normative). Uses bundled SQLite (`rusqlite`
   `bundled`, already a pinned workspace dep). **Identity and time are caller-supplied**: the crate
   NEVER calls `Uuid::new_v4()` or any clock itself (ADD15 injected Clock/IdGen discipline —
   the app wires real sources later; tests pass fixed values for full determinism). On create,
   a **`journal_meta` singleton row** stores `journal_id` (UUID as TEXT), a **monotonic
   `logical_version` (INTEGER, starts at 0 or 1 — pick one, document it)** and `created_at`
   (TEXT RFC3339 UTC). On open, the journal's identity is readable
   (`journal.id() -> Uuid`, `journal.logical_version() -> u64` — column is SQLite INTEGER/i64,
   checked conversion). Connection pragmas on every read-write open/create: `journal_mode=WAL`,
   `synchronous=NORMAL`, `busy_timeout` (a few seconds), `foreign_keys=ON` (the newer-file
   read-only path applies only the connection-local ones — see AC 5). (Sync-path detection / `journal_mode=DELETE` switching / single-instance
   lock are **Epic 5** — see scope boundaries.)
2. **Hybrid schema v1 is created by migration 1** (never by ad-hoc DDL at open): **normalized
   tables** `portfolios`, `holdings`, `transactions`, `fx_rates`, `watchlist_items` (DDL only in
   v1 — their `contract` types arrive with Epics 4/6; the architecture names these tables but
   specifies NO column list — the v1 column choice is **dev discretion grounded in the FRs**
   (FR36 holding = security/quantity/purchase price; FR39 transaction = date/quantity/unit
   price/fees/currency; FR28/architecture "fx_rate rows are dated & source-aware"; FR34
   watchlist reorder ⇒ a position column; FR42 trailing stop), kept **minimal** — a v2 migration
   is EXPECTED when the Epic 4/6 contract types land; record this interpretation in the Task 6
   issue) **and blob tables** `studies` + `judgments` for what is replayed whole.
   `studies` columns: `id` TEXT PK (UUID), `journal_id` TEXT, `security_ticker` TEXT,
   `created_at` TEXT (RFC3339 UTC), `status` TEXT (default `'active'`), `schema_version` INTEGER,
   `method_version` TEXT NULL (filled when frozen verdicts land, Epic 2), `payload` TEXT (the serde
   JSON of `contract::Study`). `judgments` (judgment-snapshot time-series, FR51 — written from
   Epic 2; DDL lands now): `id` TEXT PK, `study_id` TEXT FK → `studies(id)`, `created_at` TEXT,
   `schema_version` INTEGER, `payload` TEXT — `journal_id` intentionally omitted (reachable via
   `study_id`; the architecture's indexed-column list applies to the `studies` blob). Binding conventions (architecture Naming Patterns):
   tables snake_case **plural**, PK `id`, FKs `<entity>_id`, indexes `idx_<table>_<cols>`
   (at minimum `idx_studies_security_ticker`, `idx_studies_status`), timestamps TEXT RFC3339 UTC,
   **all monetary/decimal columns TEXT decimal strings — `REAL` is forbidden anywhere in the
   schema** (a test asserts no column of type REAL exists). No decimal arithmetic in SQL, ever.
3. **A `Study` write→read round-trips equal, atomically.** `put_study(&mut self, &Study)` upserts
   the row via **`INSERT … ON CONFLICT(id) DO UPDATE`** — NOT `INSERT OR REPLACE`, whose implicit
   DELETE+INSERT would, once Epic 2 writes `judgments` rows, either FK-fail or cascade-delete the
   FR51 time-series on every re-save. Payload = `serde_json::to_string(study)`; indexed columns
   extracted from the same struct; row `schema_version` = `study.schema_version`; `status` is the
   literal `'active'` and `method_version` is `NULL` in v1 (`Study` has neither field — these
   columns belong to Epic 2 features). `get_study(&self, id) -> Result<Option<Study>>` parses the
   payload back. `get(put(s)) == s` — and because `Money` equality is **value-based**
   (`Money("3.0") == Money("3")`), struct equality alone cannot prove scale preservation: the
   scale test MUST compare the **stored payload string** (or re-serialized JSON) and assert
   `"3.0"` survives byte-for-byte. Also covered: `value: None` cells (unknown round-trips as
   unknown, **never coerced to 0**), full `Provenance`, and `rationale`. Round-trip is exercised
   over several hand-rolled varied studies (no `proptest` in this crate — keeps the AC-8
   `Cargo.lock` delta pinned; `contract` already property-tests the serde shapes).
   **Every mutating call runs in a single rusqlite `Transaction`** that ALSO increments
   `journal_meta.logical_version` — study row + version bump commit together or not at all
   (NFR-R2; the Foundational Invariant's transactional rail). An interrupted write (simulated:
   drop the transaction without commit) leaves the journal at its prior logical_version with no
   partial row. Writing a `Study` whose `study.journal_id` ≠ the open journal's id is an **error**
   (identity integrity — a study from journal A can't be silently written into journal B).
   A `list_studies()` (id + ticker + created_at + status, no payload parse) exists for the Epic 2
   dashboard to build on.
4. **A migrations harness applies versioned steps via `PRAGMA user_version`**: an ordered registry
   `[(1, migrate_to_v1), …]`; on open/create, steps with version > current `user_version` run —
   each step inside its own transaction, setting `user_version` to the step's number in that same
   transaction. Reopen is idempotent (no step re-runs). The harness is the permanent mechanism:
   v2+ later just appends a step (lazy blob upgrade-on-save is the documented policy for
   `schema_version`; with only v1 existing it needs no code yet). `SCHEMA_VERSION` (contract, = 1)
   and `user_version` (= 1) are **distinct axes** — never conflated (versioning.rs doc).
5. **A newer-than-app journal opens read-only with a clear, neutral message** (NFR-R3
   forward-compat): if the file's `user_version` is **greater** than the app's latest known
   migration, `open` does NOT run migrations and does NOT permit writes — it returns a Journal in
   read-only mode. Concretely: the newer-file path re-opens the SQLite handle with
   `OpenFlags::SQLITE_OPEN_READ_ONLY`, applies **only connection-local pragmas** (`busy_timeout`,
   `foreign_keys`) and skips the `journal_mode=WAL` write (a file mutation — the WAL pragma
   belongs to the normal read-write path only), **and** gates writes at the API level (defense in
   depth): write methods fail with a **cause-named, neutral** error (e.g. "this journal was written by a
   newer schema (file user_version N, this build supports M); it is opened read-only" — facts, no
   banned verb, no advice). Reads still work for rows whose `schema_version` ≤ the app's
   `SCHEMA_VERSION`; a row with a **newer `schema_version`** fails its read with a clear typed
   error, never a silent partial parse. Both gates are tested (a fixture DB with bumped
   user_version; a row with bumped schema_version).
6. **A schema-drift detector + a frozen corpus fixture gate the persisted shapes in CI**:
   (a) a **pinned JSON snapshot test** — the canonical `Study` (fixed UUIDs/timestamps, at least
   one fully-populated `YearData` incl. a `None`-valued cell, a `Money` with preserved scale e.g.
   `"3.0"`, a rationale) serializes to a **byte-pinned expected string** committed in the test;
   any change to a persisted struct breaks it, forcing a conscious `SCHEMA_VERSION` bump +
   migration + new corpus file, never a silent drift; (b) a **frozen binary fixture
   `persistence/tests/corpus/v1.db`** — generated once by an `#[ignore]`d generator test from the
   canonical Study, committed, then **append-only forever** (corpus README documents: never edit,
   never regenerate; v2 adds `v2.db` beside it); the gate test opens `v1.db` read-path, asserts
   `user_version == 1`, reads the canonical study and asserts **exact equality** with the in-code
   expected value. ⚠️ **`.gitignore` currently ignores `*.db`** — append the exception
   `!persistence/tests/corpus/*.db` AFTER the `*.db` line (last match wins) and verify the fixture
   is really tracked (plain `git check-ignore` exits non-zero; `git status` shows the file),
   otherwise the corpus gate passes locally and silently never reaches CI.
7. **Per-crate error discipline**: a `persistence::Error` enum via `thiserror` (workspace dep,
   already in the crate's Cargo.toml) with cause-named variants (at minimum: io/sqlite failure,
   corrupt/unparseable payload, newer-schema read-only, journal-identity mismatch, migration
   failure); a crate `Result<T>` alias; **no `.unwrap()`/`.expect()`** outside tests (a documented
   `// INVARIANT:` is the only exception), **no silent `.ok()`**; messages neutral and
   fact-stating (FR13 posture — no banned verb; a posture test over the crate's user-facing
   error strings, following the `core::golden`-local pattern from 1.9).
8. **Gates green, method/engine untouched, interpretations filed**: `cargo fmt --all --check`,
   `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --all
   --locked` (the new persistence tests ride the existing gate — **no CI workflow change**),
   `cargo deny check` all green. `core/` and `contract/` source are NOT modified (no
   `SCHEMA_VERSION` bump, no method change — fingerprint `f79e3c11…1d1d`, determinism hash
   `eb45e761…d34f`, Spike-C digest all pass UNCHANGED). New deps: `uuid = { workspace = true }`
   in persistence `[dependencies]`; `tempfile` (new workspace entry, `[dev-dependencies]` only —
   MIT/Apache-2.0, passes deny). Every spec-underspecified interpretation goes to a **GitHub
   issue** (repo `guycorbaz/steadyinvest`), never an inline debt note.

## Tasks / Subtasks

- [x] **Task 1 — Crate skeleton, error type & journal identity (AC: 1, 7)**
  - [x] Replace the stub `persistence/src/lib.rs` (drop `#![allow(unused_crate_dependencies)]`):
        modules `journal.rs`, `schema.rs` (DDL constants), `studies.rs`, `migrations.rs` (or
        `migrations/mod.rs`), `error.rs`; re-export the public API from `lib.rs`. No `utils.rs`.
  - [x] `error.rs`: `Error` (thiserror) + `Result<T>` alias; `From<rusqlite::Error>` /
        `From<serde_json::Error>` mapped into cause-named variants; neutral message wording +
        a crate-local banned-verb posture test (1.9 pattern).
  - [x] `journal.rs`: `Journal::create(path, journal_id, created_at)` / `Journal::open(path)`;
        pragma setup (WAL, NORMAL, busy_timeout, foreign_keys); `journal_meta` singleton
        (`CHECK (id = 1)`); accessors `id()`, `logical_version()`; no internal clock/UUID calls.
        Representation: the column is SQLite INTEGER (i64); expose it as `u64` to match
        `Provenance.logical_version`'s axis (checked conversion; values are small).
  - [x] Add `uuid = { workspace = true }` to persistence `[dependencies]`; add `tempfile = "3"`
        to `[workspace.dependencies]` and persistence `[dev-dependencies]`.
- [x] **Task 2 — Migrations harness + schema v1 (AC: 2, 4)**
  - [x] `migrations.rs`: ordered `(u32, fn(&rusqlite::Transaction) -> Result<()>)` registry;
        runner reads `PRAGMA user_version`, applies pending steps each in its own transaction,
        sets `user_version` inside that transaction; refuses to run when file version > latest
        known (defers to the AC-5 read-only path). Unit tests: fresh create → v1; reopen → no-op;
        a fake v2 step in a test-local registry proves ordering + idempotence.
  - [x] `schema.rs` migration 1 DDL: `journal_meta`, `studies`, `judgments`, `portfolios`,
        `holdings`, `transactions`, `fx_rates`, `watchlist_items` + indexes, per the AC-2 naming
        and TEXT-money rules.
  - [x] Schema-posture test: introspect `pragma_table_info` over all tables — assert **no REAL
        column** and naming conventions hold.
- [x] **Task 3 — Atomic Study round-trip (AC: 3)**
  - [x] `studies.rs`: `put_study` (upsert via `INSERT … ON CONFLICT(id) DO UPDATE`, never
        `INSERT OR REPLACE` — AC-3 FK rationale; `status` = `'active'`, `method_version` = NULL;
        transaction also does `UPDATE journal_meta SET logical_version = logical_version + 1`;
        explicit `tx.commit()`), `get_study`, `list_studies`; journal-identity check
        (`study.journal_id == self.id()` else `Error::JournalIdentityMismatch`-style variant).
  - [x] Tests (tempfile-backed + in-memory where file semantics don't matter): exact round-trip
        incl. `Money` scale asserted on the **stored payload string** (`"3.0"` stays `"3.0"`
        byte-for-byte — struct equality can't prove it, `Money` Eq is value-based), `None` cell,
        rationale; several hand-rolled varied studies (no proptest dep); a re-save of an existing
        id updates in place; dropped-transaction-without-commit leaves prior logical_version and
        no row; logical_version strictly increments per mutation; wrong-journal_id write rejected.
- [x] **Task 4 — Read-only on newer file (AC: 5)**
  - [x] Open-path gate: `user_version` > latest known → read-only journal (write methods return
        the neutral cause-named error; reads work). Test by creating a journal then bumping
        `user_version` by hand.
  - [x] Row-level gate: payload `schema_version` > `contract::SCHEMA_VERSION` → typed read error
        (test with a hand-inserted future row). Note: serde tolerates unknown *fields* by design
        (forward-compat) — the version gate, not field-set divergence, is the contract.
- [x] **Task 5 — Drift detector + frozen corpus (AC: 6)**
  - [x] Pinned-snapshot test: canonical Study → byte-pinned JSON expected string (self-explaining
        assert message: "persisted shape changed — this requires a SCHEMA_VERSION bump + migration
        + corpus v{N+1}, see corpus README").
  - [x] `#[ignore]`d generator test builds the journal in a `TempDir` (fixed identity/time
        inputs), closes it cleanly, then copies the closed file to
        `persistence/tests/corpus/v1.db` (never write a live DB inside the sync-watched repo
        tree); run once, commit the file.
  - [x] Corpus gate test: copy `v1.db` to a `TempDir` first (avoids `-wal`/`-shm` sidecars in the
        repo tree and keeps the frozen fixture untouched), open it, assert `user_version == 1`,
        read + exact-match the canonical study. `persistence/tests/corpus/README.md`: append-only
        rule, how to add `v{N+1}.db`, never regenerate existing files.
  - [x] `.gitignore`: append `!persistence/tests/corpus/*.db` AFTER the `*.db` line (last match
        wins). Verify: plain `git check-ignore persistence/tests/corpus/v1.db` exits non-zero
        (with `-v` it WILL print the `!…` negation line and exit 0 — that is the success case,
        not a failure), and `git status` shows the file as untracked/added.
- [x] **Task 6 — Gates, issues & status (AC: 8)**
  - [x] `cargo fmt --all --check` · `cargo clippy --all-targets --all-features --locked -- -D
        warnings` · `cargo test --all --locked` · `cargo deny check` — all green; fingerprint,
        determinism hash, Spike-C digest unchanged; `Cargo.lock` delta = uuid edge for
        persistence + tempfile dev-dep.
  - [x] File one consolidated GitHub issue "Story 1.10 persistence interpretations" (candidate
        contents listed in Dev Notes); update `sprint-status.yaml` (1-10 transitions) and this
        story's Dev Agent Record / File List.

## Dev Notes

### What this story is — and the two disasters it must not contain

This is the journal's foundation: every later epic (studies UI, providers, portfolio, export)
writes through it. The two ways to ruin it: (1) **silent schema drift** — a persisted struct
changes, old journals stop parsing, and nothing failed in CI: the pinned snapshot + frozen corpus
exist precisely to make that impossible (and the `.gitignore` `*.db` rule would have silently kept
the corpus out of git — fix it first); (2) **floats in the decision chain** — one `REAL` column or
one SQL `SUM()` over money and the exact-decimal guarantee (NFR-C1) is gone: money is TEXT,
arithmetic happens in Rust with `core`, and a schema test enforces it forever.
[Source: epics.md#Story 1.10; architecture.md#Data Architecture; architecture.md#Naming Patterns]

### Architecture: the hybrid model, verbatim

Normalized tables for what is **aggregated/queried** (`portfolios`, `holdings`, `transactions`,
`fx_rates`, `watchlist_items` — consolidation/concentration/capital-at-risk run over these later,
by pulling rows and computing in Rust with `core`, never in SQL); a **versioned serde JSON blob**
(`payload TEXT` + `schema_version` column) for what is **replayed whole** (`studies`, `judgments`)
— append-mostly, read whole, never queried by inner field, so the judgment model can evolve without
SQL migrations. Indexed columns ride alongside the blob: `journal_id`, `security_ticker`,
`created_at`, `status`, `schema_version`, `method_version`. v1 creates ALL tables (normalized ones
as DDL-only — their contract types arrive with Epics 4/6) so the schema is whole from birth and
later epics fill rather than migrate. [Source: architecture.md#Data Architecture; architecture.md#Complete Project Directory Structure]

### The three version axes — keep them straight

1. `contract::SCHEMA_VERSION` (= 1, u32) — the serde shapes in the blob payloads.
2. SQLite `PRAGMA user_version` (= 1 after migration 1) — the SQL schema. THE migration trigger.
3. `core::METHOD_VERSION` ("ssg-1.0.0") — calculation semantics. **Not persistence's business**
   beyond the nullable `method_version` column reserved for frozen verdicts (Epic 2).

Never conflate them: a blob-shape change bumps axis 1 (+ lazy upgrade-on-save policy), a table
change bumps axis 2 (+ a migration step), and this story bumps NEITHER — it establishes both at 1.
[Source: contract/src/versioning.rs; architecture.md#Core Architectural Decisions]

### Identity & time are inputs, not side effects

ADD15 mandates injected `Clock`/`IdGen` — those traits don't exist yet anywhere (deferred to the
app layer, Epic 2). Do NOT invent them here: take `journal_id: Uuid` and `created_at: &Timestamp`
as **explicit parameters** on `create` (and nothing in the crate calls `Uuid::new_v4()` or a
clock). That keeps every test deterministic (fixed UUIDs/timestamps → reproducible corpus and
snapshots) and lets the app wire real sources later without touching persistence. Record this
interpretation in the Task 6 issue. [Source: architecture.md#Process Patterns (Clock/IdGen); explore report 2026-06-12]

### The monotonic logical version is the journal's heartbeat

`journal_meta.logical_version` increments **inside the same transaction** as every mutation —
that's what makes ADD6's later features honest: the Epic 5 "last-used pointer = (journal_id,
last-seen-version)" and the "you saw v57, this is v41" stale-restore detection both read this
counter. If a mutation can commit without bumping it (or vice versa), those features silently lie.
One transaction, both writes, explicit `tx.commit()` (rusqlite `Transaction` rolls back on drop —
that's the AC-3 interrupted-write test). `Provenance.logical_version` on cells is the SAME axis,
stamped by producers — persistence doesn't rewrite cell provenance. [Source: architecture.md#Cross-Cutting Concerns (journal identity); epics.md#ADD6]

### Contract types: consume as-is, fix nothing here

`contract` v1 is done and review-approved: `Study { id, journal_id, security_ticker,
native_currency, years, judgment, rationale, created_at, schema_version }`, `Cell` (value:
`Option<Money>` × source × freshness × review × coverage + `Provenance`), `Money` (serializes as a
scale-preserving canonical string; `from_str_exact` on parse — exactness survives the blob for
free), `Timestamp` (plain RFC3339 string). Journal types tolerate unknown fields and use
`#[serde(default)]` — that's the forward-compat rail; do NOT add `deny_unknown_fields` anywhere in
persistence parsing. **Known gap — issue #14**: `Judgment` cannot carry `recent_severe_low`,
`present_full_year_dividend` or the growth-% judgments; that is an Epic 2 contract change (with
its own `SCHEMA_VERSION` bump + migration + corpus v2). 1.10 persists the contract AS IT IS —
touching `contract/` here would cascade a version bump this story must not own.
[Source: contract/src/{study,cell,money,provenance,versioning}.rs; GitHub issue #14]

### rusqlite 0.40 specifics (first real use in the workspace)

Pinned `rusqlite = { version = "0.40", features = ["bundled"] }` since 1.1 (compiled-in SQLite,
no system dep — nothing to add in CI). The 0.40 breaking changes are confined to the VTab API
(not used here); the core surface is the stable one: `Connection::open` /
`OpenFlags::SQLITE_OPEN_READ_ONLY`, `conn.pragma_update(None, "journal_mode", "WAL")?`,
`conn.query_row("PRAGMA user_version", …)` (or `pragma_query_value`), `conn.transaction()?` →
`tx.commit()?`, `params![]`. Single connection per `Journal` is enough for this headless story
(the mutex-guarded write connection + WAL concurrent readers is app-era machinery). Note:
`journal_mode=WAL` is persistent in the DB file; `PRAGMA user_version` default is 0 on a fresh
file — which is exactly what lets `create` and the migration runner share one code path.
[Source: Cargo.toml workspace deps; https://github.com/rusqlite/rusqlite/releases]

### Testing the file-level behaviours

Use `rusqlite`'s in-memory mode only where file semantics don't matter; the round-trip, reopen,
corpus and read-only tests need real files → `tempfile::TempDir` (new dev-dep). **Do not create
test DBs inside the repo tree** (it's under Synology Drive sync — exactly the environment the
architecture warns about for live SQLite files; `tempfile` lands in `/tmp`, outside the sync
watch). The corpus `v1.db` is the one committed DB file — it's frozen/read-only in tests, never
written by the suite (the gate test copies it out before opening), so sync-watch is harmless
there. [Source: architecture.md#Technical Constraints (Synology); _bmad-output/implementation-artifacts/deferred-work.md]

### Scope boundaries — what 1.10 does NOT do

- **No export/import/backup/restore** (FR59-61 → Epic 5 stories 5-2/5-3/5-4) — `export_import.rs`
  / `backup.rs` do not exist yet.
- **No sync_guard**: sync-path detection, `journal_mode=DELETE` switching, single-instance file
  lock → Epic 5 story 5-5 (ADD7/ADD8). 1.10 hardcodes the local-use pragmas. Confirm the Task 6
  issue notes the single-instance lock is consciously deferred (ADD6 lists it under identity, but
  it only matters once an app opens journals — Epic 2+).
- **No consolidation** (`consolidation.rs` — Epic 4/6), **no price-history cache** (ADD13 —
  Epic 3/5), **no normalized-table accessors** (typed CRUD for portfolios etc. arrives with
  Epics 4/6; v1 ships DDL only).
- **No `contract` change** (issue #14 stays open), **no `core` change** (fingerprint pinned),
  **no CI workflow edits** (`cargo test --all --locked` already gates), **no async/tokio**
  (rusqlite is sync; threading is the app's concern).
- **No verdict persistence** — the frozen decision-time verdict is an Epic 2 feature; the
  `method_version` column merely reserves its seat.
- **No locale/display formatting** — payloads carry canonical `Money` strings; formatting is a
  viewmodel concern (Epic 2).

### Previous story intelligence (1-9 dev record + review)

- Gates always `--locked`; clippy `-D warnings` covers `--all-targets` — integration tests and
  helpers are linted (1.7 hit `redundant_closure`, 1.8 `double_ended_iterator_last` exactly there).
- "Done" = demonstrably works: assert messages must self-explain (the corpus/snapshot failure
  messages should TELL the developer the required ritual — bump, migrate, new corpus).
- Posture tests are crate-local (1.9 built `core::golden`'s own banned-verb gate rather than
  extending the `ssg` inventory); persistence error strings get the same local treatment.
- Issues, not inline notes: 1.7 → #12, 1.8 → #13/#14, 1.9 → #15 — 1.10 files its own.
- MSRV 1.96 (`rust-toolchain.toml`; the architecture's "1.88" is stale). CI is Linux-only
  (decision 2026-06-09) — the determinism-hash test stays the cross-OS contract.
- 1.9's serde lesson transfers inverted: fixtures/oracles are strict, **journal data is tolerant**
  — persistence reads user data, so it follows the tolerant rail (no `deny_unknown_fields`).

### Git intelligence

Recent commits (4c8f5fc, 4ff42e0, a14a245) show the established rhythm: one story = one
`feat(story-1.N): …` commit touching its crate + tests + sprint-status + story file; convention
`d("…")` Decimal helpers and builder fns in tests; self-explaining assert style throughout. The
persistence crate has had zero commits beyond the 1.1 scaffold — green field inside fixed walls.

### Project Structure Notes

- **New:** `persistence/src/{journal.rs, schema.rs, studies.rs, migrations.rs, error.rs}`;
  `persistence/tests/` (e.g. `journal_roundtrip.rs`, `migrations_gate.rs`, `corpus_gate.rs` —
  granularity at dev discretion); `persistence/tests/corpus/v1.db` (frozen, committed) +
  `persistence/tests/corpus/README.md`.
- **Modified:** `persistence/src/lib.rs` (stub → real module tree), `persistence/Cargo.toml`
  (+ `uuid`, `[dev-dependencies]` `tempfile`), root `Cargo.toml` (+ `tempfile` workspace entry),
  `Cargo.lock`, `.gitignore` (corpus exception), `_bmad-output/implementation-artifacts/
  sprint-status.yaml` (1-10 transitions).
- **Do NOT modify:** `core/**` (fingerprint `f79e3c11…1d1d`, determinism hash `eb45e761…d34f`,
  Spike-C digest), `contract/**` (issue #14 is Epic 2's), `.github/workflows/ci.yml`,
  `docs/method/**`, `deny.toml` (tempfile is MIT/Apache-2.0 — already allowed).
- **Naming:** snake_case modules, no `utils.rs`; SQL per AC-2 conventions; types `PascalCase`
  (`Journal`, `Error`, …); tests follow the existing self-explaining-assert house style.

### References

- [Source: epics.md#Story 1.10] — user story + ACs (hybrid tables, blob, journal_id + monotonic
  logical version, atomic round-trip, user_version + schema_version, migrations harness,
  schema-drift detector + frozen `tests/corpus/v1.db`, read-only-on-newer)
- [Source: epics.md#Epic 1 "Includes:"] — "`persistence` hybrid schema + journal_id + migrations
  harness (ADD5,6)"; Epic 1 closes headless
- [Source: epics.md#ADD4/ADD5/ADD6/ADD15] — version axes & forward-safe migrations; hybrid model
  & TEXT money; journal identity (UUID + monotonic version, lock, backup stamps); injected
  Clock/IdGen, thiserror, no silent `.ok()`
- [Source: prd.md#FR2/FR51/FR66 + NFR-R2/R3/R5, NFR-X3] — durable reopen; durable time-series;
  portable local store; crash-safe atomic writes; forward-safe migrations (older journal always
  opens, newer file read-only); portable journal file
- [Source: architecture.md#Data Architecture] — store/pragmas, hybrid model, identity & integrity,
  migrations policy (lazy upgrade on save, read-only on newer, frozen corpus + drift detector)
- [Source: architecture.md#Naming Patterns] — SQL conventions (plural snake_case, `id`/`<entity>_id`,
  `idx_<table>_<cols>`, RFC3339 TEXT, money TEXT never REAL)
- [Source: architecture.md#Pattern Examples] — the `studies` row "Good" example (column set
  including `status`/`method_version`); REAL-for-sort-only carve-out (NOT used in v1)
- [Source: architecture.md#Architectural Boundaries] — only `persistence` touches SQLite; decimal
  arithmetic for consolidation in Rust, never SQL on TEXT money
- [Source: architecture.md#Process Patterns] — Clock/IdGen injection, error discipline, no
  unwrap/expect/silent-ok
- [Source: contract/src/study.rs + cell.rs + money.rs + provenance.rs + versioning.rs] — the exact
  shapes persisted (verified in code 2026-06-12); SCHEMA_VERSION = 1; Money scale-preserving
  serialization; forward-compat serde policy
- [Source: persistence/src/lib.rs] — current stub ("the store lands in Story 1.10")
- [Source: .github/workflows/ci.yml] — `cargo test --all --locked` already gates; Linux-only matrix
- [Source: 1-9-golden-reference-studies-self-check-gate.md] — predecessor patterns: crate-local
  posture test, `--locked` gates, issues #12–#15, self-explaining asserts
- [Source: GitHub issue #14] — Judgment persistence gap, explicitly out of scope here

### Tech currency note (web check 2026-06-12)

`rusqlite` 0.40 verified against its release notes: the 0.40 breaking changes are VTab-module
refactors (constructors replacing macros) — irrelevant to this story's surface
(`Connection`/`Transaction`/pragmas/`OpenFlags` unchanged). `bundled` compiles SQLite into the
binary (no CI provisioning). `uuid` 1 and `serde_json` 1 are already exercised workspace deps
(contract, since 1.3). The only new crate is `tempfile` 3 (dev-only, MIT/Apache-2.0, deny-clean).
Sources: [rusqlite releases](https://github.com/rusqlite/rusqlite/releases).

## Dev Agent Record

### Agent Model Used

Claude Fable 5 (claude-fable-5) via Claude Code

### Debug Log References

- Baseline `cargo test --all --locked` green before any change (2026-06-12).
- `cargo generate-lockfile` initially re-resolved unrelated packages (regex/zerocopy bumps) —
  restored `Cargo.lock` from git and let `cargo metadata` do a minimal resolution instead: final
  lock delta is exactly the AC-8 pin (uuid + tempfile edges on `steadyinvest-persistence`;
  tempfile 3.27.0 was already in the graph transitively).
- `Journal` needed `#[derive(Debug)]` for `Result::expect_err` in tests — only friction point.
- rusqlite 0.40 `pragma_update` goes through `execute_batch` (verified in vendored source), so
  `journal_mode=WAL`'s returned row is tolerated.

### Completion Notes List

- **Task 1** — stub `lib.rs` replaced by the real module tree (`error`, `journal`, `migrations`,
  `schema`, `studies`; no `utils.rs`; `#![allow(unused_crate_dependencies)]` dropped).
  `Error` (thiserror) with 8 cause-named variants + `Result<T>` alias; `From<rusqlite::Error>` /
  `From<serde_json::Error>`; crate-local banned-verb posture test (1.9 pattern, exhaustive-match
  inventory so a new variant cannot dodge the gate). `Journal::create/open` with caller-supplied
  `journal_id`/`created_at` (no clock/UUID call anywhere in the crate); WAL/NORMAL/busy_timeout/
  foreign_keys pragmas; `journal_meta` singleton (`CHECK (id = 1)`); `id()`, `logical_version()`
  (i64 column exposed as u64 via checked conversion). `uuid` dep + `tempfile` workspace dev-dep.
- **Task 2** — ordered `(u32, fn(&Transaction) -> Result<()>)` registry; runner applies pending
  steps each in its own transaction stamping `user_version` inside it; refuses newer files
  (defers to the AC-5 read-only path). Unit tests: fresh→v1, reopen no-op, test-local fake v2
  proves ordering/idempotence, failing step leaves `user_version` at the previous step. Schema v1
  DDL: all 8 tables + idx_* indexes; schema-posture test asserts no REAL column anywhere, PK=id,
  plural snake_case, idx_<table>_<cols>.
- **Task 3** — `put_study` upserts via `INSERT … ON CONFLICT(id) DO UPDATE` (status/method_version
  untouched on update), bumps `journal_meta.logical_version` in the SAME transaction, explicit
  commit; journal-identity check rejects foreign studies. `get_study` (row schema_version gate →
  typed error before any parse), `list_studies` (indexed columns only). Tests: exact round-trip
  over 3 varied hand-rolled studies, Money scale `"3.0"`/`"1322.500000"`/`"-0.07"` asserted
  byte-for-byte on the STORED payload string, None cell stays None, re-save in place,
  interrupted write (dropped tx) leaves prior logical_version + no partial row, strict
  per-mutation increments, wrong-journal_id rejected with nothing written.
- **Task 4** — newer `user_version` → re-open `SQLITE_OPEN_READ_ONLY`, connection-local pragmas
  only (no WAL write), no migration, reads work, writes fail with the neutral cause-named error
  (fixture: hand-bumped user_version=9). Row-level gate: hand-inserted future row
  (schema_version=2) fails its read typed; unknown *fields* at a known version stay tolerated
  (forward-compat rail); corrupt payload → `CorruptPayload`.
- **Task 5** — pinned byte-exact JSON snapshot of the canonical Study (fixed UUIDs/timestamps,
  full YearData incl. None-valued dividend cell, scale-bearing `"3.0"`, rationale) with a
  self-explaining failure message (bump + migration + corpus v{N+1} ritual); `#[ignore]`d
  generator built `tests/corpus/v1.db` in a TempDir from a cleanly-closed file (run once,
  refuses to overwrite — append-only); gate test copies v1.db out, asserts `user_version == 1`
  and exact equality with the in-code canonical study; corpus README documents the append-only
  rules. `.gitignore` exception added AFTER `*.db`; verified: plain `git check-ignore` exits 1,
  `-v` prints the negation line, `git status` shows the file.
- **Task 6** — all gates green: `cargo fmt --all --check`, `cargo clippy --all-targets
  --all-features --locked -- -D warnings`, `cargo test --all --locked` (42 persistence tests
  after QA-automation and review additions; 1 intentionally ignored generator), `cargo deny
  check`. Method fingerprint, determinism hash and
  Spike-C digest pass UNCHANGED; `core/`/`contract/`/CI workflow not modified. Interpretations
  filed as GitHub issue #16 (identity-as-parameters, logical_version starts at 0,
  normalized-table column choices, no-kind/no-currency minimalism, upsert preserves
  status/method_version, crate-local banned-verb copy, single-instance lock deferred).

### File List

- `persistence/src/lib.rs` (modified — stub → real module tree)
- `persistence/src/error.rs` (new)
- `persistence/src/journal.rs` (new)
- `persistence/src/migrations.rs` (new)
- `persistence/src/schema.rs` (new)
- `persistence/src/studies.rs` (new)
- `persistence/tests/journal_roundtrip.rs` (new)
- `persistence/tests/readonly_newer.rs` (new)
- `persistence/tests/corpus_gate.rs` (new)
- `persistence/tests/e2e_lifecycle.rs` (new — multi-session lifecycle + environment failure modes; added by the QA-automation step, documented during review)
- `persistence/tests/corpus/v1.db` (new — frozen, append-only)
- `persistence/tests/corpus/README.md` (new)
- `persistence/Cargo.toml` (modified — + uuid, + [dev-dependencies] tempfile)
- `Cargo.toml` (modified — + tempfile workspace entry)
- `Cargo.lock` (modified — uuid + tempfile edges on steadyinvest-persistence only)
- `.gitignore` (modified — `!persistence/tests/corpus/*.db` exception after `*.db`)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified — 1-10 transitions)
- `_bmad-output/implementation-artifacts/1-10-persistence-v1-hybrid-store-journal-identity-migrations.md` (modified — this file)

## Senior Developer Review (AI)

**Reviewer:** Guy (autonomous story-automator review) — 2026-06-12

**Outcome: Approve** (after auto-fix). 0 Critical, 0 High, 2 Medium, 2 Low — all fixed in place.

All 8 ACs verified IMPLEMENTED against the code (not the story's claims): journal create/open with
caller-supplied identity (AC1), hybrid schema v1 + no-REAL/naming posture tests (AC2), atomic
upsert round-trip with byte-level Money-scale proof on the stored payload (AC3), `user_version`
harness with ordering/idempotence/failure-rollback tests (AC4), both read-only gates tested with
fixtures (AC5), pinned snapshot + frozen corpus with the `.gitignore` exception verified —
`git check-ignore` exits 1 (AC6), thiserror discipline with zero `unwrap/expect/ok()` outside
test modules + exhaustive-match posture inventory (AC7), all 4 gates re-run green during review,
`Cargo.lock` delta = uuid + tempfile edges only, issue #16 confirmed open (AC8).

Findings and fixes:

1. **[MEDIUM — fixed]** `Journal::open` mutated a non-journal file before discovering it was not
   a journal: on a foreign SQLite db (or a half-created file) it applied `journal_mode=WAL`, ran
   migration 1 (writing all 8 tables into the foreign file) and stamped `user_version=1`, only
   then failing on the missing `journal_meta`. With the user-selectable DB location, a wrong pick
   must stay byte-intact. Fix: `open` reads the journal identity BEFORE any file-mutating pragma
   or migration (`persistence/src/journal.rs`); `map_missing_meta` now maps
   "no such table: journal_meta" to `CorruptJournalMeta`; the e2e foreign-db test now asserts the
   file is untouched (no tables, `user_version` 0, journal mode not WAL).
2. **[MEDIUM — fixed]** `persistence/tests/e2e_lifecycle.rs` (7 tests, added by the QA-automation
   step) was missing from the File List and excluded from the "33 tests" count. Fix: File List and
   counts updated (now 42 persistence tests: 41 run + 1 ignored generator).
3. **[LOW — fixed]** A failed `Journal::create` left a half-written file behind, trapping any
   retry on `JournalExists`. Fix: best-effort removal of the file (+ `-wal`/`-shm` sidecars) on
   the create error path; new test `failed_create_leaves_no_file_behind` covers the no-leftover
   and retry-succeeds contract.
4. **[LOW — fixed]** Stale test count in the Dev Agent Record (folded into finding 2).

Post-fix verification: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features
--locked -- -D warnings`, `cargo test --all --locked` (24 suites, 0 failures; corpus gate and
pinned snapshot pass — frozen `v1.db` still opens and reads back exactly), `cargo deny check` —
all green. `core/`/`contract/`/CI untouched.

## Change Log

| Date | Change |
|------|--------|
| 2026-06-12 | Senior Developer Review (AI) — Approve, status → done. 4 findings (2 Medium, 2 Low), all auto-fixed: `Journal::open` no longer mutates a non-journal file (identity read before WAL pragma/migrations, foreign-db test asserts byte-intactness), failed `Journal::create` cleans up its half-written file (+ retry test), `e2e_lifecycle.rs` documented in the File List, test counts corrected (42 persistence tests). All 4 gates re-verified green post-fix; corpus + snapshot gates unchanged. |
| 2026-06-12 | Story 1.10 implemented (review): `persistence` v1 written from the empty scaffold — `Journal::create/open` (caller-supplied UUID + RFC3339 timestamp, no internal clock/IdGen), `journal_meta` identity singleton with monotonic `logical_version` (starts at 0), hybrid schema v1 via the `PRAGMA user_version` migrations harness (8 tables: studies/judgments blobs + 5 normalized DDL-only + journal_meta; no REAL column, enforced by test), atomic `put_study` upsert (`ON CONFLICT DO UPDATE`) bumping logical_version in the same transaction, `get_study`/`list_studies`, read-only-on-newer at both gates (file user_version → `SQLITE_OPEN_READ_ONLY` re-open + API write gate; row schema_version → typed read error), pinned byte-exact JSON snapshot + frozen corpus `tests/corpus/v1.db` (generated once, append-only) with the `.gitignore` exception verified, thiserror cause-named neutral errors + crate-local banned-verb posture test. 33 crate tests; all 4 workspace gates green; core/contract/CI untouched (fingerprint, determinism hash, Spike-C digest unchanged); Cargo.lock delta = uuid + tempfile edges only. Interpretations filed as issue #16. |
| 2026-06-12 | Story 1.10 created (ready-for-dev): first real `persistence` code — journal open/create with caller-supplied identity/time (journal_meta: journal_id UUID + monotonic logical_version), hybrid schema v1 via a `PRAGMA user_version` migrations harness (normalized DDL-only tables + studies/judgments JSON blobs, TEXT money enforced by test), atomic Study round-trip bumping logical_version in the same transaction, read-only-on-newer at both the user_version and row schema_version gates, pinned-snapshot drift detector + frozen `tests/corpus/v1.db` (with the `.gitignore` `*.db` exception trap called out), thiserror neutral-error discipline + crate-local posture test. No contract/core/CI changes; tempfile dev-dep only. Validated by a fresh-context adversarial checklist pass — fixes folded in: upsert (`ON CONFLICT DO UPDATE`) instead of `INSERT OR REPLACE` (FR51 FK landmine), `git check-ignore -v` semantics corrected, Money-scale assertion moved to the stored payload string (struct Eq is value-based), read-only pragma carve-out, normalized-table columns declared dev-discretion-with-issue (architecture has no column spec), corpus generated/opened via TempDir copies. Ultimate context engine analysis completed — comprehensive developer guide created. |

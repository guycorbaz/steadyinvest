# Story 8.4: `steadyinvest-mcp` stdio server

Status: review

<!-- Created 2026-09-28 by create-story (Epic 8 autonomous run). Sources: epics.md Story 8.4 + Epic 8
preamble; architecture.md §Phase 4 A1 / A2 / A3 / A10 / A11 / A12; ux-ai-assistance-surfaces.md §3.3
(MCP codes, Q11); Story 8.3 record (items deferred to 8.4); the real code on main at d0d40ce. -->

## Story

As Guy,
I want a local MCP server my AI client launches over stdio,
so that the AI can read my studies and drop drafts into my inbox — and nothing else.

## Acceptance Criteria

1. **New binary crate, exact dependency boundary (A1).** A workspace member `mcp/` builds the binary
   **`steadyinvest-mcp`** (package name `steadyinvest-mcp`). Its normal dependencies are limited to:
   `steadyinvest-contract`, `steadyinvest-persistence` (used **only** through `McpAccess` and its
   types), `steadyinvest-core`, `steadyinvest-report`, the new `steadyinvest-paths` (AC 3), `rmcp`
   (AC 2), `tokio`, `serde`, `serde_json`, `uuid`, `chrono`, `tracing`, `tracing-subscriber`,
   `tracing-appender`, `rust_decimal`. A CI test (`mcp/tests/closure.rs`) runs
   `cargo metadata --format-version 1 --locked`, walks the **resolved normal-dependency** graph from
   `steadyinvest-mcp`, and fails if the closure contains any of: `steadyinvest-ingestion`,
   `steadyinvest-app`, `reqwest`, `hyper`, `hyper-util`, `h2`, `ureq`, `isahc`, `curl`, `surf`,
   `attohttpc`, `keyring`, `secret-service`, `slint`, `rfd`, `axum`, `tower-http` (FR15, FR21, FR76,
   NFR-A3, NFR-S1).
2. **MCP SDK = `rmcp` 3.4 (A12 SDK choice).** `rmcp = { version = "3.4", default-features = false,
   features = ["server", "transport-io", "macros", "schemars"] }` in `[workspace.dependencies]`; no
   other rmcp feature. `cargo deny check` stays green (verified in a scratch crate 2026-09-28: the
   closure adds only MIT / Apache-2.0 crates — no HTTP client, no hyper, no reqwest). The server
   speaks the protocol version rmcp negotiates (latest known to 3.4.1: `2025-11-25`, older ones
   accepted).
3. **Shared locations crate (A10).** A new workspace member `paths/` (package
   `steadyinvest-paths`, deps: `directories`, `serde`, `serde_json` only) owns: the project
   directories tuple (`ProjectDirs::from("", "", "steadyinvest")`), `config_file_path()`,
   `default_journal_path()`, `log_dir()`, and a **read-only, tolerant** reader of the dossier
   pointers of `config.json` (`DossierPointers { last_opened_path, journal_path }`, unknown fields
   ignored, missing fields `None`). The app's `config::default_path`, `state::default_journal_path`
   and `logging::init`'s log dir delegate to it (one definition, no drift). The reader **never**
   writes, renames or creates anything (the app's `config::load` moves an invalid file aside — the
   MCP must never do that).
4. **`last_opened_path` (A10, "this story adds it").** `AppConfig` gains
   `#[serde(default)] last_opened_path: Option<PathBuf>`, set by `AppConfig::record_recent` (every
   successful open: startup, switch, create, restore reopen — all go through `record_recent`). The
   G3 M4 rule is untouched: when the startup stand-in runs because the configured dossier was refused
   by name, `journal_path` stays on the refused dossier and `last_opened_path` names the stand-in (the
   one the owner sees). App test: that exact scenario leaves `journal_path = refused`,
   `last_opened_path = default`.
5. **Per-call dossier resolution (O3, D10, A10).** Every tool call resolves the dossier afresh:
   `--dossier <path>` when given; else `config.json`'s `last_opened_path`; else its `journal_path`;
   else `default_journal_path()`. An unreadable/invalid `config.json` is refused by name
   (`config_unreadable`, AC 11) — never a guess. A test (in `paths` or `mcp`) shows that with
   `journal_path = refused`, `last_opened_path = default`, MCP serves the default dossier.
6. **Every response names the dossier (O3).** Every successful tool result carries
   `"dossier": { "journal_id": "<uuid>", "path": "<resolved path>" }` (the `DossierIdentity` of the
   call). Every submit tool **requires** `dossier_journal_id` and `dossier_path` back; they build the
   `DraftSubmission.dossier` that `McpAccess` checks (D10 → `dossier_mismatch`).
7. **Exactly eight tools, all through `McpAccess` (FR69–FR72, FR77, A2).** `tools/list` returns
   exactly (names fixed):
   - `list_studies` — `offset?`, `limit?` (default 50, clamped by `MAX_PAGE` = 200) → paged
     `StudySummary` list + `total`;
   - `get_study` — `study_id` → the study (cells with provenance, judgments, rationale, notes,
     status) **and its computed outputs** (AC 8);
   - `get_judgment_history` — `study_id`, `offset?`, `limit?` (default 20) → paged FR51 snapshots,
     newest first;
   - `get_notes` — `study_id` → the study's notes (from `read_study`);
   - `get_drafts_record` — `study_id?`, `status?`, `offset?`, `limit?` → paged `DraftRecord`s with
     outcomes (FR77);
   - `submit_draft_study` — `security_ticker`, `native_currency` (required, D2), `company_name?`,
     `comment`, `origin_client`, `origin_model`, `dossier_journal_id`, `dossier_path`;
   - `submit_draft_note` — `study_id`, `note_text`, `comment`, origin, dossier;
   - `submit_draft_value` — `study_id`, `field`, `fiscal_year?` (required for cell fields, absent for
     judgment fields), `proposed_value`, `comment`, origin, dossier.
   No other tool, resource or prompt is offered (`capabilities` advertises tools only). Every read
   runs through `McpAccess::{identity, list_studies, read_study, read_history, list_drafts}`; every
   submit through `McpAccess::submit_draft`. The server assigns the draft `id` (`Uuid::new_v4`) and
   `created_at` (UTC RFC 3339, `chrono`), and passes `steadyinvest_core::METHOD_VERSION` as
   `method_version` (ADD15: the identity is the caller's — here the server is the caller).
8. **Computed outputs (O1, 8.3 deferral).** `get_study` adds `"computed"`: built with
   `report::form::build_snapshot(&study)` — the app's single construction — and mapped field by field
   into serde DTOs owned by `mcp` (core types do not implement `Serialize`; do **not** add serde to
   `core`). Content: verdict state (`full` / `provisional` / `withheld`), `low_confidence`, the open
   gates (input + state), `method_version`, `inputs_hash`; the verdict facts with the zone as a
   **neutral code** (`low` / `middle` / `high` — the app's neutral label set, never buy/hold/sell,
   FR13) and each criterion as `met` / `unmet` / `unknown`; forecast high/low, zone bounds,
   upside/downside ratio, relative value %, projected appreciation %, projected annualized
   appreciation %, projected total annualized return % (5-year potential), sales/EPS CAGR %, quality
   flag codes. Every number is the **raw decimal as a string** (`Decimal::normalize().to_string()`),
   `null` when unknown — never a presentation format, never a re-derived value (Cardinal Rule). A
   `NormalizeError` makes `"computed": null` plus `"computed_unavailable": "<reason>"` — the call
   still succeeds (the study data is still served).
9. **Schemas carry the draftable fields (D6, 8.2b registry).** `submit_draft_value`'s input schema
   enumerates `field` from `DraftField::ALL` keys, and its description lists, per field: kind
   (cell / judgment), unit (`DraftUnit`: percent written as percent, e.g. `12` for 12 %; money in
   absolute amounts — sales and pre-tax profit **not** in millions, 8.2b decision), and for
   `forecast_low_option` its option names (`option_name`). `current_price` / `ttm_eps` never appear
   (D6). Required/optional per the story fields; `comment`, `origin_client`, `origin_model`,
   `dossier_*` are **required** in every submit schema (NFR-A4).
10. **Refusals worded as fixed in 8.0 (Q11).** A refused/failed call returns a tool result with
    `isError: true` and JSON content `{ "code": "<snake_case>", "message": "<French>", "dossier":
    {…} | null }`. The code is `SubmissionRefusal::code()` / `McpUnavailable::code()` /
    `Error::mcp_code()`; the French message is the §3.3 text **verbatim** with its placeholders
    filled from the typed data (ticker, currency, year, field key, unit, options, texts, caps, ids,
    paths). Every §3.3 code has a renderer (`dossier_mismatch`, `dossier_replaced`,
    `schema_mismatch` ×2 directions, `no_dossier`, `study_not_found`, `empty_comment`,
    `missing_origin`, `field_not_draftable`, `year_not_in_study`, `value_unparsable`,
    `value_not_an_option`, `identifier_invalid`, `target_has_pending`, `study_exists`,
    `draft_study_pending`, `write_denied`, `study_archived`, `value_out_of_range`, `empty_note_text`,
    `text_too_long`, `draft_id_conflict`, `dossier_busy`, `restore_interrupted`, `dossier_locked`,
    `dossier_needs_recovery`, `dossier_protected` ×2, `not_a_dossier`, `dossier_identity_unreadable`,
    `invalid_call`) plus the two added here (AC 11). A test asserts the renderer covers every code
    (exhaustive `match`, no wildcard) and that no message contains a banned verb (FR13 — both
    `BANNED_VERBS_FR` and `BANNED_VERBS_EN` from `core::method`).
11. **Two new codes (decisions, owner-pending).** `config_unreadable` — « La configuration de
    l'application ({chemin}) n'a pas pu être lue ; aucun dossier n'a été déterminé, rien n'a été
    lu. »; `dossier_error` (a failure with no named MCP cause — e.g. an SQLite I/O error or an
    unparseable study row) — « Le dossier n'a pas pu être lu ou écrit ({cause}) ; rien n'a été
    enregistré. ». Both added to the 8.0 spec §3.3 list with a dated note.
12. **Own log, never stdout (A12, FR14, NFR-S1, ADD15).** The binary logs to a daily-rotating file
    `steadyinvest-mcp.log` in `steadyinvest_paths::log_dir()` (same directory as the app's log, its
    own file prefix — never the app's file). Every refused or failed call is logged at `warn` with its
    tool, code, the typed data and, for `write_denied`, each `McpDenial` (action + object). No key, no
    config value other than the resolved dossier path, is ever logged. **Stdout carries only
    JSON-RPC frames**: no `println!`, the subscriber writes to the file only, the panic hook writes to
    the log and stderr. No log dir → the server runs without a file (one stderr line), never fails.
13. **Clippy boundary (A3, 8.3 deferral).** `mcp/clippy.toml` sets `disallowed-types` =
    `steadyinvest_persistence::Journal`, and `disallowed-methods` =
    `steadyinvest_persistence::restore_journal_file`,
    `steadyinvest_persistence::restore_journal_file_keeping`,
    `steadyinvest_persistence::inspect_backup`, `steadyinvest_persistence::clear_lock`,
    `steadyinvest_persistence::sweep_stale_read_copies`. Stdout printing is forbidden by a
    `#![deny(clippy::print_stdout)]` crate attribute (a lint, not a config entry — AC 12's
    stdout-only-JSON-RPC rule). The crate `clippy.toml` **replaces** the workspace one (clippy uses the nearest file) — the
    workspace `clippy.toml` has no active setting today, so nothing is lost; the file says so. Test
    helpers that must build fixtures with `Journal` carry a scoped
    `#[expect(clippy::disallowed_types, reason = "test fixture builder, never in the binary")]`.
14. **Non-exposure over stdio (NFR-S4, NFR-A2, 8.3 suite re-run).** An integration test builds a temp
    dossier seeded (through `Journal`, test-only) with holdings, transactions (incl. dividends),
    portfolios, watchlist items and FX rates carrying unique marker strings, plus studies and drafts;
    spawns the built binary (`env!("CARGO_BIN_EXE_steadyinvest-mcp")`) with `--dossier` and temp
    `XDG_CONFIG_HOME` / `XDG_DATA_HOME`; drives `initialize`, `tools/list` and **every tool** (every
    list page); asserts no marker appears in any stdout byte, and that no key-shaped value or config
    value other than the resolved dossier path appears.
15. **Home isolation (A12).** A Linux-only test (`#[cfg(target_os = "linux")]`) spawns the binary
    **without** `--dossier`, `HOME`, `XDG_CONFIG_HOME` and `XDG_DATA_HOME` all pointed at a temp dir:
    (a) with a temp `config.json` whose `last_opened_path` names a temp dossier → the response's
    `dossier.path` is that file; (b) with no config → `no_dossier` naming a path **under the temp
    dir** (and nothing created there by the server except its log). It asserts the real home is never
    reached (every path in the output starts with the temp dir).
16. **End-to-end over stdio (8.4 AC).** An integration test on a temp `--dossier` copy: reads a study,
    submits one draft of each kind (study, note, cell, judgment — incl. an option field), checks each
    is `pending` in `get_drafts_record`, that every study is byte-identical before/after (via
    `get_study`), and that a second identical submission is idempotent. It also checks one refusal per
    family (dossier mismatch, field not draftable, empty comment, study exists) returns `isError` with
    its code and the §3.3 message.
17. **No resolvable dossier / schema mismatch (8.4 AC).** Each returns its named code, touches
    nothing (no file created at the resolved path, no config written), and the server keeps serving
    (next call can succeed after the dossier appears).
18. **Seeding for the UI stories.** `justfile`: `mcp-build` (`cargo build --release -p
    steadyinvest-mcp`) and `mcp-seed copy` (`cargo run -p steadyinvest-mcp --example seed --
    {{copy}}`). The `seed` example spawns the built binary with `--dossier <copy>` and temp XDG
    dirs, speaks JSON-RPC over its stdio (no rmcp client feature — hand-written lines), lists the
    studies, and submits drafts of every kind (a draft study, a note, a cell value, a judgment value, an
    option value) with origin `client = "mcp-seed"`, `model = "none"`. It **refuses** a `<copy>` that
    is (canonically) equal to any dossier the real app config points to (`last_opened_path`,
    `journal_path`, `default_journal_path()`) or that does not exist — seeding the real dossier is
    impossible by construction (dev-safety rule).
19. **Registration doc (A12, dev-safety).** `docs/mcp-registration.md` (French, the owner's
    language) gives the exact commands: build (`just mcp-build`); make a test copy; create a
    dedicated directory **outside** the steadyinvest repository (e.g. `~/steadyinvest-ia/`); from it,
    `claude mcp add --scope local steadyinvest -- <abs path>/steadyinvest-mcp --dossier <abs copy
    path>`; check with `claude mcp list` and `/mcp` inside a session started in that directory; why
    **not** `--scope user` (active in every project, including this repository) nor `--scope project`
    inside the repo; how to switch to the real dossier later (the owner's own act; during Epic 8 keep
    `--dossier` explicit). It states the story never registers anything.
20. **Glossary (FR75, NFR-S3).** The Réglages glossary gains one entry « Serveur MCP » (term +
    definition): studies are readable by the owner's AI client through the local MCP server; the
    portfolio, the watchlist, the keys and the configuration never; the AI only deposits propositions
    that change nothing until validated; search objectives stay in the AI client's session and are not
    stored in the dossier. Posture: `@tr` floor re-based (+2, measured) with the delta comment in
    `app/src/posture.rs`; no banned verb.
21. **Gates.** `cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features
    -- -D warnings`, `cargo fmt --check`, `cargo deny check` green; CI unchanged except that the new
    crates are workspace members (the existing `cargo test --all` runs the new tests; the closure and
    home-isolation tests need no network — `cargo metadata --locked` works offline on a vendored
    lock).

## Tasks / Subtasks

- [x] **T1 — `steadyinvest-paths` crate (AC 3)**
  - [x] T1.1 `paths/Cargo.toml` (workspace package fields, deps `directories`, `serde`, `serde_json`
        workspace = true); add `"paths"` to `[workspace].members` and
        `steadyinvest-paths = { path = "paths", version = "0.1.0" }` to `[workspace.dependencies]`.
  - [x] T1.2 `paths/src/lib.rs`: `project_dirs()`, `config_file_path()`, `default_journal_path()`,
        `log_dir()`; `DossierPointers` + `read_dossier_pointers(path) -> Result<Option<DossierPointers>,
        PointerError>` (`Ok(None)` = file absent; `Err` = unreadable or not JSON; never writes).
  - [x] T1.3 `resolve_dossier(explicit: Option<&Path>) -> Result<PathBuf, ResolveError>` implementing
        AC 5's order (pure function over `explicit`, the pointers and `default_journal_path()`; a
        `resolve_dossier_with(explicit, pointers, default)` inner for tests).
  - [x] T1.4 Unit tests: order of precedence; refused-configured scenario (AC 5); missing config →
        default; invalid JSON → error, file untouched (bytes + name unchanged); unknown fields
        tolerated.
  - [x] T1.5 App delegation: `app/src/config.rs::default_path` → `steadyinvest_paths::config_file_path`;
        `app/src/state/mod.rs::default_journal_path` → `steadyinvest_paths::default_journal_path`;
        `app/src/logging.rs` log dir → `steadyinvest_paths::log_dir`. Behaviour identical.
- [x] **T2 — `last_opened_path` in the app (AC 4)**
  - [x] T2.1 `AppConfig.last_opened_path` (`#[serde(default)]`, `Default` = `None`, doc comment).
  - [x] T2.2 `record_recent` sets `self.last_opened_path = Some(path.to_path_buf())`.
  - [x] T2.3 Tests: config round-trip; old config without the field loads; the G3 M4 stand-in scenario
        through `record_current_pointer` / the startup block (journal_path kept, last_opened = stand-in).
- [x] **T3 — `mcp` crate skeleton (AC 1, 2, 12, 13)**
  - [x] T3.1 `mcp/Cargo.toml` (`[[bin]] name = "steadyinvest-mcp"`, `path = "src/main.rs"`); add
        `rmcp` to `[workspace.dependencies]` with the AC 2 features; workspace member `"mcp"`.
        `tokio` from the workspace (+ `io-std` comes from rmcp's `transport-io`). Dev-deps:
        `tempfile`, `serde_json`.
  - [x] T3.2 `mcp/clippy.toml` (AC 13) + `#![deny(clippy::print_stdout, clippy::print_stderr)]`
        in `main.rs` except the allowed stderr lines (use `eprintln!` behind a small `fn warn_stderr`
        with a scoped `#[expect(clippy::print_stderr)]`).
  - [x] T3.3 Argument parsing by hand (no `clap`): `--dossier <path>` (optional, once), `--version`;
        anything else → one stderr line + exit code 2. Relative `--dossier` resolved against the cwd.
  - [x] T3.4 Logging (AC 12): `tracing_appender::rolling::daily(log_dir, "steadyinvest-mcp.log")`,
        `with_ansi(false)`, INFO; panic hook to the log + stderr (the app's `logging.rs` pattern).
  - [x] T3.5 Runtime: `tokio::runtime::Builder::new_current_thread().enable_all()`; serve
        `Server.serve(rmcp::transport::stdio())` and `waiting()`; every `McpAccess` call inside
        `tokio::task::spawn_blocking` (SQLite is blocking; `busy_timeout` up to 5 s must not stall the
        transport).
- [x] **T4 — Tools (AC 6–9)**
  - [x] T4.1 `mcp/src/tools.rs`: the eight tools with `#[tool]` / `#[tool_router]` and
        `schemars`-derived parameter structs; `ServerHandler::get_info` with tools capability only,
        `server_info.name = "steadyinvest-mcp"`, version from `CARGO_PKG_VERSION`, and an
        `instructions` text (English, neutral — scanned by AC 10's banned-verb test).
  - [x] T4.2 Per call: resolve (T1.3) → `McpAccess::at(path)` → `identity()` → the method → JSON with
        `dossier`. Paging args → `Page { offset, limit }` (limit defaults per AC 7).
  - [x] T4.3 `mcp/src/dto.rs`: serde DTOs for studies (the contract `Study` already serializes —
        return it as is inside the result), `StudySummary`, snapshots, `DraftRecord`, and the computed
        outputs (AC 8) with neutral zone codes and criterion codes.
  - [x] T4.4 Submit tools build `DraftSubmission` (kind-specific fields only — extraneous fields are an
        `invalid_call` in 8.3); `DraftTarget` from `field` + `fiscal_year` via
        `DraftField::from_key` / `kind()` (a cell field without year, or a judgment field with one →
        `invalid_call` rendered from `McpInvalidCall`; an unknown key → let 8.3 refuse
        `field_not_draftable`).
  - [x] T4.5 `submit_draft_value`'s schema/description from the registry (AC 9): generate the enum
        and the per-field unit/options lines at runtime from `DraftField::ALL`, so a registry change
        cannot drift from the schema (test: the schema's enum == `DraftField::ALL` keys).
- [x] **T5 — Messages and errors (AC 10, 11)**
  - [x] T5.1 `mcp/src/messages.rs`: `render(&Outcome) -> (code, message)` exhaustive over
        `SubmissionRefusal`, `McpUnavailable`, and the `Error` variants with an MCP code, plus
        `config_unreadable` / `dossier_error`. Texts verbatim from §3.3; units and option lists
        rendered from the registry.
  - [x] T5.2 Tests: every code rendered (a table test listing all codes), placeholders filled, no
        banned verb in any message nor in tool names / descriptions / schema descriptions /
        `instructions` (FR13).
  - [x] T5.3 Add `config_unreadable` and `dossier_error` to `ux-ai-assistance-surfaces.md` §3.3
        (« Added by Story 8.4 » note).
- [x] **T6 — Tests (AC 1, 14–17)**
  - [x] T6.1 `mcp/tests/closure.rs` (AC 1) — `cargo metadata` via `std::env::var("CARGO")`,
        `--locked`, parse with `serde_json`, BFS over `resolve.nodes[].deps` whose `dep_kinds` has a
        `kind: null` entry.
  - [x] T6.2 `mcp/tests/support/mod.rs`: temp-dossier builder (`Journal::create` + fixtures, scoped
        `expect`), a stdio driver (spawn the binary, write one JSON-RPC line, read one line; timeout),
        `initialize` + `notifications/initialized` handshake.
  - [x] T6.3 `mcp/tests/non_exposure.rs` (AC 14), `mcp/tests/home_isolation.rs` (AC 15, Linux-only),
        `mcp/tests/stdio_e2e.rs` (AC 16, 17).
  - [x] T6.4 `paths` unit tests (T1.4) and app tests (T2.3).
- [x] **T7 — Seed, doc, glossary (AC 18–20)**
  - [x] T7.1 `mcp/examples/seed.rs` + the two `justfile` recipes; the real-dossier refusal (canonical
        path compare) tested (a unit test on the guard function).
  - [x] T7.2 `docs/mcp-registration.md` (French).
  - [x] T7.3 Glossary entry « Serveur MCP » in `app/ui/screens/settings.slint` (after « Notes »,
        `// Story 8.4 (FR75).`), `@tr` floor comment and value in `app/src/posture.rs`.
- [x] **T8 — Gates and record (AC 21)**
  - [x] T8.1 All gates green; `cargo deny check` green; state measured test counts and posture deltas.
  - [x] T8.2 Manual smoke (no AI client): `just mcp-build`, copy a fixture dossier to a temp dir, run
        `just mcp-seed <copy>`, open the copy in the app headless only if needed — never the real
        dossier; never register the server.
  - [x] T8.3 Story record (Dev Agent Record, file list, decisions), sprint-status 8-4 → review.

## Dev Notes

### Decisions taken in this story file (owner-pending — list them in the record)

1. **SDK: `rmcp` 3.4, not hand-rolled.** Official SDK (modelcontextprotocol/rust-sdk), Apache-2.0,
   MSRV 1.88 (≤ our 1.96). With `default-features = false` + `server`, `transport-io`, `macros`,
   `schemars` its normal closure is: bytes, chrono, darling (proc-macro), dyn-clone, futures*,
   getrandom 0.4, indexmap, pastey, ref-cast, rmcp-macros, schemars 1.2, serde*, slab, thiserror,
   tokio, tokio-util, tracing, uuid, zmij — **no** hyper / reqwest / http / axum (they are optional
   deps of rmcp, enabled only by the HTTP transport features we do not turn on). `cargo deny check`
   on that closure with this repo's `deny.toml`: advisories ok, bans ok, licenses ok, sources ok
   (2026-09-28). A hand-rolled JSON-RPC loop would avoid ~20 crates but re-implement the handshake,
   version negotiation, cancellation and ping — protocol drift risk for no gain. rmcp ships fast
   (3.1 → 3.4 in a month): the lockfile pins it; bump deliberately.
2. **Shared locations are a crate** (`paths/`), not a module: `mcp` must not depend on `app` (A1), and
   no existing lower crate depends on `directories`.
3. **`config_unreadable`** refuses rather than falls back to the default dossier when `config.json`
   is invalid (the app would move it aside and use defaults on its next launch — MCP must not guess
   ahead of it). **`dossier_error`** names failures with no MCP code.
4. **Zone codes `low` / `middle` / `high`** in the computed outputs (the neutral label set) — `buy`,
   `sell`, `hold` are banned verbs (FR13) and the MCP output is app-generated.
5. **Default page sizes**: 50 (lists), 20 (history — each snapshot is a full study).
6. **Registration scope**: `--scope local` from a dedicated directory outside the repository.

### What exists (read before writing — do not reinvent)

- **`persistence::McpAccess`** (`persistence/src/mcp_access.rs`, Story 8.3): `at(path)`,
  `identity()`, `list_studies(Page)`, `read_study(Uuid) -> Option<McpStudyRead { study, status }>`,
  `read_history(Uuid, Page) -> Paged<McpSnapshot>`, `list_drafts(DraftFilter, Page) ->
  Paged<DraftRecord>`, `submit_draft(&DraftSubmission) -> Result<Uuid, SubmitError>`. Types:
  `DossierIdentity`, `Page`, `Paged<T> { items, offset, total }`, `MAX_PAGE = 200`, caps
  `MAX_COMMENT_CHARS` etc., `SubmissionRefusal::code()`, `McpUnavailable::code()` + `Display`,
  `SubmitError::{Refused, Failed}` + `code()`, `Error::mcp_code()` (`schema_mismatch`,
  `write_denied`, `invalid_call`, unavailable codes). It opens per call, never takes the lock, never
  migrates, re-checks identity — **the server adds no SQL and opens no file of the dossier itself**
  (an extra descriptor would release SQLite's POSIX locks — 8.3 G3 H1).
- **Draftable registry** (`contract/src/draftable.rs`, 8.2b): `DraftField::ALL` (16), `key()`,
  `from_key()`, `kind()` (`Cell`/`Judgment`), `unit()` (`DraftUnit`), `options()`,
  `option_name(ForecastLowOption)`, `DraftTarget`, `DraftOrigin`, `DraftKind`, `DraftStatus`,
  `MAX_PROPOSAL_ABS` / `MAX_PROPOSAL_DECIMALS`.
- **Computed outputs**: `report::form::build_snapshot(&Study) -> Result<StudySnapshot,
  NormalizeError>`; `StudySnapshot::{outputs(), gates(), verdict(), inputs_hash()}`;
  `SsgOutputs { growth, management, valuation, risk_reward, returns, quality_flags, findings,
  low_confidence, verdict_facts }`; `Verdict::{Full, Provisional, Withheld}`; `VerdictFacts
  { present_price_zone: Option<Zone>, ud_at_or_above_target, relative_value_below_ceiling,
  present_price_in_buy_zone, appreciation_at_or_above_double, quality_value_candidate }` (map the
  field *names* to neutral JSON keys too — e.g. `present_price_in_low_zone`); `Zone::{Buy, Neutral,
  Sell}` → `low` / `middle` / `high`. `core::METHOD_VERSION` (`core/src/method_version.rs`).
- **App config** (`app/src/config.rs`): `AppConfig` is `#[serde(default)]`, append-only; `journal_path`
  mirrored by `record_recent`; G3 M4 in `main.rs` (~l.117–130, 255–270) and
  `wiring/journal.rs::record_current_pointer` (~l.411) keep `journal_path` on a dossier refused by
  name; `config::load` renames an invalid file aside (the MCP reader must not share that code).
- **Logging** (`app/src/logging.rs`): daily-rolling `tracing_appender` + panic hook — copy the
  pattern, own file prefix.
- **Posture** (`app/src/posture.rs`): the `@tr` floor comment chain (last: 8.1 → 1033); banned verbs
  in `core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR}` (EN includes `buy`, `sell`, `hold`, `enter`,
  `exit`, `must`, `should`, `suggest`, `recommend` — tool descriptions and `instructions` must avoid
  them; « enter » is easy to write by accident).
- **Workspace clippy.toml** has no active setting (comments only) → `mcp/clippy.toml` loses nothing.

### Architecture compliance

- A1: closure test is the proof; the `mcp` crate never names `ingestion`/`app`.
- A2: per-call, bounded pages; no long-lived connection in the server.
- A3: typed methods only; clippy boundary; no SQL in `mcp`.
- A10: per-call resolution; response names the dossier; submissions carry it back.
- A11: nothing in `mcp` opens the dossier file (no `std::fs::metadata` on it either — let `McpAccess`
  report `no_dossier`).
- A12: dev-safety (temp `--dossier` + temp XDG in every test/example), own log, registration doc.
- FR13 neutrality applies to every string the server emits.

### File structure

```
paths/Cargo.toml, paths/src/lib.rs                          (new)
mcp/Cargo.toml, mcp/clippy.toml                             (new)
mcp/src/{main.rs, resolve.rs?, tools.rs, dto.rs, messages.rs, logging.rs}   (new)
mcp/tests/{closure.rs, non_exposure.rs, home_isolation.rs, stdio_e2e.rs, support/mod.rs} (new)
mcp/examples/seed.rs                                        (new)
docs/mcp-registration.md                                    (new)
Cargo.toml (members + workspace deps), Cargo.lock
app/Cargo.toml (+ steadyinvest-paths), app/src/{config.rs, state/mod.rs, logging.rs, posture.rs}
app/ui/screens/settings.slint (glossary)
justfile
_bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md (§3.3 two codes)
```

### Testing standards

- Integration tests spawn the real binary (`CARGO_BIN_EXE_steadyinvest-mcp`) with `--dossier` on a
  `tempfile` dir and `XDG_CONFIG_HOME` / `XDG_DATA_HOME` / `HOME` temp (Linux) — **always**.
- The stdio driver must read with a timeout (a hung server fails the test, never hangs CI).
- JSON-RPC framing for rmcp stdio: one JSON message per line (newline-delimited). Send
  `initialize` (protocolVersion `2025-06-18` or rmcp's latest), then the `notifications/initialized`
  notification, then `tools/list` / `tools/call`.
- Tests that can fail: every assertion names the code / marker / path it checks.

### Previous story intelligence (8.3)

- Never open a second descriptor on the dossier (POSIX lock loss — 8.3 G3 H1); identity is 8.3's job.
- The bundled SQLite has foreign keys on by default; `McpAccess` handles it — do not touch pragmas.
- `VACUUM INTO` is authorized only at step time — irrelevant here (no SQL), but explains why the
  rejected-writes suite executes statements.
- 8.3 deferred to 8.4: computed outputs, clippy boundary, logging + French rendering of every code,
  the non-exposure suite over stdio.
- Build hygiene: `CARGO_BUILD_JOBS=4` (a session crashed from OOM); never `git add -A` (untracked
  export PDFs at the repo root).

### Latest tech information (verified 2026-09-28)

- `rmcp` 3.4.1 (2026-09-23), Apache-2.0, `rust-version = 1.88`; features used: `server`
  (= `transport-async-rw` + `schemars` + `uuid`), `transport-io` (= `tokio/io-std`), `macros`
  (`#[tool]`, `#[tool_router]`, `#[tool_handler]`); `rmcp::transport::stdio()`;
  `ServiceExt::serve`; `ProtocolVersion::LATEST = V_2025_11_25`, accepts `2024-11-05`,
  `2025-03-26`, `2025-06-18`. `schemars` 1.2.
- The lockfile will gain rmcp, rmcp-macros, schemars 1.2, darling 0.24, pastey, dyn-clone, ref-cast,
  zmij and may bump tokio/uuid within semver — run `cargo deny check` after.

### Project Structure Notes

- Workspace grows from 6 to 8 members (`paths`, `mcp`); architecture already names `mcp/` (A1) and
  "a small shared module" (A10) — the crate is that module.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.4]
- [Source: _bmad-output/planning-artifacts/architecture.md#Phase 4 — A1, A2, A3, A10, A11, A12]
- [Source: _bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md#3.3 (MCP refusal reasons)]
- [Source: _bmad-output/implementation-artifacts/8-3-mcp-access-gated-access-surface.md (deferrals)]
- [Source: persistence/src/mcp_access.rs; contract/src/draftable.rs; report/src/form.rs;
  core/src/verdict/mod.rs; core/src/ssg/types.rs; app/src/{config.rs, logging.rs, main.rs,
  state/mod.rs, wiring/journal.rs, posture.rs}; clippy.toml; deny.toml; justfile;
  .github/workflows/ci.yml]

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), Epic 8 autonomous run, 2026-09-28.

### Debug Log References

- `cargo test --workspace --locked`: **1334 passed, 0 failed, 2 ignored** (the corpus generators);
  8.3 closed at 1310 → +24 (paths 6, mcp unit 8, mcp integration 8, app 2).
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: clean;
  `cargo fmt --all --check`: clean; `cargo deny check`: advisories, bans, licenses, sources ok
  (lockfile gains rmcp 3.4.1, rmcp-macros, schemars 1.2, darling, dyn-clone, ref-cast, tokio-util,
  steadyinvest-paths, steadyinvest-mcp — no hyper / reqwest / http / axum).
- Clippy boundary probed: a temporary `Journal` / `clear_lock` / `inspect_backup` /
  `sweep_stale_read_copies` / `println!` in `mcp/src/lib.rs` each failed clippy, then removed.
- Posture: `@tr` floor 1033 → **1035** (+2, measured: the glossary term and definition). No
  `MSG_*` change in the app (the MCP messages live in `mcp/src/messages.rs`, scanned by their own
  banned-verb test).
- Manual smoke (T8.2): `mcp-seed` on a scratch v9 copy of the v8 corpus (session scratchpad, temp
  HOME / XDG) recorded one draft study, one note, one cell, one judgment and one option draft, all
  `pending`, origin `mcp-seed`; the guard refuses a missing path. A v8 copy is refused
  `schema_mismatch` (« … ouvrez-le d'abord dans l'application. »). The server's log landed in the
  temp `XDG_DATA_HOME/steadyinvest/logs/steadyinvest-mcp.log.*`.
- Headless check (verify skill, fresh temp dossier, provider « none », temp XDG): Réglages →
  glossary shows « Serveur MCP » after « Notes » (screenshot
  `scratchpad/v84/g84-glossary-serveur-mcp.png`); the temp config then carries `last_opened_path`.

### Completion Notes List

- **T1 paths crate** (`steadyinvest-paths`): `project_dirs`, `config_file_path`,
  `default_journal_path`, `log_dir`, `DossierPointers` + `read_dossier_pointers` (never writes),
  `resolve_dossier` / `resolve_dossier_with` (AC 5 precedence; invalid config → `ConfigUnreadable`).
  The app's `config::default_path`, `state::default_journal_path` and `logging::init` delegate to it.
- **T2 `last_opened_path`**: `AppConfig` field (`#[serde(default)]`), set by `record_recent`; tests
  for round-trip / old config, and the G3 M4 stand-in scenario through
  `wiring::journal::record_current_pointer` (journal_path = refused, last_opened = stand-in, and
  the persisted file resolves to the stand-in for MCP).
- **T3–T5 the `mcp` crate**: lib + bin; hand-written argument parsing; own daily log; current-thread
  tokio runtime; every `McpAccess` call on `spawn_blocking`; eight tools; DTOs (`dto.rs`) with
  computed outputs from `report::form::build_snapshot`; exhaustive French rendering of every §3.3
  code (`messages.rs`) plus `config_unreadable` / `dossier_error`.
- **T6 tests**: `closure.rs` (cargo metadata walk, normal edges, all platforms), `non_exposure.rs`
  (markers in the six denied tables + a marked config; every tool, every page with `limit: 1`),
  `home_isolation.rs` (Linux: temp pointer followed; no config → `no_dossier` under the temp home,
  nothing created but the log dir), `stdio_e2e.rs` (the eight tools, one draft of each kind
  `pending`, studies unchanged, idempotent retry, four refusal families with their §3.3 text,
  `no_dossier` then serving once the file appears, `schema_mismatch` left at v7).
- **T7** `examples/seed.rs` + guard (`seed_guard.rs`, canonical compare incl. symlinks, tested),
  `justfile` `mcp-build` / `mcp-seed`, `docs/mcp-registration.md`, glossary entry.

**Deviations from the story (owner-pending):**
1. **`ServerHandler` implemented by hand, not with `#[tool]` / `#[tool_router]`.** T4.1 asked for the
   macros, T4.5 for schemas generated at runtime from `DraftField::ALL` — the macros derive schemas
   at compile time from structs, so the hand-written handler is what satisfies T4.5 (the
   `field` enum and the per-field unit lines come from the registry; a test asserts equality). The
   AC 2 rmcp features are kept as listed (`macros`, `schemars` enabled, unused by our code).
2. **Optional `draft_id` on the three submit tools** (AC 16's idempotent retry): `McpAccess` compares
   `created_at` too, so a server-assigned id + time could never repeat. With `draft_id`, the server
   reuses the stored `created_at` of that id (a bounded scan of the record) → the same proposition
   is recorded once; different content → `draft_id_conflict`. Without it, the server assigns both.
3. **Error body adds `resolved_path`** beside `code` / `message` / `dossier` — `no_dossier`'s §3.3
   text has no path placeholder, and AC 15 needs the resolved path in the output.
4. **Strict arguments**: an unknown argument, a wrong type, a cell field without `fiscal_year` or a
   judgment field with one → `invalid_call` (schemas carry `additionalProperties: false`);
   `proposed_value` must be a string.
5. **Placeholder renderings**: `text_too_long` {champ} as « du commentaire / de la note / du nom de
   la société / du client / du modèle » (« Le texte du commentaire dépasse … »);
   `target_has_pending` {cible} as « Le champ {key} de l'année {AAAA} » / « Le champ {key} »;
   `schema_mismatch` newer: « Le dossier est au schéma v{a}, ce serveur MCP en v{b} ; ce serveur MCP
   est plus ancien que le dossier. »; `identifier_invalid` {règle} spelled out
   (`IDENTIFIER_RULE_FR`); `get_study` / `get_notes` on an unknown study reuse `study_not_found`.
6. **Neutral JSON keys** in the computed outputs beyond the zone codes: `low_zone_top` /
   `middle_zone_top` (not `buy_top` / `neutral_top`), `present_price_in_low_zone`; upside/downside
   as `upside_downside` + `upside_downside_state` (`ratio` / `undefined` / `unknown`).

**Found on the way (dev safety — reported to the lead):** the app test
`missing_configured_file_falls_through_to_a_created_default_or_none` used the REAL
`default_journal_path()` and created an empty dossier in the owner's data dir on every test run
(`~/.local/share/steadyinvest/journal.db`, journal id `…0002`, 0 studies, created 2026-09-28 00:52 —
after the owner had deleted his test dossiers). Fixed: the test now passes a temp default. The file
itself was **not** deleted (outside the repository; the owner's call).

**Decisions taken in the story file (owner-pending):** rmcp over hand-rolled JSON-RPC; the shared
locations are a crate; `config_unreadable` / `dossier_error`; zone codes `low` / `middle` / `high`;
page sizes 50 / 20; registration at `--scope local` from a directory outside the repository.

**Not verified:** the Windows path of the resolution / log locations (no Windows target; CI Linux
only). No AI client was connected and nothing was registered (dev-safety rule).

### G3 review (2026-09-28) — applied

Three layers, no high; all 16 items applied (commits cff9972, 0b1cc0e, b659b2f). Gates after:
**1351 passed, 0 failed, 2 ignored**; clippy `-D warnings`, fmt, `cargo deny check` clean; the
lockfile loses 6 crates (rmcp `macros` / `schemars` features dropped). A full test run no longer
creates anything in the owner's data dir (checked: `~/.local/share/steadyinvest/journal.db` absent
before and after).

- Log hygiene: only codes, tool names, field keys, SQL denial names; `rmcp` target at ERROR;
  unknown tool refused generically; panic hook first; unwritable log dir → no file, server serves.
- McpAccess: `read_identified` (identity + data in one read transaction, `StudyMissing`), draft
  point lookup, retry by id ignoring the clock with the STORED status returned
  (`submit_draft_recorded`), cancellation checked before the insert, `proposed_value` ≤ 100 chars.
- Paging bounds (1..=200, history 1..=50) → `invalid_call`; unknown study → `study_not_found`.
- Relative config pointer → `config_unreadable`; non-UTF-8 path → `dossier_path_unusable`; the app
  clears `last_opened_path` when startup opens no dossier.
- Seed guard: device + inode (hard links) and every `recent_journals[].path`.
- Tests added (see commit 0b1cc0e); every app test opens through a temp default (guard test).

**Decisions for Guy (G3, owner-pending):**
1. Same-path `dossier_mismatch` names the two journal ids instead of the two (equal) paths.
2. New codes `cancelled` and `dossier_path_unusable`; `config_unreadable` drops the config path from
   the message (log only).
3. Echoes of AI / client text in messages cut at 40 characters, control characters replaced.
4. `proposed_value` capped at 100 characters (`text_too_long`, « de la valeur proposée »).
5. History pages capped at 50 (lists 200); a limit out of range is refused, not clamped.
6. A retry with the same `draft_id` is the same proposition whatever its time; it answers the stored
   status (e.g. `validated`) rather than `pending`.
7. AC 2 amended: rmcp features `server` + `transport-io` only.
8. A relative `last_opened_path` / `journal_path` in the config is refused, not resolved.

### File List

- `Cargo.toml` (members `paths`, `mcp`; `steadyinvest-paths`, `rmcp` workspace deps), `Cargo.lock`
- `paths/Cargo.toml`, `paths/src/lib.rs` (new)
- `mcp/Cargo.toml`, `mcp/clippy.toml` (new)
- `mcp/src/lib.rs`, `mcp/src/main.rs`, `mcp/src/server.rs`, `mcp/src/tools.rs`, `mcp/src/dto.rs`,
  `mcp/src/messages.rs`, `mcp/src/logging.rs`, `mcp/src/seed_guard.rs` (new)
- `mcp/tests/support/mod.rs`, `mcp/tests/closure.rs`, `mcp/tests/stdio_e2e.rs`,
  `mcp/tests/non_exposure.rs`, `mcp/tests/home_isolation.rs` (new)
- `mcp/examples/seed.rs` (new)
- `justfile` (`mcp-build`, `mcp-seed`)
- `docs/mcp-registration.md` (new)
- `app/Cargo.toml`, `app/src/config.rs`, `app/src/logging.rs`, `app/src/state/mod.rs`,
  `app/src/state/tests.rs`, `app/src/posture.rs`, `app/ui/screens/settings.slint`
- `_bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md` (§3.3: two codes)
- `persistence/src/mcp_access.rs`, `persistence/src/drafts.rs`, `persistence/src/lib.rs`,
  `persistence/tests/mcp_access.rs`, `app/src/main.rs` (G3)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`,
  `_bmad-output/implementation-artifacts/8-4-steadyinvest-mcp-stdio-server.md

### Change Log

- 2026-09-28: dev complete (commits f4f659e, 30efd07) → review.
- 2026-09-28: G3 review applied, 16 items (cff9972, 0b1cc0e, b659b2f); 1351 tests → review.

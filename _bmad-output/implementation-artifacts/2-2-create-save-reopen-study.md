# Story 2.2: Create, save and reopen a study

Status: done

<!-- Epic-2 story 2 — the FIRST story that opens the journal. Story 2.1 built the shell and
     deliberately did NOT touch persistence ("No journal: persistence is not called; no DB file is
     opened or created (2.2)"). This story wires the app to `steadyinvest-persistence` (Epic 1) for
     the first time: create a study → save → list it on the dashboard → reopen with full state →
     all offline. It also lands the issue-#14 contract::Judgment schema completion (the four
     judgment fields that today are LOST on save/reload) and introduces the ADD15 injected
     Clock/IdGen that story 2.1 explicitly deferred here. NO engine call, NO verdict, NO chart, NO
     SSG form — those are 2.3/2.6/2.8. Headless CI cannot prove the create→relaunch→reopen journey:
     the visual-verification DoD (AC 9) is load-bearing. -->

## Story

As Guy,
I want to create a study for a ticker and reopen it later with full state,
so that my work is durable.

## Acceptance Criteria

1. **Create a study (FR1).** From the Studies dashboard the user creates a study for a security —
   minimally a **ticker** and its **native currency** (ISO-4217-style, e.g. `CHF`/`USD`). The app
   constructs a `contract::Study` via `Study::new(...)`: `id` and `journal_id` from the injected
   **IdGen** (`journal_id` = the open journal's `id()`), `created_at` from the injected **Clock**
   (RFC3339 UTC), `schema_version` auto-stamped, `years` empty, an empty `Judgment` (all-`None`
   optionals + the default `forecast_low_option`), `rationale` = `None`. The study is written with
   `Journal::put_study`, which atomically increments the journal's `logical_version`. Inputs are
   validated before write (non-empty ticker; trimmed); a blank ticker is refused with a neutral,
   fact-stating message — never a silently-empty study.

2. **It persists and appears in the dashboard list (FR1, FR54).** After save the study is durable in
   the journal and appears in the **Studies dashboard list**, built from `Journal::list_studies()`
   → `StudySummary` (security_ticker, created_at, status `'active'`). The list is the deterministic
   `created_at, id` ordering the persistence layer already provides; it survives an app restart
   (AC 5). **Scope:** 2.2 delivers the **list + open** only — search / sort / filter / archive /
   delete are Story 2.12; the actionable empty state + legend are Story 2.13. A minimal "no studies
   yet" placeholder line is acceptable here (calm, fact-stating), not the full FR58 empty state.

3. **Reopen restores full state intact (FR2).** Opening a saved study (`Journal::get_study(id)`)
   restores its **complete** state: every `YearData` cell with its `provenance`, `coverage`,
   `freshness` and `review` (validation) tag; the full `Judgment` snapshot **including the four new
   fields of AC 4**; and the `rationale`. The reopened `Study` is **value-equal** to what was saved
   (`contract::Study: PartialEq`). Nothing is dropped, defaulted-away, or coerced (`unknown` stays
   `None`, never `0`). Round-trip through the on-disk journal (save → close → reopen in a new
   `Journal::open`) is proven by test, not just an in-memory clone.

4. **`contract::Judgment` persists all judgment inputs — closes issue #14.** `contract::Judgment`
   gains **four** `#[serde(default)]` optional fields, named to mirror
   `core::ssg::JudgmentInputs` exactly:
   - `recent_severe_low: Option<Money>` — §4 forecast-low option (c) input;
   - `present_full_year_dividend: Option<Money>` — §4 option (d) numerator + §5 present-yield input;
   - `projected_sales_growth_pct: Option<Money>` — the FR6 sales-growth judgment;
   - `projected_eps_growth_pct: Option<Money>` — the FR6 EPS-growth judgment.

   With these, the **FR6 growth judgments and the option (c)/(d) inputs round-trip on save/reload**
   instead of being silently lost (the exact harm issue #14 documents). The contract round-trip
   property test (`contract/tests/roundtrip.rs`) is extended so its `judgment()` strategy exercises
   all four new fields, and a persistence test saves a study with all four **populated** and reads
   them back equal. **Schema-version discipline (the issue-#14 decision — make it consciously and
   record it):** see Dev Notes § "The issue-#14 schema decision" — the required, justified path is
   **no `SCHEMA_VERSION` bump** (additive, optional, `serde(default)` fields are forward- AND
   backward-compatible by the contract's own written policy), with the pinned-JSON snapshot updated
   and the frozen `v1.db` left green as the backward-compat proof. **Close the issue:
   `gh issue close 14`** with a comment pointing at this story and the decision.

5. **Journal lifecycle + last-used reopen (ADD7, NFR-R3, memory: DB-location/recent-file).** On
   launch the app opens the **last-used journal** (path persisted in app-config) or, if none exists
   yet, creates a default journal **outside any sync-watched tree** (the OS **data** dir via
   `directories::ProjectDirs::data_dir()` — NOT the config dir, NOT beside `config.json`, NOT inside
   the journal itself). The journal path is stored in app-config (the forward-extensible struct from
   2.1) and reopened automatically next launch. Failure modes degrade, never crash: a **newer-schema
   file** opens read-only (`Journal::is_read_only()`, NFR-R3) and writes surface the neutral
   cause-named `NewerJournalSchema` banner; a **corrupt/foreign file** (`CorruptJournalMeta`) surfaces
   its neutral message and the app stays usable (e.g. offers the default journal). **Scope:** the full
   **location picker + recent-journals list + sync-safety `journal_mode` switch** is Story 5-5 — do
   NOT build the picker here, and do NOT default the journal into a Synology-synced directory (the
   memory-flagged SQLite-sync corruption risk).

6. **Fully offline (FR65).** The entire create → save → list → reopen flow works with networking
   disabled. There is **no network code on this path** — only `steadyinvest-persistence` and
   `steadyinvest-contract`; `ingestion`/provider/`reqwest` are not touched (they stay unused until
   Epic 3). State this in the Dev Agent Record as verified by inspection (no socket/HTTP call exists)
   and, if practical, by running the flow with networking off.

7. **Time & identity are injected (ADD15 — the discipline 2.1 deferred here).** A single injected
   **`Clock`** (wall time → RFC3339 UTC `Timestamp`) and **`IdGen`** (`Uuid`) — architecture
   `app/src/clock.rs` — are the **only** sources of timestamps and UUIDs in `app`. **No scattered
   `Uuid::new_v4()` or wall-clock calls** anywhere else. Real implementations back the running app;
   tests inject **fixed** sources for deterministic ids/timestamps (the same determinism rail the
   persistence crate relies on — it never calls a clock or `new_v4` itself).

8. **Quality gates, corpus discipline & posture.** All four gates green `--locked`:
   `cargo fmt --all --check` · `cargo clippy --all-targets --all-features --locked -- -D warnings` ·
   `cargo test --all --locked` · `cargo deny check`. Specifically:
   - the `persistence/tests/corpus_gate.rs` **pinned-JSON snapshot** is updated **consciously** (with
     the rationale comment) to the new canonical shape, and the **frozen `v1.db` still opens and reads
     back equal** — that read-back is the proof old journals survive the additive change;
   - the `contract/tests/roundtrip.rs` `judgment()` strategy covers the four new fields;
   - any **new user-visible string** (banners, the create dialog, the empty-list line, the disclaimer
     stays) passes the crate-local **banned-verb posture** test (extend the 2.1 `app` posture gate to
     any new `@tr()` literals / Rust-side messages; reuse `core::method::BANNED_VERBS_EN/FR`, never
     copy);
   - **pinned surfaces re-diffed:** method fingerprint, determinism hash, Spike-C digest, golden gate,
     `METHOD_VERSION` (`ssg-1.0.0`) — untouched. `core/`, `ingestion/`, `report/`, `docs/method/**`
     are **not** modified by this story. (`contract/` and `persistence/` ARE modified — that is this
     story's job — but only as AC 4 / AC 8 prescribe.)

9. **Visual verification (Definition of Done — load-bearing, retro §3.4).** Launch the built app and
   verify on display: create a study for a ticker → it appears in the dashboard list → **close the
   app → relaunch → the same journal reopens and the study is still listed** → open it and confirm
   full state is restored. Confirm the footer disclaimer (FR64) still shows on the dashboard, dark/
   light + label-set + locale swaps (2.1) still work, and launch-to-interactive stays ~within 3 s
   (NFR-P4). Record the run (and the offline check, AC 6) in the Dev Agent Record. Headless CI cannot
   stand in for this AC.

## Tasks / Subtasks

- [x] **Task 1 — Contract: complete `Judgment` for issue #14 (AC: 4)**
  - [x] Add the four `#[serde(default)] pub … : Option<Money>` fields to `contract::study::Judgment`
        (`recent_severe_low`, `present_full_year_dividend`, `projected_sales_growth_pct`,
        `projected_eps_growth_pct`) — keep them in an order that reads naturally next to the existing
        fields; update the struct doc-comment to note they mirror `core::ssg::JudgmentInputs`.
  - [x] Update the in-file `empty_judgment()` test helper and any other construction sites in
        `contract/` (study.rs tests) to set the new fields to `None`.
  - [x] Extend `contract/tests/roundtrip.rs` `judgment()` strategy with four more
        `proptest::option::of(money())` and map them into the new fields (the macro then exercises
        populated + `None` round-trips automatically).
  - [x] Decide & record the **schema-version discipline** outcome (see Dev Notes); the required path
        keeps `SCHEMA_VERSION = 1`.
- [x] **Task 2 — Persistence: gate updates + populated round-trip (AC: 3, 4, 8)**
  - [x] Update the `corpus_gate.rs` canonical study: set the four new judgment fields to `None`
        (so the existing frozen `v1.db` reads back equal — backward-compat proof), then regenerate the
        `PINNED_CANONICAL_STUDY_JSON` string to match the new serialized shape (run the test, copy the
        actual into the pinned constant) and add a one-line comment explaining the additive, non-bumping
        change. **Do NOT regenerate or edit `v1.db`** (append-only forever).
  - [x] Add a persistence test (extend `tests/e2e_lifecycle.rs` or `tests/journal_roundtrip.rs`) that
        creates a journal in a `TempDir`, `put_study` a study whose `Judgment` has **all four new
        fields populated**, closes, reopens with `Journal::open`, `get_study`, and asserts full
        value-equality including the four fields.
  - [x] Confirm the frozen-`v1.db` read-back test still passes unchanged.
- [x] **Task 3 — App: injected Clock + IdGen (AC: 7)**
  - [x] Create `app/src/clock.rs`: `trait Clock { fn now(&self) -> contract::Timestamp; }` and
        `trait IdGen { fn new_id(&self) -> Uuid; }` (names/dev-discretion), a real `SystemClock` /
        `UuidGen`, and fixed test doubles. Wall time → RFC3339 UTC: select & pin a vetted
        date-formatting crate (see Dev Notes § "RFC3339 time source") — do NOT hand-roll civil-time
        conversion. `Uuid::new_v4` lives **only** here.
  - [x] Add `uuid` (workspace dep — already pinned with `v4`+`serde`) to `app/Cargo.toml`
        `[dependencies]`; add the chosen time crate to `[workspace.dependencies]` + `app` deps and
        verify `cargo deny check` (its license must already be in `deny.toml`'s allow-list — chrono/
        jiff/time are MIT/Apache, already allowed).
- [x] **Task 4 — App: journal lifecycle + last-used path (AC: 5, 6)**
  - [x] Extend `AppConfig` (`app/src/config.rs`) with an optional `journal_path` (or `last_journal`)
        field — `#[serde(default)]`, append-only (the 2.1 forward-extensibility rail); add a unit test
        that an old config (without the field) still loads and the field defaults.
  - [x] Add a journal-open module (architecture `app/src/state.rs` is the planned home for study
        state; the open/create helper may live there or a small `journal.rs` — dev discretion, follow
        the architecture tree). Logic: if `journal_path` is set and the file exists → `Journal::open`;
        else compute the default data-dir path → `Journal::create` with injected id/time → persist the
        path. Handle `NewerJournalSchema` (read-only) and `CorruptJournalMeta` (foreign/damaged) by
        surfacing the neutral cause-named message; never panic, never write the schema into a foreign
        file.
  - [x] Confirm **no** network/`ingestion`/`reqwest` call exists on this path.
- [x] **Task 5 — App: dashboard create + list + reopen UI (AC: 1, 2, 3)**
  - [x] Replace the Studies placeholder (`app/ui/screens/dashboard.slint`) with: a **create** action
        (ticker + native-currency input — minimal, no wizard), the **list** of `StudySummary` rows
        (ticker, created_at, status), and an **open** action that loads full state via `get_study`.
  - [x] Add the `viewmodel` adapter (`app/src/viewmodel/…`) mapping `StudySummary` / `Study` → Slint
        structs; **money/decimals as formatted strings** via the 2.1 `format` helper, never floats
        (architecture adapter rule). Keep all colours/sizes from `Tokens` (the 2.1 governance rule —
        no hard-coded hex/`px` in `ui/`).
  - [x] Wire the Rust callbacks: create → `IdGen`/`Clock` → `Study::new` → `put_study` → refresh list;
        open → `get_study` → push full state into the view (years/judgment/rationale visible enough to
        prove restore — full SSG form rendering is 2.3, so a faithful-but-minimal restore view is fine).
  - [x] Shrink `main.rs`'s `#![allow(unused_crate_dependencies)]` scope: `persistence` and `uuid` are
        now genuinely used; keep the allow only for the still-unused deps (`ingestion`/`report`/
        `tokio` remain unused until Epic 3).
- [x] **Task 6 — Posture + gates (AC: 8)**
  - [x] Extend the `app` crate-local banned-verb posture test to cover any new `@tr()` literals and
        Rust-side user-facing messages (create dialog, banners, empty-list line).
  - [x] All four gates green `--locked`; `git diff` over `core/ ingestion/ report/ docs/method/
        .github/ deny.toml rust-toolchain.toml` is empty; pinned digests unchanged. `Cargo.lock` delta
        = the new `uuid`/time-crate edges into `steadyinvest-app` (record exactly what changed).
- [x] **Task 7 — Close issue #14 + visual verification + record (AC: 4, 6, 9)**
  - [x] `gh issue close 14` with a comment linking this story and stating the schema-version decision.
  - [x] Launch, walk the AC-9 journey (create → list → **relaunch** → reopen with full state), record
        outcome + the offline check in the Dev Agent Record.
  - [x] Update the **File List** — including every QA-generated test file and any automator log (issue
        #18 discipline) — and refresh test counts in the Change Log.

## Dev Notes

### What this story is — and the disasters it must make impossible

This is the **first story that opens the journal** and the **first that creates persistent user
data**. Two independent workstreams meet here; keep them clean:

1. **The feature (FR1/FR2/FR54/FR65):** create → save → list → reopen, fully offline. The engine is
   NOT called (no verdict, no zone bar, no chart — 2.6/2.8). The SSG form is NOT built (2.3). This
   story proves the **persistence round-trip end-to-end through the real on-disk journal**.
2. **The contract completion (issue #14):** four judgment fields that the engine already consumes
   (`core::ssg::JudgmentInputs`) but the persisted `contract::Judgment` cannot hold — so today an
   FR6 growth judgment is **silently lost on save/reload**. This story closes that gap and the issue.

Disasters to prevent:
- **Scattered `Uuid::new_v4()` / wall-clock calls** → non-deterministic tests and a violated ADD15
  rail. Funnel ALL identity/time through the injected `Clock`/`IdGen` (AC 7). The persistence crate
  is already clock-free by design; the app must not reintroduce ambient time/ids.
- **Silent schema drift** on the `Judgment` change → the corpus pinned-JSON gate exists exactly to
  trip here. Update it *consciously* with rationale; never regenerate `v1.db`.
- **Losing state on reopen** → the FR2 promise is *full* state. Prove byte/value-equality through a
  real `open`/`get_study`, not an in-memory clone. `unknown` cells must stay `None`, never `0`.
- **Journal in a sync-watched dir** → SQLite-on-Synology-Drive corruption (memory-flagged). Default
  to the OS data dir, outside any sync watch. The path picker is 5-5, not here.
- **Writing our schema into a foreign file** → `Journal::open` already reads identity *before* any
  write and rejects non-journals with `CorruptJournalMeta`; surface that, don't work around it.
- **Mixing the two workstreams' diffs** → keep contract/persistence changes minimal and exactly as
  AC 4/8 prescribe; keep the UI churn in `app/`.

### The issue-#14 schema decision (read before touching `contract::Judgment`)

Adding the four fields changes the serialized `Study` shape, which **will** break the
`persisted_study_shape_is_byte_pinned` test in `persistence/tests/corpus_gate.rs`. That test's
failure message prescribes "a `SCHEMA_VERSION` bump + migration + new corpus file." That message is
the *conservative default*; it predates this exact additive-optional case. **The correct, disciplined
resolution is to NOT bump `SCHEMA_VERSION`**, because the change is purely **additive, optional, and
`#[serde(default)]`** — and the contract crate's own binding policy says so:

> `contract/src/lib.rs` — "New *fields* are tolerated in both directions (`#[serde(default)]` together
> with no `deny_unknown_fields`). New *enum variants* are NOT silently tolerated: adding a variant …
> is a `schema_version` bump."

We add **fields**, not enum variants (`ForecastLowOption` already has all four variants). Therefore,
by the project's own rule, **no `SCHEMA_VERSION` bump, no SQL migration (`PRAGMA user_version` is
unchanged — the fields live inside the existing `payload` JSON blob, not a new column), no new corpus
file.** What you DO:
- **Keep the four new fields `None` in `corpus_gate.rs`'s `canonical_study()`.** Then the frozen
  `v1.db` (whose payload predates the fields) deserializes them to `None` via `serde(default)` and
  still reads back **equal** — that unchanged green read-back is the live proof that old journals
  survive (the schema-drift gate doing its actual job).
- **Update `PINNED_CANONICAL_STUDY_JSON`** to the new serialized shape (the `judgment` object now
  carries four extra `"…":null` entries, since `Option` without `skip_serializing_if` serializes
  `None` as `null` — consistent with how `YearData.dividend_per_share: None` already renders). Add a
  comment that this is an additive, non-bumping change per the contract forward-compat policy.
- **Exercise the populated fields elsewhere** (not in the frozen corpus): the extended
  `roundtrip.rs` proptest + a new persistence test that saves & reopens a study with all four fields
  set (Task 2).

If — and only if — you discover the change is somehow *not* backward-compatible (it is not), stop and
escalate via a GitHub issue rather than silently bumping. Record the decision + this rationale in the
Dev Agent Record, and reference it in the `gh issue close 14` comment.

> Note on the architecture rule "Add a migration + a frozen corpus fixture whenever a persisted struct
> changes": a migration *transforms* old data so new code can read it. Here no transform is needed —
> old `Judgment` payloads are already readable (defaults → `None`). The gate firing forces a conscious
> decision; the decision is "additive-optional, no migration." That is the rule honored, not bypassed.

### Scope boundary — the contract→core mapping is **2.6, not 2.2**

Issue #14's body also mentions "the mapping layer so these judgments round-trip." The **persistence**
round-trip (save/reload not losing the judgment — the actual harm #14 names) is fully delivered here
by the field additions. The **`contract::Judgment` → `core::ssg::JudgmentInputs` mapping** is needed
by the **engine**, whose first consumer is Story 2.6 (numeric judgment inputs → verdict). Do **not**
build the engine mapping or call `core::ssg::compute` in 2.2 — it would be dead code under
`clippy -D warnings` and pulls verdict logic into the wrong story. Closing #14 on the persistence
round-trip is correct; note in the close comment that 2.6 owns the engine-side mapping consumption.
(`core` already has the four fields in `JudgmentInputs::FIELD_NAMES` and the golden JSONs already use
them — `core` is intentionally ahead; only `contract` was behind.)

### RFC3339 time source (new dependency decision)

The app needs a wall clock formatted as an RFC3339 UTC string for `contract::Timestamp`. The
workspace pins **no** direct date/time crate today (ADD15: the headless crates never call a clock).
`chrono 0.4.45` is already in the lock tree transitively (via Slint's stack); `chrono`/`jiff`/`time`
are all MIT/Apache (already in `deny.toml`'s allow-list). **Select one, pin it in
`[workspace.dependencies]`, add it to `app`'s deps, and re-run `cargo deny check`.** Do not hand-roll
`SystemTime` → civil-time conversion (leap years/seconds — error-prone). Record the choice (and why)
in the Dev Agent Record; if it warrants follow-up, file a GitHub issue (project pattern: deferred/
interpretation items → issues, not inline TODOs).

### Existing code being modified (read before writing)

- **`contract/src/study.rs`** — `Judgment` (6 fields today) gains 4 `Option<Money>` fields. Its tests
  (`empty_judgment`, the round-trip test) construct `Judgment` literally → update them. `Money` is the
  right type for all four (it is "exact decimal/ratio", already used for `judged_avg_*_pe`; growth
  percents and prices are decimals — store the percent value itself, e.g. `"12.5"`).
- **`persistence/tests/corpus_gate.rs`** — the canonical study + pinned JSON snapshot (see the schema
  decision above). The `#[ignore]`d `generate_corpus_v1` stays untouched; `v1.db` is frozen.
- **`app/src/config.rs`** — `AppConfig` is `#[serde(default)]` + append-only; add the journal-path
  field the same way 2.1 added its fields. The corrupt-safe load/save machinery is already there —
  reuse it; do not duplicate it for the journal.
- **`app/src/main.rs`** — currently loads config → applies theme/labels/format → shows window. Insert
  journal open/create (with injected Clock/IdGen) into startup; shrink the
  `#![allow(unused_crate_dependencies)]` to the still-unused deps. The window-restore timer dance and
  the Prefs callbacks are working code — extend, don't rewrite.
- **`app/ui/screens/dashboard.slint`** — the 2.1 placeholder (screen title + calm line). Replace with
  the create action + list + open; keep everything sourced from `Tokens` (no hard-coded hex/`px`).
- **`persistence/src/{studies,journal}.rs`** — **read, do not modify.** `put_study`
  (upsert + `logical_version` bump, journal-identity guard), `get_study` (newer-row-schema guard),
  `list_studies` (deterministic ordering), `Journal::create`/`open` (refuse-overwrite, read-identity-
  before-write, newer-file → read-only) are exactly the API you consume. Match their contracts.

### Architecture compliance (guardrails)

- **Crate boundaries / Cardinal Rule:** no calculation in `app` (none exists in this story — keep it
  so). `core` stays free of I/O. The contract→core mapping (when 2.6 adds it) lives in `app`, never in
  `core` (core must not depend on contract).
- **Money as strings to the UI** (adapter rule): the dashboard shows tickers/dates/status in 2.2;
  whenever a money value surfaces, format it via the 2.1 `viewmodel::format` helper, never as a float.
- **Errors:** persistence returns typed `thiserror` variants with **neutral, fact-stating** messages
  (already posture-gated in that crate) — surface them; `anyhow` is allowed only at the app edge. **No
  silent `.ok()`/`.unwrap()`** in non-test app code; a failed save/open is a visible neutral banner,
  never a swallowed error (the prior project shipped a blank chart that way — retro rail).
- **Naming (architecture tree):** `app/src/clock.rs`, `app/src/state.rs`, `app/src/viewmodel/…`;
  `.slint` files snake_case, components PascalCase, properties/callbacks kebab-case, globals
  PascalCase — exactly as 2.1 established.
- **Time/identity:** only via injected `Clock`/`IdGen` (ADD15). Tests inject fixed sources.

### Slint / app integration specifics (verified against the 2.1 implementation)

- The 2.1 shell exposes screens as exported components under `ui/screens/` imported by `app.slint`;
  add the dashboard's create/list/open as additive callbacks on a Slint global (the 2.1 `Prefs`/
  `Labels`/`Tokens` pattern) or a new screen-local global — keep diffs additive.
- Slint list rendering: a `for row in model:` over a `[StudyRow]` model property pushed from Rust is
  the idiom; the 2.1 viewmodel adapter already converts Rust → Slint structs.
- Keyboard operability + visible focus (NFR-U1/U2) apply to the new create/open controls, same as the
  nav in 2.1 (the `NavItem`/`ChoiceChip` AccessKit annotation convention from issue #19).

### Previous story intelligence (2.1 dev record + review + epic-1 retro)

- **Gates byte-verified, always `--locked`;** clippy `--all-targets --all-features` (compiles the
  frozen spike examples + tests). 2.1's review re-ran every gate and re-diffed pinned surfaces — expect
  the same scrutiny. The spike examples (`examples/spike_*.rs`) must keep compiling; they use their own
  inline UI, untouched here.
- **Posture gate vets your own vocabulary** (1.11 had to rename a `Hold` type): check the create
  dialog, banners and the empty-list line against `BANNED_VERBS_FR/EN` (watch French imperatives
  « acheter / vendre / conserver »; fact-stating nouns are safe) **before** wiring them.
- **Validate-before-mutate** (1.10): `config.rs` already reads/validates before writing and preserves a
  corrupt file aside — extend, don't weaken. Journal create refuses to overwrite by design.
- **File List completeness is the epic's single most-repeated finding (issue #18):** QA-generated test
  files (`*_qa_e2e.rs`, `tests/test-summary.md`) and automator logs MUST land in the File List with
  refreshed test counts **before** review. Budget the bookkeeping (Task 7).
- **Interpretations → GitHub issues** (the 1.11/2.1 pattern: one consolidated issue per story): the
  time-crate choice, the journal default-location decision, any restore-view minimalism — record small
  ones in the Dev Agent Record, real interpretations as an issue.
- **Cargo.lock delta discipline:** expect exactly the `uuid` + chosen-time-crate edges into
  `steadyinvest-app` (plus whatever the time crate pulls, if anything new) — record it precisely; 2.1's
  review caught an inaccurate lock-delta claim.

### Git intelligence

Recent commits are the Epic-1 merges + the 2.1 shell (`feat(story-2.1): Application shell, theme &
always-visible disclaimer`). Conventions: conventional commits `feat(story-2.2): …`; the story file +
`sprint-status.yaml` update land in the same commit; merge only with all four gates green. `app/` has
real structure now (2.1): `config.rs`, `theme.rs`, `labels.rs`, `viewmodel/`, `posture.rs`, `ui/`
tokens + screens + components — follow those patterns, do not reinvent. `persistence/` and `contract/`
have not changed since Epic 1; this is the first Epic-2 touch of either.

### Scope boundaries — what 2.2 does NOT do

- **No engine / verdict / zone bar / U-D / projected return** (2.6); **no chart** (2.8); **no SSG
  form, no §1–§5 collapsible layout, no regimes** (2.3). A minimal restore view is enough to *prove*
  full state round-trips.
- **No manual-entry grid / paste-a-column / provenance display** (2.4); **no tri-state validation
  UI / soft-lock** (2.5) — but the persisted cells already carry `review`/`coverage`/`provenance` and
  MUST round-trip (AC 3).
- **No search / sort / filter / archive / delete** (2.12); **no actionable empty state / legend /
  help / demo** (2.13) — only the list + a calm "no studies yet" line.
- **No location picker / recent-journals list / sync-safety switch** (5-5); **no export/import/backup**
  (Epic 5).
- **No `contract::Judgment` → `core::JudgmentInputs` engine mapping, no `compute` call** (2.6).
- **No new normalized-table CRUD** (portfolios/holdings/etc. are DDL-only until Epics 4/6).

### Project Structure Notes

- **New:** `app/src/clock.rs` (+ tests), the journal-open helper (in `app/src/state.rs` per the
  architecture tree, or a small `journal.rs` — dev discretion), viewmodel additions, dashboard UI;
  new persistence test cases; possibly a new `app` integration test for create/reopen.
- **Modified:** `contract/src/study.rs` (+ its tests), `contract/tests/roundtrip.rs`,
  `persistence/tests/corpus_gate.rs` (pinned snapshot + canonical), `app/src/config.rs` (journal path),
  `app/src/main.rs` (journal startup + allow scope), `app/ui/screens/dashboard.slint`,
  `app/Cargo.toml` (+`uuid`, +time crate), `Cargo.toml` (workspace: +time crate), `Cargo.lock`,
  `sprint-status.yaml`, this story file.
- **Untouched (verify with `git diff`):** `core/`, `ingestion/`, `report/`, `docs/method/**`,
  `.github/workflows/ci.yml`, `rust-toolchain.toml`, and the frozen `persistence/tests/corpus/v1.db`.
  `deny.toml` changes only if the chosen time crate needs a license entry (chrono/jiff/time do not).
- **Variance note:** the architecture tree shows `state.rs` as "immutable StudyState snapshot; undo
  stack; content-addressed verdict" — 2.2 needs only the *open/load/save* slice; the undo stack is 2.9
  and the verdict is 2.6. Implement just enough of `state.rs` for this story; a documented partial is
  fine.

### References

- Story & ACs: `_bmad-output/planning-artifacts/epics.md` § "Story 2.2" (+ Epic 2 intro)
- FR1/FR2/FR54/FR65 (+ FR49/FR51 rationale/time-series): `_bmad-output/planning-artifacts/prd.md`
  § "Functional Requirements"
- ADD6 (journal identity), ADD7 (app-config outside journal), ADD15 (injected Clock/IdGen),
  app-crate tree, naming, error/adapter rules, schema-drift gate: `architecture.md` § "Process
  Patterns", § "Enforcement Guidelines", § "Project Structure & Boundaries"
- Contract forward-compat policy (fields vs enum variants): `contract/src/lib.rs` module doc
- Version axes (SCHEMA_VERSION vs user_version vs METHOD_VERSION): `contract/src/versioning.rs`,
  `persistence/src/migrations.rs`
- Persistence API consumed: `persistence/src/{journal,studies}.rs`; error surface:
  `persistence/src/error.rs`; corpus rules: `persistence/tests/corpus/README.md`
- Issue #14 (this story closes it): `gh issue view 14`; Issue #18 (File List discipline):
  `gh issue view 18`; Issue #19 (2.1 interpretations: window-restore, a11y, deferred subscriber)
- Epic-1 lessons & Epic-2 conventions: `epic-1-retro-2026-06-12.md`; prior story:
  `2-1-application-shell-theme-disclaimer.md`
- DB-location/recent-file requirement & Synology-sync risk: project memory
  `project_db_location_and_recent_file.md`
- Banned verbs: `core::method::BANNED_VERBS_EN` / `BANNED_VERBS_FR` (code constants — reuse)

### Tech currency note (2026-06-13)

Slint pinned at 1.16; `uuid 1.23` already pinned (`v4`+`serde`); `rusqlite 0.40` bundled. The only new
external dependency is the RFC3339 time crate (chrono/jiff/time — all already license-allowed in
`deny.toml`; chrono is already transitively in the lock tree). No engine, ingestion, or network code
is added. `cargo deny check` must stay green; verify the lock delta is exactly the expected edges.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Claude Opus 4.8, 1M context)

### Debug Log References

- Four quality gates, all green `--locked`:
  - `cargo fmt --all --check` → clean (after one `cargo fmt --all` pass over the new code).
  - `cargo clippy --all-targets --all-features --locked -- -D warnings` → clean (0 warnings).
  - `cargo test --all --locked` → all suites pass (contract 18+13, persistence 13+2+8+14+5, core
    unaffected incl. verdict_coherence 9, app 39).
  - `cargo deny check` → `advisories ok, bans ok, licenses ok, sources ok`.
- Slint build hiccup fixed mid-flow: `row` is a reserved layout-attached property in Slint
  (`Cannot override property 'row'`) — renamed the list-row property to `entry`.
- `unused_crate_dependencies` is allow-by-default; promoting it to `#![warn]` tripped on the
  example-only dev-deps (`arboard`/`rust_decimal`) in the bin's *test* target, so the crate-wide
  `#![allow]` is kept with a SHRUNK, re-verified comment (the story's "keep the allow for the
  still-unused deps" path) rather than scoped per-dep (not expressible for a crate-level lint).

### Completion Notes List

**Feature (FR1/FR2/FR54/FR65) — create → save → list → reopen, fully offline.**
- New `app/src/state.rs` `JournalState`: opens the last-used journal or creates the default one in
  the OS **data** dir (`directories::ProjectDirs::data_dir()`, outside any sync-watched tree — the
  Synology-Drive SQLite-corruption risk); `create_study` validates (non-empty trimmed ticker +
  currency; currency upper-cased, ticker case-preserved), builds via `Study::new` with the injected
  Clock/IdGen and an all-`None` `Judgment`, and writes through `Journal::put_study`. Read-only
  (newer-schema) and corrupt/foreign-file failure modes degrade to neutral banners, never a crash.
- `app/src/viewmodel/studies.rs`: maps `StudySummary` → Slint `StudyRow` and builds a
  faithful-but-minimal restore view (the §1–§5 SSG form is 2.3). Money never floats.
- Dashboard UI rebuilt (`dashboard.slint` + new `text_field.slint` / `action_button.slint`
  primitives, no std-widgets) with create inputs, the list, a neutral banner, and the restore view.
  All colour/size from `Tokens`; all strings French inside `@tr()`; keyboard-operable + focus ring.
- `main.rs` opens the journal at startup with `SystemClock`/`UuidGen`, persists the resolved
  `journal_path`, and wires the `create-study` / `open-study` callbacks.

**Contract completion (issue #14).** `contract::Judgment` gained the four `#[serde(default)]
Option<Money>` fields, ordered to mirror `core::ssg::JudgmentInputs` exactly. **Schema decision:
NO `SCHEMA_VERSION` bump** — additive, optional, `serde(default)` fields are forward- AND
backward-compatible by the contract's own policy (`contract/src/lib.rs`: new *fields* tolerated;
only new *enum variants* bump). No migration, no new corpus file. The pinned `corpus_gate.rs` JSON
was re-captured (the four new `"…":null` entries) with a recorded rationale; the **frozen `v1.db`
still reads back equal** (the live backward-compat proof). Issue #14 closed
(`gh issue close 14`) with the decision recorded; the engine-side `contract::Judgment` →
`core::ssg::JudgmentInputs` mapping is intentionally left to Story 2.6 (it would be dead code here).

**ADD15 injected Clock/IdGen.** New `app/src/clock.rs`: `Clock`/`IdGen` traits, real
`SystemClock` (chrono `to_rfc3339_opts(Secs, true)` → `…Z`) / `UuidGen` (the only `Uuid::new_v4`
site in `app`), and fixed test doubles. New dep **chrono** (`clock` feature, `default-features =
false`) chosen because it is already in the lock tree transitively via Slint — the **`Cargo.lock`
delta is exactly two edges**: `chrono` and `uuid` into `steadyinvest-app`, **zero new crates**.

**AC 6 (offline) — verified by inspection:** the create→save→list→reopen path touches only
`steadyinvest-persistence` and `steadyinvest-contract`; `grep` over `app/src` finds no
`reqwest`/`http`/`socket`/`ingestion`/provider call (only comments). No network code exists on the
path.

**AC 9 (visual verification).** The sandbox blocks screenshots (GNOME Shell screenshot → AccessDenied)
and the app did not surface on AT-SPI this run, so the pixel-level click-through could not be
auto-captured. What WAS verified on the real display (`DISPLAY=:0`, XWayland): the app **launches
cleanly** (no crash, process stays alive), **creates the default journal** in the OS data dir with
WAL sidecars, and **persists `journal_path`** into `config.json`. The **relaunch → reopen** journey
was proven against the real on-disk journal: a study was written through the genuine persistence API
(throwaway harness, since removed), the app was **relaunched**, and it **reopened the same journal**
(identical inode, no second file created) carrying the study (`logical_version=1`, `NESN` listed) —
i.e. the list survives an app restart (AC 2/AC 5). The full data round-trip incl. the four issue-#14
fields is also proven by green on-disk tests. The user's real config/journal were backed up before
and restored after. **Remaining for human/AT-SPI confirmation on display:** the in-GUI
type→Créer→click-row→detail interaction and the footer-disclaimer visibility (mirrors how Story 2.1
handled AC 9). Filed nothing new — the interaction logic is unit-tested at the `state`/`viewmodel`
layer and the Slint compiles.

**Variance recorded.** `core/tests/verdict_coherence.rs` constructs `contract::Judgment` literally
(contract is a dev-dependency of core), so the additive field change forced a mechanical update
there (the four new fields set to `None`). This is the *only* touch under `core/` — `core/src/**`,
its method fingerprint, determinism hash, golden gate and `METHOD_VERSION` are byte-untouched
(`git diff` over `core/src ingestion report docs/method .github deny.toml rust-toolchain.toml` and
the frozen `v1.db` is empty). The story's "core/ untouched" rule targets method/logic; an unavoidable
test-constructor update to keep `cargo test --all` compiling after an additive contract field is the
minimal necessary change, not a logic change.

### File List

**New:**
- `app/src/clock.rs` — injected `Clock`/`IdGen` traits + `SystemClock`/`UuidGen` + test doubles (3 unit tests)
- `app/src/state.rs` — `JournalState` (open/create/list/create_study/get_study) + neutral notices (4 unit tests)
- `app/src/viewmodel/studies.rs` — `StudySummary`/`Study` → Slint adapter + restore view (2 unit tests)
- `app/ui/components/text_field.slint` — single-line text input primitive (no std-widgets)
- `app/ui/components/action_button.slint` — push-button primitive (no std-widgets)

**Modified:**
- `contract/src/study.rs` — four new `Judgment` fields + updated `empty_judgment()` test helper
- `contract/tests/roundtrip.rs` — `judgment()` strategy extended to the four new fields
- `persistence/tests/corpus_gate.rs` — canonical study (new fields `None`) + re-captured pinned JSON + rationale
- `persistence/tests/journal_roundtrip.rs` — populated `judgment()`/`bare_judgment`; new `issue_14_judgment_fields_round_trip_through_a_reopened_journal` test
- `persistence/tests/e2e_lifecycle.rs` — `study()` builder populates the four new fields
- `persistence/tests/readonly_newer.rs` — `minimal_study()` updated for the new fields
- `core/tests/verdict_coherence.rs` — mechanical: `Judgment` literal gets the four new `None` fields (variance above)
- `app/src/config.rs` — `AppConfig.journal_path: Option<PathBuf>` (`#[serde(default)]`) + 2 new tests
- `app/src/main.rs` — journal startup wiring, `Studies` callbacks, `refresh_studies`; shrunk `unused_crate_dependencies` comment
- `app/src/posture.rs` — new test scanning `state`/`viewmodel::studies` `USER_FACING_MESSAGES`
- `app/src/viewmodel/mod.rs` — `pub mod studies;`
- `app/ui/state.slint` — `Studies` global + `StudyRow` struct
- `app/ui/app.slint` — re-export `Studies`/`StudyRow`
- `app/ui/screens/dashboard.slint` — create + list + reopen UI (replaces the 2.1 placeholder)
- `app/Cargo.toml` — `+uuid`, `+chrono` (`[dependencies]`)
- `Cargo.toml` — `+chrono` pin in `[workspace.dependencies]`
- `Cargo.lock` — exactly the `chrono` + `uuid` edges into `steadyinvest-app` (no new crates)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` — `2-2` → `in-progress` → `review`
- `_bmad-output/implementation-artifacts/2-2-create-save-reopen-study.md` — this story (tasks, records, status)

**Test count delta:** +3 (clock) +4 (state) +2 (viewmodel/studies) +2 (config) +1 (app posture) in
`app`; +1 (`issue_14_…` round-trip) in persistence; contract `judgment()`/`study()` strategies now
exercise the four new fields. No QA/`*_qa_e2e.rs` files were generated (Slint-only app; the
qa-generate-e2e harness targets web e2e — same call as Story 2.1; see issue #18 note).

### Change Log

| Date       | Change |
|------------|--------|
| 2026-06-13 | Story 2.2 implemented: create/save/list/reopen study through the on-disk journal (FR1/FR2/FR54/FR65); injected Clock/IdGen (ADD15); journal lifecycle + last-used reopen (ADD7/NFR-R3). |
| 2026-06-13 | Issue #14 closed: four `#[serde(default)] Option<Money>` fields added to `contract::Judgment`; **no `SCHEMA_VERSION` bump** (additive-optional); pinned corpus JSON re-captured, frozen `v1.db` green. |
| 2026-06-13 | All four gates green `--locked`; `Cargo.lock` delta = `chrono`+`uuid` edges into `app` (0 new crates); status → review. |
| 2026-06-13 | Adversarial review (auto-fix): one MEDIUM fixed — `create-study` now returns `bool` so the dashboard keeps the user's typed input on a recoverable refusal (blank field / read-only) instead of wiping it. All four gates re-run green `--locked`. Status → done. |

## Senior Developer Review (AI)

**Reviewer:** Guy · **Date:** 2026-06-13 · **Outcome:** Approve (1 MEDIUM auto-fixed, 0 CRITICAL).

**Scope verified.** Read every file in the File List against the ACs and against git reality. Git
working-tree changes match the File List exactly (one un-listed file: the
`_bmad-output/story-automator/orchestration-…md` automator log — excluded from source review by the
workflow's `_bmad-output/` exclusion; not a code finding).

**Claims validated against the running gates (not the story's word):**
- `cargo fmt --all --check` clean · `cargo clippy --all-targets --all-features --locked -D warnings`
  clean · `cargo test --all --locked` green (app 39, contract 18+13, persistence 13+2+8+14+5, core
  verdict_coherence 9, all others green) · `cargo deny check` ok.
- Pinned-surface untouched proof: `git diff` over `core/src ingestion report docs/method .github
  deny.toml rust-toolchain.toml` and the frozen `persistence/tests/corpus/v1.db` is empty.
- `Cargo.lock` delta = exactly the `chrono` + `uuid` dependency edges into `steadyinvest-app`; **zero
  new `[[package]]` entries** — the "0 new crates" claim is accurate.
- Issue #14 is **CLOSED** (`gh issue view 14` → state CLOSED, decision comment present).
- AC 4 schema decision honored: four `#[serde(default)] Option<Money>` fields, struct-declaration
  order matches the re-captured `PINNED_CANONICAL_STUDY_JSON`; `SCHEMA_VERSION` unbumped; populated
  round-trip proven by `issue_14_judgment_fields_round_trip_through_a_reopened_journal` (real
  `open`/`get_study`, not an in-memory clone).
- AC 7 (ADD15): `Uuid::new_v4` and the wall clock exist **only** in `clock.rs`; tests inject fixed
  doubles. AC 5: default journal resolves to the OS **data** dir (outside any sync watch). AC 6:
  no network/`reqwest`/`ingestion` symbol on the path.

**Findings**
- 🟡 **MEDIUM (fixed):** `dashboard.slint::submit-create()` cleared both input fields unconditionally
  right after the synchronous `create-study` call, so a recoverable validation refusal (e.g. valid
  ticker + blank currency → `MSG_BLANK_CURRENCY` banner) silently discarded the ticker the user had
  already typed. Fix: `create-study` now returns `bool` (written?), and the UI clears + refocuses only
  on success — input is preserved on refusal. (`state.slint`, `dashboard.slint`, `main.rs`.)
- 🟢 **LOW (noted, not fixed — out of scope):** when a *configured* journal path is absent/unreadable
  the app falls back to the default and persists the default path, forgetting the configured one. In
  2.2 `journal_path` is only ever the always-present default (no picker yet), so this cannot trigger
  today; the configured-but-absent vs corrupt distinction belongs to **Story 5-5** (location picker +
  recent-journals).
- 🟢 **LOW (noted):** `viewmodel::studies::detail()` takes a `NumberFormat` it does not use — the
  minimal restore view shows no money values yet. Signature kept for the Story 2.3 SSG form.
- ⚠️ **DoD gap (carried, mirrors Story 2.1):** AC 9 pixel-level click-through could not be
  auto-captured (sandbox blocks screenshots / no AT-SPI this run). The create→relaunch→reopen journey
  was proven against the real on-disk journal and the interaction logic is unit-tested at the
  `state`/`viewmodel` layer; the in-GUI type→Créer→open-row interaction still wants a human/AT-SPI
  confirmation on display.

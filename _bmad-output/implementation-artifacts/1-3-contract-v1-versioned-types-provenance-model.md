# Story 1.3: contract v1 — versioned types & provenance model

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->
<!-- Epic 1: Proven SSG core & data foundation (headless). Depends on Stories 1.1 (scaffold) & 1.2 (method spec) — both DONE. -->

## Story

As the developer (Guy, solo),
I want versioned serde data-contract types carrying full provenance,
so that `core`, `persistence` and later epics share one vocabulary and never have to migrate the schema just to add provenance.

## Acceptance Criteria

1. **Core contract types defined in `steadyinvest-contract`.** `Study`, `Judgment`, and a `Cell` that carries a value plus the full data-state model: **source** (provider / manual / derived) × **freshness** (current / stale) × **review** (none / to-review / validated) × **coverage** (present / to-fill / not-available-accepted), each independently queryable.
2. **Provenance per asserted fact.** A `Provenance` type carrying `(source, logical_version, timestamp, hash_of_dependencies)` is attached to each `Cell` (and reused by later derived facts), realizing the Foundational Invariant at the type level.
3. **Money as exact decimal serialized as a string.** Monetary/decimal values use `rust_decimal::Decimal` and serialize to/from a **JSON string** (never a JSON number / float), via a `Money` newtype (or `#[serde(with = "rust_decimal::serde::str")]`).
4. **Explicit `schema_version`.** The contract exposes an integer `schema_version` (the serialized-contract version axis), distinct from `core`'s `method_version`. The `Study`/journal blob carries it.
5. **Round-trip property test.** For every public contract type, `parse(serialize(x)) == x` holds (proptest-generated values).
6. **Forward-compatibility by construction.** New/optional fields use `#[serde(default)]`; the journal types do **NOT** use `#[serde(deny_unknown_fields)]` (an older build must tolerate a newer file's extra fields). A test demonstrates that deserializing JSON with an unknown extra field succeeds.
7. **Enums serialize as documented snake_case strings.** `source` = `provider|manual|derived`; `freshness` = `current|stale`; `review` = `none|to_review|validated`; `coverage` = `present|to_fill|not_available_accepted`. Tri-state review is an **enum**, never `0/1/2`. Timestamps are **RFC3339 UTC strings**.
8. **Cardinal-Rule / boundary clean.** `contract` keeps **no** Slint, SQL, or network dependency (serde / rust_decimal / uuid / serde_json only). Gates green: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --all --locked`, `cargo deny check`.

## Tasks / Subtasks

- [x] **Task 1 — Money & timestamp primitives (AC: 3, 7)**
  - [x] Add `contract/src/money.rs`: a `Money(Decimal)` newtype with `Serialize`/`Deserialize` as a decimal **string** (round-trips exactly; rejects NaN/non-decimal). Prefer a `Money` newtype over a bare `#[serde(with=...)]` so the string contract is enforced in one place. Helpers: `Money::new`, `From<Decimal>`, `as_decimal`.
  - [x] Add a `Timestamp(String)` newtype (RFC3339 UTC). Keep `contract` time-dependency-free — the actual clock is injected in `app`/`core` later; the contract only stores the string. Validate format on construction where cheap (optional in v1).
- [x] **Task 2 — Data-state enums + provenance (AC: 1, 2, 7)**
  - [x] Add `contract/src/cell.rs` enums: `Source { Provider, Manual, Derived }`, `Freshness { Current, Stale }`, `Review { None, ToReview, Validated }`, `Coverage { Present, ToFill, NotAvailableAccepted }` — all `#[serde(rename_all = "snake_case")]`, `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]`.
  - [x] Add `contract/src/provenance.rs`: `Provenance { source: Source, logical_version: u64, timestamp: Timestamp, hash_of_dependencies: String }` (hash is a hex SHA-256 string; `String` keeps `contract` hash-lib-free).
  - [x] Define `Cell { value: Option<Money>, source: Source, freshness: Freshness, review: Review, coverage: Coverage, provenance: Provenance }`. `value: None` is a genuine gap — never coerced to 0. (FR17–FR20.)
- [x] **Task 3 — Study & Judgment types (AC: 1, 4)**
  - [x] Add `contract/src/study.rs`: `Study` (id `Uuid`, `journal_id: Uuid`, `security_ticker: String`, `native_currency: String`, the per-year/per-section `Cell`s, judgment inputs, `rationale: Option<String>`, `schema_version`, `created_at: Timestamp`) and `Judgment` snapshot (the judgment inputs that gate the verdict per the method spec §5: estimated high/low EPS, judged avg high/low P/E, selected forecast-low option, current price — as `Cell`/`Money`). Model the SSG sections at a level sufficient for round-trip + persistence; the engine reads these in Story 1.8. Use `#[serde(default)]` on optional/new fields.
- [x] **Task 4 — Versioning (AC: 4)**
  - [x] Add `contract/src/versioning.rs`: `pub const SCHEMA_VERSION: u32 = 1;` (move it out of `lib.rs`). Document the three version axes and that `schema_version` is the serialized-contract axis (≠ `core::METHOD_VERSION`, ≠ SQLite `user_version`).
- [x] **Task 5 — Forward-compat + round-trip tests (AC: 5, 6)**
  - [x] Add `proptest` as a `[dev-dependencies]` (workspace) to `contract`. Write `parse(serialize(x)) == x` property tests for every public type (Money, Cell, enums, Provenance, Study, Judgment), generating valid values (incl. `value: None`, each enum variant, negative/zero/large decimals).
  - [x] Add a forward-compat test: deserialize a JSON object that includes an **unknown extra field** and confirm it succeeds (proves no `deny_unknown_fields`); add a missing-optional-field test (proves `#[serde(default)]`).
  - [x] Add a money-string test: `Money` serializes to a JSON **string** (assert the serialized form contains quotes / is not a bare number) and parses back exactly.
- [x] **Task 6 — Wire & verify (AC: 8)**
  - [x] `contract/src/lib.rs`: declare `pub mod {money, cell, provenance, study, versioning};` re-export the key types; remove the old inline `SCHEMA_VERSION` (now in `versioning`).
  - [x] Confirm `contract` deps remain serde / serde_json / rust_decimal / uuid (+ proptest dev-dep); no Slint/SQL/net.
  - [x] All gates green (fmt, clippy --locked, test --all --locked, cargo deny check).

### Review Findings

_Adversarial code review (Blind Hunter + Edge Case Hunter + Acceptance Auditor), 2026-06-09. Acceptance Auditor: all 8 ACs PASS. 4 patch · 3 defer · 3 dismissed · 0 decision-needed._

- [x] [Review][Patch] **`Money` deserialize silently rounds / accepts non-canonical strings** [contract/src/money.rs] — `Decimal::from_str` **silently rounds** money strings with > 28 significant digits / scale > 28 (silent precision loss for an "exact money" contract) and accepts non-canonical forms (`"1e5"`, `"+1"`, `"-0"`, underscores). Switch to `Decimal::from_str_exact` (errors instead of rounding) **and** reject non-canonical input (`parsed.to_string() != s`). Add tests: reject `"1e5"`, `"+1"`, a scale-29 string, and a bare number embedded in a struct field; accept canonical. (blind+edge, HIGH)
- [x] [Review][Patch] **Round-trip proptest doesn't exercise the real boundaries** [contract/tests/roundtrip.rs] — `money()` caps scale at 0..=10 and only an `i64` mantissa (never high-scale/overflow), never parses adversarial *strings*, and `token()` excludes quotes/backslash/control/unicode → AC5 confidence is weaker than it reads. Add: a Money round-trip-from-canonical-string property + higher scales; broaden `token()` to include JSON-significant chars (escaping coverage); add explicit `roundtrip!` lines for `Timestamp` and `ForecastLowOption` (currently only transitively covered). (blind+edge, MEDIUM)
- [x] [Review][Patch] **Document `Money` value-equality vs scale-preserving serialization** [contract/src/money.rs, provenance.rs] — `Money` derives value-based `Eq/Ord/Hash` (`3.0 == 3`), but `serialize` preserves scale (`"3.0"` ≠ `"3"`). Anything hashing serialized bytes (notably `Provenance.hash_of_dependencies`) must **normalize** first, or it will disagree with `==`. Document this on `Money` and on `hash_of_dependencies`. (blind+edge, MEDIUM)
- [x] [Review][Patch] **Document the validation contract for free-string fields + enum-evolution policy** [contract/src/provenance.rs, study.rs, cell.rs] — `Timestamp`, `hash_of_dependencies`, `native_currency` are unvalidated `String`s with semantic promises (RFC3339 / hex / ISO-4217). Document that producers (app/ingestion) validate at construction and the contract stores the canonical string; and state that **adding an enum variant is a `schema_version` bump** (deliberately no `#[non_exhaustive]`/`#[serde(other)]` — an unknown `Source`/`Review` must fail loudly, not silently fall back). (blind+edge, LOW)
- [x] [Review][Defer] **Unknown enum-value tolerance across versions** [contract] — by design, enum evolution = `schema_version` bump; an older build rejecting a newer file's unknown enum value is the intended (fail-loud) behavior for domain correctness. Deferred (documented as policy in patch #4).
- [x] [Review][Defer] **Runtime validation of `Timestamp` / `native_currency` / `hash_of_dependencies`** [contract or app/ingestion] — add validating constructors / `TryFrom` when the producing layers land (app clock = Story 2.x, ingestion = Epic 3). v1 stores the string (story marked this optional). Deferred.
- [x] [Review][Defer] **Required-field forward-evolution robustness** [contract] — current required fields are intended mandatory-forever; making any optional later needs `#[serde(default)]` + a migration at that time. Deferred to whenever a field's optionality changes.

## Dev Notes

### Why this story matters
`contract` is the **single shared vocabulary** across `core`, `ingestion`, `persistence`, `report`, `app`, and the future read-only MCP/AI façade. Getting provenance + the data-state model into v1 means later epics *fill* these fields rather than migrate the schema to add them. [Source: architecture.md#Architectural Boundaries (Contract boundary), #Cross-Cutting Concerns]

### Architecture constraints (must follow)
- **Realizes the Foundational Invariant at the type level:** every asserted fact carries `(source, logical_version, timestamp, hash_of_dependencies)`. The `Cell`/`Provenance` types are where this lives. [Source: architecture.md#The Foundational Invariant, #Data Architecture]
- **Three version axes — `schema_version` is THIS crate's:** `schema_version` (serialized contract, **integer**) vs SQLite `user_version` (persistence) vs `method_version` (string, in `core`, Story 1.2). Don't conflate. [Source: architecture.md#Core Technical Decisions, #Format Patterns]
- **Format patterns (binding):** serde JSON, **snake_case** field names; `#[serde(default)]` on every new field; **never `deny_unknown_fields`** on the journal (forward-compat); **Decimal serialized as a string**; dates/times **RFC3339 UTC strings**; enums `#[serde(rename_all="snake_case")]`, tri-state review is an enum (never `0/1/2`); booleans as JSON booleans. [Source: architecture.md#Format Patterns]
- **Cell data-state model:** `source (provider/manual/derived) × freshness (current/stale) × review (none/?/✓)` — plus coverage `present/to-fill/not-available-accepted` (FR19). `unknown/insufficient` is first-class; never coerce a missing value to `0`. [Source: architecture.md#Cross-Cutting Concerns; contract/src/cell.rs in the arch tree; prd.md FR17–FR20, FR19]
- **Contract is decoupled from Slint & SQLite** and is the seam a later read-only MCP sits on. No I/O. [Source: architecture.md#Architectural Boundaries]
- Arch-tree files for this crate (create the ones in scope now; portfolio/fx/export are later epics): `study.rs`, `cell.rs`, `provenance.rs`, `versioning.rs` **now**; `portfolio.rs` (Epic 4/6), `fx.rs` (Epic 6), `export.rs` (Epic 5) **deferred** — do NOT build them in 1.3 (keep scope tight; add `#[serde(default)]`-friendly structs when those stories land). [Source: architecture.md#Complete Project Directory Structure]

### Money-as-string: recommended approach
Define a `Money(rust_decimal::Decimal)` newtype with hand-written `Serialize`/`Deserialize` (serialize `self.0.normalize().to_string()`; deserialize via `Decimal::from_str`). This enforces the "string, never float" contract in one place and avoids depending on a specific rust_decimal serde feature. (Alternative: enable rust_decimal's `serde-with-str` and use `#[serde(with = "rust_decimal::serde::str")]` on each field — more places to get wrong.) Either way: **assert in a test** that the JSON form is a quoted string. [Source: architecture.md#Format Patterns "Decimal in JSON: serialized as a string"]

### Previous story intelligence (1.1 & 1.2 — DONE)
- `contract` already exists from 1.1 with `pub const SCHEMA_VERSION: u32 = 1;` in `lib.rs` and deps `serde`, `serde_json`, `rust_decimal`, `uuid` (workspace-pinned). Move `SCHEMA_VERSION` into `versioning.rs`.
- **MSRV 1.96** (toolchain pinned); CI is **Linux-only** for now; CI runs `--locked`. Keep tests deterministic (exact decimal; for timestamps in tests use fixed RFC3339 strings — no wall-clock).
- Exact-decimal idiom: `Decimal::new(mantissa, scale)`, `.normalize()`, `Decimal::from_str`. No `f32`/`f64` anywhere.
- 1.2 added `core::method` constants incl. `LOAD_BEARING_JUDGMENT_INPUTS` (`estimated_high_eps`, `estimated_low_eps`, `judged_avg_high_pe`, `judged_avg_low_pe`, `current_price`) and `LOAD_BEARING_YEAR_FIELDS` (`sales`, `eps`, `high_price`, `low_price`) — the `Study`/`Judgment` field names here should align with those string keys so Story 1.8 can map them. [Source: 1-2 Dev Agent Record; core/src/method/mod.rs]
- Pattern for snapshot/property tests already in repo (`core` determinism + method fingerprint, proptest available workspace-wide).
- Process patterns: no `unwrap`/`expect` in non-test code; per-crate `thiserror` only when real fallible APIs appear (parsing money/timestamp may warrant a small error enum). [Source: architecture.md#Process Patterns]

### Project Structure Notes
- New: `contract/src/{money,cell,provenance,study,versioning}.rs`; modify `contract/src/lib.rs` and `contract/Cargo.toml` (add `proptest` dev-dep; `uuid` already present — ensure `serde`/`v4` features as needed).
- No changes to other crates. Keep the gates green; nothing here should touch `core`/`app`/`persistence`.

### References
- [Source: epics.md#Story 1.3: `contract` v1 — versioned types & provenance model] — user story + AC
- [Source: architecture.md#Format Patterns] — serde snake_case, serde(default), no deny_unknown_fields, Decimal-as-string, RFC3339, enum review (not 0/1/2)
- [Source: architecture.md#The Foundational Invariant / #Data Architecture] — (source, logical_version, timestamp, hash_of_dependencies)
- [Source: architecture.md#Complete Project Directory Structure] — `contract/src/*` file plan (study/cell/provenance/versioning now; portfolio/fx/export later)
- [Source: prd.md FR17–FR20] — per-cell source / provenance+timestamp / coverage / validated(tri-state)
- [Source: core/src/method/mod.rs] — load-bearing field/judgment string keys to align names with

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — claude-opus-4-8 — via Claude Code dev-story (2026-06-09).

### Debug Log References

- `cargo test -p steadyinvest-contract` → 27/27 (14 unit + 13 proptest/integration). `cargo test --all --locked` green (core 14 + contract 27). Gates: fmt ✅ · clippy --locked ✅ · `cargo deny check` ✅.

### Senior Developer Review (AI)

**Outcome:** Approved with fixes applied (2026-06-09). Acceptance Auditor: all 8 ACs PASS. 4 patch (applied), 3 defer, 3 dismissed.
**Patches applied:**
- [HIGH] `Money` deserialize now uses `Decimal::from_str_exact` + a canonical-form guard → rejects silent precision loss (>28 digits/scale) and non-canonical spellings (`1e5`, `+1`, `-0`, underscores, `1.`, `.5`); added 4 rejection tests incl. a nested bare-number case.
- [MED] Strengthened round-trip proptest: `money()` now spans scale 0..=28; `token()` includes JSON-significant chars (quotes/backslash/unicode); added explicit `Timestamp` + `ForecastLowOption` round-trip lines and a Money canonical-idempotence property.
- [MED] Documented `Money` value-equality vs scale-preserving serialization; `Provenance.hash_of_dependencies` must normalize decimals before hashing serialized bytes.
- [LOW] Documented the free-string validation contract (validated by producers) and the enum-evolution policy (variant add = `schema_version` bump; deliberately no `non_exhaustive`/`serde(other)`).
**Deferred:** unknown-enum-value tolerance (fail-loud by design); runtime validation of Timestamp/currency/hash (producing layers); required-field forward-evolution. See `deferred-work.md`.

### Completion Notes List

- **Data-state model (FR17–FR20):** `Cell { value: Option<Money>, source, freshness, review, coverage, provenance }` with enums `Source` (provider/manual/derived), `Freshness` (current/stale), `Review` (none/to_review/validated — tri-state enum, never 0/1/2), `Coverage` (present/to_fill/not_available_accepted), all `#[serde(rename_all="snake_case")]`. Missing value = `None`, never coerced to 0.
- **Provenance (Foundational Invariant at type level):** `Provenance { source, logical_version: u64, timestamp: Timestamp, hash_of_dependencies: String }`; `Timestamp(String)` (RFC3339 UTC) keeps `contract` time-dependency-free; hash as `String` keeps it hash-lib-free.
- **Money-as-string (AC3):** `Money(Decimal)` newtype with hand-written serde → JSON **string** (exact, scale-preserving); tests assert it serializes as a string and **rejects** a bare JSON number and non-decimal strings. No `f32/f64`.
- **Study/Judgment (AC1/4):** `Study` (Uuid id/journal_id, ticker, native_currency, `years: Vec<YearData>`, `Judgment`, `rationale`, `created_at`, `schema_version`); `Judgment` mirrors `core::method::LOAD_BEARING_JUDGMENT_INPUTS`; `YearData` carries the 4 load-bearing cells + optional dividend/PTP/book-value; `ForecastLowOption` enum (a–d). `Study::new(..)` stamps `SCHEMA_VERSION`.
- **Versioning:** `SCHEMA_VERSION = 1` moved to `versioning.rs` with the three-axes doc.
- **Forward-compat (AC6):** `#[serde(default)]` on optional fields; no `deny_unknown_fields` — tests prove an unknown extra field deserializes fine and a missing `value` defaults to `None`.
- **Round-trip (AC5):** proptest `parse(serialize(x)) == x` for all 10 public types (Money, 4 enums, Provenance, Cell, YearData, Judgment, Study), incl. `None` values, every variant, negative/zero/large decimals.
- **Boundary clean (AC8):** `contract` deps = serde / serde_json / rust_decimal / uuid (+ proptest dev-dep). No Slint/SQL/net.
- **Scope:** portfolio/FX/export contract types are deferred to their epics (4/6/5) as planned — not built here.

### File List

**Added:**
- `contract/src/money.rs` (Money newtype, string serde + tests)
- `contract/src/cell.rs` (Source/Freshness/Review/Coverage enums + Cell + tests)
- `contract/src/provenance.rs` (Timestamp + Provenance + test)
- `contract/src/study.rs` (Study/Judgment/YearData/ForecastLowOption + tests)
- `contract/src/versioning.rs` (SCHEMA_VERSION + three-axes doc)
- `contract/tests/roundtrip.rs` (proptest round-trip for all public types)

**Modified:**
- `contract/src/lib.rs` (module declarations + re-exports; removed inline SCHEMA_VERSION)
- `contract/Cargo.toml` (added `proptest` dev-dependency)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (1-3 → in-progress → review)

## Change Log

| Date | Change |
|------|--------|
| 2026-06-09 | Story 1.3 created (ready-for-dev): versioned `contract` types (Cell data-state model + Provenance + Money-as-string + Study/Judgment + schema_version) with round-trip + forward-compat tests. Portfolio/FX/export types deferred to their epics. |
| 2026-06-09 | Story 1.3 implemented: `contract` v1 — Money (string serde), Cell + Source/Freshness/Review/Coverage enums, Provenance + Timestamp, Study/Judgment/YearData/ForecastLowOption, SCHEMA_VERSION. 21 tests (10 proptest round-trip + forward-compat + money-string). Gates green (fmt/clippy/test --all/deny); no Slint/SQL/net. Status → review. |
| 2026-06-09 | Code review: applied all 4 patch findings — `Money` deserialize hardened (`from_str_exact` + canonical guard, rejects silent rounding / non-canonical / nested bare numbers); proptest strengthened (scale 0..=28, escaping, explicit Timestamp/ForecastLowOption, canonical idempotence); documented value-eq-vs-serialization + hash normalization + free-string/enum-evolution policy. 27 tests; gates green. 3 deferred, 3 dismissed. Status → done. |

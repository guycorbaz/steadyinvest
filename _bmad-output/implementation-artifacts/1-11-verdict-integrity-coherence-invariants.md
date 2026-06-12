# Story 1.11: Verdict-integrity & coherence invariants

Status: done

<!-- Epic 1 closer. The trust-invariant story: invariant 2a (a FullVerdict is constructible ONLY
     from all-validated-and-fresh load-bearing inputs — the compiler is the gate) and invariant 2b
     (mutating a load-bearing input flips its review ✓→? AND degrades the dependent verdict in the
     same coherence frame — never one without the other), both on the manual-mutation rail so
     Epic 3's provider refresh later just branches onto it. Headless: NO UI, NO persistence change,
     NO new external dependency, NO METHOD_VERSION / SCHEMA_VERSION bump. -->

## Story

As the developer (Guy, solo),
I want the trust invariants enforced by type and by test,
so that a verdict can never silently outrun the state of its inputs.

## Acceptance Criteria

1. **`core` gains an integrity vocabulary and an immutable computed-study snapshot** (new module
   `core/src/verdict.rs` — the architecture names this file). In core's OWN vocabulary (the
   `[dependencies]` of `steadyinvest-core` stay exactly `rust_decimal + serde + sha2` — core MUST
   NOT depend on `contract`; the contract→core mapping is Epic 2 glue, exercised here only in
   tests):
   - a per-load-bearing-input **gate state** (4 cases — input missing / not validated (review ≠ ✓)
     / stale / validated-and-fresh; names dev discretion, neutral and fact-stating per FR13);
   - a structured **gates collection** covering exactly the spec-§5 catalog: the 4
     `LOAD_BEARING_YEAR_FIELDS` per usable year **plus** the 5 `LOAD_BEARING_JUDGMENT_INPUTS`
     (both catalogs already exist in `core::method` — reuse them, do NOT re-declare names; a test
     asserts the gates type covers each catalog entry, mirroring 1.8's `judgment_field_present`
     exhaustiveness glue). The "usable years" scoping (§5: gates apply to the load-bearing fields
     *of the usable years*) is the caller's mapping duty — document it on the type;
   - an immutable **snapshot type** binding the outputs + the gates, built by ONE constructor in
     one call, fields private, no `&mut` accessor and no setter — once built, neither the outputs
     nor the gates can be swapped, so **no incoherent intermediate frame is representable**
     (verdict and staleness/integrity are both read from this single frame, AC 3 of the epic).
     Normative: the constructor takes the **engine inputs + gates** and calls `ssg::compute()`
     itself — `SsgOutputs` is NEVER caller-supplied (a caller-supplied outputs value would make a
     mismatched outputs/inputs frame representable, and the AC-3 digest needs the inputs anyway).
2. **Invariant 2a — `FullVerdict` is a type-state, the compiler is the gate.** `FullVerdict` has
   private fields, NO public constructor, no `Default`, no `serde` derives (nothing persists a
   verdict in Epic 1 — the frozen decision-time verdict is Epic 2, ADD10). The ONLY way to obtain
   one is the snapshot's single derivation entry point (e.g. `snapshot.verdict() -> &Verdict`,
   derived once at construction — exact shape dev discretion, single entry is normative), where
   `Verdict` is a three-state enum:
   - **Full(FullVerdict)** — iff EVERY gate is validated-and-fresh AND `low_confidence` is false
     (spec §1 + FR12 + UX "Verdict Integrity": low-confidence degrades the verdict too — this is
     spec'd, not an interpretation);
   - **Withheld** — iff any load-bearing input is **missing** (per §5 "missing" wording; with the
     §4 usable-year rule this in practice means a missing judgment input — a year missing a
     load-bearing field is simply not usable);
   - **Provisional** — every other degraded case: no input missing but ≥ 1 not-validated or stale,
     or `low_confidence` (the Withheld/Provisional split is a recorded interpretation — file it).
   Provisional and Withheld carry the `VerdictFacts` (where statable) **plus queryable holds**
   (which inputs, which gate state — FR11/FR12 "testably/queryable"). A **`compile_fail` doctest**
   on `FullVerdict` proves a literal construction attempt outside `core` does not compile.
   **Property tests (proptest)**: over arbitrary gate vectors and `low_confidence` — `Full` ⟺
   (all gates green ∧ ¬low_confidence); any single non-green gate ⇒ never `Full`; derivation is
   deterministic (same snapshot ⇒ same verdict). Explicitly: `Full` is ORTHOGONAL to
   `quality_value_candidate` — Full means *the facts rest on validated, fresh inputs*, NOT that
   the company passes the four criteria; a Full verdict may carry `Unmet`/`UnmetByInsufficiency`
   facts (e.g. a degenerate forecast range with all inputs validated stays Full — the facts state
   the insufficiency queryably). A test pins this. The `compile_fail` doctest must contain a
   SINGLE literal `FullVerdict { … }` construction and nothing else — a `compile_fail` doctest
   passes on ANY compile error, so any extra code risks a false pass for the wrong reason
   (error-code annotations are nightly-only; minimalism is the stable-channel discipline).
3. **The verdicts are content-addressed (ADD9).** The snapshot computes, at construction, a
   deterministic hex `inputs_hash` (SHA-256 — `sha2` is already a core dep) over a documented,
   stable encoding of the engine inputs (`CanonicalFinancials` + `JudgmentInputs` +
   `QuarterlyObservations`; encoding dev discretion — it MUST normalize `Decimal` scale before
   hashing (`Decimal::normalize`), per the `contract/src/provenance.rs` NOTE: value-equal inputs
   must produce equal digests). Every derived `Verdict` (all three states) is stamped with
   `(inputs_hash, METHOD_VERSION)` — `verdict = f(hash(inputs), method_version)`. Property tests:
   equal inputs ⇒ equal digest; changing any load-bearing value ⇒ different digest (orphaning of a
   prior verdict is detectable — invalidation, not silent overwrite).
4. **Invariant 2b — the manual-mutation rail lives in `contract`** (it must be visible to Epic 3's
   `ingestion`, which depends ONLY on `contract`). `Cell` gains an edit rail returning a NEW cell
   (snapshot semantics, never in-place):
   `Cell::edited(&self, new_value: Option<Money>, provenance: Provenance) -> Cell` (exact name dev
   discretion; the semantics are normative):
   - `value` ← `new_value`; `source` ← `provenance.source`; `freshness` ← `Current` (a fresh edit
     is current); `provenance` ← the caller-supplied one (`contract` NEVER calls a clock, UUID or
     hash — the 1.10/ADD15 injected discipline; tests pass fixed values);
   - **review: `Validated` → `ToReview` iff the value actually differs** (`Money` `PartialEq` is
     value-based, so re-entering `"3"` over `"3.0"` is NOT a divergence and keeps ✓); a
     non-divergent edit keeps `Validated` (provenance still updates); `None`/`ToReview` reviews
     are never promoted nor demoted by an edit (validation is an explicit user act, FR20);
   - `coverage`: `Some` value ⇒ `Present`; `None` ⇒ `ToFill` (an explicit clear reopens the gap —
     recorded interpretation);
   - the serde **shape of every contract type is unchanged**: NO field added/removed/renamed, NO
     `SCHEMA_VERSION` bump — the persistence pinned-JSON snapshot and the frozen
     `tests/corpus/v1.db` gate MUST pass byte-identical (run them; if they fail, the rail was
     implemented wrong). Unit + property tests in `contract` (proptest is already a dev-dep):
     divergent edit always demotes ✓; equal-value edit never does; the returned cell leaves the
     original untouched.
5. **Coherence end-to-end — 2b proper, tested on the rail.** A core integration test
   (`core/tests/verdict_coherence.rs`) adds `steadyinvest-contract` to core's
   `[dev-dependencies]` ONLY (the 1.9 precedent: `serde_json` is dev-only there; the shipped
   dependency surface of `core` is unchanged) and contains the Epic-2-preview glue mapping
   (contract `Cell.review`/`Freshness`/`value` → core gate states; judgment inputs: present ⇒
   validated-and-fresh, absent ⇒ missing — judgments are the user's own current assertion and
   carry no review state in contract v1, see issue #14; recorded interpretation):
   - build a `contract::Study` whose load-bearing cells are all `Validated` + `Current` (≥ 5
     usable years) with a complete judgment → map → normalize → compute → snapshot → **`Full`**;
   - mutate ONE load-bearing cell through the rail (divergent value) → rebuild the snapshot
     through the same single path → from that ONE new frame: the cell's review reads `ToReview`
     AND the verdict is NOT `Full` and its holds name that input — **never one without the
     other**, because both are reads of the same immutable frame;
   - the old `Full` verdict still carries the OLD `inputs_hash` ≠ the new snapshot's hash (the
     prior verdict is detectably orphaned, ADD9);
   - a property test varies WHICH load-bearing input is mutated (any of the 4 year fields on any
     usable year, any of the 5 judgment inputs withdrawn) — degradation holds for every choice.
6. **Posture (FR13)**: every new public type/variant name and every user-visible string introduced
   by this story is neutral and fact-stating — a crate-local banned-verb posture test over the new
   `Verdict`/holds vocabulary (the 1.9/1.10 local-gate pattern; reuse
   `core::method::BANNED_VERBS_EN/FR` — core already owns them, no copy needed).
7. **Gates green, method/schema/persistence untouched, interpretations filed**:
   `cargo fmt --all --check` · `cargo clippy --all-targets --all-features --locked -- -D warnings`
   · `cargo test --all --locked` · `cargo deny check` all green; **method fingerprint
   (`f79e3c11…1d1d`), determinism hash (`eb45e761…d34f`), Spike-C digest, `METHOD_VERSION`
   (`ssg-1.0.0`), `SCHEMA_VERSION` (= 1) all UNCHANGED** (the §5 gating rule is already part of
   spec v1 — implementing it is not a method change; no `core::method` constant is added or
   edited); `persistence/**`, `docs/method/**`, `.github/workflows/ci.yml`, `deny.toml` not
   modified; `Cargo.lock` delta = the single dev-dep edge `steadyinvest-core → steadyinvest-contract`
   (both already in the graph — no new external crate). Every spec-underspecified interpretation
   goes to ONE consolidated GitHub issue (repo `guycorbaz/steadyinvest`), never an inline debt
   note.

## Tasks / Subtasks

- [x] **Task 1 — `core/src/verdict.rs`: gates, snapshot, digest (AC: 1, 3)**
  - [x] Gate-state enum (4 cases) + structured gates collection covering exactly
        `LOAD_BEARING_YEAR_FIELDS` (per usable year) + `LOAD_BEARING_JUDGMENT_INPUTS`; catalog
        coverage asserted by test against `core::method`'s arrays.
  - [x] Immutable snapshot type: ONE constructor taking the engine inputs + gates; private fields;
        computes `inputs_hash` (sha2, `Decimal::normalize` before encoding) and derives the
        `Verdict` at construction; accessors only (`outputs()`, `gates()`, `verdict()`,
        `inputs_hash()`, `method_version()`).
  - [x] Wire the module into `core/src/lib.rs` (module + re-exports, doc update — lib.rs currently
        says "the integrity-gated verdict arrive[s] in later Epic 1 stories (1.9/1.11)").
  - [x] Digest property tests: equal inputs ⇒ equal digest; scale-normalization (`3.0` vs `3`
        inputs ⇒ same digest); any load-bearing value change ⇒ different digest.
- [x] **Task 2 — Invariant 2a: `FullVerdict` type-state + `Verdict` enum (AC: 2, 6)**
  - [x] `FullVerdict` (private fields, no public ctor, no Default, no serde) + `Verdict`
        {Full/Provisional/Withheld} with queryable holds; stamps `(inputs_hash, METHOD_VERSION)`.
  - [x] Withheld ⟺ any gate missing; Provisional ⟺ degraded-but-nothing-missing or
        `low_confidence`; Full ⟺ all green ∧ ¬low_confidence — document the split as a recorded
        interpretation.
  - [x] `compile_fail` doctest proving out-of-crate construction is impossible.
  - [x] proptest: gate-vector × low_confidence ⟺ Full equivalence; single non-green ⇒ never Full;
        determinism; Full-is-orthogonal-to-`quality_value_candidate` pin (incl. the
        degenerate-forecast-range-stays-Full case).
  - [x] Crate-local banned-verb posture test over the new public vocabulary (reuse
        `BANNED_VERBS_EN/FR`).
- [x] **Task 3 — Invariant 2b rail: `contract::Cell` edit API (AC: 4)**
  - [x] `Cell::edited(...)` per the normative semantics (divergence-only ✓→? demotion via `Money`
        value equality; freshness ← Current; caller-supplied provenance; coverage Present/ToFill).
  - [x] Unit + proptest coverage in `contract` (demote-on-divergence, keep-on-equal incl.
        `"3"` vs `"3.0"`, never-promote, original untouched, provenance replaced verbatim).
  - [x] Verify NO serde-shape change: run the persistence pinned-snapshot + corpus gates untouched
        (`cargo test -p steadyinvest-persistence --locked`).
- [x] **Task 4 — End-to-end coherence test on the rail (AC: 5)**
  - [x] Add `steadyinvest-contract` to `core/[dev-dependencies]` with a comment mirroring the 1.9
        `serde_json` precedent (test glue ONLY; shipped surface unchanged).
  - [x] `core/tests/verdict_coherence.rs`: glue mapping (documented as the Epic-2 preview), the
        all-green ⇒ Full case, the mutate-one-cell ⇒ same-frame (ToReview ∧ ¬Full ∧ hold names the
        input) case, the old-hash ≠ new-hash orphaning case.
  - [x] proptest over WHICH input is mutated (every year field × usable years, every judgment
        input) — degradation holds for all.
- [x] **Task 5 — Gates, issue & status (AC: 7)**
  - [x] All four gates green `--locked`; fingerprint / determinism hash / Spike-C digest /
        `METHOD_VERSION` / `SCHEMA_VERSION` byte-identical; `Cargo.lock` delta = the one dev-dep
        edge.
  - [x] File ONE consolidated GitHub issue "Story 1.11 verdict-integrity interpretations"
        (candidate contents in Dev Notes below); update `sprint-status.yaml`
        (1-11 → review path) and this story's Dev Agent Record / File List.

## Dev Notes

### What this story is — and the disaster it must make impossible

The product's core promise is "no silent wrong signal". The one place that promise can structurally
break is a verdict computed over a *mix* of cell states — a fresh-looking zone verdict sitting on an
edited, unvalidated or stale input. 1.11 makes that **unrepresentable**: (2a) the compiler refuses a
`FullVerdict` unless every load-bearing input is ✓ and fresh; (2b) the only way to mutate a cell
demotes its ✓ in the same frame the verdict derives from. Epic 1 closes headless with this story —
Epic 2 renders these states (verdict badge full/provisional/degraded/withheld), Epic 3's
reconciliation branches onto the same rail (divergence → auto-?). Get the shape wrong here and both
epics inherit the flaw. [Source: epics.md#Story 1.11; epics.md#Epic 1 "Includes:";
ux-design-specification.md#Verdict Integrity]

### Where the pieces live — the dependency walls decide the design

The workspace's dependency walls are absolute and they dictate the split (verified in the Cargo
manifests 2026-06-12):

- `core` deps = `rust_decimal + serde + sha2` — **no `contract`**. The architecture's directory
  spec confirms: `core/Cargo.toml # deps: rust_decimal (+maths), serde (types only)`. So
  `core/src/verdict.rs` (the architecture names this exact file for "FullVerdict — constructible
  only from validated+fresh inputs") must define its OWN gate vocabulary; it cannot import
  `contract::{Review, Freshness}`.
- `contract` deps = `serde + serde_json + rust_decimal + uuid` — no `core`. Which fields are
  load-bearing is METHOD knowledge (`core::method`), so the contract rail must be
  **load-bearing-agnostic**: `Cell::edited` applies to ANY cell; load-bearing selection happens
  only at the verdict gate in core.
- Epic 3's `ingestion` depends ONLY on `contract` (architecture: `reconcile.rs # non-destructive:
  manual wins, provider preserved, divergence→?`). That is WHY the rail must live in `contract` —
  it is the shared mutation primitive both the manual path (Epic 2 UI) and the provider path
  (Epic 3) go through. The epic says it verbatim: "the coherence-frame invariant (2b) defined on
  the manual-mutation rail so Epic 3's refresh just branches onto it."
- The bridge (contract `Study` → core gates) is Epic 2 production glue (`app/state.rs` owns the
  `StudyState` snapshot per the architecture). 1.11 exercises it ONLY in core's integration tests
  via a **dev-dependency** — the exact pattern 1.9 used for `serde_json` ("Golden-fixture parsing
  in tests ONLY — the shipped engine's dependency surface stays rust_decimal + serde + sha2").
  Do not promote it to a real dependency; do not put the glue in shipped code.
[Source: core/Cargo.toml; contract/Cargo.toml; architecture.md#Complete Project Directory
Structure; architecture.md#Architectural Boundaries; epics.md#Epic 1 "Includes:"]

### The structural trick: derive, never store

2b's "never one without the other" is NOT implemented as two coordinated writes — it falls out of
the architecture's state model: *"a single immutable study-state snapshot is the source of truth …
recompute is transactional and pure (inputs + verdict born together); the verdict is
content-addressed `f(hash(inputs), method_version)`; an input change invalidates the dependent
verdict (marked stale) rather than silently overwriting it."* The verdict is **derived from the
snapshot at construction and lives nowhere else**. Mutating a cell produces a NEW `Study` value
(the rail returns a new `Cell`), the new snapshot derives a new verdict, and the old `FullVerdict`
remains bound to the old `inputs_hash` — orphaned, detectably. There is no intermediate frame where
the input changed but the verdict didn't, because there is no stored verdict to lag.

The epic AC's "same transaction/coherence frame" maps to **the snapshot**, not a SQLite
transaction, in this headless story: nothing persists a verdict in Epic 1 (1.10 explicitly scoped
"No verdict persistence — the frozen decision-time verdict is an Epic 2 feature; the
`method_version` column merely reserves its seat"). When Epic 2 freezes a decision-time verdict,
the 1.10 transactional rail (logical_version bump in the same `rusqlite` transaction) is what it
will ride — do NOT touch persistence now. Record this transaction-=-snapshot interpretation in the
issue. [Source: architecture.md#State management = immutable snapshots;
1-10-persistence…md#Scope boundaries; epics.md#ADD9, #ADD10]

### The §5 catalog — what gates the verdict (verbatim oracle)

Method spec §5 (the authoritative oracle, already versioned in `ssg-1.0.0`): *"The verdict is
degraded or withheld when any load-bearing input is missing, not validated (review ≠ ✓), or stale.
Load-bearing inputs for the verdict: the per-year `sales`, `eps`, `high_price`, `low_price` of the
usable years; the judgment inputs that determine the zones: `estimated_high_eps`,
`estimated_low_eps`, `judged_avg_high_pe`, `judged_avg_low_pe` (or the selected forecast-low
option), and `current_price`. The `FullVerdict` type (Story 1.11) is constructible only when every
load-bearing input is ✓ and not stale."*

Both catalogs are ALREADY typed constants — reuse, never re-declare:
`core::method::LOAD_BEARING_YEAR_FIELDS` (`["sales","eps","high_price","low_price"]`) and
`core::method::LOAD_BEARING_JUDGMENT_INPUTS` (`["estimated_high_eps","estimated_low_eps",
"judged_avg_high_pe","judged_avg_low_pe","current_price"]`) — they feed the method fingerprint, so
re-declaring or editing them would break the fingerprint gate (AC 7 forbids it).

**Low-confidence also degrades.** Spec §1: "The verdict is degraded/withheld when a load-bearing
input is unvalidated **or the study is low-confidence** (FR12)". UX spec: "if a load-bearing input
is unvalidated or the study is low-confidence, the verdict shows degraded/withheld". So `Full`
requires `¬low_confidence` — `SsgOutputs.low_confidence` already exists (FR8, `usable_years <
USABLE_YEARS_FLOOR`). The low-confidence hold must stay queryable and distinct from the gate holds
(FR8 "queryable low-confidence state"). [Source: docs/method/ssg-method-spec-v1.md#§1, §4, §5;
core/src/method/mod.rs:26-35; ux-design-specification.md (line ~427)]

**The option-dependent extras can't be gated yet.** §5's "(or the selected forecast-low option)"
implies option-specific inputs (option c's `recent_severe_low`, option d's dividend) are
load-bearing when selected — but `contract::Judgment` cannot even carry them (open issue #14, an
Epic 2 contract change). v1 gates exactly the 5-entry catalog; record the deferral in the issue,
cross-referencing #14. [Source: GitHub issue #14; contract/src/study.rs#Judgment]

### Verdict states pinned (record as interpretations)

- **Withheld** ⟺ ≥ 1 load-bearing input **missing**. With §4's usable-year rule (a year missing a
  load-bearing field is not usable at all), Missing in practice fires on judgment inputs; the
  gates type still represents it for year fields (defense in depth — the glue decides usability).
- **Provisional** ⟺ nothing missing but ≥ 1 not-validated or stale, OR `low_confidence`.
- **Full** ⟺ all gates green ∧ ¬low_confidence. **Full ≠ "good company"**: a Full verdict may
  carry `Unmet` / `UnmetByInsufficiency` criteria facts — `VerdictFacts.quality_value_candidate`
  is orthogonal to integrity. A degenerate forecast range (`forecast_high ≤ forecast_low`) with
  all-validated inputs stays **Full** — the facts state the insufficiency queryably
  (`CriterionFact::UnmetByInsufficiency`, zone `None`); integrity gating is about TRUST in the
  inputs, not about computability of every number. Do not over-block.
- Epic 2's UI vocabulary is "full / provisional / degraded / withheld" (verdict_badge.slint) —
  core ships three states + queryable holds; "degraded" is a rendering of Provisional's holds, not
  a fourth core state. Note it in the issue so Epic 2 isn't surprised.
[Source: docs/method/ssg-method-spec-v1.md#§5, §9; core/src/ssg/types.rs#CriterionFact,
#VerdictFacts; architecture.md#components verdict_badge.slint]

### The rail semantics — why divergence-only demotion (2b fine print)

`Money` equality is **value-based** (`Money("3.0") == Money("3")` — proven and relied on by 1.10's
scale tests), so "the value actually differs" is exactly `Money` `PartialEq` on
`Option<Money>` — re-entering an equal value (or a provider refresh returning the same number) is
NOT a divergence and keeps ✓. This is precisely the Epic 3 reconcile rule the architecture states
("divergence → auto-?") — implementing demote-on-any-write would wrongly strip ✓ on every
non-divergent annual refresh. Never promote review on edit (validation is an explicit user act —
FR20 tri-state; the soft-lock *warning* before editing a ✓ cell is Epic 2 UI, not contract's
business). `contract` performs no clock/UUID/hash calls — `Provenance` arrives whole from the
caller (the established ADD15 discipline: 1.10's persistence takes identity/time as parameters;
`provenance.rs` documents "the producing layers validate at construction"). Adding `impl` methods
changes NO serde shape — the persistence pinned-JSON snapshot and frozen `corpus/v1.db` must pass
untouched, and running them is part of Task 3 (if they break, the rail accidentally changed a
persisted struct → wrong implementation). [Source: contract/src/money.rs (value-based Eq);
1-10-persistence…md#AC-3, #Identity & time are inputs; architecture.md#Non-destructive
reconciliation; contract/src/provenance.rs]

### Content-addressing — the digest (ADD9, kept proportionate)

ADD9: every asserted fact carries `(source, logical_version, timestamp, hash_of_dependencies)`;
content-addressed verdict `f(hash(inputs), method_version)`; `provenance.rs` already assigns the
digest duty to "the recompute path" — which is core. v1 scope: ONE deterministic SHA-256 hex digest
over the snapshot's engine inputs, computed inside the snapshot constructor (`sha2` already powers
`core::determinism_hash` and the method fingerprint — same tools, same style). The encoding is dev
discretion but MUST (a) be stable across runs/platforms (no HashMap iteration order, no
pointer-derived anything), (b) `Decimal::normalize()` every value first (the provenance.rs NOTE:
scale-preserving serialization means `"3.0"` ≠ `"3"` as strings while equal as values — value-equal
inputs must digest equal), (c) cover all three engine inputs (financials, judgment, observations).
Do NOT reach for serde serialization of `CanonicalFinancials` (it isn't `Serialize`, and adding
derives to engine types for hashing's sake widens the surface — a manual field-ordered encoder in
`verdict.rs` is bounded and explicit). Stamping `METHOD_VERSION` is one constant read
(`core::METHOD_VERSION`, "ssg-1.0.0"). Wiring this digest into per-cell
`Provenance.hash_of_dependencies` is the PRODUCERS' job in later epics — out of scope here.
[Source: epics.md#ADD9; contract/src/provenance.rs#NOTE; core/src/lib.rs#determinism_hash;
core/src/method_version.rs]

### What already exists — reuse, don't reinvent

- `core::ssg::compute(financials, judgment, observations) -> SsgOutputs` — the pure engine (1.8).
  `SsgOutputs` already carries `low_confidence: bool` and `verdict_facts: VerdictFacts`. The
  `VerdictFacts` doc says it itself: "No recommendation — the integrity-gated `FullVerdict`
  (validation/freshness) is Story 1.11, not here." 1.11 WRAPS these; it does not recompute or
  duplicate any criterion. [Source: core/src/ssg/mod.rs:48; core/src/ssg/types.rs:227-253]
- `CriterionFact::{Met, Unmet, UnmetByInsufficiency}` — the tri-valued fact pattern to imitate for
  holds (queryably distinct causes, never a silent collapse).
- `contract::Cell { value: Option<Money>, source, freshness, review, coverage, provenance }` with
  `Review::{None, ToReview, Validated}`, `Freshness::{Current, Stale}` — the rail's home; all
  enums `snake_case` on the wire; `value: None` is first-class unknown, never 0.
  [Source: contract/src/cell.rs]
- `contract::Study/YearData/Judgment` — `YearData`'s doc: "The four load-bearing fields (`sales`,
  `eps`, `high_price`, `low_price`) make the year usable"; `Judgment`'s doc: "Names mirror
  `core::method::LOAD_BEARING_JUDGMENT_INPUTS`". The e2e glue maps these by name.
  [Source: contract/src/study.rs]
- `core::normalize(raw: RawFinancials) -> Result<CanonicalFinancials, NormalizeError>` (1.7) —
  the e2e test feeds the engine through the real path (raw → canonical → compute), not hand-built
  canonical values; the caller resolves the `Result` before computing (1.8's documented contract).
  For the 2a property tests, `low_confidence` is DERIVED (`usable_years < USABLE_YEARS_FLOOR`),
  not injectable — drive it through `CanonicalFinancials.usable_years` (a pub field, so
  hand-constructible there) or year count.
- Glue conversions the e2e test will need: contract `Judgment` carries `Option<Money>` while core
  `JudgmentInputs` wants `Option<Decimal>` — use `Money::as_decimal()` (contract/src/money.rs).
  Contract `Study` carries NO quarterly cells — use `QuarterlyObservations::empty()`; harmless for
  `Full`, since observations are not in the §5 gate catalog and Full is orthogonal to the criteria
  facts (don't hunt for quarterly cells that don't exist).
- Posture tooling: `core::method::BANNED_VERBS_EN/FR` + the crate-local posture-test pattern
  (1.9 in `core::golden`, 1.10 in `persistence::error`) — same recipe over the new vocabulary.
- proptest 1.x is already a dev-dep of BOTH core and contract; existing property suites to imitate:
  `core/tests/ssg_metamorphic.rs`, `contract/tests/roundtrip.rs`.

### Scope boundaries — what 1.11 does NOT do

- **No persistence change** — no verdict persistence (Epic 2, ADD10 frozen verdict), no schema/
  migration/corpus change, `persistence/**` untouched.
- **No contract serde-shape change** — methods only; `SCHEMA_VERSION` stays 1; pinned snapshot +
  corpus stay byte-identical. The `Judgment` review-state gap is issue #14 (Epic 2), NOT this
  story.
- **No `core::method` constant change** — fingerprint `f79e3c11…1d1d` and `METHOD_VERSION`
  `ssg-1.0.0` unchanged (§5 gating is already in spec v1; implementing it is not a method change).
  No edit to `docs/method/**`.
- **No UI** (verdict badge / trust markers / soft-lock warning → Epic 2), **no provider rail**
  (reconcile divergence→? → Epic 3 — it will CALL the 2b rail, which is why the rail is
  load-bearing-agnostic and divergence-keyed), **no undo/redo stack** (Epic 2 `app/state.rs`),
  **no Study-level mutation convenience API** unless the dev finds it trivially useful (the Cell
  rail is the normative primitive).
- **No new external dependency, no CI workflow edit** — the tests ride `cargo test --all
  --locked`.
- **No FR29 cause-distinction machinery** (recompute-cause taxonomy lands with the app shell).

### Previous story intelligence (1.10 dev record + review)

- Gates always `--locked`; clippy `-D warnings` covers `--all-targets` — integration tests and
  helpers are linted (doctests are NOT clippy targets — they run under `cargo test`, which is
  what AC 2's `compile_fail` proof rides on).
- Self-explaining asserts are house style: a failing invariant test should TELL the developer
  which invariant broke and why it matters (1.10's snapshot gate names the ritual; do the same —
  e.g. "invariant 2a violated: FullVerdict derived while <input> is <state>").
- Issues, not inline notes: 1.7→#12, 1.8→#13/#14, 1.9→#15, 1.10→#16 — 1.11 files its own
  consolidated issue. Candidate contents: transaction-=-snapshot mapping; Withheld/Provisional
  split; judgment-present⇒green glue rule (cross-ref #14); option-dependent load-bearing extras
  deferred (cross-ref #14); coverage None⇒ToFill on clear; digest encoding choice; Full-with-
  degenerate-range orthogonality; "degraded" = UI rendering of Provisional (Epic 2 note).
- MSRV 1.96 (`rust-toolchain.toml`; the architecture's "1.88" is stale). CI is Linux-only
  (decision 2026-06-09); the determinism-hash test remains the cross-OS contract.
- 1.10's review lesson: an API that mutates before validating its preconditions is a finding
  (`Journal::open` mutated a foreign file) — the snapshot constructor should validate/derive
  BEFORE exposing anything, and the rail must not half-apply.
- `Journal` needed `#[derive(Debug)]` for test ergonomics — give the new core/contract types
  `Debug` (+ `Clone`/`PartialEq` where tests compare them) from the start.

### Git intelligence

The last five commits (`0350bb9`, `4c8f5fc`, `4ff42e0`, `a14a245`, `47fcf6b`) show the rhythm: one
story = one `feat(story-1.N): …` commit touching its crate(s) + tests + sprint-status + story
file. Test conventions to match: `d("…")` Decimal helpers, builder fns for fixtures
(`RawYear::empty`, `JudgmentInputs::empty` exist for exactly this), self-explaining assert
messages, property tests in dedicated `tests/*.rs` files. `core` has been frozen since 1.9
(1.10 pinned its fingerprint) — 1.11 is the story that intentionally reopens `core` (additive
module only) and touches `contract` for the first time since 1.3 (methods only).

### Project Structure Notes

- **New:** `core/src/verdict.rs` (the architecture names this file); `core/tests/verdict_coherence.rs`;
  optionally `core/tests/verdict_properties.rs` (or in-module proptest — dev discretion).
- **Modified:** `core/src/lib.rs` (add `pub mod verdict;` + re-exports + doc line);
  `core/Cargo.toml` (`[dev-dependencies] steadyinvest-contract = { workspace = true }` with the
  1.9-style comment); `contract/src/cell.rs` (the rail `impl` + tests); `Cargo.lock` (one
  workspace-internal dev edge); `_bmad-output/implementation-artifacts/sprint-status.yaml`.
- **Do NOT modify:** `persistence/**`, `docs/method/**`, `core/src/method/**` (fingerprint!),
  `core/src/ssg/**` (engine is done; the verdict module wraps it), `contract` serde shapes,
  `.github/workflows/ci.yml`, `deny.toml`, `rust-toolchain.toml`.
- **Naming:** types `PascalCase`, modules `snake_case`, no `utils.rs`; neutral fact-stating
  variant names (posture-gated); workspace-internal dep refs via `{ workspace = true }`
  (`steadyinvest-contract` is already in `[workspace.dependencies]`).

### References

- [Source: epics.md#Story 1.11] — user story + the three ACs (2a compiler-enforced, 2b same-frame
  ✓→? + degradation on the manual-mutation rail, single immutable snapshot)
- [Source: epics.md#Epic 1 "Includes:"] — "the static verdict-integrity invariant (2a) and the
  coherence-frame invariant (2b) defined on the manual-mutation rail so Epic 3's refresh just
  branches onto it"; Epic 1 closes headless
- [Source: epics.md#ADD9/ADD10/ADD15] — Foundational Invariant by construction (provenance
  4-tuple, transactional recompute, content-addressed verdict, invalidation not overwrite);
  frozen decision-time verdict is Epic 2; injected Clock/IdGen discipline
- [Source: prd.md#FR8/FR11/FR12/FR13/FR17-FR20/FR29] — queryable low-confidence; traceability;
  testably degraded/withheld verdict; neutral posture; cell state axes; deterministic recompute
- [Source: docs/method/ssg-method-spec-v1.md#§1-verdict, §4, §5, §6] — degraded/withheld rule
  (incl. low-confidence), usable-year rule, the load-bearing catalog, banned verbs; spec v1
  ALREADY contains §5 ⇒ no METHOD_VERSION bump
- [Source: architecture.md#Trust Invariants as Type Properties] — "a FullVerdict is constructible
  only from all-validated-and-fresh load-bearing inputs (compiler is the gate); verdict +
  staleness derive from the SAME immutable state snapshot"
- [Source: architecture.md#State management = immutable snapshots] — derive-don't-store; undo =
  snapshot stack (Epic 2)
- [Source: architecture.md#Complete Project Directory Structure] — `core/verdict.rs`; core deps
  "rust_decimal (+maths), serde (types only)"; ingestion deps contract-only; app/state.rs owns the
  Epic-2 StudyState
- [Source: architecture.md#Anti-patterns] — "A verdict in full colour while a load-bearing input
  is unvalidated/stale" is the named forbidden frame
- [Source: ux-design-specification.md#Verdict Integrity / State & Trust Markers] — full colour
  only when every load-bearing input ✓ and not stale; tri-state markers; conscious-override is
  traced (Epic 2)
- [Source: core/src/{method/mod.rs, method_version.rs, ssg/mod.rs, ssg/types.rs, lib.rs}] — the
  catalogs, METHOD_VERSION, compute(), VerdictFacts ("FullVerdict … is Story 1.11, not here"),
  sha2 precedent (verified in code 2026-06-12)
- [Source: contract/src/{cell.rs, study.rs, money.rs, provenance.rs, versioning.rs}] — Cell axes,
  load-bearing field alignment docs, value-based Money equality, provenance digest NOTE,
  SCHEMA_VERSION = 1 (verified in code 2026-06-12)
- [Source: 1-10-persistence-v1-…md] — predecessor patterns (no verdict persistence; caller-supplied
  identity/time; pinned snapshot + corpus gates; `--locked`; issues #12–#16)
- [Source: GitHub issues #13/#14] — method interpretations queue; Judgment contract gap (Epic 2)

### Tech currency note (2026-06-12)

This story introduces **zero new external dependencies**: `sha2` 0.11 (digest), `proptest` 1.x
(property tests), `rust_decimal` 1.42 (`normalize()` for scale-stripping before hashing) and
`serde` 1 are all pinned workspace deps already exercised by core/contract since 1.1–1.9. The only
Cargo change is the workspace-internal dev edge `steadyinvest-core → steadyinvest-contract`. No
version research required; the `Decimal::normalize` API and `compile_fail` doctest attribute are
stable Rust/rust_decimal features well within MSRV 1.96.

## Dev Agent Record

### Agent Model Used

claude-fable-5 (Claude Code)

### Debug Log References

- Baseline `cargo test --all --locked` green before any change (2026-06-12).
- One `cargo fmt` pass needed after the contract rail tests (line-length reflow only).
- `Cargo.lock` updated via `cargo update --offline --workspace`; delta verified =
  exactly one line (`steadyinvest-contract` under `steadyinvest-core`'s dependencies entry).

### Completion Notes List

- **Task 1 — `core/src/verdict.rs`.** `GateState` (Missing / NotValidated / Stale /
  ValidatedFresh), `YearGates`/`InputGates` with array lengths tied mechanically to
  `LOAD_BEARING_YEAR_FIELDS.len()` / `LOAD_BEARING_JUDGMENT_INPUTS.len()` (re-declaring nothing —
  the catalogs feed the fingerprint, which is pinned and unchanged). `StudySnapshot::new` is the
  single construction path: computes `inputs_hash` (SHA-256, `Decimal::normalize` before
  encoding, documented line-based field-ordered encoding), calls `ssg::compute()` itself
  (`SsgOutputs` never caller-supplied) and derives the `Verdict` before exposing anything
  (the 1.10 validate-before-expose lesson). Usable-years scoping documented on `InputGates`
  as the caller's duty. Catalog-coverage test mirrors 1.8's `judgment_field_present` glue.
- **Task 2 — invariant 2a.** `FullVerdict`: private fields, no public ctor, no `Default`, no
  serde; the only path is `snapshot.verdict()`. `Verdict::{Full, Provisional(DegradedVerdict),
  Withheld(DegradedVerdict)}` all stamped `(inputs_hash, METHOD_VERSION)`; degraded states carry
  the facts + queryable `open_gates()` (which input, which state) + `low_confidence()` distinct
  from the gate evidence (FR8). `compile_fail` doctest = a single literal construction (plus a
  separate plain doctest pinning the path is valid, so the compile failure is for the right
  reason). proptest: full equivalence Full ⟺ all-green ∧ ¬low_confidence over arbitrary gate
  vectors; single non-green ⇒ never Full (and named); determinism; Full-orthogonal-to-
  `quality_value_candidate` pin incl. degenerate-forecast-range-stays-Full. Posture test over
  the 40-entry new public vocabulary (note: the natural name `Hold` for the evidence type was
  rejected because "hold" is itself a banned verb — renamed `OpenGate`; the gate vetted its own
  vocabulary).
- **Task 3 — invariant 2b rail.** `contract::Cell::edited(&self, Option<Money>, Provenance) ->
  Cell`: snapshot semantics (new cell, original untouched); ✓→? iff value diverges (value-based
  `Money` equality, `"3"` vs `"3.0"` is NOT a divergence); never promotes None/ToReview (FR20);
  freshness ← Current; source ← provenance.source; coverage Present/ToFill; no clock/UUID/hash
  call (ADD15). Unit tests + one rail-semantics proptest over every cell state × a small value
  space (collisions wanted, to exercise the equal-value branch). Serde shape unchanged: methods
  only — `cargo test -p steadyinvest-persistence --locked` (pinned JSON snapshot + frozen
  `tests/corpus/v1.db`) passes byte-identical.
- **Task 4 — e2e coherence.** `steadyinvest-contract` added to core `[dev-dependencies]` ONLY
  (1.9 `serde_json` precedent, comment included). `core/tests/verdict_coherence.rs` documents
  the Epic-2-preview glue (Cell → GateState precedence missing > not-validated > stale;
  judgment present ⇒ green per issue #14; §4 usability scoping) and tests: all-green study →
  Full through the REAL path (study → raw → normalize → compute → snapshot); one divergent rail
  edit → ONE new frame where review reads ? AND the verdict is Provisional naming exactly that
  input; old Full verdict keeps the old `inputs_hash` ≠ new hash (ADD9 orphaning); judgment
  withdrawal → Withheld naming the input; proptest over ALL 25 load-bearing choices (4 fields ×
  5 usable years + 5 judgment inputs) — degradation holds for every choice. The base `Study` is
  built via `serde_json` (existing core dev-dep) precisely so `core` needs no `uuid` dev-edge —
  keeping the Cargo.lock delta to the single mandated edge.
- **Task 5 — gates & bookkeeping.** `cargo fmt --all --check` · `cargo clippy --all-targets
  --all-features --locked -- -D warnings` · `cargo test --all --locked` (26 suites, 0 failures)
  · `cargo deny check` all green. Method fingerprint `f79e3c11…1d1d`, determinism hash
  `eb45e761…d34f`, Spike-C suite, `METHOD_VERSION` (`ssg-1.0.0`), `SCHEMA_VERSION` (= 1) all
  unchanged (pinned tests pass; `core/src/method/**`, `core/src/ssg/**`, `persistence/**`,
  `docs/method/**`, `.github/workflows/ci.yml`, `deny.toml` untouched). Consolidated
  interpretations filed as GitHub issue **#17** (transaction=snapshot; Withheld/Provisional
  split; judgment-present⇒green + gate-precedence glue; option-dependent extras deferred
  (cross-ref #14); coverage None⇒ToFill; digest encoding; Full/degenerate-range orthogonality;
  "degraded" = Epic 2 rendering of Provisional).

### File List

- `core/src/verdict.rs` (new) — gates, snapshot, digest, type-state verdict + unit/posture tests
- `core/tests/verdict_properties.rs` (new) — 2a equivalence, determinism, digest property tests
- `core/tests/verdict_coherence.rs` (new) — 2b end-to-end on the rail (contract dev-dep glue)
- `core/src/lib.rs` (modified) — `pub mod verdict` + re-exports + crate-doc update
- `core/Cargo.toml` (modified) — `[dev-dependencies] steadyinvest-contract` (test glue only)
- `contract/src/cell.rs` (modified) — `Cell::edited` rail + unit/property tests
- `Cargo.lock` (modified) — the single workspace-internal dev edge core → contract
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified) — 1-11 → review
- `_bmad-output/implementation-artifacts/1-11-verdict-integrity-coherence-invariants.md` (modified)
- `_bmad-output/implementation-artifacts/tests/test-summary.md` (modified) — regenerated by the
  qa-generate-e2e-tests workflow (story 1.11 e2e additions in `verdict_coherence.rs`)
- `_bmad-output/story-automator/orchestration-1-20260610-220012.md` (modified) — automator session log

## Senior Developer Review (AI)

**Reviewer:** Guy (autonomous story-automator review) — 2026-06-12
**Outcome:** Approve (status → done). 0 Critical, 0 High, 1 Medium, 3 Low.

**Verified against the implementation (not taken on faith):**

- All 7 ACs implemented; every task marked [x] has matching code/test evidence. Spot-checked:
  `FullVerdict` has private fields / no public ctor / no `Default` / no serde derives; the
  `compile_fail` doctest is a single literal construction and PASSES alongside the
  path-validity doctest (`cargo test -p steadyinvest-core --doc`: 2 passed); the rail's
  divergence-only ✓→? demotion rides value-based `Money` equality (`contract/src/money.rs`
  derives via `Decimal`); `ssg::compute` reads per-year `usability` but `YearUsability` is
  `Usable | Insufficient{missing}` ⟺ a pure function of the four encoded load-bearing values,
  and `financials.findings` is never read by `compute` — so the digest's documented exclusions
  are sound.
- Gates re-run during review: `cargo fmt --all --check` · `cargo clippy --all-targets
  --all-features --locked -- -D warnings` · `cargo test --all --locked` (26 suites, 0 failures;
  the single ignored test is the pre-existing one-shot corpus generator) · `cargo deny check`
  — all green. Method fingerprint (`f79e3c11…1d1d`) and determinism hash (`eb45e761…d34f`)
  constants pinned in passing tests; `git diff HEAD` over `core/src/method/`, `core/src/ssg/`,
  `persistence/`, `docs/method/`, `.github/workflows/ci.yml`, `deny.toml`,
  `rust-toolchain.toml` is empty. `Cargo.lock` delta = exactly the one dev edge
  `steadyinvest-core → steadyinvest-contract`. GitHub issue #17 exists (OPEN) with the
  announced interpretations. `[dependencies]` of core unchanged (`rust_decimal + serde + sha2`).

**Findings & resolutions (all fixed in this review pass):**

1. **[Medium][Docs]** File List omitted two git-modified artifacts:
   `tests/test-summary.md` (regenerated by the QA e2e workflow) and the automator
   orchestration log. → Added to File List.
2. **[Low][Test]** FR13 posture vocabulary (AC 6) omitted the public struct-variant field
   names `field` / `name` of `GatedInput`. → Added (list is now 42 entries); test green.
3. **[Low][Docs]** `InputGates::new` silently accepts duplicate-year `YearGates` entries
   (`year_gate()` reads the first, `open_gates()` reports all). → Caller duty documented on
   the constructor.

**Noted, no action (conform to AC text):** `any_single_non_green_gate_is_never_full` samples
75 (slot × state) combos with 256 random cases (~2–3 combos statistically unvisited per run);
AC 2 prescribes proptest and the general equivalence property covers the same space — accepted.

## Change Log

| Date | Change |
|------|--------|
| 2026-06-12 | Senior Developer Review (AI) — Approve, status review → done. 0 Critical/High; 1 Medium + 2 Low fixed in-pass (File List completed with the QA test-summary + automator log; posture vocabulary extended with the `GatedInput` field names `field`/`name`; duplicate-year caller duty documented on `InputGates::new`); 1 Low observation accepted (proptest sampling of the 75-combo single-non-green space, AC-prescribed shape). All four gates re-verified green `--locked`; fingerprint/determinism/`Cargo.lock`-delta/issue #17 claims verified true. |
| 2026-06-12 | Story 1.11 implemented (status → review): invariant 2a — `core/src/verdict.rs` with `GateState`/`InputGates` (catalog-tied), immutable `StudySnapshot` (single ctor computes digest + outputs + verdict together), `FullVerdict` type-state (compile_fail-proven) and `Verdict` Full/Provisional/Withheld stamped `(inputs_hash, METHOD_VERSION)`; invariant 2b — `contract::Cell::edited` rail (divergence-keyed ✓→? via value-based Money equality, ADD15 injected provenance, zero serde-shape change — persistence snapshot + corpus gates byte-identical); e2e coherence + 25-way mutation proptest on the rail via core dev-dep on contract. All four gates green `--locked`; fingerprint/determinism/Spike-C/METHOD_VERSION/SCHEMA_VERSION unchanged; Cargo.lock delta = the one dev edge. Interpretations consolidated in GitHub issue #17. |
| 2026-06-12 | Story 1.11 created (ready-for-dev): the Epic-1 closer — invariant 2a (`FullVerdict` type-state in NEW `core/src/verdict.rs`: private fields, single derivation entry, Full ⟺ all load-bearing gates validated-and-fresh ∧ ¬low_confidence, `compile_fail` doctest + proptest), invariant 2b (manual-mutation rail `Cell::edited` in `contract` — divergence-keyed ✓→? demotion via value-based Money equality, caller-supplied provenance, NO serde-shape change), single immutable snapshot binding outputs + gates (derive-don't-store ⇒ no incoherent frame representable), content-addressed `(inputs_hash, METHOD_VERSION)` stamps (ADD9), end-to-end coherence test via core dev-dep on contract (1.9 serde_json precedent). Pinned interpretations: transaction = snapshot in headless Epic 1; Withheld ⟺ missing vs Provisional ⟺ degraded/low-confidence; judgment-present ⇒ green glue (issue #14 cross-ref); Full orthogonal to quality_value_candidate. No METHOD_VERSION/SCHEMA_VERSION bump, no persistence/CI change, zero new external deps. Ultimate context engine analysis completed — comprehensive developer guide created. |

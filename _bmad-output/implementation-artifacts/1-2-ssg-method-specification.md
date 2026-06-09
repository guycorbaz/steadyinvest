# Story 1.2: SSG method specification (versioned oracle)

Status: ready-for-dev

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->
<!-- Epic 1: Proven SSG core & data foundation (headless). Depends on Story 1.1 (workspace scaffold, DONE). -->

## Story

As the developer (Guy, solo),
I want the exact SSG method pinned in a versioned specification,
so that the engine and its golden tests have a single authoritative oracle and any change to the method is traceable.

## Acceptance Criteria

1. **Authoritative spec document exists.** A prose method specification is authored (suggested: `docs/method/ssg-method-spec-v1.md`) that resolves every PRD Appendix-A deferral and is the single source of truth the engine (Story 1.8) and golden tests (Story 1.9) implement against.
2. **It defines the SSG output set (FR4).** The complete enumerated set of values the engine produces from a study's inputs — the five SSG sections: §1 growth (historical sales/EPS/price + CAGR + projected), §2 management (pre-tax profit on sales, ROE, debt — trend + latest), §3 P/E history (high/low/avg P/E, payout), §4 risk/reward (forecast high & low price, the buy/hold/sell zone boundaries, upside/downside ratio), §5 5-year potential (projected total return: appreciation + yield). Each output: name, formula, inputs it depends on, native-currency, output scale.
3. **Quality-flag thresholds (FR7).** Exact numeric thresholds + direction for each methodology quality flag (e.g. declining/insufficient margin, high debt, weak/declining ROE, erratic growth) — each as `(metric, comparator, threshold, severity)`.
4. **Plausibility rules (FR10).** The detectable input-plausibility issues (unadjusted split / series break, currency mismatch, fiscal-period misalignment, out-of-bound values) with the concrete detection rule and bounds for each — explicitly **distinct from quality flags**.
5. **"Usable year" + low-confidence rule (FR8).** The definition of a usable year (per Appendix A: a year with all load-bearing fields present — sales, EPS, high/low price) and the rule **study is low-confidence when usable years < 5**.
6. **"Load-bearing input" definition (FR12).** The precise set of inputs whose absence/invalidation degrades or withholds the verdict (so the `FullVerdict` type in Story 1.11 can be gated on exactly these).
7. **Banned-verb list (FR13) — posture gate.** The enumerated imperative action/recommendation verbs forbidden in any system-generated signal (buy/sell/hold as commands, etc.), in a machine-checkable form, with FR/UX neutrality rationale. (This is a *posture* gate, not a string grep over all text — define the scope precisely.)
8. **Golden tolerance (FR9/NFR-C2).** The numeric tolerance for golden comparisons: **exact match on zoning + verdict**, and **±0.5%** on derived numerics (configurable), with how the comparison is computed.
9. **Named rounding mode + per-field display scale.** A single named rounding mode (decide **half-up vs banker's/MidpointNearestEven** — see Dev Notes; this resolves the code-review deferral) applied **only at display**, never mid-chain, plus the per-field display scale (decimal places per output field).
10. **`method_version` declared + wired.** The spec declares a `method_version` string (e.g. `"ssg-1.0.0"`); `core` exposes it as a constant, and the machine-readable method constants live in `core` such that **changing any method constant requires bumping `method_version`** (enforced by a test) — so, by the Foundational Invariant, the dep-hash of every derived fact changes when the method changes.
11. **Spec ↔ code coherence is testable.** A test asserts the `core` method constants are internally consistent and that `method_version` is non-empty and referenced; the prose doc and the `core` constants do not contradict (cross-checked for the values that appear in both).

## Tasks / Subtasks

- [ ] **Task 1 — Author the prose method spec (AC: 1–9)**
  - [ ] Create `docs/method/ssg-method-spec-v1.md` with a section per Appendix-A deferral (output set, quality-flag thresholds, plausibility rules, usable-year/low-confidence, load-bearing input, banned-verb list, golden tolerance, rounding mode + display scales).
  - [ ] Derive the SSG formulas/thresholds from the NAIC reference PDFs in `docs/NAIC/` (SSG Handbook, Tutorial, A Beginner's Tour, the SSG form) — cite the source per item. Use **neutral labels**; do not copy verbatim instructional prose or NAIC marks (IP posture).
  - [ ] Record each numeric threshold with rationale + source page so it is auditable and reproducible.
- [ ] **Task 2 — Machine-readable method constants in `core` (AC: 2,3,4,5,6,7,8,9,10)**
  - [ ] Add a `core::method` module (e.g. `core/src/method/mod.rs`) holding the thresholds, plausibility bounds, golden tolerance, usable-year floor (5), load-bearing input set, and banned-verb list as typed constants/enums — exact-decimal where numeric (`rust_decimal`, never `f64`).
  - [ ] Add `core/src/method_version.rs` exposing `METHOD_VERSION: &str` (semver-like string), re-exported at crate root.
  - [ ] Add `core/src/rounding.rs`: the named rounding mode + a `display_scale(field)` mapping; rounding applied only at display (document this; do not round mid-calculation).
- [ ] **Task 3 — `method_version` change-detection test (AC: 10, 11)**
  - [ ] Add a test that pins a hash/snapshot of the method constants and asserts it matches a committed value tied to `METHOD_VERSION` — so editing a constant without bumping `METHOD_VERSION` fails the build (mirrors the schema-drift detector pattern from Story 1.10). Document how to regenerate it on an intentional method change.
  - [ ] Add a test asserting `METHOD_VERSION` is non-empty and that the load-bearing-input set is a subset of the declared input fields.
- [ ] **Task 4 — Verify & wire (AC: all)**
  - [ ] `cargo test -p steadyinvest-core` green; `cargo fmt --all --check` + `cargo clippy --all-targets --all-features --locked -- -D warnings` green.
  - [ ] Confirm `core` still has **no** I/O/UI/SQL/net deps (Cardinal Rule) — the method module is pure data + functions.
  - [ ] Cross-link the prose doc and the `core` constants (doc references the module; module references the doc section).

## Dev Notes

### Why this story comes before the engine
The engine (Story 1.8), the normalization layer (1.7), the golden self-check (1.9) and the verdict-integrity types (1.11) all implement against THIS spec. Authoring it first gives them a single authoritative oracle and makes "what is correct?" answerable. [Source: epics.md Epic 1 rationale; architecture.md ADD11]

### What is deferred to here (from the PRD, by design)
The PRD Appendix A explicitly defers these "to Architecture/method spec": **SSG output set (FR4), plausibility rules (FR10), load-bearing input (FR12), golden tolerance (FR9), banned-verb list (FR13)**. The PRD already pins some values you must honour (do not contradict):
- **Capital-at-risk (FR43):** `Σ max(0, (avg_cost − stop)) × qty`, only where `stop ≤ avg_cost`; per currency natively. *(Risk math is Epic 4, but the formula is pinned — keep it consistent if you reference it.)*
- **Usable year / low-confidence (FR8):** usable = all load-bearing fields present (sales, EPS, high/low price); **low-confidence when usable years < 5**.
- **Cost basis (FR39):** weighted-average, fees included. **Dividend net (FR41):** `gross × (1 − withholding_rate)` (CH = 35%); study uses **gross**.
- **Stale threshold (FR23):** price older than the user-configured horizon (default: > 1 trading day).
- **Neutrality (FR13):** system signals contain **no imperative action verb**; state facts only.
[Source: prd.md#Appendix A — Definitions; prd.md Functional Requirements FR4/FR7/FR8/FR9/FR10/FR12/FR13]

### Architecture constraints (must follow)
- The method spec is **consumed by `core`** and **pinned by `method_version`**; it is one of three version axes (`schema_version` blob / SQLite `user_version` / **`method_version`** calculation semantics). [Source: architecture.md#Core Technical Decisions, #Three version axes]
- **Foundational Invariant:** a derived fact's identity is `f(hash(inputs), method_version)`. Therefore the method constants must feed `method_version` (or its dep-hash) so a method change invalidates prior verdicts rather than silently changing them. The change-detection test (Task 3) enforces "you can't change the method silently". [Source: architecture.md#The Foundational Invariant, #Verdict versioning]
- **Two quality-gate families — keep distinct:** *trust gates* (types/traceability/reproducibility) vs *posture gates* (neutral naming/banned-verb/swappable labels). The banned-verb list is a **posture** artifact; "do not reduce neutrality to a string grep" — define its scope (system-generated signals/labels/alerts, not user free-text). [Source: architecture.md#Core Technical Decisions, #Quality-gate families]
- **Cardinal Rule:** all of this lives in `core` with no I/O/UI/SQL/net; exact decimal only. [Source: architecture.md#Enforcement Guidelines]
- Suggested `core` files already anticipated by the architecture tree: `core/src/method_version.rs`, `core/src/rounding.rs` (named rounding mode + per-field display scale), `core/src/quality_flags.rs` (FR7 thresholds + FR10 plausibility). [Source: architecture.md#Complete Project Directory Structure]

### Decision to make in this story — rounding convention
The code review of Story 1.1 flagged that `core` currently uses `round_dp` (rust_decimal default = **MidpointNearestEven / banker's rounding**), whereas NAIC SSG paper forms and most financial presentation use **half-up**. **Decide and pin the named rounding mode here** (recommendation: half-up / `RoundingStrategy::MidpointAwayFromZero` for fidelity to the paper form, applied only at display), document the rationale, and make `core/src/rounding.rs` the single place it is defined. Update the Story-1.1 determinism probe's `round_dp` to the chosen mode if you want consistency (optional — the probe is scaffolding, but note the hash will change if you do). [Source: code-review deferral D2; architecture.md Data Architecture "named rounding mode + per-field display scale"]

### NAIC source material (in repo) — use, do not copy verbatim
`docs/NAIC/SSGHandbook.pdf`, `Stock Selection Guide Tutorial.pdf`, `A-Beginners-Tour-of-the-SSG-Jan-2015.pdf`, `SSGPlus_QuickStart.pdf`, and the forms under `docs/NAIC/forms/`. The **method/formulas are not protectable** — implement them. **Neutralize** marks, logos, verbatim instructional prose, and the copyrighted form layout. [Source: prd.md#Intellectual property / trademark; project memory: open-source naming constraint]

### Previous story intelligence (Story 1.1 — DONE)
- `core` deps are exactly `rust_decimal` (+`maths`), `serde`, `sha2` — keep it that way; add no I/O deps.
- **MSRV is 1.96** (toolchain pinned), NOT 1.88 — Slint/libsqlite3-sys transitive deps. Don't reintroduce a lower pin.
- Exact-decimal pattern: `Decimal::new(mantissa, scale)`, `MathematicalOps::powd` (needs `use rust_decimal::MathematicalOps`), `round_dp`/`round_dp_with_strategy`. A canonical-string→SHA-256 snapshot pattern already exists (`determinism_hash`) — reuse it for the method-constants snapshot test (Task 3).
- CI runs `--locked` and is green on 3 OS; keep new tests deterministic and cross-OS safe (exact decimal, no float, no time/UUID in `core`).
- `method_version` was referenced as a planned constant in Story 1.1 dev notes; this story creates it for real.
[Source: 1-1-workspace-scaffold-ci-gate-skeleton.md Dev Agent Record + Review Findings]

### Project Structure Notes
- New: `docs/method/ssg-method-spec-v1.md` (prose oracle); `core/src/method/` (or `method.rs`), `core/src/method_version.rs`, `core/src/rounding.rs`, `core/src/quality_flags.rs` — wire them in `core/src/lib.rs`.
- No changes to other crates. No new workspace dependencies expected (pure `core`).
- Keep the determinism/`--locked`/fmt/clippy gates from Story 1.1 green.

### References
- [Source: epics.md#Story 1.2: SSG method specification (versioned oracle)] — user story + AC
- [Source: epics.md#Epic 1 / ADD11] — "author a method spec consumed by `core` before implementing the engine"
- [Source: prd.md#Appendix A — Definitions] — pinned values + the deferrals to resolve here
- [Source: prd.md FR4, FR7, FR8, FR9, FR10, FR12, FR13] — the requirements this spec makes concrete
- [Source: architecture.md#Core Technical Decisions / #The Foundational Invariant / #Data Architecture] — three version axes, method_version, named rounding, quality-gate families
- [Source: architecture.md#Complete Project Directory Structure] — `core/src/{method_version,rounding,quality_flags}.rs`
- [Source: docs/NAIC/*] — the method content (use, neutralize expression)

## Dev Agent Record

### Agent Model Used

(to be filled by dev-story)

### Debug Log References

### Completion Notes List

- Ultimate context engine analysis completed — comprehensive developer guide created.

### File List

## Change Log

| Date | Change |
|------|--------|
| 2026-06-09 | Story 1.2 created (ready-for-dev): versioned SSG method specification (prose oracle + `core` method constants + `method_version` change-detection). Resolves PRD Appendix-A deferrals; pins the rounding-convention decision. |

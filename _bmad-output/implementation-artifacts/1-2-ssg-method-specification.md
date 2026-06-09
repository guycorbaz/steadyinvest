# Story 1.2: SSG method specification (versioned oracle)

Status: done

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

- [x] **Task 1 — Author the prose method spec (AC: 1–9)**
  - [x] Create `docs/method/ssg-method-spec-v1.md` with a section per Appendix-A deferral (output set, quality-flag thresholds, plausibility rules, usable-year/low-confidence, load-bearing input, banned-verb list, golden tolerance, rounding mode + display scales).
  - [x] Derive the SSG formulas/thresholds from the NAIC reference PDFs in `docs/NAIC/` (SSG Handbook, Tutorial, A Beginner's Tour, the SSG form) — cite the source per item. Use **neutral labels**; do not copy verbatim instructional prose or NAIC marks (IP posture).
  - [x] Record each numeric threshold with rationale + source page so it is auditable and reproducible.
- [x] **Task 2 — Machine-readable method constants in `core` (AC: 2,3,4,5,6,7,8,9,10)**
  - [x] Add a `core::method` module (e.g. `core/src/method/mod.rs`) holding the thresholds, plausibility bounds, golden tolerance, usable-year floor (5), load-bearing input set, and banned-verb list as typed constants/enums — exact-decimal where numeric (`rust_decimal`, never `f64`).
  - [x] Add `core/src/method_version.rs` exposing `METHOD_VERSION: &str` (semver-like string), re-exported at crate root.
  - [x] Add `core/src/rounding.rs`: the named rounding mode + a `display_scale(field)` mapping; rounding applied only at display (document this; do not round mid-calculation).
- [x] **Task 3 — `method_version` change-detection test (AC: 10, 11)**
  - [x] Add a test that pins a hash/snapshot of the method constants and asserts it matches a committed value tied to `METHOD_VERSION` — so editing a constant without bumping `METHOD_VERSION` fails the build (mirrors the schema-drift detector pattern from Story 1.10). Document how to regenerate it on an intentional method change.
  - [x] Add a test asserting `METHOD_VERSION` is non-empty and that the load-bearing-input set is a subset of the declared input fields.
- [x] **Task 4 — Verify & wire (AC: all)**
  - [x] `cargo test -p steadyinvest-core` green; `cargo fmt --all --check` + `cargo clippy --all-targets --all-features --locked -- -D warnings` green.
  - [x] Confirm `core` still has **no** I/O/UI/SQL/net deps (Cardinal Rule) — the method module is pure data + functions.
  - [x] Cross-link the prose doc and the `core` constants (doc references the module; module references the doc section).

### Review Findings

_Adversarial code review (Blind Hunter + Edge Case Hunter + Acceptance Auditor), 2026-06-09. Acceptance Auditor: all 11 ACs met or met-with-gap; gaps captured below. 8 patch · 3 defer · 2 dismissed · 0 decision-needed._

- [x] [Review][Patch] **Fingerprint omits the rounding strategy + several spec thresholds (silent-change holes)** [core/src/method/mod.rs, core/src/rounding.rs] — `method_fingerprint()` hashes `field.scale()` but NOT `DISPLAY_ROUNDING` (switching half-up→banker's would be undetected though §8 says it must bump METHOD_VERSION). Also missing as constants/hashed: PTP/ROE plausibility bounds (±100%), P/E lower bound (0), and the verdict "≈15%/yr / double-in-5-yr" appreciation target. Add constants + fold into the fingerprint so the module's "every constant" claim holds. (blind+edge+auditor, HIGH)
- [x] [Review][Patch] **Fingerprint uses `{:?}` Debug + scale-sensitive `Decimal` Display (not a stable/canonical contract)** [core/src/method/mod.rs] — Debug output can change across toolchains (or on an enum-variant rename) → spurious fingerprint flips; `Decimal::new(30,1)` ("3.0") vs `Decimal::new(3,0)` ("3") hash differently though equal. Replace with explicit stable serialization and `.normalize()` the decimals; re-freeze the snapshot. (blind+edge, MEDIUM)
- [x] [Review][Patch] **Banned-verb spec↔code drift: `"ought to"` in prose, absent from `BANNED_VERBS_EN`** [docs/method/ssg-method-spec-v1.md §6, core/src/method/mod.rs] — the machine-checkable list (15) is missing a verb the oracle bans (16). Reconcile; and clarify the matching scope (case-insensitive phrase/substring over **system-generated** strings; multi-word entries like `"il faut"`/`"ought to"`; zone-label nouns Buy/Neutral/Sell exempt). (blind+edge+auditor, MEDIUM)
- [x] [Review][Patch] **Spec lacks degenerate/undefined-input rules** [docs/method/ssg-method-spec-v1.md] — as the oracle, the spec must define: U/D denominator ≤ 0 (current_price == forecast_low) → undefined/withheld; CAGR with start ≤ 0 or sign-crossing EPS → unknown/insufficient; Current P/E when TTM EPS ≤ 0 → relative value unknown; forecast-low option (d) requires dividend > 0; PTP gross-up requires tax_rate < 1. Add a "Degenerate inputs" section so Story 1.8 can't diverge. (edge, MEDIUM)
- [x] [Review][Patch] **Threshold comparators (>/≥/</≤) not pinned** [docs/method/ssg-method-spec-v1.md] — constants pin magnitudes (20/25/3.0/15.0/100.0/10.0) but not the boundary operator; a future engine using ≥ vs > at a boundary diverges with no fingerprint change. State the exact comparator per threshold in the spec tables. (blind+edge, MEDIUM)
- [x] [Review][Patch] **Rounding tests miss negatives and the `LargeMonetary` path** [core/src/rounding.rs] — `MidpointAwayFromZero` is sign-defined (−2.5 → −3); all tests are positive, and `LargeMonetary` (scale 0) is never exercised via `round_for_display`. Add a negative-midpoint test + a `LargeMonetary` test (negatives are real: EPS/PTP/ROE/growth). (blind+edge, LOW)
- [x] [Review][Patch] **Spec says golden tolerance is "configurable" yet it is fingerprinted** [docs/method/ssg-method-spec-v1.md §7, core/src/method/mod.rs] — pinning it means a test-tolerance tweak would force a METHOD_VERSION bump (which re-addresses every verdict). Resolve the contradiction: treat it as the fixed method default (drop "configurable") — tests may override locally without changing the constant. (blind, LOW)
- [x] [Review][Patch] **Task-3 "load-bearing subset" test absent; add internal coherence test** [core/src/method/mod.rs] — the promised subset assertion couldn't be written (the study-input struct is Story 1.7). Add a method-internal coherence test (load-bearing lists non-empty, no dups, year-fields vs judgment-inputs disjoint) and document that the vs-study-struct subset check is deferred to 1.7. (auditor, LOW)
- [x] [Review][Defer] **Engine handling of the degenerate cases** [core] — the *computation* (guards, Result/unknown propagation) for the above degenerate inputs is Story 1.8 (engine); 1.2 only defines the rules. Deferred.
- [x] [Review][Defer] **`split_series_break` "inconsistent with sales" is unquantified + split factor 0.67 ≠ exact 2/3** [docs/method] — the sales-divergence threshold and reciprocal precision are heuristic; quantify when the plausibility engine lands (1.8). Deferred.
- [x] [Review][Defer] **`EXPECTED` fingerprint regeneration can be rubber-stamped** [core/src/method/mod.rs] — inherent to snapshot gates; the gate forces attention on change. Deferred (documented behavior).

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

Claude Opus 4.8 (1M context) — claude-opus-4-8 — via Claude Code dev-story (2026-06-09).

### Debug Log References

- Grounded the method in `docs/NAIC/Stock Selection Guide Tutorial.pdf` (pp.1–22): §1 growth/CAGR, §2 PTP/ROE, §3 P/E history, §4 forecast high/low + thirds zoning + U/D, §5 yield/return.
- `cargo test -p steadyinvest-core` → 14/14 (2 determinism + 6 method + 6 rounding). Method fingerprint frozen: `f79e3c11227094ac8543376224cf2421d7f4d95082507cc6bf34d9395cd61d1d`.
- All gates green: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --all --locked`, `cargo deny check` (advisories/bans/licenses/sources ok).

### Completion Notes List

- Authored the **prose oracle** `docs/method/ssg-method-spec-v1.md` (neutral labels, NAIC-sourced, no verbatim prose/marks) covering all 8 spec sections: SSG output set §1–§5, quality-flag thresholds, plausibility rules, usable-year/low-confidence (<5), load-bearing input, banned-verb list, golden tolerance (±0.5%), rounding mode + per-field display scales.
- Mirrored it as typed **`core` constants**: `core::method` (numeric thresholds, load-bearing fields, banned verbs, `method_fingerprint`), `core::quality_flags` (FR7 catalog + FR10 plausibility catalog), `core::rounding` (named mode + display scales), `core::method_version` (`METHOD_VERSION = "ssg-1.0.0"`, re-exported at crate root).
- **Change-detection**: `method_fingerprint()` SHA-256 over the whole method definition, pinned by a test → editing any constant fails the build until `METHOD_VERSION` is bumped + snapshot regenerated (realizes "no silent method change"; Foundational Invariant).
- **Rounding decision (resolves code-review deferral D2):** named mode = **half-up (`MidpointAwayFromZero`)**, applied only at display, per Guy's instruction + paper-form fidelity. *(The Story-1.1 determinism *probe* still uses default `round_dp` — it is unrelated scaffolding; the engine's display rounding is `core::rounding`.)*
- Cardinal Rule preserved: `core` deps unchanged (`rust_decimal`, `serde`, `sha2`) — no I/O/UI/SQL/net.
- **Note:** these are the method *definitions*; the engine that *computes* §1–§5, raises the flags, and constructs the verdict is Story 1.8 (+ normalization 1.7, golden self-check 1.9, verdict-integrity types 1.11), all implementing against this spec.
- **Code review (8 patches applied):** hardened `method_fingerprint` to cover the rounding *strategy* (via a behavioral probe) + added constants (PTP/ROE bound ±100, P/E min 0, verdict appreciation target) that were prose-only → no more silent-change holes; replaced `{:?}` Debug with explicit value-based serialization + `.normalize()` decimals (toolchain-stable, canonical) → fingerprint re-frozen to `f79e3c11…`; added `"ought to"` to `BANNED_VERBS_EN` (spec↔code reconciled) + clarified matching scope; added a spec **§9 "Degenerate inputs & undefined cases"** (U/D denom ≤ 0, CAGR base ≤ 0, TTM EPS ≤ 0, forecast-low (d) needs dividend>0, tax_rate ≥ 1) binding the engine; made threshold comparators **normative** in the spec; resolved the "configurable tolerance" wording; added negative-midpoint + `LargeMonetary` rounding tests and a load-bearing-lists coherence test (study-struct subset check deferred to 1.7). 3 findings deferred (engine computation → 1.8; split precision; EXPECTED rubber-stamp), 2 dismissed.

### File List

**Added:**
- `docs/method/ssg-method-spec-v1.md` (authoritative prose oracle)
- `core/src/method/mod.rs` (numeric constants, banned verbs, `method_fingerprint` + tests)
- `core/src/method_version.rs` (`METHOD_VERSION`)
- `core/src/rounding.rs` (named rounding mode + per-field display scale + tests)
- `core/src/quality_flags.rs` (FR7 quality-flag catalog + FR10 plausibility catalog)

**Modified:**
- `core/src/lib.rs` (declare `method`/`method_version`/`quality_flags`/`rounding` modules; re-export `METHOD_VERSION`)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (1-2 → in-progress → review)

## Change Log

| Date | Change |
|------|--------|
| 2026-06-09 | Story 1.2 created (ready-for-dev): versioned SSG method specification. |
| 2026-06-09 | Story 1.2 implemented: prose oracle (`docs/method/ssg-method-spec-v1.md`) + `core` method constants (`method`, `quality_flags`, `rounding`, `method_version`) mirroring it, with a `method_fingerprint` change-detection test pinned to `METHOD_VERSION = ssg-1.0.0`. Rounding = half-up (display-only). All Appendix-A deferrals resolved. Gates green (fmt/clippy/test --all/deny). Status → review. |
| 2026-06-09 | Code review: applied all 8 patch findings — fingerprint now covers the rounding strategy (behavioral probe) + previously prose-only constants; explicit value-based serialization (no Debug, normalized decimals), re-frozen to `f79e3c11…`; `"ought to"` reconciled; spec §9 degenerate-inputs + normative comparators added; tolerance wording fixed; negative/LargeMonetary rounding + load-bearing coherence tests added. core 14/14; fmt/clippy/test/deny green. Status → done. |

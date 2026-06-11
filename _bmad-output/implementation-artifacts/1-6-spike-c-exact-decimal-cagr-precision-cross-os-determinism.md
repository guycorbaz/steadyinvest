# Story 1.6: Spike C — exact-decimal CAGR precision & cross-OS determinism

Status: done

<!-- Note: THROWAWAY SPIKE (the deliverable = a GO/NO-GO decision + findings note). EXCEPTION to the
     usual "delete it after": unlike Spikes A/B (UI, need a display), this spike's artifact is a cheap
     headless test — it is KEPT as the permanent determinism gate for fractional powd math. -->
<!-- Epic 1. Last of the three Week-1 de-risking spikes (A grid GO, B chart GO). Closes the
     architecture's "Verify CAGR precision in the Week-1 spike" obligation. -->

## Story

As the developer (Guy, solo),
I want to prove `rust_decimal` (+`maths`) gives exact, reproducible compound-growth results,
so that the no-float determinism decision (exact decimal everywhere in the decision chain) is validated end to end **before** the real engine (Stories 1.7/1.8) is built on it.

## Acceptance Criteria

1. **CAGR / projection on a known multi-year series match hand-computed values to a defined precision.** Given a known multi-year series, CAGR `(end/start)^(1/n) − 1` and a 5-year projection `base × (1+g)^5` are computed with `rust_decimal`'s `maths` feature (`powd`), and:
   - the **integer-power** path (projection) matches the hand-computed value **exactly** (zero deviation — integer powers multiply exactly in `Decimal`);
   - the **fractional-power** path (CAGR, `1/n` exponent → `exp(y·ln x)` series) matches the reference value within a **defined, asserted precision bound** (default: relative error ≤ 1e-9 — see Dev Notes; the *measured* deviation is recorded), which must be orders of magnitude tighter than both the golden tolerance (±0.5%, method spec §7) and the Percent display quantum (0.05 at 1 dp, §8).
2. **The CI determinism-hash job asserts an identical hash.** A spike-specific SHA-256 over the CAGR/projection result vector (serialized as normalized decimal strings, same scheme as `core::determinism_hash`) is asserted against a **pinned constant** in a test, and that test is wired into the existing CI "Determinism hash" step. The pinned constant *is* the cross-OS contract (any OS running the test must reproduce it). **CI stays Linux-only** (decision 2026-06-09) — do NOT re-add macOS/Windows runners; the cross-OS claim holds by construction (pure-Rust `rust_decimal`, no libm/f64) and becomes mechanically verified on all three OS the day the matrix is restored.
3. **A GO/NO-GO note records the precision/rounding behaviour for the method spec.** `docs/spikes/spike-c-decimal-cagr-determinism.md` records: what was measured, the integer-path exactness, the fractional-path measured deviation, panic/checked behaviour of `powd`/`ln` relevant to Story 1.8, the GO/NO-GO decision, and — if NO-GO — the fallback per the architecture (scaled integers / reformulate without transcendentals; **never `f64`** in the decision chain).
4. **Gate-clean.** `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --all --locked`, `cargo deny check` all green; no production code modified (see Project Structure Notes).

## Tasks / Subtasks

- [x] **Task 1 — Spike test: hand-computed CAGR & projection precision (AC: 1)**
  - [x] Add integration test `core/tests/spike_c_cagr_precision.rs`. Add `rust_decimal` and `sha2` to `core` `[dev-dependencies]` (both already pinned in `[workspace.dependencies]`; test-only — `core`'s runtime deps unchanged). *(No Cargo.toml edit was needed: both crates are already in `core`'s `[dependencies]` — used by `determinism_probe`/`determinism_hash` — and regular deps are available to integration tests; duplicating them in `[dev-dependencies]` would be redundant. Runtime deps unchanged, as required.)*
  - [x] **Exact-by-construction series:** build series where the true CAGR is known exactly, e.g. EPS `1.00 → 1.61051` over 5 intervals (`1.61051 = 1.1^5` exactly ⇒ true CAGR = `0.10` exactly); a second series at another rate/length (e.g. `1.08^9` over 9 intervals ⇒ true CAGR = `0.08`). Compute the end values **in the test** by exact integer-power multiplication — do not hand-type long constants.
  - [x] **Fractional path:** `cagr = (end/start).powd(Decimal::ONE / n) − 1`; assert `|cagr − true_cagr| / true_cagr ≤ 1e-9` AND `eprintln!` the exact measured deviation (run with `--nocapture` to read it).
  - [x] **Round-trip metamorphic check (no hand constant needed):** raise the `powd` result back to the integer power `n` (exact path) and compare to `end/start` — pins the series-approximation error without trusting any typed-in reference digits.
  - [x] **Integer path is exact:** `1000 × 1.15^5` via `powd(Decimal::from(5))` equals `2011.3571875` **exactly** (`assert_eq!`, no tolerance). Verify the constant by exact multiplication in the test too.
  - [x] **Display-rounding interaction:** confirm full-precision CAGR → `core::rounding::round_for_display(v, DisplayField::Percent)` gives the hand-expected 1-dp half-up value (e.g. true 10% stays `10.0`); rounding remains display-only, never mid-chain.
- [x] **Task 2 — Spike determinism hash, pinned + wired into CI (AC: 2)**
  - [x] In the same test file: hash the result vector (CAGRs + projection) with SHA-256 over `d.normalize().to_string()` + `\n` per value — the **same serialization scheme** as `core::determinism_hash` (value-only, representation-independent). Pin the digest as `const EXPECTED: &str` (run once, copy the printed digest, commit).
  - [x] Extend the CI **"Determinism hash"** step in `.github/workflows/ci.yml` with a second command: `cargo test -p steadyinvest-core --test spike_c_cagr_precision --locked`. Do NOT modify the existing `determinism_hash_matches_cross_os_contract` line, the existing probe, or its pinned `EXPECTED`.
- [x] **Task 3 — Run wiring (AC: 4)**
  - [x] Add `justfile` recipe `spike-c: cargo test -p steadyinvest-core --test spike_c_cagr_precision -- --nocapture` (headless — unlike `spike-a`/`spike-b`, no display needed; the measured deviations print to stderr).
- [x] **Task 4 — GO/NO-GO findings note (AC: 3)**
  - [x] Create `docs/spikes/spike-c-decimal-cagr-determinism.md` following the Spike A/B note shape: question, what was built, measured evidence (integer-path exactness, fractional-path deviation, the pinned hash), `powd`/`ln` panic vs `checked_*` behaviour notes for Story 1.8, the **GO/NO-GO decision**, and the fallback if NO-GO.
  - [x] Record the precision statement **for the method spec** in the note. If a spec §8 amendment (computation-precision clause) is wanted, file it as a **GitHub issue** (issues are the single source of truth) — do NOT edit `docs/method/ssg-method-spec-v1.md` in this story (change control couples spec edits to a `METHOD_VERSION` bump; out of spike scope).
  - [x] Unlike Spikes A/B, **no on-display step is needed**: the evidence is fully automated, so the GO/NO-GO can be concluded from the test output (Guy confirms by reading the findings note / CI).

## Dev Notes

### This is a SPIKE — what "done" means here

The deliverable is the **GO/NO-GO decision + findings note**. But this spike's artifact is a headless, sub-second test — so unlike A/B it is **kept** as the permanent CI regression gate that pins fractional-`powd` behaviour (e.g. against a future `rust_decimal` upgrade silently changing the series math). The "throwaway" clause means *no production code*: a test is a gate, not product code. [Source: epics.md Epic 1 "(Spikes are throwaway…)"; architecture.md "Verify CAGR precision in the Week-1 spike"]

### What ALREADY exists — build on it, do not reinvent or modify

- **`core::determinism_probe()` / `determinism_hash()`** (`core/src/lib.rs`) already compute a compound-growth vector `1000 × 1.15^n` via `powd` — including one **fractional** exponent (`n = 1.5`, the `exp(y·ln x)` series path) — hash it (SHA-256 over normalized decimal strings) and assert it against a pinned constant in the unit test `determinism_hash_matches_cross_os_contract`. **Do not modify any of this** — changing the probe invalidates its pinned `EXPECTED` hash; that is scaffolding owned by Story 1.1 and consumed by the CI gate. The spike *adds* a CAGR-specific test alongside, reusing the same hashing scheme. [Source: core/src/lib.rs]
- **The CI "Determinism hash" step exists** (`.github/workflows/ci.yml`, `quality` job) and runs the probe test. Extend it with one extra command (Task 2); leave everything else in the workflow untouched. [Source: .github/workflows/ci.yml]
- **Method spec pins the semantics this spike validates:** §1 historical sales/EPS CAGR; §7 golden tolerance ±0.5% relative; §8 half-up display rounding (`MidpointAwayFromZero`) + Percent scale 1 dp, rounding **only at display**; §9 CAGR base ≤ 0 / sign-crossing ⇒ `unknown/insufficient`, **never computed** — so the spike does NOT need to probe `powd`/`ln` on degenerate bases (the engine guards before calling); it only *documents* the panic vs `checked_*` behaviour for Story 1.8. [Source: docs/method/ssg-method-spec-v1.md §1/§7/§8/§9]
- **`core::rounding`** provides `round_for_display` + `DisplayField::Percent` (1 dp, half-up) — reuse for the display-interaction check; never round mid-chain. [Source: core/src/rounding.rs]
- **`METHOD_VERSION = "ssg-1.0.0"`** — this spike changes no formula/threshold ⇒ **no bump**. [Source: core/src/method_version.rs]

### Cross-OS AC vs Linux-only CI — the resolution (read before touching ci.yml)

The epic AC says "identical hash across Windows/macOS/Linux", but CI is **deliberately Linux-only** (decision 2026-06-09; comment in ci.yml; sprint-status header). Resolution: the **pinned-constant** hash is the cross-OS assertion mechanism — it does not compare two runners, it compares every runner to one recorded truth, so it *is* the contract on any OS that ever runs it. Determinism is by construction (pure-Rust `rust_decimal`; `maths` is pure Rust, no platform libm; no `f64` in the chain — the architecture chose it precisely so cross-OS holds "WITHOUT vendoring a libm or bit-hashing f64"). **Do NOT re-add macOS/Windows runners in this story.** Record the caveat ("mechanically verified on 3 OS when the matrix returns") in the findings note. [Source: architecture.md "rust_decimal 1.42"; .github/workflows/ci.yml; project memory Linux-only 2026-06-09]

### Precision: what to expect and what to assert

- `powd` with a **whole** exponent multiplies exactly — assert **equality**, no tolerance.
- `powd` with a **fractional** exponent computes the approximation `exp(y·ln(x))`; `ln` uses a Taylor series. So the CAGR result has a small series-truncation error in the trailing digits of the 28-29 significant-digit `Decimal` — that error is **deterministic** (same bits everywhere), which is exactly why a pinned hash works. The spike's job is to *measure* it (expect relative error far below 1e-9, likely ~1e-15 or better on these magnitudes) and assert a conservative bound (default **1e-9 relative**) so the test never flakes while still sitting ~6-7 orders of magnitude inside the ±0.5% golden tolerance and the 0.05 Percent display quantum. If the measured error is materially worse than ~1e-9, that is a finding → weigh GO/NO-GO. [Source: docs.rs rust_decimal MathematicalOps; architecture.md §rust_decimal]
- **Panic surfaces (document for Story 1.8, don't engineer around in the spike):** `powd` can panic on overflow; `ln`/`log10` panic on input ≤ 0; the `checked_powd` / `checked_ln` variants (and the `maths-nopanic` feature) are the non-panicking paths. Story 1.8 must use checked paths (already in `deferred-work.md` from the 1-1 review); the findings note should restate this with the spike's observations. [Source: docs.rs rust_decimal; deferred-work.md "No overflow / Result handling in core decimal math"]
- **Hand-computed references without typing long constants:** construct test series from a known rate (`end = start × (1+g)^n` computed by exact integer multiplication in the test), so the true CAGR is exact **by construction**; plus the round-trip check (`powd` fractional result re-raised to the integer power ≈ `end/start`). Avoid pinning hand-typed transcendental digits (e.g. `2^(1/10) = 1.0717734625…`) unless independently derived with a high-precision tool.
- **`1/n` exponent subtlety:** `Decimal::ONE / n` is exact only when `1/n` terminates (n = 5 → `0.2`, n = 10 → `0.1`); for n = 9 it truncates at 28 digits (`0.111…1`), adding a ~`ln(end/start)×1e-28` exponent error — negligible against the 1e-9 bound, but it means the n = 9 series exercises a slightly different error mix. Use a terminating-`1/n` series (5 or 10 intervals) for the headline hand-match assertion; the non-terminating one is still valuable as a second measured data point.

### Previous story intelligence (1-1 → 1-5 dev records)

- **MSRV 1.96** (`rust-toolchain.toml` — the architecture's "1.88" is stale); CI **Linux-only**; gates run `--locked`; clippy `-D warnings` covers `--all-targets` (integration tests ARE linted — keep them clean).
- Spike-note + `justfile`-recipe pattern established by 1.5/1.4: `docs/spikes/spike-{a,b}-*.md` + `spike-a`/`spike-b` recipes — follow the same shape (`spike-c`).
- "Done" = it **demonstrably works**, not "it compiles" — here the evidence is the test output + measured deviations, so this spike is fully verifiable headless (no Guy-on-display step, unlike A/B).
- Don't silence errors; in tests prefer explicit `assert!`/`assert_eq!` with messages carrying the measured deviation so a CI failure is self-explaining.
- The existing CI determinism step uses a `tests::…  -- --exact` filter that only matches the **lib unit test**; an integration test in `core/tests/` needs its own `--test spike_c_cagr_precision` invocation (hence Task 2's second command).

### Project Structure Notes

- **New:** `core/tests/spike_c_cagr_precision.rs` (kept as a permanent gate); `docs/spikes/spike-c-decimal-cagr-determinism.md`.
- **Modified:** `core/Cargo.toml` (add `[dev-dependencies]` `rust_decimal` + `sha2`, workspace-pinned); `.github/workflows/ci.yml` (ONE added command in the existing "Determinism hash" step); `justfile` (add `spike-c`); `_bmad-output/implementation-artifacts/sprint-status.yaml` (1-6 status transitions).
- **Do NOT modify:** `core/src/lib.rs` (probe/hash + pinned `EXPECTED`), `core/src/method/*`, `core/src/rounding.rs`, `core/src/method_version.rs`, `docs/method/ssg-method-spec-v1.md` (spec change ⇒ `METHOD_VERSION` bump — out of scope; file a GitHub issue if an amendment is wanted), `contract`/`app`/other crates, the CI matrix (Linux-only stays).
- **No new workspace dependencies** — `rust_decimal` and `sha2` are already in `[workspace.dependencies]`.

### References

- [Source: epics.md#Story 1.6: Spike C] — user story + AC + GO/NO-GO; "Spikes are throwaway" clause
- [Source: architecture.md#Core Technical Decisions "Exact decimal arithmetic"] — the no-float decision; fallback if revisited (scaled integers / no-transcendental reformulation — never f64)
- [Source: architecture.md "rust_decimal 1.42"] — `maths` feature pure-Rust & deterministic cross-platform; "Verify CAGR precision in the Week-1 spike"
- [Source: architecture.md "Week-1 spikes … (C)"] — CAGR precision check + cross-OS determinism hash
- [Source: prd.md NFR-X1] — identical numeric results across OS (held by construction; CI-verified again when the matrix returns)
- [Source: docs/method/ssg-method-spec-v1.md §1/§7/§8/§9] — CAGR definition, ±0.5% golden tolerance, half-up display-only rounding + scales, degenerate-base rule
- [Source: core/src/lib.rs] — existing `determinism_probe`/`determinism_hash` + pinned-hash test (the pattern to reuse, NOT modify)
- [Source: .github/workflows/ci.yml] — the "Determinism hash" step to extend; Linux-only matrix comment
- [Source: deferred-work.md (1-1 review)] — checked decimal math required for Story 1.8; banker's-rounding probe note
- [Source: docs.rs/rust_decimal MathematicalOps] — `powd` = `exp(y·ln x)` for non-whole exponents; `ln` Taylor series, panics on ≤ 0; `checked_*` variants; `maths-nopanic`
- [Source: 1-5 / 1-4 dev records] — spike pattern (findings note + justfile recipe), MSRV 1.96, `--locked` gates, Linux-only

## Dev Agent Record

### Agent Model Used

Claude Fable 5 (claude-fable-5)

### Debug Log References

- RED→GREEN for the pinned hash: first run with placeholder `EXPECTED` failed as designed, printing the computed digest `d9af555376f754c8543bb4a6c94267257c227612df4a18e7ee6cba02b57e5557`; pinned and re-run green.
- Measured deviations (`just spike-c`, Linux, rustc 1.96, rust_decimal 1.42.0): n=5 CAGR `0.1000000000000000000000000001` (relative error 1e-27); n=9 CAGR `0.0800000000000000000000000000` (relative error 0); round-trip n=5 4e-28, n=9 0; integer projection `2011.3571875` exact.
- Empirical `powd`/`ln` probe (throwaway, /tmp, rust_decimal =1.42.0): `ln(0)`/`ln(-1)`/`log10(0)` panic; `powd` overflow panics; `checked_ln(≤0)` = None; `checked_powd` overflow = None; ⚠️ `powd(-2, 0.5)` does NOT panic — silently returns `-1.4142…` (`sign(x)·|x|^y`), and `checked_powd` returns `Some(-1.4142…)` too ⇒ the spec §9 degenerate-base guard is load-bearing for Story 1.8 (recorded in the findings note).
- Existing CI command `cargo test -p steadyinvest-core --locked tests::… -- --exact` verified to still run the lib unit test (1 passed) and filter the new integration binary (6 filtered out) — hence the separate `--test spike_c_cagr_precision` command, as the Dev Notes predicted.

### Completion Notes List

- **Decision: GO** (2026-06-11) — recorded in `docs/spikes/spike-c-decimal-cagr-determinism.md`. Integer-power path exact (zero deviation); fractional `powd` (`exp(y·ln x)` series) measured at ≤1e-27 relative error — ~18 orders of magnitude inside the asserted 1e-9 bound, ~24 inside the ±0.5% golden tolerance (§7) and far inside the 0.05 Percent display quantum (§8). The exact-decimal no-float decision is validated end to end for Stories 1.7/1.8.
- `core/tests/spike_c_cagr_precision.rs` (6 dev tests; 11 after the same-day QA hardening pass, 14 after the second review pass — see test-summary.md in File List) is KEPT as the permanent CI determinism gate; its pinned SHA-256 uses the exact serialization scheme of `core::determinism_hash` (`normalize().to_string()` + `\n`, value-only). Story 1.1's probe/hash/pinned constant untouched.
- CI "Determinism hash" step extended with one added command (multi-line `run:`); existing command byte-identical; YAML validated; CI stays Linux-only (decision 2026-06-09) — the pinned constant is the cross-OS contract, mechanically verified on 3 OS when the matrix returns (caveat recorded in the note).
- **Deviation (documented):** no `[dev-dependencies]` added to `core/Cargo.toml` — `rust_decimal` and `sha2` are already in `core`'s `[dependencies]` (used by `determinism_probe`), and regular deps are available to integration tests; duplicating them would be redundant. The subtask's intent (test can use both crates; runtime deps unchanged) is satisfied.
- No spec edit, no `METHOD_VERSION` bump (no formula/threshold change). The findings note records the precision statement for the method spec; an optional §8 computation-precision amendment can be filed as a GitHub issue if Guy wants it.
- Gates all green: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings` (0 warnings), `cargo test --all --locked` (41 existing + 6 new at dev time; 52 total after the QA pass added 5, 0 failures), `cargo deny check` (advisories/bans/licenses/sources ok).

### File List

- `core/tests/spike_c_cagr_precision.rs` (new — permanent spike/determinism gate; 6 dev tests + 5 QA-hardening tests + 3 second-review tests = 14)
- `docs/spikes/spike-c-decimal-cagr-determinism.md` (new — GO/NO-GO findings note)
- `.github/workflows/ci.yml` (modified — one command added to the existing "Determinism hash" step)
- `justfile` (modified — added `spike-c` recipe)
- `_bmad-output/implementation-artifacts/tests/test-summary.md` (new — QA pass: gap analysis + the 5 added tests)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified — 1-6 status transitions)
- `_bmad-output/implementation-artifacts/1-6-spike-c-exact-decimal-cagr-precision-cross-os-determinism.md` (modified — this story file)
- `.gitignore` (modified — review fix: `__pycache__/` + story-automator runtime marker under a labeled "Tooling runtime noise" section; the marker line itself predates this review, added by the automator session)

Not story work, but present in the same uncommitted working tree (automation-session artifacts, for transparency): `.claude/skills/bmad-story-automator/data/agent-config-presets.json` (automator preset saved 2026-06-10), `.claude/settings.json`, `_bmad-output/story-automator/` (session logs).

## Senior Developer Review (AI)

**Reviewer:** Guy (automated adversarial review) · **Date:** 2026-06-11 · **Outcome:** **Approve** (status → done)

All technical claims were independently re-verified and held: the 11 spike tests pass with the measured deviations matching the Dev Agent Record digit-for-digit (n=5 rel. error 1e-27, n=9 exact, round-trip 4e-28/0, projection `2011.3571875` exact); the pinned digest `d9af5553…5557` reproduces; the CI "Determinism hash" step's original command is byte-identical with the spike command added; all four gates re-run green (fmt clean, clippy 0 warnings, `cargo test --all --locked` 52 passed/0 failed, `cargo deny` ok); no production code touched (`core/src/` unmodified); the documented `[dev-dependencies]` deviation is correct (both crates already in `core` `[dependencies]`). ACs 1-4: IMPLEMENTED. All tasks marked [x]: verified done.

Findings (0 Critical, 0 High, 3 Medium, 2 Low) — all fixed in this review:

1. **[MEDIUM][fixed]** Stale Dev Agent Record: story said "6 tests" / "47 existing + 6 new" but the file holds 11 — a same-day QA pass (`bmad-qa-generate-e2e-tests`) added 5 tests (documented in `_bmad-output/implementation-artifacts/tests/test-summary.md`, previously unreferenced here). Counts corrected, test-summary added to File List.
2. **[MEDIUM][fixed]** Git-vs-File-List discrepancies: `.gitignore` and automation-session artifacts (`agent-config-presets.json`, `.claude/settings.json`, `_bmad-output/story-automator/`) changed in the working tree but undocumented. Now recorded in File List with provenance.
3. **[MEDIUM][fixed]** Three `__pycache__/` directories (story-automator skill) polluted `git status`; `.gitignore` lacked a `__pycache__/` rule. Added.
4. **[LOW][fixed]** The `.claude/.story-automator-active` ignore line had been appended under the unrelated "_bmad-output artifacts" comment block; moved under a labeled "Tooling runtime noise" section.
5. **[LOW][fixed]** The findings note described only the original 6 tests; added a "Post-spike QA hardening" paragraph noting the 5 tests that now mechanically pin its `checked_*`/negative-base behavioural claims (pinned digest unchanged).

Post-fix verification: `cargo fmt --all --check` clean; `cargo test --all --locked` 52 passed, 0 failed; spike digest unchanged.

## Senior Developer Review (AI) — second pass

**Reviewer:** Guy (automated adversarial review) · **Date:** 2026-06-11 · **Outcome:** **Approve** (status stays done)

Independent re-verification of every claim in the Dev Agent Record and the first review, against the working tree: the 11 spike tests passed with measured deviations matching digit-for-digit (n=5 relative error 1e-27, n=9 exact, round-trip 4e-28/0, projection `2011.3571875` exact); pinned digest `d9af5553…5557` reproduced; all four gates re-run green (fmt clean, clippy 0 warnings, `cargo test --all --locked` 52 passed/0 failed at review start, `cargo deny` ok); `core/src/` unmodified (`git diff HEAD -- core/src/` empty); spike hash serialization byte-compatible with `core::determinism_hash` (verified against `core/src/lib.rs`); CI original command byte-identical with the spike command added; sprint-status `1-6 → done` already synced. ACs 1-4: IMPLEMENTED. All tasks marked [x]: verified done. The first review's 5 fixes all held.

Findings (0 Critical, 0 High, 0 Medium, 2 Low) — all fixed in this review:

1. **[LOW][fixed]** `test-summary.md` AC-coverage breakdown counted only 10 of the 11 tests (AC1 listed as 5; the §8 display-rounding test was attributed to no AC). Corrected to "AC1 precision + display-rounding interaction: 6 tests".
2. **[LOW][fixed]** 3 of the 6 rows in the findings-note `powd`/`ln` behaviour table rested only on the throwaway `/tmp` probe with no permanent regression test: `ln(0)` panics, `powd` overflow panics, `powd(0,0) = 1`. The panic-vs-`checked_*` contrast is the stated rationale for Story 1.8's mandatory checked paths, so a `rust_decimal` upgrade changing it should fail the build. Added 3 tests (`bare_ln_panics_on_zero_unlike_checked_ln`, `bare_powd_panics_on_overflow_unlike_checked_powd`, `powd_zero_to_the_zero_is_one_by_convention`; 11 → 14); every table row is now gated. Findings note and story counts updated; pinned digest unchanged.

Post-fix verification: `cargo fmt --all --check` clean; `cargo clippy --all-targets --all-features --locked -- -D warnings` 0 warnings; `cargo test --all --locked` 55 passed, 0 failed; spike digest unchanged (`d9af5553…5557`); `cargo deny check` ok.

## Change Log

| Date | Change |
|------|--------|
| 2026-06-11 | Story 1.6 created (ready-for-dev): spike test `core/tests/spike_c_cagr_precision.rs` proving exact-decimal CAGR/projection precision (`powd` integer path exact, fractional path bounded + measured) + pinned spike determinism hash wired into the CI "Determinism hash" step (Linux-only stays) + `just spike-c` + GO/NO-GO findings note recording precision/rounding behaviour for the method spec. Ultimate context engine analysis completed — comprehensive developer guide created. |
| 2026-06-11 | Story 1.6 implemented (review): added `core/tests/spike_c_cagr_precision.rs` (exact-by-construction series, fractional path ≤1e-9 asserted / ~1e-27 measured, round-trip metamorphic check, integer path exact `2011.3571875`, §8 display-rounding interaction, pinned SHA-256 `d9af5553…5557`); CI "Determinism hash" step extended with the spike test command; `just spike-c` recipe added; GO decision + `powd`/`ln` panic-vs-`checked_*` observations (incl. silent negative-base result ⇒ §9 guard load-bearing) recorded in `docs/spikes/spike-c-decimal-cagr-determinism.md`. All gates green; no production code modified. |
| 2026-06-11 | Senior Developer Review (AI) — **Approve**, status review → done. All ACs/tasks/claims re-verified against the working tree (gates re-run green, measured deviations and pinned digest reproduced). 5 findings (3 Medium, 2 Low), all fixed in-review: stale test counts (6 → 11 after QA pass) corrected + QA `test-summary.md` referenced; git-vs-File-List discrepancies documented; `__pycache__/` gitignored; `.gitignore` section labeling fixed; findings note updated with the QA-hardening tests. Sprint status synced 1-6 → done. |
| 2026-06-11 | Senior Developer Review (AI), second pass — **Approve**, status stays done. Every Dev Agent Record and first-review claim independently re-verified (11 tests green with matching measured deviations, pinned digest reproduced, 4 gates green, `core/src/` untouched, hash scheme byte-compatible with `core::determinism_hash`). 2 findings (both Low), fixed in-review: `test-summary.md` AC-coverage off-by-one corrected; 3 unpinned behaviour-table rows (`ln(0)` panic, `powd` overflow panic, `powd(0,0)=1`) now regression-gated by 3 new tests (11 → 14, digest unchanged). Post-fix gates green (55 tests passed). |

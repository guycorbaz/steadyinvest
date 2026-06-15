# Story 2.14: Neutral voice & banned-verb enforcement

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want every system signal to state facts, never advice,
so that the app reinforces me as the sole decider.

## Acceptance Criteria

(From epics.md §Story 2.14, lines 727–738. BDD, verbatim intent. FR13 [P1], spec §6. Scope-resolved 2026-06-15 — see Dev Notes "Scope decision".)

1. **(FR13 — no banned verb, total coverage)** **Given** any system-generated message, label or alert, **when** it is rendered, **then** it contains **no imperative action/recommendation verb** from the canonical banned-verb list (`core::method::BANNED_VERBS_EN` + `BANNED_VERBS_FR`), proven by a **single consolidated posture test** that scans the *union* of every user-facing string surface in the `app` crate — `@tr()` literals (all `.slint`), `USER_FACING_MESSAGES` (state.rs), `USER_FACING_LABELS` (engine.rs), the NAIC↔neutral label table (labels.rs), and the **rendered `persistence::Error` Display strings** that splice into UI banners — against the canonical list, case-insensitive whole-word. The zone-label nouns (Buy/Neutral/Sell band names) are the only exemption, per spec §6.
2. **(FR13 — no bare literal bypasses the gate)** **Given** the `.slint` UI, **then** a test proves **no user-visible string literal escapes `@tr()`**: every user-facing text property (`text:`, `title:`, `placeholder-text:`, `accessible-label:` and the like) is either an `@tr(...)` call or a binding/expression — never a bare `"..."` string literal — so that the gate in AC1 actually sees 100% of rendered prose. A small, documented allow-list covers non-prose technical literals (glyph/icon strings, single-symbol markers already in the legend, format scaffolds).
3. **(FR13 — neutral-fact phrasing)** **Given** every registered system string, **then** it is **phrased as a neutral fact, not advice** ("the price entered the zone you defined" / "le prix est entré dans la zone que vous avez définie"), confirmed by an audit of the full inventory recorded in Dev Notes; any advice-phrased or imperative string found is rewritten to fact-stating form (the rewrite, if any, stays neutral-label and keeps the existing `@tr`/`MSG_*` registration).
4. **(FR13 — single source of truth, auditable in one place)** **Given** the now-consolidated posture framework, **then** the `app` posture scans use the **canonical** `core::method::BANNED_VERBS_*` (no re-declared copy in `app`), and the posture module carries a module-level doc that enumerates **every scanned surface**, the **zone-label exemption** (with rationale), and what is **explicitly never scanned** (user free-text: tickers, rationale notes, data-cell values, fixture/demo data) — so a reviewer can audit FR13 coverage from one file. The intentional crate-local duplicate in `persistence` (no-core-dep boundary) is verified indirectly by AC1's scan of the *rendered* error strings, and a one-line doc-cross-link records the boundary rationale.
5. **(DoD)** **Given** the Definition of Done for a posture/enforcement story, **then** it is unit-tested (the consolidated union scan; the bare-literal leak gate with a passing run over the real UI; the phrasing audit's spot-checks), the binary launches and runs the event loop, and the in-GUI click-through is a documented partial (human/AT-SPI, as 2.1–2.13). 4 CI gates green `--locked`; the **method-fingerprint** + golden gates stay green (this story does **not** change the method — the banned-verb list is the existing canonical one, untouched); **`core`/`contract`/`persistence`/`Cargo.lock`/`deny.toml`/`rust-toolchain` re-diff unchanged** (app-crate-only — see Scope decision). File List ⇄ git exact (issue #18).

## Tasks / Subtasks

- [x] **Task 1 — Close the bare-literal leak: every user-visible string goes through `@tr()` (app crate)** (AC: 2)
  - [x] Add a posture test in `app/src/posture.rs` that walks all `.slint` files under `app/ui/` (reuse the existing `slint_files()` helper) and, for each user-facing text property assignment (`text:`, `title:`, `placeholder-text:`, `accessible-label:`, `accessible-description:`, and any other prose-bearing property in use), asserts the right-hand side is **not a bare string literal** — it must be an `@tr(...)` call, a property/expression binding, or an allow-listed technical literal.
  - [x] Build the parser as a focused extension of the existing manual `@tr` scanner (no regex dep, no new crate): detect `<prop>: "literal"` where `<prop>` is in the user-facing-property set and the value is a quoted literal not immediately preceded by `@tr(`. Track the offending file + property + literal in the failure message.
  - [x] Define a small, **documented** allow-list constant (`POSTURE_BARE_LITERAL_ALLOW`) for the handful of legitimate non-prose literals (e.g. single-glyph markers `"◦"`, `"▦"`, `"?"`, `"✓"`, `"△"` that are themselves the legend's visual tokens; icon/asset names; pure format scaffolds like `" · "`). Each entry carries an inline reason. The test fails if a *new* bare literal appears that is not allow-listed.
  - [x] Run the test over the current UI; if it finds existing bare prose literals, **convert them to `@tr(...)`** (and they then fall under AC1's scan — bump the `@tr` floor accordingly). Record every conversion in the File List + Dev Notes.

- [x] **Task 2 — Consolidate to one canonical source + one umbrella scan (app crate)** (AC: 1, 4)
  - [x] Confirm `app/src/posture.rs` references `steadyinvest_core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR}` directly. If the app re-declares the verbs anywhere, **delete the copy and import the canonical const** (app already depends on `steadyinvest-core`). There must be exactly one verb list the app scans against.
  - [x] Add a single **umbrella posture test** `all_user_facing_app_strings_are_neutral()` that builds the *union* of every app-side user-facing surface and scans it against the canonical list with the existing whole-word, case-insensitive `contains_word` matcher:
    - all `@tr()` literals across `.slint` (existing extractor),
    - `USER_FACING_MESSAGES` (state.rs),
    - `USER_FACING_LABELS` (engine.rs),
    - the NAIC↔neutral label-table strings (labels.rs),
    - the **rendered `persistence::Error` Display strings** — construct each `persistence::Error` variant (app depends on `steadyinvest-persistence`) and scan its `to_string()` output, so the banner text that actually reaches the user is verified by the canonical list (this is what makes persistence's crate-local copy belt-and-suspenders rather than load-bearing).
    - Apply the spec-§6 **zone-label exemption** (Buy/Neutral/Sell band nouns) in the same documented way the existing per-surface tests do.
  - [x] Keep the existing per-surface tests (they give precise failure locality and the floor assertions); the umbrella test is the auditable FR13 completeness proof. Assert the union is non-empty (guards against the scan silently collecting nothing).

- [x] **Task 3 — Neutral-fact phrasing audit (app crate)** (AC: 3)
  - [x] Enumerate the full registered inventory (the `@tr` literals + `USER_FACING_MESSAGES` + `USER_FACING_LABELS` + label table) and audit each for **fact-stating vs advice** phrasing. Record the audit outcome in Dev Notes (a short table: surface → verdict clean / rewritten).
  - [x] For any string phrased as advice or as an imperative directed at the user (beyond the already-banned verbs — e.g. a soft "pensez à…", "n'oubliez pas de…"), rewrite to a neutral fact, preserving meaning and the `@tr`/`MSG_*` registration. The reference exemplar is the AC's "the price entered the zone you defined".
  - [x] Where automatable, add spot assertions (e.g. a couple of representative alert/notice strings assert they contain no second-person imperative scaffold from a tiny documented heuristic set) — but do **not** over-engineer a natural-language grader; the canonical banned-verb gate (AC1) is the hard contract, the phrasing audit is the human-judgment layer recorded in Dev Notes.

- [x] **Task 4 — Document the posture framework in one place (app crate)** (AC: 4)
  - [x] Add a module-level doc comment at the top of `app/src/posture.rs` that enumerates: (a) every scanned surface and where it lives, (b) the canonical source `core::method::BANNED_VERBS_*` and the spec §6 scope ("system-generated signals only"), (c) the zone-label exemption with its one-line rationale, (d) the explicit non-scanned set (tickers, rationale notes, data-cell values, fixture/demo data) and why, (e) a one-line cross-link to the intentional `persistence::error` crate-local copy and the no-core-dep boundary that justifies it.
  - [x] Ensure the doc is the single audit entry point a reviewer reads to confirm FR13 coverage.

- [x] **Task 5 — Gates, floors, DoD** (AC: 5)
  - [x] If Task 1 converted any bare literals to `@tr`, bump the `@tr` floor to the new measured count; if Task 3 rewrote any `MSG_*`/labels, keep the inventory counts exact (a rewrite changes content, not count). Never lower a floor to hide a regression — measure and set to the true count.
  - [x] 4 CI gates green `--locked` (`fmt`, `clippy --all-targets --all-features -D warnings`, `test --all`, `deny check`). The **method-fingerprint**, **determinism hash**, **golden gate**, **golden drift** and **frozen `v1.db`** stay green — this story does not touch the method or any pinned surface.
  - [x] **`core`/`contract`/`persistence`/`Cargo.lock`/`deny.toml`/`rust-toolchain` re-diff unchanged** (app-crate-only). No new dependency.
  - [x] DoD: launch the binary + run the event loop; in-GUI click-through = documented partial (human/AT-SPI). File List ⇄ git exact — append any QA-generated test files and the automator log (issue #18). Don't mark `[x]` for a non-existent test.

## Dev Notes

### Scope decision (2026-06-15) — READ FIRST

**Story 2.14 is a consolidation + enforcement-completeness story, not a feature story.** The banned-verb infrastructure is ~80% built and distributed across stories 2.1–2.13. This story closes the genuine FR13 gaps and makes coverage auditable, **app-crate-only** (consistent with every Epic 2 story; `core`/`contract`/`persistence` re-diff unchanged).

What already exists (do **not** rebuild — verify and consolidate):
- **Canonical banned-verb list** — `core/src/method/mod.rs:115–146`: `BANNED_VERBS_EN` (16) + `BANNED_VERBS_FR` (10), with the FR13 / spec-§6 docstring ("system-generated signals only"; zone labels exempt). This is the single source of truth and is **untouched** by 2.14 (touching it would move the method fingerprint).
- **Per-surface app posture tests** — `app/src/posture.rs`: `@tr()` extractor + scan (floor ≥212 literals, ≥21 `.slint` files), `USER_FACING_MESSAGES` scan (=23), `USER_FACING_LABELS` scan (=22), label-table scan, and the whole-word `contains_word` matcher.
- **Core self-check gates** — `verdict.rs`, `golden/compare.rs`, `ssg/mod.rs` (cheap posture checks on engine-emitted strings; zone labels exempt). These are core's own gates — leave them; 2.14 is app-side.
- **Persistence error gate** — `persistence/src/error.rs:94–123,203–213`: an **intentional crate-local copy** of the verb list (persistence must not depend on `core` — dependency-graph boundary) + a test that error variants are neutral.

The three genuine gaps 2.14 closes:
1. **Bare-literal leak (the real hole).** The `@tr()` scan only sees strings *inside* `@tr()`. A bare `Text { text: "Achetez"; }` in a `.slint` would render to the user yet bypass every gate. Task 1 adds a leak gate so the "verifiable test over UI strings" AC is actually *total*.
2. **Two truths for the verb list.** The app should scan against the **canonical** `core::method::BANNED_VERBS_*` (import, never re-declare). The persistence copy stays (boundary), but 2.14 verifies the *rendered* error strings against the canonical list at the app layer (Task 2), so the copy is belt-and-suspenders, not load-bearing.
3. **No single audit point.** Coverage is spread over many tests. Task 2's umbrella scan + Task 4's module doc give one place to confirm FR13.

**Open question for Guy (saved per workflow — answer at dev time, does not block):** the phrasing audit (Task 3) may surface a soft-advice string that is *not* on the banned-verb list (e.g. "pensez à valider"). Default decision baked into this spec: **rewrite to a neutral fact**. If you'd rather keep any such string as-is, flag it during dev — the hard contract (banned-verb gate) is unaffected either way.

### Current state of files this story touches (UPDATE, not NEW)

- **`app/src/posture.rs`** — the home of this story. Today: `slint_files()` recursive walker, `tr_literals(source)` manual `@tr` extractor, `contains_word()` whole-word matcher, and the per-surface tests with floors. 2.14 **adds**: the bare-literal leak test (Task 1), the umbrella union scan (Task 2), the module-level audit doc (Task 4); and **consolidates** the verb source to `core::method::BANNED_VERBS_*` if any local copy exists. Preserve every existing test and floor — only raise the `@tr` floor if Task 1 adds conversions.
- **`app/ui/**/*.slint`** — only touched **if** Task 1 finds a bare user-visible literal to convert to `@tr(...)`. Expect few or none (the codebase has been `@tr`-disciplined since 2.1). Each conversion is a one-line change; record it.
- **`app/src/state.rs` (`USER_FACING_MESSAGES`, `MSG_*`)**, **`app/src/viewmodel/engine.rs` (`USER_FACING_LABELS`)**, **`app/src/labels.rs` (label table)** — read-only unless Task 3's audit rewrites a string's wording (content change, not count). Keep inventory counts exact.
- **Do NOT touch** `core/src/method/mod.rs` (canonical list — moving it moves the fingerprint), nor any `core`/`contract`/`persistence` source. The persistence copy stays as-is (verified indirectly).

### Testing standards

- All gates `--locked`: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings` (remember: `--all-targets` compiles without `cfg(test)`, so any new helper must be reachable or `#[cfg(test)]`), `cargo test --all`, `cargo deny check` (note the pre-existing `GPL-3.0` unmatched-allowance warning is not introduced here).
- New tests live in `app/src/posture.rs` under `#[cfg(test)]`, matching the existing style (manual parsers, no new dependency).
- The umbrella scan must assert a **non-empty** union before scanning (a scan that silently collects zero strings would falsely "pass").
- DoD parity with 2.1–2.13: launch + event loop; in-GUI click-through = documented partial (human/AT-SPI; the Wayland sandbox blocks screenshots/AT-SPI — headless tests carry the proof).

### Previous-story intelligence (2.13 and the epic)

- **2.13** already registered its new strings and bumped `@tr` 177→212, `USER_FACING_MESSAGES` 20→23 — so the floors 2.14 inherits are current. 2.13's pattern "scan labels, not fixture data" is the rule 2.14 documents centrally.
- **Epic 2 retro (2026-06-15) standing lessons** to honor here: File List ⇄ git exact (issue #18 — append QA/automator artifacts); don't mark `[x]` for non-existent tests (the 2.6 finding); re-diff all pinned surfaces empty every review.
- **Posture-floor over-bumping** recurred across the epic (2.3/2.5/2.8/2.10/2.13) — measure the true `@tr` count after any Task-1 conversion and set the floor to exactly that; do not pad.

### Project Structure Notes

- App-crate-only, no new module, no new dependency, no schema/method change — fully aligned with the Epic 2 "app-crate-only, pinned-surfaces-unchanged" pattern. The only possible cross-file ripple is Task 1 converting a stray `.slint` literal to `@tr`, which is additive and self-contained.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 2.14] — user story + ACs (lines 727–738); FR13 cross-cutting note (lines 52, 247, 253).
- [Source: _bmad-output/planning-artifacts/prd.md] — FR13 (lines 685–686); Appendix A neutrality definition (lines 919–920: "Exact banned-verb list finalized in Architecture").
- [Source: _bmad-output/planning-artifacts/architecture.md] — FR13 cross-cutting (line 46); posture gates vs trust gates (line 433).
- [Source: docs/method/ssg-method-spec-v1.md] — canonical neutrality scope ("system-generated signals only"; zone-label exemption).
- [Source: core/src/method/mod.rs:108–146] — canonical `BANNED_VERBS_EN`/`BANNED_VERBS_FR` + FR13 docstring (the single source of truth; untouched).
- [Source: app/src/posture.rs] — existing `@tr` extractor, `contains_word`, per-surface tests + floors (the file 2.14 extends).
- [Source: app/src/state.rs:33–162] — `MSG_*` + `USER_FACING_MESSAGES` inventory (=23).
- [Source: app/src/viewmodel/engine.rs:750–911] — label consts + `USER_FACING_LABELS` inventory (=22).
- [Source: persistence/src/error.rs:86–221] — intentional crate-local verb copy + error-neutrality test (boundary rationale; verified indirectly by 2.14's app-side rendered-error scan).
- [Source: _bmad-output/implementation-artifacts/epic-2-retro-2026-06-15.md] — File-List/#18 + posture-floor lessons; 2.14 = critical-path B1 to close Epic 2.

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — `claude-opus-4-8[1m]`

### Debug Log References

- `cargo test -p steadyinvest-app --bin steadyinvest-app posture` — 10/10 posture tests pass (6 pre-existing + 4 new).
- `cargo clippy --all-targets --all-features --locked -- -D warnings` — clean.
- `cargo test --all --locked` — app 141 (was 137, +4), all workspace suites green; golden_gate / corpus_gate / verdict_coherence intact (method untouched).
- `cargo deny check` — advisories/bans/licenses/sources ok.
- `timeout 8 cargo run` — exit 124 (event loop ran healthily, no panic).
- ⚠️ `cargo fmt --all --check` — see Completion Note on the **pre-existing repo-wide rustfmt skew**.

### Completion Notes List

- **Story scope: consolidation + completeness, app-crate-only.** No `core`/`contract`/`persistence`/`Cargo.lock`/`deny.toml`/`rust-toolchain` change (re-diff confirmed empty). Single source file touched: `app/src/posture.rs`. No new dependency. Method fingerprint + golden gates untouched (the canonical banned-verb list in `core` was not modified).
- **Task 1 — bare-literal leak gate (the real FR13 hole, now closed).** New `bare_user_facing_literals()` parser + `no_bare_user_facing_literal_bypasses_tr()` test. Scanning the live UI found **6 bare literals, all single-glyph visual markers** — `⦸` (not-available), `⇕` (drag handle), `╱╱╱╱` + `╱╱╱╱╱╱` (provisional hatching), `?` + `✓` (review tags). **Zero bare *prose* literals** — the codebase has been `@tr`-disciplined since 2.1, so **no `@tr` conversions were needed and the `@tr` floor stays 212.** The 6 glyphs are allow-listed in `BARE_LITERAL_ALLOW` (named `BARE_LITERAL_ALLOW`, not the story's tentative `POSTURE_BARE_LITERAL_ALLOW`), each with a one-line reason; the legend's `◦`/`▦`/`△` markers are *not* bare — they come through `@tr`/bindings. The gate's teeth are proven by a separate unit test `bare_literal_detector_distinguishes_tr_bindings_and_bare_prose` (correctly flags `"Achetez maintenant"` and `"garder"`, ignores `@tr(...)` and bindings).
- **Task 2 — one canonical source + umbrella scan.** `app/src/posture.rs` already imported `core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR}` (no re-declared copy existed — confirmed). New `all_user_facing_app_strings_are_neutral()` scans the union of `@tr` literals + `USER_FACING_MESSAGES` + `USER_FACING_LABELS` + `labels::LABELS` + **rendered `persistence::Error` Display strings** (via `sample_persistence_error_messages()`, constructing every variant except `Sqlite` — `app` doesn't dep `rusqlite`; its only own prose is the clean static prefix) against the canonical list, with a non-empty-union guard. This re-validates persistence's output against `core`'s list without persistence depending on `core`.
- **Task 3 — phrasing audit (all clean, zero rewrites).** Audited the full registered inventory for advice-vs-fact phrasing. **Every string already states facts** — no advice scaffolds present (verified by grep + the new `user_facing_strings_state_facts_not_advice()` heuristic test over a documented non-exhaustive scaffold set: `pensez à`, `n'oubliez`, `veuillez`, `devriez`, `make sure`, `be sure to`, `remember to`, `don't forget`, `you should`, `assurez-vous`). No `MSG_*`/`@tr` wording changed → inventory counts stay exact (`USER_FACING_MESSAGES` = 23, `USER_FACING_LABELS` = 22).
- **Task 4 — single audit point.** Rewrote the `posture.rs` module doc to enumerate every scanned surface, the canonical source, the zone-label exemption + rationale, the never-scanned user-data set, and the `persistence` crate-local-copy boundary cross-link.
- **⚠️ Pre-existing repo-wide rustfmt skew (NOT introduced by 2.14 — flag for Guy).** `cargo fmt --all --check` reports **20 diffs at pristine HEAD (5644c8e)** in files this story never touches (`state.rs`, `viewmodel/studies.rs`, `viewmodel/verify.rs`, `persistence/tests/journal_roundtrip.rs`). Root cause: the local pinned-toolchain rustfmt is `1.9.0-stable (ac68faa20c 2026-05-25)`, which makes different line-wrapping decisions than whatever rustfmt formatted those files at commit time (rustfmt patch drift since the 2.13 commit). **My new code is fmt-clean under the local rustfmt** (verified — `posture.rs` no longer appears in the diff; I used own-line comments + std-first imports to stay version-stable). I deliberately did **not** reformat the 20 untouched files — that churn is unrelated to 2.14 and could break the actual CI which expects the other formatting. **Recommend filing a GitHub issue** (per the issues-are-source-of-truth convention) to realign the repo to one rustfmt and re-green `cargo fmt --all --check`.
- **DoD:** in-GUI click-through remains a documented partial (human/AT-SPI; the Wayland sandbox blocks screenshots/AT-SPI) — headless posture tests carry the proof, as for 2.1–2.13.

### File List

- `app/src/posture.rs` (M) — expanded module doc (single FR13 audit point); new `USER_FACING_SLINT_PROPS` + `BARE_LITERAL_ALLOW` consts; new helpers `bare_user_facing_literals()`, `rhs_non_tr_literals()` (whole-RHS scan, ternary/concat aware, `==`/`!=` operand exclusion), `sample_persistence_error_messages()`, `provenance_display_labels()`; new tests `no_bare_user_facing_literal_bypasses_tr`, `all_user_facing_app_strings_are_neutral`, `user_facing_strings_state_facts_not_advice`, `bare_literal_detector_distinguishes_tr_bindings_and_bare_prose`, `bare_literal_detector_sees_ternary_branches_and_skips_state_key_comparisons`.
- `app/src/viewmodel/verify.rs` (M) — extracted the verify-panel / demo-notice prose to named consts (`MSG_FIXTURE_UNREADABLE_PREFIX`, `MSG_DEMO_MISSING`, `MSG_DEMO_UNREADABLE_PREFIX`, `MSG_DEVIATION_TEMPLATE`) + a `USER_FACING_TEMPLATES` inventory scanned by the posture gate (review fix P4).

## Senior Developer Review (AI) — 2026-06-15

**Reviewer:** Amelia (Developer), via the 3-layer adversarial review (Blind Hunter + Edge Case Hunter + Acceptance Auditor).
**Outcome:** Changes Requested → all addressed in-session. Re-verified green.

### Acceptance Auditor
All five ACs independently verified satisfied (tests pass, real-UI bare-literal count matches, pinned-surface diff empty, floors 212/23/22 intact). No High/Med. Benign Low notes: constant rename `POSTURE_BARE_LITERAL_ALLOW`→`BARE_LITERAL_ALLOW` (disclosed); the zone-label exemption is implicit/data-driven (consistent with the pre-existing per-surface test pattern).

### Findings & disposition

- **[P1 — High, Blind+Edge — FIXED] The leak gate only inspected the first token of a property value**, so a bare literal in a **ternary branch** (`cond ? @tr("…") : "Achetez"`) or **concatenation** (`base + "…"`) bypassed both the leak gate *and* the `@tr` scan — falsifying the "100% of rendered prose" guarantee. **Present bypassing cases** the old parser missed: `settings.slint` `"△"` and the `" — "`/`" · "` scaffolds, `collapsible_section.slint` `"▾"`/`"▸"`. **Fix:** rewrote the parser (`rhs_non_tr_literals`) to scan the whole RHS up to the statement `;`, flagging every string literal that is neither an `@tr(...)` argument nor an `==`/`!=` state-key comparison operand (e.g. `zone == "buy"` — compared, not displayed; rendered value is the result branch). Surfaced glyphs/separators added to `BARE_LITERAL_ALLOW`; new test `bare_literal_detector_sees_ternary_branches_and_skips_state_key_comparisons` proves it catches a ternary-else banned verb and ignores state-key comparisons.
- **[P2 — Med, Edge — FIXED] `entry::source_label` provenance words ("fournisseur"/"calculé") were unscanned** (rendered via `@tr("Source : {}", …)` as a dynamic value; only "manuel" was registered). **Fix:** `provenance_display_labels()` builds a present cell per `Source` variant and scans `source_label`'s output in the union.
- **[P3 — Med, Edge — FIXED] `persistence::Error::Sqlite` Display reaches the UI** via the `format!("{MSG_SAVE_FAILED} {error}")` catch-all in `state.rs`, contradicting the "omitted variant" justification. **Fix:** scan the `Sqlite` static own-prefix `"sqlite operation failed:"` directly (the variant needs `rusqlite` to construct); corrected the doc claim.
- **[P4 — Med, Edge — FIXED] verify-panel / demo-notice prose was unscanned.** **Fix:** extracted the `format!` prose to named consts + a `verify::USER_FACING_TEMPLATES` inventory scanned by the union (the deviation values themselves are `core::golden::GoldenDeviation`, gated in `core`).
- **[D1 — Low, dev/auditor — DEFERRED → issue #36] Pre-existing repo-wide rustfmt skew** (`cargo fmt --all --check` red at HEAD in untouched files). Filed as GitHub issue #36; my changed code is fmt-clean.
- **Dismissed (≈6):** order-coupled detector test (brittle but correct for a deterministic parser); escape-un-escaping divergence (latent — allow-list is raw glyphs, no escaped markers exist); comment-containing-`text:` false positive (latent — no such comments, would fail loudly); string-leading concatenation "false positive" (now *intended* — concat literals are rendered and correctly flagged, then allow-listed if non-prose); constant rename (disclosed/benign); implicit zone-label exemption (by-design, pre-existing pattern).

### Post-fix verification
4 new review-fix surfaces added; app tests **141 → 142**; `cargo clippy --all-targets --all-features --locked -D warnings` clean; `cargo test --all --locked` green; `cargo deny check` ok; binary launches + event loop (exit 124); `core`/`contract`/`persistence`/`Cargo.lock`/`deny.toml`/`rust-toolchain` re-diff **unchanged**; only `app/src/posture.rs` + `app/src/viewmodel/verify.rs` changed; both fmt-clean under local rustfmt (only the pre-existing #36 skew lines remain).

## Change Log

- 2026-06-15 — Story 2.14 implemented: consolidated the neutral-voice/banned-verb posture gate into one auditable module, closed the bare-literal-bypasses-`@tr` hole (leak gate), added the union completeness scan over all app user-facing surfaces incl. rendered `persistence::Error` strings, and a non-exhaustive advice-phrasing heuristic. App tests 137→141. App-crate-only; pinned surfaces unchanged. Flagged a pre-existing repo-wide rustfmt skew (20 diffs at HEAD) → GitHub issue #36.
- 2026-06-15 — 3-layer code review: addressed 4 findings (P1 High: ternary/concat literals bypassed the leak gate → whole-RHS scan with `@tr`/comparison-operand exclusion; P2/P3/P4 Med: provenance labels, `persistence::Error::Sqlite` prefix, and verify-panel prose were unscanned → registered). App tests 141→142. Closes Epic 2 (14/14).

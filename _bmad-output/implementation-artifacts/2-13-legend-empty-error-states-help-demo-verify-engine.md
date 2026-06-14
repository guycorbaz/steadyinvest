# Story 2.13: Legend, empty/error states, help, demo & verify-engine

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want guidance without a wizard and a way to trust the engine,
so that I can learn by exploration and verify correctness on demand.

## Acceptance Criteria

(From epics.md §Story 2.13, lines 712–725. BDD, verbatim intent. Scope-resolved 2026-06-14 — see Dev Notes "Scope decision".)

1. **(FR58 — actionable empty state)** **Given** any main surface with no data or an error, **when** it renders, **then** an **actionable empty state** is shown (the dashboard "no studies" line becomes a heading + neutral explanation + two calls-to-action: create a study (focuses the create field) and **load the demonstration study**), and the existing neutral error/feedback notices remain clear and neutral (FR13).
2. **(FR57 — legend)** **Given** the help surface, **then** a **consistent legend** explains every marker the app uses for **freshness** (current / stale dot `◦`), **provenance/source** (manuel / fournisseur / calculé), **coverage** (present / à remplir `▦` / non disponible), **review/trust** (`?` à revoir / `✓` validé) and **verdict confidence** (pleine / provisoire hachurée / suspendue) — each as a fact-stating neutral label beside its real visual marker, in one place.
3. **(FR62 — help/glossary + demo)** **Given** the help surface, **then** a non-blocking **contextual help / glossary** of the key SSG terms (neutral definitions) is available, **and** a **read-only demonstration study** can be loaded (the bundled `g01-worked-example`) and explored with the full faithful form + verdict — **without persisting anything to the journal** and **without accepting edits** (it is a look-only sample).
4. **(FR9-UI — verify engine)** **Given** the help surface, **when** I run **"Vérifier le moteur"**, **then** the app replays the **bundled golden studies** (Epic 1, `app/assets/golden/g01..g11`) through the engine via `core::golden::check_all`, and reports the **method version + fingerprint** and a **pass / deviation** result per fixture (any deviation listed with its path, expected, actual) — a green "all passed" or a named list of what drifted (FR9).
5. **(neutral voice, FR13)** **Given** every new label, empty-state copy, legend entry, glossary term and verify-engine result string, **then** they are **neutral, fact-stating** (no banned buy/sell/hold verb), `@tr` (UI) or registered `MSG_*`/label (Rust) so the posture gate scans them. The demo study's data is the worked-example's own values (not posture-scanned — fixture data).
6. **(DoD)** **Given** the Definition of Done for a UI story, **then** it is unit-tested (the demo-fixture → `contract::Study` conversion round-trips the worked-example's years/judgment; `core::golden::check_all` over the bundled fixtures returns all-passed at runtime; the empty-state CTA wiring; legend/glossary entries are present & neutral), the binary launches and runs the event loop, and the in-GUI click-through is a documented partial (human/AT-SPI, as 2.1–2.12). 4 CI gates green `--locked`; the golden-gate + drift gates stay green; **`core`/`contract`/`persistence`/`Cargo.lock`/`deny.toml`/`rust-toolchain` re-diff unchanged** (see Scope decision).

## Tasks / Subtasks

- [x] **Task 1 — Bundle & replay the golden fixtures: verify-engine (app crate)** (AC: 4)
  - [x] Embed the 11 bundled fixtures at compile time: a `const GOLDEN_FIXTURES: &[(&str, &str)]` of `(id, include_str!("../assets/golden/gNN-….json"))` in a new `app/src/viewmodel/verify.rs` (one `include_str!` per file — no new dependency, compile-time-checked paths). The golden-gate drift test already guarantees these are byte-identical to `core/tests/golden/`.
  - [x] `verify::run() -> VerifyReport`: parse each fixture via `serde_json::from_str::<steadyinvest_core::golden::GoldenStudy>` → collect → `core::golden::check_all(&studies)` → fold into a `VerifyReport { method_version, method_fingerprint, results: Vec<{id, passed, deviations: Vec<{path, expected, actual}>}> }`. Surface `core::method::METHOD_VERSION` + `core::method::method_fingerprint()` (or the existing public accessors). A parse error for any fixture is itself a reported failure (never a panic / silent skip).
  - [x] Headless tests: `verify::run()` over the real bundled fixtures returns **every fixture passed** (mirrors the golden-gate, but through the app's runtime path); a deliberately corrupted in-memory fixture string yields a non-passing result with a deviation/parse note (never a panic).
- [x] **Task 2 — Demonstration study: fixture → in-memory read-only `Study` (app crate)** (AC: 3)
  - [x] A converter `verify::demo_study() -> Result<contract::Study, String>` (or `viewmodel`): parse `g01-worked-example`, map `GoldenInput` → `contract::Study` — `FixtureYear` → `YearData` (each present amount wrapped in a `Cell` with a **provider/derived** provenance + `Coverage::Present`; absent → `None`/`ToFill`), `FixtureJudgment` → `contract::Judgment` (field-for-field — names match), `native_currency`, a fixed demo id/`created_at` (deterministic; the demo is never persisted so its id need not be journal-unique). Reuse `entry::tofill_cell` for gaps.
  - [x] Read-only by construction: the demo is held in an in-memory `Rc<RefCell<Option<Study>>>` (`demo_study`) in `main.rs` and rendered via the existing `push_form`. **`current_study` stays `None` while the demo is open**, so every edit/judgment/extend/review/drag callback hits its existing `else { return }` guard and no-ops — the form is look-only with zero new gating code. A `Studies.demo-active` flag drives a neutral "lecture seule — étude de démonstration" banner; "‹ Retour" clears `demo_study` + `demo-active` + `study-open`.
  - [x] Headless test: `demo_study()` returns a `Study` whose §2/§3 years + §4/§5 judgment equal the worked-example's input values (the conversion is faithful), and `engine::build_frame`/`snapshot_for` on it yields a verdict (the demo renders a real, coherent frame).
- [x] **Task 3 — The help & verification hub (Réglages screen)** (AC: 2, 3, 4)
  - [x] Build out `app/ui/screens/settings.slint` (today a placeholder) into a calm **"Aide & vérification"** hub with three collapsible/stacked panels, ink-only:
    - **Légende des marqueurs (FR57):** each state marker shown beside its real visual (reuse the actual glyph tokens / `TrustMarker` / a `ZoneSwatch`) + a neutral one-line meaning. Cover freshness (`◦` stale), coverage (`▦` à remplir, `n/a`), trust (`?`/`✓`), source (manuel/fournisseur/calculé), verdict confidence (pleine/provisoire-hachurée/suspendue).
    - **Glossaire (FR62):** a static list of key SSG terms with neutral definitions (e.g. "Zone d'achat", "Ratio hausse/baisse", "BPA", "Marge de sécurité", "Historique suffisant"). Data-driven from a Rust label table (posture-scanned) or `@tr` literals.
    - **Vérifier le moteur (FR9):** a button → calls `verify::run()` → renders `method_version` + `method_fingerprint` + a per-fixture line ("g01-worked-example — Réussi" / "… — Écart : {path} attendu {x}, obtenu {y}"). A summary line ("11/11 réussis" / "N écart(s)"). The check is fast (pure compute, Epic-1-proven sub-ms each) — run it inline on click; push the report into a `Studies`/`Verify` global.
  - [x] Wire the Rust callbacks (`main.rs`): `on_run_verify` → `verify::run()` → push the report; `on_load_demo` → build demo study → push_form + set `demo-active`. Keep these on a small `Verify`/`Help` global or extend `Studies` (consistency with the existing globals).
- [x] **Task 4 — Actionable empty state + demo entry (dashboard)** (AC: 1, 3)
  - [x] Replace the dashboard's calm "Aucune étude pour le moment." (shown when `study-count == 0`) with an **actionable empty state**: a heading + a neutral explanatory line + a "Créer une étude" affordance (focuses the ticker field) + a **"Charger l'étude de démonstration"** button (`Studies.load-demo()`). Ink only.
  - [x] Keep the existing neutral `notice` banner for errors (already FR13-neutral). Confirm the open-study-with-no-data case shows the faithful form's existing em-dash gaps (already calm) — no new empty state needed there unless trivially additive.
- [x] **Task 5 — Gates, posture floors, DoD** (AC: 5, 6)
  - [x] Register every new `@tr` label (legend/glossary/verify/empty-state) + any new Rust `MSG_*`/label-table string for the posture gate; bump the `@tr` floor and (if Rust strings added) the message-inventory count. Fixture data + the demo's worked-example values are NOT scanned.
  - [x] 4 CI gates green `--locked`; the **golden-gate** + **golden drift** + **method-fingerprint** tests stay green (this story does NOT change the method — it only *replays* the goldens). `core`/`contract`/`persistence`/`Cargo.lock`/`deny.toml`/`rust-toolchain` re-diff **unchanged**. File List ⇄ git exact (issue #18).
  - [x] DoD: launch + run the event loop; in-GUI click-through = documented partial (human/AT-SPI). Don't mark `[x]` for a non-existent test.

## Dev Notes

### Scope decision (Guy, 2026-06-14) — READ FIRST

1. **All four parts ship in 2.13** (legend FR57 + actionable empty/error states FR58 + help/glossary + read-only demo FR62 + verify-engine FR9) — the full epics story, not split.
2. **One hub in the Réglages (Settings) screen** holds the legend + glossary + verify-engine (today `settings.slint` is an empty placeholder, the natural home — a calm "Aide & vérification" surface outside the study flow). The **demo study loads from the dashboard empty-state CTA** ("Charger l'étude de démonstration").
3. **App-crate only.** `core` already ships the verify infrastructure: `core::golden::{check, check_all, GoldenStudy, GoldenReport, GoldenDeviation}` are **public, non-test API** (core/src/golden/mod.rs:29-37), the 11 fixtures are **bundled byte-identical** in `app/assets/golden/` (the golden-gate drift test enforces this), and the app already depends on `steadyinvest-core` + `serde_json`. ⇒ **NO `core`/`contract`/`persistence` change**, no new dependency, no method change (the method-fingerprint gate stays pinned — we *replay*, never redefine).

### Verify-engine — the infrastructure already exists (Epic 1 built it for THIS story)

- `app/assets/golden/README.md` states verbatim: *"the Story-2.13 'verify engine' screen replays at runtime via `core::golden::check` (FR9, ADD12)."*
- Public API: `core::golden::check_all(&[GoldenStudy]) -> Vec<GoldenReport>`; `GoldenReport { id, passed, deviations: Vec<GoldenDeviation> }`; `GoldenDeviation { path, expected, actual, relative_error }` (core/src/golden/compare.rs).
- Method identity: `core::method::method_fingerprint()` (SHA-256 over the pinned constants, core/src/method/mod.rs) + `core::method::METHOD_VERSION` / `core::method_version` (`"ssg-1.0.0"`). Surface both in the report.
- Bundle the fixtures with `include_str!("../assets/golden/gNN-….json")` (11 explicit entries) — compile-time-checked, no runtime FS, no new dep. List the 11 ids exactly (the drift test lists them).

### Demo study — read-only by construction (no journal pollution)

- `GoldenInput { native_currency, years: Vec<FixtureYear>, judgment: FixtureJudgment }` (core/src/golden/schema.rs:151). `FixtureYear` fields (year, sales, eps, high_price, low_price, dividend_per_share, pre_tax_profit, book_value_per_share) and `FixtureJudgment` fields **match `contract::YearData`/`Judgment` one-for-one** — a direct field map, wrapping each present `FixtureAmount` as a `Cell` (provider/derived provenance, `Coverage::Present`) and each absent as a `ToFill` gap (reuse `entry::tofill_cell`).
- **The read-only trick:** open the demo with `current_study == None`. Every edit rail (`on_commit_cell`, `on_set_judgment`, `on_extend_history`, `on_set_review`, `on_set_rationale`, the §1 drag, …) begins with `let Some(id_text) = current_study.borrow().clone() else { return; }` — so with no current study they all no-op. The demo renders fully (push_form + the engine frame) but accepts nothing and writes nothing. A `demo-active` flag only drives the "lecture seule" banner + hides the dashboard create form while open. "‹ Retour" exits.
- Do **not** persist the demo (`put_study` is never called) — it leaves the journal untouched, so archive/delete/list (2.12) never see it.

### What exists today (reuse — do not reinvent)

- **States/enums (FR57 source of truth):** `contract::cell` — `Source{Provider,Manual,Derived}`, `Freshness{Current,Stale}`, `Coverage{Present,ToFill,NotAvailableAccepted}`, `Review{None,ToReview,Validated}`; verdict `low_confidence: bool` (core/src/verdict.rs). Their **visual markers** live in `app/ui/components/editable_cell.slint` (gap `▦`, stale `◦`), `trust_markers.slint` (`?`/`✓` heavier-ring), `verdict_badge.slint` (zone hue / hatched / withheld). The legend must show the SAME markers (import the components / glyph tokens) — never a divergent re-drawing.
- **Source labels:** `app/src/viewmodel/entry.rs:137` ("manuel"/"fournisseur"/"calculé") — reuse for the legend's source row.
- **Notice banner + messages:** `Studies.notice` + the `MSG_*` inventory (`app/src/state.rs`) — already FR13-neutral; nothing to redo, just keep new strings neutral.
- **Nav + screens:** `app/ui/app.slint` (4 destinations incl. "Réglages" → `settings.slint`); dashboard ⇄ study via `Studies.study-open`. The empty state lives in `dashboard.slint` (the `study-count == 0` branch, post-2.12).

### Established conventions (carry forward)

- Cardinal Rule: no calculation in the app layer — verify-engine **calls `core`** (the math stays in core); the demo conversion is data-shaping, not arithmetic. No `.unwrap()`/`.expect()` in non-test code; no silent `.ok()`.
- Money/values cross as formatted strings; deviations cross as the already-human-readable `expected`/`actual` strings `core::golden` produces. No `Decimal`/enum into `.slint`.
- Colour budget: the hub spends **NO new saturated colour** — the legend reuses the existing markers (incl. the §4 zone hues, shown as the legend's verdict row); everything else is ink. Neutral microcopy (FR13).
- 4 CI gates `--locked`; `Cargo.lock`/`deny.toml` unchanged (no new dep); current app `#[test]` count **135** (you add to it).

### Recorded traps to avoid

1. **Don't redefine the method** — verify-engine *replays* the goldens through `core`. Touch nothing in `core/src/method` (the fingerprint test `f79e3c11…` must stay green) or `core/src/golden`. If a fixture fails at runtime, that's a real signal, not a thing to "fix" by editing core here.
2. **Demo must not persist** — never `put_study` the demo; keep `current_study == None` so edits no-op and nothing reaches the journal.
3. **Legend must mirror, not fork** — render the real glyph tokens / components, so the legend can never drift from what cells actually show.
4. **Posture: scan labels, not fixture data** — register the new `@tr`/`MSG_*` strings; the demo's worked-example numbers and the fixture JSON are data, not scanned.
5. **No new dependency** — `include_str!` + `serde_json` (already present) cover the fixture bundling; do NOT add `include_dir`/`rust-embed`.
6. **File List ⇄ git exact** (issue #18); don't mark `[x]` for a missing test.

### Project Structure Notes

- All work in `steadyinvest-app`. **No `core`/`contract`/`persistence` change.** No new dependency.
- New: `app/src/viewmodel/verify.rs` (fixture bundling + `run` + `demo_study`); build out `app/ui/screens/settings.slint`; a `Verify`/help global in `state.slint` (or extend `Studies`). Touch `app/src/main.rs` (callbacks), `app/ui/screens/dashboard.slint` (empty state), `app/src/posture.rs` (floors).
- Slint/Rust naming: components `PascalCase`, `.slint` `snake_case`, props/callbacks `kebab-case` (`run-verify`, `load-demo`, `demo-active`).

### Tech stack (pinned)

- Rust workspace MSRV **1.96**; **Slint 1.16.1**; `rusqlite 0.40`. Linux-only dev/CI. 4 gates `--locked`.

### References

- [Source: epics.md#Story 2.13] (712–725: BDD AC). [Source: prd.md] FR57 (legend), FR58 (actionable empty/error states), FR62 (help/glossary + read-only demo), FR9 (verify-engine UI).
- [Source: core/src/golden/mod.rs:29-37] public `check`/`check_all`/`GoldenStudy`/`GoldenReport`/`GoldenDeviation`. [core/src/golden/compare.rs] the comparator. [core/src/golden/schema.rs:122,151,172,222] `GoldenStudy`/`GoldenInput`/`FixtureYear`/`FixtureJudgment`. [app/assets/golden/README.md] the runtime-replay intent + the 11 ids. [core/tests/golden_gate.rs] golden + drift gates (stay green).
- [Source: core/src/method/mod.rs] `method_fingerprint()` (pinned `f79e3c11…`) + `METHOD_VERSION`. [Source: contract/src/cell.rs] the four state enums. [app/ui/components/{editable_cell,trust_markers,verdict_badge}.slint] the real markers the legend mirrors. [app/ui/screens/settings.slint] the placeholder to build out. [app/ui/screens/dashboard.slint] the `study-count == 0` empty-state branch (2.12).

## Open Questions (for Guy / dev — non-blocking, defaults chosen)

- **Q1 — Which fixture is the demo?** **Default:** `g01-worked-example` (the "tutorial-style worked example"). Confirm vs another fixture.
- **Q2 — Verify-engine async?** **Default:** run inline on click (pure compute, ~ms for 11 fixtures, Epic-1-proven). If it ever feels janky, move to a background task. Confirm inline is fine.
- **Q3 — Glossary depth?** **Default:** ~8–12 core SSG terms (zone d'achat, ratio U/D, BPA, marge de sécurité, historique suffisant, P/E jugé, provenance, fraîcheur). Confirm the set; it can grow later.
- **Q4 — Demo in the dashboard list, or only via the CTA?** **Default:** only via the empty-state CTA + a small "Charger la démonstration" affordance also reachable when studies exist (so it's not lost once the list is non-empty). The demo never appears as a journal row. Confirm.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- `cargo test --workspace --locked` all green (app 135→137; golden-gate + drift + method-fingerprint pinned tests stay green). `cargo clippy --all-targets --locked` clean. Binary launches + runs the event loop (exit 124).
- posture floors: `@tr` 177→212 (+35), `USER_FACING_MESSAGES.len()` 20→23.

### Completion Notes List

- **App-crate only** — `core`/`contract`/`persistence`/`Cargo.lock`/`deny.toml`/`rust-toolchain`/`app/assets` re-diff **empty** (verified). The verify infra (public `core::golden::check_all` + the 11 byte-identical bundled fixtures) and the method fingerprint were built in Epic 1 for exactly this story; we **replay**, never redefine. No new dependency.
- **Verify-engine (FR9):** `viewmodel::verify::run()` embeds the 11 fixtures via `include_str!`, parses each (a parse error is itself a reported non-passing fixture, never a panic/silent skip), runs `core::golden::check_all`, and folds into a `VerifyReport { method_version, method_fingerprint, per-fixture pass/deviation }`. Rendered in the Réglages hub.
- **Demo (FR62):** `verify::demo_study()` converts `g01-worked-example` (`GoldenInput` → `contract::Study`, fields map 1:1) into an in-memory study. Opened with `current_study == None` → every one of the ~22 edit/mutation callbacks early-returns (read-only by construction, verified by the Edge reviewer); `push_view_state(default)` + `reset_undo()` so it renders entry-regime/all-open with disabled undo/redo; a "lecture seule" banner; never persisted.
- **Legend (FR57) + glossary (FR62):** built out `settings.slint` into an "Aide & vérification" hub — `LegendRow`s show the REAL marker tokens (présent, `▦`, `n/a`, `◦`, `?`, `✓`, `△`) beside neutral meanings + a provenance/verdict prose line; 8 neutral glossary terms.
- **Empty state (FR58):** dashboard `study-count == 0` → heading + explanation + "Charger l'étude de démonstration" CTA; `study-count > 0 && rows == 0` → "no match"; existing notice banner unchanged.
- **Posture (FR13):** new `MSG_VERIFY_*`/`MSG_DEMO_UNAVAILABLE` registered + scanned; all new `@tr` neutral. Fixture/demo data never scanned.
- 2 new headless tests (verify all-pass at runtime; demo conversion faithful + yields a frame).
- AC6 in-GUI click-through left as a documented partial (human/AT-SPI sandbox), as 2.1–2.12.

### Senior Developer Review (AI)

3-layer adversarial review (Blind + Edge Case + Acceptance), 2026-06-14.

- **Acceptance Auditor: ACCEPT — AC1–AC6 + scope (a–d) all PASS.** Independently verified app-only diff, no new dep, fingerprint pinned/green, named tests assert the AC claims.
- **Edge Case Hunter: the read-only-by-construction safety holds** — enumerated all ~22 mutation callbacks; every one guards on `current_study == None`.
- **5 patches applied:**
  - [x] [MED] demo rendered with stale/empty folds+regime → `push_view_state(StudyViewState::default())` in `on_load_demo` (entry regime, all open).
  - [x] [LOW/MED] undo/redo appeared enabled on the demo → `reset_undo()` in `on_load_demo`.
  - [x] [LOW] `verify_summary` `total - passed` underflow guard → `saturating_sub`.
  - [x] [LOW] added a "présent" (no-marker) legend row (AC2 completeness).
  - [x] [LOW] fixed a comment string drift ("réussis" → "réussies").
- **Dismissed:** demo drops per-amount `FixtureAmount.currency` (benign — single-security fixtures are mono-currency, native_currency; demo is display-only). Other confirmed-safe items (verify count/dedup, demo→real-study no form bleed, verify pure-compute no journal).

### File List

- `app/src/viewmodel/verify.rs` — NEW: fixture bundling + `run()` (verify-engine) + `demo_study()` (+ conversion helpers) + 2 tests
- `app/src/viewmodel/mod.rs` — register `verify`
- `app/src/state.rs` — `MSG_VERIFY_PASSED`/`MSG_VERIFY_DEVIATIONS`/`MSG_DEMO_UNAVAILABLE` + `verify_summary` + inventory
- `app/src/main.rs` — `Verify.on_run` + `Studies.on_load_demo` callbacks (push_view_state default + reset_undo); demo reset on real open
- `app/src/posture.rs` — `@tr` floor 177→212, message inventory 20→23
- `app/ui/state.slint` — `Verify` global + `FixtureLine` struct; `load-demo()`/`demo-active` on `Studies`
- `app/ui/app.slint` — re-export `Verify` + `FixtureLine`
- `app/ui/screens/settings.slint` — the "Aide & vérification" hub (LegendRow + GlossaryRow + 3 panels)
- `app/ui/screens/dashboard.slint` — actionable empty state + demo CTA + filtered-empty line
- `app/ui/screens/study_screen.slint` — read-only demo banner + Retour clears `demo-active`

### Change Log

- 2026-06-14 — Story 2.13 implemented: Réglages help hub (legend + glossary + verify-engine), actionable empty state, read-only demo study. App-crate only; core/contract/persistence/assets unchanged. app tests 135→137. 3-layer review ACCEPT; 5 patches applied. Status → done.

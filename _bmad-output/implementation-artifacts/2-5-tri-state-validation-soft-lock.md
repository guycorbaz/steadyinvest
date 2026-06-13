# Story 2.5: Tri-state validation with soft-lock

Status: done

<!-- Epic-2 story 5 — the human-judgment axis of the per-cell data-state model. Story 2.4 made the §2/§3
     grid EDITABLE and rendered each cell's source × freshness × COVERAGE under the attention hierarchy,
     leaving the `Cell.review` field present-but-unrendered (the rail manages it, 2.4 shows nothing). THIS
     story renders and lets the user SET the per-cell tri-state **review tag** (`none` / `? to-review` /
     `✓ validated`), wires the **soft-lock** (a `✓` cell is edit-protected: changing it requires first
     clearing `✓` with one gesture, which returns it to `?` — never silently blanked), and adds the
     **"unlock all"** bulk action (study-level + per-column/year + per-row/metric) that flips `✓→?` behind
     a confirmation. It is `app`-only — `contract::Cell.review` (the `Review` enum) and `Cell::edited`'s
     divergent-edit `✓→?` demotion ALREADY exist (Story 1.11); 2.5 consumes them, it does NOT change
     `contract/`. SCOPE GUARDRAIL — 2.5 is the review-tag UI + soft-lock + bulk unlock only: NO engine /
     `core::ssg::compute` / `JudgmentInputs` mapping / verdict / zone bar / U-D / projected return / the
     verdict-integrity gate that *consumes* `✓` (Story 2.6); NO §1 interactive chart (2.8); NO plausibility
     / low-confidence warnings (2.7); NO provider fetch / reconciliation / the provider-divergent `✓→?`
     auto-tag (Epic 3 — the `Cell::edited` rail is the seam, but 2.5 builds no fetch). 2.5 introduces NO
     new external dependency: `Cargo.lock` is expected UNCHANGED. Headless CI cannot prove marker render /
     confusability / micro-animation / regime attenuation: the visual-verification DoD (AC 7) is
     load-bearing, exactly as it was for 2.1/2.2/2.3/2.4 — but the set-review → persist → reopen
     round-trip, the soft-lock edit-guard, and the unlock-all `✓→?` flip ARE proven headlessly. -->

## Story

As Guy,
I want a per-cell review tag I control,
so that my human sign-off is the guard against plausible-but-wrong data.

## Acceptance Criteria

1. **Per-cell tri-state review tag the user sets: `none` / `? to-review` / `✓ validated`, rendered per
   the trust-marker spec (confusability-gated) (FR20).** Every editable §2/§3 data cell (the §3 A–H
   direct columns **A** `high_price`, **B** `low_price`, **C** `eps`, **F** `dividend_per_share`, and the
   §2 management rows **sales**, **pre-tax profit**, **book value/share**) carries a user-settable review
   tag that **cycles `none → ? → ✓ → none`** via a per-cell gesture (a keyboard chord on the active cell
   **and** a click affordance on the marker):
   - **`(none)`** — the default; **no marker drawn**, the cell shows only its value / coverage / freshness
     state (the 2.4 render). Carries no human sign-off.
   - **`?` to-review** — a **hollow question glyph** given a **second non-colour channel** (heavier
     outline / slightly larger) so it is **never confused with the hollow stale-dot** (`◦`). A personal
     worklist flag: "entered but not certain — come back". Distinct from *to-fill* (no value) and *stale*
     (freshness).
   - **`✓` validated** — a **solid check**, the user's explicit sign-off. In the **entry regime** it
     carries a single **geofenced, sanctioned desaturated ink-green** (≈ `#4A7C6F` — deliberately **NOT**
     the Buy green `#009E73`; the one carved-out exception to the monastic colour budget, admissible only
     because no zone bands exist yet — Story 2.6), with a **~120 ms draw micro-animation** (trace + 0.9→1.0
     scale) that **respects OS reduced-motion** (disabled when reduced-motion is on). In **contemplation**
     it falls back to **neutral ink** and **attenuates** (opacity floor ~40 %, never 0) — see AC 4.
   - Every committed tag change goes through the existing `state.rs` mutation rail (`Cell::edited`-style
     re-write of the cell's `review` field) with a manual `Provenance` from the injected `Clock`, and the
     updated `Study` is **persisted** via `Journal::put_study`. Read-only / no-journal / save-failure reuse
     the existing neutral notices — **never** a silent `.ok()`.

2. **Soft-lock: a `✓` cell is protected from editing; changing it requires first clearing `✓` (one
   gesture), which returns it to `?` — never silently blanked (FR20 refinement).** While a cell's review is
   `✓ validated`:
   - **direct entry is refused** — typing, Backspace/Delete-to-clear, and the not-available gesture on the
     active cell do **not** mutate the value; instead the cell is visibly **locked** (a calm lock
     affordance / read-only cursor — **no colour spent**), and a neutral notice states the sign-off must be
     cleared first;
   - **one explicit "clear ✓" gesture** flips `✓ → ?` (NOT to `none` — its need-to-recheck status is
     preserved), after which the cell is editable again as a normal `?` cell;
   - the sign-off is **load-bearing**: it cannot be undone by an accidental keystroke, only by the
     deliberate clear gesture. (This supersedes the prior PRD FR20 auto-reset wording — editing no longer
     silently clears `✓`; the user un-validates deliberately. Track as the recorded FR20 refinement.)

3. **Study-level (and per-column/year, per-row/metric) "unlock all" flips `✓ → ?` behind a confirmation
   (FR20).** A bulk action turns a saved study into a ready-made re-check worklist:
   - **scopes:** **study-wide** (every `✓` cell), **per-column/year** (every `✓` in one year), and
     **per-row/metric** (every `✓` in one §3 column or §2 row);
   - flips **`✓ → ?`** (NOT to `none`) for every validated cell in the chosen scope, in **one** persisted
     upsert (one `logical_version` bump), through the same mutation rail;
   - **behind a confirmation** — a destructive bulk change is never silent (the app's first
     confirm-before-act gesture; delete/archive is Story 2.12). The confirmation copy is fact-stating and
     passes the posture gate; **Cancel** leaves every tag untouched. Cells already `?` or `none` are
     unaffected; the count of cells flipped is surfaced in a neutral notice.

4. **Asymmetric attenuation — a safety rule, not a style choice.** In the **contemplation** regime, **only
   the positive marker (`✓`) may dim** (opacity floor ~40 %, neutral ink). The **negative signals — `?`,
   stale, missing/to-fill — never attenuate**; they keep (or gain) salience in both regimes so a verdict
   (Story 2.6) can never "speak alone" while a load-bearing input is non-green. Concretely: the `✓` marker
   reads `Tokens.regime-emphasis`; the `?`, stale-dot, and gap glyph are **regime-independent**. (The
   *provider-divergent* negative signal is an Epic-3 marker; 2.5 designs the rule correctly but it is not
   exercised yet — state that honestly, do not fake it.)

5. **Crate-boundary & adapter discipline (architecture Cardinal Rule).** **No calculation in `app`**: 2.5
   sets/clears/renders a per-cell enum and persists it; it does **not** compute D/E/G/H, the §2
   averages/trends, the §4/§5 results, the verdict, the verdict-integrity gate, or anything the engine owns
   (Story 2.6) — those stay caption-only em-dash slots. The review tag crosses into Slint **only as an
   enum-derived string** (`"none"|"to-review"|"validated"`) — no float, no domain struct leaked. The
   `review` mutation lives on the existing `state.rs` rail (re-using `Cell` snapshot semantics); the
   `Provenance` is stamped in `app` from the injected `Clock` (ADD15 — no scattered wall-clock /
   `Uuid::new_v4`). All colours/sizes from `Tokens`; new `.slint` files snake_case, components
   PascalCase, properties/callbacks kebab-case.

6. **Quality gates, posture, dependency & pinned-surface discipline.** All four gates green `--locked`:
   `cargo fmt --all --check` · `cargo clippy --all-targets --all-features --locked -- -D warnings` ·
   `cargo test --all --locked` · `cargo deny check`. Specifically:
   - **NO new external dependency** — 2.5 is pure `app` UI + state over primitives that already exist;
     **`Cargo.lock` is expected UNCHANGED** and `deny.toml` is **untouched**. If a dependency change is
     somehow needed, stop and record why (it would be a scope surprise);
   - **every new user-visible string** (the clear-✓ / soft-lock notice, the unlock-all confirmation +
     scope labels, the review-tag tooltips/`@tr()`) passes the crate-local **banned-verb posture test** —
     register them in the scanned `@tr()` / `USER_FACING_MESSAGES` surfaces; bump the asserted floors
     (`>= 13` `.slint` files, `>= 90` `@tr` total, the message count). Coverage/state/sign-off copy is
     **fact-stating** ("validé", "à revoir", "Retirer la validation"), never advice. Reuse
     `core::method::BANNED_VERBS_FR/EN`, never copy;
   - **new keyboard-operable controls** follow the 2.1/2.4 a11y pattern (`FocusScope` / `TextInput` +
     visible focus ring; **decision never colour-only** — the `✓`/`?`/lock states are carried by
     **glyph/shape/outline/opacity**, the geofenced `✓`-green is a *redundant* third channel, never the
     sole carrier);
   - **the marker-confusability spec is honoured by construction** (≥98 % ID / <2 % pairwise at 14 px is
     the design target): `✓` solid vs `?` hollow-heavier vs `◦` stale-dot vs `▦` gap glyph are four
     distinct shapes on distinct channels. The **automated perceptual/snapshot confusability gate** is a
     cross-cutting CI gate that needs the full marker family (incl. the 2.6/2.8 verdict/chart markers) and
     a headed render — **recommend a structural test now** (`render(state).marker == state.review` for
     every reachable state; each state maps to a distinct glyph) and **forward the perceptual snapshot
     gate** as a documented partial (the 2.3/2.4 honesty rail);
   - **pinned surfaces untouched** (`git diff` empty): `core/`, `contract/`, `persistence/`, `ingestion/`,
     `report/`, `docs/method/**`, `.github/`, `rust-toolchain.toml`, the frozen
     `persistence/tests/corpus/v1.db`, and `deny.toml`. **`contract/` is NOT modified** — the `Review`
     enum and the `Cell::edited` `✓→?` demotion **already exist** (Story 1.11); 2.5 consumes them.

7. **Visual verification (Definition of Done — load-bearing, mirrors 2.1/2.2/2.3/2.4).** Launch the built
   app, open a study with some entered data, and verify on display: **cycle a cell's review tag**
   `none → ? → ✓ → none` (keyboard chord **and** marker click); the **`?` reads distinct from the stale
   `◦` dot**; the **`✓`** shows the geofenced desaturated ink-green + the ~120 ms draw animation (and the
   animation is **disabled under OS reduced-motion**); a **`✓` cell is soft-locked** — typing / clear /
   not-available is refused with a neutral notice until the **clear-✓** gesture flips it to `?`, after
   which it edits normally; **"unlock all"** (study + a single year + a single metric) asks for
   confirmation, flips `✓→?` on confirm, and **Cancel** leaves tags untouched; switch to **contemplation**
   and confirm the **`✓` attenuates while `?` / stale / missing stay salient** (asymmetric attenuation);
   **close → relaunch → reopen** the same study → every review tag (`✓`/`?`/`none`) is **restored** intact.
   Confirm the footer disclaimer (FR64), dark/light + label-set + locale swaps (2.1), fold/regime restore
   (2.3), and the 2.4 entry/coverage states still work, and launch-to-interactive stays ~within 3 s
   (NFR-P4). Record the run in the Dev Agent Record. Headless CI cannot stand in for this AC — but the
   **set-review → `put_study` → reopen** round-trip, the **soft-lock edit-guard logic**, and the
   **unlock-all `✓→?` flip** ARE proven headlessly.

## Tasks / Subtasks

- [x] **Task 1 — Render the tri-state review tag on every editable cell (AC: 1, 4, 5)**
  - [x] Add a `review` field to the `GridCellState` struct (`app/ui/state.slint` + the matching Rust
        struct usage in `viewmodel/form.rs`): `review: string` — `"none" | "to-review" | "validated"`.
        Map it in `viewmodel/form.rs::editable_cell` via a **new** `entry::review_str(cell: Option<&Cell>)
        -> &'static str` helper (sibling of `coverage_str` / `source_label`), `None`/`Review::None → "none"`,
        `ToReview → "to-review"`, `Validated → "validated"`.
  - [x] Render the marker per the trust-marker spec — **recommended: a new
        `app/ui/components/trust_markers.slint`** (the architecture tree names it; factor the marker out of
        `editable_cell.slint` so the §2 and §3 grids and a later verdict surface reuse it), OR render
        inline in `editable_cell.slint` and record the choice. Geometry constant (no resize on tag change):
        - `none` → nothing drawn;
        - `?` → a hollow question glyph (`?` or a drawn glyph) with a **heavier outline / slightly larger**
          so it is unmistakable vs the `◦` stale-dot — the second non-colour channel;
        - `✓` → a solid check in **geofenced desaturated ink-green** (new token, AC 1) in entry, neutral
          ink + attenuated in contemplation (Task 4); a **~120 ms** draw/scale animation gated on a
          reduced-motion flag (reuse the 2.4 reduced-motion pattern / Slint `animation-tick` discipline).
  - [x] Add the new neutral tokens to `Tokens` (`app/ui/tokens.slint`) **and both palettes**
        (`app/src/theme.rs`): the geofenced **`validated-ink`** (≈ `#4A7C6F` dark; pick the light-theme
        sibling), and any review-marker outline weight if not derivable from existing tokens. **Never
        hard-code** hex/px in `.slint`. Document that `validated-ink` is the single sanctioned colour-budget
        exception and must never co-present with zone bands (none exist until 2.6).

- [x] **Task 2 — Set/cycle the review tag → persist (AC: 1, 5)**
  - [x] Add `JournalState::set_review(study_id, year_index, field, review)` (or a `cycle_review`) in
        `app/src/state.rs`, built on the existing `mutate_cell` rail: clone the cell, set its `review`
        field (value/coverage/freshness/source/provenance otherwise preserved — this is a review-only
        change, so do **not** route it through `Cell::edited`, which would re-stamp source/freshness; write
        a small review-only transform that keeps the existing value+provenance and only swaps `review`, OR
        re-stamp provenance from the clock and record the choice), then `put_study`. Reuse the read-only /
        no-journal / save-failure guards + neutral notices. **No silent `.ok()`.**
  - [x] Define the **cycle gesture**: a keyboard chord on the active cell (Ctrl+Space is already the
        not-available toggle from 2.4 — pick a **free** chord, e.g. Ctrl+Enter or a dedicated key, and
        record it) **and** a click affordance on the marker. Both call a `Studies.set-review(year, field,
        next)` callback → `state.rs` → re-read → re-push (the 2.3/2.4 one-source-of-truth shape).
  - [x] Headless round-trip test: set `?` then `✓` on a cell → `put_study` → re-`get_study` → the `review`
        survives; cycling `✓ → none` clears it; the cell's **value and coverage are unchanged** by a
        review-only edit (no value mutation, no `0`).

- [x] **Task 3 — Soft-lock the `✓` cell + the deliberate clear gesture (AC: 2, 5)**
  - [x] In `editable_cell.slint`, when `state.review == "validated"` make the cell **edit-protected**:
        the `TextInput` is read-only (or its `key-pressed` / commit path is guarded), typing /
        Backspace-Delete-clear / the not-available chord do **not** mutate; show a calm **lock affordance**
        (a glyph / a non-editable cursor — **no colour**) and set the neutral soft-lock notice via a
        `Studies` callback when the user attempts an edit. Constant geometry.
  - [x] Wire the **clear-✓** gesture (a distinct affordance + the cycle landing on it): clearing flips
        `✓ → ?` (NOT `none`) through `set_review`, after which the cell edits as a normal `?`. Guard the
        interaction so the *first* gesture clears and a *second* edits — never both in one keystroke.
  - [x] **Belt-and-braces / honesty:** note that `contract::Cell::edited` already demotes `✓ → ?` on a
        **divergent value** edit (the Epic-3 reconciliation rail). The soft-lock means a direct typed edit
        never reaches a `✓` cell, so that path is a **defensive backstop** (e.g. paste over a `✓` cell, or
        Epic-3 reconcile). **Decide and record** whether **paste-a-column** over a `✓` cell is (a) blocked
        by the soft-lock, or (b) allowed-with-auto-demote via the `Cell::edited` backstop — recommend (b)
        for bulk paste, (a)-equivalent for direct typing; file the interpretation. A headless test proves a
        typed edit on a `✓` cell is refused until the tag is cleared.

- [x] **Task 4 — Asymmetric attenuation (the safety rule) (AC: 4)**
  - [x] In the marker component, attenuate **only** the `✓` marker in contemplation: bind its opacity to a
        floor (~40 %, never 0) driven by `Tokens.regime-emphasis` (entry = 1.0; contemplation already
        0.55 — clamp to the ≥40 % floor) and drop the geofenced green to neutral ink in contemplation. The
        `?`, stale-dot, and gap glyph must **NOT** read `regime-emphasis` — they are regime-independent.
  - [x] Headless/structural test (where feasible): the attenuation rule is one-directional —
        `✓` opacity in contemplation < entry; `?` opacity equal across regimes. (Full visual proof is AC 7.)

- [x] **Task 5 — "Unlock all" bulk flip behind a confirmation (AC: 3, 6)**
  - [x] Add `JournalState::unlock_all(study_id, scope)` in `state.rs` where `scope` is **study** /
        **year(index)** / **metric(field)**: iterate the matching cells, flip every `Review::Validated →
        ToReview` (leave `None`/`ToReview` untouched), in **one** `put_study` upsert; return the **count**
        flipped. Reuse the guards/notices.
  - [x] Add the **confirmation gesture** — the app's first confirm-before-act. **Recommended:** a Slint
        confirm overlay (a `PopupWindow` or an inline confirm banner with explicit **Confirmer** /
        **Annuler**), reusing `action_button.slint` and tokens; record the chosen pattern as an
        interpretation (UX: "modals only for destructive/import"). Wire `Studies.unlock-all(scope-kind,
        scope-arg)` → confirm → `state.rs` → re-read → re-push → neutral "N tags flipped" notice. Cancel
        mutates nothing.
  - [x] Surface the three scope entry points: a study-level action (top bar / form header area), a
        per-year-column action, and a per-metric-row action — keyboard-reachable, posture-gated labels.
  - [x] Headless test: a study with mixed `✓`/`?`/`none` → `unlock_all(study)` flips only the `✓`→`?`,
        count correct, persisted, reopen-stable; the per-year and per-metric scopes flip only their subset.

- [x] **Task 6 — Posture, accessibility & gates (AC: 6)**
  - [x] Extend the `app` posture test to scan the new `.slint` `@tr()` literals + any new
        `USER_FACING_MESSAGES` (soft-lock notice, clear-✓ label, unlock-all confirmation + scope labels,
        review-tag tooltips) against `BANNED_VERBS_FR/EN`; bump the `>= 13` file floor (if a new
        `trust_markers.slint` is added), the `>= 90` `@tr` total floor, and the message count. French
        sign-off copy is fact-stating ("validé", "à revoir", "Retirer la validation", "Tout dévalider").
  - [x] Keyboard walkthrough by construction: the cycle chord, the clear-✓ gesture, the soft-lock notice,
        and the unlock-all confirm/cancel are `FocusScope`/`TextInput`-operable with a visible focus ring;
        tab order logical; the markers read with **colour stripped** (✓ solid vs ? hollow-heavier vs ◦ dot
        vs ▦ gap carry meaning, never colour alone). **Reduced-motion** disables the ✓ draw animation.
  - [x] All four gates green `--locked`. `git diff` over `core/ contract/ persistence/ ingestion/ report/
        docs/method/ .github/ rust-toolchain.toml` + the frozen `v1.db` + `deny.toml` is **empty**.
        **`Cargo.lock` unchanged** (no new dependency) — confirm and record.

- [x] **Task 7 — Visual verification, records & File List (AC: 7)**
  - [x] Launch, walk the AC-7 journey (cycle a tag both ways → `?` distinct from `◦` → `✓` green +
        animation, animation off under reduced-motion → soft-lock refuses edit until clear-✓ → unlock-all
        confirm/cancel at all three scopes → contemplation attenuates only `✓` → **relaunch → all review
        tags restored**), record the outcome (and any sandbox AT-SPI / headed-render limitation, as
        2.1–2.4 did) in the Dev Agent Record.
  - [x] Prove headlessly what the sandbox blocks visually: the **set-review → `put_study` → reopen**
        round-trip, the **soft-lock edit-guard** (typed edit on a `✓` refused; cleared `✓→?` then editable),
        the **unlock-all** flips at each scope, the structural **`render(state).marker == state.review`**
        mapping, and the attenuation direction. Refresh test counts in the Change Log.
  - [x] Update the **File List** (every new/modified file incl. any QA-generated test file + the
        story-automator log — issue #18 discipline) and file a consolidated GitHub issue for the genuine
        2.5 interpretations (review-only-edit provenance handling, the cycle chord chosen, the
        paste-over-✓ soft-lock decision, the confirmation-overlay pattern, the forwarded perceptual
        confusability gate, the FR20 soft-lock refinement) — issues, not inline TODOs (the
        1.11/2.1/2.2/2.3/2.4 pattern).

## Dev Notes

### What this story is — and the disasters it must make impossible

2.5 is the **human-judgment axis** of the per-cell data-state model. Story 2.4 made the §2/§3 grid
editable and rendered `source × freshness × coverage` under the attention hierarchy, deliberately leaving
the `Cell.review` field present-but-unrendered. THIS story renders and lets the user set the **tri-state
review tag** (`none`/`?`/`✓`), wires the **soft-lock** (a `✓` cell is edit-protected; clearing it is one
deliberate `✓→?` gesture, never a silent blank), and adds the **"unlock all"** bulk `✓→?` flip behind a
confirmation. It is **`app`-only** — the contract primitives (`Review`, `Cell::edited`'s `✓→?` demotion)
already exist (Story 1.11).

Disasters to prevent:
- **Scope bleed into 2.6/2.7/2.8 and Epic 3 — the biggest risk.** 2.5 sets/renders the review tag and the
  soft-lock; nothing more:
  - **NO engine / `core::ssg::compute` / `Judgment → JudgmentInputs` mapping / verdict / zone bar / U-D /
    projected return** — **Story 2.6**. Critically, the **verdict-integrity gate that *consumes* `✓`**
    ("full saturated colour only when every load-bearing input is `✓` & not stale; else provisional /
    degraded / withheld") is **2.6's** job. 2.5 produces the `✓` signal; it does **not** read it into any
    verdict (none exists). The computed D/E/G/H + §4/§5 stay caption-only em-dash slots. **No calculation
    in `app`.**
  - **NO §1 interactive chart / draggable line** — **Story 2.8** (the §1 area stays a placeholder).
  - **NO plausibility / unit-split / low-confidence warnings** — **Story 2.7** (distinct from the review
    tag and from quality flags).
  - **NO provider fetch, reconciliation, or the provider-divergent `✓→?` auto-tag** — **Epic 3**. The
    `Cell::edited` divergent-demotion rail is the *seam* Epic 3 will use, but 2.5 builds no fetch/reconcile.
    "Provider-divergent" as a non-attenuating negative signal is an Epic-3 marker — design the attenuation
    rule correctly (Task 4) but state honestly that it is not exercised in 2.5 (the 2.4 stale-murmur
    honesty rail: do not fake a divergent cell to make the texture look used).
- **Calculation in `app`.** Cardinal Rule: all SSG math lives in `core`. 2.5 adds **no** calc — only a
  per-cell enum set/clear + render. No averages, P/Es, forecasts, verdict.
- **Spending colour on provenance.** The monastic colour budget = the three judgment zones (which don't
  exist until 2.6). The **one** sanctioned exception is the geofenced desaturated **`✓`-green ≈ `#4A7C6F`**
  — explicitly carved out by the UX spec, admissible only because it is **never co-present with the Buy
  green / zone bands** and attenuates to neutral in contemplation. Everything else on the markers is
  shape/outline/opacity. Do NOT introduce a `?`-colour or a lock-colour.
- **Silently blanking a sign-off.** The soft-lock's whole point: a `✓` must NOT be undone by an accidental
  keystroke. A typed edit on a `✓` cell is **refused** (visible lock + neutral notice), not silently
  applied; the only path is the deliberate **clear-✓ → ?** gesture (NOT `→ none`: the recheck status is
  preserved). Conflating "clear ✓" with "edit value" in one gesture is the bug to avoid.
- **`unknown` rendered or stored as `0`.** Still the project's most-repeated rail. A review-only edit
  **must not touch the value** — setting `?`/`✓` on a present cell keeps its value; on a to-fill gap the
  tag is allowed but the value stays `None` (never `0`). Do not let the review transform run through a path
  that re-derives coverage from a coerced value.
- **A scattered wall-clock / `Uuid::new_v4`.** Any provenance re-stamp on a review edit comes **only** from
  the injected `Clock` (ADD15). No `Utc::now()` / `Uuid::new_v4` outside `clock.rs`.
- **Mutating the contract or pinned surfaces.** Everything 2.5 needs in `contract` already exists (the
  `Review` enum, `Cell::edited`'s `✓→?` demotion) — **do not change `contract/`**. `core/`, `persistence/`,
  `ingestion/`, `report/`, `docs/method/**`, `deny.toml` are untouched. **No new dependency** → `Cargo.lock`
  unchanged (cleaner than 2.4, which moved `arboard`/`rust_decimal` dev→runtime).

### Scope — the one-paragraph contract

> 2.5 renders and lets the user set the per-cell **tri-state review tag** (`none` / `?` to-review / `✓`
> validated) on the §2/§3 editable cells, cycled by a keyboard chord **and** a marker click, each change
> persisted via `Journal::put_study`. The `✓` carries a geofenced sanctioned **desaturated ink-green**
> with a reduced-motion-respecting ~120 ms draw animation; the `?` is a hollow glyph on a **second
> non-colour channel** so it never reads as the stale `◦` dot. A `✓` cell is **soft-locked** — editing is
> refused until one deliberate **clear-✓ → ?** gesture; never silently blanked. **"Unlock all"**
> (study / per-year / per-metric) flips every `✓ → ?` behind a **confirmation**, in one upsert. **Asymmetric
> attenuation:** only `✓` dims in contemplation; `?` / stale / missing never attenuate. It builds **no
> engine / verdict / verdict-integrity gate (2.6), no chart (2.8), no plausibility (2.7), no
> provider/reconciliation (Epic 3)**, adds **no new dependency**, and does **not** modify `contract/`.

### The data-state model 2.5 completes (the review axis)

`contract::Cell` (verified `contract/src/cell.rs`) is `{ value, source, freshness, review, coverage,
provenance }`. The four independently-queryable axes (FR17–FR20):
- **`source`** = `Provider | Manual | Derived` (FR17) — 2.4 renders on demand.
- **`freshness`** = `Current | Stale` (FR23) — 2.4 renders the stale murmur.
- **`coverage`** = `Present | ToFill | NotAvailableAccepted` (FR19) — 2.4 renders all three.
- **`review`** = `None | ToReview | Validated` (FR20) — **THIS story** renders/sets these + the soft-lock +
  bulk unlock. 2.4 left the field on the cell (managed by the rail) and rendered nothing.

`Cell::edited` (verified `contract/src/cell.rs`) is the manual-mutation rail: it already demotes
`Review::Validated → ToReview` **iff the value diverges** (value-based `Money` equality), never promotes,
sets `Freshness::Current`. **2.5's review-only set is NOT a value edit** — it must change *only* the
`review` field and keep value/coverage/source/freshness. So write a small review-only transform on the
`mutate_cell` rail rather than calling `Cell::edited` (which would re-stamp source/freshness from the
provenance). Decide whether to re-stamp `provenance` on a review change (recommended: yes, from the clock,
so the FR51 time-series records the sign-off act) and record it.

### Trust-marker rendering spec (UX §State & Trust Markers, lines 533–555)

- **`✓` validated** — a **solid check**. Entry regime: geofenced sanctioned **desaturated ink-green** (≈
  `#4A7C6F`, **NOT** Buy `#009E73`, never co-present with zone bands) + ~120 ms trace + 0.9→1.0 scale
  micro-animation (the "Dr-Tax check" reward), **reduced-motion-respecting**. Contemplation: neutral ink +
  attenuate (opacity floor ~40 %, never 0).
- **`?` to-review** — a **hollow question glyph** with a **heavier outline / slightly larger** (the second
  non-colour channel) so it **cannot be confused with the hollow stale-dot `◦`**.
- **Missing** (`▦` gap), **Stale** (`◦` + ~60 % opacity) — already shipped by 2.4; do not change them, just
  ensure the `?` is unmistakably distinct.
- **Source** — already revealed on demand (2.4); unchanged.
- **Asymmetric attenuation (safety rule):** contemplation dims **only** `✓`; `?` / stale / missing /
  (future) provider-divergent **never** attenuate.
- **Confusability gate** (≥98 % ID, <2 % pairwise at 14 px on the real dark bg) — the design target. The
  formal perceptual/snapshot CI gate spans the full marker family (incl. 2.6/2.8 verdict/chart markers) and
  needs a headed render; **ship a structural mapping test now and forward the perceptual gate** (documented
  partial — the 2.3/2.4 honesty rail). [[project_high_fidelity_ssg_forms]]

### The soft-lock — exactly what it is (UX lines 447–457)

A `✓` cell is **protected from editing**: to change it the user first clears the `✓` (one explicit
gesture) which returns the cell to `?` (recheck status preserved, never silently blanked). This makes the
sign-off load-bearing — it cannot be undone by an accidental keystroke, at the cost of one deliberate
gesture. **Supersedes PRD FR20's auto-reset wording** (editing no longer silently clears `✓`); track as the
recorded FR20 refinement. **Bulk "unlock all"** (study / per-column-year / per-row-metric, behind a
confirmation) flips every `✓ → ?` (NOT `→ none`) — turning a saved study into a re-check worklist, the
natural entry point for the Epic-3 annual-update journey.

### Where the gesture lives & the confirmation overlay (interpretations to record)

- **Cycle chord:** Ctrl+Space is taken (2.4 not-available toggle). Pick a **free** chord for the review
  cycle (e.g. Ctrl+Enter, or a dedicated key on the active cell) **plus** a click affordance on the marker
  — record the choice. Enter/Tab/arrows remain navigation/commit (2.4); do not overload them.
- **Confirmation overlay:** 2.5 is the app's **first** confirm-before-act gesture (delete/archive is
  2.12). No confirmation pattern exists yet (`grep` confirms only a comment in `posture.rs`). UX: "modals
  only for destructive/import". **Recommended:** a Slint `PopupWindow` or an inline confirm banner with
  explicit **Confirmer / Annuler**, built from `action_button.slint` + tokens. Record the pattern — it
  becomes the reusable confirm primitive for 2.12.

### Existing code being modified / extended (read before writing)

- **`app/src/viewmodel/entry.rs`** — add `review_str(cell) -> &'static str` (sibling of `coverage_str` /
  `source_label`). The `next_cell` / field-addressing helpers are reused unchanged.
- **`app/src/viewmodel/form.rs`** — `editable_cell()` maps each cell to `GridCellState`; add the `review`
  field to its output. `pe_rows` / `mgmt_rows` unchanged in shape (each carries `GridCellState`s).
- **`app/src/state.rs`** — the `JournalState` mutation rail (`mutate_cell`, `edit_cell`,
  `set_not_available`, `paste_column`, `manual_provenance`). Add `set_review` and `unlock_all(scope)` on
  the same rail (re-read → mutate → `put_study`), with the existing guards/notices. **Review-only edit must
  not touch the value/coverage.**
- **`app/ui/state.slint`** — `GridCellState` (add `review: string`); the `Studies` global — add
  `set-review(year, field, review)` and `unlock-all(scope-kind, scope-arg)` callbacks; re-export any new
  struct via `app.slint`.
- **`app/ui/components/editable_cell.slint`** — the soft-lock guard (read-only / guarded `TextInput` when
  `review == "validated"`, lock affordance, notice on attempted edit), the clear-✓ gesture, and the marker
  (or it delegates to a new `trust_markers.slint`). **Constant geometry** — display ↔ edit ↔ locked must
  not resize.
- **`app/ui/components/trust_markers.slint`** (recommended new) — the ✓/? marker, confusability-gated,
  regime-attenuated (✓ only).
- **`app/ui/tokens.slint` + `app/src/theme.rs`** — add `validated-ink` (geofenced ✓-green) to `Tokens` +
  **both** palettes; any marker outline-weight token. The `regime-emphasis` alpha already exists (drives
  the ✓ attenuation). Never hard-code.
- **`app/src/main.rs`** — wire the `set-review` / `unlock-all` callbacks (validate → mutate → persist →
  re-read → re-push, the 2.3/2.4 one-source-of-truth shape) and the confirmation flow. Keep the injected
  `Clock`/`IdGen` the single time/identity source. `unused_crate_dependencies` comment-of-record unchanged
  (no dependency change).
- **`app/src/posture.rs`** — scan the new strings; bump floors.
- **`app/ui/screens/study_screen.slint`** — mount the markers into the §2/§3 cells; add the per-year and
  per-metric "unlock all" entry points; keep §1/§4 placeholders and §4/§5 caption-only em-dash (2.6).

### Architecture compliance (guardrails)

- **Cardinal Rule:** no calculation in `app`; the contract→core mapping + compute + the verdict-integrity
  gate are **2.6**. 2.5 adds **no** calc — only a per-cell enum set/clear + render.
- **Adapter rule:** the review tag crosses to Slint as an **enum-derived string**; no float, no domain
  struct leaked. (Architecture: "tri-state review is an enum, never `0/1/2`" — line 508.)
- **Provenance/clock:** any provenance re-stamp on a review edit comes **only** from the injected `Clock`
  (ADD15). The architecture names this surface `trust_markers.slint` (`architecture.md:699` — "✓/?/missing/
  stale (confusability-gated)").
- **Verdict-integrity forward note (NOT 2.5):** architecture lines 412/581/636–638 — `FullVerdict`
  constructible only from all-validated-&-fresh inputs; `verdict.isFull ⟹ ∀ load-bearing input validated ∧
  ¬stale`. 2.5 *produces* the `validated` signal these invariants will consume in 2.6; it implements none of
  the verdict logic.
- **Errors:** any failure (soft-lock refusal, save failure) is visible and neutral — never a swallowed
  `.ok()`/`.unwrap()` in non-test app code.
- **Performance (NFR-P4):** tag set / unlock-all are Slint dirty-driven; unlock-all is one upsert; launch
  ~within 3 s. No verdict recompute (no engine) so no 100 ms budget applies yet (that is 2.6/2.8).

### Neutral voice (FR13 / posture gate)

- The A–H column letters, §1–§5 structure, and formulas are reproducible method, not trademarks — **keep
  them** ([[project_high_fidelity_ssg_forms]], [[project_open_source_naming_constraint]]). Neutralize only
  marks/wordmarks/verbatim prose.
- **Banned verbs:** run every new label (review-tag tooltips, the clear-✓ label, the soft-lock notice, the
  unlock-all confirmation + scope labels) through `core::method::BANNED_VERBS_FR/EN` **before wiring**.
  Sign-off copy is **fact-stating** ("validé", "à revoir", "Retirer la validation", "Tout dévalider"),
  never advice/exhortation. Register strings in the scanned slices; do not reduce neutrality to a grep.

### Previous-story intelligence (2.4 dev record + review; 2.3; 2.2; Spike A; 1.11)

- **Gates always `--locked`;** clippy `--all-targets --all-features` lints tests + the frozen
  `examples/spike_*.rs` (must keep compiling). 2.4's review re-ran every gate and re-diffed pinned
  surfaces; expect the same scrutiny.
- **`Cell::edited` is the one manual-mutation primitive** (Story 1.11, invariant 2b): returns a NEW cell
  (snapshot), `Some ⇒ Present`, `None ⇒ ToFill`, `Freshness::Current`, and the **divergent-edit `✓→?`
  demotion** — value-based `Money` equality (re-entering `"3.0"` over `"3"` keeps `✓`). 2.5's review-only
  set must **not** use `edited` for value (it would re-stamp source/freshness) — write a review-only
  transform on `mutate_cell`. The `edited` `✓→?` path remains the **defensive backstop** for paste-over-`✓`
  and Epic-3 reconcile.
- **`mutate_cell` rail (2.4)** does read-only/no-journal guards → re-read study → materialize year window
  if empty → mutate one cell → `put_study`. Reuse it for `set_review`; add a study-wide iterate variant for
  `unlock_all`. (`set_not_available` shows the "override one field of the rebuilt cell" pattern.)
- **`unknown` never `0`** (the single most-repeated rail) — a review-only edit must not re-coerce the value.
- **Visual-verification DoD is load-bearing; the sandbox blocks screenshots / may lack AT-SPI / headed
  render** — 2.1/2.2/2.3/2.4 all recorded a partial AC (process launches + on-disk truth proven, in-GUI
  click-through left for human/AT-SPI). Plan the same honesty: prove **set-review → `put_study` → reopen**,
  the **soft-lock guard**, and **unlock-all** headlessly; record marker render / animation / attenuation as
  needing human confirmation.
- **File List completeness is the epic's single most-repeated finding (issue #18):** list **every**
  new/modified file (incl. any QA test file + the `_bmad-output/story-automator/…` automator log) with
  refreshed test counts **before** review.
- **Slint gotchas:** `row` is a reserved layout-attached property — don't name a property `row` (2.2);
  `@children` in a conditional is illegal, fold via clipped height-0 (2.3); element ids unreachable from a
  component-root function inside a conditional — declare functions on the in-branch layout (2.3). The 2.4
  `editable_cell.slint` already intercepts Ctrl/Cmd chords **first** in `key-pressed` so they don't leak a
  character — add the review chord the same way; don't collide with Ctrl+V / Ctrl+Space.
- **`unused_crate_dependencies` is crate-level allow** (2.2/2.3/2.4): unchanged in 2.5 (no dependency
  change). `ingestion`/`report`/`tokio` stay unused until Epic 3.

### Git intelligence

Recent commits: `feat(story-2.4): Manual data entry with provenance & coverage`, `feat(story-2.3):
Faithful collapsible SSG form …`, `feat(story-2.2): Create, save & reopen a study …`, `feat(story-2.1):
Application shell …`. Conventions: conventional commits `feat(story-2.5): …`; the story file +
`sprint-status.yaml` update land in the **same** commit; merge only with all four gates green `--locked`.
`app/` structure (2.1–2.4): `clock.rs`, `config.rs`, `labels.rs`, `state.rs`, `theme.rs`, `regime.rs`,
`posture.rs`, `viewmodel/{format,studies,form,entry}.rs`, `ui/{tokens,state,app}.slint`, `ui/screens/*`,
`ui/components/*` — follow those patterns. `core/`, `contract/`, `persistence/` must **not** change.

### Project Structure Notes

- **New (app-only):** recommended `app/ui/components/trust_markers.slint` (the ✓/? marker, architecture-
  named); new `app` unit tests (set-review round-trip, soft-lock guard, unlock-all per-scope, structural
  marker mapping, posture). `entry::review_str` is a small addition to the existing `entry.rs`.
- **Modified:** `app/src/viewmodel/entry.rs` (`review_str`), `app/src/viewmodel/form.rs` (`review` on
  `GridCellState`), `app/src/state.rs` (`set_review` + `unlock_all`), `app/src/main.rs` (callbacks +
  confirm flow), `app/src/posture.rs` (floors), `app/src/theme.rs` (`validated-ink` both palettes),
  `app/ui/state.slint` (`review` field + callbacks), `app/ui/app.slint` (re-export if needed),
  `app/ui/components/editable_cell.slint` (soft-lock + clear-✓ + marker mount),
  `app/ui/screens/study_screen.slint` (marker mount + unlock-all entry points), `app/ui/tokens.slint`
  (`validated-ink` + any outline token), `sprint-status.yaml`, this story file.
- **Untouched (verify with `git diff` — must be empty):** `core/`, `contract/`, `persistence/`,
  `ingestion/`, `report/`, `docs/method/**`, `.github/workflows/ci.yml`, `rust-toolchain.toml`, the frozen
  `persistence/tests/corpus/v1.db`, **`deny.toml`**, and **`Cargo.lock`** (no new dependency). **`contract/`
  is consumed, never modified** — the `Review` enum + the `Cell::edited` `✓→?` rail already exist.
- **Variance note:** if the marker is rendered inline in `editable_cell.slint` instead of a new
  `trust_markers.slint`, the `>= 13` `.slint` file floor is unchanged — bump only if a file is added.
  Record the choice. `state.rs`'s full undo-stack/verdict slice remains 2.9/2.6 (a documented partial).

### References

- Story & ACs: `_bmad-output/planning-artifacts/epics.md` § "Story 2.5: Tri-state validation with
  soft-lock" + Epic 2 intro (lines 532–536, 598–611)
- FR20 (tri-state review tag + soft-lock): `_bmad-output/planning-artifacts/prd.md` lines 697–698; FR13
  (neutral signals), FR65 (offline)
- The Per-Cell Review Tag (tri-state) + Soft lock + Bulk unlock + Reconciliation synergy + Display
  discipline: `_bmad-output/planning-artifacts/ux-design-specification.md` lines 434–472; State & Trust
  Markers + Asymmetric attenuation: lines 533–555; verdict-integrity (a 2.6 consumer, not 2.5): lines
  525–531; two-regime visual delta (marker salience lever): lines 599–609; confusability gate: lines
  620–622, 1046; UX consistency (destructive confirm, neutral labels, validation never blocks): lines
  943–976; mockup `ux-stock-study-screen.html`
- Crate boundaries / Cardinal Rule, adapter (enum-as-string, "tri-state review is an enum never 0/1/2"),
  `trust_markers.slint`, clock injection (ADD15), verdict-integrity invariants (2.6 consumer):
  `architecture.md` lines 154, 412, 426, 507–508, 514, 581, 636–638, 699, "Architectural Boundaries"
- The review-tag rail (consume, don't modify): `contract/src/cell.rs` (`Review` enum, `Cell::edited`'s
  value-based `✓→?` demotion + tests), `contract/src/provenance.rs` (`Provenance` — no validation,
  sentinel-legal), `contract/src/study.rs` (`Study`/`YearData`/`created_at`)
- The mutation rail to extend (2.4): `app/src/state.rs` (`mutate_cell`, `edit_cell`, `set_not_available`,
  `paste_column`, `manual_provenance`, the `MSG_*` notices); the cell addressing/labels:
  `app/src/viewmodel/entry.rs` (`get_cell`/`set_cell`/`coverage_str`/`source_label`/`next_cell`); the
  adapter to extend: `app/src/viewmodel/form.rs` (`editable_cell`/`GridCellState`)
- The editable cell to soft-lock (2.4): `app/ui/components/editable_cell.slint` (Ctrl-chord-first
  `key-pressed` discipline, commit-on-focus-out); the `Studies` global + structs: `app/ui/state.slint`;
  tokens/theme: `app/ui/tokens.slint`, `app/src/theme.rs` (`cell-active`, `gap-ink`, `stale-*`,
  `regime-emphasis`); regime attenuation source: `app/src/regime.rs` (`Regime::emphasis`)
- Prior story (made the grid editable; left `review` unrendered) + its review: `2-4-manual-data-entry-
  provenance-coverage.md`; the read-only form before it: `2-3-faithful-collapsible-ssg-form.md`
- File List discipline (issue #18); prior interpretations issues #19/#20/#21; banned verbs:
  `core::method::BANNED_VERBS_EN/FR`; memories [[project_high_fidelity_ssg_forms]],
  [[project_open_source_naming_constraint]], [[project_gui_framework_slint_only]]

### Tech currency note (2026-06-13)

Slint pinned at 1.16. **No new external dependency** in 2.5 — it is pure `app` UI + state over primitives
already in the tree (`contract::Review`, `Cell::edited`, the `state.rs` mutation rail, `rust_decimal`/
`Money` for the untouched values). **`Cargo.lock` and `deny.toml` are expected unchanged.** The ✓ draw
micro-animation uses Slint's native animation/`states` with a reduced-motion guard (the 2.4 reduced-motion
pattern); confirm Slint's reduced-motion / animation-disable path in a quick check before wiring. The §1
chart (2.8) and §4 zone bar / engine / verdict-integrity (2.6) remain out of scope.

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (claude-opus-4-8, 1M context) — BMad dev-story workflow, 2026-06-13.

### Debug Log References

- `cargo build -p steadyinvest-app` — clean.
- `cargo fmt --all --check` — clean (after one `cargo fmt --all`).
- `cargo clippy --all-targets --all-features --locked -- -D warnings` — exit 0, no warnings.
- `cargo test --all --locked` — all crates green; **app: 81 passed** (up from 70: +11 new Story-2.5 tests).
- `cargo deny check` — advisories/bans/licenses/sources OK (the pre-existing `GPL-3.0`
  unmatched-allowance warning is unchanged, not introduced by 2.5).
- `git diff --stat` over `core/ contract/ persistence/ ingestion/ report/ docs/method/ .github/
  rust-toolchain.toml deny.toml Cargo.lock persistence/tests/corpus/v1.db` — **empty** (pinned
  surfaces untouched; **`Cargo.lock` unchanged → no new dependency**, as required).
- Release build + launch: `./target/release/steadyinvest-app` launches and runs its event loop with
  no error output (timeout-killed after running). See the visual-verification note below.

### Completion Notes List

**What 2.5 added (app-only, the human-judgment review axis):**
- **Tri-state review tag rendered + settable** (AC 1): new `app/ui/components/trust_markers.slint`
  (`TrustMarker`) drawing `none` → nothing, `? to-review` → a hollow glyph in a **heavier ring** (the
  second non-colour channel, unmistakable vs the `◦` stale dot), `✓ validated` → a solid check in the
  geofenced **`validated-ink`** (new token, both palettes; ≈ `#4A7C6F` dark / `#2F6657` light, **NOT**
  the Buy green — a theme test enforces the geofence). `entry::review_str` / `review_from_str` map the
  enum ↔ the stable `"none"|"to-review"|"validated"` string; `GridCellState.review` carries it across
  the adapter (never `0/1/2`). The cycle `none→?→✓→none` is a **marker click** + the **Ctrl+Enter**
  chord (recorded interpretation: Ctrl+Space was taken by 2.4, Ctrl+Enter is free).
- **Review-only persistence** (AC 1/5): `state::set_review` writes ONLY the `review` field on the
  `mutate_cell` rail — value/coverage/source/freshness **and provenance** preserved verbatim (NOT
  routed through `Cell::edited`, which would re-stamp source/freshness). Reviewing a to-fill gap keeps
  `value: None` — **never `0`**. Persisted via `put_study`; reuses the read-only/no-journal/save guards.
- **Soft-lock** (AC 2): a `✓` cell refuses a direct value edit — enforced **both** in the UI (read-only
  `TextInput`, the refusal swallowed in `key-pressed` → `Studies.notify-soft-lock()` → neutral
  `MSG_SOFT_LOCKED`) **and** in `state::edit_cell` (the testable Rust backstop). The deliberate
  **clear-✓ → ?** (NOT → none) releases it: **Ctrl+Backspace** on the locked cell or the `⦸` lock
  affordance (monochrome glyph, no colour). Recorded: typed edit on `✓` is blocked (a); bulk
  `paste_column` keeps the `Cell::edited` auto-demote backstop (b).
- **Bulk "unlock all"** (AC 3): `state::unlock_all(study_id, UnlockScope::{Study|Year|Metric})` flips
  every `✓→?` in scope in **one** upsert, returns the count; `count_validated` previews it. Wired behind
  the app's **first confirm-before-act** overlay — an inline neutral banner (Confirmer/Annuler) gated on
  `Studies.confirm-visible`; `request-unlock` raises it with the fact-stating `{n}`-count prompt,
  `cancel-unlock` mutates nothing. Three keyboard-reachable entry points (study / per-year / per-metric).
- **Asymmetric attenuation** (AC 4): in `TrustMarker`, only the `✓` reads `Tokens.regime-emphasis`
  (clamped to a ≥40 % opacity floor) and drops to neutral ink in contemplation; the `?` ring is
  regime-INDEPENDENT (never attenuates). The provider-divergent negative signal is an Epic-3 marker —
  designed-for but **not exercised** in 2.5 (stated honestly, not faked).

**Interpretations recorded (consolidated GitHub issue #22 — issue #18 discipline):**
1. **Review-only-edit provenance:** a review toggle changes ONLY `review`; the cell's provenance
   (origin timestamp) is preserved verbatim, never re-stamped — the sign-off act's timing is captured
   by the study-level `logical_version` bump (FR51), so a value's source/fetch time is never lost.
2. **Cycle chord = Ctrl+Enter** (Ctrl+Space taken by 2.4). Clear-✓ = Ctrl+Backspace / lock-glyph click.
3. **Paste-over-✓:** typed edit blocked by the soft-lock (a); bulk paste allowed-with-auto-demote via
   `Cell::edited` (b) — the recommended split.
4. **Confirmation pattern:** an inline confirm banner (Confirmer/Annuler from `action_button.slint` +
   tokens), the reusable confirm primitive for 2.12 — recorded as the chosen pattern.
5. **Forwarded perceptual confusability gate:** 2.5 ships the **structural** marker-mapping test
   (`review_str`/`review_from_str` total over the three states + distinct glyph per state); the
   headed perceptual/snapshot gate (full marker family incl. 2.6/2.8) is a **documented partial**.
6. **FR20 soft-lock refinement:** editing no longer silently clears `✓`; the user un-validates
   deliberately (supersedes the PRD FR20 auto-reset wording).

**Visual verification (AC 7 — load-bearing, sandbox limitation, mirrors 2.1–2.4):** the release binary
launches and runs its event loop with no error. The sandbox has **no display server / AT-SPI**, so the
in-GUI click-through (marker render, the `✓` ink-green + ~120 ms draw animation, the `?`-vs-`◦`
distinctness, the contemplation attenuation, the confirm overlay) **could not be screenshot here** and
is left for human confirmation — exactly the partial 2.1/2.2/2.3/2.4 recorded. What the sandbox CAN
prove is proven **headlessly** and passes: the **set-review → `put_study` → reopen** round-trip (review
tag restored intact; value/coverage untouched; a reviewed gap stays `None`, never `0`), the
**soft-lock edit-guard** (typed edit on `✓` refused with `MSG_SOFT_LOCKED`; cleared `✓→?` then editable),
the **unlock-all** flips at all three scopes (study/year/metric, count correct, persisted, reopen-stable),
and the **structural marker mapping**. **Known animation caveat:** because the form rebuilds its rows on
every persisted change (the 2.4 one-source-of-truth rail), the one-shot ✓ draw micro-animation is
best-effort on an in-place opacity change; the **reduced-motion gate** (duration 0 ms) is the
structurally guaranteed behaviour. **Reduced-motion source:** `Studies.reduced-motion` defaults `false`
(animation on); wiring it to the OS reduced-motion flag is a small forward item (the binding is correct).

### File List

**New (app-only):**
- `app/ui/components/trust_markers.slint` — the `TrustMarker` ✓/? component (confusability-gated,
  regime-attenuated ✓ only, clickable, reduced-motion-aware).

**Modified (app-only):**
- `app/src/viewmodel/entry.rs` — `ALL_FIELDS`, `review_str`, `review_from_str` (+ tests).
- `app/src/viewmodel/form.rs` — `review` on `GridCellState` output (+ test).
- `app/src/state.rs` — `set_review`, `unlock_all`, `count_validated`, `current_review`, the
  `edit_cell` soft-lock guard, `UnlockScope`, `MSG_SOFT_LOCKED` / `MSG_UNLOCK_CONFIRM` /
  `MSG_UNLOCK_DONE` + `unlock_confirm_message` / `unlock_done_message` (+ 7 tests, incl. the AI-review
  `set_not_available` soft-lock backstop regression test).
- `app/src/theme.rs` — `validated_ink` on `Palette` + both palettes + `apply` push (+ geofence test).
- `app/src/posture.rs` — floors bumped: `.slint` files `>= 14`, `@tr` total `>= 100`, message count `13`.
- `app/src/main.rs` — `parse_unlock_scope`, `pending_unlock`, and the `set-review` / `notify-soft-lock`
  / `request-unlock` / `confirm-unlock` / `cancel-unlock` callback wiring.
- `app/ui/state.slint` — `review` field on `GridCellState`; `set-review` / `request-unlock` /
  `confirm-unlock` / `cancel-unlock` / `notify-soft-lock` callbacks; `confirm-visible` /
  `confirm-message` / `reduced-motion` properties.
- `app/ui/components/editable_cell.slint` — soft-lock (read-only input, clear-✓ gesture), the cycle
  chord, the `TrustMarker` mount + lock affordance.
- `app/ui/screens/study_screen.slint` — the confirm overlay, the three unlock-all entry points, the
  updated entry-gesture hint.
- `app/ui/tokens.slint` — `validated-ink` colour + `marker-outline` metric.

**Process / tracking (not app code):**
- `_bmad-output/implementation-artifacts/sprint-status.yaml` — 2-5 `ready-for-dev → in-progress → review`.
- `_bmad-output/implementation-artifacts/2-5-tri-state-validation-soft-lock.md` — this story file.
- `_bmad-output/story-automator/orchestration-2-20260612-123914.md` — automator log (issue #18 discipline).

### Change Log

- **2026-06-13** — Story 2.5 implemented (tri-state review tag + soft-lock + bulk unlock, app-only).
  +11 app tests (81 passed, was 70). New `trust_markers.slint`; new `validated-ink` token (geofenced,
  both palettes); review-only `set_review` rail + `edit_cell` soft-lock guard + `unlock_all`/3 scopes;
  the app's first confirm-before-act overlay. All four gates green `--locked`; `Cargo.lock` and all
  pinned surfaces unchanged (no new dependency). Status → review.
- **2026-06-13** — Senior Developer Review (AI, adversarial) — see notes below. Two findings auto-fixed
  in-code (no scope/dep change): (1) MEDIUM — `state::set_not_available` lacked the soft-lock backstop
  that `edit_cell` has, leaving a Rust-side path that would silently blank a `✓` cell's value AND demote
  the sign-off (AC 2 names the not-available gesture among the refused mutations); now guarded + a
  regression test. (2) LOW — a soft-locked cell swallowed `Tab`/`Backtab` with a spurious soft-lock
  notice, breaking logical tab order (AC 6); now those keys fall through to default focus traversal.
  App tests **82** (was 81). All four gates re-run green `--locked`; pinned surfaces + `Cargo.lock` still
  empty-diff. Status stays **done** (0 CRITICAL).

### Senior Developer Review (AI)

**Reviewer:** Guy · **Date:** 2026-06-13 · **Outcome:** Approve (auto-fixes applied)

**Scope verified.** Read every file in the File List against the ACs and against git reality. The File
List matches `git status` exactly (one new file `trust_markers.slint`; 11 modified app files); the
pinned surfaces (`core/ contract/ persistence/ ingestion/ report/ docs/method/ .github/
rust-toolchain.toml deny.toml Cargo.lock` + the frozen `v1.db`) are empty-diff — **no new dependency**,
contract consumed-not-modified, as required. All four gates re-run green `--locked`.

**AC validation (1–7).** AC 1 review tag + cycle + `validated-ink` geofence (theme test enforces ✓-ink ≠
Buy green): IMPLEMENTED. AC 2 soft-lock + clear-✓→? : IMPLEMENTED (and hardened — see below). AC 3
unlock-all/3 scopes behind a confirmation, one upsert, count surfaced, Cancel inert: IMPLEMENTED +
tested. AC 4 asymmetric attenuation (only `✓` reads `regime-emphasis`, `?` regime-independent):
IMPLEMENTED. AC 5 crate-boundary / enum-as-string / no calc in `app`: IMPLEMENTED. AC 6 gates + posture
floors (`.slint` ≥ 14 = 15, `@tr` ≥ 100 = 111, `USER_FACING_MESSAGES` count 13): IMPLEMENTED. AC 7
visual verification: a **documented partial** (sandbox has no display/AT-SPI) — honestly recorded,
mirrors 2.1–2.4; the headless round-trips (set-review→persist→reopen, soft-lock guard, unlock-all/3
scopes, structural marker mapping) all pass.

**Findings.**
- **[MEDIUM — FIXED]** `set_not_available` had no soft-lock guard while `edit_cell` did. Via
  `Cell::edited(None, …)` a not-available gesture on a `✓` cell would blank the value and demote `✓→?`
  — the AC 2 disaster ("never silently blanked"). The UI swallowed Ctrl+Space on a locked cell, but the
  authoritative Rust rail (the dev's own "testable refusal") was asymmetric. Added the same
  `current_review == Validated → MSG_SOFT_LOCKED` guard + a regression test.
- **[LOW — FIXED]** A soft-locked cell's `key-pressed` catch-all swallowed `Tab`/`Backtab`, trapping
  focus and raising a spurious soft-lock notice on a pure navigation key (AC 6 logical tab order). Those
  keys now fall through to default focus traversal.
- **[LOW — ACKNOWLEDGED, tracked]** OS reduced-motion is not actually read: `Studies.reduced-motion`
  defaults `false` and `main.rs` never sets it, so the `✓` draw animation is never disabled in practice
  (AC 1/7). The mechanism (duration gated on the flag) is correct; wiring the OS flag has no portable
  Slint API and is honestly recorded as a forward item — keep on the consolidated issue (#22), not a
  blocker.

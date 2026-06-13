# Story 2.1: Application shell, theme & always-visible disclaimer

Status: done

<!-- Epic 2 opener — the FIRST real UI story of the project. It replaces the Story-1.1 placeholder
     window with the durable application shell every later Epic-2 story plugs into: nav rail + top
     bar + footer disclaimer, the token design system (dark default + light), French UI via @tr(),
     app-config persistence (window + theme), and the two runtime-swap mechanisms (NAIC↔neutral
     label set, locale number format). NO journal access, NO study screen, NO engine call.
     Headless CI cannot prove the perceptual ACs — the visual-verification DoD is load-bearing. -->

## Story

As Guy,
I want a calm native shell with nav, theming and the educational disclaimer,
so that I can move between Studies/Watchlist/Portfolio/Settings and always see the app's neutral posture.

## Acceptance Criteria

1. **Application shell renders and navigates.** On launch the main window shows a **persistent left
   nav rail** with exactly four destinations — Études / Liste de suivi / Portefeuille / Réglages
   (Studies / Watchlist / Portfolio / Settings) — and a **top bar** (UX-DR19, "Navigation Patterns").
   Selecting a destination switches the content area to that screen; the **active destination is
   clearly indicated** (not by colour alone — e.g. indicator bar + ink step, NFR-U1 discipline).
   The three not-yet-built screens (Studies, Watchlist, Portfolio) are neutral placeholders (screen
   title + a calm fact-stating line; the actionable empty states of FR58 are Story 2.13). Navigation
   is **fully keyboard-operable with an always-visible focus indicator** (NFR-U2). The app is
   deliberately shallow: no breadcrumbs, no nested routing.

2. **Token design system, dark default + light, swappable at runtime (UX-DR1/2/6/7).** A Slint
   global (e.g. `Tokens`, PascalCase per the naming pattern) is the **single source of truth** every
   component reads — **no hard-coded colour/size anywhere in `ui/`** (governance rule). It carries
   the two token families:
   - *colour/alpha* (freely swappable at runtime): the full **dark ink scale** (bg `#0E0F12`,
     surface `#16181D`, surface-alt `#1C1F26`, separator `#2A2E37`, text-high `#ECEEF2`, text-mid
     `#B8BDC7`, text-low `#8A8F98`) and **light ink scale** (bg `#FBFBFC`, surface `#FFFFFF`,
     surface-alt `#F4F5F7`, separator `#E2E4E9`, text-high `#14161A`, text-mid `#3F454F`, text-low
     `#6B7280`), plus the three judgment-zone hues (Buy `#009E73`, Hold `#E69F00`, Sell `#D55E00`)
     and their per-theme zone alphas (dark 32–40 % / light 15–18 %) — defined NOW so later stories
     consume them, even though no zone renders in 2.1;
   - *metric/typo* (quasi-static, never swapped during interaction): 4 px spacing scale
     (4·8·12·16·24·32·48), type scale (verdict 28 / H2 18 / H3 15 / body 14 / caption 12), grid row
     height 28 px, flat elevation (1 px borders, no shadows).
   **Dark is the default.** Switching to light at runtime restyles the whole shell with **no
   restart and no geometry change** (only the colour/alpha family swaps). A Rust-side
   `app/src/theme.rs` owns the canonical theme state and pushes it into the `Tokens` global
   (single neutral source of truth, UX-DR7).

3. **French UI via `@tr()`, i18n-ready (UX-DR29).** Every user-visible string in `.slint` is
   wrapped in `@tr()` with **French source text** (UI language axis — strictly distinct from the
   NAIC↔neutral label-set axis of AC 5). No translation catalogs ship yet; the extraction pipeline
   (`slint-tr-extractor` → `.pot`, bundled translations when a second language lands) is documented
   in the crate doc or README so it is a drop-in later, not a refactor.

4. **Footer disclaimer always visible on every screen (FR64).** A footer carries the educational
   disclaimer (French, e.g. « Outil éducatif — ne constitue pas un conseil financier. ») on **all
   four destinations**, pinned outside any scrollable area so it can never be scrolled away. Neutral
   ink (caption scale), calm — visible, not shouting.

5. **Label set (NAIC↔neutral) and locale number format are runtime-swappable (FR63, no wizard).**
   - `app/src/labels.rs`: the NAIC↔neutral **runtime-swappable data table** (a lookup keyed by
     stable identifiers → per-set display strings), exposed to the UI via a Slint global. Seed it
     with a small honest set (e.g. the app/window title block and the four nav destination labels
     are NOT label-set entries — they are UI strings; seed instead the first method-vocabulary
     terms that exist today, e.g. "SSG"/"Étude d'action" or equivalent — exact seed dev
     discretion). Switching the set updates the UI live, no restart.
   - **Locale number format setting** (decimal comma vs point, thousands separator — NFR-X2,
     configurable independently of OS locale) stored in app-config, plus a Rust formatting helper
     in the app crate (e.g. `viewmodel/format.rs`) that later stories (2.4 grid, money-as-strings)
     reuse. In 2.1 a sample formatted number on the Settings screen proves the live swap.
   - A minimal **Réglages (Settings) screen** hosts the three controls — theme (dark/light), label
     set, locale format — as plain panels, **no wizard, nothing blocking** (FR63). Provider/key,
     currency and threshold panels are later stories (3.2, 4.x).

6. **Window size, theme and (later) fold/regime state persist across launches.** App-config lives
   in the per-machine config dir via `directories` (`ProjectDirs`), serialized with serde
   (`serde_json` is already a workspace dep) — **never inside the journal** (ADD7; the journal is
   not even opened in this story). Persist now: window size (and maximized flag if the Slint API
   exposes it cleanly), theme choice, label-set choice, locale format. The config struct is
   **forward-extensible**: every field `#[serde(default)]`, unknown fields tolerated (the
   "user data is tolerant" rail — retro lesson 5), so 2.3 can add fold/regime state without a
   migration. A **missing or corrupt config file falls back to defaults and never blocks launch**
   (validate-before-trust; the app must not crash on its own config). On relaunch the window
   reopens at the persisted size with the persisted theme.

7. **Typography foundation bundled and verified (UX-DR4 + the UX forward-note).** Bundle **Inter
   (400/600)** for UI text and a **tabular-figures-by-default numeric font** (e.g. IBM Plex Sans /
   Source Sans 3 — weights 400/500/600) under `app/assets/fonts/` with their OFL license files.
   Register them with Slint (`.ttf` import in `.slint` or `register_font_from_memory` — dev
   discretion) and set the shell's default font family from the tokens. **Run the deferred UX
   week-1 check now:** render a column of digits in the numeric font and visually confirm tabular
   (fixed-width) figures in Slint — do NOT rely on `font-feature-settings: "tnum"` over Inter.
   Record the verdict (font chosen, tabular confirmed yes/no) in the Dev Agent Record; if no
   candidate renders tabular, fall back to a monospaced-digits face and file a GitHub issue.

8. **Posture & quality gates.** A crate-local **banned-verb posture test** in `app` (the
   1.9/1.10/1.11 local-gate pattern) scans every new user-visible string — the `@tr()` literals in
   `ui/**/*.slint` (read the files from the test) and the `labels.rs` table — against
   `core::method::BANNED_VERBS_EN/FR` (reuse, never copy). All four gates green `--locked`:
   `cargo fmt --all --check` · `cargo clippy --all-targets --all-features --locked -- -D warnings` ·
   `cargo test --all --locked` · `cargo deny check`. **Pinned gates untouched:** method fingerprint
   (`f79e3c11…1d1d`), determinism hash (`eb45e761…d34f`), Spike-C digest, golden gate, persistence
   pinned-JSON snapshot + frozen `tests/corpus/v1.db`, `METHOD_VERSION` (`ssg-1.0.0`),
   `SCHEMA_VERSION` (= 1) — this story must not touch `core/`, `contract/`, `ingestion/`,
   `persistence/`, `report/`, `docs/method/**` at all.

9. **Visual verification (Definition of Done — load-bearing, retro §3.4).** Launch the built app
   and visually verify on display: dark shell renders per the ink scale; nav switches all four
   screens; disclaimer visible on each; theme toggles live to light and back; label-set and locale
   swaps update live; window size + theme survive an app restart; a **minimum window size** is
   enforced (UX-DR28) so the shell stays legible; launch-to-interactive feels within ~3 s (NFR-P4).
   Record the run in the Dev Agent Record. Headless CI cannot stand in for this AC.

## Tasks / Subtasks

- [x] Task 1 — Token design system + typography (AC: 2, 7)
  - [x] Create `app/ui/tokens.slint`: exported `Tokens` global with the colour/alpha family (both
        ink scales, zone hues + alphas) and metric/typo family (spacing, type scale, row height);
        dark values as defaults
  - [x] Create `app/src/theme.rs`: canonical theme state (Dark/Light), push-to-`Tokens` on change,
        forced redraw via property update
  - [x] Bundle Inter + the chosen tabular-figures numeric font under `app/assets/fonts/` (+ OFL
        license files); register fonts; set default font family
  - [x] Run and record the tabular-figures verification (UX forward-note)
- [x] Task 2 — Shell layout (AC: 1, 4)
  - [x] Rework `app/ui/app.slint`: window (min size enforced) → left nav rail (4 destinations,
        active indicator, keyboard focus) + top bar + content area + pinned footer disclaimer
  - [x] Create placeholder screens `app/ui/screens/{dashboard,watchlist,portfolio,settings}.slint`
        (snake_case files, PascalCase components, kebab-case properties)
  - [x] Wire nav selection (Rust callback or pure-Slint state — dev discretion), keyboard reachable
- [x] Task 3 — French i18n via @tr() (AC: 3)
  - [x] Wrap every user-visible string in `@tr()` with French source text
  - [x] Document the extraction/bundling pipeline (slint-tr-extractor → .pot; bundled translations
        + `slint::select_bundled_translation` when a 2nd language lands) in crate doc or README
- [x] Task 4 — Label set + locale format mechanisms (AC: 5)
  - [x] `app/src/labels.rs`: `LabelSet` (Naic | Neutral) + keyed lookup table + Slint global
        exposure; live swap
  - [x] Locale number-format setting + formatting helper (e.g. `app/src/viewmodel/format.rs`),
        sample rendering on Settings proves the live swap
  - [x] Minimal Settings screen: theme toggle, label-set toggle, locale format choice (no wizard)
  - [x] Unit tests: every label key is defined in BOTH sets (no silent fallback); formatting helper
        covers decimal comma vs point + thousands separator + negative sign `−`
- [x] Task 5 — App-config persistence (AC: 6)
  - [x] `app/src/config.rs`: serde struct (all fields `#[serde(default)]`, unknown-tolerant),
        load-with-fallback-to-defaults, atomic-ish save (write-then-rename is enough here)
  - [x] Locate via `directories::ProjectDirs`; persist window size, theme, label set, locale format
  - [x] Restore on startup (size + theme before show); save on exit and/or on change
  - [x] Unit tests: config serde round-trip; unknown fields tolerated; missing file → defaults;
        corrupt file → defaults without panic (and the original is not destroyed); tests use
        `tempfile` paths (workspace dev-dep), never the real config dir
- [x] Task 6 — Posture test + gates (AC: 8)
  - [x] Crate-local posture test in `app` scanning `ui/**/*.slint` @tr() literals + `labels.rs`
        strings against `core::method::BANNED_VERBS_EN/FR`
  - [x] All four gates green `--locked`; verify `git diff` over `core/ contract/ ingestion/
        persistence/ report/ docs/method/` is empty; keep the two spike examples compiling
        (`--all-targets` builds them)
- [x] Task 7 — Visual verification + record (AC: 9)
  - [x] Launch, walk the AC-9 checklist on display, record outcome + the typography verdict in the
        Dev Agent Record
  - [x] Update File List (including any QA-generated files — issue #18 discipline)

## Dev Notes

### What this story is — and the disaster it must make impossible

This is the **foundation every Epic-2 UI story builds on**. The disasters to prevent are not
calculation bugs (no engine call here) but **structural ones that would force rework across 13
later stories**:

- A component hard-coding a colour/size → the theme switch breaks the day 2.3 lands. **Tokens only,
  from the first line.**
- UI strings outside `@tr()` → French-first i18n becomes a sweep-and-pray refactor.
- Mixing the two label axes (UI language vs NAIC↔neutral label set) → FR63's runtime swap becomes
  entangled with translations. They are **independent axes**: `@tr()` is compile-time translation
  of UI chrome; the label set is a **runtime data table** of method vocabulary.
- App-config written into (or beside) the journal → violates ADD7. The journal is **not opened** in
  this story; config lives in `ProjectDirs` only.
- A config struct without `#[serde(default)]` → every later field (fold/regime in 2.3) becomes a
  breaking change. User-data rail is tolerant (retro lesson 5).

### Existing code being modified (read before writing)

- `app/src/main.rs` (15 lines) — scaffold entry: `slint::include_modules!()` + `MainWindow::new()`
  + `run()`. Has `#![allow(unused_crate_dependencies)]` because core/contract/ingestion/
  persistence/report/tokio are declared but unused. **This story starts using `directories` and
  `steadyinvest-core` (posture test via dev path or normal dep — core is already a dependency).**
  Keep the allow only while genuinely-unused deps remain (tokio/ingestion/report/persistence stay
  unused until 2.2/3.x); shrink its scope if cleanly possible.
- `app/ui/app.slint` (32 lines) — the Story-1.1 placeholder window (3 centered Text items, one of
  which is already the English disclaimer). **Replaced entirely** by the shell. Note its
  `font-size: 24px` style hard-codes — exactly what AC 2 bans from now on.
- `app/build.rs` — compiles `ui/app.slint` via `slint_build::compile`. Additional `.slint` files
  are pulled in through `import` statements from `app.slint`; **no build.rs change needed** unless
  you opt into a `CompilerConfiguration` (e.g. style selection) — keep it minimal.
- `app/Cargo.toml` — dev-deps `rust_decimal` + `arboard` belong to the two **throwaway spike
  examples** (`examples/spike_a_grid.rs`, `spike_b_chart.rs`). The spikes are frozen evidence; do
  NOT refactor them, but `clippy --all-targets` compiles them — they must keep building. They use
  their own inline UI (not `ui/app.slint`), so the shell rework should not disturb them; verify.
- `Cargo.toml` (workspace) — `directories = "6"` and `serde_json = "1"` already pinned. **Expected
  new external dependencies: NONE.** `arc_swap` (architecture's theme mention) is NOT needed yet:
  Slint properties are main-thread; a plain Rust owner in `theme.rs` pushing into the `Tokens`
  global is the v1 mechanism. Add `arc_swap` only when a second thread actually reads tokens
  (file an issue instead if tempted). `serde`/`serde_json` must be added to `app`'s
  `[dependencies]` (workspace = true) for config.rs.

### Architecture compliance (the guardrails)

- **Crate boundaries:** everything lands in `app/`. No change to any other crate. The Cardinal Rule
  is trivially satisfied (no calculation exists in this story — keep it that way; the sample
  locale-formatted number on Settings formats a **constant**, it computes nothing).
- **Naming (architecture "Implementation Patterns"):** `.slint` files snake_case; components
  PascalCase; properties/callbacks **kebab-case** (`current-theme`, `nav-selected`); exported
  globals PascalCase (`Tokens`, `Labels`); Rust modules snake_case by domain (`theme.rs`,
  `config.rs`, `labels.rs`) — these exact module names come from the architecture tree.
- **Planned app-crate layout** (architecture): `src/{main,state,config,keychain,clock,fetch,theme,
  i18n,labels}.rs` + `src/viewmodel/` + `ui/{app.slint,components/,screens/}`. This story creates
  `theme.rs`, `config.rs`, `labels.rs`, optionally `viewmodel/format.rs` and `i18n.rs` (the latter
  may be just documentation if no runtime wiring is needed for source-language French). It does
  NOT create `state.rs`, `clock.rs`, `keychain.rs`, `fetch.rs` — later stories.
- **Two token families** (UX-DR): colour/alpha may swap at any time; metric/typo never swaps
  during interaction. The theme toggle swaps ONLY colour/alpha values — geometry constant.
- **Errors:** `thiserror`-style domain errors are overkill for config-load in `app`; the
  architecture allows `anyhow` at the app edge. Whatever the shape: **no silent `.ok()`** — a
  config that fails to parse logs a `tracing::warn!` with the cause and falls back to defaults
  (visible event, never a silence). `tracing` is already a dependency; a full rotating-log setup
  (ADD15) is NOT required here — a minimal subscriber is acceptable, dev discretion; file an issue
  if deferred.
- **No clock/UUID:** nothing in this story needs time or IDs. The `Clock`/`IdGen` traits (ADD15,
  retro gap #3) are first needed when a study is created — **Story 2.2 territory, not here.**
- **Issue #14 (schema bump)** is explicitly NOT this story: no judgment is persisted here. The
  story that first persists judgments (2.2/2.6 territory) must own it (retro A3).

### Slint 1.16 technical specifics (verified June 2026)

- **`@tr()` / translations:** strings annotated `@tr("…")` are extractable by `slint-tr-extractor`
  into gettext `.pot` files. Two runtime paths exist: the `gettext` cargo feature +
  `slint::init_translations!` (needs `.mo` files at `<dir>/<locale>/LC_MESSAGES/<crate>.mo`), or
  **bundled translations** compiled into the binary with runtime switching via
  `slint::select_bundled_translation()`. **Decision for 2.1: French as the `@tr()` source text, no
  catalogs shipped.** Rationale: single-binary posture (no gettext system dep on Windows/macOS),
  and only one language exists. When a second language lands, bundled translations are the
  documented path. `@tr()` also supports `"ctx" => "…"` disambiguation and `{}` placeholders with
  plural forms (`| "plural" % count`) — use placeholders, never string concatenation.
- **Theme switching:** the custom `Tokens` global is the law for our own components. If any
  `std-widgets` are used (buttons, switches on the Settings screen are fine — "restyled Slint
  primitives" per the UX component strategy), ALSO set `Palette.color-scheme`
  (`ColorScheme.dark`/`light`, available since Slint 1.6) in the same swap so built-in widgets
  don't clash with the token theme.
- **Window state:** `slint::Window` exposes size get/set (`window().size()` /
  `window().set_size(...)`); restore size **before** `run()`/show to avoid a visible jump.
  Maximized-state API availability in 1.16 should be checked at implementation time — if absent or
  awkward, persist size only (AC 6 allows it). Minimum size: `min-width`/`min-height` on the root
  window element.
- **Fonts:** `.ttf` files can be imported directly from `.slint` (`import "path/to/font.ttf";`)
  which registers the family, or registered from Rust via `slint::register_font_from_memory`/
  `_from_path` before component creation. Set `default-font-family` on the window from the tokens.
- **Per-component screens:** keep each screen an exported component in its own file under
  `ui/screens/`, imported by `app.slint` — matches the architecture tree and keeps 2.2+ diffs
  additive.

### Previous story intelligence (1.11 dev record + review + epic-1 retro)

Epic 1 was headless; the transferable lessons are process and discipline, all confirmed by the
retro as Epic-2 conventions (A5):

- **Gates byte-verified:** always `--locked`; clippy `--all-targets --all-features` (it compiles
  the spike examples and test code too). The 1.11 review re-ran every gate and diffed the pinned
  surfaces — expect the same scrutiny here.
- **Posture gate vets your own vocabulary:** 1.11 had to rename a type because `Hold` is itself a
  banned verb. Check nav labels, settings strings and the disclaimer against
  `BANNED_VERBS_FR/EN` **before** building the UI around them (French strings: watch « acheter /
  vendre / conserver » imperatives; fact-stating nouns are safe).
- **Validate-before-mutate (1.10 lesson, retro insight 3):** `config.rs` must read and validate
  before any write; never truncate-then-parse; a foreign/corrupt file is never destroyed — rename
  it aside or leave it and write fresh, dev discretion, but no data-destroying open.
- **File List completeness (the epic's most repeated finding, issue #18):** QA-generated test
  files and automator logs belong in the File List. Budget the bookkeeping.
- **Interpretations → GitHub issues, never inline notes:** any spec-underspecified decision made
  here (e.g. exact disclaimer wording, label-set seed entries, font fallback) gets recorded —
  small ones in the Dev Agent Record, real interpretations as an issue (the 1.11 pattern:
  one consolidated issue for the story).
- **Cargo.lock delta discipline:** with zero new external deps expected, the lock delta should be
  empty or exactly the `serde`/`serde_json` edges into `steadyinvest-app`. Verify it.

### Git intelligence

Recent commits are the Epic-1 story merges (1.7→1.11 + retro). Patterns established: conventional
commits `feat(story-X.Y): title`; story files + sprint-status updated in the same commit; every
story branch merged only with all four gates green. The `app/` crate has not changed since 1.1
except the two spike examples (1.4/1.5) — this story is the first real `app/` work; there is no
existing UI pattern to conform to beyond the spikes' evidence (which proved `Path`/`TouchArea`
charts and `TableModel` grids feasible — neither is used here).

### Scope boundaries — what 2.1 does NOT do

- **No journal:** `persistence` is not called; no DB file is opened or created (2.2).
- **No dashboard content:** the Studies screen is a placeholder; list/search/sort/filter is
  2.2/2.12; actionable empty states + legend + help/demo are 2.13.
- **No study screen, no regimes:** the entry↔contemplation toggle and fold presets are 2.3 — but
  AC 6's config struct must be extensible enough to absorb their state without migration.
- **No sticky verdict bar** (2.6), **no charts** (2.8), **no provider/keys/currency/threshold
  settings** (3.2/4.x), **no keyring** (3.2 — the dependency is deliberately absent).
- **No app icon / packaging polish** — neutral icon (CR #1) is not in the ACs; file an issue if
  the empty `assets/icons/` bothers anyone.
- **No full WCAG audit** (out of scope per UX) — but keyboard nav + visible focus + non-colour-only
  active state are in scope NOW (NFR-U1/U2 apply from the first screen).

### Project Structure Notes

- New files: `app/ui/tokens.slint`, `app/ui/screens/{dashboard,watchlist,portfolio,settings}.slint`,
  `app/src/{theme,config,labels}.rs` (+ optional `app/src/viewmodel/format.rs`, `app/src/i18n.rs`),
  `app/assets/fonts/*` (+ licenses), posture/unit tests in `app` (`#[cfg(test)]` or `app/tests/`).
- Modified: `app/ui/app.slint` (replaced), `app/src/main.rs` (config load → theme init → window
  restore → run → save), `app/Cargo.toml` (+`serde`, `serde_json` from workspace), possibly
  `Cargo.lock`.
- Untouched (verify with `git diff`): `core/`, `contract/`, `ingestion/`, `persistence/`,
  `report/`, `docs/method/**`, `.github/workflows/ci.yml`, `deny.toml`, `rust-toolchain.toml`.
- Variance note: the architecture tree shows `app/src/i18n.rs` as "@tr() wiring"; with
  French-as-source-language there may be nothing to wire at runtime — an empty module is noise;
  documentation-only is an acceptable, documented variance.

### References

- Story & ACs: `_bmad-output/planning-artifacts/epics.md` § "Story 2.1" (+ Epic 2 intro)
- FR63/FR64 + NFR-U/X/P: `_bmad-output/planning-artifacts/prd.md` § "Configuration, Posture &
  Operation", § "Non-Functional Requirements"
- Tokens, ink scales, zone hues/alphas, typography, spacing: `ux-design-specification.md`
  § "Visual Design Foundation", § "Design System Foundation"
- Nav, button hierarchy, microcopy, window-size/persistence: `ux-design-specification.md`
  § "UX Consistency Patterns", § "Responsive Design & Accessibility", § "Component Strategy"
- App-crate layout, naming, error/logging rules, ADD7/ADD15: `architecture.md` § "Project
  Structure & Boundaries", § "Implementation Patterns & Consistency Rules"
- Epic-1 lessons & Epic-2 conventions: `epic-1-retro-2026-06-12.md` (§5 insights, §7 A3/A5)
- Banned verbs: `core::method::BANNED_VERBS_EN` / `BANNED_VERBS_FR` (code constants — reuse)
- Slint translations: https://docs.slint.dev/latest/docs/slint/guide/development/translations/
- Slint Palette/color-scheme: https://docs.slint.dev/latest/docs/slint/reference/std-widgets/style/

### Tech currency note (2026-06-12)

Slint pinned at 1.16 (workspace); translations (`@tr()`, slint-tr-extractor, bundled translations
+ `select_bundled_translation`) and `Palette.color-scheme` (≥1.6) are current stable APIs as of
June 2026. No new external crates expected; `directories` 6 and `serde_json` 1 already pinned and
license-vetted in `deny.toml`. Fonts: Inter and IBM Plex Sans / Source Sans 3 are OFL-1.1 —
bundled assets, not cargo deps, so `cargo deny` is unaffected; ship the license files alongside.

## Dev Agent Record

### Agent Model Used

claude-fable-5 (Claude Code)

### Debug Log References

- Four gates green `--locked` after final code state: `cargo fmt --all --check` · `cargo clippy
  --all-targets --all-features --locked -- -D warnings` · `cargo test --all --locked` (283
  passed, 0 failed — 27 new in `app`) · `cargo deny check`. Spike examples still compile
  (`--all-targets`). `git diff` over `core/ contract/ ingestion/ persistence/ report/
  docs/method/ ci.yml deny.toml rust-toolchain.toml` is empty.
- Cargo.lock delta: the `serde`/`serde_json`/`tempfile` edges into `steadyinvest-app` (no new
  external crates added). **Correction (review 2026-06-13):** the lock additionally consolidated
  two duplicate transitive font crates — `read-fonts 0.39.2` and `skrifa 0.42.1` were removed (0
  added), collapsing onto the already-present 0.37.0/0.40.0 lines. Benign de-duplication; all four
  gates remain green under `--locked` (`cargo deny check` ok). The earlier "exactly the …/no other
  change" wording was inaccurate and is corrected here.
- **Window-restore bug found & fixed during AC-9:** `set_size(PhysicalSize)` before `show()` is
  misapplied by winit on X11/XWayland (width fell back to min-width; height scaled as if
  logical). Fix: best-effort pre-show set + a 6×50 ms post-show retry timer in `main.rs`;
  round-trip then pixel-exact (tested 1600×900 and 1200×800). `window().size()` reads back the
  *requested* size, so the fixed tick count is deliberate. Recorded in issue #19.

### Visual verification (AC 9) — run 2026-06-12, on display (XWayland, 5120×1440, scale 1.34)

Driven via AT-SPI accessibility actions + a focus-guarded virtual keyboard (Wayland blocks raw
input injection); ~20 screenshots reviewed. Results:

- Dark shell renders per the ink scale (bg `#0E0F12` family); nav rail + top bar + content +
  pinned footer; **disclaimer visible on all four destinations** ✓
- Nav switches all four screens; active destination shown by indicator bar + surface step +
  weight step (not colour alone, NFR-U1) ✓
- **Keyboard fully operable (NFR-U2):** Tab walks the nav with an always-visible 2 px focus
  ring; **Enter** activated Portefeuille; **Space** activated Réglages ✓
- **Theme toggles live** dark↔light, no restart, no geometry change (window stayed 1200×800) ✓
- **Label set swaps live** (Réglages chips + vocab line; dashboard shows « Méthode : Étude
  d'action » after swap) ✓ — UI-language axis untouched (strings stay French)
- **Locale format swaps live** (samples `−1 234 567,89` ↔ `−1,234,567.89`, true minus U+2212) ✓
- **Persistence across restart:** resize → close → relaunch reopens at the same size with the
  persisted theme/label-set/format (light + neutral + point verified) ✓
- **Minimum window size enforced (UX-DR28):** 500×300 request clamped to 965×643 physical
  = 720×480 logical ✓
- **Launch-to-interactive ≈ 0.12 s** debug build (window mapped; NFR-P4 ~3 s: large margin) ✓
- **Typography verdict (UX forward-note):** IBM Plex Sans (400/500/600) renders **tabular
  figures by default** in Slint — digit columns align exactly across stacked rows in both
  themes (zoomed crops reviewed). No fallback needed. Inter 400/600 for UI text.

### Completion Notes List

- Token system: `ui/tokens.slint` exports `Tokens` (colour/alpha in-out, metric/typo out);
  `src/theme.rs` owns canonical state and pushes whole ink scales; no std-widgets used, so no
  `Palette.color-scheme` sync needed (all components are restyled primitives).
- Two label axes kept strictly independent: `@tr()` French source strings (extraction pipeline
  documented in the `main.rs` crate doc) vs `labels.rs` runtime data table (4 seed keys, both
  sets complete, posture-gated).
- App-config: `ProjectDirs` + serde, container-level `#[serde(default)]`, unknown fields
  tolerated; corrupt file renamed aside (`config.json.invalid`), never destroyed; load warnings
  via `tracing::warn!` + stderr (subscriber deferred — issue #19).
- Posture gate: crate-local test scans every `@tr()` literal in `ui/**/*.slint` (own extractor,
  handles ctx/plurals/escapes) + the label table against `core::method::BANNED_VERBS_EN/FR`
  (reused). Sanity floors: ≥8 files, ≥15 literals.
- Accessibility: `NavItem`/`ChoiceChip` expose role/label/checked/default-action (AccessKit) —
  interpretation recorded in issue #19; keep as convention for future interactive components.
- Interpretations consolidated in GitHub issue #19 (window-restore workaround, deferred ADD15
  subscriber, a11y annotations, content seeds).

### File List

New:
- app/src/config.rs
- app/src/labels.rs
- app/src/posture.rs
- app/src/theme.rs
- app/src/viewmodel/mod.rs
- app/src/viewmodel/format.rs
- app/ui/tokens.slint
- app/ui/state.slint
- app/ui/components/nav_item.slint
- app/ui/components/choice_chip.slint
- app/ui/screens/dashboard.slint
- app/ui/screens/watchlist.slint
- app/ui/screens/portfolio.slint
- app/ui/screens/settings.slint
- app/assets/fonts/Inter-Regular.ttf
- app/assets/fonts/Inter-SemiBold.ttf
- app/assets/fonts/IBMPlexSans-Regular.ttf
- app/assets/fonts/IBMPlexSans-Medium.ttf
- app/assets/fonts/IBMPlexSans-SemiBold.ttf
- app/assets/fonts/LICENSE-Inter-OFL.txt
- app/assets/fonts/LICENSE-IBMPlexSans-OFL.txt

Modified:
- app/src/main.rs
- app/ui/app.slint
- app/Cargo.toml
- Cargo.lock
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/2-1-application-shell-theme-disclaimer.md

## Senior Developer Review (AI)

**Reviewer:** Guy · **Date:** 2026-06-13 · **Outcome:** Approve (auto-fix applied) · **Story status → done**

Adversarial review (story-automator review flow, auto-fix). Every AC was cross-checked against the
actual implementation; all four gates were re-run independently and the pinned surfaces re-diffed.

**Independently verified green:**
- `cargo fmt --all --check` ✓ · `cargo clippy --all-targets --all-features --locked -- -D warnings` ✓
  (clean) · `cargo test -p steadyinvest-app --locked` ✓ (27 passed, 0 failed) · `cargo deny check` ✓
  (advisories/bans/licenses/sources ok).
- Pinned-surface guard: `git diff` over `core/ contract/ ingestion/ persistence/ report/ docs/method/
  .github/ deny.toml rust-toolchain.toml` is **empty** — this story touched only `app/` + lockfile +
  tracking files.
- AC2 governance: no hard-coded hex colour or `px` anywhere in `ui/` outside `tokens.slint`.
- Bundled fonts are real TTFs (Inter 400/600; IBM Plex Sans 400/500/600) with OFL license files
  present — not placeholder stubs.
- File List is complete and accurate against `git status` (all new/modified files accounted for).
- AC-by-AC: shell + nav + top bar + pinned footer (AC1/4), `Tokens` dark-default/light runtime swap
  with geometry held constant (AC2), French `@tr()` with pipeline documented in the crate doc (AC3),
  NAIC↔neutral label table + locale formatter both unit-tested incl. true minus U+2212 (AC5),
  tolerant corrupt-safe `ProjectDirs` config with fallback tests (AC6), tabular-figures verdict
  recorded (AC7), crate-local banned-verb posture gate over `@tr()` literals + label table (AC8),
  on-display visual verification recorded (AC9). All implemented.

**Findings:**
1. 🟡 **MEDIUM (fixed — record corrected).** The Debug Log claimed the Cargo.lock delta was "exactly
   the serde/serde_json/tempfile edges (no new external crates)". Reality: the lock also removed two
   duplicate transitive font crates (`read-fonts 0.39.2`, `skrifa 0.42.1`; 0 added). Benign de-dup,
   gates still green under `--locked`. Fixed by correcting the Debug Log rather than rewriting a green
   lockfile (which would risk the `--locked` gate).
2. 🟢 **LOW (recorded, not changed).** `app/ui/screens/settings.slint:129,137` use a `width: 50%`
   layout fraction not sourced from `Tokens`. Borderline against AC2 (which governs the colour and
   metric/typo token scales, not relative layout proportions). Left as-is: the screen is AC-9
   visually verified and a headless review cannot re-verify the layout after a change — not worth a
   visual regression risk for a pedantic gain. Non-blocking.

No CRITICAL or HIGH issues. 0 CRITICAL remaining → status advanced to **done**.

## Change Log

- 2026-06-12 — Story 2.1 implemented: application shell (nav rail + top bar + pinned FR64
  disclaimer), `Tokens` design system with runtime dark/light swap, French UI via `@tr()`,
  NAIC↔neutral label set + locale number format (live swaps, no wizard), app-config persistence
  via `ProjectDirs` (tolerant serde, corrupt-safe), bundled Inter + IBM Plex Sans (tabular
  figures verified), crate-local banned-verb posture gate, full AC-9 visual verification on
  display. Window-restore winit workaround + interpretations in issue #19. All four gates green
  `--locked`; pinned surfaces untouched. Status → review.
- 2026-06-13 — Senior Developer Review (AI, auto-fix): all four gates re-verified green
  `--locked`, pinned surfaces re-diffed empty, ACs 1–9 cross-checked against implementation. One
  MEDIUM (inaccurate Cargo.lock Debug Log claim) corrected; one LOW (`width: 50%` in settings.slint)
  recorded as non-blocking. No CRITICAL/HIGH. Status → done.

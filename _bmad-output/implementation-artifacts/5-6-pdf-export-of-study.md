# Story 5.6 — PDF export of a study (FR52)

Status: done

## Story

As Guy,
I want a faithful PDF of a study,
so that I can archive or print my conviction.

## Acceptance Criteria

1. **AC1 — UI-independent PDF from `core`/`contract` via the `report` crate (FR52).** Exporting a study to PDF is produced by the **`report` crate**, which depends only on `core` + `contract` (no `app`, no Slint) so it runs headless. The single construction path `Study → core::StudySnapshot` is **shared** (not duplicated): the `contract → core` mapping + `build_frame`/`build_snapshot` move into `report` and `app` re-exports them — there remains exactly ONE construction of the snapshot, so the PDF's computed figures cannot drift from the live form's. A drift-pinning test (in `app`, which depends on `report`) asserts `report`'s snapshot equals `app`'s for a fixture.

2. **AC2 — Faithful, neutral, all-sections-expanded layout.** The PDF reproduces the faithful SSG form layout (§1 visual-analysis historical series, §2 management %PTP/%ROE, §3 P/E history, §4 risk/reward forecast band + zoning, §5 5-year potential) with the project's **neutral labels** (the `core`/`contract`-side neutral wording, **no NAIC marks/logos or verbatim instructional text** — open-source constraint), **all sections expanded** (a PDF has no collapsibles). Missing values render as the faithful em-dash, never `0` (the project rail).

3. **AC3 — Readable in pure greyscale (NFR-U3).** No information is conveyed by colour alone: zones/verdict/markers read via text + position + line weight (the on-screen UX-DR10 discipline, carried to print). The PDF is legible printed on a monochrome printer.

4. **AC4 — Provider-independent; minimal, audited dependency; deterministic.** The export needs no market-data provider (it renders the stored study). The PDF backend is **`pdf-writer`** (write-only, precise coordinates, 4 tiny leaf deps, no PDF parser → no untrusted-parse advisory); `cargo deny` stays green. Output is **deterministic** (no embedded timestamp / random IDs) so a fixture's bytes are testable. No `core`/`contract` API change beyond the construction-path move; `SCHEMA_VERSION`/migrations untouched; copy neutral & posture-gated.

## Tasks / Subtasks

- [x] **Task 1 — Relocate the single construction path into `report` (AC1)** — `report/src/form.rs`, `app/src/viewmodel/engine.rs`
  - [x] Move the pure `contract → core` mapping (`raw_amount`, `to_raw_financials`, `money_dec`, `to_forecast_low_option`, `to_judgment_inputs`, `to_observations`, `cell_to_gate_state`, `judgment_to_gate_state`, `to_input_gates`) + `StudyFrame` + `build_frame`/`build_snapshot` from `app/src/viewmodel/engine.rs` into a new `report::form` module (depends only on `core` + `contract`).
  - [x] In `app/src/viewmodel/engine.rs`, `pub use steadyinvest_report::form::{…}` so all existing app call-sites keep resolving via `crate::viewmodel::engine::…` unchanged.
  - [x] Drift-pin test (in `app`): `report::form::build_snapshot(&demo)` equals the app path for the demo fixture (one construction, no drift).

- [x] **Task 2 — The PDF renderer (AC2, AC3, AC4)** — `report/src/pdf.rs`
  - [x] `render_study_pdf(study: &Study) -> Result<Vec<u8>, ReportError>`: build the frame via `report::form::build_frame`; lay out the faithful SSG grid with `pdf-writer` (A4 portrait, coordinate-placed text + line/rect grid). Neutral labels; all sections expanded; em-dash for `None`; greyscale only.
  - [x] Deterministic output (no timestamp/random); a fixed document/info dictionary.
  - [x] Tests: non-empty `%PDF` bytes; deterministic (same study → identical bytes); contains the neutral section headers and NOT any NAIC wordmark; a study that fails to normalize yields a neutral `ReportError` (no panic).

- [x] **Task 3 — App export action (AC1)** — `app/src/main.rs`, `app/ui/screens/dashboard.slint`
  - [x] A per-row **"Exporter PDF"** action on a saved study → `report::render_study_pdf` → write to `data_dir/exports/study-<id>.pdf` (mirror the Story-5.2 study-export path UX; native save picker is out of scope / a later refinement). Neutral success/refusal notice; no new MSG unless required (reuse `MSG_STUDY_EXPORTED` shape).
  - [x] Posture: `@tr` floor bumped by the exact number of new literals; any new `MSG_*` registered + inventory bumped.

- [x] **Task 4 — Gates (AC4)** — fmt, clippy `-D`, `test --workspace`, `deny` + smoke. Confirm: construction-path move keeps all app tests green (single path, no drift); **no `core`/`contract` API change** beyond the move; migration/`SCHEMA_VERSION` untouched; **one new dependency `pdf-writer`** (Cargo.lock +5 leaves, `deny` green); `@tr`/MSG inventories bumped exactly.

## Dev Notes

### Scope
- The faithful PDF of ONE study (FR52). Other forms' PDFs (FR53) are Epic 7 (Story 7.5). No print-dialog integration (write a file); native save picker is a later refinement (path-based, like 5.2/5.3).

### Architecture decisions this story honours
- [architecture.md §142] — "PDF/print fidelity (FR52) via a PDF backend, UI-independent" — `report` depends only on `core`+`contract`. Backend = `pdf-writer` (printpdf 0.7 trips the lopdf parse-advisory we never hit; 0.9 is heavy — `pdf-writer` is write-only, minimal, deny-green, and gives the precise coordinate control the faithful grid needs). Guy chose `pdf-writer` over a printpdf advisory-exception (2026-06-30).
- [core deliberately does NOT depend on `contract`] — so the `Study → core` mapping cannot live in `core`; the shared home is `report` (depends on both; `app` already depends on `report`). Moving the construction there preserves the **single construction path** invariant (one `build_frame`, used by both the live form and the PDF — no second normalize, no drift).
- [open-source/naming constraint] — neutral labels only; no NAIC marks/logos/verbatim prose in the PDF.
- [GUI = Slint-only] — N/A to the PDF itself (UI-independent); the export action is a path-based control like 5.2.

### Where things live
- `report/src/form.rs` (NEW) — the relocated construction path.
- `report/src/pdf.rs` (NEW) — the `pdf-writer` layout.
- `report/src/lib.rs` — `pub mod form; pub mod pdf;` + `pub use pdf::{render_study_pdf, ReportError};`.
- `app/src/viewmodel/engine.rs` — re-export of the construction path (the formatting/presentation fns stay).
- `app/src/main.rs` + `app/ui/screens/dashboard.slint` — the "Exporter PDF" row action.

### References
- [epics.md#Story 5.6] — faithful PDF, neutral labels, all sections expanded, greyscale-readable (FR52, NFR-U3).
- [architecture.md §142, §673–675, §743] — `report` crate, PDF backend, UI-independent.
- [app/src/viewmodel/engine.rs] — the construction path being relocated (mapping + build_frame + StudyFrame).
- [5-2-export-import-single-study.md] — the per-row export-to-`data_dir/exports/` path UX mirrored here.

## Dev Agent Record

### Agent Model Used
Claude Opus 4.8 (1M context).

### Debug Log References
Gates green: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -D warnings`, `cargo test --workspace --locked` (**596 tests**), `cargo deny check` (advisories/bans/licenses/sources ok), smoke `timeout cargo run -p steadyinvest-app` (exit 124). Demo PDF rendered + eyeballed (A4, 1 page, faithful greyscale layout); byte-identical across runs (determinism).

### Completion Notes List
- **AC1 — UI-independent, single shared construction.** The `contract → core` mapping + `build_frame`/`build_snapshot` + `StudyFrame` MOVED from `app/src/viewmodel/engine.rs` into a new `report/src/form.rs` (depends only on `core` + `contract`). `app::viewmodel::engine` re-exports `{build_frame, build_snapshot, money_dec, StudyFrame}`, so all ~60 call-sites resolve unchanged. There is now literally ONE `build_frame` — drift between the live form and the PDF is structurally impossible (stronger than a pin test). `core` deliberately doesn't depend on `contract`, which is why the shared home is `report`, not `core`. `report::form` carries its own coherence test (`build_frame_constructs_one_coherent_frame`).
- **AC2 — faithful neutral layout.** `report::pdf::render_study_pdf` lays out all 5 SSG sections + a Synthèse, all expanded, with neutral French labels (header "Analyse de sélection de titre", NOT "Stock Selection Guide"; zone nouns "Zone basse/médiane/haute"). `None` → em-dash (never 0). Figures formatted from `Decimal` via `core::rounding::round_for_display` (no f64 in the chain; f32 is coordinates only).
- **AC3 — greyscale only.** The content stream uses only `set_fill_gray`/`set_stroke_gray` — no RGB/CMYK op anywhere; information reads by text + position + line weight (UX-DR10 carried to print).
- **AC4 — provider-independent, audited dep, deterministic.** Backend = **`pdf-writer 0.12`** (write-only; Cargo.lock + pdf-writer + ryu, the other 3 leaves pre-existing; NO lopdf/allsorts/printpdf/genpdf). `deny.toml` UNCHANGED, `cargo deny` green. Deterministic (no `/ID`/timestamp). NO `core`/`contract` API change beyond the relocation; no schema/migration/SCHEMA_VERSION change. Posture `@tr` floor 314→315 (one new `@tr` "Exporter PDF"); export reuses `MSG_STUDY_EXPORTED` + `MSG_SAVE_FAILED` (no new MSG, inventory unchanged).
- **Dependency decision (Guy, 2026-06-30):** chose `pdf-writer` over a printpdf advisory-exception — printpdf 0.7 pins `lopdf 0.31` which trips a parse-path stack-overflow RUSTSEC (unreachable for us — we only WRITE), and printpdf 0.9 pulls a heavy tree; `pdf-writer` is write-only, minimal (4 tiny leaves), deny-green, and gives the precise coordinate control the faithful grid needs.
- **3-layer adversarial review (Blind Hunter / Edge Case Hunter / Acceptance Auditor) — 4 patches, 1 defer:**
  - **MED (Auditor #7 + Blind):** `report`'s French strings live outside the app posture gate. Added a `report` neutrality test (`report_strings_are_neutral_no_banned_verb`) scanning a `REPORT_USER_FACING` inventory + the `zone_label`/`trend`/`upside`/verdict labels against `core::method::BANNED_VERBS_{FR,EN}` (the same shared catalogs the app gate uses) + a source-level no-NAIC test (stronger than the prior hex-encoded byte scan).
  - **LOW (Blind):** multi-page pagination was untested → added `a_many_year_study_paginates_to_multiple_pages` (56-year fixture asserts `/Count ≥ 2` + well-formed graph).
  - **LOW (Edge Case):** `winansi` mapped the C1 range 0x80–0x9F to raw bytes → now routes them to `'?'` (those code points aren't identity-mapped in WinAnsi).
  - **LOW (Edge Case):** `ensure()` under-reserved a few pt for title/head rows → reserves now match the true advance (no footer collision either way; the demo PDF is byte-identical).
  - **1 defer → #74:** repeat table column headers on continuation pages + truncate over-long tickers (fidelity nits) + the §1-raw-vs-§2/§3-canonical row-set note.
- All 4 ACs PASS, all guardrails clean (Auditor). 596 tests; fmt/clippy/deny/smoke green.

### File List
- `report/Cargo.toml` — add `pdf-writer` + `rust_decimal` deps, `uuid` dev-dep.
- `report/src/lib.rs` — `pub mod form; pub mod pdf;` + re-export.
- `report/src/form.rs` (NEW) — the relocated single construction path + its coherence test.
- `report/src/pdf.rs` (NEW) — the `pdf-writer` faithful/neutral/greyscale layout + paginator + neutrality/determinism/multi-page tests.
- `report/examples/study_pdf.rs` (NEW) — a preview tool (renders a demo study to a PDF).
- `app/src/viewmodel/engine.rs` — re-export the construction path; prune now-unused imports.
- `app/src/seam_check.rs` — test calls `report::form::cell_to_gate_state` directly.
- `app/src/main.rs` — `write_study_pdf` + the `on_export_study_pdf` callback.
- `app/src/posture.rs` — `@tr` floor 314→315.
- `app/ui/state.slint` — `Studies.export-study-pdf(string)` callback.
- `app/ui/screens/dashboard.slint` — the "Exporter PDF" row action.
- `Cargo.toml` / `Cargo.lock` — workspace `pdf-writer` dependency.

### Change Log
- 2026-06-30 — Story 5.6 implemented (faithful, neutral, greyscale study PDF via `pdf-writer`; the single `Study → snapshot` construction relocated to `report::form`, re-exported by `app`). 3-layer review: 4 patches, 1 defer (#74). 596 tests; all gates green. Status → done. **Closes Epic 5.**

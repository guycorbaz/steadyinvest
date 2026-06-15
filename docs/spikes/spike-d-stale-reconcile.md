# Spike D — provider/reconciliation seam verification (stale · ✓→? · verdict degrade)

**Epic:** 3 (Provider data & reconciliation) · **Origin:** Epic 2 retrospective action **B3** · **Date:** 2026-06-15 · **Type:** seam-verification spike (go/no-go). The headless half is a **kept regression guard**, not throwaway.
**Question:** do the three provider/reconciliation seams that Epic 2 *built but only fixture-exercised* actually fire through the **real** rails when a provider sets `Freshness::Stale` / a divergent value — before Epic 3's reconciliation stories (3.3/3.4) build on them?

## The three seams (mapped)

| # | Seam | Real rail | Built in | Production constructor before Epic 3 |
|---|------|-----------|----------|--------------------------------------|
| 1 | **Stale murmur** — `Freshness::Stale` → dimmed `◦` | `form::editable_cell` sets `GridCellState.stale`; `editable_cell.slint` renders `◦` at 60% opacity | 2.4 | **none** (only test fixtures) |
| 2 | **Divergent-edit `✓→?` demotion** | `contract::Cell::edited` (Validated→ToReview iff value diverges) | 1.11 / 2.5 | **none with `Source::Provider`** (app only calls it with `manual_provenance()`) |
| 3 | **Verdict degradation** on a stale load-bearing input | `engine::cell_to_gate_state` → `(Validated, Stale)`→`GateState::Stale` → `Verdict::Provisional` | 2.6 | mapping wired; never reached in prod |

## What was built

`app/src/seam_check.rs` — a `#[cfg(test)]` module (wired `mod seam_check;` in `main.rs`) that drives all three seams through the **real** contract/engine/form code, no real provider:

- **SEAM 2** `seam2_provider_divergent_edit_demotes_validated_keeps_unchanged`: `validated_cell.edited(divergent, provenance(Source::Provider))` → `Review::ToReview` + `Source::Provider` + `Freshness::Current`; a **non-divergent** re-fetch keeps `✓` (value equality, the Epic-3 reconcile rule).
- **SEAM 3** `seam3_stale_load_bearing_input_degrades_full_to_provisional`: a green study derives `Verdict::Full` through `engine::build_frame`; flipping one load-bearing cell to `Freshness::Stale` → `cell_to_gate_state == GateState::Stale` and the verdict degrades to `Verdict::Provisional`.
- **SEAM 1** `seam1_stale_cell_surfaces_in_the_form_grid`: a stale sales cell → `form::mgmt_rows` → `GridCellState.stale == true` on that cell, `false` on a current sibling (the bool the `◦` + dimming bind to).

Run: `cargo test -p steadyinvest-app --bin steadyinvest-app seam_check`.

## Result — **GO (headless)**

All 3 seam tests pass through the real rails. The Epic-2 reconciliation machinery is sound: the contract primitive auto-demotes on provider divergence, the engine degrades the verdict on stale, and the form adapter surfaces the murmur bool. Reconciliation stories (3.3/3.4) can build on these with confidence.

## Spike finding for Story 3.3 (the one real gap)

`state::JournalState::mutate_cell` **hardcodes `self.manual_provenance()`** (and `Source::Manual`) — there is **no provider-provenance path** today. `mutate_cell`'s closure receives a manual provenance, so a provider refresh cannot route through it as-is. **Story 3.3 must add a provider rail** that threads a `Source::Provider` `Provenance` (with the real fetch `logical_version` + dependency digest, issue #21) into `Cell::edited`. The contract primitive already accepts it (SEAM 2 proves this) — only the app-side `mutate_*` rail needs the new entry point. Note `mutate_cell` also applies no soft-lock guard internally (that lives in `edit_cell`), so a provider refresh can legitimately auto-demote a `✓` cell on divergence per FR-reconcile.

## Residual — on-display GO/NO-GO (needs Guy's display)

The headless tests prove the **state** is correct; the **perceptual** render is for the target display (Wayland sandbox blocks screenshots/AT-SPI). To confirm when convenient — or fold into Story 3.3's DoD once a real refresh sets stale:

1. A present-but-stale cell shows the `◦` murmur and dims to ~60% **without** stealing attention from a `▦ to-fill` gap (the attention hierarchy holds).
2. A provider-divergent `✓` cell visibly returns to `?` (the validated ink clears).
3. The verdict badge visibly drops from full-colour to the **provisional hatched** state with the stale input named ("périmé") in the open-gates list.

Until then: **GO** to proceed with Epic 3, opening at Story 3.1 (`MarketDataProvider` trait + EODHD adapter), with the 3.3 provider-rail finding recorded above.

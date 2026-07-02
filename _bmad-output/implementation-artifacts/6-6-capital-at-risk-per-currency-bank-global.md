# Story 6.6 — Capital-at-risk per currency → per bank → global total (FR44)

Status: done

## Story

As Guy,
I want my capital-at-risk consolidated **per currency, then per bank, then as one global total in my reference currency** — every converted figure carrying the date and source of the rate that produced it,
so that I can see my whole downside across banks at a glance without any figure ever being silently mixed across currencies or converted with a rate I cannot inspect.

## Acceptance Criteria

1. **AC1 — The consolidation hierarchy, journal-wide, in the app (PRD line 63; MIGRATION-FREE, no new dependency).** A new app read `journal_capital_at_risk_consolidation(reference_currency)` computes, for **every** portfolio (bank) of the journal: (a) the per-currency native buckets (the EXISTING single-currency `core::risk::capital_at_risk`/`total_invested` called once per bucket — the 6.2 pattern, `core::risk` unchanged); (b) the bank's **converted subtotal** in the reference currency (each foreign bucket × its rate via the pure `core::risk::fx::convert`, reference buckets pass through); and (c) the **global total** across banks. Per-currency buckets stay FX-free (FR28 — conversion happens ONLY at the bank/global consolidation points). All arithmetic checked (a `None` from `convert`/an overflowing sum → the consolidated figure is ABSENT, never wrong).

2. **AC2 — Rates: the latest dated rate per pair, surfaced with date + source; a missing rate refuses honestly.** Conversion uses `latest_fx_rate(foreign, reference, None)` (the most recent stored rate — 6.5's arbitration). Every consolidated figure names the rates behind it: the view carries, per converted pair, `(pair, rate, rate_date, source)` and the UI shows them (the FR28 inspectability rule). When ANY required pair is missing, the affected bank subtotal and the global total are **absent with the missing pair(s) named** (« taux manquant EUR → CHF ») — never a partial sum passed off as total, never a silent 1/rate inversion. A stale-but-present rate converts normally (its visible date IS the honesty).

3. **AC3 — Portefeuille UI: the consolidated block (FR13-neutral).** Under the existing per-currency capital-at-risk block, a « Consolidation (toutes banques) » section shows: one line per bank — name + converted subtotal in the reference currency (or the named missing-pair refusal); the « Total global » line (converted total + its share of the converted total invested, the FR43 "% du capital investi" parallel); and the rates footnote (« convertis aux taux : EUR → CHF 0.93 le 2026-07-02 (manuel)… »). Native per-currency lines stay untouched (they remain the primary, FX-free facts). All copy neutral, posture-gated; `@tr` floor (356) and MSG inventory (98) bumped by the exact new counts. The block re-renders with the register (every holdings/ledger/FX mutation flows through `refresh_holdings`).

4. **AC4 — 6.5 invariants keep holding; scope pins.** The 6.2 per-currency reads stay conversion-free; `core::ssg`/method/goldens untouched; `fx_rates` writers unchanged. Scope pins (PRD-grounded): CaR consolidation ONLY — the 6.4 reinvestable-cash conversion, open-profit-at-risk (vs current price) and concentration (FR45) are NOT this story (6.7+/later); no per-portfolio reference currencies (the reference stays the single global value); no rate freshness threshold (the visible date is the honesty — a staleness murmur can ride #90's panel work).

5. **AC5 — Gates.** Tests: two banks × two currencies consolidate correctly with exact conversion; a missing pair absents the right subtotal AND the global while naming the pair; a rate arriving (manual or fetched) makes the next render consolidate; reference-currency buckets convert at 1 implicitly (no self-rate row needed, no lookup for reference→reference); checked-overflow absents rather than corrupts; sold holdings stay EXCLUDED from CaR (they carry no position risk — unchanged 4.6 semantics). fmt + clippy `-D` + `test --workspace` + `cargo deny` + smoke exit 124; `Cargo.lock`/`deny.toml` unchanged; NO migration.

## Tasks / Subtasks

- [x] **Task 1 — `app` state: the consolidation read (AC1, AC2)** — `app/src/state/fx.rs` (or a sibling `consolidation.rs`)
  - [x] View types (plain app structs): `BankConsolidation { portfolio_id, name, buckets: Vec<(ccy, car, invested)>, converted: Option<(car_ref, invested_ref)>, missing_pairs: Vec<String> }` and `JournalConsolidation { banks, global: Option<(car_ref, invested_ref)>, missing_pairs, rates_used: Vec<FxRateItem> }`.
  - [x] `journal_capital_at_risk_consolidation(reference)`: for EVERY portfolio, group its ACTIVE holdings by effective currency (the exact `portfolio_capital_at_risk_by_currency` logic generalized per portfolio — extract a shared helper rather than duplicating); convert each foreign bucket via `latest_fx_rate` + `core::risk::fx::convert`; reference buckets pass through; missing pair → the bank's converted = None + pair recorded; global = checked sum over banks (any None bank or overflow → global None); `rates_used` deduplicated per pair.
  - [x] Tests (AC5 list).
- [x] **Task 2 — UI: the consolidated block (AC3)** — `app/ui/screens/portfolio.slint`, `app/ui/state.slint`, `app/src/wiring/holdings.rs`
  - [x] Slint structs `BankCarRow { name, amount, pct, missing }` + `Holdings.consolidation-banks`, `consolidation-global`, `consolidation-rates`, `consolidation-missing` (display strings built in Rust; formatted via the locale path like the CaR block).
  - [x] `refresh_holdings` builds + pushes the consolidation block (after the CaR/cash pushes).
  - [x] Posture floors bumped exactly.
- [x] **Task 3 — Gates + close-out (AC4, AC5)** — full workspace gates; note on #90 (the staleness murmur home); story records.

### Review Findings (2-layer, 2026-07-02 — Blind Hunter / Edge+Audit combined)

- [x] [Review][Patch] HIGH (edge, AC3 FAIL clause): neither FX mutation path re-rendered the consolidation block (pure-Slint navigation → « taux manquant » persisted after the rate arrived) → `refresh_holdings` now runs after a successful manual add-rate AND after the provider FxRates outcome [wiring/fx.rs, wiring/fetch.rs]
- [x] [Review][Patch] HIGH (blind+edge): a failed per-bank holdings read rendered a confident « 0.00 CHF » folded into the global → an ABSENT bank (`unavailable`), never a zero [state/fx.rs]
- [x] [Review][Patch] HIGH/MED (both): the overflow-absence was unnamed — a dangling « Banque —  » line and a globally-vanished total with no explanation → `BankConsolidation.unavailable` + the two plain-« indisponible » UI variants (per-bank + global) [state/fx.rs, state.slint, portfolio.slint]
- [x] [Review][Patch] MED (blind): `pct_of` used unchecked Decimal `/`+`*` (a panic path on the render thread) → checked_div/checked_mul, "" on None [wiring/holdings.rs]
- [x] [Review][Patch] MED (blind): French prose (« le ») baked into the Rust footnote, invisible to the posture scan → the entry is pure data `PAIR RATE (date, source)` [wiring/holdings.rs]
- [x] [Review][Patch] MED (edge): a parseable-but-nonpositive imported rate converted to a confident zero → `filter(r > 0)` folds it into the named missing-pair refusal [state/fx.rs]
- [x] [Review][Patch] MED (edge, AC5): the checked-overflow consolidation test was missing → added (Decimal::MAX-scale CaR × rate 2 → bank unavailable, global absent) [state/tests.rs]
- [x] [Review][Doc] Bank order pinned in a comment (= the 6.1 portfolio list order); the rates_used dedup-per-pair invariant (sound only while all lookups share the None as-of) noted in place.

Dismissed (4): inverse-pair fallback (6.5 pins quote = reference; the reference-switch strand is #90); DB-error vs missing-rate message conflation on the rate lookup (the row-read path; the holdings-read path IS fixed — revisit with #90's panel work); case-sensitive identity check (the allow-list uppercases every write/import site); global-pct caption nuance (the pooled ratio is the honest FR44 figure; a caption tweak can ride later UX polish).

**Review resolution (2026-07-02):** all patches applied + 1 pin test; @tr floor 364 → 366 (+2, documented). 694 workspace tests, clippy 0, fmt clean, deny ok, smoke exit 124, lock/deny untouched.

## Dev Notes

### Scope
- **In:** the FR44 hierarchy (native per-currency per bank → converted per bank → global), rate inspectability, honest missing-pair refusals, the Portefeuille consolidated block.
- **Out:** reinvestable-cash conversion; open-profit-at-risk; concentration (FR45 = 6.7); per-bank reference currencies; rate staleness thresholds (#90); provider work of any kind.

### Design decisions (grounded, post-PR #91)
- **App-side orchestration, core-side arithmetic** (the 6.2/6.5 split): grouping in the app; every multiply via `core::risk::fx::convert`; every sum `checked_add` → None on overflow. `core::risk` API unchanged.
- **Reference buckets convert at identity** — no `reference→reference` rate row is ever looked up or required (a self-rate would be nonsense data).
- **latest_fx_rate(pair, None)**: the newest stored rate wins (6.5's created_at arbitration); its `(date, source)` is ALWAYS displayed. Missing ⇒ absent-with-name, never inverted, never partial-as-total.
- **Sold holdings excluded**: CaR is position risk (Appendix A) — unchanged 4.6/6.2 semantics (contrast: the 6.4 cash read includes them; the 6.5 pair SET includes their currencies for future cash conversion — three deliberately different reads, each documented).
- **Extract, don't duplicate**: `portfolio_capital_at_risk_by_currency` becomes a thin wrapper over a shared per-portfolio bucket helper the consolidation also uses.

### Where things live (verified paths, post-PR #91)
- `app/src/state/holdings.rs` — `portfolio_capital_at_risk_by_currency` (the grouping to extract), `effective_currency`.
- `app/src/state/fx.rs` — `list_fx_rates`; `persistence/src/fx.rs` — `latest_fx_rate`; `core/src/risk/fx.rs` — `convert`.
- `app/src/wiring/holdings.rs` — `refresh_holdings` (the CaR/cash push pattern + `format_scaled(DisplayField::Price/Percent, format)`).
- `app/ui/screens/portfolio.slint` — the CaR block to extend; `app/src/posture.rs` — floors (`@tr` 356, MSG 98).

### Previous story intelligence (6.5 + review)
- A display fold fails PER ROW/BUCKET, never blanking the whole surface; every converted figure must carry date+source; missing ⇒ named refusal.
- Posture floors are exact-count disciplines (probe trick documented in posture.rs history).
- The Fx panel re-render sites (journal switch/import/restore) now exist — the consolidation block rides `refresh_holdings`, which those sites already call.

### References
- [prd.md#FR44] + PRD line 63 (the hierarchy + "FX applied ONLY at these consolidation points").
- [prd.md#Appendix A] — CaR formula; "converted at current FX for the global total".
- [6-5-fx-acquisition-consolidation.md#Review Findings] — the arbitration/honesty rules this story consumes.

## Dev Agent Record

### Agent Model Used

Claude Fable 5 (story creation + dev, 2026-07-02).

### Debug Log References

### Completion Notes List

- Ultimate context engine analysis completed - comprehensive developer guide created.
- The FR44 hierarchy landed app-side on the 6.5 primitives: `car_buckets_by_currency` EXTRACTED from the 6.2 read (thin wrapper preserved), `journal_capital_at_risk_consolidation` (per bank: native buckets → converted subtotal at `latest_fx_rate` + `core::risk::fx::convert`, identity for the reference, checked sums; global = try_fold over banks), every absence honest (named missing pairs / plain `unavailable`), every rate used surfaced `(pair, rate, date, source)` in the footnote. `core::risk` unchanged; conversion still has exactly TWO call sites, both in the consolidation read (FR28 structural).
- UI: « Consolidation (toutes banques, en {ref}) » block under the native CaR lines — per-bank lines (converted / « indisponible : taux manquant … » / plain « indisponible »), « Total global » with % du capital investi, the rates footnote; hidden when there is nothing to consolidate (single bank, all-reference). Re-rendered by every holdings/ledger mutation AND both FX mutation paths (review fix).
- Gates: 694 workspace tests (274 app, +4 story tests incl. the overflow pin); clippy 0; fmt clean; deny ok; smoke exit 124; NO migration; NO new dependency; @tr 366; MSG 98 (unchanged — every new string is @tr).
- #90 note at close-out: the rate-staleness murmur and the lookup-error-vs-missing message distinction belong to the rates-panel management work.

### File List

- `app/src/state/holdings.rs` — `car_buckets_by_currency` extracted (wrapper kept); `app/src/state/fx.rs` — `BankConsolidation`/`JournalConsolidation` + the consolidation read (+ review hardening); `app/src/state/tests.rs` — +4 tests.
- `app/src/wiring/holdings.rs` — the consolidation push (checked pct, data-only footnote); `app/src/wiring/fx.rs` + `app/src/wiring/fetch.rs` — register re-render on FX mutations.
- `app/ui/state.slint` — `BankCarRow` + Holdings consolidation properties; `app/ui/screens/portfolio.slint` — the block; `app/ui/app.slint` — exports; `app/src/posture.rs` — @tr 366.

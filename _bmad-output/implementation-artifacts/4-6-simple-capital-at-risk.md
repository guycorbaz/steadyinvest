# Story 4.6: Simple capital-at-risk

Status: done (3-layer review 2026-06-29 — 4/4 ACs satisfied; 2 patches applied [locale percent + doc comment], 2 deferred → GitHub, 3 dismissed; workspace 515 tests, fmt/clippy -D green; core::ssg fingerprint intact; no migration/contract change)

## Story

As Guy,
I want a single capital-at-risk figure for my portfolio,
So that I understand my downside at a glance.

## Acceptance Criteria

1. **AC1 — The capital-at-risk formula (Appendix-A, `core` math, FR43).** Capital-at-risk = `Σ max(0, (avg_cost − stop)) × qty`, summed over holdings, counted **only** where a trailing stop is set **and** `stop ≤ avg_cost`. A holding with no stop, or whose stop has ratcheted **above** cost, contributes **0** (its capital-loss risk is gone). The result is **≥ 0 by construction**. The sum is a **pure `core::risk` function** (exact Decimal, deterministic), beside the Story-4.5 ratchet — decoupled from `core::ssg`.
2. **AC2 — Single reference currency (no FX in Epic 4).** All holdings are summed in the portfolio's single reference currency (FR63); there is **no** FX conversion (Epic 6). `avg_cost` = the holding's `purchase_price` (single lot in Epic 4); `stop` = the persisted `trailing_stop_level` (Story 4.5); `qty` = the holding's quantity.
3. **AC3 — Shown on the Portefeuille surface, recomputed on every refresh.** The figure is displayed on the Portefeuille surface as a **neutral fact** (ink, no hue, no action verb — FR13), labelled with the reference currency, plus its share of invested capital (`÷ Σ purchase_price × qty`, as a percent — omitted when invested = 0). It is recomputed whenever the register re-renders — including after a manual price refresh (Story 4.4) that ratchets stops (Story 4.5). A portfolio with no at-risk holdings shows `0`.
4. **AC4 — `core` decoupled; no schema / contract change.** The new math lives in `core::risk` (additive); the SSG **method fingerprint / golden / determinism** gates stay green. **No migration, no `contract` change, no new dependency** (`Cargo.lock` / `deny.toml` unchanged) — the inputs all already persist (purchase_price, trailing_stop_level, quantity). Copy neutral, posture-gated.

## Tasks / Subtasks

- [x] **Task 1 — `core::risk`: the capital-at-risk sum (AC1, AC4)** — `core/src/risk/mod.rs`
  - [x] A small input type (e.g. `pub struct PositionRisk { pub avg_cost: Decimal, pub stop: Option<Decimal>, pub quantity: Decimal }`) and `pub fn capital_at_risk(positions: &[PositionRisk]) -> Decimal` = `Σ over positions with stop.is_some() && stop ≤ avg_cost of (avg_cost − stop) × quantity`. `≥ 0` by the `stop ≤ avg_cost` guard. Exact Decimal, no `f64`.
  - [x] (helper) `pub fn total_invested(positions: &[PositionRisk]) -> Decimal` = `Σ avg_cost × quantity` (for the % display; `None` %/omit when 0).
  - [x] Unit tests: a stop below cost contributes `(cost−stop)×qty`; a stop **above** cost contributes 0; no stop → 0; mixed portfolio sums correctly; empty → 0; the `≥ 0` invariant; exact-decimal scale. **Does NOT touch `core::ssg` — fingerprint/golden/determinism stay green.**
- [x] **Task 2 — App: read the holdings into the sum (AC1, AC2, AC3)** — `app/src/state.rs`
  - [x] `portfolio_capital_at_risk(&self) -> (Decimal, Decimal)` (or a small struct): map `list_holdings()` → `PositionRisk { avg_cost = purchase_price, stop = trailing_stop_level, quantity }` (parse the TEXT decimals; a holding whose numbers don't parse is skipped defensively), return `(capital_at_risk, total_invested)`. Pure read (no journal write).
  - [x] Test: a portfolio with a mix (stop-below-cost, stop-above-cost, no-stop) returns the expected at-risk + invested totals.
- [x] **Task 3 — main.rs + Slint: the Portefeuille figure (AC3)** — `app/src/main.rs`, `app/ui/{state.slint, screens/portfolio.slint}`
  - [x] `Holdings` global gains `capital-at-risk: string` + `capital-at-risk-pct: string` (already-formatted display strings; "" / "0" handled in Rust). `refresh_holdings` computes them via `state::portfolio_capital_at_risk` + `format_scaled` (Price) and the percent (1 decimal), so they refresh with the register (and after a price refresh).
  - [x] portfolio.slint: a neutral header fact under the reference-currency line — `Capital à risque : {} {} ({} % du capital investi)` (drop the `(… %)` clause when invested = 0). Neutral ink, no hue, no action. Keep the 4.3/4.4/4.5 register intact.
  - [x] Posture: any new `MSG_*` registered + count bumped; `@tr` floor bumped by the exact number of new literals (probe empirically).
- [x] **Task 4 — Gates (AC4)** — fmt, clippy -D, test --workspace, deny + smoke launch. Confirm `core::ssg` re-diffs clean (fingerprint/golden/determinism green); `contract` / `Cargo.lock` / `deny.toml` unchanged; no migration (REGISTRY stays v3).

## Dev Notes

### Scope
The **single** capital-at-risk figure (vs purchase price) + its share of invested capital, on the Portefeuille surface, recomputed on refresh. Pure `core::risk` math over the already-persisted holdings.

### Out of scope (deferred)
- **Open-profit-at-risk** (vs *current* price — the PRD's second view) → a later refinement; 4.6 ships the capital-at-risk (vs cost) figure the epics AC specifies.
- **Multi-currency / FX aggregation** (Epic 6) — 4.6 is single-currency.
- **Concentration / position-sizing limits** (FR44/45) — separate stories.
- The **neutral sell / raise-stop action sheet** on a breach (FR47) → Story 4.7.

### Where things live
- **`core/src/risk/mod.rs`** (additive, beside `ratchet_trailing_stop`): the pure sum. Decoupled from `core::ssg` (the fingerprint/golden gates must re-diff clean).
- **`app/src/state.rs`**: a read-only `portfolio_capital_at_risk` over `list_holdings()` (parses the TEXT decimals; the inputs already persist — purchase_price, trailing_stop_level from Story 4.5, quantity).
- **`app/src/main.rs` `refresh_holdings`** + **`app/ui/{state.slint, screens/portfolio.slint}`**: the formatted figure on the Portefeuille header, recomputed each render (so a price refresh → stop ratchet → CaR all flow through the existing rebuild).

### Notes
- **≥ 0 invariant** holds by the `stop ≤ avg_cost` guard (a stop above cost yields a negative `(cost−stop)` that is excluded, never subtracted). No `max(0, …)` needed beyond the guard, but assert it in a test.
- **No stop set ⇒ 0 contribution** — this metric measures *stop-protected* downside; an un-stopped holding has no defined stop-loss so it is not part of capital-at-risk (per the Appendix-A / PRD definition). (A later story could surface un-stopped exposure separately.)
- **Defensive parsing**: a holding whose TEXT decimals don't parse is skipped (never panics, never a wrong figure) — they always parse in practice (validated on write).

### Manual on-display GO/NO-GO (Guy)
Set stops on two holdings (one stop below cost, one ratcheted above cost) → the figure equals only the below-cost holding's `(cost−stop)×qty`; the above-cost one adds nothing; an un-stopped holding adds nothing; the % matches `÷ Σ cost×qty`; a price refresh that ratchets a stop updates the figure; an empty portfolio shows `0`.

### Review Findings (3-layer adversarial — 2026-06-29)

3 layers (Blind Hunter / Edge Case Hunter / Acceptance Auditor). All 4 ACs satisfied (no migration / contract / dep change; `core::ssg` intact; `@tr` floor +2 exact). 0 decision-needed, 2 patch, 2 defer, 3 dismissed.

- [x] [Review][Patch] Percent display is not locale-aware — figure uses `format_scaled` (comma under FR preset) but the percent uses `.round_dp(1).normalize().to_string()` (always `.`), so a French-preset display reads `1 234,57 CHF (12.5 %)`. Use the existing `format_scaled(car/invested*100, DisplayField::Percent, format)` (scale 1 dp, locale-aware) for the non-empty branch; keep the `""`-on-invested=0 branch. [app/src/main.rs:336-344]
- [x] [Review][Patch] Doc comment inaccuracy — `capital-at-risk` is `"0.00"` (Price scale) when none, not `"0"` as the comment claims. Fix the comment. [app/ui/state.slint:597]
- [x] [Review][Defer] Decimal overflow → panic on unbounded manual qty/price — `(avg_cost−stop)×quantity` / `.sum()` panic if a product/sum exceeds `Decimal::MAX` (~7.9e28); `validate_holding_amounts` checks sign only, never magnitude, so an absurd persisted row crashes the Portefeuille on every render. Pre-existing write-side validation gap; unrealistic trigger but crash-class. [app/src/state.rs:1837 validate_holding_amounts] — deferred → GitHub #60
- [x] [Review][Defer] `0,0 % du capital investi` shown when no stop is set — spec-compliant (AC3: no-at-risk → 0, % omitted only when invested=0) but can read as "no downside exposure". Product/UX question for Guy. [app/src/main.rs:336-344] — deferred → GitHub #61 (product decision)

Dismissed (3, noise/handled): malformed-field skip drops position from both sums (defensive parse is intentional per Dev Notes); malformed stop → treated as no-stop (validated on write); negative qty/stop (qty>0 + stop≥0 enforced on write).

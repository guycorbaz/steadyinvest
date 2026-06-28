# Story 4.5: Trailing stop per holding (ratchet-up only)

Status: review (dev complete 2026-06-28 — 5/5 tasks; workspace 507 tests, fmt/clippy -D/deny green; core::ssg fingerprint intact; awaiting 3-layer review + GO/NO-GO)

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want a trailing stop I set per holding (as a percentage) that ratchets up only,
So that I define my own capital-protection threshold and it never loosens on its own.

## Acceptance Criteria

1. **AC1 — Set / clear a per-holding trailing stop (%).** On the Portefeuille register, each holding gets a control to set a trailing-stop **percentage** (e.g. `15` = 15 %), validated as an exact decimal in `(0, 100)`; an empty value clears the stop. The value persists to `holdings.trailing_stop_pct` (the column already exists, NULL until now). Invalid input → a neutral notice, nothing written (mirrors the 4.3 amount validation). (FR42)
2. **AC2 — The stop level ratchets UP only, never down.** The persisted stop **level** (a price) is `max(prior_level, reference_price × (1 − pct/100))`, recomputed (a) when the pct is set/changed and (b) on every holdings price refresh (Story 4.4). A falling price never lowers the level; a rising price raises it. The ratchet math is a **pure `core::risk` function** (exact decimal, deterministic). `reference_price` = the linked study's `current_price` when known, else the holding's `purchase_price`. (FR42)
3. **AC3 — Persist the ratcheted level (schema v2→v3).** A new `holdings.trailing_stop_level TEXT` column (NULL when no stop set) holds the ratcheted price, so the ratchet survives restarts (the high-water-mark can't be re-derived from the latest price alone). This is the project's **second** migration: `PRAGMA user_version 2 → 3`, `ALTER TABLE holdings ADD COLUMN trailing_stop_level TEXT`. `contract::SCHEMA_VERSION` stays unchanged (holdings is a normalized table, not a blob). No new `v3.db` corpus — a forward-migration test proves NFR-R3.
4. **AC4 — Neutral stop display.** Each holding with a stop set shows, as **neutral facts** (ink + glyph, never a saturated hue, never an action verb — FR13): the stop level (labelled with the reference currency), and a neutral state — `sous le stop` (current price ≤ stop level) vs the distance above it. A holding with no stop shows nothing extra. The display is recomputed each render; **no alert, no action sheet** (the sell / raise-stop action sheet is Story 4.7).
5. **AC5 — Configurable default % in Settings (FR63 thresholds).** Réglages exposes a **default trailing-stop %** (append-only `AppConfig` field, validate-on-read, like `reference_currency`). When the user opens the set-stop control with no existing value, it pre-fills the default. Changing the default does not retroactively touch existing holdings.
6. **AC6 — `core::risk` is decoupled; the SSG method stays frozen.** The ratchet lives in a NEW `core/risk/` module, **separate** from the SSG engine (PRD: risk is a decoupled overlay that never weighs down the pure SSG calc). The SSG **method fingerprint / golden corpus / determinism gates stay green** (no SSG method change). All copy neutral, posture-gated.

## Tasks / Subtasks

- [x] **Task 1 — `core::risk`: the ratchet-up-only formula (AC2, AC6)** — NEW `core/src/risk/mod.rs` (+ `pub mod risk;` in `core/src/lib.rs`)
  - [x] `pub fn ratchet_trailing_stop(prior_level: Option<Decimal>, reference_price: Decimal, pct: Decimal) -> Decimal` = `max(prior_level.unwrap_or(MIN), reference_price * (1 - pct/100))`; exact `rust_decimal` math, no `f64`; `pct` assumed already validated `(0,100)`. Ratchet-up-only by construction (the `max` with the prior level).
  - [x] (optional helper) `pub fn stop_breached(stop_level: Decimal, current_price: Decimal) -> bool` = `current_price <= stop_level` — a pure state, used by the display.
  - [x] Unit tests: a rising reference price raises the level; a falling price leaves it unchanged (the ratchet); first-set (prior `None`) seeds from the reference; exact-decimal scale preserved; pct at the edges. **Does NOT touch `core::ssg` / `core::method` — method fingerprint + golden + serde corpus stay green** (assert by re-running those gates).
- [x] **Task 2 — Persistence: the v2→v3 migration + `trailing_stop_level` CRUD (AC1, AC3)** — `persistence/src/{migrations,schema,holdings}.rs`
  - [x] Migration registry: add the v2→v3 step `ALTER TABLE holdings ADD COLUMN trailing_stop_level TEXT` (reuse the `migrate_to_v2` pattern; `PRAGMA user_version` 2→3). Keep `migrate_to_v2` intact. The harness already exercises a two-step registry (`fake_v2`/`TWO_STEP_REGISTRY`) — extend to three steps.
  - [x] `HoldingItem` gains `trailing_stop_level: Option<String>`; `add_holding` leaves both stop fields NULL; SELECTs read the new column; a focused `set_trailing_stop(holding_id, pct: Option<String>, level: Option<String>)` writes both (the rail computes them). Preserve every Story-4.3 idempotency no-op guard (a no-op set must not bump `logical_version` — C4 Synology-sync lesson).
  - [x] Tests: forward-migrate a v2 journal → v3 (column present, existing rows get NULL, NFR-R3); set/clear the stop round-trips; idempotent re-set is a no-op.
- [x] **Task 3 — App state: the set-stop rail + ratchet-on-refresh (AC1, AC2, AC5)** — `app/src/state.rs`
  - [x] `set_holding_trailing_stop(holding_id, pct_input: &str)` — validate the pct as an exact decimal in `(0,100)` (empty → clear both fields); on set, compute the initial level via `core::risk::ratchet_trailing_stop(prior_level, reference_price, pct)` where `reference_price` = the matched study's `current_price` (via `study_id_for_ticker` + `get_study`) else `purchase_price`; persist pct + level through the holdings rail. 1–2 new `MSG_*` (invalid pct).
  - [x] Ratchet-on-refresh: when a holdings price refresh lands a new `current_price` (Story 4.4's `apply_holding_price` path), for each holding of that ticker with a stop set, ratchet `trailing_stop_level` and persist. (Thread it so the refresh updates the stop in the same surface.)
  - [x] `AppConfig.default_trailing_stop_pct: Option<String>` (append-only `#[serde(default)]`, validate-on-read) + accessor.
- [x] **Task 4 — App main + Slint: per-row stop control + neutral display + Settings default (AC1, AC4, AC5)** — `app/src/main.rs`, `app/ui/{state.slint, screens/portfolio.slint, screens/settings.slint}`
  - [x] `HoldingRow` gains `stop-level: string`, `stop-pct: string`, `stop-breached: bool` (+ a `has-stop: bool`). `Holdings` global gains a `set-trailing-stop(id, pct)` callback. `refresh_holdings` fills the stop fields (level formatted + breach state from `core::risk`).
  - [x] Portefeuille row: a compact set-stop control (a small field + apply, pre-filled with the default %) and the neutral stop facts (`stop : {} {currency}` + `sous le stop` / `à {} au-dessus`). Glyphs inside `@tr`; neutral ink; **no action verb, no hue** (geofenced). Keep the 4.3/4.4 register + zone/freshness columns intact.
  - [x] Réglages: a default trailing-stop % field (validate, persist, mirror on startup) beside the reference-currency picker.
  - [x] Posture: register new `MSG_*`, bump the exact `USER_FACING_MESSAGES` count; bump the `@tr` floor by the exact number of new literals (probe empirically).
- [x] **Task 5 — Gates (AC6)** — run all gates `--locked` (fmt, clippy -D, test --workspace, deny) + smoke launch. **Confirm `core::ssg` re-diffs clean** (method fingerprint / golden / determinism green — the new `core::risk` is additive and the SSG calc is untouched). `Cargo.lock`/`deny.toml` unchanged (no new dep).

## Dev Notes

### Scope decisions (Guy, 2026-06-28 — read first)
- **% only.** The trailing stop is a **percentage** parameter for this story. The PRD's **ATR** mode (needs OHLC volatility / true-range history) and a **manual fixed-price** mode are **deferred** to a later refinement — they widen the surface (extra fetch/calc, per-holding mode selection) beyond FR42's core "ratchet-up-only" requirement.
- **Persist the ratcheted level (migration v2→v3).** "Ratchets up only" cannot be re-derived from the latest price alone (that loses the high-water mark), so the ratcheted **stop level** (a price) is persisted in a new `holdings.trailing_stop_level` column. Storing the *level* (not the peak price) keeps Story 4.6's capital-at-risk formula direct: `Σ max(0, (purchase − stop_level)) × qty`. This is the project's **second** migration (mirrors Story 4.1's v1→v2); the migration harness already supports a multi-step registry.

### In scope
Set/clear a per-holding trailing-stop %; persist + ratchet the stop level (up-only) on set and on each price refresh; a neutral per-holding stop display (level + breach/distance fact); a configurable default % in Settings. The ratchet math is pure `core::risk`.

### Out of scope (deferred)
- The neutral **sell / raise-stop action sheet** on a stop breach or Sell-zone entry → **Story 4.7** (FR47); 4.5 shows the breach only as a neutral fact, never an action.
- **Capital-at-risk** aggregate (Σ across the portfolio) → **Story 4.6** (FR43); 4.5 persists the per-holding `stop_level` that 4.6 sums.
- **ATR** and **manual fixed-price** stop modes (this story is %-only).
- Multi-currency / FX aggregation (Epic 6); the stop level is in the security's price terms (same currency as `current_price`/`purchase_price`).

### Architecture / where things live
- **`core/risk/` (NEW, additive):** the ratchet formula + breach predicate, pure exact-decimal. Architecture maps FR42-48 → `core/risk/`. It is a **decoupled** subsystem (PRD): it does NOT touch `core::ssg` / `core::method`, so the SSG method fingerprint / golden corpus / determinism gates stay green. First `core` change in Epic 4 — keep it isolated under `core::risk`.
- **`persistence`:** the v2→v3 migration + `trailing_stop_level` column + `HoldingItem` field + `set_trailing_stop` CRUD. [Source: persistence/src/migrations.rs (the v1→v2 `migrate_to_v2` precedent), schema.rs (holdings DDL — `trailing_stop_pct` exists, `trailing_stop_level` is new), holdings.rs]
- **`app`:** `set_holding_trailing_stop` rail (validate + compute initial level + persist) + ratchet-on-refresh hook in the Story-4.4 `apply_holding_price` path + `AppConfig.default_trailing_stop_pct` + the Portefeuille per-row control/display + Réglages default. [Source: app/src/state.rs holdings rail (4.3) + `apply_holding_price` (4.4, #50); app/src/main.rs `refresh_holdings` + holdings callbacks; app/ui/screens/{portfolio,settings}.slint]

### Ratchet semantics (the crux)
- `stop_level = max(prior_level_or_−∞, reference_price × (1 − pct/100))`.
- On **set** (no prior level): `reference_price` = study `current_price` if linked+known, else `purchase_price`; seed the level from it.
- On **refresh** (new `current_price`): ratchet against the new price; the level only rises.
- On **pct change** (existing level): recompute the candidate from the current reference price and `max` with the prior level — lowering the pct can RAISE the candidate (tighter stop) and ratchet up; raising the pct yields a lower candidate that the `max` ignores (the level never drops). State this explicitly in a test.
- **Breach** = `current_price ≤ stop_level` — a neutral fact only (FR42 sets/ratchets; the action is 4.7).

### Guards & posture (reuse the Epic-2/3/4 rails)
- Exact-decimal validation (`Decimal::from_str_exact`, `(0,100)`) like the 4.3 amount validation; invalid → neutral notice, nothing written.
- Idempotency: a no-op `set_trailing_stop` (same pct+level) must not bump `logical_version` (C4 Synology-sync).
- All new copy neutral, no banned verb (FR13); glyphs inside `@tr`; bump the posture floors empirically.
- Read-only journal / no-journal / save-failure guards on the new rail (no silent `.ok()`).

### Risks / watch-items
- **The migration is the highest-risk piece** (second-ever) — prove the v2→v3 forward path on a real v2 journal (existing rows get NULL `trailing_stop_level`), and that a v3 journal still opens. Keep `migrate_to_v2` byte-for-byte intact.
- **Keep `core::risk` fully decoupled** — do not import or be imported by `core::ssg`; the SSG gates must re-diff clean. If any SSG golden/fingerprint test moves, the decoupling was violated.
- **Ratchet-on-refresh ordering:** the stop ratchet must run AFTER `current_price` is filled (Story 4.4 `apply_holding_price`), in the same surface, so the displayed stop reflects the just-refreshed price.

### Manual on-display GO/NO-GO (Guy, after dev+review)
Set a 15 % stop on a holding → stop level shows ≈ price × 0.85; refresh with a higher price → stop ratchets up; refresh with a lower price → stop unchanged (the ratchet); price ≤ stop → neutral `sous le stop` fact (no action sheet); set a default % in Réglages → the set-stop control pre-fills it; restart → the stop level persists (migration).

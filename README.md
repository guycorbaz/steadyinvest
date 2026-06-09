# steadyinvest

A personal, **offline-first desktop application** (Rust + Slint, local SQLite) for disciplined,
self-directed investment decisions. It faithfully reproduces the *method* of the
NAIC/BetterInvesting Stock Selection Guide (SSG) — the formulas, which are not protectable — using
**neutral terminology and original layouts**, and layers on an interactive analytical experience:
live recalculation, draggable judgment lines, and color-coded buy/hold/sell zones.

> **Independent project — not affiliated with NAIC / BetterInvesting / ICLUBcentral.**
> This software surfaces **facts, never recommendations**. It is **educational** and **does not
> replace a financial advisor**. The user is the sole decider. *(Design intent, not a legal opinion.)*

## What it is

steadyinvest is **not merely a faster stock-study tool**. It is a durable personal system of
investment discipline with a **cumulative memory of judgments** — a sovereign, revisitable journal
of *why* each decision was made, confrontable against real outcomes over time. It is framed around
the full loop: *discover → study → buy → watch portfolio risk → protect → exit → redeploy*.

### v1 (MVP) scope
Faithful Stock Study (auto-fetch + first-class manual entry, per-cell provenance); interactive
growth/valuation chart (draggable judgment lines, colored zones, live recalc); calculations in the
security's native currency; watchlist with neutral buy-zone alerts; a **single-portfolio,
single-currency** holdings register with a **simple capital-at-risk**; local SQLite with a
journal-ready, **versioned data contract**. (Multi-portfolio, multi-currency/FX, the full
transaction ledger and the complete risk overlay are **Phase 2**.)

### Out of scope
Brokerage / order execution; regulated or personalized investment advice; real-time intraday data;
multi-user / accounts / cloud sync. No vendor market data ships with the app — **you bring your own
data-provider API key** (stored in the OS keychain).

## Architecture (summary)

A **Cargo workspace** with a thin Slint UI over a UI-independent, tested calculation core:

| Crate | Role |
|-------|------|
| `core` | Pure, deterministic SSG calculation engine (`rust_decimal`, no I/O/UI/SQL/net) |
| `contract` | Versioned serde data contract (`schema_version` / `method_version`), decoupled from Slint & SQLite |
| `ingestion` | Provider-agnostic acquisition + normalization (IFRS↔GAAP, splits, fiscal periods, currency) |
| `persistence` | rusqlite (bundled SQLite) hybrid store, journal identity, migrations, export/import |
| `report` | PDF/print (faithful, neutral, grayscale-safe), UI-independent |
| `app` | Thin Slint UI — native charts (`Path`/`TouchArea`), app-config, OS keychain |

**Foundational Invariant:** every asserted fact (input, derived value, verdict, journal) carries a
dated proof of *(source, version, timestamp, hash-of-dependencies)* — any break in that link is a
**visible event, never a silence**. Charts are drawn **natively in Slint** (no web, no egui).

Full planning artifacts (PRD, UX, Architecture, Epics & Stories) live in
[`_bmad-output/planning-artifacts/`](_bmad-output/planning-artifacts/).

## Status

Pre-implementation. Planning complete (PRD · UX · Architecture · Epics & Stories · Implementation
Readiness — all done, FR coverage 100%). Phase 4 (implementation) starting with the Cargo workspace
scaffold and the Week-1 native-Slint charting spike.

## License

[GPL-3.0](LICENSE). License applied from the start even while the repository is private
(license ≠ distribution), subject to a dependency-license audit (`cargo deny`).

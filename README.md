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
| `persistence` | rusqlite (bundled SQLite) hybrid store, journal identity, migrations, export/import, the gated MCP access surface |
| `report` | PDF/print (faithful, neutral, grayscale-safe), UI-independent |
| `paths` | The per-machine locations (app-config, default dossier, logs), shared by the app and the MCP server |
| `app` | Thin Slint UI — native charts (`Path`/`TouchArea`), app-config, OS keychain |
| `mcp` | `steadyinvest-mcp`, the stdio MCP server an AI client launches: reads studies, records propositions — never writes a study, never sees the portfolio, keys or config |

**Foundational Invariant:** every asserted fact (input, derived value, verdict, journal) carries a
dated proof of *(source, version, timestamp, hash-of-dependencies)* — any break in that link is a
**visible event, never a silence**. Charts are drawn **natively in Slint** (no web, no egui).

Full planning artifacts (PRD, UX, Architecture, Epics & Stories) live in
[`_bmad-output/planning-artifacts/`](_bmad-output/planning-artifacts/).

## Status

Epics 1–8 implemented (see [`sprint-status.yaml`](_bmad-output/implementation-artifacts/sprint-status.yaml)):
the faithful study and its charts, provider data and reconciliation, watchlist and portfolio risk,
cumulative memory and portability (history, export/import, backups), multi-portfolio and
multi-currency, comparison / health review / screening / reports, and **AI assistance over MCP**
(human-gated propositions, frozen decision-time verdict). Not in production: the owner builds from
`main`.

## Build and run

```sh
cargo run                      # the desktop app (or: just run)
just ci                        # fmt + clippy -D warnings + tests + cargo-deny, as CI
just mcp-build                 # the MCP server: target/release/steadyinvest-mcp
```

Linux is the verified platform. Configuration lives in `~/.config/steadyinvest/config.json`,
logs in `~/.local/share/steadyinvest/logs/`; the dossier (one SQLite file) is wherever
Réglages shows it.

## Using an AI with SteadyInvest

No AI runs inside the app. An AI client of your choice (verified: Claude Code) launches the
`steadyinvest-mcp` server, which lets it read your studies and deposit **propositions** that change
nothing until you validate them, one by one, in the app. Setup, directories, the in-app workflow,
troubleshooting and an MCP primer for newcomers: **[docs/guide-ia.md](docs/guide-ia.md)** (French).

## Documentation

| Document | Content |
|---|---|
| [docs/guide-ia.md](docs/guide-ia.md) | Using an AI with SteadyInvest (MCP setup, workflow, troubleshooting, MCP primer) |
| [docs/method/ssg-method-spec-v1.md](docs/method/ssg-method-spec-v1.md) | The SSG method as implemented |
| In-app glossary (Réglages) | Every term the app uses, in French |
| [`_bmad-output/planning-artifacts/`](_bmad-output/planning-artifacts/) | PRD, UX specifications, architecture, epics |
| [`_bmad-output/implementation-artifacts/`](_bmad-output/implementation-artifacts/) | Story records, retrospectives, test checklists |
| [docs/review-checklist.md](docs/review-checklist.md) | Development rules ([docs/definition-of-done.md](docs/definition-of-done.md): legacy web-stack document, kept for history — reconciled 2026-10-01) |

## License

[GPL-3.0](LICENSE). License applied from the start even while the repository is private
(license ≠ distribution), subject to a dependency-license audit (`cargo deny`).

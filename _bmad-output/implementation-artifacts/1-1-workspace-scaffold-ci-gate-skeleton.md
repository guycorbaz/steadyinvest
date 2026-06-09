# Story 1.1: Workspace scaffold & CI gate skeleton

Status: ready-for-dev

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->
<!-- Epic 1: Proven SSG core & data foundation (headless). First story of the project (fresh restart 2026-06-05). -->

## Story

As the developer (Guy, solo),
I want a Cargo workspace with the six crates and a cross-platform CI pipeline,
so that every later story builds on a consistent structure with quality gates from day one.

## Acceptance Criteria

1. **Workspace & crates.** Given an empty repository, when the workspace is scaffolded, then the six crates `core`, `contract`, `ingestion`, `persistence`, `report`, `app` exist as workspace members, each with package name `steadyinvest-<crate>` and short directory name (`core/`, `contract/`, …), and the root `Cargo.toml` declares `[workspace]` members + a `[workspace.dependencies]` table pinning the agreed versions (single source of versions; member crates reference deps via `workspace = true`).
2. **Pinned dependencies (in `[workspace.dependencies]`).** slint 1.16, slint-build 1.16, rusqlite 0.40 (feature `bundled`), rust_decimal 1.42 (feature `maths`), reqwest 0.13 (features `rustls-tls`, `json`), tokio 1.52, serde 1 (feature `derive`), thiserror 2.0, proptest 1.9, tracing, keyring 4.0 (no default features), directories, uuid, serde_json. (A dep appears here even if only one crate uses it, so versions are centralized.)
3. **Toolchain & lint/audit config present at workspace root.** `rust-toolchain.toml` pins MSRV ≥ 1.88; `rustfmt.toml`, `clippy.toml`, and `deny.toml` (cargo-deny, GPL-3.0 license policy) exist; `Cargo.lock` is committed (application → reproducible builds); `.gitignore` already present (do not regress it).
4. **`app` UI crate seeded from the official Slint template.** The `app` crate's Slint build wiring (`build.rs` using `slint-build`, a minimal `.slint` file, and an `include!`/`slint::include_modules!` hook in `main.rs`) follows the official Slint Rust template (`cargo generate --git https://github.com/slint-ui/slint-rust-template`); the app launches to an empty/placeholder window. *(Seed step, then rework the generated crate into the workspace `app` member.)*
5. **Crate boundaries enforced from the start (Cardinal Rule skeleton).** `core` declares **no** I/O/UI/SQL/net dependencies (only `rust_decimal`, `serde`); only `persistence` depends on `rusqlite`; only `ingestion` depends on `reqwest`/`tokio`; only `app` depends on `slint`/`directories`/`keyring`; `report` depends only on `core`/`contract` (+ a PDF crate placeholder). Each crate compiles as an empty-but-valid lib (or bin for `app`).
6. **CI green on the 3-OS matrix.** When CI runs on the Windows/macOS/Linux matrix, then `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and `cargo deny check` all pass on the empty workspace.
7. **Determinism-hash placeholder job.** A cross-OS determinism-hash CI job exists and is green: it computes a trivial vector in `core` (using `rust_decimal`), serializes it canonically, and asserts an **identical SHA-256** across the three OS (the job fails the build if the hashes differ).
8. **Dev ergonomics.** A `justfile` exposes at least `run`, `test`, `lint`, `spike` (placeholder), and `ci` tasks mirroring the CI commands so the developer can reproduce gates locally.

## Tasks / Subtasks

- [ ] **Task 1 — Bootstrap the workspace root (AC: 1, 2, 3)**
  - [ ] `cargo new --vcs none` is NOT needed — repo + `.gitignore` + LICENSE already exist; create the root `Cargo.toml` with `[workspace] resolver = "2"`, `members = ["core","contract","ingestion","persistence","report","app"]`.
  - [ ] Add `[workspace.dependencies]` with every pinned dep from AC#2 (centralized versions).
  - [ ] Add `rust-toolchain.toml` (`[toolchain] channel = "1.88"` or newer stable; components `rustfmt`, `clippy`).
  - [ ] Add `rustfmt.toml`, `clippy.toml` (workspace lint config), `deny.toml` (cargo-deny: licenses = allow GPL-3.0 + permissive [MIT, Apache-2.0, BSD, Unicode, Zlib]; flag anything GPL-incompatible).
  - [ ] Commit `Cargo.lock` once it exists.
- [ ] **Task 2 — Create the five non-UI crates (AC: 1, 5)**
  - [ ] `core/` — `cargo new --lib core --name steadyinvest-core`; deps: `rust_decimal` (workspace, feature `maths`), `serde`. NO other deps.
  - [ ] `contract/` — lib `steadyinvest-contract`; deps: `serde`, `rust_decimal`, `uuid`, `serde_json`.
  - [ ] `ingestion/` — lib `steadyinvest-ingestion`; deps: `reqwest`, `tokio`, `serde`, `thiserror`, `steadyinvest-contract`.
  - [ ] `persistence/` — lib `steadyinvest-persistence`; deps: `rusqlite` (feature `bundled`), `steadyinvest-contract`, `serde_json`, `thiserror`.
  - [ ] `report/` — lib `steadyinvest-report`; deps: `steadyinvest-core`, `steadyinvest-contract` (+ a PDF crate placeholder, e.g. `printpdf`/`genpdf`, can be added in the report story — keep minimal here).
  - [ ] Each lib exposes a trivial item so it compiles and `cargo test` finds an (empty) test target.
- [ ] **Task 3 — Seed and rework the `app` crate from the Slint template (AC: 4, 5)**
  - [ ] `cargo install cargo-generate` (if absent), then `cargo generate --git https://github.com/slint-ui/slint-rust-template --name app`.
  - [ ] Rework the generated crate into the workspace `app` member: package name `steadyinvest-app`, deps via workspace table: `slint`, `tokio`, `directories`, `keyring`, `tracing`, `steadyinvest-core`, `steadyinvest-contract`, `steadyinvest-ingestion`, `steadyinvest-persistence`, `steadyinvest-report`; build-dep `slint-build`.
  - [ ] Keep a minimal `app.slint` (placeholder window) wired via `build.rs` + `slint::include_modules!()`; `cargo run -p steadyinvest-app` opens an empty window.
- [ ] **Task 4 — Determinism-hash check in `core` (AC: 7)**
  - [ ] Add `core::determinism_probe()` returning a small `Vec<Decimal>` from a fixed trivial computation (e.g. a tiny CAGR using `rust_decimal`'s `maths::powd`), plus a `determinism_hash() -> String` that serializes the vector to a canonical decimal-string form and returns its SHA-256 (add `sha2` to `core` deps — pure-Rust, GPL-compatible).
  - [ ] A test asserts the hash equals a committed expected constant; CI runs this test on all 3 OS so divergence fails the build.
- [ ] **Task 5 — CI pipeline `.github/workflows/ci.yml` (AC: 6, 7)**
  - [ ] Matrix `os: [ubuntu-latest, macos-latest, windows-latest]`; pinned toolchain from `rust-toolchain.toml`; cache cargo.
  - [ ] Steps: `cargo fmt --all --check` · `cargo clippy --all-targets --all-features -- -D warnings` · `cargo test --all` · `cargo deny check` (install cargo-deny) · the determinism-hash test.
  - [ ] Linux runner note: `keyring` secret-service backend needs no D-Bus at build/test time (it's a runtime concern); ensure `keyring` is added with `default-features = false` so CI build does not require a secret agent.
- [ ] **Task 6 — `justfile` (AC: 8)**
  - [ ] Tasks: `run` (`cargo run -p steadyinvest-app`), `test` (`cargo test --all`), `lint` (`cargo fmt --all --check && cargo clippy --all-targets -- -D warnings`), `ci` (lint + test + `cargo deny check`), `spike` (placeholder echo pointing to Story 1.5).
- [ ] **Task 7 — Verify & commit (AC: all)**
  - [ ] Run `just ci` locally; confirm all gates green; `cargo run -p steadyinvest-app` opens the placeholder window (**Definition of Done: launch the app and visually verify**).
  - [ ] Commit on a feature branch; push; confirm the GitHub Actions matrix is green before marking done.

## Dev Notes

### What this story is (and is NOT)
- **IS:** the scaffolding + quality-gate harness for the whole project. Empty-but-valid crates, correct boundaries, green CI on 3 OS, a working determinism hash, and a launchable empty Slint window.
- **IS NOT:** any SSG math (Story 1.8), the method spec (Story 1.2), the contract types (Story 1.3), persistence schema (Story 1.10), or the charting spike (Story 1.5). Do **not** implement domain logic here — just the skeleton each later story fills.

### Locked technical decisions (must follow exactly)
- **Stack:** Rust + **Slint 1.16 only** — **NO web, NO egui** (egui was removed from the architecture 2026-06-08). Charts will later be native Slint (`Path`/`TouchArea`); nothing chart-related in this story. [Source: architecture.md#Core Technical Decisions]
- **Exact decimal everywhere in the decision chain:** `rust_decimal` (+`maths` for `powd`/`exp`/`ln`); **never `f32`/`f64`** for money/ratios. Cross-OS determinism comes from this choice — the determinism-hash job proves it. [Source: architecture.md#Core Architectural Decisions; prd.md#NFR-C1, NFR-X1]
- **Cardinal Rule:** ALL calculation lives in `steadyinvest-core`; `core` has **zero** I/O/UI/SQL/net deps. Enforce by construction in this story (only `rust_decimal`, `serde`, `sha2` in `core`). [Source: architecture.md#Enforcement Guidelines, #Architectural Boundaries]
- **Three version axes** (relevant later, not implemented here): `schema_version` (blob) · SQLite `PRAGMA user_version` · `method_version` (string). Just be aware. [Source: architecture.md#Core Technical Decisions]

### Naming conventions (apply now — agents diverge here)
- **Packages:** `steadyinvest-core`, `steadyinvest-contract`, `steadyinvest-ingestion`, `steadyinvest-persistence`, `steadyinvest-report`, `steadyinvest-app`. **Directories:** short (`core/`, `contract/`, …). Internal deps via `[workspace.dependencies]` (single version source). [Source: architecture.md#Naming Patterns]
- **Rust:** types/traits `PascalCase`; fns/vars/modules/files `snake_case`; consts `SCREAMING_SNAKE_CASE`; organize by domain (no `utils.rs` grab-bag). rustfmt + clippy enforced. [Source: architecture.md#Naming Patterns]
- **Slint (app crate, for later):** components `PascalCase`, `.slint` files `snake_case`, properties/callbacks `kebab-case`, exported globals `PascalCase`. Only the placeholder window matters this story. [Source: architecture.md#Naming Patterns]

### Target project structure (authoritative — create these crate roots; inner files come in later stories)
```text
steadyinvest/
├── Cargo.toml              # [workspace] members + [workspace.dependencies]
├── Cargo.lock              # committed
├── rustfmt.toml · clippy.toml · deny.toml · rust-toolchain.toml · justfile
├── .github/workflows/ci.yml   # 3-OS: fmt, clippy -D, test, deny, determinism hash
├── core/        (steadyinvest-core — PURE: rust_decimal, serde, sha2)
├── contract/    (steadyinvest-contract — serde, rust_decimal, uuid, serde_json)
├── ingestion/   (steadyinvest-ingestion — reqwest, tokio, serde, thiserror, contract)
├── persistence/ (steadyinvest-persistence — rusqlite[bundled], contract, serde_json, thiserror)
├── report/      (steadyinvest-report — core, contract, + pdf placeholder)
└── app/         (steadyinvest-app — slint, tokio, directories, keyring, tracing, all crates; build.rs slint-build)
```
[Source: architecture.md#Complete Project Directory Structure, #Selected Starter]

### Versions (verified June 2026 by the architecture; confirm latest patch via `cargo add`)
Slint 1.16.1 (MSRV 1.88, GPLv3 — compatible with this project's GPL-3.0) · rusqlite 0.40 (`bundled`) · rust_decimal 1.42 (`maths`) · reqwest 0.13 (`rustls-tls`, `json`) · tokio 1.52 · thiserror 2.0 · proptest 1.9 · keyring 4.0.1 (no default features) · directories. **Do not downgrade or swap** without recording why (cargo-deny will also gate licenses). [Source: architecture.md#Versions Verified]

### CI / determinism specifics
- `rustls-tls` is chosen so no system OpenSSL is needed → portable single binary and simpler CI. [Source: architecture.md#API & Communication Patterns]
- `keyring` with `default-features = false` avoids needing a D-Bus/secret agent on the Linux CI runner (backend is a runtime concern). [Source: architecture.md#Versions Verified]
- The determinism-hash job is the **early proof** of NFR-X1 (identical numeric results across OS) and NFR-C1 (deterministic engine); keep it tiny but real (a `rust_decimal` computation, not a string constant). [Source: prd.md#NFR-C1/NFR-X1; epics.md Story 1.1 AC]

### Testing standards
- Unit tests co-located in `#[cfg(test)] mod tests`; integration tests in each crate's `tests/`. This story only needs the determinism test + each crate compiling a (possibly empty) test target so `cargo test --all` is meaningful. Golden/property/metamorphic suites arrive in Stories 1.7–1.9. [Source: architecture.md#Structure Patterns]
- **Definition of Done includes "launch the app and visually verify"** (the prior project shipped a blank chart marked done for 4 epics because nothing rendered it). For this story: confirm `cargo run -p steadyinvest-app` opens the placeholder window. [Source: architecture.md#Infrastructure & Deployment (UI visual-verification strategy)]

### Process guardrails (start as you mean to go on)
- No `.unwrap()`/`.expect()` in non-test code (except documented `// INVARIANT:`); no silent `.ok()`. Per-crate `thiserror` error enums arrive with real logic; not required for empty crates but don't add bad patterns. [Source: architecture.md#Process Patterns]
- Pattern violations / deferred items → **GitHub Issues** (single source of truth), never inline TODO debt tables. [Source: architecture.md#Enforcement Guidelines; project memory]

### Project Structure Notes
- The repo already exists (git initialized, `main`, remote `origin` = `github.com/guycorbaz/steadyinvest`, **public**) with `.gitignore`, `LICENSE` (GPL-3.0), `README.md`, and the `_bmad-output/` planning artifacts. **Do not** re-init git or overwrite these. The `.gitignore` already ignores `/target/`, `*.db`, secrets, `/backups/`, `/exports/`, and a TEMP block excluding legacy `_bmad-output/implementation-artifacts/` etc. — **note:** that TEMP block currently ignores this very directory; the new Rust source lives at the repo root (`core/`, `app/`, …), which is NOT ignored, so the scaffold commits normally.
- **Legacy contamination warning:** `_bmad-output/implementation-artifacts/` and `sprint-status.yaml` contain OLD-PROJECT (web/Leptos/Loco) story files — they are NON-INPUTS. Ignore them entirely; this story follows the new `architecture.md`/`epics.md` only. [Source: architecture.md notes]

### References
- [Source: epics.md#Story 1.1: Workspace scaffold & CI gate skeleton] — user story + acceptance criteria
- [Source: epics.md#Epic 1: Proven SSG core & data foundation (headless)] — epic goal, ADD1/ADD2/ADD14 includes
- [Source: architecture.md#Selected Starter: Custom Cargo workspace] — init commands, workspace layout, crate list
- [Source: architecture.md#Versions Verified (web, June 2026)] — pinned versions + rationale (Slint GPLv3 compat, rustls-tls, keyring no-default-features)
- [Source: architecture.md#Naming Patterns / #Structure Patterns / #Enforcement Guidelines] — crate/package naming, justfile, Cardinal Rule, CI gates
- [Source: architecture.md#Complete Project Directory Structure] — full target tree
- [Source: architecture.md#Infrastructure & Deployment] — 3-OS CI matrix, determinism hash, visual-verification DoD
- [Source: prd.md#NFR-C1, NFR-C5, NFR-X1] — determinism, CI-gated correctness, cross-OS numeric identity
- [Source: architecture.md#Implementation Handoff] — "First Implementation Priority": scaffold workspace + 6 crates (+ seed UI) = the first story

## Dev Agent Record

### Agent Model Used

(to be filled by dev-story)

### Debug Log References

### Completion Notes List

- Ultimate context engine analysis completed — comprehensive developer guide created.

### File List

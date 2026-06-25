# Story 3.2: Provider configuration & API keys in the OS keychain

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to manage provider keys securely and use keyless providers,
so that my credentials never live in the repo or config and I can switch providers.

## Acceptance Criteria

(From epics.md §Story 3.2, lines 763–775. FR25, FR63 provider/key, NFR-S1. Scope-resolved with Guy 2026-06-25 — see Dev Notes "Scope decision".)

1. **AC1 — Manage a key in the OS secret store.** Given the Réglages screen (no wizard), when Guy **adds / replaces / deletes** a provider API key, then the key is stored **only** in the OS secret store via `keyring` 3.x (secret-service backend; `async-secret-service`/`async-io`, see Task 1) and is **never** written to `config.json`, logs, exports, or backups (FR25, NFR-S1). The Settings panel shows a **status** ("configured" / "not configured") but **never echoes the secret value back** to the screen.
2. **AC2 — Test a key (real, off-thread).** Given a configured key, when Guy clicks **Tester**, then the app performs a **minimal live validation fetch** through the Story-3.1 worker path (off the UI thread, returned via `invoke_from_event_loop`) and reports **success or a cause-named failure** (invalid/absent key · network · quota) via the neutral notice — never blocking the UI.
3. **AC3 — Keyless provider.** A provider that needs no key can be selected and used with **no key configured**; the fetch path passes `api_key = None` and the engine/fetch behaves exactly as Story 3.1's keyless branch (FR25). Selecting a key-requiring provider with no key configured surfaces the existing `MSG_PROVIDER_NO_KEY` neutral notice at fetch time (unchanged behaviour).
4. **AC4 — Preferred provider recorded & injected by the app.** The chosen provider is recorded in **app-config** (`AppConfig.preferred_provider`, append-only `#[serde(default)]`), and the app injects the key into `ingestion` at fetch time. The key is **read by `app` (from the keychain), never inside `ingestion`** — `ingestion` stays offline-testable and key-agnostic (FR63, architecture invariant line 728).
5. **AC5 — Replace the interim env-var key source.** The Story-3.1 interim `STEADYINVEST_EODHD_API_KEY` env-var read in `main.rs::on_fetch_provider` is replaced by a **keychain lookup keyed by the preferred provider**. The env var is **retained only as a documented fallback** for environments with no running secret agent (headless/NAS), and that fallback is logged at most as a neutral one-line note (never the key value).
6. **AC6 — Graceful keychain absence.** Given no running D-Bus secret agent (gnome-keyring/KWallet), when Guy adds/reads a key, then the failure is **cause-named and neutral** (the secret store is unavailable), last-known behaviour is preserved, and the env-var fallback (AC5) still allows a fetch — the app never panics and never silently drops the key.

## Tasks / Subtasks

- [x] **Task 1 — Pin & wire the `keyring` dependency (AC1, AC6)**
  - [x] Root `Cargo.toml [workspace.dependencies]`: pinned `keyring = { version = "3", default-features = false, features = ["async-secret-service", "crypto-rust", "async-io"] }`. **DEVIATION (with rationale):** the story specified `sync-secret-service`, but that backend pulls `dbus-secret-service` → `libdbus-sys`, a **C system lib** needing `libdbus-1-dev` + pkg-config — against this project's no-C-deps / no-cmake posture (proven: the build failed on missing `dbus-1.pc`). `async-secret-service` gives the **same persistent secret-service backend** in pure Rust (`zbus`) and keeps `Entry` synchronous (block_on internally). The runtime feature is **`async-io`, NOT `tokio`** (changed during code review, finding F1): the keyring docs warn that the `tokio` runtime deadlocks on main/UI-thread calls (issue #132) and all our keychain calls run on the UI thread. `crypto-rust` keeps the encrypted session pure-Rust. Persistence-across-reboot (Guy's decision) is preserved.
  - [x] `app/Cargo.toml`: `keyring = { workspace = true }` (+ promoted `thiserror = { workspace = true }` for the keychain error enum, ADD15 — already in the lock tree, zero lock growth).
  - [x] `cargo deny check` passes **with NO `deny.toml` change needed**: `zbus`/`zvariant`/`secret-service`'s licenses were all already covered (zbus was already in the tree via the accesskit a11y stack, so secret-service v4 reuses it). The anticipated license-allowance subtask turned out unnecessary.
  - [x] `Cargo.lock` grew (keyring 3.6.3 + secret-service 4.0 + transitive) — recorded in File List.

- [x] **Task 2 — `app/src/keychain.rs`: secret-store access (AC1, AC4, AC6)**
  - [x] New module `keychain.rs`; `mod keychain;` wired in `main.rs`.
  - [x] API: `set_key` / `get_key` (`Ok(None)` = absent, distinct from `Err` = unavailable) / `delete_key` (idempotent on absent) / `has_key`.
  - [x] Per-provider slot: `keyring::Entry::new("steadyinvest", &format!("provider:{}", provider.wire()))`.
  - [x] `KeychainError` (`thiserror`, ADD15) cause-named: `Unavailable` / `Backend`. **Refined to UNIT variants** (no `detail` field) — a stronger NFR-S1 guarantee: no field can ever carry the key. `keyring::Error::NoEntry` → `Ok(None)`. The key is never logged; only the key-free `keyring` error + provider name go to `tracing`.
  - [x] NFR-S1 guard test: `error_display_is_neutral_and_carries_no_secret`. Live-store paths deferred to Task 8 (need a D-Bus agent absent in CI); the key-free logic is what's proven headless.

- [x] **Task 3 — `ProviderChoice` + `AppConfig.preferred_provider` (AC3, AC4)**
  - [x] New `app/src/provider.rs`: `ProviderChoice { Eodhd, None }` with `parse`/`wire`/`requires_key`, kebab-case serde, `#[default] Eodhd` — mirrors `Theme`/`Regime`. (Put in its own module, not `keychain.rs`, to avoid a config→keychain dep.) Full unit tests.
  - [x] `#[serde(default)] pub preferred_provider: ProviderChoice` added to `AppConfig`, defaulting to `Eodhd`. Append-only tests added: round-trip + `old_config_without_preferred_provider_loads_and_defaults_the_field` + `config_never_serializes_an_api_key_field` (NFR-S1 structural guard).
  - [x] No key/secret field in `AppConfig` — confirmed by the serialize-guard test.

- [x] **Task 4 — Settings UI: provider/key panel (AC1, AC2, AC3)**
  - [x] New `SettingsPanel` `@tr("Fournisseur de données")` at the top of `settings.slint`.
  - [x] Provider `ChoiceChip`s: EODHD + "Aucun (sans clé)"; one chip per future provider.
  - [x] `TextField` key input, write-only; status is `@tr("Clé configurée")` / `@tr("Aucune clé configurée")` — the secret value is never seeded back. Buttons Enregistrer / Supprimer / Tester. The field is cleared after Enregistrer.
  - [x] `Prefs` extended: `provider`, `key-configured`, `provider-status` + callbacks `provider-selected`, `key-saved(string)`, `key-deleted()`, `key-tested()`. The key travels one-way as the `key-saved` argument — never an `in-out`/persisted property.
  - [x] FR13: all new prose `@tr()`; the key is user data, never scanned.

- [x] **Task 5 — Wire callbacks in `main.rs` (AC1, AC2, AC4, AC5, AC6)**
  - [x] `on_provider_selected`: updates `preferred_provider`, persists, re-mirrors via `mirror_provider_prefs`.
  - [x] `on_key_saved`: `keychain::set_key` → `MSG_KEY_SAVED` / `MSG_KEYCHAIN_UNAVAILABLE`; blank trimmed → no-op.
  - [x] `on_key_deleted`: `keychain::delete_key` → `MSG_KEY_DELETED` / `MSG_KEYCHAIN_UNAVAILABLE`.
  - [x] `on_key_tested`: keyless → `MSG_KEY_OK`; no key → `MSG_PROVIDER_NO_KEY`; else enqueue a `WorkerJob::TestKey` + `MSG_KEY_TESTING`. Outcome handler maps `Ok` → `MSG_KEY_OK`, `InvalidOrAbsentKey` → `MSG_KEY_INVALID`, else `MSG_PROVIDER_FAILED`.
  - [x] **AC5:** the env read in `on_fetch_provider` is replaced by `resolve_provider_key(preferred_provider)` — keychain first, env-var (`STEADYINVEST_EODHD_API_KEY`) fallback when the store is empty *or* unavailable (AC6), with a key-free `tracing::info!` note. Keyless → `None`; key-requiring with no key → `MSG_PROVIDER_NO_KEY`.

- [x] **Task 6 — Minimal validation fetch on the worker (AC2)**
  - [x] `fetch.rs` refactored to `WorkerJob { Fetch, TestKey }` + `WorkerOutcome { Fetch, TestKey }` on the **same** worker thread/runtime (no second runtime). `TestKeyRequest { api_key }`.
  - [x] The test runs `fetch_canonical(provider, "AAPL.US", key)` and discards the data (`.map(|_| ())`) — only the `Result`/`ProviderError` verdict matters; zero new `ingestion` surface.
  - [x] The `Send` `Result<(), IngestionError>` marshals back via `invoke_from_event_loop` and is classified into the neutral `MSG_KEY_*` notices.

- [x] **Task 7 — Messages, posture floors & gates (AC1–AC6, FR13)**
  - [x] Six `MSG_*` consts added + registered in `USER_FACING_MESSAGES`: saved / deleted / testing / ok / invalid / keychain-unavailable. Fact-stating, banned-verb-clean (posture gate green).
  - [x] Posture floors bumped to the exact new totals: message inventory `27 → 33`; `@tr` literal floor `212 → 223` (the measured extractor count after the new panel).
  - [x] All gates green `--locked`: `cargo fmt --all --check` ✓, `cargo clippy -- -D warnings` ✓, `cargo test --workspace` ✓ (app 157, full suite green), `cargo deny check` ✓. Method fingerprint / determinism / golden / corpus all green (no calc change — `core`/`contract`/`persistence` untouched).

- [ ] **Task 8 — Manual GO/NO-GO + visual verification (AC1, AC2, AC6) — Guy on display** *(PENDING — needs Guy's desktop + a running D-Bus secret agent; headless CI/sandbox cannot run it, same caveat as Story 3.1's GO/NO-GO)*
  - [ ] On Guy's Linux desktop: add a real EODHD key in Réglages → confirm it lands in the keychain (`secret-tool search service steadyinvest`, value not printed) and **never** appears in `config.json`/logs. Click **Tester** → success. Delete → entry gone. Fetch a real ticker → cells fill (the 3.1 path, now keychain-fed).
  - [ ] Negative: with no secret agent → adding a key reports the neutral "store unavailable" cause; the env-var fallback still allows a fetch (AC6).
  - [ ] Headless launch confirmed clean (exit 124, no panic) — the keychain read on the UI thread at startup degrades gracefully when no agent is present.

## Dev Notes

### Scope decision (Guy, 2026-06-25)

Three forks resolved before authoring (mirrors the 3.1 scope-resolution pattern):

1. **keyring backend = `sync-secret-service`** (persistent across reboot). Rationale: an API key is "set once"; `linux-native`/keyutils is session-scoped and would force re-entry every login. Cost: needs a running D-Bus secret agent → **graceful failure + env-var fallback** is a hard requirement (AC6), not optional.
2. **"Test" = a real minimal HTTP call** through the 3.1 worker path (off-UI-thread, cause-named). A local-only "is it non-empty" check was rejected — it can't prove the key is valid, which is the whole point of the button.
3. **UI = EODHD only, structured multi-provider.** One provider/key panel with a provider selector (EODHD + keyless) and add/replace/delete/test, built so a second adapter is one more chip — but no speculative multi-provider machinery beyond that.

### What this story changes vs. preserves (files marked UPDATE — read before editing)

- **`app/src/main.rs` (UPDATE)** — `on_fetch_provider` (lines ~424–469) currently reads the key from `STEADYINVEST_EODHD_API_KEY` (lines 442–449). This story replaces that read with a keychain lookup keyed by `AppConfig.preferred_provider`, keeping the env var as a documented fallback (AC5/AC6). **Preserve:** the off-thread `fetch_tx.send(...)` flow, the `set_fetching`/notice handling, and the worker-gone P1 guard (lines 459–468) — all unchanged. Add the new `Prefs` provider/key callbacks alongside the existing `on_theme_selected`/`on_label_set_selected`/`on_number_format_selected` block (lines ~1483–1520) using the **same apply→persist→mirror** pattern.
- **`app/src/config.rs` (UPDATE)** — `AppConfig` is the append-only app-config struct. Add `preferred_provider` exactly like `journal_path`/`study_view_state` were added: `#[serde(default)]`, a `Default`, a round-trip test, and an "old config loads & defaults" test. **Preserve:** the load-fallback/corrupt-file-aside behaviour and the strict "app-config never holds secrets" boundary (no key field — NFR-S1).
- **`app/src/fetch.rs` (UPDATE)** — single worker thread + `current_thread` tokio runtime + `thread_local` outcome handler (the 3.1 bridge for non-`Send` `Rc`). Add the key-test job to this **same** worker (an enum job or sibling request) — do NOT add a second runtime or thread. **Preserve:** the `Send`-only outcome contract and the reused `EodhdProvider`/`reqwest::Client` (review P2 from 3.1).
- **`app/ui/screens/settings.slint` (UPDATE)** — no-wizard panel stack. Add the provider/key panel as a new `SettingsPanel` using the existing `ChoiceChip`/`ActionButton`/`text_field` components and the panel pattern. **Preserve:** the existing theme/vocabulary/number-format/legend/glossary/verify panels.
- **`app/ui/state.slint` (UPDATE)** — extend the `Prefs` global (lines 479–490) with provider/key properties + callbacks. **Preserve:** the existing `in-out` mirror properties and the "intents handled in Rust" convention. **Critical:** the secret key must never be an `in-out` property that round-trips to config — pass it one-way through the `key-saved(string)` callback argument only.
- **`app/src/posture.rs` (UPDATE)** — bump the `@tr` and `USER_FACING_MESSAGES` floors to the exact new counts.
- **`Cargo.toml` / `app/Cargo.toml` / `deny.toml` (UPDATE)** — pin & enable `keyring`; allow its new transitive licenses (minimum set, each with a reason).

### NEW files

- **`app/src/keychain.rs`** — the only module that touches `keyring`. Cause-named `KeychainError`, per-provider entries, key never logged/Displayed.

### Architecture & constraints

- **The key-injection invariant (architecture line 728, FR63):** keys are read by `app` (from `keyring`) and **passed into** `ingestion`; `ingestion` never reads a key itself — it stays offline-testable. Story 3.1 already honours this (`fetch_fundamentals(ticker, api_key: Option<&str>)`); 3.2 only changes *where* `app` gets the key (env → keychain).
- **App-config vs journal vs secrets boundary (ADD7, architecture lines 166–167, 292):** `directories` for app-config (provider *choice*), `keyring` for secrets (the key), journal SQLite for studies. The key crosses none of the other two stores.
- **keyring crate choice (architecture lines 217–227, root Cargo.toml lines 50–55, issue #5):** `keyring = "3"` (the real library), **NOT** `keyring` 4.0.x (the sample/CLI meta-crate). `default-features = false` + explicit `sync-secret-service`. Linux secret-service needs a D-Bus/secret agent — hence AC6's graceful-absence path. This is the deferred half of the #5/B5 revalidation (the reqwest TLS half landed in 3.1, `335978f`/`3ae230b`).
- **No calculation change:** `core`/`contract`/`persistence` are untouched. Method fingerprint, determinism hash, golden gate, frozen `v1.db` corpus must re-diff clean. This is an `app`-crate story plus the workspace dep/lock/deny growth that the keychain mandates.
- **NFR-S1 (PRD lines 846–847):** keys live **only** in the OS secret store — never repo, plaintext config, logs, exports, backups. Enforce by construction (no key field in `AppConfig`, no key in any `tracing` call, no key in any export path) and by a headless guard test.
- **NFR-S2/S3:** the only network calls are user-initiated; the key-test fetch is user-initiated (the Tester button), consistent with FR65 offline-first.

### Testing standards

- Headless Rust unit/integration tests are the norm (this is a Slint-native, no-web app — the QA e2e/Playwright step is N/A, as every Epic 2/3 story recorded).
- **keyring in CI:** the secret-service backend needs a live agent absent in headless CI. Gate the real-store tests behind a feature or `#[ignore]`, and unit-test the *logic* (provider→entry naming, `NoEntry`→`Ok(None)` mapping, `KeychainError` Display never contains the key) without hitting the store. Consider `keyring`'s `mock` backend (feature `mock`) for deterministic store behaviour in tests if it keeps the tree lean under `cargo deny`.
- **AppConfig** append-only tests: round-trip `preferred_provider`, and an old-config-without-the-field test (copy the `journal_path`/`study_view_state` test shape in config.rs).
- All four gates `--locked`; pinned rustfmt 1.9.0 (`cargo fmt --all --check` must stay green — issue #36 realignment is on main).
- UI story → on-display visual verification is part of DoD (Task 8), per the Epic-2 convention (B8).

### Open questions for dev (resolve during implementation, don't block)

- Does `keyring` 3.6.x `sync-secret-service` pull a license not yet in `deny.toml`? Resolve the exact allow-list against the real resolved tree (Task 1) — don't pre-guess.
- Default `preferred_provider`: `Eodhd` vs a keyless default. Leaning `Eodhd` (only real adapter); confirm it doesn't make a fresh install nag for a key before the user opens Settings (it shouldn't — `MSG_PROVIDER_NO_KEY` only fires on an explicit fetch click).
- `mock` feature for tests vs `#[ignore]` real-store tests — pick whichever keeps `cargo deny` lean.

### Project Structure Notes

- Matches the architecture source tree: `app/src/keychain.rs` (line 689), `app/ui/screens/settings.slint` (line 710), `app/config.rs` (line 688). No new crate; `ingestion` is untouched (the whole point of the injection invariant).
- No schema/DDL change, no `SCHEMA_VERSION` bump, no `method_version` change.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.2 (lines 763–775)] — AC source.
- [Source: _bmad-output/planning-artifacts/epics.md#Epic 3 (lines 311–318, 740–746)] — epic framing, FR coverage.
- [Source: _bmad-output/planning-artifacts/architecture.md#keyring 3.x (lines 217–227)] — crate choice, backend trade-off, D-Bus caveat.
- [Source: _bmad-output/planning-artifacts/architecture.md (lines 166–167, 282–292, 326, 377–378, 688–689, 728, 745)] — config/secret boundary, source tree, key-injection invariant.
- [Source: _bmad-output/planning-artifacts/prd.md (lines 708–709 FR25, 846–847 NFR-S1, 565–566 keyless)] — requirements.
- [Source: Cargo.toml (lines 50–56)] — the deferred keyring pin note (#5/B5) this story executes.
- [Source: app/src/main.rs (lines 424–469)] — `on_fetch_provider`, the env-var read to replace.
- [Source: app/src/config.rs] — `AppConfig` append-only rail + tests to mirror.
- [Source: app/src/fetch.rs] — the off-thread worker to reuse for the key test.
- [Source: app/ui/screens/settings.slint, app/ui/state.slint (Prefs, lines 479–490)] — Settings panel + Prefs global to extend.
- [Source: Story 3.1 — 3-1-marketdataprovider-trait-eodhd-adapter.md + commits 3ae230b, 82190f7] — the fetch path, `ProviderError` taxonomy, GO/NO-GO harness this story builds on.
- [Source: memory project-planning-progress — CHECKPOINT 2026-06-25] — 3.1 GO/NO-GO passed; 3.2 = move the env key into the keychain.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- `cargo build -p steadyinvest-app` initially failed: `sync-secret-service` → `libdbus-sys` needs C `libdbus-1-dev` + pkg-config. Switched to `async-secret-service` + `tokio` (pure-Rust `zbus`) → builds clean.
- `cargo deny check` → all ok with no `deny.toml` change (zbus already in tree via accesskit).
- Posture floors: message inventory 27→33; `@tr` literal floor 212→223 (extractor reported 223).
- `timeout 8 cargo run -p steadyinvest-app` → exit 124 (healthy event loop; keychain read on UI thread degrades gracefully with no agent).

### Completion Notes List

- **Tasks 1–7 complete; Task 8 (manual GO/NO-GO on Guy's display) PENDING** — needs a desktop + running D-Bus secret agent, like Story 3.1's GO/NO-GO. Headless launch confirmed clean.
- **Backend deviation:** `async-secret-service` (pure-Rust `zbus`) instead of the story's `sync-secret-service` (which needs the `libdbus-1-dev` C lib) — same persistent secret-service store, honours Guy's persistence-across-reboot decision, keeps the no-C-deps posture. `Entry` stays synchronous (block_on via the `tokio` feature already in the tree).
- **NFR-S1 by construction:** the key lives only in the OS secret store; `AppConfig` has no key field (guarded by `config_never_serializes_an_api_key_field`); `KeychainError` is a unit enum so no key can ride in an error; the key is never logged (only the key-free `keyring` error). The UI mirrors a `key-configured` bool, never the value.
- **No calc change:** `core`/`contract`/`persistence`/`deny.toml` untouched; method fingerprint / determinism / golden / corpus gates all green.
- App tests 148→157 (+9: provider 5, keychain 1, config 3). All gates green `--locked`.

### File List

**New**
- `app/src/provider.rs` — `ProviderChoice` enum (config + keychain slot)
- `app/src/keychain.rs` — OS secret-store access (the only `keyring` consumer)

**Modified**
- `Cargo.toml` — pin `keyring` 3.x `async-secret-service`/`crypto-rust`/`tokio`
- `Cargo.lock` — keyring + secret-service + transitive (in scope)
- `app/Cargo.toml` — `keyring`, `thiserror` deps
- `app/src/main.rs` — `mod keychain/provider`; `resolve_provider_key` + `mirror_provider_prefs`; provider/key callbacks; keychain-fed `on_fetch_provider` (env fallback); `WorkerOutcome` match (+ TestKey arm)
- `app/src/config.rs` — `AppConfig.preferred_provider` + append-only/NFR-S1 tests
- `app/src/fetch.rs` — `WorkerJob`/`WorkerOutcome` enums + `TestKeyRequest` key-test job
- `app/src/state.rs` — six `MSG_KEY_*`/`MSG_KEYCHAIN_*` consts + inventory registration
- `app/src/posture.rs` — message-inventory (33) and `@tr` (223) floors
- `app/ui/screens/settings.slint` — provider/key `SettingsPanel`
- `app/ui/state.slint` — `Prefs` provider/key properties + callbacks

### Change Log

- 2026-06-25 — Story 3.2 implemented (provider config + API keys in the OS keychain). keyring 3.x async-secret-service (pure-Rust), per-provider slots, write-only key entry, env-var fallback, off-thread key test. Status → review. Tasks 1–7 done; Task 8 (manual GO/NO-GO) pending Guy's display.
- 2026-06-25 — 3-layer adversarial code review; **9 patch findings applied**, 8 deferred (→ GitHub Issues), 3 dismissed. Key fixes: **F1** keyring runtime `tokio`→`async-io` (the tokio backend deadlocks on UI-thread keyring calls per the crate docs / issue #132); **BH5** `reqwest::Error::without_url()` in `eodhd.rs` so the api_token can't leak into a user notice (NFR-S1); **F5** `set_fetching(false)` only on the Fetch outcome arm; **F3** clear `provider_status` on provider switch; **F9** trim the env key; **F13** hide key-status for keyless; **BH1/BH2** doc + test hardening; mock-backed keychain test added. All gates green: app 158 tests, full workspace suite green, clippy/fmt/deny clean.

## Review Findings (3-layer adversarial code review, 2026-06-25)

Layers: Blind Hunter + Edge Case Hunter + Acceptance Auditor (all 3 completed). Acceptance Auditor: AC1–AC6 all PASS on intent; the findings below are defects/edge-cases found by the hunters. 9 patch · 8 defer · 3 dismissed.

### Patches

- [x] [Review][Patch] **F1 (CRITICAL): keyring `async-secret-service`+`tokio` deadlocks when called on the UI thread** — the keyring docs (secret_service.rs "Tokio runtime caution", issue #132) state main-thread calls "will likely deadlock". All `keychain::*` calls run on the UI thread. Fix: switch the keyring runtime feature `tokio` → `async-io` (the caution is tokio-specific). [Cargo.toml]
- [x] [Review][Patch] **BH5 (HIGH, NFR-S1 leak): the EODHD api_token can leak into a user-facing notice** — `ProviderError::Network { detail: e.to_string() }` includes the reqwest URL, which carries `?api_token=…`; it surfaces via `MSG_PROVIDER_FAILED`. Fix: `e.without_url().to_string()` at the two reqwest error sites. [ingestion/src/adapters/eodhd.rs:54,61]
- [x] [Review][Patch] **F5 (HIGH): the shared `fetching` flag is cross-reset by a TestKey outcome** — the outcome handler calls `set_fetching(false)` for BOTH arms; a TestKey completing while a study Fetch is in flight re-enables the Fetch button → possible concurrent fetch. Fix: reset `fetching` only in the `Fetch` arm. [app/src/main.rs outcome handler]
- [x] [Review][Patch] **F3 (MEDIUM): `provider-status` lingers across a provider switch** — switching the provider chip leaves the previous key-test/save verdict visible, now misattributed. Fix: clear `provider_status` in `on_provider_selected`. [app/src/main.rs]
- [x] [Review][Patch] **F9 (LOW): env-var key is not trimmed (inconsistent with keychain path)** — `resolve_provider_key` stores `key.trim()` in the keychain but passes the raw env value; a padded env key fails. Fix: trim the env value. [app/src/main.rs:resolve_provider_key]
- [x] [Review][Patch] **F13 (LOW): "Aucune clé configurée" shows for the keyless "none" provider** — the status Text is outside the `if provider == "eodhd"` block. Fix: move it inside. [app/ui/screens/settings.slint]
- [x] [Review][Patch] **BH1 (LOW): stale `app/Cargo.toml` comment says "sync-secret-service"** — the workspace pins `async-secret-service`. Fix the comment. [app/Cargo.toml]
- [x] [Review][Patch] **BH2 (LOW): config NFR-S1 test has an over-broad `!json.contains("key")` assertion** — will false-positive on any future field name containing "key". Fix: drop the over-broad check, keep `api_key`/`secret`. [app/src/config.rs]
- [x] [Review][Patch] **Test gap (MEDIUM): keychain store logic is untested** — only the error-Display test ships; the spec recommended the keyring `mock` backend. Add mock-backed tests (set/get/delete roundtrip, NoEntry→Ok(None), idempotent delete). [app/src/keychain.rs]

### Deferred (→ GitHub Issues, per the project's issue-tracking convention)

- [x] [Review][Defer] **F12: switching to "none" orphans the stored EODHD key with no in-app delete path** — secret-hygiene UX gap; the key persists after the provider is disabled. (→ #38)
- [x] [Review][Defer] **F4: no reqwest request timeout → a hung connection latches the "testing"/"fetching" state** — pre-existing from Story 3.1's fetch; a client timeout belongs there. (→ #39)
- [x] [Review][Defer] **F6: provider-panel Save/Delete/Test buttons are not disabled during an in-flight worker job** — allows stacking duplicate jobs. (→ #40)
- [x] [Review][Defer] **F8: a blank "Enregistrer" silently clears the field with no feedback** — looks like a successful save. (→ #41)
- [x] [Review][Defer] **F14: a quota/network error during the key test reads as a failed key** — a `Quota` reply actually proves the key is accepted; distinguish "accepted but quota/network" from "rejected". (→ #42)
- [x] [Review][Defer] **F15/F16: env fallback shadows a deliberately-emptied keychain; a keyless provider ignores a present stored key** — fallback-precedence semantics need an intentional, surfaced policy. (→ #43)
- [x] [Review][Defer] **F11: `classify` flattens `Ambiguous`/`Invalid`/`TooLong` to a generic error** — a duplicate-credential store needs a remediation hint; also the locked-store false-negative on `has_key` (F10). (→ #44)
- [x] [Review][Defer] **BH4: the plaintext key is not zeroized in memory** — defense-in-depth hardening (zeroize on drop across the String copies). (→ #45)

### Dismissed (noise / consistent with existing patterns)

- F2 (startup keychain read latency) — the real issue was the deadlock (fixed by F1); a local secret-store read at startup is negligible, consistent with reading config.
- F7 (a dead worker degrades silently for the session) — acceptable degradation; "should never happen", consistent with Story 3.1.
- Auditor's "worker-gone inline French string" — mirrors the existing 3.1 worker-gone string (Rust-side, not prose-scanned); consistent with the established pattern.

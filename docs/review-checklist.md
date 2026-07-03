# steadyinvest — review & dev checklist (accumulated doctrines)

Loaded automatically by the dev-story and code-review skills (via `_bmad/custom/*.toml`).
These are the project's accumulated defect classes: every one earned its place by shipping (or
nearly shipping) a real bug. Cite the rule by name when applying or reviewing.

## 1. Absence honesty (Epics 3–6)

- **Absent, never wrong; absent, never zero.** A figure that cannot be stated (missing FX pair,
  checked overflow, failed read) is ABSENT — never `0`, never a partial sum passed off as a total.
- **Every refusal names itself.** A missing pair is named (« taux manquant EUR → CHF »); a
  non-nameable absence renders plainly « indisponible » — never a dangling empty amount.
- **An IO failure is « indisponible », never an empty-looking zero state.** A failed `list_*`
  read must not render as "nothing exists" (6.7 P7, 6.8 P7). Watch every `Option`-swallowing read.
- **An absent fact never flags/murmurs.** Thresholds fire on PRESENT (and positive) values only.
- **Misattribution is a lie.** A named reason must be the RIGHT reason (6.7: overflow ≠ « chiffre
  d'affaires indisponible »; classification pairs ≠ denominator pairs).

## 2. The discriminator rule (Epic-6 retro F3 — NEW)

Any state derived from **position, index, ordering, or string-emptiness** must justify why it is
not keyed by **identity**. Two shipped instances of this family:
- 6.8: the « Vente enregistrée » header keyed off a non-empty context string — a trigger-open
  asserted a sale that never happened.
- 6.9 (CRITICAL): the fallback event keyed off chain POSITION — a keyless primary dropped at
  enqueue let the fallback serve at index 0 in silence. Fixed by comparing provider IDENTITY
  against the configured primary.
Ask at review: "if the list were filtered/reordered upstream, would this still mean what it says?"

## 3. Named UI conventions live AT the code site

A convention that lives only in an old review note gets re-broken (the F4 notice-slot rule
regressed in 6.9 after being fixed in 4.4 and honored in 6.8). When a review establishes a rule,
write a comment naming it at every site that must honor it.
- **The notice-slot rule (F4):** a success/info notice only replaces the in-progress banner or an
  empty slot — never a sibling's failure notice.

## 4. Locale & display consistency (6.7 P4)

Every number rendered beside a locale-formatted number goes through `format_scaled` — never a raw
config string in the same sentence as a formatted one. Display strings are built in Rust via the
locale path; French prose is never baked into Rust data (the posture scan cannot see it) — every
visible string is a `@tr()` literal or a registered `MSG_*`.

## 5. Read-first, and read the failure surface (Epic-4/5 lessons)

Before touching a file: read it. Before any filesystem/path operation: enumerate self-aliasing,
atomicity (temp+rename), TOCTOU, and sync-folder (`-wal`) effects. Before any schema-adjacent
change: remember the 5.3 `deny_unknown_fields` cliff — even an "additive" contract field is an
export/import compatibility break (the 6.9 no-contract-change decision is the precedent: prefer
reformatting an opaque String's VALUE over adding a field).

## 6. Exact-count posture floors

`@tr` floor and `USER_FACING_MESSAGES` count are exact-count disciplines: probe (set the floor
absurdly high, read the scanned total from the failure message), set the exact number, document
the delta in the running tally comment in `posture.rs`.

## 7. Re-render completeness (6.6/6.7/6.8)

Every mutation path that changes a surface re-renders it — enumerate the paths (both FX mutation
paths, settings commits, journal switch/open/restore clears, navigation arrivals since #94's
fix). A pushed panel re-syncs while open and clears on journal switch.

## 8. Pure decisions, dumb loops (6.9)

Timing/retry/pacing decisions are pure functions consumed by the loop that sleeps — no sleep-based
tests, ever. `Instant` for infra timing; the injected ADD15 clock for journal facts only.

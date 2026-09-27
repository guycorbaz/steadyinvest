# Frozen journal corpus — append-only, forever

These SQLite files are **real journals written by old builds**, committed as fixtures. They are
the CI gate (NFR-R3) that proves every future build still opens and exactly reads what past
builds persisted.

| File | Written by | Contents |
|------|------------|----------|
| `v1.db` | Story 1.10 (`user_version` 1, `SCHEMA_VERSION` 1) | the canonical study of `tests/corpus_gate.rs` |
| `v8.db` | Story 8.2a (`user_version` 8, `SCHEMA_VERSION` 1) | the canonical study + one AI draft of every kind and status (`canonical_drafts`) |

## The rules

1. **Never edit a committed corpus file. Never regenerate one.** A corpus file simulates a user's
   existing journal on disk — rewriting it with current code destroys the only evidence that old
   files still open. (`generate_corpus_v1` refuses to run when `v1.db` exists, on purpose.)
2. **A schema change adds the next file beside the old ones.** When `SCHEMA_VERSION` and/or
   `PRAGMA user_version` bump to N: write the migration step, update the pinned snapshot in
   `corpus_gate.rs`, add an `#[ignore]`d `generate_corpus_vN` one-shot generator, run it once,
   commit `vN.db`, and extend the gate so **every** `v*.db` (old AND new) opens and reads back
   exactly under the new build.
3. **Tests never open a corpus file in place.** Copy it to a `TempDir` first — that keeps the
   frozen fixture byte-identical and the `-wal`/`-shm` sidecars out of the repo tree.
4. **`.gitignore` has `*.db` with a `!persistence/tests/corpus/*.db` exception** (after the rule —
   last match wins). If `git status` does not show a new corpus file, fix the ignore rules before
   anything else: an untracked corpus passes locally and silently never reaches CI.

**Gap v2–v7, as found (Story 8.2a, 2026-09-27).** The practice lapsed between v1 and v8: the
migrations v2–v7 shipped without their corpus file. They are **not** back-filled — a file written
today by current code would not be evidence of what those builds wrote, and the app is not in
production (owner, 2026-09-27). `v8.db` restarts the practice; every later step adds its file.

## How `v1.db` was generated (for the record — do not repeat)

```
cargo test -p steadyinvest-persistence --test corpus_gate -- --ignored
git add persistence/tests/corpus/v1.db
```

Built in a `TempDir` from fixed identity/time inputs (`11111111-…`, `2026-06-12T00:00:00Z`),
closed cleanly (WAL checkpointed), then copied here as a plain closed file.

`v8.db` the same way (`-- --ignored generate_corpus_v8`): the canonical study through the API,
then the five drafts by raw SQL (no draft writer exists before Story 8.3), closed, copied.
`v8.db` is a **WAL** journal (the `journal_mode` persists in the file): copy it before inspecting
it with `sqlite3`, or the tool leaves `-wal`/`-shm` sidecars in the repo tree (rule 3).

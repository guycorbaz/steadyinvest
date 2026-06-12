# Frozen journal corpus — append-only, forever

These SQLite files are **real journals written by old builds**, committed as fixtures. They are
the CI gate (NFR-R3) that proves every future build still opens and exactly reads what past
builds persisted.

| File | Written by | Contents |
|------|------------|----------|
| `v1.db` | Story 1.10 (`user_version` 1, `SCHEMA_VERSION` 1) | the canonical study of `tests/corpus_gate.rs` |

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

## How `v1.db` was generated (for the record — do not repeat)

```
cargo test -p steadyinvest-persistence --test corpus_gate -- --ignored
git add persistence/tests/corpus/v1.db
```

Built in a `TempDir` from fixed identity/time inputs (`11111111-…`, `2026-06-12T00:00:00Z`),
closed cleanly (WAL checkpointed), then copied here as a plain closed file.

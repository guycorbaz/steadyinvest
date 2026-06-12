# Bundled golden reference studies (runtime assets)

These `*.json` files are the golden reference studies the Story-2.13 "verify engine" screen
replays at runtime via `core::golden::check` (FR9, ADD12).

**Single source of truth: `core/tests/golden/`.** This directory is a plain byte-for-byte
copy (symlinks are forbidden — they break on Windows). To update, edit the source fixtures
and re-copy:

```sh
cp core/tests/golden/g*.json app/assets/golden/
```

The drift test in `core/tests/golden_gate.rs` fails CI whenever the two sets differ, in
either file list or content. Fixture schema, provenance rules and authoring guidance live
in `core/tests/golden/README.md` (the two READMEs intentionally differ and are excluded
from the drift comparison).

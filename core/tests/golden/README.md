# Golden reference studies (Story 1.9, FR9 / NFR-C1/C2)

This directory is the **single source of truth** for the bundled golden reference studies.
Every `*.json` file here is one `GoldenStudy` fixture, replayed by the CI gate
(`core/tests/golden_gate.rs`) through the real pipeline `normalize` → `ssg::compute` via
`core::golden::check`. The same files are copied byte-identically to `app/assets/golden/`
for the Story 2.13 "verify engine" screen (a drift test keeps the two sets equal).

All fixtures are **synthetic**: no vendor data, no NAIC-published content.

## The one rule that matters: independent provenance

**Never paste engine output into `expected`.** A golden whose expected values came from the
engine is circular: it passes forever and can never catch a method deviation. Every expected
value must come from an independent derivation — paper or spreadsheet, at full precision —
and `meta.provenance` must record *how* it was computed **and how it was cross-checked**
(a second derivation, a back-substitution, an exact-power construction…).

If the gate fails on a fixture, re-derive the expected value by hand. Either the fixture is
wrong (fix the fixture, record the corrected derivation in `provenance`) or the engine has a
real bug (file it) — never "adjust" engine code or fixture numbers until they agree.

## Fixture schema (`fixture_format_version: 1`)

Top level: `{ "meta": …, "input": …, "expected": … }`. The schema is defined by the serde
types in `core/src/golden/schema.rs`; the JSON layer follows the project format patterns:
snake_case keys, **decimals as canonical strings** (parsed with `Decimal::from_str_exact`;
`"1e5"`, `"+1"`, `"-0"` etc. are rejected), JSON `null` = unknown/absent — never 0.

- `meta` — `id` (matches the file name, kebab-case), `title`, `description`, `provenance`
  (free text, multi-line allowed), `method_version` (must equal `core::METHOD_VERSION`;
  a stale value makes the check FAIL — re-validate the golden at a method bump, never
  replay it silently), `fixture_format_version` (must be `1`).
- `input` — `native_currency`, `years` (all 12 `RawYear` fields, each explicitly present),
  `splits`, `judgment` (the engine's `JudgmentInputs` shape; `forecast_low_option` is one of
  `avg_low_pe_times_eps` / `avg_low_price_last5y` / `recent_severe_low` /
  `dividend_supported`), `quarterly` (the `QuarterlyObservations` shape).
- `expected` — the full SSG output surface, sections `growth` / `management` / `valuation` /
  `risk_reward` / `returns`, plus `quality_flags` (pinned catalog strings, **in raise
  order**), `findings` (engine pass order; `year: null` = study-level), `low_confidence`,
  `verdict_facts` (criteria are `met` / `unmet` / `unmet_by_insufficiency`), and optionally
  `normalize_findings`.

### Strictness (inverted from the journal rule)

Fixtures are oracles: every struct rejects unknown fields, and **required fields must be
written explicitly** — an *omitted* required field is a parse error, an explicit `null`
means "expected unknown". A typo can therefore never silently weaken a golden.

### Optional blocks (omitted = not asserted)

Only three blocks may be omitted: `management.per_year`, `valuation.per_year` (the per-year
tables) and `expected.normalize_findings`. Everything else is required.

### Comparison semantics (spec §7)

- **Categorical = exact**: zones, trends, the U/D state (`{"ratio": "…"}` / `"undefined"` /
  `"unknown"`), criteria, flags, findings, `low_confidence`, and every unknown — expected
  `null` ⇔ actual `None`, in both directions.
- **Derived numerics**: `|actual − expected| ≤ 0.005 × |expected|` (±0.5%, relative to the
  EXPECTED value — an expected of exactly `0` therefore demands exact equality). Write
  expected values at 6–10+ significant digits; the §4 zone thirds truncate at 28 digits,
  ~24 orders of magnitude inside the tolerance — do not chase 28-digit strings.

## Adding a golden

1. Construct the inputs so the interesting boundary is **exact by construction**
   (terminating decimals; ranges divisible by 3 where a zone edge must land cleanly;
   exact powers where a CAGR or an annualised rate must be a round number).
2. Hand-compute every expected value at full precision; cross-check each against a second
   derivation; record both in `meta.provenance`.
3. Name the file `gNN-<kebab-case-id>.json` with `meta.id` equal to the file stem.
4. Copy the file to `app/assets/golden/` (plain copy — symlinks are forbidden, they break
   on Windows). The drift test fails until the two directories match.
5. `cargo test -p steadyinvest-core --test golden_gate` must pass, and the suite fails if
   fewer than 10 fixtures are present.

Negative controls (intentionally wrong goldens) live as embedded strings inside
`golden_gate.rs` — **never** as files in this directory or in `app/assets/golden/`.

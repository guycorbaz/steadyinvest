//! Whole-journal portable export/import (Story 5.3, FR60).
//!
//! Scales the single-study envelope (Story 5.2, `contract::export`) up to the **entire journal**: a
//! versioned, integrity-checked JSON file carrying the journal identity tuple `(journal_id,
//! logical_version, hash)` plus **every journaled entity** — studies (with their lifecycle status),
//! watchlist items, **all portfolios** (Story 6.1 — each holding keeps its own `portfolio_id`),
//! holdings (**including soft-deleted/sold ones** — a sold holding stays a live FK referent for its
//! sell transaction) and sell transactions. **Not** a raw `.db` copy (architecture §"Export / backup
//! format").
//!
//! - [`Journal::export_journal`] reads the complete journal into a [`JournalSnapshot`], wraps it in an
//!   envelope, and hashes the canonical payload (reusing the **one** hashing implementation,
//!   `contract::export::sha256_hex`).
//! - [`Journal::import_journal`] parses + verifies the envelope (integrity, then `schema_version`),
//!   then applies **every entity in ONE transaction** — all-or-nothing, **never partially applied**
//!   (NFR-R5). Each entity is upserted by its own `id` (a re-import updates in place, no duplicates);
//!   a foreign `journal_id` is **rebound** to the current journal so a shared/seeded journal joins it.
//!   Import is a **merge/seed**, not a destructive replace — the current journal survives and absorbs
//!   the file. (Destructive restore-from-backup is Story 5.4.)
//!
//! Post-v1 additions ride the #78 additive rail (`#[serde(default)]` + `skip_serializing_if`):
//! `fx_rates` landed with Epic 6, the FR51 `judgments` time-series with issue #34 (PR 3). The
//! CURRENT judgment still travels inside each [`Study`] blob; the time-series carries the past.

use crate::error::{Error, Result};
use crate::fx::FxRateItem;
use crate::holdings::{HoldingItem, PortfolioItem};
use crate::journal::Journal;
use crate::transactions::TransactionItem;
use crate::util::bump_logical_version;
use crate::watchlist::WatchItem;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use steadyinvest_contract::{ImportError, SCHEMA_VERSION, Study, Timestamp, sha256_hex};
use uuid::Uuid;

/// One study plus its lifecycle `status` (the indexed column, not part of the [`Study`] blob — so it
/// must be carried explicitly to survive a round-trip: an archived study re-imports archived).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyRecord {
    pub status: String,
    pub study: Study,
}

/// The complete, serializable contents of a journal (Story 5.3). All collections are `Vec`s read in a
/// fixed, deterministic order (so the canonical serialization — and its hash — is stable across runs
/// and OSes); no `HashMap`/`BTreeMap` anywhere.
///
/// `deny_unknown_fields` makes a **newer-format** file that adds a future **entity ARRAY** (a new
/// top-level field here) a typed **rejection** rather than a silent partial import that drops the
/// unknown collection.
///
/// Issue #78 (decided 2026-07-08, product/architecture): this guarantee is **envelope-level only**.
/// The per-entity item types (`HoldingItem`/`PortfolioItem`/`WatchItem`/`TransactionItem`) do NOT
/// carry `deny_unknown_fields`, so a future build adding a **field** to one of them (e.g. a 6.2-style
/// `currency` on `HoldingItem`) is accepted by an OLDER build, which silently drops that field —
/// the same forward-compat rail the `Study` blob deliberately uses (unknown fields tolerated, only
/// unknown *shape* at the version axis rejected). Kept intentionally, not tightened to
/// per-entity `deny_unknown_fields`: every additive entity field would otherwise force an envelope
/// `schema_version` bump, which is disproportionate to a single new field. A field whose ABSENCE
/// would be unsafe (silently wrong, not just silently missing) needs its own guard at the point
/// that reads it — this rail does not promise universal safety, only "never a corrupt parse".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalSnapshot {
    pub schema_version: u32,
    pub journal_id: Uuid,
    pub logical_version: u64,
    pub studies: Vec<StudyRecord>,
    pub watch_items: Vec<WatchItem>,
    pub portfolios: Vec<PortfolioItem>,
    pub holdings: Vec<HoldingItem>,
    pub transactions: Vec<TransactionItem>,
    /// The dated, source-aware FX rates (Story 6.5, FR28). `#[serde(default)]` +
    /// `skip_serializing_if` is the deliberate #78 additive rail: an OLD file (no array) imports
    /// fine into this build; an EMPTY store exports WITHOUT the array (so a pre-6.5 build still
    /// reads a no-FX export); a file that DOES carry rates is a typed rejection on an old build
    /// (`deny_unknown_fields` above) — never a silent drop.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx_rates: Vec<FxRateItem>,
    /// The FR51 durable-history snapshots (issue #34, PR 3) — the same #78 additive rail as
    /// `fx_rates`: an OLD file (no array) imports fine; a history-less journal exports WITHOUT
    /// the array (a pre-#34 build still reads it); a file that DOES carry history is a typed
    /// rejection on an old build — never a silent drop of the time-series.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub judgment_snapshots: Vec<JudgmentSnapshotRecord>,
}

/// One FR51 snapshot row, exported **byte-faithfully** (issue #34, PR 3): `payload` is the RAW
/// stored study-state JSON string — never re-parsed/re-serialized on export, so an old-schema
/// historical state crosses exactly as the journal holds it (a time-series is an archive; its
/// `schema_version` column keeps describing its own payload truthfully).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JudgmentSnapshotRecord {
    pub id: Uuid,
    pub study_id: Uuid,
    pub created_at: Timestamp,
    pub schema_version: i64,
    pub payload: String,
}

/// The on-disk whole-journal export envelope. `payload` is the canonical serialized
/// [`JournalSnapshot`] JSON; `integrity_hash` is the lowercase-hex SHA-256 of `payload`'s UTF-8
/// bytes; the `(journal_id, logical_version)` identity tuple is surfaced at the envelope level so a
/// reader sees it without parsing the payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalExport {
    pub schema_version: u32,
    pub journal_id: String,
    pub logical_version: u64,
    pub integrity_hash: String,
    pub payload: String,
}

/// What an import applied (Story 5.3) — counts per entity plus the source journal identity, so the
/// app can surface "imported N studies, M watchlist rows…" and whether this was a same-journal update
/// or a foreign seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    pub source_journal_id: Uuid,
    pub source_logical_version: u64,
    pub studies: usize,
    pub watch_items: usize,
    pub portfolios: usize,
    pub holdings: usize,
    pub transactions: usize,
    pub fx_rates: usize,
    /// The FR51 history snapshots the file carried (issue #34, PR 3; `0` for a pre-#34 file).
    pub judgment_snapshots: usize,
}

/// Serialize a snapshot into its envelope JSON. The hash is taken over the **payload** (the snapshot
/// JSON), never over the envelope (which contains the hash) — the Story 5.2 rule.
fn snapshot_to_envelope_json(snapshot: &JournalSnapshot) -> Result<String> {
    let payload = serde_json::to_string(snapshot)?;
    let envelope = JournalExport {
        schema_version: snapshot.schema_version,
        journal_id: snapshot.journal_id.to_string(),
        logical_version: snapshot.logical_version,
        integrity_hash: sha256_hex(payload.as_bytes()),
        payload,
    };
    Ok(serde_json::to_string(&envelope)?)
}

/// Parse + verify an envelope back into a [`JournalSnapshot`] (no DB access). Rejects a hash mismatch
/// ([`ImportError::Integrity`]), an unsupported `schema_version` ([`ImportError::Version`]), or a
/// malformed envelope/payload ([`ImportError::Malformed`]). Never panics.
fn parse_and_verify(text: &str) -> std::result::Result<JournalSnapshot, ImportError> {
    let envelope: JournalExport =
        serde_json::from_str(text).map_err(|e| ImportError::Malformed(e.to_string()))?;
    // Integrity first: a corrupt payload must not even be parsed as a snapshot.
    if sha256_hex(envelope.payload.as_bytes()) != envelope.integrity_hash {
        return Err(ImportError::Integrity);
    }
    if envelope.schema_version != SCHEMA_VERSION {
        return Err(ImportError::Version {
            found: envelope.schema_version,
            supported: SCHEMA_VERSION,
        });
    }
    let snapshot: JournalSnapshot = serde_json::from_str(&envelope.payload)
        .map_err(|e| ImportError::Malformed(e.to_string()))?;
    // The envelope's declared version must agree with the snapshot it wraps (an honest export sets
    // both from `SCHEMA_VERSION`). A disagreement means a hand-crafted/inconsistent envelope.
    if snapshot.schema_version != envelope.schema_version {
        return Err(ImportError::Malformed(format!(
            "envelope schema_version {} disagrees with the snapshot's {}",
            envelope.schema_version, snapshot.schema_version
        )));
    }
    Ok(snapshot)
}

/// Peek an envelope's identity **without applying anything** (issue #65): the full
/// [`parse_and_verify`] gate (integrity + `schema_version` + parse — the peek refuses exactly what
/// the import would), then just the `(journal_id, logical_version)` pair the caller arbitrates
/// with (same journal + an OLDER version = a regression the user must confirm; the merge-import
/// would otherwise silently snap shared entities back to their old state).
pub fn inspect_journal_envelope(
    text: &str,
) -> std::result::Result<(Uuid, u64), steadyinvest_contract::ImportError> {
    let snapshot = parse_and_verify(text)?;
    Ok((snapshot.journal_id, snapshot.logical_version))
}

impl Journal {
    /// Read the complete journal into a [`JournalSnapshot`] (Story 5.3). Includes sold holdings and
    /// each study's lifecycle status. Deterministic order throughout (stable hash).
    fn journal_snapshot(&self) -> Result<JournalSnapshot> {
        let mut studies = Vec::new();
        for summary in self.list_studies()? {
            // `get_study` returns the full blob; pair it with the indexed `status` column. A study the
            // index lists but whose blob is unreadable is a journal inconsistency — fail **closed**
            // (a clear error) rather than silently dropping it from the export (data loss).
            let study = self
                .get_study(summary.id)?
                .ok_or_else(|| Error::CorruptPayload {
                    detail: format!(
                        "study {} is listed but its blob is absent; the export was not written",
                        summary.id
                    ),
                })?;
            studies.push(StudyRecord {
                status: summary.status,
                study,
            });
        }
        Ok(JournalSnapshot {
            schema_version: SCHEMA_VERSION,
            journal_id: self.id(),
            logical_version: self.logical_version()?,
            studies,
            watch_items: self.list_watch_items()?,
            portfolios: self.list_portfolios()?,
            holdings: self.list_all_holdings()?,
            transactions: self.list_all_transactions()?,
            fx_rates: self.list_fx_rates()?,
            judgment_snapshots: self.all_judgment_snapshot_rows()?,
        })
    }

    /// Every FR51 snapshot row, raw and deterministic (issue #34, PR 3): ordered by
    /// `(study_id, created_at, rowid)` so the canonical serialization — and its hash — is stable;
    /// payloads cross **byte-faithfully** (no parse, no re-serialization — an archive).
    fn all_judgment_snapshot_rows(&self) -> Result<Vec<JudgmentSnapshotRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, study_id, created_at, schema_version, payload FROM judgments
             ORDER BY study_id, created_at, rowid",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, study_id, created_at, schema_version, payload) = row?;
            let parse = |text: &str| {
                Uuid::parse_str(text).map_err(|e| Error::CorruptJournalMeta {
                    detail: format!("judgments id {text:?} is not a valid UUID: {e}"),
                })
            };
            out.push(JudgmentSnapshotRecord {
                id: parse(&id)?,
                study_id: parse(&study_id)?,
                created_at: Timestamp(created_at),
                schema_version,
                payload,
            });
        }
        Ok(out)
    }

    /// Export the whole journal to its portable envelope JSON (Story 5.3, FR60) — the serialized data
    /// contract + `schema_version` + `(journal_id, logical_version)` + integrity hash (NOT a raw
    /// `.db`). A pure read; the caller writes the string to a user-chosen file.
    pub fn export_journal(&self) -> Result<String> {
        let snapshot = self.journal_snapshot()?;
        snapshot_to_envelope_json(&snapshot)
    }

    /// Import a whole journal from its portable envelope JSON (Story 5.3, FR60/NFR-R5).
    ///
    /// Verifies integrity then `schema_version`, then applies **every entity in ONE transaction** —
    /// if any row fails the whole import rolls back (**never partially applied**). Each entity is
    /// upserted by its own `id` (a re-import updates in place, no duplicates); studies are **rebound**
    /// to this journal's `journal_id`, and every imported holding attaches to its **own** portfolio
    /// (Story 6.1, FR37 — the snapshot's portfolios are upserted first, ids preserved). Returns an
    /// [`ImportSummary`] of what was applied. Guarded: a read-only journal refuses; a verification
    /// failure maps to a typed [`Error`]; never panics.
    pub fn import_journal(&mut self, text: &str) -> Result<ImportSummary> {
        self.check_writable()?;
        // Verify before opening a transaction (a rejection writes nothing by construction).
        let snapshot = parse_and_verify(text)?; // From<ImportError> for Error
        let target_journal = self.id();

        let tx = self.conn.transaction()?;

        // Portfolios — seed/merge by id (Story 6.1, FR37: many portfolios). Each imported portfolio is
        // upserted under its OWN id (a same-id portfolio updates its name/created_at; a new id inserts),
        // so the multi-portfolio structure is preserved and holdings keep their own `portfolio_id`. We
        // pre-check existence per id so the summary counts true **inserts**, not mere updates.
        let mut portfolio_inserted = 0usize;
        for p in &snapshot.portfolios {
            let existed = tx
                .query_row(
                    "SELECT 1 FROM portfolios WHERE id = ?1",
                    rusqlite::params![p.id.to_string()],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            tx.execute(
                "INSERT INTO portfolios (id, name, created_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET name = excluded.name, created_at = excluded.created_at",
                rusqlite::params![p.id.to_string(), p.name, p.created_at.0],
            )?;
            if !existed {
                portfolio_inserted += 1;
            }
        }

        // Studies — rebind journal_id (blob + column) and restore the lifecycle status. Unlike
        // `put_study`'s upsert, we DO update `status` here: the export carried it, so a round-trip
        // restores an archived study as archived.
        for rec in &snapshot.studies {
            // Gate each study blob's own `schema_version` (a distinct axis from the envelope's): a blob
            // newer than this build would be written then be unreadable (`NewerRowSchema`) on the next
            // `get_study`. Reject up front so the whole import rolls back, never half-written.
            if rec.study.schema_version != SCHEMA_VERSION {
                return Err(Error::ImportVersion {
                    found: rec.study.schema_version,
                    supported: SCHEMA_VERSION,
                });
            }
            let mut study = rec.study.clone();
            study.journal_id = target_journal;
            let payload = serde_json::to_string(&study)?;
            tx.execute(
                "INSERT INTO studies
                     (id, journal_id, security_ticker, created_at, status, schema_version,
                      method_version, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                     journal_id = excluded.journal_id,
                     security_ticker = excluded.security_ticker,
                     created_at = excluded.created_at,
                     status = excluded.status,
                     schema_version = excluded.schema_version,
                     payload = excluded.payload",
                rusqlite::params![
                    study.id.to_string(),
                    target_journal.to_string(),
                    study.security_ticker,
                    study.created_at.0,
                    rec.status,
                    study.schema_version,
                    payload,
                ],
            )?;
        }

        // FR51 history snapshots (issue #34, PR 3) — after studies (the `judgments.study_id` FK).
        // A snapshot referencing a study absent from the file is a **malformed** snapshot (the
        // whole import rolls back); a row written by a NEWER schema is rejected up front (the same
        // rule as the studies gate — it would poison the timeline read otherwise). Upsert by id:
        // byte-faithful and idempotent (a re-import updates nothing new).
        let study_ids: std::collections::HashSet<Uuid> =
            snapshot.studies.iter().map(|r| r.study.id).collect();
        for js in &snapshot.judgment_snapshots {
            if !study_ids.contains(&js.study_id) {
                return Err(Error::ImportMalformed {
                    detail: "a history snapshot references a study absent from the snapshot"
                        .to_string(),
                });
            }
            if js.schema_version > i64::from(SCHEMA_VERSION) {
                return Err(Error::ImportVersion {
                    found: u32::try_from(js.schema_version).unwrap_or(u32::MAX),
                    supported: SCHEMA_VERSION,
                });
            }
            tx.execute(
                "INSERT INTO judgments (id, study_id, created_at, schema_version, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                     study_id = excluded.study_id,
                     created_at = excluded.created_at,
                     schema_version = excluded.schema_version,
                     payload = excluded.payload",
                rusqlite::params![
                    js.id.to_string(),
                    js.study_id.to_string(),
                    js.created_at.0,
                    js.schema_version,
                    js.payload
                ],
            )?;
        }

        // Holdings — attach to their OWN portfolio (Story 6.1, FR37). The set of portfolio ids just
        // upserted; a holding pointing at a portfolio absent from the snapshot is a **malformed**
        // snapshot — caught here as the neutral [`Error::ImportMalformed`] (the whole import rolls
        // back) rather than leaking a raw FK error to the user. Inserted before transactions so the
        // `transactions.holding_id` FK also has a live referent.
        let portfolio_ids: std::collections::HashSet<Uuid> =
            snapshot.portfolios.iter().map(|p| p.id).collect();
        for h in &snapshot.holdings {
            if !portfolio_ids.contains(&h.portfolio_id) {
                return Err(Error::ImportMalformed {
                    detail: "a holding references a portfolio absent from the snapshot".to_string(),
                });
            }
            tx.execute(
                "INSERT INTO holdings
                     (id, portfolio_id, security_ticker, quantity, purchase_price,
                      currency, sector, trailing_stop_pct, trailing_stop_level, sold_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(id) DO UPDATE SET
                     portfolio_id = excluded.portfolio_id,
                     security_ticker = excluded.security_ticker,
                     quantity = excluded.quantity,
                     purchase_price = excluded.purchase_price,
                     currency = excluded.currency,
                     sector = excluded.sector,
                     trailing_stop_pct = excluded.trailing_stop_pct,
                     trailing_stop_level = excluded.trailing_stop_level,
                     sold_at = excluded.sold_at,
                     created_at = excluded.created_at",
                rusqlite::params![
                    h.id.to_string(),
                    h.portfolio_id.to_string(),
                    h.security_ticker,
                    h.quantity,
                    h.purchase_price,
                    h.currency,
                    h.sector,
                    h.trailing_stop_pct,
                    h.trailing_stop_level,
                    h.sold_at,
                    h.created_at.0,
                ],
            )?;
        }

        // Transactions — after holdings (FK order). Like the holding→portfolio case above, a
        // transaction referencing a holding absent from the snapshot is a **malformed** snapshot —
        // caught as the neutral [`Error::ImportMalformed`] (whole import rolls back) rather than
        // leaking a raw FK error to the user.
        let holding_ids: std::collections::HashSet<Uuid> =
            snapshot.holdings.iter().map(|h| h.id).collect();
        for t in &snapshot.transactions {
            if !holding_ids.contains(&t.holding_id) {
                return Err(Error::ImportMalformed {
                    detail: "a transaction references a holding absent from the snapshot"
                        .to_string(),
                });
            }
            tx.execute(
                "INSERT INTO transactions
                     (id, holding_id, occurred_at, quantity, unit_price, fees, currency,
                      kind, rationale, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET
                     holding_id = excluded.holding_id,
                     occurred_at = excluded.occurred_at,
                     quantity = excluded.quantity,
                     unit_price = excluded.unit_price,
                     fees = excluded.fees,
                     currency = excluded.currency,
                     kind = excluded.kind,
                     rationale = excluded.rationale,
                     created_at = excluded.created_at",
                rusqlite::params![
                    t.id.to_string(),
                    t.holding_id.to_string(),
                    t.occurred_at.0,
                    t.quantity,
                    t.unit_price,
                    t.fees,
                    t.currency,
                    t.kind,
                    t.rationale,
                    t.created_at.0,
                ],
            )?;
        }

        // Watchlist — the `study_id` soft link is nullable (no hard FK); studies are already imported.
        for w in &snapshot.watch_items {
            tx.execute(
                "INSERT INTO watchlist_items (id, security_ticker, position, study_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                     security_ticker = excluded.security_ticker,
                     position = excluded.position,
                     study_id = excluded.study_id,
                     created_at = excluded.created_at",
                rusqlite::params![
                    w.id.to_string(),
                    w.security_ticker,
                    w.position,
                    w.study_id.map(|s| s.to_string()),
                    w.created_at.0,
                ],
            )?;
        }
        // Re-pack every watchlist row to a contiguous `0..n` by current `(position, id)` order. A merge
        // into a non-empty journal can otherwise leave the imported rows colliding with existing
        // positions (no UNIQUE constraint), breaking the FR34 contiguous user order. Only meaningful
        // when watchlist rows were imported.
        if !snapshot.watch_items.is_empty() {
            crate::watchlist::repack_positions(&tx)?;
        }

        // FX rates (Story 6.5, FR28) — 2026-07-02 review (HIGH): the upsert is keyed by the
        // NATURAL key `(base, quote, rate_date, source)`, exactly like the live writer — an
        // id-keyed upsert would let a MERGE (two machines minting different ids for the same
        // dated rate) plant natural-key duplicates the writer can never repair and the
        // arbitration then mis-picks. Rows are validated first (the manual form's invariants —
        // a foreign file must not plant states the live path refuses, the 6.1 precedent).
        for r in &snapshot.fx_rates {
            // Shape-only validation (this layer stays calc-agnostic — no decimal parser here):
            // digits with at most one dot, and at least one non-zero digit ⇒ a positive decimal.
            let rate_ok = !r.rate.is_empty()
                && r.rate.chars().all(|c| c.is_ascii_digit() || c == '.')
                && r.rate.matches('.').count() <= 1
                && r.rate.chars().any(|c| ('1'..='9').contains(&c));
            let date_ok = r.rate_date.len() == 10
                && r.rate_date.as_bytes()[4] == b'-'
                && r.rate_date.as_bytes()[7] == b'-';
            let pair_ok = r.base_currency != r.quote_currency
                && !r.base_currency.is_empty()
                && !r.quote_currency.is_empty();
            if !rate_ok || !date_ok || !pair_ok {
                return Err(Error::ImportMalformed {
                    detail: "an fx rate row is not a valid dated positive rate".to_string(),
                });
            }
            let existing: Option<String> = tx
                .query_row(
                    "SELECT id FROM fx_rates
                     WHERE base_currency = ?1 AND quote_currency = ?2
                       AND rate_date = ?3 AND source = ?4
                     ORDER BY id LIMIT 1",
                    rusqlite::params![r.base_currency, r.quote_currency, r.rate_date, r.source],
                    |row| row.get(0),
                )
                .optional()?;
            match existing {
                Some(existing_id) => {
                    tx.execute(
                        "UPDATE fx_rates SET rate = ?2, created_at = ?3
                          WHERE id = ?1 AND (rate IS NOT ?2 OR created_at IS NOT ?3)",
                        rusqlite::params![existing_id, r.rate, r.created_at.0],
                    )?;
                }
                None => {
                    tx.execute(
                        "INSERT INTO fx_rates
                             (id, base_currency, quote_currency, rate, rate_date, source, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        rusqlite::params![
                            r.id.to_string(),
                            r.base_currency,
                            r.quote_currency,
                            r.rate,
                            r.rate_date,
                            r.source,
                            r.created_at.0,
                        ],
                    )?;
                }
            }
        }

        // One heartbeat bump for the whole import act (an explicit user action, not a phantom write).
        // Only when the file actually **wrote** something — an empty snapshot, or an empty snapshot
        // merged into a journal that already has a portfolio, is a true no-op (no phantom revision on a
        // sync-sensitive store). Any non-empty entity set is a write here — the portfolio upserts
        // included (a same-id `ON CONFLICT DO UPDATE` rewrites the row, e.g. a renamed portfolio), so
        // a portfolios-only snapshot still bumps (NFR-R2), consistent with studies/holdings. An empty
        // snapshot is the true no-op.
        let applied = !snapshot.studies.is_empty()
            || !snapshot.watch_items.is_empty()
            || !snapshot.holdings.is_empty()
            || !snapshot.transactions.is_empty()
            || !snapshot.portfolios.is_empty()
            || !snapshot.fx_rates.is_empty()
            || !snapshot.judgment_snapshots.is_empty();
        if applied {
            bump_logical_version(&tx)?;
        }
        tx.commit()?;

        Ok(ImportSummary {
            source_journal_id: snapshot.journal_id,
            source_logical_version: snapshot.logical_version,
            studies: snapshot.studies.len(),
            fx_rates: snapshot.fx_rates.len(),
            watch_items: snapshot.watch_items.len(),
            // The count of portfolios the import actually **created** (new ids), not mere existence:
            // a same-id portfolio that was updated in place does not count.
            portfolios: portfolio_inserted,
            holdings: snapshot.holdings.len(),
            transactions: snapshot.transactions.len(),
            judgment_snapshots: snapshot.judgment_snapshots.len(),
        })
    }
}

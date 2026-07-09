//! Study storage — the blob side of the hybrid model.
//!
//! A [`Study`] is persisted **whole** as its serde-JSON payload (scale-preserving `Money`
//! strings included); indexed columns are extracted from the same struct. **Every mutating call
//! runs in a single transaction that also increments `journal_meta.logical_version`** — the
//! journal's heartbeat (NFR-R2; ADD6's stale-restore detection reads this counter): study row and
//! version bump commit together or not at all.

use crate::error::{Error, Result};
use crate::journal::Journal;
use crate::util::bump_logical_version;
use rusqlite::OptionalExtension;
use steadyinvest_contract::{SCHEMA_VERSION, Study, Timestamp};
use uuid::Uuid;

/// One row of [`Journal::list_studies`]: the indexed columns only — no payload parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudySummary {
    pub id: Uuid,
    pub security_ticker: String,
    pub created_at: Timestamp,
    /// `'active'` in v1 (archive/delete are Epic 2 features).
    pub status: String,
}

/// One row of [`Journal::list_judgment_snapshots`] (FR51, issue #34): the indexed columns of a
/// durable study snapshot — no payload parse (the timeline lists these; the diff parses two
/// payloads on demand via [`Journal::get_judgment_snapshot`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgmentSnapshotSummary {
    pub id: Uuid,
    pub created_at: Timestamp,
    pub schema_version: i64,
}

/// The deterministic identity of the `n`-th snapshot of a study (FR51, issue #34): the first 16
/// bytes of SHA-256 over `(study_id, ordinal)`. Persistence mints NO wall-clock/random identity
/// (the ADD15 injected-sources rule) and the app's `IdGen` cannot be threaded here without
/// breaking the fixed-id test harnesses — content-derived identity is deterministic, collision-free
/// per (study, n) by construction, and the ordinal only grows within a study's lifetime
/// (`delete_study` purges the rows with the study itself).
fn snapshot_id_for(study_id: Uuid, ordinal: i64) -> Uuid {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("{study_id}:judgment-snapshot:{ordinal}").as_bytes());
    Uuid::from_slice(&digest[..16]).expect("a SHA-256 digest always holds 16 bytes")
}

/// The shared studies upsert of [`Journal::put_study`] / [`Journal::put_study_with_history`]:
/// `ON CONFLICT(id) DO UPDATE` — NOT `INSERT OR REPLACE`, whose implicit DELETE+INSERT would
/// FK-fail (or cascade-delete) the FR51 `judgments` rows on every re-save. `status` and
/// `method_version` are not touched on update (Epic-2 lifecycle fields).
fn upsert_study_row(tx: &rusqlite::Transaction<'_>, study: &Study, payload: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO studies
             (id, journal_id, security_ticker, created_at, status, schema_version,
              method_version, payload)
         VALUES (?1, ?2, ?3, ?4, 'active', ?5, NULL, ?6)
         ON CONFLICT(id) DO UPDATE SET
             journal_id = excluded.journal_id,
             security_ticker = excluded.security_ticker,
             created_at = excluded.created_at,
             schema_version = excluded.schema_version,
             payload = excluded.payload",
        rusqlite::params![
            study.id.to_string(),
            study.journal_id.to_string(),
            study.security_ticker,
            study.created_at.0,
            study.schema_version,
            payload
        ],
    )?;
    Ok(())
}

impl Journal {
    /// Insert or update a study, atomically bumping the journal's logical version.
    ///
    /// Upsert is `INSERT … ON CONFLICT(id) DO UPDATE` — NOT `INSERT OR REPLACE`, whose implicit
    /// DELETE+INSERT would, once Epic 2 writes `judgments` rows, either FK-fail or cascade-delete
    /// the FR51 time-series on every re-save. `status` and `method_version` are not touched on
    /// update: they belong to Epic 2 features (`'active'` literal / `NULL` on first insert).
    ///
    /// A study whose `journal_id` differs from this journal's identity is rejected — a study
    /// from journal A is never silently written into journal B.
    pub fn put_study(&mut self, study: &Study) -> Result<()> {
        self.check_writable()?;
        if study.journal_id != self.id() {
            return Err(Error::JournalIdentityMismatch {
                study_journal_id: study.journal_id,
                journal_id: self.id(),
            });
        }
        let payload = serde_json::to_string(study)?;
        let tx = self.conn.transaction()?;
        upsert_study_row(&tx, study, &payload)?;
        bump_logical_version(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// [`Self::put_study`] **plus the FR51 durable snapshot** (issue #34): in the SAME transaction,
    /// the study upsert also appends a full-study snapshot row to the `judgments` time-series —
    /// unless the payload is **identical** to the study's latest snapshot (dedup: a value-identical
    /// re-save records no phantom history entry, the C4 spirit). `now` is the app's injected clock
    /// (ADD15); the snapshot id is content-derived ([`snapshot_id_for`]). The app rails write
    /// through THIS; the plain [`Self::put_study`] remains for callers that must not journal
    /// (test fabrication, the import merge — whose history travels in the envelope, PR 3).
    pub fn put_study_with_history(&mut self, study: &Study, now: &Timestamp) -> Result<()> {
        self.check_writable()?;
        if study.journal_id != self.id() {
            return Err(Error::JournalIdentityMismatch {
                study_journal_id: study.journal_id,
                journal_id: self.id(),
            });
        }
        let payload = serde_json::to_string(study)?;
        let tx = self.conn.transaction()?;
        upsert_study_row(&tx, study, &payload)?;
        // Dedup against the LATEST snapshot only (rowid breaks a same-instant tie — insertion
        // order): an A→B→A history keeps all three states; only a truly redundant re-save skips.
        let latest: Option<String> = tx
            .query_row(
                "SELECT payload FROM judgments WHERE study_id = ?1
                 ORDER BY created_at DESC, rowid DESC LIMIT 1",
                rusqlite::params![study.id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        if latest.as_deref() != Some(payload.as_str()) {
            let ordinal: i64 = tx.query_row(
                "SELECT COUNT(*) FROM judgments WHERE study_id = ?1",
                rusqlite::params![study.id.to_string()],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO judgments (id, study_id, created_at, schema_version, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    snapshot_id_for(study.id, ordinal).to_string(),
                    study.id.to_string(),
                    now.0,
                    study.schema_version,
                    payload
                ],
            )?;
        }
        bump_logical_version(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// The study's FR51 snapshot summaries, oldest first (`created_at`, then insertion order) —
    /// indexed columns only, no payload parse (issue #34; the timeline's listing read).
    pub fn list_judgment_snapshots(&self, study_id: Uuid) -> Result<Vec<JudgmentSnapshotSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, schema_version FROM judgments
             WHERE study_id = ?1 ORDER BY created_at, rowid",
        )?;
        let rows = stmt.query_map(rusqlite::params![study_id.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id_text, created_at, schema_version) = row?;
            let id = Uuid::parse_str(&id_text).map_err(|e| Error::CorruptJournalMeta {
                detail: format!("judgments.id {id_text:?} is not a valid UUID: {e}"),
            })?;
            out.push(JudgmentSnapshotSummary {
                id,
                created_at: Timestamp(created_at),
                schema_version,
            });
        }
        Ok(out)
    }

    /// Read one FR51 snapshot back as its full [`Study`] state (issue #34; the timeline's diff
    /// read). `Ok(None)` when the id is absent; the same newer-row-schema gate as
    /// [`Self::get_study`] — never a silent partial parse.
    pub fn get_judgment_snapshot(&self, id: Uuid) -> Result<Option<Study>> {
        let row: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT schema_version, payload FROM judgments WHERE id = ?1",
                rusqlite::params![id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((row_schema_version, payload)) = row else {
            return Ok(None);
        };
        if row_schema_version > i64::from(SCHEMA_VERSION) {
            return Err(Error::NewerRowSchema {
                row_schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        let study: Study = serde_json::from_str(&payload)?;
        Ok(Some(study))
    }

    /// Read a study back by id, parsing the stored payload. `Ok(None)` when the id is absent.
    ///
    /// A row whose `schema_version` is newer than this build's `contract::SCHEMA_VERSION` fails
    /// with a clear typed error — never a silent partial parse. (Unknown *fields* within a
    /// known-version payload are tolerated by design — the contract's forward-compat rail.)
    pub fn get_study(&self, id: Uuid) -> Result<Option<Study>> {
        let row: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT schema_version, payload FROM studies WHERE id = ?1",
                rusqlite::params![id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((row_schema_version, payload)) = row else {
            return Ok(None);
        };
        if row_schema_version > i64::from(SCHEMA_VERSION) {
            return Err(Error::NewerRowSchema {
                row_schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        let study: Study = serde_json::from_str(&payload)?;
        Ok(Some(study))
    }

    /// List the indexed columns of every study — no payload parse (the Epic 2 dashboard's
    /// building block). Ordered by `created_at` then `id` for a deterministic listing.
    pub fn list_studies(&self) -> Result<Vec<StudySummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, security_ticker, created_at, status
             FROM studies ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id_text, security_ticker, created_at, status) = row?;
            let id = Uuid::parse_str(&id_text).map_err(|e| Error::CorruptPayload {
                detail: format!("studies.id {id_text:?} is not a valid UUID: {e}"),
            })?;
            out.push(StudySummary {
                id,
                security_ticker,
                created_at: Timestamp(created_at),
                status,
            });
        }
        Ok(out)
    }

    /// Set a study's lifecycle `status` (Story 2.12, FR54/FR55): `"archived"` hides it from the
    /// default dashboard view, `"active"` restores it. One transaction that also bumps the journal's
    /// logical version. A pure indexed-column change — the Study blob payload and the FR51
    /// `judgments` time-series are untouched, so archive is fully reversible. An absent id is a no-op
    /// success (idempotent).
    pub fn set_study_status(&mut self, id: Uuid, status: &str) -> Result<()> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        let changed = tx.execute(
            "UPDATE studies SET status = ?1 WHERE id = ?2",
            rusqlite::params![status, id.to_string()],
        )?;
        // Only a real change bumps the heartbeat — an absent id is a true no-op (no phantom version
        // drift, which the stale-restore detection / external sync would otherwise see).
        if changed > 0 {
            bump_logical_version(&tx)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Permanently delete a study **and its own FR51 `judgments` time-series rows** in a single
    /// transaction (Story 2.12, FR55 — "without corrupting the journal time-series"): the judgments
    /// rows are removed first so the `judgments.study_id REFERENCES studies(id)` constraint (RESTRICT,
    /// `PRAGMA foreign_keys = true`) is never violated and **no orphan** is left behind; every other
    /// study's rows are untouched. Bumps the logical version. An absent id is a no-op success
    /// (idempotent — nothing to delete is not an error).
    pub fn delete_study(&mut self, id: Uuid) -> Result<()> {
        self.check_writable()?;
        let id_text = id.to_string();
        let tx = self.conn.transaction()?;
        let removed_judgments = tx.execute(
            "DELETE FROM judgments WHERE study_id = ?1",
            rusqlite::params![id_text],
        )?;
        let removed_study = tx.execute(
            "DELETE FROM studies WHERE id = ?1",
            rusqlite::params![id_text],
        )?;
        // Story 4.1 (FR34): a watchlist entry that pointed at this study must not dangle — clear the
        // soft link in the SAME transaction (the column is nullable, never a hard FK). Counts as a
        // real change so the version bumps when a link was actually cleared.
        let cleared_links = tx.execute(
            "UPDATE watchlist_items SET study_id = NULL WHERE study_id = ?1",
            rusqlite::params![id_text],
        )?;
        // Only a real removal bumps the heartbeat — deleting an absent id is a true no-op (no phantom
        // version drift for the stale-restore detection / external sync to see).
        if removed_study > 0 || removed_judgments > 0 || cleared_links > 0 {
            bump_logical_version(&tx)?;
        }
        tx.commit()?;
        Ok(())
    }
}

//! Watchlist storage (Story 4.1, FR34) — the normalized side of the hybrid model.
//!
//! A watchlist row is **typed columns**, not a serde blob: `id`, `security_ticker`, a contiguous
//! `position` (0-based, the user's order), an optional `study_id` (the saved study whose buy zone
//! this entry tracks — Story 4.2 reads it; a soft link, NULL-able, cleared on study delete), and
//! `created_at`. The DDL was frozen in v1 (Story 1.10); `study_id` arrives via migration v2.
//!
//! Like the study side, **every mutating call runs in one transaction that also bumps
//! `journal_meta.logical_version`** (NFR-R2) — and a no-op (an update to identical values, a
//! reorder that moves nothing) bumps **nothing** (the Epic-3 idempotency lesson: avoid phantom
//! journal revisions on a sync-sensitive store). Ids/timestamps come from the app's injected
//! `IdGen`/`Clock` (ADD15); persistence owns only the `position`.

use crate::error::{Error, Result};
use crate::journal::Journal;
use serde::{Deserialize, Serialize};
use steadyinvest_contract::Timestamp;
use uuid::Uuid;

/// One watchlist row (FR34). `study_id` is the optional soft link to a saved study (its buy zone).
/// `Serialize`/`Deserialize` so the whole-journal export (Story 5.3) carries it verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchItem {
    pub id: Uuid,
    pub security_ticker: String,
    pub position: i64,
    pub study_id: Option<Uuid>,
    pub created_at: Timestamp,
}

impl Journal {
    /// Append a watched security to the end of the list (FR34). `position` is computed as
    /// `max(position) + 1` (0 on an empty list) so the new entry sorts last. Bumps the logical
    /// version. Returns the inserted [`WatchItem`] (with its assigned position).
    pub fn add_watch_item(
        &mut self,
        id: Uuid,
        security_ticker: &str,
        study_id: Option<Uuid>,
        created_at: &Timestamp,
    ) -> Result<WatchItem> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        let position: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM watchlist_items",
            [],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO watchlist_items (id, security_ticker, position, study_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                id.to_string(),
                security_ticker,
                position,
                study_id.map(|s| s.to_string()),
                created_at.0,
            ],
        )?;
        tx.execute(
            "UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1",
            [],
        )?;
        tx.commit()?;
        Ok(WatchItem {
            id,
            security_ticker: security_ticker.to_string(),
            position,
            study_id,
            created_at: created_at.clone(),
        })
    }

    /// Every watchlist row, ordered by `position` then `id` (deterministic) — the surface's list.
    pub fn list_watch_items(&self) -> Result<Vec<WatchItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, security_ticker, position, study_id, created_at
             FROM watchlist_items ORDER BY position, id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id_text, security_ticker, position, study_text, created_at) = row?;
            out.push(WatchItem {
                id: parse_uuid(&id_text, "watchlist_items.id")?,
                security_ticker,
                position,
                study_id: study_text
                    .map(|s| parse_uuid(&s, "watchlist_items.study_id"))
                    .transpose()?,
                created_at: Timestamp(created_at),
            });
        }
        Ok(out)
    }

    /// Edit a watched security's ticker and/or its study link (FR34). `position` is untouched
    /// (reorder is [`Self::set_watch_positions`]). A no-op (identical ticker + link) writes nothing
    /// and bumps no version. An absent id is a no-op success (idempotent).
    pub fn update_watch_item(
        &mut self,
        id: Uuid,
        security_ticker: &str,
        study_id: Option<Uuid>,
    ) -> Result<()> {
        self.check_writable()?;
        let id_text = id.to_string();
        let study_text = study_id.map(|s| s.to_string());
        let tx = self.conn.transaction()?;
        let changed = tx.execute(
            "UPDATE watchlist_items SET security_ticker = ?2, study_id = ?3
             WHERE id = ?1 AND (security_ticker IS NOT ?2 OR study_id IS NOT ?3)",
            rusqlite::params![id_text, security_ticker, study_text],
        )?;
        if changed > 0 {
            tx.execute(
                "UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1",
                [],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Remove a watched security (FR34), then **re-pack** the remaining rows to a contiguous
    /// `0..n` order (so `position` stays meaningful). One transaction; bumps the version only on a
    /// real removal (an absent id is an idempotent no-op).
    pub fn delete_watch_item(&mut self, id: Uuid) -> Result<()> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        let removed = tx.execute(
            "DELETE FROM watchlist_items WHERE id = ?1",
            rusqlite::params![id.to_string()],
        )?;
        if removed > 0 {
            repack_positions(&tx)?;
            tx.execute(
                "UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1",
                [],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Reorder the watchlist (FR34): apply each `(id, position)` pair. One transaction; bumps the
    /// version only when at least one row actually moved (a reorder to the current order is a no-op).
    pub fn set_watch_positions(&mut self, positions: &[(Uuid, i64)]) -> Result<()> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        let mut moved = 0usize;
        for (id, position) in positions {
            moved += tx.execute(
                "UPDATE watchlist_items SET position = ?2 WHERE id = ?1 AND position IS NOT ?2",
                rusqlite::params![id.to_string(), position],
            )?;
        }
        if moved > 0 {
            tx.execute(
                "UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1",
                [],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

/// Re-number every watchlist row to a contiguous `0..n` by current `position` order (after a
/// delete). Runs inside the caller's transaction.
fn repack_positions(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    let ids: Vec<String> = {
        let mut stmt = tx.prepare("SELECT id FROM watchlist_items ORDER BY position, id")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    for (new_position, id) in ids.into_iter().enumerate() {
        tx.execute(
            "UPDATE watchlist_items SET position = ?2 WHERE id = ?1 AND position IS NOT ?2",
            rusqlite::params![id, new_position as i64],
        )?;
    }
    Ok(())
}

fn parse_uuid(text: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(text).map_err(|e| Error::CorruptPayload {
        detail: format!("{field} {text:?} is not a valid UUID: {e}"),
    })
}

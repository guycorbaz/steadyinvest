//! Dated, source-aware exchange rates — the **FR28 FX store** (Story 6.5, AC1).
//!
//! The `fx_rates` table was frozen in the v1 DDL (Story 1.10, `schema.rs`) and stayed inert
//! through 6.2's narrow multi-currency slice; Story 6.5 makes it load-bearing — MIGRATION-FREE
//! (`user_version` stays 6). Each row is one rate **BASE → QUOTE** on one day from one source
//! (`"manuel"`, `"eodhd"`, `"twelvedata"`, …): FR28 requires every conversion the app ever shows
//! (Story 6.6, consolidation only) to carry a date and a source the user can inspect, so the
//! store keeps them per row and never derives (no inverse-pair guessing — 1/rate is refused
//! honestly at the read, not synthesized here).
//!
//! [`Journal::upsert_fx_rate`] is keyed by the **natural key**
//! `(base_currency, quote_currency, rate_date, source)`: a same-day re-fetch from the same
//! source updates in place (no duplicate rows, no phantom history), while a manual entry and a
//! provider row on the same day **coexist** (distinct sources) — [`Journal::latest_fx_rate`]
//! arbitrates by `rate_date`, then `created_at` (the later write wins a same-day tie). One
//! transaction per mutation with exactly one `logical_version` bump on an APPLIED change
//! (NFR-R2 — unlike `price_history`, `fx_rates` **is** an exported axis, AC5); an
//! identical-values re-upsert is a true no-op: nothing written, no bump (Epic-3 C4).
//!
//! `rate` is an exact TEXT decimal (NFR-C1 — never REAL, no SQL arithmetic); `rate_date` is
//! stored **verbatim** (AAAA-MM-JJ or RFC3339 — the app normalizes before writing), and the
//! lexical `<=` on ISO spellings is the chronological comparison [`Journal::latest_fx_rate`]
//! relies on. Ids/timestamps are caller-supplied (ADD15). Conversion itself lives in pure core
//! (`core::risk::fx`, AC2) — this layer stores rates and never multiplies anything.

use crate::error::Result;
use crate::journal::Journal;
use crate::util::{bump_logical_version, parse_uuid};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use steadyinvest_contract::Timestamp;
use uuid::Uuid;

/// One dated, source-aware exchange rate **BASE → QUOTE** (Story 6.5, FR28). `rate` is the exact
/// TEXT decimal spelling the app validated (NFR-C1); `rate_date` is the day the rate is *for*
/// (AAAA-MM-JJ or RFC3339 — stored verbatim, app-normalized), distinct from the row's
/// `created_at` clock stamp; `source` names where it came from (`"manuel"` or a provider wire
/// name). `Serialize`/`Deserialize` so the whole-journal export (Story 5.3, AC5) carries it
/// verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FxRateItem {
    pub id: Uuid,
    pub base_currency: String,
    pub quote_currency: String,
    pub rate: String,
    pub rate_date: String,
    pub source: String,
    pub created_at: Timestamp,
}

/// Map one `fx_rates` row (SELECTed in schema column order) into an [`FxRateItem`] — shared by
/// every reader so the id parse names the column once.
fn item_from_row(
    (id_text, base, quote, rate, rate_date, source, created): (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ),
) -> Result<FxRateItem> {
    Ok(FxRateItem {
        id: parse_uuid(&id_text, "fx_rates.id")?,
        base_currency: base,
        quote_currency: quote,
        rate,
        rate_date,
        source,
        created_at: Timestamp(created),
    })
}

/// The row tuple every `fx_rates` SELECT maps to (schema column order).
type FxRow = (String, String, String, String, String, String, String);

fn row_tuple(r: &rusqlite::Row<'_>) -> rusqlite::Result<FxRow> {
    Ok((
        r.get::<_, String>(0)?,
        r.get::<_, String>(1)?,
        r.get::<_, String>(2)?,
        r.get::<_, String>(3)?,
        r.get::<_, String>(4)?,
        r.get::<_, String>(5)?,
        r.get::<_, String>(6)?,
    ))
}

impl Journal {
    /// Upsert one exchange rate by its **natural key**
    /// `(base_currency, quote_currency, rate_date, source)` (Story 6.5, AC1 — FR28). In one
    /// transaction: SELECT the existing row first; when it exists with the **identical** `rate`,
    /// nothing is written and nothing bumps (`Ok(false)` — a same-day re-fetch is a true no-op,
    /// Epic-3 C4); when it exists with a different `rate`, the rate is UPDATEd **in place**
    /// keeping the row's original `id` (no duplicate, no phantom history) while `created_at`
    /// refreshes to the caller's stamp (the later write wins the same-day tie — 2026-07-02
    /// review); when absent, the row is INSERTed with the caller's `id`/`created_at`
    /// (ADD15) plus one bump. Returns whether anything was applied. Distinct sources on the same
    /// `(pair, date)` are distinct rows by design — [`Self::latest_fx_rate`] arbitrates.
    pub fn upsert_fx_rate(&mut self, item: &FxRateItem) -> Result<bool> {
        self.check_writable()?;
        let tx = self.conn.transaction()?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT id, rate FROM fx_rates
                 WHERE base_currency = ?1 AND quote_currency = ?2
                   AND rate_date = ?3 AND source = ?4
                 ORDER BY id LIMIT 1",
                rusqlite::params![
                    item.base_currency,
                    item.quote_currency,
                    item.rate_date,
                    item.source
                ],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?;
        match existing {
            Some((_, ref rate)) if *rate == item.rate => {
                // Identical values → true no-op: nothing written, no bump (Epic-3 C4). The
                // uncommitted transaction drops without effect.
                Ok(false)
            }
            Some((existing_id, _)) => {
                // The in-place update also refreshes `created_at` to the CALLER's stamp
                // (2026-07-02 review, HIGH): `latest_fx_rate` breaks a same-day tie by
                // `created_at DESC` ("the later write wins") — a corrected rate that kept its
                // original stamp would stay permanently outranked by a mid-day row from the
                // other source. The row keeps its `id` (no duplicate, no phantom history);
                // `created_at` records the latest write, which is exactly the arbitration fact.
                tx.execute(
                    "UPDATE fx_rates SET rate = ?2, created_at = ?3 WHERE id = ?1",
                    rusqlite::params![existing_id, item.rate, item.created_at.0],
                )?;
                bump_logical_version(&tx)?;
                tx.commit()?;
                Ok(true)
            }
            None => {
                tx.execute(
                    "INSERT INTO fx_rates
                         (id, base_currency, quote_currency, rate, rate_date, source, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![
                        item.id.to_string(),
                        item.base_currency,
                        item.quote_currency,
                        item.rate,
                        item.rate_date,
                        item.source,
                        item.created_at.0,
                    ],
                )?;
                bump_logical_version(&tx)?;
                tx.commit()?;
                Ok(true)
            }
        }
    }

    /// Every stored exchange rate, deterministic (Story 6.5, AC1): pair ascending, then
    /// `rate_date` **DESC** (most recent first per pair — the Réglages panel order, AC4), then
    /// `source`, then `id`. The whole-journal export (Story 5.3, AC5) reads this — the fixed
    /// order keeps the export bytes deterministic.
    pub fn list_fx_rates(&self) -> Result<Vec<FxRateItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, base_currency, quote_currency, rate, rate_date, source, created_at
             FROM fx_rates
             ORDER BY base_currency, quote_currency, rate_date DESC, source, id",
        )?;
        let rows = stmt.query_map([], row_tuple)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(item_from_row(row?)?);
        }
        Ok(out)
    }

    /// The most recent rate for **exactly** `base → quote` (Story 6.5, AC1 — FR28): the row with
    /// the greatest `rate_date` ≤ `on_or_before` when `Some` (a lexical `<=` — ISO date spellings
    /// compare chronologically), or the greatest overall when `None`. A `rate_date` tie (e.g. a
    /// manual and a provider rate on the same day) is broken by `created_at` DESC — the later
    /// write wins — then `id` (deterministic). Returns the **full item** so the caller can always
    /// show the date and source next to any converted figure (FR28). `Ok(None)` when no rate for
    /// the pair exists in the window — the inverted pair is **never** consulted (no silent
    /// 1/rate; Story 6.6 refuses honestly instead).
    pub fn latest_fx_rate(
        &self,
        base: &str,
        quote: &str,
        on_or_before: Option<&str>,
    ) -> Result<Option<FxRateItem>> {
        let row: Option<FxRow> = self
            .conn
            .query_row(
                "SELECT id, base_currency, quote_currency, rate, rate_date, source, created_at
                 FROM fx_rates
                 WHERE base_currency = ?1 AND quote_currency = ?2
                   AND (?3 IS NULL OR rate_date <= ?3)
                 ORDER BY rate_date DESC, created_at DESC, id
                 LIMIT 1",
                rusqlite::params![base, quote, on_or_before],
                row_tuple,
            )
            .optional()?;
        row.map(item_from_row).transpose()
    }
}

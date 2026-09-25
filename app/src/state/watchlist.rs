//! Watchlist CRUD (Story 4.1, FR34): watched securities ordered by position — add/edit/delete/move
//! — plus the case-insensitive most-recent-study auto-match backing the watch→study link. All
//! writes ride the shared read-only / no-journal guards and map persistence errors to neutral
//! notices via [`super::watch_error`].

use steadyinvest_persistence::WatchItem;
use uuid::Uuid;

use super::{
    JournalState, MSG_BLANK_TICKER, MSG_NO_JOURNAL, MSG_READ_FAILED, MSG_READ_ONLY_WRITE,
    MSG_WATCH_DUPLICATE, watch_error,
};

impl JournalState {
    // ── Watchlist (Story 4.1, FR34) ──

    /// Every watched security, ordered by position. Empty when no journal is open. Absence-blind —
    /// a consumer that STATES absence must use [`Self::try_list_watch_items`] (issue #95).
    pub fn list_watch_items(&self) -> Vec<WatchItem> {
        self.try_list_watch_items().unwrap_or_default()
    }

    /// Fallible [`Self::list_watch_items`] (issue #95): `Err` is a read failure, never an empty list.
    pub fn try_list_watch_items(&self) -> Result<Vec<WatchItem>, String> {
        let Some(journal) = self.journal.as_ref() else {
            return Ok(Vec::new());
        };
        journal.list_watch_items().map_err(|error| {
            tracing::warn!("list_watch_items failed: {error}");
            error.to_string()
        })
    }

    /// The most-recent saved study whose ticker matches `ticker` **case-insensitively** (Story 4.1
    /// watchlist link auto-match), or `None`. `list_studies` is ascending by `created_at`, so the
    /// last match is the newest. Case-insensitive so a watched `"nesn"` still finds the `"NESN"`
    /// study (tickers are stored as entered, not normalized). Absence-blind — a consumer that
    /// STATES absence must use [`Self::try_study_id_for_ticker`] (issue #95).
    pub fn study_id_for_ticker(&self, ticker: &str) -> Option<Uuid> {
        self.try_study_id_for_ticker(ticker).ok().flatten()
    }

    /// Fallible [`Self::study_id_for_ticker`] (issue #95): `Ok(None)` is a true no-match, `Err` a
    /// read failure — so « aucune étude » is never stated over a failed listing.
    pub fn try_study_id_for_ticker(&self, ticker: &str) -> Result<Option<Uuid>, String> {
        Ok(self
            .try_list_studies()?
            .into_iter()
            .rev()
            .find(|s| same_ticker(&s.security_ticker, ticker))
            .map(|s| s.id))
    }

    /// Issue #81: like [`Self::study_id_for_ticker`], but a holding auto-match ALSO requires the same
    /// currency — so a CHF holding never links a USD study of the same ticker (which would then price
    /// its sale / ratchet its stop / display its register row at the wrong-currency figure). A holding
    /// with **no** declared currency (`None`) falls back to the ticker-only match (today's behaviour).
    /// The watchlist link (no currency) keeps using [`Self::study_id_for_ticker`]. Absence-blind —
    /// a consumer that STATES absence must use [`Self::try_matched_study_in_currency`] (issue #95).
    /// Test-only since G1 P: every lot resolves through [`Self::lot_study_id`] / [`Self::try_lot_study`].
    #[cfg(test)]
    pub fn study_id_for_ticker_in_currency(
        &self,
        ticker: &str,
        currency: Option<&str>,
    ) -> Option<Uuid> {
        self.try_study_id_for_ticker_in_currency(ticker, currency)
            .ok()
            .flatten()
    }

    /// Fallible [`Self::study_id_for_ticker_in_currency`] (issue #95): `Ok(None)` is a true
    /// no-match, `Err` a read failure — including a candidate whose payload could not be parsed
    /// (the absence-blind version silently skipped it).
    pub fn try_study_id_for_ticker_in_currency(
        &self,
        ticker: &str,
        currency: Option<&str>,
    ) -> Result<Option<Uuid>, String> {
        // No declared currency → keep the cheap ticker-only match (today's behaviour).
        let Some(currency) = currency else {
            return self.try_study_id_for_ticker(ticker);
        };
        // The currency lives in the JSON payload (not an indexed summary column), so load only the
        // same-ticker candidates — usually 0–2 — newest-first, and take the first currency match.
        for summary in self
            .try_list_studies()?
            .into_iter()
            .rev()
            .filter(|s| same_ticker(&s.security_ticker, ticker))
        {
            let Some(study) = self.try_get_study(summary.id)? else {
                continue; // deleted between the listing and the read — a true absence
            };
            if study
                .native_currency
                .trim()
                .eq_ignore_ascii_case(currency.trim())
            {
                return Ok(Some(study.id));
            }
        }
        Ok(None)
    }

    /// THE link of a held lot (D5, Guy 2026-09-25; G1 P review H1): the newest study of the
    /// lot's ticker in its EFFECTIVE currency — a legacy lot without a declared currency is
    /// presumed in the reference currency and links by it, never ticker-only. The register, the
    /// review, the stop's seed and ratchet, the trigger sale and the price refresh all resolve a
    /// lot through here. A same-ticker study in another currency is only ever a HINT (named by
    /// the surfaces that state « aucune étude liée »), never the link.
    pub fn try_lot_study(
        &self,
        ticker: &str,
        declared_currency: Option<&str>,
        reference_currency: &str,
    ) -> Result<Option<steadyinvest_contract::Study>, String> {
        self.try_matched_study_in_currency(
            ticker,
            Some(declared_currency.unwrap_or(reference_currency)),
        )
    }

    /// Absence-blind [`Self::try_lot_study`]'s id (the price-refresh targets).
    pub fn lot_study_id(
        &self,
        ticker: &str,
        declared_currency: Option<&str>,
        reference_currency: &str,
    ) -> Option<Uuid> {
        self.try_study_id_for_ticker_in_currency(
            ticker,
            Some(declared_currency.unwrap_or(reference_currency)),
        )
        .ok()
        .flatten()
    }

    /// The tri-state holding→study auto-match (issue #95): `Ok(Some)` the matched study,
    /// `Ok(None)` truly none — the only case a consumer may state « aucune étude liée » —
    /// `Err` a read failure (« indisponible »).
    pub fn try_matched_study_in_currency(
        &self,
        ticker: &str,
        currency: Option<&str>,
    ) -> Result<Option<steadyinvest_contract::Study>, String> {
        match self.try_study_id_for_ticker_in_currency(ticker, currency)? {
            Some(sid) => self.try_get_study(sid),
            None => Ok(None),
        }
    }

    /// Add a watched security (FR34) — appended at the end. Id/timestamp from the injected sources
    /// (ADD15). Guarded (read-only / no-journal / save-failure → a neutral notice).
    pub fn add_watch_item(&mut self, ticker: &str, study_id: Option<Uuid>) -> Result<(), String> {
        let ticker = ticker.trim();
        if ticker.is_empty() {
            return Err(MSG_BLANK_TICKER.to_string());
        }
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        // G1 P (G3 L4): the duplicate check SEES a failed read — refused by name, never taken for
        // an empty list (which would let a duplicate through).
        if self
            .try_list_watch_items()
            .map_err(|_| MSG_READ_FAILED.to_string())?
            .iter()
            .any(|w| w.security_ticker.eq_ignore_ascii_case(ticker))
        {
            return Err(MSG_WATCH_DUPLICATE.to_string());
        }
        let id = self.idgen.new_id();
        let created_at = self.clock.now();
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .add_watch_item(id, ticker, study_id, &created_at)
            .map(|_| ())
            .map_err(watch_error)
    }

    /// Edit a watched security's ticker and/or study link (FR34). Blank ticker is refused.
    pub fn update_watch_item(
        &mut self,
        id: Uuid,
        ticker: &str,
        study_id: Option<Uuid>,
    ) -> Result<(), String> {
        let ticker = ticker.trim();
        if ticker.is_empty() {
            return Err(MSG_BLANK_TICKER.to_string());
        }
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .update_watch_item(id, ticker, study_id)
            .map_err(watch_error)
    }

    /// Remove a watched security (FR34); the remaining rows re-pack to contiguous positions.
    pub fn delete_watch_item(&mut self, id: Uuid) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal.delete_watch_item(id).map_err(watch_error)
    }

    /// Move a watched security one slot up (`up = true`) or down in the order (FR34): swap its
    /// position with its neighbour. A no-op at the list edge.
    pub fn move_watch_item(&mut self, id: Uuid, up: bool) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let items = self.list_watch_items();
        let Some(index) = items.iter().position(|w| w.id == id) else {
            return Ok(()); // gone — nothing to move
        };
        let neighbour = if up {
            index.checked_sub(1)
        } else {
            (index + 1 < items.len()).then_some(index + 1)
        };
        let Some(n) = neighbour else {
            return Ok(()); // already at the edge — a neutral no-op
        };
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .set_watch_positions(&[
                (items[index].id, items[n].position),
                (items[n].id, items[index].position),
            ])
            .map_err(watch_error)
    }
}

/// Two tickers name the same security (G1 P review L-h): compared ignoring case AND surrounding
/// spaces — the rule the persistence stop-clearing compare (`UPPER(TRIM(…))`) shares.
pub(crate) fn same_ticker(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

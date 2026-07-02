//! Transaction-ledger rails (Story 6.3, FR39) — buys, partial sells, edit/delete, and the
//! weighted-average cost basis.
//!
//! The app is the ORCHESTRATOR of the three-layer split: it reads the holding's ledger rows
//! (persistence), replays them through the pure [`steadyinvest_core::risk::derive_position`]
//! (Appendix A: weighted-average, fees included), and hands the recomputed aggregate to ONE atomic
//! persistence writer — persistence does no arithmetic, core does no IO. `holdings.quantity` /
//! `holdings.purchase_price` are the **materialized aggregate** of the replay.
//!
//! **Opening-position materialization (AC5):** a pre-6.3 holding has no buy rows — its stored
//! `(quantity, purchase_price)` IS the opening position (4.7 whole-position sells never reduced
//! `quantity`, so this also holds for a sold legacy holding). The first ledger mutation therefore
//! writes an opening `kind = "buy"` row (dated `holdings.created_at`'s DAY, fees 0) in the same
//! transaction, making the ledger self-contained: from then on the replay is always
//! `derive_position(None, all_rows)`. The rule (2026-07-02 review, CRITICAL fix): materialize iff
//! the holding's **current** ledger holds no buy row — and ALWAYS computed against the current
//! rows, never the post-mutation remainder. Once any 6.3 mutation has run, the stored aggregate is
//! DERIVED and can no longer stand in for the opening; re-seeding from it would double-count the
//! surviving sells. Deleting the only buy row that sells depend on is therefore an honest
//! [`MSG_LEDGER_OVERSELL`] refusal, not a fabricated position.
//!
//! Replay order (2026-07-02 review): `(occurred_at, created_at, buys-first, id)` — the event date
//! first (every 6.3 event is **date-granular**, stored at midnight UTC, so a backdated entry
//! re-averages history exactly as if it had been recorded on time), then the insertion clock stamp
//! (same-day entries replay in the order they were recorded), then **buys before sells** on a full
//! tie (conservative: never a manufactured over-sell; consecutive buys commute, so the final id
//! tiebreak cannot change the outcome). Legacy 4.7 sells keep their wall-clock `occurred_at`; an
//! edit that leaves the visible date unchanged keeps the original stamp verbatim (the Epic-3 C4
//! no-op guarantee holds for them too). An edit that would make any sell exceed the quantity held
//! at its point in history is a typed refusal ([`MSG_LEDGER_OVERSELL`]), never a negative
//! position.
//!
//! Scope notes (review): **undo/redo stays study-scoped** — ledger mutations are journal writes
//! outside `UndoHistory`, like every holdings edit before them. The replay reads the ledger
//! *outside* the persistence transaction that writes the aggregate — sound because the app is the
//! journal's single writer (the 5.5 single-instance lock); a second concurrent writer would
//! require moving the derivation inside the transaction. The **write** rails read via
//! [`JournalState::ledger_rows_strict`] (a failed read REFUSES the mutation — an error is not an
//! empty ledger); only the display path tolerates a failed read.

use rust_decimal::Decimal;
use steadyinvest_core::risk::{
    derive_position, LedgerError, LedgerEvent, LedgerEventKind, PositionBasis,
};
use steadyinvest_persistence::{HoldingItem, LedgerEntry, TransactionItem, KIND_BUY};
use uuid::Uuid;

use super::{
    effective_currency, watch_error, JournalState, MSG_HOLDING_INVALID_NUMBER, MSG_HOLDING_SOLD,
    MSG_LEDGER_INVALID_DATE, MSG_LEDGER_OVERSELL, MSG_LEDGER_PARTIAL_SOLD, MSG_NO_JOURNAL,
    MSG_READ_ONLY_WRITE, MSG_SAVE_FAILED,
};

/// An owned ledger-row draft — the borrow-free twin of [`LedgerEntry`] (which borrows), so the
/// rails can build opening/new rows and then lend them to persistence.
struct OwnedEntry {
    id: Uuid,
    occurred_at: String,
    quantity: String,
    unit_price: String,
    fees: String,
    currency: String,
    rationale: Option<String>,
}

impl OwnedEntry {
    fn as_entry(&self) -> LedgerEntry<'_> {
        LedgerEntry {
            id: self.id,
            occurred_at: &self.occurred_at,
            quantity: &self.quantity,
            unit_price: &self.unit_price,
            fees: &self.fees,
            currency: &self.currency,
            rationale: self.rationale.as_deref(),
        }
    }
}

/// Normalize the user's transaction date (FR39's "date") to the stored RFC3339 spelling.
/// `""` defaults to today (the injected clock's date); otherwise a plausible `YYYY-MM-DD` is
/// required and stored as midnight UTC so it orders correctly against full timestamps.
fn normalize_event_date(input: &str, now_rfc3339: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let date = if trimmed.is_empty() {
        now_rfc3339.get(..10).unwrap_or_default().to_string()
    } else {
        trimmed.to_string()
    };
    let bytes = date.as_bytes();
    let shape_ok = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && date
            .char_indices()
            .all(|(i, c)| matches!(i, 4 | 7) || c.is_ascii_digit());
    if !shape_ok {
        return Err(MSG_LEDGER_INVALID_DATE.to_string());
    }
    let year: u32 = date[..4]
        .parse()
        .map_err(|_| MSG_LEDGER_INVALID_DATE.to_string())?;
    let month: u32 = date[5..7]
        .parse()
        .map_err(|_| MSG_LEDGER_INVALID_DATE.to_string())?;
    let day: u32 = date[8..10]
        .parse()
        .map_err(|_| MSG_LEDGER_INVALID_DATE.to_string())?;
    // A REAL calendar date (2026-07-02 review: Feb 30 / Apr 31 / year 0000 must refuse): proper
    // month lengths incl. the Gregorian leap rule, and a sane year window for a personal journal.
    let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    let month_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err(MSG_LEDGER_INVALID_DATE.to_string()),
    };
    if !(1900..=2200).contains(&year) || day == 0 || day > month_days {
        return Err(MSG_LEDGER_INVALID_DATE.to_string());
    }
    Ok(format!("{date}T00:00:00Z"))
}

/// Validate the ledger amounts (Story 6.3 twin of `validate_holding_amounts`): quantity strictly
/// positive, unit price and fees non-negative; empty fees default to `"0"`. Returns the canonical
/// decimal spellings.
fn validate_ledger_amounts(
    quantity: &str,
    unit_price: &str,
    fees: &str,
) -> Result<(String, String, String), String> {
    let qty = Decimal::from_str_exact(quantity.trim())
        .ok()
        .filter(|q| q.is_sign_positive() && !q.is_zero())
        .ok_or(MSG_HOLDING_INVALID_NUMBER.to_string())?;
    let price = Decimal::from_str_exact(unit_price.trim())
        .ok()
        .filter(|p| !p.is_sign_negative())
        .ok_or(MSG_HOLDING_INVALID_NUMBER.to_string())?;
    let fees = fees.trim();
    let fees = if fees.is_empty() { "0" } else { fees };
    let fees = Decimal::from_str_exact(fees)
        .ok()
        .filter(|f| !f.is_sign_negative())
        .ok_or(MSG_HOLDING_INVALID_NUMBER.to_string())?;
    Ok((qty.to_string(), price.to_string(), fees.to_string()))
}

/// One replay candidate: the `(occurred_at, created_at, buys-first, id)` sort key plus the parsed
/// event (see the module doc for why each key exists).
struct Candidate {
    occurred_at: String,
    created_at: String,
    id: Uuid,
    event: LedgerEvent,
}

/// Parse a stored transaction row into its pure replay event. A `NULL` kind is read as a sell
/// (4.7 was the only pre-6.3 writer and always wrote sells; nothing else ever produced NULL);
/// any other unknown kind — or an unparseable stored decimal — is a corrupt row and refuses.
fn event_of(item: &TransactionItem) -> Result<LedgerEvent, String> {
    let kind = match item.kind.as_deref() {
        Some(KIND_BUY) => LedgerEventKind::Buy,
        Some("sell") | None => LedgerEventKind::Sell,
        Some(_) => return Err(MSG_SAVE_FAILED.to_string()),
    };
    let dec = |s: &str| Decimal::from_str_exact(s).map_err(|_| MSG_SAVE_FAILED.to_string());
    Ok(LedgerEvent {
        kind,
        quantity: dec(&item.quantity)?,
        unit_price: dec(&item.unit_price)?,
        fees: dec(&item.fees)?,
    })
}

fn candidate_of(item: &TransactionItem) -> Result<Candidate, String> {
    Ok(Candidate {
        occurred_at: item.occurred_at.0.clone(),
        created_at: item.created_at.0.clone(),
        id: item.id,
        event: event_of(item)?,
    })
}

fn candidate_of_owned(
    entry: &OwnedEntry,
    kind: LedgerEventKind,
    created_at: &str,
) -> Result<Candidate, String> {
    let dec =
        |s: &str| Decimal::from_str_exact(s).map_err(|_| MSG_HOLDING_INVALID_NUMBER.to_string());
    Ok(Candidate {
        occurred_at: entry.occurred_at.clone(),
        created_at: created_at.to_string(),
        id: entry.id,
        event: LedgerEvent {
            kind,
            quantity: dec(&entry.quantity)?,
            unit_price: dec(&entry.unit_price)?,
            fees: dec(&entry.fees)?,
        },
    })
}

/// Sort candidates into replay order — `(occurred_at, created_at, buys-first, id)`, see the module
/// doc — and fold them through the pure core derivation, mapping the typed [`LedgerError`]s to the
/// neutral notices.
fn replay(mut candidates: Vec<Candidate>) -> Result<PositionBasis, String> {
    let sell_rank = |c: &Candidate| matches!(c.event.kind, LedgerEventKind::Sell) as u8;
    candidates.sort_by(|a, b| {
        a.occurred_at
            .cmp(&b.occurred_at)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| sell_rank(a).cmp(&sell_rank(b)))
            .then_with(|| a.id.cmp(&b.id))
    });
    let events: Vec<LedgerEvent> = candidates.iter().map(|c| c.event).collect();
    derive_position(None, &events).map_err(|e| match e {
        LedgerError::OverSell => MSG_LEDGER_OVERSELL.to_string(),
        LedgerError::NonPositiveQuantity | LedgerError::NegativeAmount | LedgerError::Overflow => {
            MSG_HOLDING_INVALID_NUMBER.to_string()
        }
    })
}

impl JournalState {
    /// The holding's ledger rows, oldest first (Story 6.3, FR39) — read-only, `[]` without a
    /// journal or on a read failure (a DISPLAY surface, never a hard error). Write rails must use
    /// [`Self::ledger_rows_strict`] instead.
    pub fn holding_ledger(&self, holding_id: Uuid) -> Vec<TransactionItem> {
        self.journal
            .as_ref()
            .and_then(|j| j.list_transactions(holding_id).ok())
            .unwrap_or_default()
    }

    /// The holding's ledger rows for a WRITE rail (2026-07-02 review, HIGH): a failed read is a
    /// refused mutation — treating it as an empty ledger would materialize a second opening row
    /// and silently double the position on the next replay.
    fn ledger_rows_strict(&self, holding_id: Uuid) -> Result<Vec<TransactionItem>, String> {
        self.journal
            .as_ref()
            .ok_or(MSG_NO_JOURNAL.to_string())?
            .list_transactions(holding_id)
            .map_err(watch_error)
    }

    /// Any holding by id — including a sold one (the ledger of a retired holding stays editable;
    /// deleting its retiring sell un-retires it).
    fn any_holding(&self, holding_id: Uuid) -> Result<HoldingItem, String> {
        self.journal
            .as_ref()
            .ok_or(MSG_NO_JOURNAL.to_string())?
            .list_all_holdings()
            .map_err(watch_error)?
            .into_iter()
            .find(|h| h.id == holding_id)
            .ok_or(MSG_SAVE_FAILED.to_string())
    }

    /// The opening-position row to materialize, iff the holding's **current** ledger holds no buy
    /// yet (see the module doc — NEVER computed against a post-mutation remainder: once a buy row
    /// exists the stored aggregate is derived and cannot stand in for the opening). Seeds from the
    /// stored `(quantity, purchase_price)`, dated `created_at`'s **day** (midnight UTC, the 6.3
    /// date granularity — an opening-day sell must not sort before its own opening), fees `0`.
    fn opening_for(
        &mut self,
        holding: &HoldingItem,
        current_rows: &[TransactionItem],
        reference_currency: &str,
    ) -> Option<OwnedEntry> {
        let has_buy = current_rows
            .iter()
            .any(|r| r.kind.as_deref() == Some(KIND_BUY));
        if has_buy {
            return None;
        }
        let day = holding.created_at.0.get(..10).unwrap_or("1900-01-01");
        Some(OwnedEntry {
            id: self.idgen.new_id(),
            occurred_at: format!("{day}T00:00:00Z"),
            quantity: holding.quantity.clone(),
            unit_price: holding.purchase_price.clone(),
            fees: "0".to_string(),
            currency: effective_currency(holding, reference_currency),
            rationale: None,
        })
    }

    /// Record a **buy** against a holding (Story 6.3, FR39): validates, replays the ledger with
    /// the new row through the pure WAC derivation, and writes the row + the recomputed aggregate
    /// atomically. The currency is the holding's own (FR28) — stamped, never chosen. Guarded.
    #[allow(clippy::too_many_arguments)]
    pub fn record_buy_for(
        &mut self,
        holding_id: Uuid,
        date_input: &str,
        quantity: &str,
        unit_price: &str,
        fees: &str,
        rationale: &str,
        reference_currency: &str,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let holding = self.any_holding(holding_id)?;
        if holding.sold_at.is_some() {
            // Buying back is a NEW position (FR36 add rail), not a mutation of a retired one.
            return Err(MSG_SAVE_FAILED.to_string());
        }
        let (qty, price, fees) = validate_ledger_amounts(quantity, unit_price, fees)?;
        let now = self.clock.now();
        let occurred_at = normalize_event_date(date_input, &now.0)?;
        let rows = self.ledger_rows_strict(holding_id)?;
        let opening = self.opening_for(&holding, &rows, reference_currency);
        let rationale = rationale.trim();
        let entry = OwnedEntry {
            id: self.idgen.new_id(),
            occurred_at,
            quantity: qty,
            unit_price: price,
            fees,
            currency: effective_currency(&holding, reference_currency),
            rationale: (!rationale.is_empty()).then(|| rationale.to_string()),
        };

        let mut candidates: Vec<Candidate> =
            rows.iter().map(candidate_of).collect::<Result<_, _>>()?;
        if let Some(op) = &opening {
            candidates.push(candidate_of_owned(op, LedgerEventKind::Buy, &now.0)?);
        }
        candidates.push(candidate_of_owned(&entry, LedgerEventKind::Buy, &now.0)?);
        let basis = replay(candidates)?;

        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .record_buy(
                holding_id,
                opening.as_ref().map(OwnedEntry::as_entry).as_ref(),
                &entry.as_entry(),
                &basis.quantity.normalize().to_string(),
                &basis.avg_cost.normalize().to_string(),
                &now,
            )
            .map(|_| ())
            .map_err(watch_error)
    }

    /// Record a **sell** (Story 4.7 whole-position / Story 6.3 partial, FR46/FR47/FR39). An empty
    /// `quantity_input` sells the whole position; otherwise the quantity is validated and the
    /// ledger replayed with the sell applied — selling more than is held at that point refuses
    /// neutrally ([`MSG_LEDGER_OVERSELL`]). `unit_price` = the matched study's `current_price`
    /// (the market fact, Story 4.4) if known, else the holding's weighted-average cost; `fees` = 0
    /// (an explicit fee rides a recorded buy or a later edit); `currency` = the **holding's own**
    /// (Story 6.2, FR38/FR28), the caller's `reference_currency` only as the legacy-NULL coalesce
    /// fallback. A sell that empties the position retires the holding atomically (the 4.7
    /// `sold_at` soft delete — the row keeps its FK referent); a partial sell leaves it in the
    /// register with the reduced quantity and an unchanged average cost (Appendix A). Returns the
    /// matching notice ([`MSG_HOLDING_SOLD`] / [`MSG_LEDGER_PARTIAL_SOLD`]). Guarded.
    pub fn sell_holding(
        &mut self,
        holding_id: Uuid,
        quantity_input: &str,
        rationale: &str,
        reference_currency: &str,
    ) -> Result<&'static str, String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        // Selling is an action on an ACTIVE position (a retired holding re-enters via a new add).
        let holding = self
            .list_holdings()
            .into_iter()
            .find(|h| h.id == holding_id)
            .ok_or(MSG_SAVE_FAILED.to_string())?;
        let quantity_input = quantity_input.trim();
        let quantity = if quantity_input.is_empty() {
            holding.quantity.clone()
        } else {
            Decimal::from_str_exact(quantity_input)
                .ok()
                .filter(|q| q.is_sign_positive() && !q.is_zero())
                .ok_or(MSG_HOLDING_INVALID_NUMBER.to_string())?
                .to_string()
        };
        // The sale price: the matched study's current market price if known, else the cost basis.
        let unit_price = self
            .study_id_for_ticker(&holding.security_ticker)
            .and_then(|sid| self.get_study(sid))
            .and_then(|s| s.judgment.current_price)
            .map(|m| m.as_decimal().to_string())
            .unwrap_or_else(|| holding.purchase_price.clone());
        let now = self.clock.now();
        // Date-granular like every 6.3 event (midnight UTC) — a same-day buy/sell pair replays by
        // insertion order (`created_at`), not by which path stamped a wall-clock time.
        let occurred_at = normalize_event_date("", &now.0)?;
        let rows = self.ledger_rows_strict(holding_id)?;
        let opening = self.opening_for(&holding, &rows, reference_currency);
        let rationale = rationale.trim();
        let entry = OwnedEntry {
            id: self.idgen.new_id(),
            occurred_at,
            quantity,
            unit_price,
            fees: "0".to_string(),
            currency: effective_currency(&holding, reference_currency),
            rationale: (!rationale.is_empty()).then(|| rationale.to_string()),
        };
        self.apply_sell(holding_id, opening, entry, &rows, &now)
    }

    /// Record a **sell from the ledger form** (Story 6.3 review decision, FR39 to the letter):
    /// unlike the trigger-panel [`Self::sell_holding`], every field is explicit — the event date,
    /// the quantity, the **unit price the user actually obtained**, the fees and the rationale.
    /// Same replay, same refusals, same retire-on-empty semantics; the currency stays the
    /// holding's own (FR28).
    #[allow(clippy::too_many_arguments)]
    pub fn record_sell_for(
        &mut self,
        holding_id: Uuid,
        date_input: &str,
        quantity: &str,
        unit_price: &str,
        fees: &str,
        rationale: &str,
        reference_currency: &str,
    ) -> Result<&'static str, String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let holding = self
            .list_holdings()
            .into_iter()
            .find(|h| h.id == holding_id)
            .ok_or(MSG_SAVE_FAILED.to_string())?;
        let (qty, price, fees) = validate_ledger_amounts(quantity, unit_price, fees)?;
        let now = self.clock.now();
        let occurred_at = normalize_event_date(date_input, &now.0)?;
        let rows = self.ledger_rows_strict(holding_id)?;
        let opening = self.opening_for(&holding, &rows, reference_currency);
        let rationale = rationale.trim();
        let entry = OwnedEntry {
            id: self.idgen.new_id(),
            occurred_at,
            quantity: qty,
            unit_price: price,
            fees,
            currency: effective_currency(&holding, reference_currency),
            rationale: (!rationale.is_empty()).then(|| rationale.to_string()),
        };
        self.apply_sell(holding_id, opening, entry, &rows, &now)
    }

    /// Shared sell tail: replay with the sell applied, write atomically, return the notice.
    fn apply_sell(
        &mut self,
        holding_id: Uuid,
        opening: Option<OwnedEntry>,
        entry: OwnedEntry,
        rows: &[TransactionItem],
        now: &steadyinvest_contract::Timestamp,
    ) -> Result<&'static str, String> {
        let mut candidates: Vec<Candidate> =
            rows.iter().map(candidate_of).collect::<Result<_, _>>()?;
        if let Some(op) = &opening {
            candidates.push(candidate_of_owned(op, LedgerEventKind::Buy, &now.0)?);
        }
        candidates.push(candidate_of_owned(&entry, LedgerEventKind::Sell, &now.0)?);
        let basis = replay(candidates)?;

        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .record_partial_sell(
                holding_id,
                opening.as_ref().map(OwnedEntry::as_entry).as_ref(),
                &entry.as_entry(),
                &basis.quantity.normalize().to_string(),
                now,
            )
            .map_err(watch_error)?;
        Ok(if basis.quantity.is_zero() {
            MSG_HOLDING_SOLD
        } else {
            MSG_LEDGER_PARTIAL_SOLD
        })
    }

    /// Edit a ledger row (Story 6.3, FR39): date, quantity, unit price, fees, rationale — never
    /// its `kind` (the row's identity) nor its `currency` (pinned to the holding's, FR28). The
    /// whole ledger is replayed with the edit applied; an impossible history (over-sell at any
    /// point) refuses neutrally. The recomputed aggregate — and the holding's retired state
    /// (`sold_at`), which the edit can flip in either direction — land atomically.
    #[allow(clippy::too_many_arguments)]
    pub fn update_transaction_for(
        &mut self,
        holding_id: Uuid,
        transaction_id: Uuid,
        date_input: &str,
        quantity: &str,
        unit_price: &str,
        fees: &str,
        rationale: &str,
        reference_currency: &str,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let holding = self.any_holding(holding_id)?;
        let (qty, price, fees) = validate_ledger_amounts(quantity, unit_price, fees)?;
        let now = self.clock.now();
        let normalized = normalize_event_date(date_input, &now.0)?;
        let rows = self.ledger_rows_strict(holding_id)?;
        let target = rows
            .iter()
            .find(|r| r.id == transaction_id)
            .ok_or(MSG_SAVE_FAILED.to_string())?
            .clone();
        // Preserve the stamp when the visible DATE is unchanged (2026-07-02 review): the edit form
        // carries only the day, so re-normalizing a legacy 4.7 wall-clock stamp to midnight would
        // silently reorder same-day history and break the C4 no-op guarantee for value-identical
        // edits. Only a genuinely changed date re-dates the row (to midnight, the 6.3 granularity).
        let occurred_at = if target.occurred_at.0.get(..10) == normalized.get(..10) {
            target.occurred_at.0.clone()
        } else {
            normalized
        };
        let opening = self.opening_for(&holding, &rows, reference_currency);

        let mut candidates = Vec::with_capacity(rows.len() + 1);
        for row in &rows {
            if row.id == transaction_id {
                let kind = match row.kind.as_deref() {
                    Some(KIND_BUY) => LedgerEventKind::Buy,
                    _ => LedgerEventKind::Sell,
                };
                let edited = OwnedEntry {
                    id: row.id,
                    occurred_at: occurred_at.clone(),
                    quantity: qty.clone(),
                    unit_price: price.clone(),
                    fees: fees.clone(),
                    currency: row.currency.clone(),
                    rationale: None,
                };
                candidates.push(candidate_of_owned(&edited, kind, &row.created_at.0)?);
            } else {
                candidates.push(candidate_of(row)?);
            }
        }
        if let Some(op) = &opening {
            candidates.push(candidate_of_owned(op, LedgerEventKind::Buy, &now.0)?);
        }
        let basis = replay(candidates)?;
        let retired_at = self.retired_at_for(&basis, &holding, &now.0);

        let rationale = rationale.trim();
        let rationale = (!rationale.is_empty()).then_some(rationale);
        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        let applied = journal
            .update_transaction(
                transaction_id,
                holding_id,
                opening.as_ref().map(OwnedEntry::as_entry).as_ref(),
                &occurred_at,
                &qty,
                &price,
                &fees,
                rationale,
                &basis.quantity.normalize().to_string(),
                &basis.avg_cost.normalize().to_string(),
                retired_at.as_deref(),
                &now,
            )
            .map_err(watch_error)?;
        if !applied {
            // The row vanished (or changed owner) between the read and the write — never report a
            // success the journal did not record (2026-07-02 review).
            return Err(MSG_SAVE_FAILED.to_string());
        }
        Ok(())
    }

    /// Delete a ledger row (Story 6.3, FR39): the remaining ledger is replayed and the aggregate
    /// rewritten atomically. Deleting the sell that retired a holding **un-retires** it (`sold_at`
    /// cleared, the position returns to the register with its restored quantity). Deleting the
    /// only buy row that later sells depend on refuses ([`MSG_LEDGER_OVERSELL`]) — the opening is
    /// materialized against the **current** rows, never re-seeded from the derived aggregate
    /// (2026-07-02 review, CRITICAL: re-seeding double-counted the surviving sells).
    pub fn delete_transaction_for(
        &mut self,
        holding_id: Uuid,
        transaction_id: Uuid,
        reference_currency: &str,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let holding = self.any_holding(holding_id)?;
        let rows = self.ledger_rows_strict(holding_id)?;
        if !rows.iter().any(|r| r.id == transaction_id) {
            return Err(MSG_SAVE_FAILED.to_string());
        }
        let now = self.clock.now();
        // Opening from the CURRENT rows (pre-delete): if a buy row exists, the aggregate is
        // derived and can never stand in for the opening again. (Deleting a 4.7-legacy holding's
        // only sell: current rows hold no buy → the opening materializes and restores the
        // position — the un-retire path.)
        let opening = self.opening_for(&holding, &rows, reference_currency);

        let mut candidates: Vec<Candidate> = rows
            .iter()
            .filter(|r| r.id != transaction_id)
            .map(candidate_of)
            .collect::<Result<_, _>>()?;
        if let Some(op) = &opening {
            candidates.push(candidate_of_owned(op, LedgerEventKind::Buy, &now.0)?);
        }
        let basis = replay(candidates)?;
        let retired_at = self.retired_at_for(&basis, &holding, &now.0);

        let journal = self.journal.as_mut().ok_or(MSG_NO_JOURNAL.to_string())?;
        let applied = journal
            .delete_transaction(
                transaction_id,
                holding_id,
                opening.as_ref().map(OwnedEntry::as_entry).as_ref(),
                &basis.quantity.normalize().to_string(),
                &basis.avg_cost.normalize().to_string(),
                retired_at.as_deref(),
                &now,
            )
            .map_err(watch_error)?;
        if !applied {
            // Vanished (or wrong-holding) row between read and write — never a false success.
            return Err(MSG_SAVE_FAILED.to_string());
        }
        Ok(())
    }

    /// The holding's retired state implied by a replayed position: an empty position keeps its
    /// existing `sold_at` (or stamps now when the mutation just emptied it); a non-empty position
    /// is active (`None` — un-retire).
    fn retired_at_for(
        &self,
        basis: &PositionBasis,
        holding: &HoldingItem,
        now: &str,
    ) -> Option<String> {
        if basis.quantity.is_zero() {
            Some(holding.sold_at.clone().unwrap_or_else(|| now.to_string()))
        } else {
            None
        }
    }
}

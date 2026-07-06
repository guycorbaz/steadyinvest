//! Holdings + portfolio wiring (Stories 4.3–4.7 + 6.1/6.2): the register re-render
//! (`refresh_holdings` — auto-matched study zone FR40, trailing stop FR42, neutral triggers
//! FR46/FR47, per-currency capital-at-risk FR43/FR38, portfolio selector FR37), the add / edit /
//! remove / sell / stop / dismiss / refresh-prices intents, the portfolio select / add / rename /
//! delete intents, and the transient per-ticker freshness machinery (Story 4.4 — display-time
//! only, never persisted). Moved verbatim from `main.rs` — no logic change.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use uuid::Uuid;

use crate::provider::ProviderChoice;
use crate::state::JournalState;
use crate::viewmodel::format::NumberFormat;
use crate::wiring::{Session, persist};
use crate::{
    BankCarRow, CapitalAtRiskRow, ConcentrationLine, HoldingRow, Holdings, LedgerRow, MainWindow,
    PortfolioRow, SizeMixLine, UnclassifiedLine,
};
use crate::{fetch, state, viewmodel};

/// Transient (NOT persisted) per-ticker price-refresh freshness for the holdings register (Story
/// 4.4, FR40): the outcome of the last manual refresh. Keyed by **upper-cased** ticker so it joins
/// the case-insensitive `study_id_for_ticker` match; rebuilt each session (display-time only — no
/// schema change). `as_of` is the display stamp of the last *successful* refresh.
#[derive(Clone, Default)]
pub(crate) struct HoldingFreshness {
    pub(crate) stale: bool,
    pub(crate) as_of: Option<String>,
}
pub(crate) type HoldingFreshnessMap = std::collections::HashMap<String, HoldingFreshness>;

/// Flag a holding's transient freshness `stale` (Story 4.4, AC4) after a failed / no-data refresh,
/// **preserving** the last successful `as_of` so the row still states when it was last fresh while
/// keeping its last-known zone visibly marked stale — never a fresh-looking wrong zone.
pub(crate) fn mark_holding_stale(freshness: &Rc<RefCell<HoldingFreshnessMap>>, key: &str) {
    let prev = freshness.borrow().get(key).and_then(|f| f.as_of.clone());
    freshness.borrow_mut().insert(
        key.to_string(),
        HoldingFreshness {
            stale: true,
            as_of: prev,
        },
    );
}

/// Drop transient freshness entries for tickers no longer held by ANY holding (issue #51). Called
/// after every holdings mutation: a removed (or ticker-edited-away) holding's stale `as_of` must not
/// resurface if that ticker is later re-added, and the map must not grow unbounded. Keyed by
/// upper-cased ticker — the same join as `study_id_for_ticker`. A ticker still held by a *sibling*
/// holding legitimately keeps its entry (the freshness is the ticker's, shared across its rows).
pub(crate) fn retain_held_freshness(
    freshness: &Rc<RefCell<HoldingFreshnessMap>>,
    state: &JournalState,
) {
    let held: std::collections::HashSet<String> = state
        .list_holdings()
        .iter()
        .map(|h| h.security_ticker.to_uppercase())
        .collect();
    freshness
        .borrow_mut()
        .retain(|ticker, _| held.contains(ticker));
}

/// Rebuild the holdings register from the journal (Story 4.3, FR36). Reference-currency labelling is
/// set separately (from app-config, on startup + on change) — a holdings mutation doesn't touch it.
/// Story 4.4 (FR40): each row also carries its auto-matched study's §4 zone (neutral key) + present
/// price + transient freshness, so the register shows Achat/Neutre/Vente + à jour/périmé per holding.
pub(crate) fn refresh_holdings(
    ui: &MainWindow,
    state: &JournalState,
    freshness: &HoldingFreshnessMap,
    dismissed: &std::collections::HashSet<String>,
    format: NumberFormat,
) {
    use steadyinvest_core::rounding::DisplayField;
    let holdings = ui.global::<Holdings>();
    let items = state.list_holdings();
    // Story 6.2 (FR38): the global reference currency is the fallback for a pre-6.2 holding whose own
    // currency is NULL (None) — the app coalesces None → reference at this read boundary.
    let reference_currency = holdings.get_reference_currency().to_string();
    let rows: Vec<HoldingRow> = items
        .iter()
        .map(|h| {
            // Auto-match the holding to the most-recent saved study of the same ticker AND currency
            // (issue #81 — a cross-currency study must not lend this row its price / stop); `None` →
            // a neutral "no linked study" row, never an error.
            let study = state
                .study_id_for_ticker_in_currency(&h.security_ticker, h.currency.as_deref())
                .and_then(|sid| state.get_study(sid));
            let f = freshness
                .get(&h.security_ticker.to_uppercase())
                .cloned()
                .unwrap_or_default();
            let (zone, current_price, study_link) = match &study {
                Some(s) => (
                    viewmodel::engine::zone_key(viewmodel::engine::study_zone(s)).to_string(),
                    s.judgment
                        .current_price
                        .map(|p| {
                            viewmodel::format::format_scaled(
                                p.as_decimal(),
                                DisplayField::Price,
                                format,
                            )
                        })
                        .unwrap_or_default(),
                    s.security_ticker.clone(),
                ),
                None => (String::new(), String::new(), String::new()),
            };
            // Story 4.5 (FR42): the trailing stop. `stop_breached` is a neutral fact — current price
            // ≤ the ratcheted level — computed only when both are known (no action; that's Story 4.7).
            let stop_level_dec = h
                .trailing_stop_level
                .as_deref()
                .and_then(|s| rust_decimal::Decimal::from_str_exact(s).ok());
            let current_price_dec = study
                .as_ref()
                .and_then(|s| s.judgment.current_price)
                .map(|m| m.as_decimal());
            let stop_breached = match (stop_level_dec, current_price_dec) {
                (Some(level), Some(price)) => steadyinvest_core::risk::stop_breached(level, price),
                _ => false,
            };
            let stop_level_display = stop_level_dec
                .map(|l| viewmodel::format::format_scaled(l, DisplayField::Price, format))
                .unwrap_or_default();
            // The margin above the stop (AC4 neutral fact) — only when a stop + price are known and
            // the price is NOT at/below the stop (a breach shows "◆ sous le stop" instead).
            let stop_distance = match (stop_level_dec, current_price_dec) {
                (Some(level), Some(price)) if price > level => {
                    viewmodel::format::format_scaled(price - level, DisplayField::Price, format)
                }
                _ => String::new(),
            };
            // Story 4.7 (FR46/FR47): the neutral trigger — the stop takes priority over the Sell
            // zone. The kind is computed in `core::risk` from the very same `stop_breached` + zone the
            // row already carries (no second source of truth); the per-row action panel is geofenced
            // to a still-shown (not dismissed) trigger.
            let trigger_kind =
                match steadyinvest_core::risk::trigger_state(stop_breached, zone == "sell") {
                    Some(steadyinvest_core::risk::TriggerKind::Stop) => "stop",
                    Some(steadyinvest_core::risk::TriggerKind::Sell) => "sell",
                    None => "",
                };
            let id_text = h.id.to_string();
            let dismissed = dismissed.contains(&id_text);
            // Story 6.2 (FR38): the holding's effective currency — its own, or the reference currency
            // for a pre-6.2 (None) row. Shown beside every amount; the register never mixes currencies.
            let currency = crate::state::effective_currency(h, &reference_currency);
            HoldingRow {
                id: id_text.into(),
                ticker: h.security_ticker.clone().into(),
                quantity: h.quantity.clone().into(),
                purchase_price: h.purchase_price.clone().into(),
                currency: currency.into(),
                linked: study.is_some(),
                study_link: study_link.into(),
                zone: zone.into(),
                current_price: current_price.into(),
                stale: f.stale,
                as_of: f.as_of.unwrap_or_default().into(),
                has_stop: h.trailing_stop_pct.is_some(),
                stop_pct: h.trailing_stop_pct.clone().unwrap_or_default().into(),
                stop_level: stop_level_display.into(),
                stop_breached,
                stop_distance: stop_distance.into(),
                trigger_kind: trigger_kind.into(),
                dismissed,
            }
        })
        .collect();
    holdings.set_holding_count(items.len() as i32);
    holdings.set_rows(ModelRc::new(VecModel::from(rows)));
    holdings.set_read_only(state.is_read_only());

    // Story 4.6 (FR43) / Story 6.2 (FR38): the portfolio capital-at-risk — now a **per-currency**
    // subtotal (the holdings can differ in currency, so a single mixed total is forbidden until FX
    // lands in Story 6.5). One row per currency: the figure + its share of that currency's invested
    // capital. Recomputed with the register (so a price refresh → stop ratchet → CaR all flow here).
    let car_rows: Vec<CapitalAtRiskRow> = state
        .portfolio_capital_at_risk_by_currency(&reference_currency)
        .into_iter()
        .map(|(currency, car, invested)| {
            let amount = viewmodel::format::format_scaled(car, DisplayField::Price, format);
            // Issue #61: the "% du capital investi" clause is shown ONLY when there IS capital at risk.
            // When CaR is 0 (no stop is set on any holding) the "0 %" reads as "no downside" — the
            // opposite of the truth (no *protection* is defined). The un-stopped-exposure line below
            // states that plainly instead.
            let pct = if car > rust_decimal::Decimal::ZERO && invested > rust_decimal::Decimal::ZERO
            {
                // Format the percent through the same locale-aware path as the figure (Percent = 1 dp).
                viewmodel::format::format_scaled(
                    car / invested * rust_decimal::Decimal::from(100),
                    DisplayField::Percent,
                    format,
                )
            } else {
                String::new()
            };
            CapitalAtRiskRow {
                currency: currency.into(),
                amount: amount.into(),
                pct: pct.into(),
            }
        })
        .collect();
    holdings.set_capital_at_risk_by_currency(ModelRc::new(VecModel::from(car_rows)));

    // Issue #61: the un-protected exposure — one neutral line per currency naming how many holdings
    // have NO trailing stop and their uncovered value. The honest complement to the CaR figure above
    // (a "0 % du capital investi" means no *protection* is set, not "no downside"). Empty → nothing shown.
    let unstopped_lines: Vec<SharedString> = state
        .portfolio_unstopped_exposure_by_currency(&reference_currency)
        .into_iter()
        .map(|(currency, count, value)| {
            let value = format!(
                "{} {}",
                viewmodel::format::format_scaled(value, DisplayField::Price, format),
                currency
            );
            state::unstopped_exposure_notice(count, &value).into()
        })
        .collect();
    holdings.set_unstopped_exposure(ModelRc::new(VecModel::from(unstopped_lines)));

    // Story 6.4 (FR41): the NET reinvestable dividend cash, per currency — includes SOLD holdings'
    // dividends (cash received is cash); recomputed with the register so every ledger mutation and
    // portfolio switch keeps the panel truthful.
    let cash_rows: Vec<CapitalAtRiskRow> = state
        .portfolio_reinvestable_cash_by_currency(&reference_currency)
        .into_iter()
        .map(|(currency, net)| CapitalAtRiskRow {
            currency: currency.into(),
            amount: viewmodel::format::format_scaled(net, DisplayField::Price, format).into(),
            pct: SharedString::new(),
        })
        .collect();
    holdings.set_reinvestable_cash(ModelRc::new(VecModel::from(cash_rows)));

    // Story 6.6 (FR44): the consolidation hierarchy — shown only when there is something to
    // consolidate (more than one bank, or any foreign-currency bucket); FX applied ONLY here.
    let consolidation = state.journal_capital_at_risk_consolidation(&reference_currency);
    let any_foreign = consolidation
        .banks
        .iter()
        .any(|b| b.buckets.iter().any(|(c, _, _)| c != &reference_currency));
    let show = consolidation.banks.len() > 1 || any_foreign;
    let pct_of = |car: rust_decimal::Decimal, invested: rust_decimal::Decimal| {
        if invested > rust_decimal::Decimal::ZERO {
            // Checked like every sum on this surface (review): an overflowing ratio shows
            // nothing rather than panicking the render path.
            car.checked_div(invested)
                .and_then(|r| r.checked_mul(rust_decimal::Decimal::from(100)))
                .map(|r| viewmodel::format::format_scaled(r, DisplayField::Percent, format))
                .unwrap_or_default()
        } else {
            String::new()
        }
    };
    let bank_rows: Vec<BankCarRow> = if show {
        consolidation
            .banks
            .iter()
            .map(|bank| match bank.converted {
                Some((car, invested)) => BankCarRow {
                    name: bank.name.clone().into(),
                    amount: format!(
                        "{} {}",
                        viewmodel::format::format_scaled(car, DisplayField::Price, format),
                        reference_currency
                    )
                    .into(),
                    pct: pct_of(car, invested).into(),
                    missing: SharedString::new(),
                    unavailable: false,
                },
                None => BankCarRow {
                    name: bank.name.clone().into(),
                    amount: SharedString::new(),
                    pct: SharedString::new(),
                    missing: bank.missing_pairs.join(" · ").into(),
                    unavailable: bank.unavailable,
                },
            })
            .collect()
    } else {
        Vec::new()
    };
    holdings.set_consolidation_banks(ModelRc::new(VecModel::from(bank_rows)));
    let (global_text, global_pct) = match consolidation.global.filter(|_| show) {
        Some((car, invested)) => (
            format!(
                "{} {}",
                viewmodel::format::format_scaled(car, DisplayField::Price, format),
                reference_currency
            ),
            pct_of(car, invested),
        ),
        None => (String::new(), String::new()),
    };
    holdings.set_consolidation_global(global_text.into());
    holdings.set_consolidation_global_pct(global_pct.into());
    holdings.set_consolidation_missing(if show {
        consolidation.missing_pairs.join(" · ").into()
    } else {
        SharedString::new()
    });
    // The FR28 footnote: every rate actually used, with its day and source.
    holdings.set_consolidation_rates(if show {
        consolidation
            .rates_used
            .iter()
            .map(|r| {
                // No prose baked into Rust (posture — the @tr scan cannot see it): the entry is
                // pure data — pair, rate, then "(date, source)".
                format!(
                    "{} → {} {} ({}, {})",
                    r.base_currency, r.quote_currency, r.rate, r.rate_date, r.source
                )
            })
            .collect::<Vec<_>>()
            .join(" · ")
            .into()
    } else {
        SharedString::new()
    });

    // Story 6.7 (FR45): concentration against the TOTAL invested capital + the diversify-by-size
    // mix. The threshold/boundary mirrors are canonical validated strings pushed from app-config
    // (startup + Réglages change) — parsed here with the pinned defaults as the safety net (they
    // are Rust-written, so the fallback is unreachable through the UI).
    let parse_or = |s: String, default: &str| {
        rust_decimal::Decimal::from_str_exact(&s)
            .or_else(|_| rust_decimal::Decimal::from_str_exact(default))
            .unwrap_or_default()
    };
    let small_max = parse_or(
        holdings.get_size_small_max().to_string(),
        crate::config::DEFAULT_SIZE_SMALL_MAX,
    );
    let medium_max = parse_or(
        holdings.get_size_medium_max().to_string(),
        crate::config::DEFAULT_SIZE_MEDIUM_MAX,
    );
    let threshold = parse_or(
        holdings.get_concentration_threshold_pct().to_string(),
        crate::config::DEFAULT_CONCENTRATION_THRESHOLD_PCT,
    );
    let div = state.journal_diversification(&reference_currency, small_max, medium_max);
    // The threshold and the targets render BESIDE locale-formatted shares — same locale path
    // (2026-07-03 review: never two decimal conventions in one sentence). The canonical config
    // strings stay in their mirror properties; these are the display spellings.
    let fmt_pct = |d: rust_decimal::Decimal| {
        viewmodel::format::format_scaled(d, DisplayField::Percent, format)
    };
    holdings.set_concentration_threshold_display(fmt_pct(threshold).into());
    let fmt_target = |raw: slint::SharedString| -> SharedString {
        rust_decimal::Decimal::from_str_exact(&raw)
            .map(&fmt_pct)
            .unwrap_or_else(|_| raw.to_string())
            .into()
    };
    // Hidden when there is nothing held anywhere (rows empty, read fine) — an empty portfolio
    // needs no concentration facts.
    let show_div = !div.rows.is_empty() || div.unavailable;
    let conc_rows: Vec<ConcentrationLine> = if show_div {
        div.rows
            .iter()
            .map(|r| {
                if !r.missing_pairs.is_empty() {
                    return ConcentrationLine {
                        ticker: r.ticker.clone().into(),
                        amount: SharedString::new(),
                        share: SharedString::new(),
                        flagged: false,
                        missing: r.missing_pairs.join(" · ").into(),
                    };
                }
                match r.invested {
                    Some(invested) => {
                        let share = r.share_pct.map(|s| {
                            viewmodel::format::format_scaled(s, DisplayField::Percent, format)
                        });
                        // The FR45 murmur: near/above the configured majority share — a neutral
                        // fact stated in the line's own words, never a hue (colour budget).
                        let flagged = r
                            .share_pct
                            .map(|s| steadyinvest_core::risk::concentration_flagged(s, threshold))
                            .unwrap_or(false);
                        ConcentrationLine {
                            ticker: r.ticker.clone().into(),
                            amount: format!(
                                "{} {}",
                                viewmodel::format::format_scaled(
                                    invested,
                                    DisplayField::Price,
                                    format
                                ),
                                reference_currency
                            )
                            .into(),
                            share: share.unwrap_or_default().into(),
                            flagged,
                            missing: SharedString::new(),
                        }
                    }
                    // Absent for a non-nameable reason (checked overflow) — a plain
                    // « indisponible » line, never a dangling empty amount (the 6.6 rule).
                    None => ConcentrationLine {
                        ticker: r.ticker.clone().into(),
                        amount: SharedString::new(),
                        share: SharedString::new(),
                        flagged: false,
                        missing: SharedString::new(),
                    },
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    holdings.set_concentration_rows(ModelRc::new(VecModel::from(conc_rows)));
    holdings.set_concentration_unavailable(div.unavailable);
    // Also fires on a PRESENT nonpositive total (all-zero-cost positions): the shares are
    // undefined then, and their absence must state itself (2026-07-03 review), never render
    // as rows that silently lack their percentages.
    let global_positive = div
        .global_invested
        .map(|g| g > rust_decimal::Decimal::ZERO)
        .unwrap_or(false);
    holdings.set_concentration_global_absent(show_div && !div.unavailable && !global_positive);
    holdings.set_concentration_missing(if show_div {
        div.missing_pairs.join(" · ").into()
    } else {
        SharedString::new()
    });
    let mix_rows: Vec<SizeMixLine> = if show_div && !div.unavailable {
        [
            ("small", &div.small, holdings.get_size_target_small_pct()),
            ("medium", &div.medium, holdings.get_size_target_medium_pct()),
            ("large", &div.large, holdings.get_size_target_large_pct()),
        ]
        .into_iter()
        .map(|(key, slot, target)| SizeMixLine {
            class_key: key.into(),
            share: slot.share_pct.map(&fmt_pct).unwrap_or_default().into(),
            target: fmt_target(target),
        })
        .collect()
    } else {
        Vec::new()
    };
    holdings.set_size_mix_rows(ModelRc::new(VecModel::from(mix_rows)));
    let unclassified_rows: Vec<UnclassifiedLine> = if show_div {
        div.unclassified
            .iter()
            .map(|u| {
                let (reason_key, missing) = match &u.reason {
                    state::UnclassifiedReason::NoStudy => ("no-study", SharedString::new()),
                    state::UnclassifiedReason::NoSales => ("no-sales", SharedString::new()),
                    state::UnclassifiedReason::MissingRate(pair) => {
                        ("missing-rate", pair.clone().into())
                    }
                    state::UnclassifiedReason::Unconvertible => {
                        ("unconvertible", SharedString::new())
                    }
                };
                UnclassifiedLine {
                    ticker: u.ticker.clone().into(),
                    reason_key: reason_key.into(),
                    missing,
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    holdings.set_size_unclassified_rows(ModelRc::new(VecModel::from(unclassified_rows)));
    // The FR28 footnote — pure data entries, no prose baked into Rust (posture).
    holdings.set_concentration_rates(if show_div {
        div.rates_used
            .iter()
            .map(|r| {
                format!(
                    "{} → {} {} ({}, {})",
                    r.base_currency, r.quote_currency, r.rate, r.rate_date, r.source
                )
            })
            .collect::<Vec<_>>()
            .join(" · ")
            .into()
    } else {
        SharedString::new()
    });

    // Story 6.1 (FR37): the portfolio selector + the active id (the register above is the active
    // portfolio's holdings). Pushed here so every holdings re-render keeps the selector in sync.
    let portfolios: Vec<PortfolioRow> = state
        .list_portfolios()
        .iter()
        .map(|p| PortfolioRow {
            id: p.id.to_string().into(),
            name: p.name.clone().into(),
        })
        .collect();
    holdings.set_portfolios(ModelRc::new(VecModel::from(portfolios)));
    holdings.set_active_portfolio_id(
        state
            .active_portfolio_id()
            .map(|id| id.to_string())
            .unwrap_or_default()
            .into(),
    );

    // Story 6.8 (FR48): keep an OPEN candidates panel truthful across every holdings/ledger/FX
    // mutation that re-renders the register (a closed panel costs nothing).
    crate::wiring::replacement::sync_candidates(ui, state);
}

/// Push one holding's transaction ledger (Story 6.3, FR39) into the `Holdings` global and mark it
/// as the opened one. Rows are the exact canonical TEXT spellings (no display rounding — the
/// ledger IS the record); the date shows the event day (`occurred_at`'s date part); a `NULL`
/// legacy `kind` renders as a sell (the only pre-6.3 writer).
pub(crate) fn push_ledger(ui: &MainWindow, state: &JournalState, holding_id: Uuid) {
    let rows: Vec<LedgerRow> = state
        .holding_ledger(holding_id)
        .iter()
        .map(|t| LedgerRow {
            id: t.id.to_string().into(),
            // A malformed/short stored stamp falls back to the FULL string (review: an empty date
            // cell would round-trip through "Modifier" as an empty — i.e. today's — date).
            date: t.occurred_at.0.get(..10).unwrap_or(&t.occurred_at.0).into(),
            kind: t.kind.clone().unwrap_or_else(|| "sell".to_string()).into(),
            quantity: t.quantity.clone().into(),
            unit_price: t.unit_price.clone().into(),
            fees: t.fees.clone().into(),
            currency: t.currency.clone().into(),
            rationale: t.rationale.clone().unwrap_or_default().into(),
            // Story 6.4 (FR41): a dividend row also shows its NET (gross − retenue); "" elsewhere
            // (and on an unparseable row — display only, never a hard error).
            net: if t.kind.as_deref() == Some(steadyinvest_persistence::KIND_DIVIDEND) {
                (|| {
                    let q = rust_decimal::Decimal::from_str_exact(&t.quantity).ok()?;
                    let p = rust_decimal::Decimal::from_str_exact(&t.unit_price).ok()?;
                    let f = rust_decimal::Decimal::from_str_exact(&t.fees).ok()?;
                    let net = q.checked_mul(p)?.checked_sub(f)?;
                    // Cash received is never negative — an invalid (imported/legacy) row shows
                    // no net rather than a nonsense figure (2026-07-02 review).
                    (!net.is_sign_negative()).then(|| net.normalize().to_string())
                })()
                .unwrap_or_default()
                .into()
            } else {
                SharedString::new()
            },
        })
        .collect();
    let holdings = ui.global::<Holdings>();
    holdings.set_ledger_rows(ModelRc::new(VecModel::from(rows)));
    holdings.set_ledger_holding_id(holding_id.to_string().into());
    // Issue #86: any pending delete-confirm belongs to the previous view of the ledger — clear it on
    // (re)open so the overlay never lingers over a different holding / after the row it targeted is gone.
    holdings.set_ledger_delete_confirm_visible(false);
    holdings.set_ledger_delete_pending_id(SharedString::new());
}

/// Re-sync the ledger panel after a mutation (2026-07-02 review): re-push the rows while the
/// holding is still in the active register; CLEAR the two globals when the mutation retired it
/// (the row — and the panel inside it — left the register; stale globals must not resurface on
/// the next render).
pub(crate) fn sync_ledger_panel(ui: &MainWindow, state: &JournalState, holding_id: Uuid) {
    let open_for = ui.global::<Holdings>().get_ledger_holding_id();
    if open_for.as_str() != holding_id.to_string() {
        return;
    }
    if state.list_holdings().iter().any(|h| h.id == holding_id) {
        push_ledger(ui, state, holding_id);
    } else {
        let holdings = ui.global::<Holdings>();
        holdings.set_ledger_rows(ModelRc::new(VecModel::from(Vec::<LedgerRow>::new())));
        holdings.set_ledger_holding_id(SharedString::new());
    }
}

/// Surface a holdings write's outcome (neutral notice on refusal) and re-render the register.
fn apply_holdings_result(
    ui: &MainWindow,
    state: &JournalState,
    result: Result<(), String>,
    freshness: &HoldingFreshnessMap,
    dismissed: &std::collections::HashSet<String>,
    format: NumberFormat,
) {
    let holdings = ui.global::<Holdings>();
    match result {
        Ok(()) => holdings.set_notice(SharedString::new()),
        Err(message) => holdings.set_notice(message.into()),
    }
    refresh_holdings(ui, state, freshness, dismissed, format);
}

/// Wire the holdings + portfolio domain: the holding add / edit / remove / sell / trailing-stop /
/// dismiss-trigger intents, the manual price refresh (one worker job per unique linked ticker,
/// FR65 — user-initiated only), and the Story 6.1 portfolio select / add / rename / delete.
pub(crate) fn wire_holdings(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        config_path,
        holding_freshness,
        holding_dismissed,
        fetch_tx,
        refresh_pending,
        refresh_total,
        fetch_cancel,
        ..
    } = s;
    // ── Holdings intents (Story 4.3, FR36) ── add / edit / remove a holding, each validated +
    // persisted then re-rendered with a neutral notice on refusal (invalid number / empty symbol /
    // read-only / no journal). Amounts are in the global reference currency (no FX in Epic 4).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Holdings>()
            .on_add_holding(move |ticker, quantity, price, currency| {
                let ui = ui_weak.unwrap();
                let result = journal_state
                    .borrow_mut()
                    .add_holding(&ticker, &quantity, &price, &currency);
                let written = result.is_ok();
                let format = config.borrow().number_format;
                retain_held_freshness(&holding_freshness, &journal_state.borrow());
                apply_holdings_result(
                    &ui,
                    &journal_state.borrow(),
                    result,
                    &holding_freshness.borrow(),
                    &holding_dismissed.borrow(),
                    format,
                );
                // Report whether the holding was written so the UI keeps the user's input on refusal.
                written
            });
    }

    // ── Story 6.1 (FR37): multiple-portfolio intents — select / add / rename / delete. The active
    // portfolio id is persisted to AppConfig (ADD7, outside the journal); each refresh re-renders the
    // selector + the active portfolio's register. The deletes are guarded (neutral notices). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let config_path = config_path.clone();
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        let js_for_change = Rc::clone(&journal_state);
        let on_portfolio_change = move |result: Result<(), String>| {
            let ui = ui_weak.unwrap();
            // Persist the (possibly changed) active portfolio id outside the journal.
            let active = js_for_change
                .borrow()
                .active_portfolio_id()
                .map(|id| id.to_string());
            config.borrow_mut().active_portfolio_id = active;
            persist(config_path.as_ref(), &config.borrow());
            let format = config.borrow().number_format;
            apply_holdings_result(
                &ui,
                &js_for_change.borrow(),
                result,
                &holding_freshness.borrow(),
                &holding_dismissed.borrow(),
                format,
            );
        };
        let h = ui.global::<Holdings>();
        {
            let journal_state = Rc::clone(&journal_state);
            let on_change = on_portfolio_change.clone();
            h.on_select_portfolio(move |id| {
                if let Ok(id) = Uuid::parse_str(&id) {
                    journal_state.borrow_mut().set_active_portfolio(id);
                }
                on_change(Ok(()));
            });
        }
        {
            let journal_state = Rc::clone(&journal_state);
            let on_change = on_portfolio_change.clone();
            h.on_add_portfolio(move |name| {
                let result = journal_state.borrow_mut().add_portfolio(&name).map(|_| ());
                on_change(result);
            });
        }
        {
            let journal_state = Rc::clone(&journal_state);
            let on_change = on_portfolio_change.clone();
            h.on_rename_portfolio(move |id, name| {
                let result = match Uuid::parse_str(&id) {
                    Ok(id) => journal_state.borrow_mut().rename_portfolio(id, &name),
                    Err(_) => Ok(()),
                };
                on_change(result);
            });
        }
        {
            let journal_state = Rc::clone(&journal_state);
            let on_change = on_portfolio_change;
            h.on_delete_portfolio(move |id| {
                let result = match Uuid::parse_str(&id) {
                    Ok(id) => journal_state.borrow_mut().delete_portfolio(id),
                    Err(_) => Ok(()),
                };
                on_change(result);
            });
        }
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Holdings>()
            .on_edit_holding(move |id, ticker, quantity, price, currency| {
                let ui = ui_weak.unwrap();
                let Ok(id) = Uuid::parse_str(&id) else {
                    return false;
                };
                let result = journal_state
                    .borrow_mut()
                    .update_holding(id, &ticker, &quantity, &price, &currency);
                let written = result.is_ok();
                let format = config.borrow().number_format;
                retain_held_freshness(&holding_freshness, &journal_state.borrow());
                apply_holdings_result(
                    &ui,
                    &journal_state.borrow(),
                    result,
                    &holding_freshness.borrow(),
                    &holding_dismissed.borrow(),
                    format,
                );
                written
            });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Holdings>().on_remove_holding(move |id| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id) else {
                return;
            };
            let result = journal_state.borrow_mut().delete_holding(id);
            // The holding is gone — drop any dismiss entry so the session set can't grow unbounded
            // (mirrors the sell path). `id.to_string()` is the same canonical key the rows use.
            holding_dismissed.borrow_mut().remove(&id.to_string());
            let format = config.borrow().number_format;
            retain_held_freshness(&holding_freshness, &journal_state.borrow());
            apply_holdings_result(
                &ui,
                &journal_state.borrow(),
                result,
                &holding_freshness.borrow(),
                &holding_dismissed.borrow(),
                format,
            );
        });
    }
    // ── Story 4.4 (FR40) — manual price refresh for every linked holding, off the UI thread. One
    // job per UNIQUE linked ticker (reusing the Epic-3 worker); holdings with no matching study are
    // skipped. Only ever user-initiated (FR65 — no background polling). Outcomes route to the
    // holdings surface via `WorkerOutcome::HoldingFetch` (the transient-freshness handler above). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let fetch_tx = fetch_tx.clone();
        let refresh_pending = Rc::clone(refresh_pending);
        let refresh_total = Rc::clone(refresh_total);
        let fetch_cancel = std::sync::Arc::clone(fetch_cancel);
        ui.global::<Holdings>().on_refresh_prices(move || {
            let ui = ui_weak.unwrap();
            let holdings = ui.global::<Holdings>();
            let jobs: Vec<(Uuid, String)> = {
                let state = journal_state.borrow();
                let mut seen = std::collections::HashSet::new();
                state
                    .list_holdings()
                    .into_iter()
                    .filter_map(|h| {
                        // Issue #81: the price-refresh target is the study in the holding's currency.
                        state
                            .study_id_for_ticker_in_currency(
                                &h.security_ticker,
                                h.currency.as_deref(),
                            )
                            .map(|sid| (sid, h.security_ticker))
                    })
                    .filter(|(_, ticker)| seen.insert(ticker.to_uppercase()))
                    .collect()
            };
            if jobs.is_empty() {
                holdings.set_notice(state::MSG_HOLDINGS_REFRESH_NONE.into());
                return;
            }
            // Story 6.9 (FR26): the PRICE fallback chain, resolved once and shared by every
            // per-ticker job of this batch (the worker paces them within the declared limits).
            let primary = config.borrow().preferred_provider;
            if primary == ProviderChoice::None {
                holdings.set_notice(state::MSG_PROVIDER_NONE.into());
                return;
            }
            let chain = crate::wiring::fetch::resolve_chain(
                &config.borrow(),
                steadyinvest_ingestion::FieldKind::Price,
            );
            if chain.is_empty() {
                holdings.set_notice(state::MSG_PROVIDER_NO_KEY.into());
                return;
            }
            // Issue #100: a fresh batch — clear any leftover cancel flag BEFORE sending jobs (the
            // worker checks it per job, so it must read false for the new batch). Safe: the button is
            // disabled while `refreshing`, so a prior batch has fully drained before we get here.
            fetch_cancel.store(false, std::sync::atomic::Ordering::Relaxed);
            // Count only jobs the worker actually accepted — if the worker is gone, don't latch
            // `refreshing` (which would disable the button for the rest of the session). (Issue #52.)
            let mut enqueued = 0usize;
            for (study_id, ticker) in jobs {
                if fetch_tx
                    .send(fetch::WorkerJob::RefreshHolding(fetch::FetchRequest {
                        study_id,
                        ticker,
                        chain: chain.clone(),
                        primary,
                    }))
                    .is_ok()
                {
                    enqueued += 1;
                }
            }
            if enqueued == 0 {
                return;
            }
            // Latch the in-flight state: the button is disabled while `refreshing` (no double-click
            // → no duplicate jobs). The outcome handler decrements the pending count and clears the
            // flag when the last job resolves. No race — outcomes are marshalled to THIS (UI) thread.
            *refresh_pending.borrow_mut() = enqueued;
            *refresh_total.borrow_mut() = enqueued; // issue #100: the batch size, for the n/t counter
            holdings.set_refreshing(true);
            holdings.set_refresh_progress(format!("0 / {enqueued}").into());
            holdings.set_notice(state::MSG_HOLDINGS_REFRESHING.into());
        });
    }
    // ── Issue #100 — cancel the in-flight holdings price-refresh batch: raise the shared flag; the
    //    worker drains the remaining queued jobs (the in-flight request finishes) and the outcome
    //    handler clears the latch. ──
    {
        let fetch_cancel = std::sync::Arc::clone(fetch_cancel);
        ui.global::<Holdings>().on_cancel_refresh(move || {
            fetch_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        });
    }
    // ── Story 4.5 (FR42) — set / clear a holding's trailing-stop percentage. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Holdings>()
            .on_set_trailing_stop(move |id, pct| {
                let ui = ui_weak.unwrap();
                let Ok(id) = Uuid::parse_str(&id) else {
                    return;
                };
                let result = journal_state
                    .borrow_mut()
                    .set_holding_trailing_stop(id, &pct);
                let format = config.borrow().number_format;
                apply_holdings_result(
                    &ui,
                    &journal_state.borrow(),
                    result,
                    &holding_freshness.borrow(),
                    &holding_dismissed.borrow(),
                    format,
                );
            });
    }
    // ── Story 4.7 (FR46/FR47) — record a sell on a neutral trigger / dismiss a trigger's panel. The
    // app never auto-acts: it only persists a sell the user explicitly chose, or hides a trigger. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Holdings>()
            .on_sell_holding(move |id, quantity, rationale| {
                let ui = ui_weak.unwrap();
                let Ok(uuid) = Uuid::parse_str(&id) else {
                    return;
                };
                // Story 6.8 (FR48): capture the ticker BEFORE the sell — a whole-position sell
                // retires the row, and the candidates panel is headed by the sold ticker.
                let sold_ticker = journal_state
                    .borrow()
                    .list_holdings()
                    .iter()
                    .find(|h| h.id == uuid)
                    .map(|h| h.security_ticker.clone());
                // The reference currency is only the coalesce fallback for a pre-6.2 legacy row —
                // `sell_holding` stamps the transaction with the holding's OWN currency (FR28).
                let reference = config.borrow().reference_currency_or_default();
                // Story 6.3 (FR39): an empty quantity sells the whole position (the 4.7 flow);
                // otherwise it is a PARTIAL sell and the holding stays in the register.
                let result = journal_state
                    .borrow_mut()
                    .sell_holding(uuid, &quantity, &rationale, &reference);
                // Drop the stale dismiss entry only when the position actually LEFT the register
                // (a whole-position sell) — a partial sell keeps the row, and its dismissed
                // trigger must stay dismissed (2026-07-02 review).
                if result == Ok(state::MSG_HOLDING_SOLD) {
                    holding_dismissed.borrow_mut().remove(id.as_str());
                }
                let format = config.borrow().number_format;
                retain_held_freshness(&holding_freshness, &journal_state.borrow());
                // A neutral confirmation on success (full vs partial — sell_holding says which);
                // the guarded refusal notice otherwise.
                let result = result.map(|notice| {
                    ui.global::<Holdings>().set_notice(notice.into());
                });
                let sold = result.is_ok();
                if sold {
                    refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                } else {
                    apply_holdings_result(
                        &ui,
                        &journal_state.borrow(),
                        result,
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                }
                // Keep an open ledger panel truthful (review): re-push after a partial sell,
                // clear the globals when the sell retired the holding.
                sync_ledger_panel(&ui, &journal_state.borrow(), uuid);
                // Story 6.8 (FR48): a recorded sell (whole OR partial — freed capital is freed
                // capital) surfaces the replacement candidates, headed by the sold ticker.
                if sold && let Some(ticker) = sold_ticker {
                    crate::wiring::replacement::open_candidates(
                        &ui,
                        &journal_state.borrow(),
                        &ticker,
                        true,
                    );
                }
            });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Holdings>().on_dismiss_trigger(move |id| {
            let ui = ui_weak.unwrap();
            holding_dismissed.borrow_mut().insert(id.to_string());
            let format = config.borrow().number_format;
            refresh_holdings(
                &ui,
                &journal_state.borrow(),
                &holding_freshness.borrow(),
                &holding_dismissed.borrow(),
                format,
            );
        });
    }
    // ── Story 6.3 (FR39): the transaction ledger — open/close the per-holding view, record a buy,
    // edit/delete a row. Every mutation replays the ledger through the pure core derivation and
    // re-renders BOTH the ledger and the register (the aggregate — and possibly the retired state —
    // changed). Neutral notices throughout (FR13). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Holdings>().on_open_ledger(move |id| {
            let ui = ui_weak.unwrap();
            let Ok(uuid) = Uuid::parse_str(&id) else {
                return;
            };
            push_ledger(&ui, &journal_state.borrow(), uuid);
        });
    }
    {
        let ui_weak = ui.as_weak();
        ui.global::<Holdings>().on_close_ledger(move || {
            let ui = ui_weak.unwrap();
            let holdings = ui.global::<Holdings>();
            holdings.set_ledger_rows(ModelRc::new(VecModel::from(Vec::<LedgerRow>::new())));
            holdings.set_ledger_holding_id(SharedString::new());
            // Issue #86: close the panel → drop any pending delete-confirm too.
            holdings.set_ledger_delete_confirm_visible(false);
            holdings.set_ledger_delete_pending_id(SharedString::new());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Holdings>()
            .on_record_buy(move |id, date, quantity, price, fees, rationale| {
                let ui = ui_weak.unwrap();
                let Ok(uuid) = Uuid::parse_str(&id) else {
                    return false;
                };
                let reference = config.borrow().reference_currency_or_default();
                let result = journal_state.borrow_mut().record_buy_for(
                    uuid, &date, &quantity, &price, &fees, &rationale, &reference,
                );
                let written = result.is_ok();
                let format = config.borrow().number_format;
                let result = result.map(|()| {
                    ui.global::<Holdings>()
                        .set_notice(state::MSG_LEDGER_BUY_RECORDED.into());
                });
                if result.is_ok() {
                    refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                } else {
                    apply_holdings_result(
                        &ui,
                        &journal_state.borrow(),
                        result,
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                }
                sync_ledger_panel(&ui, &journal_state.borrow(), uuid);
                // Report whether the buy was written so the form keeps the user's input on refusal.
                written
            });
    }
    // Story 6.3 review decision (FR39 to the letter): an ordinary sell from the ledger form, every
    // field explicit — date, quantity, the unit price actually obtained, fees, rationale. The
    // trigger-panel sell (above) keeps its 4.7 shape.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Holdings>().on_record_sell(
            move |id, date, quantity, price, fees, rationale| {
                let ui = ui_weak.unwrap();
                let Ok(uuid) = Uuid::parse_str(&id) else {
                    return false;
                };
                // Story 6.8 (FR48): the ticker BEFORE the sell (a whole sell retires the row).
                let sold_ticker = journal_state
                    .borrow()
                    .list_holdings()
                    .iter()
                    .find(|h| h.id == uuid)
                    .map(|h| h.security_ticker.clone());
                let reference = config.borrow().reference_currency_or_default();
                let result = journal_state.borrow_mut().record_sell_for(
                    uuid, &date, &quantity, &price, &fees, &rationale, &reference,
                );
                let written = result.is_ok();
                if result == Ok(state::MSG_HOLDING_SOLD) {
                    holding_dismissed.borrow_mut().remove(id.as_str());
                }
                let format = config.borrow().number_format;
                retain_held_freshness(&holding_freshness, &journal_state.borrow());
                let result = result.map(|notice| {
                    ui.global::<Holdings>().set_notice(notice.into());
                });
                if result.is_ok() {
                    refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                } else {
                    apply_holdings_result(
                        &ui,
                        &journal_state.borrow(),
                        result,
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                }
                sync_ledger_panel(&ui, &journal_state.borrow(), uuid);
                // Story 6.8 (FR48): a ledger-form sell surfaces the candidates too (whole or
                // partial — the same redeployment moment as the trigger-panel sell).
                if written && let Some(ticker) = sold_ticker {
                    crate::wiring::replacement::open_candidates(
                        &ui,
                        &journal_state.borrow(),
                        &ticker,
                        true,
                    );
                }
                written
            },
        );
    }
    // Story 6.4 (FR41): a dividend from the ledger form — quantity = shares paid on, the price
    // field = GROSS per share, the fees field = the withholding ("" auto-computes at the Réglages
    // default rate). The register re-render refreshes the reinvestable-cash panel.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Holdings>().on_record_dividend(
            move |id, date, quantity, gross_per_share, withholding, rationale| {
                let ui = ui_weak.unwrap();
                let Ok(uuid) = Uuid::parse_str(&id) else {
                    return false;
                };
                let (reference, rate) = {
                    let cfg = config.borrow();
                    (
                        cfg.reference_currency_or_default(),
                        cfg.withholding_rate_pct_or_default(),
                    )
                };
                let result = journal_state.borrow_mut().record_dividend_for(
                    uuid,
                    &date,
                    &quantity,
                    &gross_per_share,
                    &withholding,
                    &rationale,
                    &reference,
                    &rate,
                );
                let written = result.is_ok();
                let format = config.borrow().number_format;
                let result = result.map(|()| {
                    ui.global::<Holdings>()
                        .set_notice(state::MSG_DIVIDEND_RECORDED.into());
                });
                if result.is_ok() {
                    refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                } else {
                    apply_holdings_result(
                        &ui,
                        &journal_state.borrow(),
                        result,
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                }
                sync_ledger_panel(&ui, &journal_state.borrow(), uuid);
                written
            },
        );
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Holdings>().on_update_transaction(
            move |txn_id, date, quantity, price, fees, rationale| {
                let ui = ui_weak.unwrap();
                let holding_id = ui.global::<Holdings>().get_ledger_holding_id();
                let (Ok(txn_uuid), Ok(holding_uuid)) =
                    (Uuid::parse_str(&txn_id), Uuid::parse_str(&holding_id))
                else {
                    return false;
                };
                let reference = config.borrow().reference_currency_or_default();
                let result = journal_state.borrow_mut().update_transaction_for(
                    holding_uuid,
                    txn_uuid,
                    &date,
                    &quantity,
                    &price,
                    &fees,
                    &rationale,
                    &reference,
                );
                let written = result.is_ok();
                let format = config.borrow().number_format;
                let result = result.map(|()| {
                    ui.global::<Holdings>()
                        .set_notice(state::MSG_LEDGER_UPDATED.into());
                });
                if result.is_ok() {
                    refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                } else {
                    apply_holdings_result(
                        &ui,
                        &journal_state.borrow(),
                        result,
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                }
                sync_ledger_panel(&ui, &journal_state.borrow(), holding_uuid);
                written
            },
        );
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let holding_freshness = Rc::clone(holding_freshness);
        let holding_dismissed = Rc::clone(holding_dismissed);
        ui.global::<Holdings>()
            .on_delete_transaction(move |txn_id| {
                let ui = ui_weak.unwrap();
                let holding_id = ui.global::<Holdings>().get_ledger_holding_id();
                let (Ok(txn_uuid), Ok(holding_uuid)) =
                    (Uuid::parse_str(&txn_id), Uuid::parse_str(&holding_id))
                else {
                    return;
                };
                let reference = config.borrow().reference_currency_or_default();
                let result = journal_state.borrow_mut().delete_transaction_for(
                    holding_uuid,
                    txn_uuid,
                    &reference,
                );
                let format = config.borrow().number_format;
                let result = result.map(|()| {
                    ui.global::<Holdings>()
                        .set_notice(state::MSG_LEDGER_DELETED.into());
                });
                if result.is_ok() {
                    refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                } else {
                    apply_holdings_result(
                        &ui,
                        &journal_state.borrow(),
                        result,
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                }
                sync_ledger_panel(&ui, &journal_state.borrow(), holding_uuid);
            });
    }
}

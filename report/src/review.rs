//! Story 7.2 (FR53) — the portfolio health review as a neutral, greyscale PDF, in the study PDF's
//! style. The `app` builds a [`PortfolioReview`] of already-formatted figures + enum-like keys; this
//! module owns EVERY label (its own neutral inventory, tested like `pdf.rs`'s) and lays them out:
//! page 1 = the header + the four répartition blocks + the concentration; page 2+ = the positions
//! table (header repeated across breaks), the studies due for review, the counts.

use crate::pdf::{Doc, EM_DASH, MARGIN, PAGE_W};

/// One line of a share block: a label (data — a sector, a currency, a bank, a ticker, or a size
/// key `small` | `medium` | `large`; which one is known from the FIELD the line sits in, never
/// from its value — a bank named « small » stays « small »), an amount, a share, a target, the
/// missing pair(s) blocking the figure, and a reason key (an unclassified security) — all
/// pre-formatted by the app, `""` when absent. The screen's row, field for field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShareLine {
    pub label: String,
    pub amount: String,
    pub share: String,
    pub target: String,
    /// The pair(s) blocking the figure (« EUR → CHF · USD → CHF »), `""` when none.
    pub missing: String,
    /// `""` | `no_study` | `study_unavailable` | `no_sales` | `missing_rate` | `unconvertible`
    /// — an unclassified size row (« non classé »); `missing_rate` names its pair in `missing`.
    pub reason: String,
    pub flagged: bool,
}

/// One held ticker's row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewLine {
    pub ticker: String,
    pub name: String,
    pub banks: String,
    pub invested: String,
    pub share: String,
    /// The position's currency (the other-currency cause names it).
    pub currency: String,
    /// The pair(s) blocking the invested figure, `""` when none.
    pub missing: String,
    /// `full` | `provisional` | `withheld` | `none` | `unavailable` | `not_computable`.
    pub study: String,
    /// `study == none`: a same-ticker study in ANOTHER currency (the cause), `""` when none.
    pub other_currency: String,
    pub low_confidence: bool,
    /// `buy` | `neutral` | `sell` | `below` | `above` | `""`.
    pub zone: String,
    pub ud: String,
    pub relative: String,
    /// The flags already worded by the app (its inventory), joined; `""` when none.
    pub flags: String,
    /// How many flags `flags` carries (« Signaux (n) », as on the screen).
    pub flag_count: usize,
    /// The study's present price WITH its currency (« 123,45 USD »), `""` when unknown.
    pub price: String,
    /// The study's last effective save (`YYYY-MM-DD`), `""` without a study.
    pub last_saved: String,
    /// The last save is unknown (the history read failed) — stated, never a dash.
    pub last_saved_unknown: bool,
    /// The lots do not all link to the same study: the distinct links' study currencies, joined
    /// (`"—"` for a lot without a readable study); `""` when every lot shares the study.
    pub mixed_links: String,
    /// `stale` | `fresh` | `""`, and the as-of date.
    pub data_state: String,
    pub as_of: String,
    /// Every lot's stop — level, currency and bank — joined (data, formatted by the app).
    pub stop: String,
    pub stop_breached: bool,
    /// The breached stop(s) among `stop` (G1 review: which level, which bank).
    pub stop_breached_levels: String,
    /// `stop` | `sell` | `""`.
    pub trigger: String,
}

/// A study due for review, with every reason that applies: `age` · `age_unknown` · `withheld`
/// · `low_confidence` · `not_computable`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DueLine {
    pub ticker: String,
    pub date: String,
    /// The last-save date is unknown (the history read failed) — stated, never a dash.
    pub date_unknown: bool,
    pub reasons: Vec<String>,
}

/// The review, ready to lay out — figures formatted by the app, labels owned here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortfolioReview {
    pub dossier: String,
    pub date: String,
    pub reference_currency: String,
    pub bank_count: String,
    pub position_count: String,
    pub linked_count: String,
    /// The FR28 footnote entries (data: pair, rate, date, source).
    pub rates: Vec<String>,
    pub size_lines: Vec<ShareLine>,
    pub unclassified: Vec<ShareLine>,
    /// The 6.7 read failed: the size block AND the concentration block are « indisponible »
    /// (their lines are then empty — never stale rows, never « Aucune donnée. »).
    pub diversification_unavailable: bool,
    pub sector_lines: Vec<ShareLine>,
    pub sectors_unavailable: bool,
    pub currency_lines: Vec<ShareLine>,
    pub currencies_unavailable: bool,
    pub bank_lines: Vec<ShareLine>,
    pub global_invested: String,
    /// The pair(s) that absent the global total, `""` when none.
    pub global_missing: String,
    /// The global total is absent for a reason that is NOT a nameable pair (a failed bank read,
    /// an overflow) — a plain « indisponible ».
    pub global_unavailable: bool,
    pub concentration: Vec<ShareLine>,
    /// The pair(s) that absent the concentration shares' denominator, `""` when none.
    pub concentration_missing: String,
    /// The shares' denominator is absent (or not positive) without a nameable pair.
    pub concentration_global_absent: bool,
    pub concentration_threshold: String,
    pub positions: Vec<ReviewLine>,
    pub due: Vec<DueLine>,
    /// `(count-key, value)` — keys: positions · linked · full · provisional · withheld ·
    /// not_computable · flagged · high_zone · stop_breached · due.
    pub counts: Vec<(String, String)>,
}

// ── the neutral inventory (FR13) — every static string this layout emits ──
const TITLE: &str = "Revue de santé du portefeuille";
const H_DOSSIER: &str = "Dossier";
const H_DATE: &str = "Date";
const H_CURRENCY: &str = "Monnaie de référence";
const H_BANKS: &str = "Banques";
const H_POSITIONS: &str = "Positions";
const H_LINKED: &str = "Études liées";
const RATES_NOTE: &str = "Convertis aux taux :";
const S_SIZE: &str = "1. Répartition par taille (chiffre d'affaires)";
const S_SECTOR: &str = "2. Répartition par secteur";
const S_CURRENCY: &str = "3. Répartition par devise";
const S_BANK: &str = "4. Répartition par banque";
const S_CONCENTRATION: &str = "5. Concentration par titre";
const S_POSITIONS: &str = "6. Positions et leurs études";
const S_DUE: &str = "7. Études à revoir";
const S_COUNTS: &str = "8. En chiffres";
const C_LABEL: &str = "Catégorie";
const C_AMOUNT: &str = "Montant";
const C_SHARE: &str = "Part";
const C_TARGET: &str = "Cible";
const C_NOTE: &str = "Remarque";
const SIZE_SMALL: &str = "Petite";
const SIZE_MEDIUM: &str = "Moyenne";
const SIZE_LARGE: &str = "Grande";
const SECTOR_UNLABELED: &str = "non renseigné";
const GLOBAL_TOTAL: &str = "Total global";
const THRESHOLD_NOTE: &str = "seuil de concentration :";
const MURMUR: &str = "seuil approché ou atteint";
const MURMUR_REACHED: &str = "seuil atteint ou dépassé";
const UNAVAILABLE: &str = "indisponible";
const SIZE_UNAVAILABLE: &str = "Répartition par taille indisponible.";
const SECTORS_UNAVAILABLE: &str = "Exposition par secteur indisponible.";
const CURRENCIES_UNAVAILABLE: &str = "Exposition par devise indisponible.";
const CONCENTRATION_UNAVAILABLE: &str = "Concentration indisponible.";
const GLOBAL_MISSING: &str = "Total global indisponible : taux manquant";
const GLOBAL_UNAVAILABLE: &str = "Total global indisponible.";
const SHARES_MISSING: &str = "Parts indisponibles : taux manquant";
const SHARES_UNAVAILABLE: &str = "Parts indisponibles.";
const AMOUNT_MISSING: &str = "montant indisponible : taux manquant";
const OTHER_CURRENCY_STUDY: &str = "aucune étude liée : étude en";
const OTHER_CURRENCY_POSITION: &str = "position en";
const REASON_NO_STUDY: &str = "non classé : aucune étude";
const REASON_STUDY_UNAVAILABLE: &str = "non classé : étude indisponible";
const REASON_NO_SALES: &str = "non classé : chiffre d'affaires indisponible";
const REASON_MISSING_RATE: &str = "non classé : taux manquant";
const REASON_UNCONVERTIBLE: &str = "non classé : conversion impossible";
const MISSING_RATE: &str = "taux manquant";
const P_TICKER: &str = "Titre";
const P_BANKS: &str = "Banques";
const P_INVESTED: &str = "Investi";
const P_SHARE: &str = "Part";
const P_STUDY: &str = "Étude";
const P_ZONE: &str = "Zone";
const P_UD: &str = "H/B";
const P_RELATIVE: &str = "Val. rel.";
const P_DATA: &str = "Données";
const STUDY_FULL: &str = "critères validés";
const STUDY_PROVISIONAL: &str = "provisoire";
const STUDY_WITHHELD: &str = "en attente";
const STUDY_NONE: &str = "aucune étude";
const STUDY_UNAVAILABLE: &str = "indisponible";
const STUDY_NOT_COMPUTABLE: &str = "non calculable";
const PRICE_LABEL: &str = "prix actuel :";
const LAST_SAVED_LABEL: &str = "dernière sauvegarde :";
const MIXED_LINKS: &str = "lots liés à des études différentes, études en";
const LOW_CONFIDENCE: &str = "confiance réduite";
const ZONE_BUY: &str = "basse";
const ZONE_NEUTRAL: &str = "médiane";
const ZONE_SELL: &str = "haute";
const ZONE_BELOW: &str = "sous la bande";
const ZONE_ABOVE: &str = "au-dessus de la bande";
const DATA_STALE: &str = "périmé";
const DATA_FRESH: &str = "à jour le";
const FLAGS_LABEL: &str = "Signaux";
const STOP_LABEL: &str = "Seuil suiveur :";
const STOP_BREACHED: &str = "sous le seuil";
const TRIGGER_STOP: &str = "Le prix a atteint le seuil suiveur.";
const TRIGGER_SELL: &str = "Le prix est dans la zone haute.";
const D_TICKER: &str = "Titre";
const D_DATE: &str = "Dernière sauvegarde";
const D_REASON: &str = "Motif";
const DUE_AGE: &str = "plus de 12 mois";
const DUE_WITHHELD: &str = "une donnée requise manque";
const DUE_LOW_CONFIDENCE: &str = "confiance réduite";
const DUE_AGE_UNKNOWN: &str = "ancienneté inconnue";
const DUE_NOT_COMPUTABLE: &str = "données non calculables";
const DUE_NONE: &str = "Aucune étude à revoir.";
const DUE_DATE_UNKNOWN: &str = "inconnue (historique indisponible)";
const K_POSITIONS: &str = "positions";
const K_LINKED: &str = "avec une étude";
const K_FULL: &str = "critères validés";
const K_PROVISIONAL: &str = "provisoires";
const K_WITHHELD: &str = "en attente";
const K_NOT_COMPUTABLE: &str = "non calculables";
const K_FLAGGED: &str = "avec au moins un signal";
const K_HIGH_ZONE: &str = "dans la zone haute ou au-dessus";
const K_STOP_BREACHED: &str = "sous leur seuil suiveur";
const K_DUE: &str = "à revoir";
const EMPTY_BLOCK: &str = "Aucune donnée.";

#[cfg(test)]
const REVIEW_USER_FACING: &[&str] = &[
    TITLE,
    H_DOSSIER,
    H_DATE,
    H_CURRENCY,
    H_BANKS,
    H_POSITIONS,
    H_LINKED,
    RATES_NOTE,
    S_SIZE,
    S_SECTOR,
    S_CURRENCY,
    S_BANK,
    S_CONCENTRATION,
    S_POSITIONS,
    S_DUE,
    S_COUNTS,
    C_LABEL,
    C_AMOUNT,
    C_SHARE,
    C_TARGET,
    C_NOTE,
    SIZE_SMALL,
    SIZE_MEDIUM,
    SIZE_LARGE,
    SECTOR_UNLABELED,
    GLOBAL_TOTAL,
    THRESHOLD_NOTE,
    MURMUR,
    MURMUR_REACHED,
    UNAVAILABLE,
    SIZE_UNAVAILABLE,
    SECTORS_UNAVAILABLE,
    CURRENCIES_UNAVAILABLE,
    CONCENTRATION_UNAVAILABLE,
    GLOBAL_MISSING,
    GLOBAL_UNAVAILABLE,
    SHARES_MISSING,
    SHARES_UNAVAILABLE,
    AMOUNT_MISSING,
    OTHER_CURRENCY_STUDY,
    OTHER_CURRENCY_POSITION,
    REASON_NO_STUDY,
    REASON_STUDY_UNAVAILABLE,
    REASON_NO_SALES,
    REASON_MISSING_RATE,
    REASON_UNCONVERTIBLE,
    MISSING_RATE,
    P_TICKER,
    P_BANKS,
    P_INVESTED,
    P_SHARE,
    P_STUDY,
    P_ZONE,
    P_UD,
    P_RELATIVE,
    P_DATA,
    STUDY_FULL,
    STUDY_PROVISIONAL,
    STUDY_WITHHELD,
    STUDY_NONE,
    STUDY_UNAVAILABLE,
    STUDY_NOT_COMPUTABLE,
    PRICE_LABEL,
    LAST_SAVED_LABEL,
    MIXED_LINKS,
    LOW_CONFIDENCE,
    ZONE_BUY,
    ZONE_NEUTRAL,
    ZONE_SELL,
    ZONE_BELOW,
    ZONE_ABOVE,
    DATA_STALE,
    DATA_FRESH,
    FLAGS_LABEL,
    STOP_LABEL,
    STOP_BREACHED,
    TRIGGER_STOP,
    TRIGGER_SELL,
    D_TICKER,
    D_DATE,
    D_REASON,
    DUE_AGE,
    DUE_WITHHELD,
    DUE_LOW_CONFIDENCE,
    DUE_AGE_UNKNOWN,
    DUE_NOT_COMPUTABLE,
    DUE_NONE,
    DUE_DATE_UNKNOWN,
    K_POSITIONS,
    K_LINKED,
    K_FULL,
    K_PROVISIONAL,
    K_WITHHELD,
    K_NOT_COMPUTABLE,
    K_FLAGGED,
    K_HIGH_ZONE,
    K_STOP_BREACHED,
    K_DUE,
    EMPTY_BLOCK,
];

// The share blocks: label · amount · share · target, then the note — prose, left-aligned and
// given the room (a note wraps inside its column, never across the rules).
const COLS_SHARE: [f32; 6] = [
    MARGIN,
    MARGIN + 150.0,
    MARGIN + 235.0,
    MARGIN + 285.0,
    MARGIN + 335.0,
    PAGE_W - MARGIN,
];

// The positions table: a symbol column wide enough for « NESN.SW », the banks, then the figures.
const COLS_POSITIONS: [f32; 9] = [
    MARGIN,
    MARGIN + 62.0,
    MARGIN + 142.0,
    MARGIN + 212.0,
    MARGIN + 254.0,
    MARGIN + 334.0,
    MARGIN + 394.0,
    MARGIN + 439.0,
    PAGE_W - MARGIN,
];

/// A size line's label: the class key worded. Applied to the size FIELD only (keyed by the
/// block, never by the value — a bank or a sector named « small » is data).
fn size_label(key: &str) -> String {
    match key {
        "small" => SIZE_SMALL,
        "medium" => SIZE_MEDIUM,
        "large" => SIZE_LARGE,
        other => other,
    }
    .to_string()
}

/// A sector line's label: `""` is the « non renseigné » bucket (the sector field only).
fn sector_label(key: &str) -> String {
    if key.is_empty() {
        SECTOR_UNLABELED.to_string()
    } else {
        key.to_string()
    }
}

/// The lines of one block with their labels worded by the block's own rule.
fn labelled(lines: &[ShareLine], label: fn(&str) -> String) -> Vec<ShareLine> {
    lines
        .iter()
        .map(|l| ShareLine {
            label: label(&l.label),
            ..l.clone()
        })
        .collect()
}

/// An unclassified row's « non classé » reason — the missing-rate reason names its pair.
fn reason_label(line: &ShareLine) -> String {
    match line.reason.as_str() {
        "no_study" => REASON_NO_STUDY.to_string(),
        "study_unavailable" => REASON_STUDY_UNAVAILABLE.to_string(),
        "no_sales" => REASON_NO_SALES.to_string(),
        "missing_rate" => format!("{REASON_MISSING_RATE} {}", line.missing),
        "unconvertible" => REASON_UNCONVERTIBLE.to_string(),
        _ => UNAVAILABLE.to_string(),
    }
}

/// A share block's note, as the screen states it: an unclassified reason, else the missing
/// pair(s) blocking the figure, else a plain « indisponible » when no figure could be stated at
/// all (never a row of dashes that explains nothing); then the block's threshold murmur.
fn note_label(line: &ShareLine, murmur: &str) -> String {
    let mut parts = Vec::new();
    if !line.reason.is_empty() {
        parts.push(reason_label(line));
    } else if !line.missing.is_empty() {
        parts.push(format!("{MISSING_RATE} {}", line.missing));
    } else if line.amount.is_empty() && line.share.is_empty() {
        parts.push(UNAVAILABLE.to_string());
    }
    if line.flagged {
        parts.push(murmur.to_string());
    }
    parts.join(" · ")
}

/// One share block: its « indisponible » statement when the read failed (never stale rows, never
/// « Aucune donnée. »), else its grid; `murmur` is the block's threshold wording.
fn share_block(
    doc: &mut Doc,
    title: &str,
    unavailable: Option<&str>,
    lines: &[ShareLine],
    with_target: bool,
    murmur: &str,
) {
    doc.section(title);
    if let Some(statement) = unavailable {
        doc.line(statement);
        doc.gap(4.0);
        return;
    }
    if lines.is_empty() {
        doc.line(EMPTY_BLOCK);
        doc.gap(4.0);
        return;
    }
    doc.grid_begin(lines.len());
    let target_head = if with_target { C_TARGET } else { "" };
    doc.grid_row_range(
        &[C_LABEL, C_AMOUNT, C_SHARE, target_head, C_NOTE],
        &COLS_SHARE,
        true,
        1..4,
    );
    for l in lines {
        let note = note_label(l, murmur);
        let cells = [
            l.label.clone(),
            or_dash(&l.amount),
            or_dash(&l.share),
            if with_target {
                or_dash(&l.target)
            } else {
                String::new()
            },
            note,
        ];
        let refs: Vec<&str> = cells.iter().map(String::as_str).collect();
        doc.grid_row_range(&refs, &COLS_SHARE, false, 1..4);
    }
    doc.grid_end(&COLS_SHARE);
    doc.gap(4.0);
}

fn or_dash(s: &str) -> String {
    if s.is_empty() {
        EM_DASH.to_string()
    } else {
        s.to_string()
    }
}

/// The study's state in its (narrow) column; « confiance réduite » goes to the small-print line.
fn study_label(l: &ReviewLine) -> &'static str {
    match l.study.as_str() {
        "full" => STUDY_FULL,
        "provisional" => STUDY_PROVISIONAL,
        "withheld" => STUDY_WITHHELD,
        "unavailable" => STUDY_UNAVAILABLE,
        "not_computable" => STUDY_NOT_COMPUTABLE,
        _ => STUDY_NONE,
    }
}

fn zone_label(key: &str) -> &str {
    match key {
        "buy" => ZONE_BUY,
        "neutral" => ZONE_NEUTRAL,
        "sell" => ZONE_SELL,
        "below" => ZONE_BELOW,
        "above" => ZONE_ABOVE,
        _ => EM_DASH,
    }
}

fn data_label(l: &ReviewLine) -> String {
    match l.data_state.as_str() {
        "stale" => DATA_STALE.to_string(),
        "fresh" if !l.as_of.is_empty() => format!("{DATA_FRESH} {}", l.as_of),
        _ => EM_DASH.to_string(),
    }
}

/// Render the review (FR53). Deterministic bytes; greyscale; neutral labels only.
pub fn render_portfolio_review(review: &PortfolioReview) -> Vec<u8> {
    let mut doc = Doc::new();
    doc.title(TITLE);
    doc.header_box(&[
        [
            (H_DOSSIER, review.dossier.as_str()),
            (H_DATE, review.date.as_str()),
            (H_CURRENCY, review.reference_currency.as_str()),
        ],
        [
            (H_BANKS, review.bank_count.as_str()),
            (H_POSITIONS, review.position_count.as_str()),
            (H_LINKED, review.linked_count.as_str()),
        ],
    ]);
    if !review.rates.is_empty() {
        doc.small_line(&format!("{RATES_NOTE} {}", review.rates.join("   ·   ")));
    }
    doc.gap(4.0);

    // Répartition blocks — each label worded by its OWN block's rule (sizes, sectors), never by
    // the label's value; an unavailable block states itself, as on the screen.
    let unavailable = |flag: bool, statement: &'static str| flag.then_some(statement);
    let mut size_lines = labelled(&review.size_lines, size_label);
    size_lines.extend(review.unclassified.iter().cloned());
    share_block(
        &mut doc,
        S_SIZE,
        unavailable(review.diversification_unavailable, SIZE_UNAVAILABLE),
        &size_lines,
        true,
        MURMUR,
    );
    // Sectors murmur AT or OVER the threshold (no « approché » band); currencies never murmur.
    share_block(
        &mut doc,
        S_SECTOR,
        unavailable(review.sectors_unavailable, SECTORS_UNAVAILABLE),
        &labelled(&review.sector_lines, sector_label),
        false,
        MURMUR_REACHED,
    );
    share_block(
        &mut doc,
        S_CURRENCY,
        unavailable(review.currencies_unavailable, CURRENCIES_UNAVAILABLE),
        &review.currency_lines,
        false,
        MURMUR,
    );
    let mut bank_lines = review.bank_lines.clone();
    if !review.global_invested.is_empty() {
        bank_lines.push(ShareLine {
            label: GLOBAL_TOTAL.to_string(),
            amount: review.global_invested.clone(),
            ..ShareLine::default()
        });
    }
    share_block(&mut doc, S_BANK, None, &bank_lines, false, MURMUR);
    // The global total's named absence (the screen's band): its pair(s), else plain.
    if !review.global_missing.is_empty() {
        doc.small_line(&format!("{GLOBAL_MISSING} {}", review.global_missing));
    } else if review.global_unavailable {
        doc.small_line(GLOBAL_UNAVAILABLE);
    }
    share_block(
        &mut doc,
        S_CONCENTRATION,
        unavailable(
            review.diversification_unavailable,
            CONCENTRATION_UNAVAILABLE,
        ),
        &review.concentration,
        false,
        MURMUR,
    );
    if !review.diversification_unavailable {
        if !review.concentration_missing.is_empty() {
            doc.small_line(&format!(
                "{SHARES_MISSING} {}",
                review.concentration_missing
            ));
        } else if review.concentration_global_absent {
            doc.small_line(SHARES_UNAVAILABLE);
        }
    }
    if !review.concentration_threshold.is_empty() {
        doc.small_line(&format!(
            "{THRESHOLD_NOTE} {}",
            review.concentration_threshold
        ));
    }

    // The positions table — page 2 onwards.
    doc.new_page();
    doc.section(S_POSITIONS);
    if review.positions.is_empty() {
        doc.line(EMPTY_BLOCK);
    } else {
        doc.grid_begin(review.positions.len());
        doc.grid_row_num(
            &[
                P_TICKER, P_BANKS, P_INVESTED, P_SHARE, P_STUDY, P_ZONE, P_UD, P_RELATIVE,
            ],
            &COLS_POSITIONS,
            true,
            2,
        );
        for l in &review.positions {
            let cells = [
                l.ticker.clone(),
                l.banks.clone(),
                // An absent amount reads « indisponible » as on the screen (its pair, when
                // nameable, is in the note line) — never a bare dash.
                if l.invested.is_empty() {
                    UNAVAILABLE.to_string()
                } else {
                    l.invested.clone()
                },
                or_dash(&l.share),
                study_label(l).to_string(),
                zone_label(&l.zone).to_string(),
                or_dash(&l.ud),
                or_dash(&l.relative),
            ];
            let refs: Vec<&str> = cells.iter().map(String::as_str).collect();
            doc.grid_row_num(&refs, &COLS_POSITIONS, false, 2);
            // A second, small-print line under the row: the named absences (the pair blocking
            // the invested figure, the other-currency cause of « aucune étude »), the flags with
            // their count, the data state, the stop — the screen's facts, none dropped.
            let mut extra: Vec<String> = Vec::new();
            if !l.name.is_empty() {
                extra.push(l.name.clone());
            }
            if !l.missing.is_empty() {
                extra.push(format!("{AMOUNT_MISSING} {}", l.missing));
            }
            if l.study == "none" && !l.other_currency.is_empty() {
                extra.push(format!(
                    "{OTHER_CURRENCY_STUDY} {}, {OTHER_CURRENCY_POSITION} {}",
                    l.other_currency, l.currency
                ));
            }
            if !l.mixed_links.is_empty() {
                extra.push(format!("{MIXED_LINKS} {}", l.mixed_links));
            }
            if !l.price.is_empty() {
                extra.push(format!("{PRICE_LABEL} {}", l.price));
            }
            if l.low_confidence {
                extra.push(LOW_CONFIDENCE.to_string());
            }
            if !l.flags.is_empty() {
                extra.push(format!("{FLAGS_LABEL} ({}) : {}", l.flag_count, l.flags));
            }
            extra.push(format!("{P_DATA} : {}", data_label(l)));
            if !l.stop.is_empty() {
                // Which level(s) — and bank(s) — the price reached, never a blanket mark.
                let breached = if l.stop_breached {
                    format!(" ({STOP_BREACHED} : {})", l.stop_breached_levels)
                } else {
                    String::new()
                };
                extra.push(format!("{STOP_LABEL} {}{breached}", l.stop));
            }
            if l.last_saved_unknown {
                extra.push(format!("{LAST_SAVED_LABEL} {DUE_DATE_UNKNOWN}"));
            } else if !l.last_saved.is_empty() {
                extra.push(format!("{LAST_SAVED_LABEL} {}", l.last_saved));
            }
            match l.trigger.as_str() {
                "stop" => extra.push(TRIGGER_STOP.to_string()),
                "sell" => extra.push(TRIGGER_SELL.to_string()),
                _ => {}
            }
            doc.grid_note_row(&extra.join("   ·   "), COLS_POSITIONS[1], &COLS_POSITIONS);
        }
        doc.grid_end(&COLS_POSITIONS);
    }
    doc.gap(6.0);

    doc.section(S_DUE);
    if review.due.is_empty() {
        doc.line(DUE_NONE);
    } else {
        let cols = [MARGIN, MARGIN + 120.0, MARGIN + 260.0, PAGE_W - MARGIN];
        doc.grid_begin(review.due.len());
        doc.grid_row_num(&[D_TICKER, D_DATE, D_REASON], &cols, true, 9);
        for d in &review.due {
            // Every reason that applies, in the app's fixed order.
            let reasons = d
                .reasons
                .iter()
                .map(|r| match r.as_str() {
                    "age" => DUE_AGE,
                    "withheld" => DUE_WITHHELD,
                    "low_confidence" => DUE_LOW_CONFIDENCE,
                    "age_unknown" => DUE_AGE_UNKNOWN,
                    "not_computable" => DUE_NOT_COMPUTABLE,
                    _ => EM_DASH,
                })
                .collect::<Vec<_>>()
                .join(" · ");
            let date = if d.date_unknown {
                DUE_DATE_UNKNOWN.to_string()
            } else {
                or_dash(&d.date)
            };
            doc.grid_row_num(&[&d.ticker, &date, &or_dash(&reasons)], &cols, false, 9);
        }
        doc.grid_end(&cols);
    }
    doc.gap(6.0);

    doc.section(S_COUNTS);
    for (key, value) in &review.counts {
        let label = match key.as_str() {
            "positions" => K_POSITIONS,
            "linked" => K_LINKED,
            "full" => K_FULL,
            "provisional" => K_PROVISIONAL,
            "withheld" => K_WITHHELD,
            "not_computable" => K_NOT_COMPUTABLE,
            "flagged" => K_FLAGGED,
            "high_zone" => K_HIGH_ZONE,
            "stop_breached" => K_STOP_BREACHED,
            "due" => K_DUE,
            other => other,
        };
        doc.line(&format!("{value}   {label}"));
    }
    doc.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PortfolioReview {
        PortfolioReview {
            dossier: "test".into(),
            date: "2026-09-24".into(),
            reference_currency: "CHF".into(),
            bank_count: "2".into(),
            position_count: "3".into(),
            linked_count: "2".into(),
            rates: vec!["USD → CHF 0.80 (2026-09-23, manual)".into()],
            size_lines: vec![ShareLine {
                label: "large".into(),
                amount: "".into(),
                share: "34,1 %".into(),
                target: "25 %".into(),
                ..Default::default()
            }],
            unclassified: vec![ShareLine {
                label: "NVDA.US".into(),
                missing: "USD → CHF".into(),
                reason: "missing_rate".into(),
                ..Default::default()
            }],
            sector_lines: vec![ShareLine {
                label: "".into(),
                share: "100 %".into(),
                ..Default::default()
            }],
            currency_lines: vec![ShareLine {
                label: "CHF".into(),
                share: "100 %".into(),
                ..Default::default()
            }],
            bank_lines: vec![ShareLine {
                label: "UBS".into(),
                amount: "3 000 CHF".into(),
                share: "60 %".into(),
                ..Default::default()
            }],
            global_invested: "5 000 CHF".into(),
            concentration: vec![ShareLine {
                label: "NVDA.US".into(),
                amount: "3 880 CHF".into(),
                share: "65,9 %".into(),
                flagged: true,
                ..Default::default()
            }],
            concentration_threshold: "50 %".into(),
            positions: vec![ReviewLine {
                ticker: "NVDA.US".into(),
                banks: "UBS, Swissquote".into(),
                invested: "3 880 CHF".into(),
                share: "65,9 %".into(),
                study: "provisional".into(),
                zone: "buy".into(),
                ud: "5,3:1".into(),
                relative: "114 %".into(),
                flags: "PER haut jugé au-dessus de 25 · ROE en baisse".into(),
                flag_count: 2,
                data_state: "fresh".into(),
                as_of: "2026-09-23".into(),
                stop: "180 USD".into(),
                ..Default::default()
            }],
            due: vec![DueLine {
                ticker: "SCHN.SW".into(),
                date: "2025-06-01".into(),
                reasons: vec!["age".into(), "withheld".into()],
                ..Default::default()
            }],
            counts: vec![("positions".into(), "3".into()), ("due".into(), "1".into())],
            ..Default::default()
        }
    }

    /// Whether `needle` (ASCII; a line may wrap at a space, so keep needles short) appears in the
    /// rendered content streams — as a literal string, or hex-encoded (the WinAnsi writer emits a
    /// string carrying any non-ASCII glyph as `<…>` hex).
    fn carries(bytes: &[u8], needle: &str) -> bool {
        let hex: String = needle.bytes().map(|b| format!("{b:02X}")).collect();
        [needle.as_bytes(), hex.as_bytes()]
            .iter()
            .any(|n| bytes.windows(n.len()).any(|w| w == *n))
    }

    #[test]
    fn an_unavailable_block_states_itself_never_stale_rows_nor_aucune_donnee() {
        let mut r = sample();
        r.sectors_unavailable = true;
        r.sector_lines = vec![ShareLine {
            label: "StaleSector".into(),
            share: "40 %".into(),
            ..Default::default()
        }];
        r.currencies_unavailable = true;
        r.currency_lines = Vec::new();
        let bytes = render_portfolio_review(&r);
        assert!(
            !carries(&bytes, "StaleSector"),
            "an unavailable block prints no row"
        );
        assert!(carries(&bytes, "Exposition par secteur indisponible."));
        assert!(carries(&bytes, "Exposition par devise indisponible."));
        assert!(
            !carries(&bytes, "Aucune donn"),
            "an unavailable block is never « Aucune donnée. »"
        );
    }

    #[test]
    fn the_named_absences_of_the_screen_reach_the_pdf() {
        let mut r = sample();
        r.global_invested = String::new();
        r.global_missing = "EUR -> CHF".into();
        r.concentration_missing = "GBP -> CHF".into();
        r.positions[0].invested = String::new();
        r.positions[0].missing = "JPY -> CHF · SEK -> CHF".into();
        r.positions.push(ReviewLine {
            ticker: "ROG.SW".into(),
            currency: "CHF".into(),
            study: "none".into(),
            other_currency: "USD".into(),
            ..Default::default()
        });
        let bytes = render_portfolio_review(&r);
        assert!(carries(
            &bytes,
            "Total global indisponible : taux manquant EUR -> CHF"
        ));
        assert!(carries(
            &bytes,
            "Parts indisponibles : taux manquant GBP -> CHF"
        ));
        assert!(carries(&bytes, "JPY"), "every missing pair is named");
        assert!(carries(&bytes, "SEK"), "every missing pair is named");
        assert!(carries(&bytes, "position en"), "the other-currency cause");
        // A global total absent for an un-nameable reason still states itself.
        let mut r = sample();
        r.global_invested = String::new();
        r.global_unavailable = true;
        assert!(carries(
            &render_portfolio_review(&r),
            "Total global indisponible."
        ));
    }

    #[test]
    fn labels_follow_the_block_not_the_value_and_reasons_keep_non_classe() {
        // A bank named « small » is data; only the size block words its class keys.
        let bank = ShareLine {
            label: "small".into(),
            ..Default::default()
        };
        assert_eq!(
            labelled(std::slice::from_ref(&bank), size_label)[0].label,
            "Petite"
        );
        let mut r = sample();
        r.bank_lines = vec![ShareLine {
            label: "small".into(),
            amount: "1 CHF".into(),
            ..Default::default()
        }];
        let bytes = render_portfolio_review(&r);
        assert!(
            bytes.windows(7).any(|w| w == b"(small)"),
            "the bank's own name"
        );
        // An unclassified MissingRate row keeps « non classé » AND names its pair.
        let row = ShareLine {
            label: "NVDA.US".into(),
            missing: "USD → CHF".into(),
            reason: "missing_rate".into(),
            ..Default::default()
        };
        assert_eq!(
            note_label(&row, MURMUR),
            "non classé : taux manquant USD → CHF"
        );
        // A row with no figure and no pair is plainly « indisponible », never a row of dashes.
        assert_eq!(note_label(&bank, MURMUR), "indisponible");
        // The sector block's murmur is its own (at or over).
        let flagged = ShareLine {
            share: "60 %".into(),
            flagged: true,
            ..Default::default()
        };
        assert_eq!(note_label(&flagged, MURMUR_REACHED), MURMUR_REACHED);
    }

    #[test]
    fn signals_carry_their_count_and_every_due_reason_is_printed() {
        let bytes = render_portfolio_review(&sample());
        assert!(carries(&bytes, "Signaux (2)"));
        assert!(carries(&bytes, "plus de 12 mois"), "the age reason");
        assert!(carries(&bytes, "une donn"), "the withheld reason beside it");
        let mut r = sample();
        r.due[0].date = String::new();
        r.due[0].date_unknown = true;
        assert!(carries(
            &render_portfolio_review(&r),
            "(historique indisponible)"
        ));
    }

    #[test]
    fn the_position_line_carries_what_the_screen_states() {
        let mut r = sample();
        let p = &mut r.positions[0];
        p.price = "123 USD".into();
        p.last_saved = String::new();
        p.last_saved_unknown = true;
        p.mixed_links = "CHF".into();
        p.stop = "63 CHF UBS".into();
        p.stop_breached = true;
        p.stop_breached_levels = "BREACHEDLVL".into();
        p.invested = String::new();
        r.due = vec![DueLine {
            ticker: "ROG.SW".into(),
            date_unknown: true,
            reasons: vec!["age_unknown".into(), "not_computable".into()],
            ..Default::default()
        }];
        r.counts = vec![("not_computable".into(), "1".into())];
        let bytes = render_portfolio_review(&r);
        assert!(
            carries(&bytes, "prix actuel : 123 USD"),
            "the present price"
        );
        assert!(
            carries(&bytes, "sauvegarde : inconnue"),
            "the unknown last save"
        );
        assert!(
            carries(&bytes, "BREACHEDLVL"),
            "which stop level is breached"
        );
        assert!(
            carries(&bytes, "indisponible"),
            "an absent amount is stated"
        );
        assert!(carries(&bytes, "anciennet"), "the unknown-age due reason");
        assert!(
            carries(&bytes, "non calculables"),
            "the not-computable reason / count"
        );
        // A known last save is printed as a date.
        let mut r = sample();
        r.positions[0].last_saved = "2026-01-02".into();
        assert!(carries(&render_portfolio_review(&r), "2026-01-02"));
    }

    #[test]
    fn the_positions_header_repeats_across_a_page_break() {
        let mut r = sample();
        let row = r.positions[0].clone();
        r.positions = (0..80)
            .map(|i| ReviewLine {
                ticker: format!("T{i:03}"),
                ..row.clone()
            })
            .collect();
        let bytes = render_portfolio_review(&r);
        // « Val. rel. » is a positions-only header cell: one per page the table touches.
        let count = bytes
            .windows(b"Val. rel.".len())
            .filter(|w| *w == b"Val. rel.")
            .count();
        assert!(count >= 2, "the header must repeat, saw {count}");
    }

    #[test]
    fn renders_a_well_formed_deterministic_pdf() {
        let a = render_portfolio_review(&sample());
        let b = render_portfolio_review(&sample());
        assert!(a.starts_with(b"%PDF-"));
        assert!(a.windows(5).any(|w| w == b"%%EOF"));
        assert_eq!(a, b, "deterministic bytes");
        // Two pages at least (the positions table starts a new page).
        let pos = a
            .windows(7)
            .position(|w| w == b"/Count ")
            .expect("page tree");
        let n: u32 = a[pos + 7..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .fold(0, |acc, b| acc * 10 + u32::from(b - b'0'));
        assert!(n >= 2, "got /Count {n}");
    }

    #[test]
    fn an_empty_review_renders_calmly() {
        let bytes = render_portfolio_review(&PortfolioReview::default());
        assert!(bytes.starts_with(b"%PDF-") && bytes.windows(5).any(|w| w == b"%%EOF"));
    }

    #[test]
    fn review_strings_are_neutral_no_banned_verb_no_wordmark() {
        use steadyinvest_core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR};
        for s in REVIEW_USER_FACING {
            let lower = s.to_lowercase();
            for token in lower.split(|c: char| !c.is_alphanumeric()) {
                for banned in BANNED_VERBS_EN.iter().chain(BANNED_VERBS_FR.iter()) {
                    assert_ne!(
                        token,
                        banned.to_lowercase(),
                        "review string {s:?} contains banned verb {banned:?}"
                    );
                }
            }
            for mark in ["NAIC", "Stock Selection Guide", "Better Investing", "SSG"] {
                assert!(
                    !s.contains(mark),
                    "review string {s:?} carries the mark {mark:?}"
                );
            }
        }
    }
}

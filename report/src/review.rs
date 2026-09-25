//! Story 7.2 (FR53) — the portfolio health review as a neutral, greyscale PDF, in the study PDF's
//! style. The `app` builds a [`PortfolioReview`] of already-formatted figures + enum-like keys; this
//! module owns EVERY label (its own neutral inventory, tested like `pdf.rs`'s) and lays them out:
//! page 1 = the header + the four répartition blocks + the concentration; page 2+ = the positions
//! table (header repeated across breaks), the studies due for review, the counts.

use pdf_writer::Content;

use crate::pdf::{Doc, EM_DASH, MARGIN, PAGE_W, SMALL};

/// One line of a share block: a label (data — a sector, a currency, a bank, a ticker, or a size
/// key `small` | `medium` | `large`), an amount, a share, a target, and a note (a missing pair or a
/// reason key) — all pre-formatted by the app, `""` when absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShareLine {
    pub label: String,
    pub amount: String,
    pub share: String,
    pub target: String,
    pub note: String,
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
    /// `full` | `provisional` | `withheld` | `none` | `unavailable`.
    pub study: String,
    pub low_confidence: bool,
    /// `buy` | `neutral` | `sell` | `below` | `above` | `""`.
    pub zone: String,
    pub ud: String,
    pub relative: String,
    /// The flags already worded by the app (its inventory), joined; `""` when none.
    pub flags: String,
    /// `stale` | `fresh` | `""`, and the as-of date.
    pub data_state: String,
    pub as_of: String,
    pub stop: String,
    pub stop_breached: bool,
    /// `stop` | `sell` | `""`.
    pub trigger: String,
}

/// A study due for review: `age` | `withheld` | `low_confidence`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DueLine {
    pub ticker: String,
    pub date: String,
    pub reason: String,
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
    pub sector_lines: Vec<ShareLine>,
    pub currency_lines: Vec<ShareLine>,
    pub bank_lines: Vec<ShareLine>,
    pub global_invested: String,
    pub concentration: Vec<ShareLine>,
    pub concentration_threshold: String,
    pub positions: Vec<ReviewLine>,
    pub due: Vec<DueLine>,
    /// `(count-key, value)` — keys: positions · linked · full · provisional · withheld ·
    /// flagged · high_zone · stop_breached · due.
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
const LOW_CONFIDENCE: &str = "confiance réduite";
const ZONE_BUY: &str = "basse";
const ZONE_NEUTRAL: &str = "médiane";
const ZONE_SELL: &str = "haute";
const ZONE_BELOW: &str = "sous la bande";
const ZONE_ABOVE: &str = "au-dessus de la bande";
const DATA_STALE: &str = "périmé";
const DATA_FRESH: &str = "à jour le";
const FLAGS_LABEL: &str = "Signaux :";
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
const DUE_NONE: &str = "Aucune étude à revoir.";
const K_POSITIONS: &str = "positions";
const K_LINKED: &str = "avec une étude";
const K_FULL: &str = "critères validés";
const K_PROVISIONAL: &str = "provisoires";
const K_WITHHELD: &str = "en attente";
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
    DUE_NONE,
    K_POSITIONS,
    K_LINKED,
    K_FULL,
    K_PROVISIONAL,
    K_WITHHELD,
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

fn size_label(key: &str) -> &str {
    match key {
        "small" => SIZE_SMALL,
        "medium" => SIZE_MEDIUM,
        "large" => SIZE_LARGE,
        "" => SECTOR_UNLABELED,
        other => other,
    }
}

fn reason_label(key: &str) -> String {
    match key.split_once(':') {
        Some(("missing_rate", pair)) => format!("{REASON_MISSING_RATE} {pair}"),
        _ => match key {
            "no_study" => REASON_NO_STUDY.to_string(),
            "study_unavailable" => REASON_STUDY_UNAVAILABLE.to_string(),
            "no_sales" => REASON_NO_SALES.to_string(),
            "unconvertible" => REASON_UNCONVERTIBLE.to_string(),
            other => other.to_string(),
        },
    }
}

/// A share block's note: a missing pair (`missing_rate:EUR → CHF`), a murmur, or plain data.
fn note_label(line: &ShareLine) -> String {
    let mut parts = Vec::new();
    if let Some(("missing_rate", pair)) = line.note.split_once(':') {
        parts.push(format!("{MISSING_RATE} {pair}"));
    } else if !line.note.is_empty() {
        parts.push(reason_label(&line.note));
    }
    if line.flagged {
        parts.push(MURMUR.to_string());
    }
    parts.join(" · ")
}

fn share_block(doc: &mut Doc, title: &str, lines: &[ShareLine], with_target: bool) {
    doc.section(title);
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
        let note = note_label(l);
        let cells = [
            size_label(&l.label).to_string(),
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

    // Répartition blocks.
    let mut size_lines = review.size_lines.clone();
    size_lines.extend(review.unclassified.iter().cloned());
    share_block(&mut doc, S_SIZE, &size_lines, true);
    share_block(&mut doc, S_SECTOR, &review.sector_lines, false);
    share_block(&mut doc, S_CURRENCY, &review.currency_lines, false);
    let mut bank_lines = review.bank_lines.clone();
    if !review.global_invested.is_empty() {
        bank_lines.push(ShareLine {
            label: GLOBAL_TOTAL.to_string(),
            amount: review.global_invested.clone(),
            ..ShareLine::default()
        });
    }
    share_block(&mut doc, S_BANK, &bank_lines, false);
    share_block(&mut doc, S_CONCENTRATION, &review.concentration, false);
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
                or_dash(&l.invested),
                or_dash(&l.share),
                study_label(l).to_string(),
                zone_label(&l.zone).to_string(),
                or_dash(&l.ud),
                or_dash(&l.relative),
            ];
            let refs: Vec<&str> = cells.iter().map(String::as_str).collect();
            // A second, small-print line under the row: the flags, the data state, the stop —
            // laid out with its row as one block (G1 D: never split from it by a page break).
            let mut extra: Vec<String> = Vec::new();
            if !l.name.is_empty() {
                extra.push(l.name.clone());
            }
            if l.low_confidence {
                extra.push(LOW_CONFIDENCE.to_string());
            }
            if !l.flags.is_empty() {
                extra.push(format!("{FLAGS_LABEL} {}", l.flags));
            }
            extra.push(format!("{P_DATA} : {}", data_label(l)));
            if !l.stop.is_empty() {
                let breached = if l.stop_breached {
                    format!(" ({STOP_BREACHED})")
                } else {
                    String::new()
                };
                extra.push(format!("{STOP_LABEL} {}{breached}", l.stop));
            }
            match l.trigger.as_str() {
                "stop" => extra.push(TRIGGER_STOP.to_string()),
                "sell" => extra.push(TRIGGER_SELL.to_string()),
                _ => {}
            }
            doc.grid_row_num_with_note(
                &refs,
                &COLS_POSITIONS,
                2,
                &extra.join("   ·   "),
                COLS_POSITIONS[1],
            );
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
            let reason = match d.reason.as_str() {
                "age" => DUE_AGE,
                "withheld" => DUE_WITHHELD,
                "low_confidence" => DUE_LOW_CONFIDENCE,
                _ => EM_DASH,
            };
            doc.grid_row_num(&[&d.ticker, &or_dash(&d.date), reason], &cols, false, 9);
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
            "flagged" => K_FLAGGED,
            "high_zone" => K_HIGH_ZONE,
            "stop_breached" => K_STOP_BREACHED,
            "due" => K_DUE,
            other => other,
        };
        doc.line(&format!("{value}   {label}"));
    }
    let _ = SMALL; // the small-print size is reachable through Doc; kept for symmetry with pdf.rs
    let _: Option<&Content> = None;
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
                note: "missing_rate:USD → CHF".into(),
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
                flags: "PER haut jugé au-dessus de 25".into(),
                data_state: "fresh".into(),
                as_of: "2026-09-23".into(),
                stop: "180 USD".into(),
                ..Default::default()
            }],
            due: vec![DueLine {
                ticker: "SCHN.SW".into(),
                date: "2025-06-01".into(),
                reason: "age".into(),
            }],
            counts: vec![("positions".into(), "3".into()), ("due".into(), "1".into())],
        }
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

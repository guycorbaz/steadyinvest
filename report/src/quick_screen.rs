//! Story 7.3 (FR53) — the « Examen rapide » (the Stock Check List for Beginning Investors) as a
//! neutral, greyscale, A4-portrait PDF: the two ladders, the five-year price / P/E record, the
//! reader's own fields and the four conclusions worded as facts. The `app` formats the figures
//! and passes KEYS for every worded fact; this module owns the words (its inventory, tested).

use crate::pdf::{Doc, EM_DASH, MARGIN};

/// One ladder (§1 sales, §2 EPS): the form's ten lines, formatted, plus the rate and the years.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuickScreenLadder {
    /// Lines (1)…(10) in the form's order; `""` = absent.
    pub lines: Vec<String>,
    /// The years feeding lines (1), (2), (5), (6); `""` when unknown.
    pub years: Vec<String>,
    /// The compound annual rate, formatted (« 8,1 % »); `""` = absent.
    pub rate: String,
    /// The span in years the rate was computed over (« 5 »); `""` when the ladder is unavailable.
    pub span_years: String,
    pub unavailable: bool,
}

/// One row of the §3 price record, formatted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuickScreenPriceRow {
    pub year: String,
    pub high: String,
    pub low: String,
    pub eps: String,
    pub pe_high: String,
    pub pe_low: String,
}

/// The examination, ready to lay out. Keys: `eps_vs_sales` ∈ eps | sales | same | "";
/// `factors_continue` ∈ yes | less | no | ""; `pe_position` ∈ higher | similar | lower | "";
/// `sales_meets` / `eps_meets` / `eps_will_meet` ∈ yes | no | "" (`""` = objective not given).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuickScreen {
    pub ticker: String,
    pub name: String,
    pub currency: String,
    pub date: String,
    /// « depuis l'étude » or the provider's name — the app words it (a source name is data).
    pub source: String,
    pub sales: QuickScreenLadder,
    pub eps: QuickScreenLadder,
    pub eps_vs_sales: String,
    pub reasons: String,
    pub factors_continue: String,
    pub price_rows: Vec<QuickScreenPriceRow>,
    pub pe_high_total: String,
    pub pe_low_total: String,
    pub pe_high_avg: String,
    pub pe_low_avg: String,
    pub pe_avg_of_avgs: String,
    pub present_price: String,
    pub present_eps: String,
    pub present_pe: String,
    pub high_five_years_ago: String,
    pub price_vs_high_pct: String,
    pub years_sold_as_high: String,
    pub pe_position: String,
    pub pe_notes: String,
    pub objective: String,
    pub sales_meets: String,
    pub eps_meets: String,
    pub eps_will_meet: String,
}

// ── the neutral inventory (FR13) ──
const TITLE: &str = "Examen rapide";
const SUBTITLE: &str = "Liste de contrôle pour débutant — un regard avant l'étude complète";
const DATE: &str = "Date";
const SOURCE: &str = "Source des données";
const S1: &str = "1 · Ventes passées";
const S2: &str = "2 · Bénéfice par action passé";
const S3: &str = "3 · Cours de l'action";
const S4: &str = "4 · Conclusion";
const L_RECENT: &str = "(1) Ventes de l'année la plus récente";
const L_RECENT_PRIOR: &str = "(2) Ventes de l'année précédente";
const L_TOTAL_A: &str = "(3) Total de (1) + (2)";
const L_HALF_A: &str = "(4) (3) divisé par 2";
const L_OLD: &str = "(5) Ventes il y a {} ans";
const L_OLD_PRIOR: &str = "(6) Ventes il y a {} ans";
const L_TOTAL_B: &str = "(7) Total de (5) + (6)";
const L_HALF_B: &str = "(8) (7) divisé par 2";
const L_INCREASE: &str = "(9) Hausse sur la période, (4) − (8)";
const L_INCREASE_PCT: &str = "(10) Hausse en pour cent, (9) ÷ (8)";
const E_RECENT: &str = "(1) BPA de l'année la plus récente";
const E_RECENT_PRIOR: &str = "(2) BPA de l'année précédente";
const E_OLD: &str = "(5) BPA il y a {} ans";
const E_OLD_PRIOR: &str = "(6) BPA il y a {} ans";
const RATE_SALES: &str = "Taux annuel composé de croissance des ventes";
const RATE_EPS: &str = "Taux annuel composé de croissance du BPA";
const SPAN_NOTE: &str = "Base : deux moyennes de deux ans, à {} ans d'écart (formulaire : 5).";
const UNAVAILABLE: &str = "indisponible (moins de trois années exploitables)";
const EPS_FASTER: &str = "Le BPA a augmenté plus vite que les ventes sur la période.";
const SALES_FASTER: &str = "Le BPA a augmenté moins vite que les ventes sur la période.";
const SAME_PACE: &str = "Le BPA et les ventes ont augmenté au même rythme sur la période.";
const REASONS: &str = "Raisons apparentes de l'écart et de la croissance passée :";
const FACTORS: &str = "Les facteurs de la croissance passée resteront-ils effectifs cinq ans ?";
const FACTORS_YES: &str = "oui";
const FACTORS_LESS: &str = "oui, mais moins";
const FACTORS_NO: &str = "non";
const NOT_FILLED: &str = "non renseigné";
const PRESENT_PRICE: &str = "Cours actuel";
const PRESENT_EPS: &str = "BPA actuel";
const PRESENT_PE: &str = "PER actuel";
const H_YEAR: &str = "Année";
const H_HIGH: &str = "Cours haut (A)";
const H_LOW: &str = "Cours bas (B)";
const H_EPS: &str = "BPA (C)";
const H_PE_HIGH: &str = "PER au haut (A ÷ C)";
const H_PE_LOW: &str = "PER au bas (B ÷ C)";
const TOTALS: &str = "Totaux";
const AVERAGES: &str = "Moyennes";
const AVG_OF_AVGS: &str = "Moyenne des PER moyens haut et bas sur cinq ans";
const PRICE_VS_HIGH: &str =
    "Cours actuel par rapport au cours haut d'il y a cinq ans ({}) : {} ({})";
const HIGHER: &str = "plus haut";
const LOWER: &str = "plus bas";
const SOLD_AS_HIGH: &str = "L'action s'est vendue aussi haut que le cours actuel au cours de {} des cinq dernières années.";
const PE_POS: &str = "Le PER actuel ({}) est {} de la moyenne des cinq ans ({}).";
const PE_HIGHER: &str = "au-dessus";
const PE_SIMILAR: &str = "voisin";
const PE_LOWER: &str = "au-dessous";
const PE_NOTES: &str = "PER inhabituels, moyenne à ajuster (à l'appréciation du lecteur) :";
const OBJECTIVE: &str = "Objectif de croissance annuelle du lecteur";
const OBJECTIVE_NONE: &str = "objectif non renseigné";
const C1: &str = "1. Le taux de croissance passé des ventes ({}) {} l'objectif.";
const C2: &str = "2. Le taux de croissance passé du BPA ({}) {} l'objectif.";
const C1_NONE: &str = "1. Le taux de croissance passé des ventes ({}) : objectif non renseigné.";
const C2_NONE: &str = "2. Le taux de croissance passé du BPA ({}) : objectif non renseigné.";
const C3: &str = "3. Croissance possible du BPA sur cinq ans, avis du lecteur : {}";
const C4: &str = "4. Le cours : PER actuel {} de la norme des cinq ans.";
const MEETS: &str = "atteint";
const MISSES: &str = "n'atteint pas";
const YES: &str = "oui";
const NO: &str = "non";
const FOOTER: &str = "Ce formulaire n'est pas une analyse suffisante ; il aide à poser les questions avant une étude complète.";

#[cfg(test)]
const QUICK_SCREEN_USER_FACING: &[&str] = &[
    TITLE,
    SUBTITLE,
    DATE,
    SOURCE,
    S1,
    S2,
    S3,
    S4,
    L_RECENT,
    L_RECENT_PRIOR,
    L_TOTAL_A,
    L_HALF_A,
    L_OLD,
    L_OLD_PRIOR,
    L_TOTAL_B,
    L_HALF_B,
    L_INCREASE,
    L_INCREASE_PCT,
    E_RECENT,
    E_RECENT_PRIOR,
    E_OLD,
    E_OLD_PRIOR,
    RATE_SALES,
    RATE_EPS,
    SPAN_NOTE,
    UNAVAILABLE,
    EPS_FASTER,
    SALES_FASTER,
    SAME_PACE,
    REASONS,
    FACTORS,
    FACTORS_YES,
    FACTORS_LESS,
    FACTORS_NO,
    NOT_FILLED,
    PRESENT_PRICE,
    PRESENT_EPS,
    PRESENT_PE,
    H_YEAR,
    H_HIGH,
    H_LOW,
    H_EPS,
    H_PE_HIGH,
    H_PE_LOW,
    TOTALS,
    AVERAGES,
    AVG_OF_AVGS,
    PRICE_VS_HIGH,
    HIGHER,
    LOWER,
    SOLD_AS_HIGH,
    PE_POS,
    PE_HIGHER,
    PE_SIMILAR,
    PE_LOWER,
    PE_NOTES,
    OBJECTIVE,
    OBJECTIVE_NONE,
    C1,
    C2,
    C1_NONE,
    C2_NONE,
    C3,
    C4,
    MEETS,
    MISSES,
    YES,
    NO,
    FOOTER,
];

fn or_dash(s: &str) -> &str {
    if s.is_empty() { EM_DASH } else { s }
}

fn fill(template: &str, values: &[&str]) -> String {
    let mut out = template.to_string();
    for v in values {
        out = out.replacen("{}", v, 1);
    }
    out
}

fn ladder(doc: &mut Doc, l: &QuickScreenLadder, labels: [&str; 10], rate_label: &str) {
    if l.unavailable {
        doc.line(UNAVAILABLE);
        return;
    }
    let get = |i: usize| l.lines.get(i).map(String::as_str).unwrap_or("");
    let year = |i: usize| l.years.get(i).map(String::as_str).unwrap_or("");
    let span = l.span_years.as_str();
    let plus_one = span
        .parse::<u32>()
        .map(|n| (n + 1).to_string())
        .unwrap_or_default();
    let with_year = |label: String, y: &str| {
        if y.is_empty() {
            label
        } else {
            format!("{label} ({y})")
        }
    };
    let rows: [(String, &str); 10] = [
        (with_year(labels[0].to_string(), year(0)), get(0)),
        (with_year(labels[1].to_string(), year(1)), get(1)),
        (labels[2].to_string(), get(2)),
        (labels[3].to_string(), get(3)),
        (with_year(fill(labels[4], &[span]), year(2)), get(4)),
        (with_year(fill(labels[5], &[&plus_one]), year(3)), get(5)),
        (labels[6].to_string(), get(6)),
        (labels[7].to_string(), get(7)),
        (labels[8].to_string(), get(8)),
        (labels[9].to_string(), get(9)),
    ];
    for (label, value) in rows {
        doc.two_columns(&label, or_dash(value));
    }
    doc.two_columns(rate_label, or_dash(&l.rate));
    doc.small_line(&fill(SPAN_NOTE, &[span]));
}

/// Render the examination (FR53): A4 portrait, deterministic, greyscale, neutral labels.
pub fn render_quick_screen(q: &QuickScreen) -> Vec<u8> {
    let mut doc = Doc::new();
    doc.title(TITLE);
    doc.small_line(SUBTITLE);
    // A criblage row without a study has no declared currency (the watchlist carries none): the
    // head then names the ticker alone rather than an empty « () ».
    let named = if q.name.is_empty() {
        q.ticker.clone()
    } else {
        format!("{} — {}", q.ticker, q.name)
    };
    let head = if q.currency.is_empty() {
        named
    } else {
        format!("{named} ({})", q.currency)
    };
    doc.line(&head);
    doc.small_line(&format!(
        "{DATE} : {} · {SOURCE} : {}",
        or_dash(&q.date),
        or_dash(&q.source)
    ));
    doc.gap(4.0);

    doc.section(S1);
    ladder(
        &mut doc,
        &q.sales,
        [
            L_RECENT,
            L_RECENT_PRIOR,
            L_TOTAL_A,
            L_HALF_A,
            L_OLD,
            L_OLD_PRIOR,
            L_TOTAL_B,
            L_HALF_B,
            L_INCREASE,
            L_INCREASE_PCT,
        ],
        RATE_SALES,
    );
    doc.gap(4.0);

    doc.section(S2);
    ladder(
        &mut doc,
        &q.eps,
        [
            E_RECENT,
            E_RECENT_PRIOR,
            L_TOTAL_A,
            L_HALF_A,
            E_OLD,
            E_OLD_PRIOR,
            L_TOTAL_B,
            L_HALF_B,
            L_INCREASE,
            L_INCREASE_PCT,
        ],
        RATE_EPS,
    );
    match q.eps_vs_sales.as_str() {
        "eps" => doc.line(EPS_FASTER),
        "sales" => doc.line(SALES_FASTER),
        "same" => doc.line(SAME_PACE),
        _ => {}
    }
    doc.small_line(REASONS);
    doc.indent_line(or_dash(&q.reasons));
    let factors = match q.factors_continue.as_str() {
        "yes" => FACTORS_YES,
        "less" => FACTORS_LESS,
        "no" => FACTORS_NO,
        _ => NOT_FILLED,
    };
    doc.small_line(FACTORS);
    doc.indent_line(factors);
    doc.gap(4.0);

    doc.section(S3);
    doc.two_columns(PRESENT_PRICE, or_dash(&q.present_price));
    doc.two_columns(PRESENT_EPS, or_dash(&q.present_eps));
    doc.two_columns(PRESENT_PE, or_dash(&q.present_pe));
    let label_w = 90.0;
    let n = 6;
    let col_w = (doc.right() - MARGIN - label_w) / 5.0;
    let mut edges = vec![MARGIN, MARGIN + label_w];
    for i in 1..n {
        edges.push(MARGIN + label_w + col_w * i as f32);
    }
    doc.grid_begin(q.price_rows.len() + 3);
    doc.grid_row_small(
        &[H_YEAR, H_HIGH, H_LOW, H_EPS, H_PE_HIGH, H_PE_LOW],
        &edges,
        true,
        1,
    );
    for r in &q.price_rows {
        let cells = [
            r.year.as_str(),
            or_dash(&r.high),
            or_dash(&r.low),
            or_dash(&r.eps),
            or_dash(&r.pe_high),
            or_dash(&r.pe_low),
        ];
        doc.grid_row_small(&cells, &edges, false, 1);
    }
    doc.grid_row_small(
        &[
            TOTALS,
            "",
            "",
            "",
            or_dash(&q.pe_high_total),
            or_dash(&q.pe_low_total),
        ],
        &edges,
        true,
        1,
    );
    doc.grid_row_small(
        &[
            AVERAGES,
            "",
            "",
            "",
            or_dash(&q.pe_high_avg),
            or_dash(&q.pe_low_avg),
        ],
        &edges,
        true,
        1,
    );
    doc.grid_end(&edges);
    doc.two_columns(AVG_OF_AVGS, or_dash(&q.pe_avg_of_avgs));
    if !q.price_vs_high_pct.is_empty() {
        let word = if q.price_vs_high_pct.starts_with('-') || q.price_vs_high_pct.starts_with('−')
        {
            LOWER
        } else {
            HIGHER
        };
        doc.line(&fill(
            PRICE_VS_HIGH,
            &[or_dash(&q.high_five_years_ago), word, &q.price_vs_high_pct],
        ));
    }
    if !q.years_sold_as_high.is_empty() {
        doc.line(&fill(SOLD_AS_HIGH, &[&q.years_sold_as_high]));
    }
    let pe_word = match q.pe_position.as_str() {
        "higher" => Some(PE_HIGHER),
        "similar" => Some(PE_SIMILAR),
        "lower" => Some(PE_LOWER),
        _ => None,
    };
    if let Some(word) = pe_word {
        doc.line(&fill(PE_POS, &[&q.present_pe, word, &q.pe_avg_of_avgs]));
    }
    doc.small_line(PE_NOTES);
    doc.indent_line(or_dash(&q.pe_notes));
    doc.gap(4.0);

    doc.section(S4);
    doc.two_columns(
        OBJECTIVE,
        if q.objective.is_empty() {
            OBJECTIVE_NONE
        } else {
            &q.objective
        },
    );
    let meets = |key: &str| match key {
        "yes" => Some(MEETS),
        "no" => Some(MISSES),
        _ => None,
    };
    let conclusion = |with: &str, without: &str, rate: &str, key: &str| match meets(key) {
        Some(word) => fill(with, &[or_dash(rate), word]),
        None => fill(without, &[or_dash(rate)]),
    };
    doc.line(&conclusion(C1, C1_NONE, &q.sales.rate, &q.sales_meets));
    doc.line(&conclusion(C2, C2_NONE, &q.eps.rate, &q.eps_meets));
    let will = match q.eps_will_meet.as_str() {
        "yes" => YES,
        "no" => NO,
        _ => NOT_FILLED,
    };
    doc.line(&fill(C3, &[will]));
    if let Some(word) = pe_word {
        doc.line(&fill(C4, &[word]));
    }
    doc.gap(6.0);
    doc.small_line(FOOTER);
    doc.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> QuickScreen {
        let ladder = |rate: &str| QuickScreenLadder {
            lines: (1..=10).map(|i| format!("v{i}")).collect(),
            years: vec!["2026".into(), "2025".into(), "2021".into(), "2020".into()],
            rate: rate.into(),
            span_years: "5".into(),
            unavailable: false,
        };
        QuickScreen {
            ticker: "NESN.SW".into(),
            name: "Nestlé".into(),
            currency: "CHF".into(),
            date: "2026-09-24".into(),
            source: "depuis l'étude".into(),
            sales: ladder("8,1 %"),
            eps: ladder("5,2 %"),
            eps_vs_sales: "sales".into(),
            reasons: String::new(),
            factors_continue: "less".into(),
            price_rows: (2022..=2026)
                .map(|y| QuickScreenPriceRow {
                    year: y.to_string(),
                    high: "100".into(),
                    low: "50".into(),
                    eps: "5".into(),
                    pe_high: "20,0".into(),
                    pe_low: "10,0".into(),
                })
                .collect(),
            pe_high_total: "100,0".into(),
            pe_low_total: "50,0".into(),
            pe_high_avg: "20,0".into(),
            pe_low_avg: "10,0".into(),
            pe_avg_of_avgs: "15,0".into(),
            present_price: "110".into(),
            present_eps: "5".into(),
            present_pe: "22,0".into(),
            high_five_years_ago: "100".into(),
            price_vs_high_pct: "10,0 %".into(),
            years_sold_as_high: "1".into(),
            pe_position: "higher".into(),
            pe_notes: String::new(),
            objective: "7 %".into(),
            sales_meets: "yes".into(),
            eps_meets: "no".into(),
            eps_will_meet: String::new(),
        }
    }

    #[test]
    fn renders_a_portrait_deterministic_pdf() {
        let q = sample();
        let a = render_quick_screen(&q);
        let b = render_quick_screen(&q);
        assert!(a.starts_with(b"%PDF-") && a.windows(5).any(|w| w == b"%%EOF"));
        assert_eq!(a, b);
        assert!(a.windows(18).any(|w| w.starts_with(b"/MediaBox [0 0 595")));
    }

    #[test]
    fn an_empty_examination_and_an_unavailable_ladder_render_calmly() {
        let mut q = QuickScreen::default();
        assert!(render_quick_screen(&q).starts_with(b"%PDF-"));
        q.sales.unavailable = true;
        assert!(render_quick_screen(&q).starts_with(b"%PDF-"));
    }

    #[test]
    fn templates_fill_in_order() {
        assert_eq!(
            fill(C1, &["8,1 %", "atteint"]),
            "1. Le taux de croissance passé des ventes (8,1 %) atteint l'objectif."
        );
        assert_eq!(fill(L_OLD, &["5"]), "(5) Ventes il y a 5 ans");
    }

    #[test]
    fn quick_screen_strings_are_neutral_no_banned_verb_no_wordmark() {
        use steadyinvest_core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR};
        for s in QUICK_SCREEN_USER_FACING {
            let lower = s.to_lowercase();
            for token in lower.split(|c: char| !c.is_alphanumeric()) {
                for banned in BANNED_VERBS_EN.iter().chain(BANNED_VERBS_FR.iter()) {
                    assert_ne!(token, banned.to_lowercase(), "{s:?} contains {banned:?}");
                }
            }
            for mark in [
                "NAIC",
                "Stock Selection Guide",
                "Better Investing",
                "SSG",
                "Stock Check List",
            ] {
                assert!(!s.contains(mark), "{s:?} carries {mark:?}");
            }
        }
    }
}

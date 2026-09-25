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
    /// Line (10) is absent because the old average (8) is zero or negative (G1 D review).
    pub nonpositive_base: bool,
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
/// `sales_meets` / `eps_meets` ∈ yes | no | no-rate | unread | "" (`""` = objective blank,
/// `no-rate` = the rate is absent, `unread` = the objective is not a number);
/// `eps_will_meet` ∈ yes | no | "". The §3 « bases » say which wording a fact may carry: « cinq
/// ans » only over the form's full five-year record (absence honesty, G1 review).
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
    /// five | all | partial | "" — the rows the P/E totals and averages cover: the form's five
    /// consecutive years, every row of a shorter record, some rows only, or none.
    pub pe_basis: String,
    /// How many rows the P/E figures cover, and how many the record holds.
    pub pe_years: String,
    pub record_years: String,
    pub present_price: String,
    pub present_eps: String,
    pub present_pe: String,
    pub high_five_years_ago: String,
    /// five | year | "" — the high is the form's « il y a cinq ans », the named `high_year`, or
    /// absent.
    pub high_basis: String,
    pub high_year: String,
    pub price_vs_high_pct: String,
    /// higher | lower | same | "" — decided on the DISPLAYED percentage, passed as a key (never
    /// parsed back from the formatted string).
    pub price_vs_high: String,
    pub years_sold_as_high: String,
    /// five | count | "" — « des cinq dernières années », over `sold_of` rows with a known high,
    /// or absent (no present price, no known high).
    pub sold_basis: String,
    pub sold_of: String,
    pub pe_position: String,
    /// Why `pe_position` is absent: pe (present P/E) | average (the record's) | both | "".
    pub pe_absent: String,
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
const L_INCREASE_PCT_NONPOS: &str = "(10) Hausse en pour cent, (9) ÷ (8) : base non positive";
const E_RECENT: &str = "(1) BPA de l'année la plus récente";
const E_RECENT_PRIOR: &str = "(2) BPA de l'année précédente";
const E_OLD: &str = "(5) BPA il y a {} ans";
const E_OLD_PRIOR: &str = "(6) BPA il y a {} ans";
const RATE_SALES: &str = "Taux annuel composé de croissance des ventes";
const RATE_EPS: &str = "Taux annuel composé de croissance du BPA";
const SPAN_NOTE: &str = "Base : deux moyennes de deux ans, à {} ans d'écart (formulaire : 5).";
const UNAVAILABLE: &str =
    "indisponible (les années exploitables ne forment pas deux paires consécutives)";
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
const AVG_OF_AVGS_N: &str = "Moyenne des PER moyens haut et bas sur {} années du relevé";
const AVG_OF_AVGS_1: &str = "Moyenne des PER moyens haut et bas sur la seule année du relevé";
const AVG_OF_AVGS_NONE: &str = "Moyenne des PER moyens haut et bas";
const PE_PARTIAL: &str = "Totaux et moyennes des PER sur {} des {} années du relevé ; un cours ou un BPA absent ou non positif ne donne pas de PER.";
const PRICE_VS_HIGH: &str = "Cours actuel par rapport au cours haut d'il y a cinq ans ({}) : {}";
const PRICE_VS_HIGH_YEAR: &str = "Cours actuel par rapport au cours haut de {} ({}) : {}";
const PRICE_VS_HIGH_NONE: &str = "Cours actuel par rapport au cours haut du début du relevé : {}";
const HIGHER: &str = "plus haut";
const LOWER: &str = "plus bas";
const SAME_LEVEL: &str = "au même niveau";
const SOLD_AS_HIGH: &str = "L'action s'est vendue aussi haut que le cours actuel au cours de {} des cinq dernières années.";
const SOLD_AS_HIGH_N: &str = "L'action s'est vendue aussi haut que le cours actuel au cours de {} des {} années dont le cours haut est connu.";
const SOLD_AS_HIGH_ONE_YES: &str = "L'action s'est vendue aussi haut que le cours actuel lors de la seule année dont le cours haut est connu.";
const SOLD_AS_HIGH_ONE_NO: &str = "L'action ne s'est pas vendue aussi haut que le cours actuel lors de la seule année dont le cours haut est connu.";
const SOLD_AS_HIGH_NONE: &str =
    "Années où l'action s'est vendue aussi haut que le cours actuel : {}";
const PE_POS: &str = "Le PER actuel ({}) est {} de la moyenne des cinq ans ({}).";
const PE_POS_N: &str = "Le PER actuel ({}) est {} de la moyenne des {} années du relevé ({}).";
const PE_POS_1: &str = "Le PER actuel ({}) est {} du PER moyen de la seule année du relevé ({}).";
const PE_POS_NONE: &str = "Le PER actuel ({}) par rapport à la moyenne des PER du relevé ({}) : {}";
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
const C1_NO_RATE: &str =
    "1. Le taux de croissance passé des ventes est indisponible : pas de comparaison à l'objectif.";
const C2_NO_RATE: &str =
    "2. Le taux de croissance passé du BPA est indisponible : pas de comparaison à l'objectif.";
const C1_UNREAD: &str =
    "1. Le taux de croissance passé des ventes ({}) : objectif non reconnu comme un pourcentage.";
const C2_UNREAD: &str =
    "2. Le taux de croissance passé du BPA ({}) : objectif non reconnu comme un pourcentage.";
const C3: &str = "3. Croissance possible du BPA sur cinq ans, avis du lecteur : {}";
const C4: &str = "4. Le cours : PER actuel {} de la norme des cinq ans.";
const C4_N: &str = "4. Le cours : PER actuel {} de la norme des {} années du relevé.";
const C4_1: &str = "4. Le cours : PER actuel {} de la norme de la seule année du relevé.";
const C4_NO_PE: &str = "4. Le cours : PER actuel indisponible.";
const C4_NO_AVG: &str = "4. Le cours : moyenne des PER du relevé indisponible.";
const C4_NO_BOTH: &str = "4. Le cours : PER actuel et moyenne des PER du relevé indisponibles.";
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
    L_INCREASE_PCT_NONPOS,
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
    AVG_OF_AVGS_N,
    AVG_OF_AVGS_1,
    AVG_OF_AVGS_NONE,
    PE_PARTIAL,
    PRICE_VS_HIGH,
    PRICE_VS_HIGH_YEAR,
    PRICE_VS_HIGH_NONE,
    HIGHER,
    LOWER,
    SAME_LEVEL,
    SOLD_AS_HIGH,
    SOLD_AS_HIGH_N,
    SOLD_AS_HIGH_ONE_YES,
    SOLD_AS_HIGH_ONE_NO,
    SOLD_AS_HIGH_NONE,
    PE_POS,
    PE_POS_N,
    PE_POS_1,
    PE_POS_NONE,
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
    C1_NO_RATE,
    C2_NO_RATE,
    C1_UNREAD,
    C2_UNREAD,
    C3,
    C4,
    C4_N,
    C4_1,
    C4_NO_PE,
    C4_NO_AVG,
    C4_NO_BOTH,
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
        // Line (10) is absent on a non-positive old average: its label says why (G1 D review).
        (
            if l.nonpositive_base {
                L_INCREASE_PCT_NONPOS.to_string()
            } else {
                labels[9].to_string()
            },
            get(9),
        ),
    ];
    for (label, value) in rows {
        doc.two_columns(&label, or_dash(value));
    }
    doc.two_columns(rate_label, or_dash(&l.rate));
    doc.small_line(&fill(SPAN_NOTE, &[span]));
}

fn price_vs_high_word(key: &str) -> Option<&'static str> {
    match key {
        "higher" => Some(HIGHER),
        "lower" => Some(LOWER),
        "same" => Some(SAME_LEVEL),
        _ => None,
    }
}

fn pe_word(q: &QuickScreen) -> Option<&'static str> {
    match q.pe_position.as_str() {
        "higher" => Some(PE_HIGHER),
        "similar" => Some(PE_SIMILAR),
        "lower" => Some(PE_LOWER),
        _ => None,
    }
}

/// The three §3 facts, worded from their keys and bases: the present price against the oldest
/// high (the word from the `price_vs_high` key), the years it sold as high, the present P/E
/// against the record's average. Each always has a line; an absent figure reads « — ».
fn price_facts(q: &QuickScreen) -> [String; 3] {
    let vs_high = match price_vs_high_word(&q.price_vs_high) {
        Some(word) if !q.price_vs_high_pct.is_empty() => {
            format!("{word} ({})", q.price_vs_high_pct)
        }
        _ => EM_DASH.to_string(),
    };
    let high = match q.high_basis.as_str() {
        "five" => fill(PRICE_VS_HIGH, &[or_dash(&q.high_five_years_ago), &vs_high]),
        "year" => fill(
            PRICE_VS_HIGH_YEAR,
            &[&q.high_year, or_dash(&q.high_five_years_ago), &vs_high],
        ),
        _ => fill(PRICE_VS_HIGH_NONE, &[EM_DASH]),
    };
    let sold = match q.sold_basis.as_str() {
        "five" => fill(SOLD_AS_HIGH, &[&q.years_sold_as_high]),
        // One known high: « de 1 des 1 années » would not read — the singular says it plainly.
        "count" if q.sold_of == "1" => match q.years_sold_as_high.as_str() {
            "1" => SOLD_AS_HIGH_ONE_YES.to_string(),
            _ => SOLD_AS_HIGH_ONE_NO.to_string(),
        },
        "count" => fill(SOLD_AS_HIGH_N, &[&q.years_sold_as_high, &q.sold_of]),
        _ => fill(SOLD_AS_HIGH_NONE, &[EM_DASH]),
    };
    let pe = match (pe_word(q), q.pe_basis.as_str()) {
        (Some(word), "five") => fill(PE_POS, &[&q.present_pe, word, &q.pe_avg_of_avgs]),
        (Some(word), _) if q.pe_years == "1" => {
            fill(PE_POS_1, &[&q.present_pe, word, &q.pe_avg_of_avgs])
        }
        (Some(word), _) => fill(
            PE_POS_N,
            &[&q.present_pe, word, &q.pe_years, &q.pe_avg_of_avgs],
        ),
        (None, _) => fill(
            PE_POS_NONE,
            &[or_dash(&q.present_pe), or_dash(&q.pe_avg_of_avgs), EM_DASH],
        ),
    };
    [high, sold, pe]
}

/// Conclusions 1 / 2 from the `*_meets` key: `[met-or-not, objective blank, rate absent,
/// objective unread]` — « objectif non renseigné » only when the objective IS blank.
fn conclusion(templates: [&str; 4], rate: &str, key: &str) -> String {
    let [with, blank, no_rate, unread] = templates;
    match key {
        "yes" => fill(with, &[or_dash(rate), MEETS]),
        "no" => fill(with, &[or_dash(rate), MISSES]),
        "no-rate" => no_rate.to_string(),
        "unread" => fill(unread, &[or_dash(rate)]),
        _ => fill(blank, &[or_dash(rate)]),
    }
}

/// Conclusion 4: the P/E fact in the form's words, or which figure is missing — the present
/// P/E, the record's average, or both (never « PER actuel » blamed for the average).
fn conclusion_4(q: &QuickScreen) -> String {
    match (pe_word(q), q.pe_basis.as_str(), q.pe_absent.as_str()) {
        (Some(word), "five", _) => fill(C4, &[word]),
        (Some(word), _, _) if q.pe_years == "1" => fill(C4_1, &[word]),
        (Some(word), _, _) => fill(C4_N, &[word, &q.pe_years]),
        (None, _, "average") => C4_NO_AVG.to_string(),
        (None, _, "both") => C4_NO_BOTH.to_string(),
        (None, _, _) => C4_NO_PE.to_string(),
    }
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

    // The form's two pages: §1–§2 on the first, §3–§4 on the second (spec §5, §7) — a break
    // only while still on the first page: when a long note already carried §2 onto page 2, §3
    // follows it there rather than jumping to a third (G1 D review).
    if doc.page_index() == 0 {
        doc.new_page();
    }
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
    if q.pe_basis == "partial" {
        doc.small_line(&fill(PE_PARTIAL, &[&q.pe_years, &q.record_years]));
    }
    let avg_label = match q.pe_basis.as_str() {
        "five" => AVG_OF_AVGS.to_string(),
        "all" | "partial" if q.pe_years == "1" => AVG_OF_AVGS_1.to_string(),
        "all" | "partial" => fill(AVG_OF_AVGS_N, &[&q.pe_years]),
        _ => AVG_OF_AVGS_NONE.to_string(),
    };
    doc.two_columns(&avg_label, or_dash(&q.pe_avg_of_avgs));
    // The three §3 facts always print — « — » where a figure is absent (spec §6), never vanish.
    for fact in price_facts(q) {
        doc.line(&fact);
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
    doc.line(&conclusion(
        [C1, C1_NONE, C1_NO_RATE, C1_UNREAD],
        &q.sales.rate,
        &q.sales_meets,
    ));
    doc.line(&conclusion(
        [C2, C2_NONE, C2_NO_RATE, C2_UNREAD],
        &q.eps.rate,
        &q.eps_meets,
    ));
    let will = match q.eps_will_meet.as_str() {
        "yes" => YES,
        "no" => NO,
        _ => NOT_FILLED,
    };
    doc.line(&fill(C3, &[will]));
    // Conclusion 4 always prints, with the screen's key and wording (G1 review).
    doc.line(&conclusion_4(q));
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
            nonpositive_base: false,
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
            pe_basis: "five".into(),
            pe_years: "5".into(),
            record_years: "5".into(),
            present_price: "110".into(),
            present_eps: "5".into(),
            present_pe: "22,0".into(),
            high_five_years_ago: "100".into(),
            high_basis: "five".into(),
            high_year: "2022".into(),
            price_vs_high_pct: "10,0 %".into(),
            price_vs_high: "higher".into(),
            years_sold_as_high: "1".into(),
            sold_basis: "five".into(),
            sold_of: "5".into(),
            pe_position: "higher".into(),
            pe_absent: String::new(),
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

    fn page_count(bytes: &[u8]) -> u32 {
        let at = bytes
            .windows(7)
            .position(|w| w == b"/Count ")
            .expect("a page tree with a /Count");
        bytes[at + 7..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .fold(0, |acc, b| acc * 10 + u32::from(b - b'0'))
    }

    /// Spec §5 / §7: the form's two pages — §1–§2, then §3–§4.
    #[test]
    fn the_examination_prints_on_the_forms_two_pages() {
        assert_eq!(page_count(&render_quick_screen(&sample())), 2);
        assert_eq!(page_count(&render_quick_screen(&QuickScreen::default())), 2);
    }

    #[test]
    fn without_a_price_the_three_facts_read_a_dash_and_never_vanish() {
        let mut q = sample();
        q.present_price.clear();
        q.present_pe.clear();
        q.price_vs_high_pct.clear();
        q.price_vs_high.clear();
        q.years_sold_as_high.clear();
        q.sold_basis.clear();
        q.pe_position.clear();
        q.pe_absent = "pe".into();
        let [high, sold, pe] = price_facts(&q);
        assert_eq!(
            high,
            "Cours actuel par rapport au cours haut d'il y a cinq ans (100) : —"
        );
        assert_eq!(
            sold,
            "Années où l'action s'est vendue aussi haut que le cours actuel : —"
        );
        assert_eq!(
            pe,
            "Le PER actuel (—) par rapport à la moyenne des PER du relevé (15,0) : —"
        );
        assert_eq!(conclusion_4(&q), C4_NO_PE);
    }

    #[test]
    fn the_higher_lower_word_comes_from_the_key_not_the_string() {
        let mut q = sample();
        let [high, ..] = price_facts(&q);
        assert!(high.ends_with(": plus haut (10,0 %)"), "{high}");
        // A formatted value the old code parsed back: the key decides, whatever the string.
        q.price_vs_high = "lower".into();
        q.price_vs_high_pct = "−40,2 %".into();
        assert!(price_facts(&q)[0].ends_with(": plus bas (−40,2 %)"));
        q.price_vs_high = "same".into();
        q.price_vs_high_pct = "0,0 %".into();
        assert!(price_facts(&q)[0].ends_with(": au même niveau (0,0 %)"));
    }

    #[test]
    fn a_short_record_names_its_years_never_five() {
        let mut q = sample();
        q.pe_basis = "partial".into();
        q.pe_years = "3".into();
        q.record_years = "4".into();
        q.high_basis = "year".into();
        q.high_year = "2023".into();
        q.sold_basis = "count".into();
        q.sold_of = "4".into();
        let facts = price_facts(&q);
        assert_eq!(
            facts[0],
            "Cours actuel par rapport au cours haut de 2023 (100) : plus haut (10,0 %)"
        );
        assert!(facts[1].contains("au cours de 1 des 4 années dont le cours haut est connu"));
        assert!(facts[2].contains("de la moyenne des 3 années du relevé (15,0)"));
        assert_eq!(
            conclusion_4(&q),
            "4. Le cours : PER actuel au-dessus de la norme des 3 années du relevé."
        );
        for f in facts {
            assert!(!f.contains("cinq"), "{f}");
        }
    }

    #[test]
    fn a_single_year_reads_in_the_singular() {
        let mut q = sample();
        q.pe_basis = "all".into();
        q.pe_years = "1".into();
        q.high_basis = "year".into();
        q.sold_basis = "count".into();
        q.sold_of = "1".into();
        q.years_sold_as_high = "0".into();
        let facts = price_facts(&q);
        assert_eq!(facts[1], SOLD_AS_HIGH_ONE_NO);
        assert_eq!(
            facts[2],
            "Le PER actuel (22,0) est au-dessus du PER moyen de la seule année du relevé (15,0)."
        );
        assert_eq!(
            conclusion_4(&q),
            "4. Le cours : PER actuel au-dessus de la norme de la seule année du relevé."
        );
        q.years_sold_as_high = "1".into();
        assert_eq!(price_facts(&q)[1], SOLD_AS_HIGH_ONE_YES);
        for f in price_facts(&q) {
            assert!(!f.contains(" 1 années") && !f.contains("des 1 "), "{f}");
        }
    }

    /// A long note that already carries §2 onto page 2: §3 follows it there, never on a third.
    #[test]
    fn a_long_note_does_not_push_section_3_to_a_third_page() {
        let mut q = sample();
        q.reasons = "une raison de la croissance passée, notée longuement. ".repeat(90);
        assert_eq!(page_count(&render_quick_screen(&q)), 2);
    }

    #[test]
    fn the_conclusions_name_what_is_missing() {
        let c1 = [C1, C1_NONE, C1_NO_RATE, C1_UNREAD];
        assert_eq!(
            conclusion(c1, "8,1 %", "yes"),
            "1. Le taux de croissance passé des ventes (8,1 %) atteint l'objectif."
        );
        assert_eq!(
            conclusion(c1, "", ""),
            "1. Le taux de croissance passé des ventes (—) : objectif non renseigné."
        );
        assert_eq!(conclusion(c1, "", "no-rate"), C1_NO_RATE);
        assert_eq!(
            conclusion(c1, "8,1 %", "unread"),
            "1. Le taux de croissance passé des ventes (8,1 %) : objectif non reconnu comme un pourcentage."
        );
        // Conclusion 4: the screen's key and wording, the missing figure named.
        let mut q = sample();
        assert_eq!(
            conclusion_4(&q),
            "4. Le cours : PER actuel au-dessus de la norme des cinq ans."
        );
        q.pe_position.clear();
        q.pe_absent = "average".into();
        assert_eq!(conclusion_4(&q), C4_NO_AVG);
        q.pe_absent = "both".into();
        assert_eq!(conclusion_4(&q), C4_NO_BOTH);
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

//! Story 7.1 (FR53) — the company comparison as a neutral, A4-LANDSCAPE PDF that reads in black
//! and white (colour only as a second channel — owner decision, Guy 2026-09-26): up to five
//! studies as columns, the comparison form's thirty rows in four groups. The `app` formats the
//! figures (its one float→string boundary) and passes keys for the two worded rows (the zone,
//! the study state); this module owns every label (its neutral inventory, tested).
//!
//! Owner decision (Guy, 2026-09-26), the NAIC Stock Comparison Guide's semantics: a row that
//! corresponds to a judged input (2, 4, 12, 14) shows the JUDGED value the calculation used, as
//! the app marked it (a trailing [`JUDGED_SIGIL`]); the sigil is explained under the group.

use crate::pdf::{
    Doc, EM_DASH, JUDGED_NOTE, JUDGED_SIGIL, MARGIN, SMALL, fit, judged, wrap_to_width,
};

/// One study's column: the header facts and the thirty rows (index 0 = the form's row 1).
/// Rows 20 and 28 are keys carried in `zone` / `state` instead; their string slot stays `""`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComparisonColumn {
    pub ticker: String,
    pub name: String,
    pub currency: String,
    /// The decision date (the study's creation), `YYYY-MM-DD`.
    pub date: String,
    /// The study could not be read — every row reads « indisponible ».
    pub unavailable: bool,
    /// The picked study no longer exists — every row reads « introuvable » (an absence, not a
    /// read failure: misattribution is a lie). Comes with `unavailable` (no figures either).
    pub missing: bool,
    /// The study reads but its frame does not compute — every row reads « non calculable »; the
    /// header keeps the study's facts. Comes with `unavailable` (no figures).
    pub uncomputable: bool,
    pub rows: Vec<String>,
    /// `buy` | `neutral` | `sell` | `below` | `above` | `""`.
    pub zone: String,
    /// `full` | `provisional` | `withheld` | `""`.
    pub state: String,
    pub low_confidence: bool,
    /// Rows 5 / 6: how many years this column's average actually runs over (`0` = no average
    /// shown) — the row label says that number, never « 5 ans » over three (G1, #237).
    pub ptp_avg_years: usize,
    pub roe_avg_years: usize,
}

/// Rows 5 / 6 (`row` = 5 or 6): the number of years every SHOWN average of the row runs over —
/// `Some(n)` when the columns agree (the label says « moyenne n ans »); `None` when no column
/// shows one or they differ (the label says « moyenne » and each cell names its own years).
pub fn average_years(columns: &[ComparisonColumn], row: usize) -> Option<usize> {
    let mut shown = columns
        .iter()
        .filter(|c| !c.unavailable)
        .map(|c| {
            if row == 5 {
                c.ptp_avg_years
            } else {
                c.roe_avg_years
            }
        })
        .filter(|n| *n > 0);
    let first = shown.next()?;
    shown.all(|n| n == first).then_some(first)
}

/// The comparison, ready to lay out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Comparison {
    pub date: String,
    /// The selected studies are not all in one currency — prices stay native (FR28).
    pub currency_mix: bool,
    pub columns: Vec<ComparisonColumn>,
}

// ── the neutral inventory (FR13) ──
const TITLE: &str = "Comparaison de sociétés";
const DATE: &str = "Date";
const CURRENCY_MIX: &str = "Les études comparées ne sont pas toutes dans la même monnaie : les cours restent dans la monnaie de chaque étude, sans conversion.";
const UNAVAILABLE: &str = "indisponible";
const MISSING: &str = "introuvable";
const UNCOMPUTABLE: &str = "non calculable";
const G_GROWTH: &str = "Croissance (section 1)";
const G_MANAGEMENT: &str = "Gestion (section 2)";
const G_PRICE: &str = "Cours (sections 3 à 5)";
const G_OTHER: &str = "Autres";
const ROWS: [&str; 30] = [
    "Croissance historique des ventes",
    "Croissance estimée des ventes",
    "Croissance historique du BPA",
    "Croissance estimée du BPA",
    // Rows 5 / 6 are worded by `row_label` (the years actually averaged).
    PTP_AVG,
    ROE_AVG,
    "Part du capital détenue par la direction",
    "BPA total estimé sur 5 ans",
    "Fourchette de cours sur 5 ans",
    "Cours actuel",
    "PER le plus haut",
    // Rows 12 / 14 show the judged average P/E the zones are computed with (owner decision,
    // 2026-09-26) — named so, as the study's §4 names it.
    "PER haut moyen jugé",
    "PER moyen",
    "PER bas moyen jugé",
    "PER le plus bas",
    "PER actuel",
    "Zone basse",
    "Zone médiane",
    "Zone haute",
    "Position du cours actuel",
    "Ratio hausse / baisse",
    "Rendement présent",
    "Rendement annuel total estimé",
    "Actions en circulation",
    "Dilution potentielle",
    "Taux de distribution moyen",
    "Signaux de qualité",
    "État de l'étude",
    "Date des données",
    "Place de cotation",
];
// Row 20 names the band of rows 17–19 in their own words — the screen says the same (G1, #237) —
// in lower case like every other worded cell (owner decision, 2026-09-26: « zone médiane »,
// « sous la bande », as « provisoire » or « critères validés » in row 28).
const ZONE_BUY: &str = "zone basse";
const ZONE_NEUTRAL: &str = "zone médiane";
const ZONE_SELL: &str = "zone haute";
const ZONE_BELOW: &str = "sous la bande";
const ZONE_ABOVE: &str = "au-dessus de la bande";
// Owner decision (Guy, 2026-09-26): rows 20–23 — where the current price sits, the ratio, the
// yields — are the comparison's conclusions, printed in bold (label and figures).
const BOLD_ROWS: std::ops::RangeInclusive<usize> = 20..=23;
// The Cours group breaks, when it must, between the P/E history (rows 8–16, sections 3) and the
// zones and returns (rows 17–23, sections 4–5) — never with row 23 alone on the next page.
const PRICE_TAIL: std::ops::RangeInclusive<usize> = 17..=23;
const STATE_FULL: &str = "critères validés";
const STATE_PROVISIONAL: &str = "provisoire";
const STATE_WITHHELD: &str = "en attente";
const LOW_CONFIDENCE: &str = "confiance réduite";
const EMPTY: &str = "Aucune étude sélectionnée.";
const FLAGS_TITLE: &str = "Signaux de qualité (ligne 27)";
// Rows 5 / 6 say the years their averages run over (G1, #237): `{n}` years, one year, or — when
// the columns differ — no number in the label (each cell names its own).
const PTP_AVG_N: &str = "Marge avant impôt, moyenne {n} ans · tendance";
const PTP_AVG_ONE: &str = "Marge avant impôt, moyenne 1 an · tendance";
const PTP_AVG: &str = "Marge avant impôt, moyenne · tendance";
const ROE_AVG_N: &str = "Rendement des capitaux propres, moyenne {n} ans · tendance";
const ROE_AVG_ONE: &str = "Rendement des capitaux propres, moyenne 1 an · tendance";
const ROE_AVG: &str = "Rendement des capitaux propres, moyenne · tendance";

#[cfg(test)]
const COMPARISON_USER_FACING: &[&str] = &[
    TITLE,
    DATE,
    CURRENCY_MIX,
    UNAVAILABLE,
    MISSING,
    UNCOMPUTABLE,
    G_GROWTH,
    G_MANAGEMENT,
    G_PRICE,
    G_OTHER,
    ZONE_BUY,
    ZONE_NEUTRAL,
    ZONE_SELL,
    ZONE_BELOW,
    ZONE_ABOVE,
    STATE_FULL,
    STATE_PROVISIONAL,
    STATE_WITHHELD,
    LOW_CONFIDENCE,
    EMPTY,
    FLAGS_TITLE,
    PTP_AVG_N,
    PTP_AVG_ONE,
    PTP_AVG,
    ROE_AVG_N,
    ROE_AVG_ONE,
    ROE_AVG,
    JUDGED_NOTE,
];

/// The label of row `n` (1-based): the fixed inventory, but rows 5 / 6 say the years averaged.
fn row_label(columns: &[ComparisonColumn], n: usize) -> String {
    let (many, one, none) = match n {
        5 => (PTP_AVG_N, PTP_AVG_ONE, PTP_AVG),
        6 => (ROE_AVG_N, ROE_AVG_ONE, ROE_AVG),
        _ => return ROWS[n - 1].to_string(),
    };
    match average_years(columns, n) {
        Some(1) => one.to_string(),
        Some(years) => many.replace("{n}", &years.to_string()),
        None => none.to_string(),
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

fn state_label(c: &ComparisonColumn) -> String {
    let base = match c.state.as_str() {
        "full" => STATE_FULL,
        "provisional" => STATE_PROVISIONAL,
        "withheld" => STATE_WITHHELD,
        _ => EM_DASH,
    };
    if c.low_confidence && base != EM_DASH {
        format!("{base} · {LOW_CONFIDENCE}")
    } else {
        base.to_string()
    }
}

/// Row 27 arrives as « N : flag · flag », « 0 » (every rule evaluated, none raised) or `""` (not
/// assessable — the em-dash): the count before the colon is the cell; the words after it are
/// listed under the grids.
fn flag_count(row: &str) -> &str {
    row.split_once(" : ").map(|(n, _)| n).unwrap_or(row).trim()
}
fn flag_words(c: &ComparisonColumn) -> Option<String> {
    let row = c.rows.get(26)?;
    let (_, words) = row.split_once(" : ")?;
    Some(words.trim().to_string())
}

/// A column's identity as its header states it — « TICKER (CUR) · date » (the ticker carries the
/// « · n » of two studies of one ticker, currency and day). Two columns of one ticker are two
/// studies: the flag list names each by this, never by the bare ticker (G1, #237).
fn column_identity(c: &ComparisonColumn) -> String {
    let head = header_line(c);
    if c.date.is_empty() {
        head
    } else {
        format!("{head} · {}", c.date)
    }
}

/// The flag list under the grids: one line per column with raised flags, in column order, each
/// named by its own column's identity.
fn flag_list(cols: &[ComparisonColumn]) -> Vec<String> {
    cols.iter()
        .filter(|c| !c.unavailable && !c.missing)
        .filter_map(|c| flag_words(c).map(|w| format!("{} : {w}", column_identity(c))))
        .collect()
}

/// The cell of row `n` (1-based) for a column: the figure, the worded key rows, or the absence.
fn cell(c: &ComparisonColumn, n: usize) -> String {
    if c.missing {
        return MISSING.to_string();
    }
    if c.uncomputable {
        return UNCOMPUTABLE.to_string();
    }
    if c.unavailable {
        return UNAVAILABLE.to_string();
    }
    match n {
        20 => zone_label(&c.zone).to_string(),
        27 => c
            .rows
            .get(26)
            .map(|r| flag_count(r))
            .filter(|s| !s.is_empty())
            .unwrap_or(EM_DASH)
            .to_string(),
        28 => state_label(c),
        _ => c
            .rows
            .get(n - 1)
            .filter(|s| !s.is_empty())
            .map(|s| strip_arrows(s))
            .unwrap_or_else(|| EM_DASH.to_string()),
    }
}

/// A cell as printed: a judged figure (the app's trailing [`JUDGED_SIGIL`]) takes the judged
/// colour beside its sigil; any other cell is printed as is.
fn printed(cell: &str) -> String {
    match cell.strip_suffix(JUDGED_SIGIL) {
        Some(figure) if !figure.is_empty() => judged(figure),
        _ => cell.to_string(),
    }
}

/// The trend rows (5, 6) arrive as « 47,6 % · ↑ hausse »: the word carries the fact, the arrow
/// has no glyph in the PDF's WinAnsi font, so it is dropped here (never rendered as « ? »).
fn strip_arrows(s: &str) -> String {
    s.replace("↑ ", "").replace("↓ ", "").replace("→ ", "")
}

/// The first header line: « TICKER (CUR) », or the bare ticker when there is no currency to name.
fn header_line(c: &ComparisonColumn) -> String {
    if c.currency.is_empty() {
        c.ticker.clone()
    } else {
        format!("{} ({})", c.ticker, c.currency)
    }
}

/// The second header line: « date · name », either alone when the other is absent; a column
/// with no figures names its state (« date · non calculable » keeps the study's date).
fn second_header_line(c: &ComparisonColumn) -> String {
    if c.missing {
        return MISSING.to_string();
    }
    if c.uncomputable {
        return if c.date.is_empty() {
            UNCOMPUTABLE.to_string()
        } else {
            format!("{} · {UNCOMPUTABLE}", c.date)
        };
    }
    if c.unavailable {
        return UNAVAILABLE.to_string();
    }
    match (c.date.is_empty(), c.name.is_empty()) {
        (false, false) => format!("{} · {}", c.date, c.name),
        (false, true) => c.date.clone(),
        (true, false) => c.name.clone(),
        (true, true) => String::new(),
    }
}

/// Render the comparison (FR53): A4 landscape, deterministic, black-and-white-safe, neutral
/// labels.
pub fn render_comparison(comparison: &Comparison) -> Vec<u8> {
    let mut doc = Doc::landscape();
    doc.title(TITLE);
    doc.small_line(&format!("{DATE} : {}", comparison.date));
    if comparison.currency_mix {
        doc.small_line(CURRENCY_MIX);
    }
    doc.gap(4.0);
    let cols = &comparison.columns;
    if cols.is_empty() {
        doc.line(EMPTY);
        return doc.finish();
    }
    // Column edges: the label column, then one column per study up to the right margin.
    let label_w = 236.0;
    let n = cols.len().max(1);
    let col_w = (doc.right() - MARGIN - label_w) / n as f32;
    let mut edges = vec![MARGIN, MARGIN + label_w];
    for i in 1..=n {
        edges.push(MARGIN + label_w + col_w * i as f32);
    }
    // Two header rows: « TICKER (CUR) » then « decision date · company name » (G1 decision 9b:
    // the date as on screen, first so an ellipsis never eats it), cut to its column at the real
    // Helvetica widths (an over-long name ends with an ellipsis, never spills into the
    // neighbour's column; the grid wraps any other over-long cell inside its own). A column
    // with no study behind it names its state instead — never a dangling « () ».
    let name_w = col_w - 10.0;
    let header: Vec<String> = std::iter::once(String::new())
        .chain(cols.iter().map(header_line))
        .collect();
    let header_refs: Vec<&str> = header.iter().map(String::as_str).collect();
    let names: Vec<String> = std::iter::once(String::new())
        .chain(
            cols.iter()
                .map(|c| fit(&second_header_line(c), name_w, SMALL)),
        )
        .collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let any_name = names.iter().any(|n| !n.is_empty());
    let groups: [(&str, std::ops::RangeInclusive<usize>); 4] = [
        (G_GROWTH, 1..=4),
        (G_MANAGEMENT, 5..=7),
        (G_PRICE, 8..=23),
        (G_OTHER, 24..=30),
    ];
    let row_cells = |row: usize| -> Vec<String> {
        let mut cells = vec![format!("({row}) {}", row_label(cols, row))];
        cells.extend(cols.iter().map(|c| printed(&cell(c, row))));
        cells
    };
    for (title, range) in groups {
        doc.section(title);
        doc.grid_begin(range.end() - range.start() + 3);
        doc.grid_row_small(&header_refs, &edges, true, 1);
        if any_name {
            doc.grid_row_small(&name_refs, &edges, true, 1);
        }
        // The judged-value note follows its sigils: printed under each page's portion of the
        // grid that holds a judged cell (a group split by a page break explains it on both).
        doc.set_grid_note(JUDGED_NOTE);
        for row in range {
            let marked = cols.iter().any(|c| cell(c, row).ends_with(JUDGED_SIGIL));
            let cells = row_cells(row);
            let refs: Vec<&str> = cells.iter().map(String::as_str).collect();
            if row == *PRICE_TAIL.start() {
                // Rows 17–23 stay together: they break to the next page as one block (with the
                // replayed header) rather than leave their last row alone there.
                let tail: f32 = PRICE_TAIL
                    .clone()
                    .map(|r| {
                        let cells = row_cells(r);
                        let refs: Vec<&str> = cells.iter().map(String::as_str).collect();
                        doc.grid_rows_height(&refs, &edges, SMALL)
                    })
                    .sum();
                doc.grid_keep_rows(tail, &edges);
            }
            doc.set_grid_bold(BOLD_ROWS.contains(&row));
            doc.grid_row_small(&refs, &edges, false, 1);
            doc.set_grid_bold(false);
            if marked {
                doc.grid_note_due();
            }
        }
        doc.grid_end(&edges);
        doc.gap(2.0);
    }
    // Row 27 in the grid carries the count only; the flags themselves are listed here, one
    // paragraph per study, wrapped to the page (a flag's words never spill past the margin).
    let listed = flag_list(cols);
    if !listed.is_empty() {
        doc.section(FLAGS_TITLE);
        let width = doc.right() - MARGIN;
        for line in listed {
            for chunk in wrap_to_width(&line, width, SMALL) {
                doc.small_line(&chunk);
            }
        }
    }
    doc.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(ticker: &str, unavailable: bool) -> ComparisonColumn {
        ComparisonColumn {
            ticker: ticker.into(),
            name: "Société".into(),
            currency: "CHF".into(),
            date: "2026-09-24".into(),
            unavailable,
            rows: (1..=30).map(|i| format!("v{i}")).collect(),
            zone: "buy".into(),
            state: "provisional".into(),
            low_confidence: true,
            ..ComparisonColumn::default()
        }
    }

    #[test]
    fn renders_a_landscape_deterministic_pdf() {
        let c = Comparison {
            date: "2026-09-24".into(),
            currency_mix: true,
            columns: vec![column("NESN.SW", false), column("ROG.SW", true)],
        };
        let a = render_comparison(&c);
        let b = render_comparison(&c);
        assert!(a.starts_with(b"%PDF-") && a.windows(5).any(|w| w == b"%%EOF"));
        assert_eq!(a, b, "deterministic bytes");
        // A4 landscape: the media box is 842 wide × 595 high.
        assert!(
            a.windows(18).any(|w| w.starts_with(b"/MediaBox [0 0 842")),
            "landscape media box"
        );
    }

    #[test]
    fn the_key_rows_word_themselves_and_an_unavailable_column_says_so() {
        let c = column("X", false);
        assert_eq!(cell(&c, 20), ZONE_BUY);
        assert_eq!(
            cell(&c, 28),
            format!("{STATE_PROVISIONAL} · {LOW_CONFIDENCE}")
        );
        assert_eq!(cell(&c, 1), "v1");
        let u = column("Y", true);
        assert_eq!(cell(&u, 1), UNAVAILABLE);
        assert_eq!(cell(&u, 20), UNAVAILABLE);
        // An empty figure is the em-dash, never a blank cell.
        let mut e = column("Z", false);
        e.rows[9] = String::new();
        assert_eq!(cell(&e, 10), EM_DASH);
    }

    #[test]
    fn row_27_keeps_the_count_in_the_cell_and_lists_the_words_below() {
        let mut c = column("X", false);
        c.rows[26] =
            "3 : PER haut jugé au-dessus de la moyenne · ratio sous la cible · marge en baisse"
                .into();
        assert_eq!(cell(&c, 27), "3");
        assert_eq!(
            flag_words(&c).as_deref(),
            Some("PER haut jugé au-dessus de la moyenne · ratio sous la cible · marge en baisse")
        );
        c.rows[26] = "0".into();
        assert_eq!(cell(&c, 27), "0");
        assert_eq!(flag_words(&c), None);
        // Not assessable: the em-dash, never « 0 ».
        c.rows[26] = String::new();
        assert_eq!(cell(&c, 27), EM_DASH);
        let mut t = column("T", false);
        t.rows[4] = "47,6 % · ↑ hausse".into();
        assert_eq!(cell(&t, 5), "47,6 % · hausse");
    }

    #[test]
    fn row_20_uses_the_band_rows_own_nouns() {
        // The screen words row 20 with the same nouns (comparison.slint `zone-words`), in lower
        // case like the other worded cells (owner decision, 2026-09-26).
        let mut c = column("X", false);
        for (key, row) in [("buy", 17), ("neutral", 18), ("sell", 19)] {
            c.zone = key.into();
            assert_eq!(cell(&c, 20), ROWS[row - 1].to_lowercase());
        }
        for key in ["below", "above"] {
            c.zone = key.into();
            let word = cell(&c, 20);
            assert_eq!(word, word.to_lowercase(), "{word}");
        }
        c.zone = String::new();
        assert_eq!(cell(&c, 20), EM_DASH);
    }

    #[test]
    fn a_missing_study_is_not_worded_as_a_read_failure() {
        let mut m = column("NESN.SW", false);
        m.missing = true;
        assert_eq!(cell(&m, 1), MISSING);
        assert_eq!(cell(&m, 28), MISSING);
        assert_ne!(MISSING, UNAVAILABLE);
    }

    #[test]
    fn the_headers_carry_the_decision_date_and_never_dangle() {
        let c = column("NESN.SW", false);
        assert_eq!(header_line(&c), "NESN.SW (CHF)");
        assert_eq!(second_header_line(&c), "2026-09-24 · Société");
        let mut nameless = column("NESN.SW", false);
        nameless.name = String::new();
        assert_eq!(second_header_line(&nameless), "2026-09-24");
        // An unavailable column carries no currency / date / name: no « () », no « · ».
        let u = ComparisonColumn {
            ticker: "ROG.SW".into(),
            unavailable: true,
            rows: vec![String::new(); 30],
            ..ComparisonColumn::default()
        };
        assert_eq!(header_line(&u), "ROG.SW");
        assert_eq!(second_header_line(&u), UNAVAILABLE);
        let m = ComparisonColumn {
            missing: true,
            ..u.clone()
        };
        assert_eq!(second_header_line(&m), MISSING);
        // A study that reads but does not compute keeps its facts and names its own state.
        let mut x = column("NESN.SW", true);
        x.uncomputable = true;
        assert_eq!(header_line(&x), "NESN.SW (CHF)");
        assert_eq!(second_header_line(&x), "2026-09-24 · non calculable");
        assert_eq!(cell(&x, 1), UNCOMPUTABLE);
        assert_ne!(UNCOMPUTABLE, UNAVAILABLE);
    }

    #[test]
    fn two_columns_of_one_ticker_list_their_flags_apart() {
        // Two studies of AAPL.US, in two currencies, then in one currency on two days.
        let mut a = column("AAPL.US", false);
        a.currency = "USD".into();
        a.rows[26] = "1 : marge en baisse".into();
        let mut b = column("AAPL.US", false);
        b.rows[26] = "1 : ratio sous la cible".into();
        let listed = flag_list(&[a.clone(), b.clone()]);
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0], "AAPL.US (USD) · 2026-09-24 : marge en baisse");
        assert_eq!(
            listed[1],
            "AAPL.US (CHF) · 2026-09-24 : ratio sous la cible"
        );
        b.currency = "USD".into();
        b.date = "2026-09-25".into();
        let listed = flag_list(&[a.clone(), b.clone()]);
        assert_ne!(
            listed[0].split(" : ").next(),
            listed[1].split(" : ").next(),
            "each line names its own column"
        );
        // Same ticker, currency and day: the header's « · n » tells them apart.
        let (mut x, mut y) = (a.clone(), a);
        x.ticker = "AAPL.US · 1".into();
        y.ticker = "AAPL.US · 2".into();
        let listed = flag_list(&[x, y]);
        assert!(listed[0].starts_with("AAPL.US · 1 (USD)"));
        assert!(listed[1].starts_with("AAPL.US · 2 (USD)"));
    }

    #[test]
    fn rows_5_and_6_say_the_years_actually_averaged() {
        let mut a = column("A", false);
        let mut b = column("B", false);
        (a.ptp_avg_years, a.roe_avg_years) = (5, 3);
        (b.ptp_avg_years, b.roe_avg_years) = (5, 4);
        let cols = [a.clone(), b.clone()];
        assert_eq!(average_years(&cols, 5), Some(5));
        assert_eq!(
            row_label(&cols, 5),
            "Marge avant impôt, moyenne 5 ans · tendance"
        );
        // The columns differ: no number in the label (each cell names its own years).
        assert_eq!(average_years(&cols, 6), None);
        assert_eq!(row_label(&cols, 6), ROE_AVG);
        // Three years everywhere: « moyenne 3 ans », never « 5 ans ».
        b.roe_avg_years = 3;
        assert_eq!(
            row_label(&[a.clone(), b.clone()], 6),
            "Rendement des capitaux propres, moyenne 3 ans · tendance"
        );
        // A column with no average, or no figures, does not vote.
        b.roe_avg_years = 0;
        let mut u = column("U", true);
        u.roe_avg_years = 1;
        assert_eq!(average_years(&[a.clone(), b, u], 6), Some(3));
        a.ptp_avg_years = 1;
        assert_eq!(row_label(&[a], 5), PTP_AVG_ONE);
        assert_eq!(row_label(&[], 5), PTP_AVG);
        assert_eq!(row_label(&[], 7), ROWS[6]);
    }

    /// The PDF's pages, as raw content streams, in order.
    fn pages(bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut rest = bytes;
        while let Some(end) = rest.windows(9).position(|w| w == b"endstream") {
            out.push(rest[..end].to_vec());
            rest = &rest[end + 9..];
        }
        out
    }

    fn carries(hay: &[u8], s: &str) -> bool {
        let hex: Vec<u8> = crate::pdf::winansi_for_tests(s)
            .iter()
            .flat_map(|b| format!("{b:02X}").into_bytes())
            .collect();
        let raw = crate::pdf::winansi_for_tests(s);
        hay.windows(raw.len()).any(|w| w == raw.as_slice())
            || hay.windows(hex.len()).any(|w| w == hex.as_slice())
    }

    #[test]
    fn judged_rows_are_marked_their_note_printed_and_the_return_rows_bold() {
        // Owner decision (Guy, 2026-09-26): rows 12 / 14 are the judged P/E, named « jugé »; a
        // judged cell keeps its sigil, takes the judged colour, and the group states the note;
        // rows 20–23 are bold.
        assert_eq!(ROWS[11], "PER haut moyen jugé");
        assert_eq!(ROWS[13], "PER bas moyen jugé");
        let mut a = column("A", false);
        a.rows[11] = "78,0*".into();
        assert_eq!(cell(&a, 12), "78,0*");
        assert_eq!(printed(&cell(&a, 12)), judged("78,0"));
        assert_eq!(printed("—"), "—");
        let bytes = render_comparison(&Comparison {
            date: "2026-09-26".into(),
            currency_mix: false,
            columns: vec![a, column("B", false)],
        });
        assert!(carries(&bytes, JUDGED_NOTE));
        assert!(carries(&bytes, "78,0*"));
        // Bold: the return rows' labels are shown in F1 (Helvetica-Bold).
        let text = String::from_utf8_lossy(&bytes);
        let bold_label = text
            .split("/F1 ")
            .skip(1)
            .any(|chunk| chunk.contains("(20) Position du cours actuel"));
        assert!(bold_label, "row 20 in bold");
        let regular_label = text.split("/F0 ").skip(1).any(|chunk| {
            chunk
                .split("ET")
                .next()
                .unwrap_or("")
                .contains("(19) Zone haute")
        });
        assert!(regular_label, "row 19 in the regular face");
    }

    #[test]
    fn the_price_group_never_leaves_its_last_row_alone_on_the_next_page() {
        // Three columns with two-line headers — the owner's case: rows 17–23 move together.
        let mut cols = vec![column("NESN.SW", false), column("NVDA.US", false)];
        cols.push(column("SCHN.SW", false));
        let bytes = render_comparison(&Comparison {
            date: "2026-09-26".into(),
            currency_mix: false,
            columns: cols,
        });
        let pages = pages(&bytes);
        let page_of = |s: &str| pages.iter().position(|p| carries(p, s));
        let p17 = page_of("(17) Zone basse").unwrap();
        let p23 = page_of("(23) Rendement annuel total estimé").unwrap();
        assert_eq!(p17, p23, "rows 17–23 share a page");
        // …and when the block moves, several rows follow the replayed header.
        if p17 > 0 {
            assert!(page_of("(16) PER actuel").unwrap() < p17);
        }
    }

    #[test]
    fn an_empty_comparison_renders_calmly() {
        let bytes = render_comparison(&Comparison::default());
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn comparison_strings_are_neutral_no_banned_verb_no_wordmark() {
        use steadyinvest_core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR};
        for s in COMPARISON_USER_FACING
            .iter()
            .copied()
            .chain(ROWS.iter().copied())
        {
            let lower = s.to_lowercase();
            for token in lower.split(|c: char| !c.is_alphanumeric()) {
                for banned in BANNED_VERBS_EN.iter().chain(BANNED_VERBS_FR.iter()) {
                    assert_ne!(token, banned.to_lowercase(), "{s:?} contains {banned:?}");
                }
            }
            for mark in ["NAIC", "Stock Selection Guide", "Better Investing", "SSG"] {
                assert!(!s.contains(mark), "{s:?} carries {mark:?}");
            }
        }
    }
}

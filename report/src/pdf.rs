//! The faithful, neutral, greyscale-safe study PDF (Story 5.6, FR52).
//!
//! Lays out one study's five SSG sections with `pdf-writer` — a low-level, write-only PDF byte
//! writer (precise coordinate control for the faithful grid; no PDF parser, so no untrusted-parse
//! advisory). The figures come from the SINGLE construction path [`crate::form::build_frame`], so the
//! PDF cannot drift from the on-screen form. Discipline carried from the UI:
//!
//! - **Neutral labels only** (no NAIC marks/logos or verbatim instructional text — open-source
//!   constraint); the zone nouns mirror the app's neutral set ("Zone basse/médiane/haute").
//! - **All sections expanded** (a PDF has no collapsibles).
//! - **Greyscale only** (NFR-U3): nothing reads by colour — text + position + line weight, never hue.
//!   The renderer emits only black/grey strokes and black text.
//! - **`None` → the faithful em-dash**, never `0` (the project's most-repeated rail).
//! - **Deterministic bytes**: no timestamp / file-id / random — the same study renders identically
//!   (so a fixture's bytes are testable).

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use steadyinvest_contract::Study;
use steadyinvest_core::method::{FORECAST_HORIZON_YEARS, USABLE_YEARS_FLOOR};
use steadyinvest_core::normalize::{CanonicalYear, NormalizeError};
use steadyinvest_core::rounding::{DisplayField, round_for_display};
use steadyinvest_core::ssg::{Trend, UpsideDownside, Zone, ZoneBounds};

use pdf_writer::{Content, Name, Pdf, Rect, Ref, Str};

/// A study could not be rendered to a PDF. Neutral, cause-named — never a panic.
#[derive(Debug)]
pub enum ReportError {
    /// The study's inputs did not normalize (the same cause the live form surfaces).
    Normalize(NormalizeError),
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Neutral, no provider/path leak; the form is simply not computable as entered.
            ReportError::Normalize(_) => f.write_str("l'étude ne peut pas être mise en forme"),
        }
    }
}

impl std::error::Error for ReportError {}

// ── page geometry (A4 portrait, points) ──
pub(crate) const PAGE_W: f32 = 595.0;
pub(crate) const PAGE_H: f32 = 842.0;
pub(crate) const MARGIN: f32 = 42.0;
pub(crate) const FONT: f32 = 9.0;
pub(crate) const SMALL: f32 = 7.5;
const TITLE_FONT: f32 = 15.0;
pub(crate) const HEAD_FONT: f32 = 11.0;
pub(crate) const LINE_H: f32 = 13.0;
pub(crate) const BOTTOM: f32 = MARGIN + 24.0; // keep clear of the footer disclaimer

// ── chart geometry (issue #105 — vector graphics into the PDF, greyscale-safe; issue #207 — the
//    §1 plot fills the rest of page 1, as on the printed form) ──
const CHART_MIN_H: f32 = 150.0; // the §1 plot never gets shorter than this (points)
const CHART_AXIS_W: f32 = 30.0; // left gutter for the y-axis decade labels
const ZONEBAR_H: f32 = 26.0; // §4 zone bar height (points)
const ZONEBAR_H_RESERVE: f32 = ZONEBAR_H + 2.0 * LINE_H + 12.0; // the bar + its labels, as reserved
const SECTION_H: f32 = HEAD_FONT + LINE_H; // the advance of one section heading
const SERIES_PAD_DECADES: f64 = 0.12; // per-series head/foot room (issue #25)
const MIN_SERIES_DECADES: f64 = 0.6; // a flat series still gets this much span (no false drama)
const YEAR_PAD: f64 = 0.5; // room (in years) at each end of the §1 x axis — no bar on the frame
// Issue #207: the growth guide lines of the printed form — compound rates from the last EPS point.
const GUIDE_RATES_PCT: [u32; 6] = [5, 10, 15, 20, 25, 30];
// The form's quarterly box, under the plot (owner decision 7): size and the space around it.
const QUARTER_BOX_W: f32 = 200.0;
const QUARTER_BOX_H: f32 = 4.0 * (SMALL + 3.0) + 8.0;
const QUARTER_BOX_GAP: f32 = 4.0;

// ── grid tables (issue #104 — visible SSG grid) ──
const CELL_PAD: f32 = 5.0; // left/right padding of text inside a grid cell
const GRID_INSET: f32 = 1.5; // the least clearance a cell's text keeps from its rules
// Column boundaries (left … right) for the annexe table (year + seven figures).
pub(crate) const COLS8: [f32; 9] = [
    MARGIN,
    MARGIN + 42.0,
    MARGIN + 112.0,
    MARGIN + 182.0,
    MARGIN + 240.0,
    MARGIN + 300.0,
    MARGIN + 360.0,
    MARGIN + 428.0,
    PAGE_W - MARGIN,
];
// Issue #207: the §3 price–earnings table (year + the form's eight columns A–H).
const COLS9: [f32; 10] = [
    MARGIN,
    MARGIN + 46.0, // « Moyenne » (37 pt at 9 pt) fits between the rules
    MARGIN + 100.0,
    MARGIN + 160.0,
    MARGIN + 216.0,
    MARGIN + 272.0,
    MARGIN + 328.0,
    MARGIN + 392.0,
    MARGIN + 452.0,
    PAGE_W - MARGIN,
];
const RULE_GRAY: f32 = 0.35; // the default rule/grid grey (restored after a chart)
const GRID_GRAY: f32 = 0.75; // faint decade gridlines
const GUIDE_GRAY: f32 = 0.82; // the growth guide lines (lighter than the grid)
const SERIES_GRAY: f32 = 0.0; // series strokes (black; told apart by weight + dash, never hue)

/// Render a study to a faithful, neutral, greyscale PDF (FR52). Read-only: it computes nothing the
/// engine does not already compute, writes no journal, and needs no provider.
///
/// Issue #207 — the layout follows the printed two-page form: page 1 = the header block + the
/// full-page §1 semi-log plot with its four growth lines; page 2 = §2 (years as columns), §3 (the
/// eight columns A–H over the window, totals and averages, current P/E), §4 (the high price, the
/// four low-price candidates and the one retained, the zoning, the upside/downside ratio, the price
/// target), §5 (present yield, average yield, the total return); then the synthesis; then an annexe
/// with every historical figure the form plots but does not tabulate.
///
/// G1 I — every figure is spelled in the reader's number format (`numbers`: « 128,9 » and
/// « 1 234,5 » under [`NumberStyle::Comma`], « 128.9 » and « 1,234.5 » under
/// [`NumberStyle::Point`]), as the app shows it; the comparison / review / quick-screen reports
/// receive their strings already spelled by the app.
pub fn render_study_pdf(study: &Study, numbers: NumberStyle) -> Result<Vec<u8>, ReportError> {
    let nf = numbers;
    let frame = crate::form::build_frame(study).map_err(ReportError::Normalize)?;
    let outputs = frame.snapshot.outputs();
    let judgment = &study.judgment;
    let current_price = judgment.current_price.map(|m| m.as_decimal());

    let mut doc = Doc::new();

    // ── Page 1 — the header block (neutral — NOT the form's wordmark) ──
    doc.title("Analyse de sélection de titre");
    // Issue #74 / G1 F: a pathological identifier cannot run past its cell — `header_box` fits
    // every value to its column at the real glyph widths.
    let company = study
        .company_name
        .as_deref()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or(EM_DASH);
    // G1 final (L7): « et saisie manuelle » is never elided — see `DataSource::header_lines`.
    let sources = data_source(study)
        .header_lines(header_room((PAGE_W - 2.0 * MARGIN) / 3.0), FONT)
        .join("\n");
    doc.header_box(&[
        [
            ("Société", company),
            ("Symbole", &study.security_ticker),
            // G1 final (L11): the date is the study's CREATION date, labelled as the screen
            // labels it (« Créée le ») — the comparison PDF's per-study date is the same one.
            (CREATED_ON, &date_prefix(&study.created_at.0)),
        ],
        [
            ("Monnaie", &study.native_currency),
            ("Données", &sources),
            ("Préparé par", EM_DASH),
        ],
    ]);
    // Capitalisation — the form's box; v1 carries only the latest book value per share.
    let latest_bvps = study
        .years
        .iter()
        .rev()
        .find_map(|y| y.book_value_per_share.as_ref().and_then(|c| c.value))
        .map(|m| m.as_decimal());
    doc.small_line(&format!(
        "Capitalisation — actions en circulation : {}   ·   actions privilégiées : {}   ·   dette à long terme : {}   ·   valeur comptable / action : {}",
        EM_DASH,
        EM_DASH,
        EM_DASH,
        nf.fmt_dec(latest_bvps, DisplayField::PerShare),
    ));
    doc.gap(4.0);

    // ── §1 — the full-page semi-log plot + the four growth lines ──
    doc.section("1. Analyse visuelle des ventes, bénéfices et cours");
    doc.growth_chart(&frame, nf);
    doc.gap(2.0);
    doc.two_columns(
        &format!(
            "(1) Croissance historique des ventes : {}",
            nf.pct(outputs.growth.sales_cagr_pct)
        ),
        &format!(
            "(3) Croissance historique du BPA : {}",
            nf.pct(outputs.growth.eps_cagr_pct)
        ),
    );
    doc.two_columns(
        &format!(
            "(2) Croissance estimée des ventes : {}",
            nf.pct(judgment.projected_sales_growth_pct.map(|m| m.as_decimal()))
        ),
        &format!(
            "(4) Croissance estimée du BPA : {}",
            nf.pct(judgment.projected_eps_growth_pct.map(|m| m.as_decimal()))
        ),
    );
    doc.new_page();

    // ── Page 2 — §2 Management: the years as COLUMNS (the form's layout), the 5-yr average and
    //    the trend at the right. ──
    doc.section("2. Évaluation de la gestion");
    {
        let m = &outputs.management;
        // The last ten reported years (the form has ten columns).
        let rows: Vec<&steadyinvest_core::ssg::YearRatios> = m
            .per_year
            .iter()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let n = rows.len().max(1);
        let label_w = 150.0;
        let avg_w = 48.0;
        let trend_w = 56.0;
        let year_w = (PAGE_W - 2.0 * MARGIN - label_w - avg_w - trend_w) / n as f32;
        let mut edges = vec![MARGIN, MARGIN + label_w];
        for i in 1..=n {
            edges.push(MARGIN + label_w + year_w * i as f32);
        }
        edges.push(PAGE_W - MARGIN - trend_w);
        edges.push(PAGE_W - MARGIN);
        let mut head: Vec<String> = vec![String::new()];
        head.extend(rows.iter().map(|r| r.year.to_string()));
        if rows.is_empty() {
            head.push(EM_DASH.to_string());
        }
        // G1 F — « Moy. 5 ans » only when five years were averaged: the average runs over the
        // known ratios of the usable-years window (the §3 window), which may hold fewer.
        let window: Vec<i32> = outputs.valuation.per_year.iter().map(|v| v.year).collect();
        let averaged = |pick: fn(&steadyinvest_core::ssg::YearRatios) -> Option<Decimal>| {
            m.per_year
                .iter()
                .filter(|r| window.contains(&r.year) && pick(r).is_some())
                .count()
        };
        let (n_a, n_b) = (averaged(|r| r.ptp_pct), averaged(|r| r.roe_pct));
        let floor = USABLE_YEARS_FLOOR as usize;
        let five = n_a >= floor && n_b >= floor;
        head.push(if five { AVG_FIVE } else { AVG_FEWER }.to_string());
        head.push("Tendance".to_string());
        let head_refs: Vec<&str> = head.iter().map(String::as_str).collect();
        doc.grid_begin(2);
        doc.grid_row_small(&head_refs, &edges, true, 1);
        let mut ptp: Vec<String> = vec!["A · % marge avant impôt".to_string()];
        ptp.extend(rows.iter().map(|r| nf.pct_bare(r.ptp_pct)));
        if rows.is_empty() {
            ptp.push(EM_DASH.to_string());
        }
        ptp.push(nf.pct_bare(m.avg_ptp_pct));
        ptp.push(trend(m.ptp_trend).to_string());
        let refs: Vec<&str> = ptp.iter().map(String::as_str).collect();
        doc.grid_row_small(&refs, &edges, false, 1);
        let mut roe: Vec<String> = vec!["B · % rendement des c. propres".to_string()];
        roe.extend(rows.iter().map(|r| nf.pct_bare(r.roe_pct)));
        if rows.is_empty() {
            roe.push(EM_DASH.to_string());
        }
        roe.push(nf.pct_bare(m.avg_roe_pct));
        roe.push(trend(m.roe_trend).to_string());
        let refs: Vec<&str> = roe.iter().map(String::as_str).collect();
        doc.grid_row_small(&refs, &edges, false, 1);
        doc.grid_end(&edges);
        if !five {
            doc.small_line(&format!(
                "{AVG_FEWER_NOTE} A : {}, B : {}.",
                years_count(n_a),
                years_count(n_b)
            ));
        }
    }
    doc.small_line("A = bénéfice avant impôt ÷ ventes × 100   ·   B = BPA ÷ valeur comptable par action × 100   ·   tendance = dernière année face à la moyenne");
    doc.gap(6.0);

    // ── §3 Price / earnings history — the form's columns A–H over the window, totals, averages,
    //    the average and current P/E. ──
    //    G1 final (L10): the rows and the notes under the table are gathered first, and the whole
    //    section is reserved as one block when it fits on a page — a note never lands alone at
    //    the top of the next page, away from its heading and its table.
    {
        let v = &outputs.valuation;
        let head = [
            "Année",
            "A · Haut",
            "B · Bas",
            "C · BPA",
            "D · A÷C",
            "E · B÷C",
            "F · Div.",
            "G · F÷C %",
            "H · F÷B %",
        ];
        let mut body: Vec<[String; 9]> = Vec::new();
        for row in &v.per_year {
            let cy = frame.series.iter().find(|y| y.year == row.year);
            let (hp, lp, ep, dv) = match cy {
                Some(y) => (y.high_price, y.low_price, y.eps, y.dividend_per_share),
                None => (None, None, None, None),
            };
            let cells = [
                row.year.to_string(),
                nf.money(hp),
                nf.money(lp),
                nf.fmt_dec(ep, DisplayField::PerShare),
                nf.num(row.high_pe),
                nf.num(row.low_pe),
                nf.fmt_dec(dv, DisplayField::PerShare),
                nf.pct(row.payout_pct),
                nf.pct(row.high_yield_pct),
            ];
            body.push(cells);
        }
        // G1 F — a column's total is stated only over EVERY year of the window: an unknown year,
        // an undefined ratio (its denominator — the EPS, or the low price for H — not positive)
        // or a sum past the decimal range leaves it absent, never a partial sum passed off as
        // the total; each reason is named under the table, the right one (never « no figure »
        // for a ratio the method leaves undefined).
        let non_positive = |year: i32, pick: fn(&CanonicalYear) -> Option<Decimal>| {
            frame
                .series
                .iter()
                .find(|y| y.year == year)
                .and_then(pick)
                .is_some_and(|d| d <= Decimal::ZERO)
        };
        let entries = |value: fn(&steadyinvest_core::ssg::YearValuation) -> Option<Decimal>,
                       denominator: fn(&CanonicalYear) -> Option<Decimal>| {
            v.per_year
                .iter()
                .map(|r| match value(r) {
                    Some(d) => Entry::Known(d),
                    None if non_positive(r.year, denominator) => Entry::Undefined,
                    None => Entry::Unknown,
                })
                .collect::<Vec<Entry>>()
        };
        let totals = [
            column_total(entries(|r| r.high_pe, |y| y.eps)),
            column_total(entries(|r| r.low_pe, |y| y.eps)),
            column_total(entries(|r| r.payout_pct, |y| y.eps)),
            column_total(entries(|r| r.high_yield_pct, |y| y.low_price)),
        ];
        let total_of = |t: &Total| match t {
            Total::Sum(d) => Some(*d),
            _ => None,
        };
        let total = [
            "Total".to_string(),
            String::new(),
            String::new(),
            String::new(),
            nf.num(total_of(&totals[0])),
            nf.num(total_of(&totals[1])),
            String::new(),
            nf.pct(total_of(&totals[2])),
            nf.pct(total_of(&totals[3])),
        ];
        body.push(total);
        let avg = [
            "Moyenne".to_string(),
            String::new(),
            String::new(),
            String::new(),
            nf.num(v.avg_high_pe),
            nf.num(v.avg_low_pe),
            String::new(),
            nf.pct(v.avg_payout_pct),
            nf.pct(v.avg_high_yield_pct),
        ];
        body.push(avg);
        let mut notes = Block::default();
        if totals
            .iter()
            .any(|t| matches!(t, Total::Absent { unknown: true, .. }))
        {
            notes.small_line(TOTAL_UNKNOWN_YEAR);
        }
        if totals.iter().any(|t| {
            matches!(
                t,
                Total::Absent {
                    undefined: true,
                    ..
                }
            )
        }) {
            notes.small_line(TOTAL_UNDEFINED);
        }
        if totals.contains(&Total::Overflow) {
            notes.small_line(TOTAL_OVERFLOW);
        }
        notes.line(&format!(
            "8 · C/B moyen (D et E) : {}   ·   9 · C/B actuel : {}   ·   valeur relative : {}",
            nf.num(v.avg_pe),
            nf.num(v.current_pe),
            nf.pct(v.relative_value_pct),
        ));
        notes.line(&format!(
            "Cours actuel : {}   ·   plus haut de l'année en cours : {}   ·   plus bas de l'année en cours : {}",
            nf.money(current_price),
            EM_DASH,
            EM_DASH,
        ));
        let body_refs: Vec<Vec<&str>> = body
            .iter()
            .map(|r| r.iter().map(String::as_str).collect())
            .collect();
        // The grid's own advances: its top rule (`grid_begin`), its rows, its close (`grid_end`).
        let mut table_h = doc.grid_rows_height(&head, &COLS9, FONT) + 3.0;
        for r in &body_refs {
            table_h += doc.grid_rows_height(r, &COLS9, FONT);
        }
        let notes_h = doc.block_height(&notes);
        doc.keep_together_if_it_fits(SECTION_H + table_h + notes_h);
        doc.section("3. Historique cours / bénéfice");
        doc.grid_begin(body.len());
        doc.grid_row_num(&head, &COLS9, true, 1);
        for r in &body_refs {
            doc.grid_row_num(r, &COLS9, false, 1);
        }
        doc.grid_end(&COLS9);
        // Should the table itself run past a page, its notes still move as one block.
        doc.keep_together(notes_h);
        doc.block(&notes);
    }
    doc.gap(6.0);

    // ── §4 Risk & reward — the form's A–E with every intermediate figure ──
    {
        let r = &outputs.risk_reward;
        // G1 F: the block is measured before it is drawn, so its keep-together reserve counts
        // every wrapped line (a long formula line wraps at the margin).
        let mut b = Block::default();
        let c = &r.low_candidates;
        let est_high = outputs.growth.estimated_high_eps;
        let est_low = outputs.growth.estimated_low_eps;
        b.line(&format!(
            "A · Prix haut à 5 ans : PER haut moyen {} × BPA estimé haut {} = {}",
            nf.num(judgment.judged_avg_high_pe.map(|m| m.as_decimal())),
            nf.fmt_dec(est_high, DisplayField::PerShare),
            nf.money(r.forecast_high),
        ));
        b.line("B · Prix bas à 5 ans, les quatre candidats :");
        b.indent_line(&format!(
            "(a) PER bas moyen {} × BPA estimé bas {} = {}",
            nf.num(judgment.judged_avg_low_pe.map(|m| m.as_decimal())),
            nf.fmt_dec(est_low, DisplayField::PerShare),
            nf.money(c.avg_low_pe_times_eps),
        ));
        b.indent_line(&format!(
            "(b) Prix bas moyen des 5 dernières années = {}",
            nf.money(c.avg_low_price_last_5y),
        ));
        b.indent_line(&format!(
            "(c) Plus bas sévère récent = {}",
            nf.money(c.recent_severe_low),
        ));
        b.indent_line(&format!(
            "(d) Prix soutenu par le dividende : dividende {} ÷ rendement haut moyen {} = {}",
            nf.fmt_dec(
                judgment.present_full_year_dividend.map(|m| m.as_decimal()),
                DisplayField::PerShare
            ),
            nf.pct(outputs.valuation.avg_high_yield_pct),
            nf.money(c.dividend_supported),
        ));
        b.indent_line(&format!(
            "Prix bas retenu ({}) = {}",
            option_label(judgment.forecast_low_option),
            nf.money(r.forecast_low),
        ));
        match &r.zones {
            Some(z) => {
                let range = z.forecast_high - z.forecast_low;
                let third = z.buy_top - z.forecast_low;
                b.line(&format!(
                    "C · Zonage : étendue {} − {} = {}   ·   un tiers = {}",
                    nf.money(Some(z.forecast_high)),
                    nf.money(Some(z.forecast_low)),
                    nf.money(Some(range)),
                    nf.money(Some(third)),
                ));
                b.indent_line(&format!(
                    "{} : {} à {}   ·   {} : {} à {}   ·   {} : {} à {}",
                    ZONE_LOW,
                    nf.money(Some(z.forecast_low)),
                    nf.money(Some(z.buy_top)),
                    ZONE_MID,
                    nf.money(Some(z.buy_top)),
                    nf.money(Some(z.neutral_top)),
                    ZONE_HIGH,
                    nf.money(Some(z.neutral_top)),
                    nf.money(Some(z.forecast_high)),
                ));
                // G1 final (M1): an absent current price is said absent — never « outside the
                // range », which would state a position it does not have.
                match current_price {
                    Some(_) => b.indent_line(&format!(
                        "Le cours actuel {} se situe : {}",
                        nf.money(current_price),
                        price_position(price_place(z, current_price)),
                    )),
                    None => b.indent_line(PRICE_ABSENT),
                }
            }
            None => b.line("C · Zonage : — (prévision incomplète ou plage dégénérée)"),
        }
        b.line(&format!(
            "D · Ratio hausse / baisse : (prix haut {} − cours {}) ÷ (cours {} − prix bas {}) = {}",
            nf.money(r.forecast_high),
            nf.money(current_price),
            nf.money(current_price),
            nf.money(r.forecast_low),
            upside(&r.upside_downside, nf),
        ));
        b.line(&format!(
            "E · Objectif de cours : (prix haut {} ÷ cours {} × 100) − 100 = {} d'appréciation",
            nf.money(r.forecast_high),
            nf.money(current_price),
            nf.pct(outputs.returns.projected_appreciation_pct),
        ));
        let bar_h = if r.zones.is_some() {
            ZONEBAR_H_RESERVE
        } else {
            0.0
        };
        doc.keep_together(SECTION_H + doc.block_height(&b) + 4.0 + bar_h);
        doc.section("4. Risque et rendement sur 5 ans");
        doc.block(&b);
        doc.gap(4.0);
        // Issue #105 — the zone bar (low/median/high thirds + the current-price marker).
        doc.zone_bar(r.zones.as_ref(), current_price, nf);
    }
    doc.gap(6.0);

    // ── §5 Five-year potential — the form's A–C with the intermediate figures ──
    {
        let ret = &outputs.returns;
        let mut b = Block::default();
        b.line(&format!(
            "A · Rendement présent : dividende {} ÷ cours {} × 100 = {}",
            nf.fmt_dec(
                judgment.present_full_year_dividend.map(|m| m.as_decimal()),
                DisplayField::PerShare
            ),
            nf.money(current_price),
            nf.pct(ret.present_yield_pct),
        ));
        b.line(&format!(
            "B · Rendement moyen sur 5 ans : BPA moyen projeté {} × % distribution moyen {} = dividende moyen {}",
            nf.fmt_dec(ret.avg_annual_eps, DisplayField::PerShare),
            nf.pct(outputs.valuation.avg_payout_pct),
            nf.fmt_dec(ret.avg_annual_dividend, DisplayField::PerShare),
        ));
        b.indent_line(&format!(
            "dividende moyen {} ÷ cours {} × 100 = {}",
            nf.fmt_dec(ret.avg_annual_dividend, DisplayField::PerShare),
            nf.money(current_price),
            nf.pct(ret.avg_yield_pct),
        ));
        b.line(&format!(
            "C · Rendement annuel total estimé : appréciation sur 5 ans {}, soit {} annualisée",
            nf.pct(ret.projected_appreciation_pct),
            nf.pct(ret.projected_annualized_appreciation_pct),
        ));
        b.indent_line(&format!(
            "appréciation annualisée {} + rendement moyen {} = {}",
            nf.pct(ret.projected_annualized_appreciation_pct),
            nf.pct(ret.avg_yield_pct),
            total_return(ret, nf),
        ));
        b.small_line(
            "Les taux annualisés sont composés (et non simples) : (haut ÷ cours)^(1/5) − 1.",
        );
        doc.keep_together(SECTION_H + doc.block_height(&b));
        doc.section("5. Potentiel à 5 ans");
        doc.block(&b);
    }
    doc.gap(8.0);

    // ── Verdict (neutral) + flags ──
    doc.section("Synthèse");
    doc.line(&format!(
        "Position : {}",
        verdict_label(frame.snapshot.verdict())
    ));
    if outputs.low_confidence {
        doc.line("Confiance réduite : moins d'années exploitables que le seuil de la méthode.");
    }

    // ── Annexe — every historical figure (the form plots them; the table keeps the exact values) ──
    doc.new_page();
    doc.section("Annexe — données historiques");
    doc.grid_begin(study.years.len());
    doc.grid_row_num(
        &[
            "Année",
            "Ventes",
            "Bén. av. impôt",
            "BPA",
            "Cours haut",
            "Cours bas",
            "Div./action",
            "Val. compt./act.",
        ],
        &COLS8,
        true,
        1,
    );
    for y in &study.years {
        let cells = [
            y.year.to_string(),
            nf.cell(y.sales.value, DisplayField::LargeMonetary),
            nf.cell(
                y.pre_tax_profit.as_ref().and_then(|c| c.value),
                DisplayField::LargeMonetary,
            ),
            nf.cell(y.eps.value, DisplayField::PerShare),
            nf.cell(y.high_price.value, DisplayField::Price),
            nf.cell(y.low_price.value, DisplayField::Price),
            nf.cell(
                y.dividend_per_share.as_ref().and_then(|c| c.value),
                DisplayField::PerShare,
            ),
            nf.cell(
                y.book_value_per_share.as_ref().and_then(|c| c.value),
                DisplayField::PerShare,
            ),
        ];
        let refs: Vec<&str> = cells.iter().map(String::as_str).collect();
        doc.grid_row_num(&refs, &COLS8, false, 1);
    }
    doc.grid_end(&COLS8);

    Ok(doc.finish())
}

// ── neutral formatting helpers (None → em-dash, never 0; exact-decimal display rounding) ──

/// The reader's number format for the study PDF (G1 I, #237) — the report crate's own mirror of
/// the app's setting, so `report` stays independent of `app`: `Comma` → `1 234,56` (no-break
/// space grouping, decimal comma), `Point` → `1,234.56` (comma grouping, decimal point). Pure
/// spelling over the rounded decimal — no arithmetic, the scale and the rounding come from
/// `core::rounding` as before.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NumberStyle {
    #[default]
    Comma,
    Point,
}

/// The no-break space the app emits for the comma format's grouping (WinAnsi 0xA0).
const GROUP_NBSP: char = '\u{00A0}';

impl NumberStyle {
    /// A decimal's canonical digits, grouped and marked for the format (a leading `-` kept).
    pub fn spell(self, d: Decimal) -> String {
        let canonical = d.to_string();
        let (negative, unsigned) = match canonical.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, canonical.as_str()),
        };
        let (integer, fraction) = match unsigned.split_once('.') {
            Some((i, f)) => (i, Some(f)),
            None => (unsigned, None),
        };
        let (group, mark) = match self {
            NumberStyle::Comma => (GROUP_NBSP, ','),
            NumberStyle::Point => (',', '.'),
        };
        let mut out = String::with_capacity(canonical.len() + integer.len() / 3 + 1);
        if negative {
            out.push('-');
        }
        let len = integer.chars().count();
        for (i, digit) in integer.chars().enumerate() {
            if i != 0 && (len - i) % 3 == 0 {
                out.push(group);
            }
            out.push(digit);
        }
        if let Some(fraction) = fraction {
            out.push(mark);
            out.push_str(fraction);
        }
        out
    }

    pub(crate) fn fmt_dec(self, v: Option<Decimal>, field: DisplayField) -> String {
        match v {
            None => EM_DASH.to_string(),
            // G1 final (L6): the screen's path — `round_for_display`, never `normalize` — so a
            // « 4,0 % » on the screen is « 4,0 % » in the PDF, never « 4 % ».
            Some(d) => self.spell(round_for_display(d, field)),
        }
    }

    fn cell(self, v: Option<steadyinvest_contract::Money>, field: DisplayField) -> String {
        self.fmt_dec(v.map(|m| m.as_decimal()), field)
    }

    pub(crate) fn money(self, v: Option<Decimal>) -> String {
        self.fmt_dec(v, DisplayField::Price)
    }

    pub(crate) fn num(self, v: Option<Decimal>) -> String {
        self.fmt_dec(v, DisplayField::PeRatio)
    }

    pub(crate) fn pct(self, v: Option<Decimal>) -> String {
        match v {
            None => EM_DASH.to_string(),
            Some(_) => format!("{} %", self.fmt_dec(v, DisplayField::Percent)),
        }
    }

    /// A percentage without its unit — for a table whose header already says « % » (the §2
    /// columns).
    fn pct_bare(self, v: Option<Decimal>) -> String {
        self.fmt_dec(v, DisplayField::Percent)
    }
}

/// A §3 column's « Total » (G1 F): the sum over EVERY year of the window, or the reason it cannot
/// be stated — never a partial sum over the known years passed off as the total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Total {
    Sum(Decimal),
    /// The window is empty: nothing to sum (the table has no row either).
    Empty,
    /// At least one year of the window has no figure in the column: `unknown` when an input is
    /// missing, `undefined` when the ratio's denominator is not positive (both can hold).
    Absent {
        unknown: bool,
        undefined: bool,
    },
    /// The sum left the decimal range (absent, never restarted from the next year).
    Overflow,
}

/// One year's cell of a §3 ratio column, as far as its total is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    Known(Decimal),
    /// An input of the ratio is missing.
    Unknown,
    /// The ratio is undefined: its denominator (EPS, or the low price) is not positive.
    Undefined,
}

fn column_total(entries: Vec<Entry>) -> Total {
    let unknown = entries.contains(&Entry::Unknown);
    let undefined = entries.contains(&Entry::Undefined);
    if unknown || undefined {
        return Total::Absent { unknown, undefined };
    }
    let mut sum: Option<Decimal> = None;
    for e in entries {
        if let Entry::Known(v) = e {
            match sum.map_or(Some(v), |s| s.checked_add(v)) {
                Some(s) => sum = Some(s),
                None => return Total::Overflow,
            }
        }
    }
    sum.map_or(Total::Empty, Total::Sum)
}

/// « 1 an », « 3 ans », « aucune année » — the §2 average's year count.
fn years_count(n: usize) -> String {
    match n {
        0 => NO_YEAR.to_string(),
        1 => format!("1 {YEAR_ONE}"),
        n => format!("{n} {YEAR_MANY}"),
    }
}

/// The §4 forecast-low option, named as on the screen's chips.
fn option_label(option: steadyinvest_contract::ForecastLowOption) -> &'static str {
    use steadyinvest_contract::ForecastLowOption as O;
    match option {
        O::AvgLowPeTimesEps => OPTION_A,
        O::AvgLowPriceLast5y => OPTION_B,
        O::RecentSevereLow => OPTION_C,
        O::DividendSupported => OPTION_D,
    }
}

/// Where the figures came from (G1 F): read off EVERY valued cell of the study's years, not the
/// latest sales cell alone. A provider-fetched cell names its provider (the tag of its
/// provenance, `"{tag}:{sha}"`, Story 6.9); a user-entered one reads « saisie manuelle »; a study
/// with both names both. A computed cell (`Source::Derived`) descends from the others, so it adds
/// no origin of its own and is never passed off as a manual entry — it reads « calculé » only when
/// nothing else is valued. No valued cell → the em-dash. Data, not prose; never a path or key.
fn data_source(study: &Study) -> DataSource {
    use steadyinvest_contract::Source;
    let mut tags: Vec<&str> = Vec::new();
    let (mut untagged, mut manual, mut derived) = (false, false, false);
    for y in &study.years {
        let cells = [
            Some(&y.sales),
            Some(&y.eps),
            Some(&y.high_price),
            Some(&y.low_price),
            y.dividend_per_share.as_ref(),
            y.pre_tax_profit.as_ref(),
            y.book_value_per_share.as_ref(),
        ];
        for c in cells.into_iter().flatten().filter(|c| c.value.is_some()) {
            match c.source {
                Source::Provider => {
                    let tag = c
                        .provenance
                        .hash_of_dependencies
                        .split(':')
                        .next()
                        .filter(|tag| !tag.is_empty() && tag.len() <= 24);
                    match tag {
                        Some(t) if !tags.contains(&t) => tags.push(t),
                        Some(_) => {}
                        None => untagged = true,
                    }
                }
                Source::Manual => manual = true,
                Source::Derived => derived = true,
            }
        }
    }
    let provider = match tags.as_slice() {
        [] if untagged => Some(PROVIDER_ONE.to_string()),
        [] => None,
        [one] if !untagged => Some(format!("{PROVIDER_ONE} {one}")),
        many => Some(format!("{PROVIDER_MANY} {}", many.join(", "))),
    };
    match (provider, manual) {
        (Some(p), true) => DataSource {
            head: p,
            and_manual: true,
        },
        (Some(p), false) => DataSource::alone(p),
        (None, true) => DataSource::alone(MANUAL_ENTRY),
        (None, false) if derived => DataSource::alone(COMPUTED),
        (None, false) => DataSource::alone(EM_DASH),
    }
}

/// The header's « Données » value (G1 final, L7): the provider part, and whether « et saisie
/// manuelle » follows — kept apart so a shortened header never loses the manual entry.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DataSource {
    head: String,
    and_manual: bool,
}

impl DataSource {
    fn alone(head: impl Into<String>) -> Self {
        DataSource {
            head: head.into(),
            and_manual: false,
        }
    }

    /// The whole phrase, as one line of prose.
    fn phrase(&self) -> String {
        if self.and_manual {
            format!("{} {AND_MANUAL}", self.head)
        } else {
            self.head.clone()
        }
    }

    /// The value's lines in a header cell `room` wide at `size` — at most two. The whole phrase
    /// when it fits one line; else, with a manual entry, the provider part on the first line
    /// (ended by « … » only when it alone overflows) and « et saisie manuelle » whole on the
    /// second — never elided, never split; else the phrase wrapped onto two lines at most.
    fn header_lines(&self, room: f32, size: f32) -> Vec<String> {
        let whole = wrap_to_width(&self.phrase(), room, size);
        if whole.len() <= 1 {
            return whole;
        }
        if self.and_manual {
            vec![fit(&self.head, room, size), AND_MANUAL.to_string()]
        } else if whole.len() <= 2 {
            whole
        } else {
            let mut lines = whole;
            lines.truncate(2);
            let rest = lines[1].clone();
            lines[1] = fit(&format!("{rest}…"), room, size);
            lines
        }
    }
}

/// The §5 projected total (issue #189): the full total when both terms are known; when ONLY the
/// dividend history is missing (`ReturnOutputs::appreciation_only_potential`), the annualised
/// appreciation alone with the honest « (hors div.) » marker — mirrors `app`'s
/// `fmt_total_return`.
fn total_return(r: &steadyinvest_core::ssg::ReturnOutputs, nf: NumberStyle) -> String {
    match (
        r.projected_total_annualized_return_pct,
        r.appreciation_only_potential(),
    ) {
        (Some(total), _) => nf.pct(Some(total)),
        (None, Some(appreciation)) => format!("{} (hors div.)", nf.pct(Some(appreciation))),
        (None, None) => nf.pct(None),
    }
}

fn trend(t: Option<Trend>) -> &'static str {
    match t {
        Some(Trend::Up) => "en hausse",
        Some(Trend::Even) => "stable",
        Some(Trend::Down) => "en baisse",
        None => EM_DASH,
    }
}

/// Where the current price sits against the §4 zoning (G1 final, M1 / L9): in one of the three
/// zones (the engine's own interval comparators), below the forecast low, above the forecast
/// high — or `Absent` when there is no current price. Read off the price and the bounds, never off
/// the engine's `present_price_zone == None`, which means « absent » and « out of range » alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PricePlace {
    In(Zone),
    Below,
    Above,
    Absent,
}

fn price_place(z: &ZoneBounds, price: Option<Decimal>) -> PricePlace {
    let Some(p) = price else {
        return PricePlace::Absent;
    };
    if p < z.forecast_low {
        PricePlace::Below
    } else if p > z.forecast_high {
        PricePlace::Above
    } else if p <= z.buy_top {
        PricePlace::In(Zone::Buy)
    } else if p <= z.neutral_top {
        PricePlace::In(Zone::Neutral)
    } else {
        PricePlace::In(Zone::Sell)
    }
}

fn price_position(place: PricePlace) -> &'static str {
    match place {
        PricePlace::In(Zone::Buy) => "dans la zone basse",
        PricePlace::In(Zone::Neutral) => "dans la zone médiane",
        PricePlace::In(Zone::Sell) => "dans la zone haute",
        PricePlace::Below => BELOW_RANGE,
        PricePlace::Above => ABOVE_RANGE,
        PricePlace::Absent => EM_DASH,
    }
}

fn upside(u: &UpsideDownside, nf: NumberStyle) -> String {
    match u {
        UpsideDownside::Ratio(d) => format!("{} : 1", nf.fmt_dec(Some(*d), DisplayField::Ratio)),
        UpsideDownside::Undefined => "— (dénominateur non positif)".to_string(),
        UpsideDownside::Unknown => EM_DASH.to_string(),
    }
}

fn verdict_label(verdict: &steadyinvest_core::verdict::Verdict) -> &'static str {
    use steadyinvest_core::verdict::Verdict;
    match verdict {
        Verdict::Full(_) => VERDICT_FULL,
        Verdict::Provisional(_) => VERDICT_PROVISIONAL,
        Verdict::Withheld(_) => VERDICT_WITHHELD,
    }
}

/// The `YYYY-MM-DD` prefix of an ISO timestamp (char-safe, no byte slicing).
fn date_prefix(ts: &str) -> String {
    ts.chars().take(10).collect()
}

pub(crate) const EM_DASH: &str = "—";

// ── neutral user-facing string inventory (FR13) ──
//
// `report` lives outside the `app` posture gate, so it guards its own neutrality: every static
// user-facing string the PDF emits is listed in [`REPORT_USER_FACING`] (or is a `zone_label` /
// `trend` / verdict / option const, all covered by the neutrality test). Add new strings here when
// you add them to the layout — the test scans them against `core::method::BANNED_VERBS_{FR,EN}` and
// asserts no NAIC wordmark, the same shared catalogs the app gate uses.
const VERDICT_FULL: &str = "Tous les critères validés et à jour";
const VERDICT_PROVISIONAL: &str = "Provisoire — données à revérifier ou confiance réduite";
const VERDICT_WITHHELD: &str = "En attente — au moins une donnée requise manque";
const CREATED_ON: &str = "Créée le";
const PROVIDER_ONE: &str = "fournisseur";
const PROVIDER_MANY: &str = "fournisseurs";
const MANUAL_ENTRY: &str = "saisie manuelle";
const AND_MANUAL: &str = "et saisie manuelle";
const COMPUTED: &str = "calculé";
const AVG_FIVE: &str = "Moy. 5 ans";
const AVG_FEWER: &str = "Moyenne";
const AVG_FEWER_NOTE: &str = "Moyenne sur moins de cinq années connues —";
const NO_YEAR: &str = "aucune année";
const YEAR_ONE: &str = "an";
const YEAR_MANY: &str = "ans";
const TOTAL_UNKNOWN_YEAR: &str =
    "Total « — » : au moins une année de la période n'a pas le chiffre de la colonne.";
const TOTAL_OVERFLOW: &str = "Total « — » : la somme dépasse la plage calculable.";
const TOTAL_UNDEFINED: &str = "Total « — » : ratio non défini pour au moins une année (BPA, ou cours bas pour H, négatif ou nul).";
const OPTION_A: &str = "PER bas × BPA bas";
const OPTION_B: &str = "prix bas moyen 5 ans";
const OPTION_C: &str = "plus bas sévère récent";
const OPTION_D: &str = "soutenu par le dividende";

// ── issue #105 / #207 — the embedded charts' neutral labels (greyscale legend + zone bands) ──
const CHART_LEGEND: &str = "BPA (trait épais)   ·   Ventes (trait fin)   ·   Cours haut–bas (barres)   ·   projection (pointillés)   ·   guides de croissance 5–30 % (gris clair)";
const CHART_SCALE: &str =
    "Échelle logarithmique, propre à chaque série (l'axe gradué est celui du BPA)";
const GUIDES_FROM: &str = "les guides partent du BPA positif de";
const GUIDES_SPAN: &str = "et couvrent les";
const YEARS_OF_FORECAST: &str = "ans de la prévision";
const NO_POSITIVE_EPS: &str = "aucun BPA positif : pas de guides";
const PROJECTION_FROM: &str = "la projection part du BPA de";
const PROJECTION_NONE: &str = "projection non tracée : le BPA de";
const NOT_POSITIVE: &str = "n'est pas positif";
const PROJECTION_NO_BASE: &str = "projection non tracée : aucune année utilisable";
const QUARTER_BOX_TITLE: &str = "Chiffres trimestriels récents";
const QUARTER_LATEST: &str = "Dernier trimestre";
const QUARTER_YEAR_AGO: &str = "Même trimestre, un an avant";
const QUARTER_CHANGE: &str = "Variation";
const ZONE_LOW: &str = "Zone basse";
const ZONE_MID: &str = "Zone médiane";
const ZONE_HIGH: &str = "Zone haute";
const CURRENT_PRICE: &str = "Cours actuel";
// G1 final (M1 / L9): the current price's place when it is not inside a zone — named, never
// « hors de la plage » for an absent price, never a marker pinned at the edge as if on it.
const PRICE_ABSENT: &str =
    "Le cours actuel est absent : sa place dans le zonage n'est pas établie.";
const BELOW_RANGE: &str = "sous la plage prévue (sous le prix bas)";
const ABOVE_RANGE: &str = "au-dessus de la plage prévue (au-dessus du prix haut)";
const MARKER_BELOW: &str = "sous la plage";
const MARKER_ABOVE: &str = "au-dessus de la plage";

#[cfg(test)]
const REPORT_USER_FACING: &[&str] = &[
    // Header block.
    "Analyse de sélection de titre",
    "Société",
    "Symbole",
    CREATED_ON,
    "Monnaie",
    "Données",
    "Préparé par",
    PROVIDER_ONE,
    PROVIDER_MANY,
    MANUAL_ENTRY,
    AND_MANUAL,
    COMPUTED,
    "Capitalisation — actions en circulation :",
    "actions privilégiées :",
    "dette à long terme :",
    "valeur comptable / action :",
    // Section titles (all expanded).
    "1. Analyse visuelle des ventes, bénéfices et cours",
    "2. Évaluation de la gestion",
    "3. Historique cours / bénéfice",
    "4. Risque et rendement sur 5 ans",
    "5. Potentiel à 5 ans",
    "Synthèse",
    "Annexe — données historiques",
    // §1 growth lines.
    "(1) Croissance historique des ventes :",
    "(2) Croissance estimée des ventes :",
    "(3) Croissance historique du BPA :",
    "(4) Croissance estimée du BPA :",
    // §2.
    AVG_FIVE,
    AVG_FEWER,
    AVG_FEWER_NOTE,
    NO_YEAR,
    YEAR_ONE,
    YEAR_MANY,
    "Tendance",
    "A · % marge avant impôt",
    "B · % rendement des c. propres",
    "A = bénéfice avant impôt ÷ ventes × 100   ·   B = BPA ÷ valeur comptable par action × 100   ·   tendance = dernière année face à la moyenne",
    // §3 column headers + rows.
    "Année",
    "A · Haut",
    "B · Bas",
    "C · BPA",
    "D · A÷C",
    "E · B÷C",
    "F · Div.",
    "G · F÷C %",
    "H · F÷B %",
    "Total",
    "Moyenne",
    TOTAL_UNKNOWN_YEAR,
    TOTAL_OVERFLOW,
    TOTAL_UNDEFINED,
    "8 · C/B moyen (D et E) :",
    "9 · C/B actuel :",
    "valeur relative :",
    "Cours actuel :",
    "plus haut de l'année en cours :",
    "plus bas de l'année en cours :",
    // §4.
    "A · Prix haut à 5 ans : PER haut moyen",
    "× BPA estimé haut",
    "B · Prix bas à 5 ans, les quatre candidats :",
    "(a) PER bas moyen",
    "× BPA estimé bas",
    "(b) Prix bas moyen des 5 dernières années =",
    "(c) Plus bas sévère récent =",
    "(d) Prix soutenu par le dividende : dividende",
    "÷ rendement haut moyen",
    "Prix bas retenu",
    "C · Zonage : étendue",
    "un tiers =",
    "Le cours actuel",
    "se situe :",
    "C · Zonage : — (prévision incomplète ou plage dégénérée)",
    "D · Ratio hausse / baisse : (prix haut",
    "− cours",
    "− prix bas",
    "E · Objectif de cours : (prix haut",
    "÷ cours",
    "× 100) − 100 =",
    "d'appréciation",
    // §5.
    "A · Rendement présent : dividende",
    "B · Rendement moyen sur 5 ans : BPA moyen projeté",
    "× % distribution moyen",
    "= dividende moyen",
    "dividende moyen",
    "C · Rendement annuel total estimé : appréciation sur 5 ans",
    ", soit",
    "annualisée",
    "appréciation annualisée",
    "+ rendement moyen",
    "Les taux annualisés sont composés (et non simples) : (haut ÷ cours)^(1/5) − 1.",
    "(hors div.)",
    // Synthèse.
    "Position :",
    "Confiance réduite : moins d'années exploitables que le seuil de la méthode.",
    // Annexe columns.
    "Ventes",
    "Bén. av. impôt",
    "BPA",
    "Cours haut",
    "Cours bas",
    "Div./action",
    "Val. compt./act.",
    // The embedded charts' labels.
    CHART_LEGEND,
    CHART_SCALE,
    GUIDES_FROM,
    GUIDES_SPAN,
    YEARS_OF_FORECAST,
    NO_POSITIVE_EPS,
    PROJECTION_FROM,
    PROJECTION_NONE,
    NOT_POSITIVE,
    PROJECTION_NO_BASE,
    QUARTER_BOX_TITLE,
    QUARTER_LATEST,
    QUARTER_YEAR_AGO,
    QUARTER_CHANGE,
    ZONE_LOW,
    ZONE_MID,
    ZONE_HIGH,
    CURRENT_PRICE,
    PRICE_ABSENT,
    BELOW_RANGE,
    ABOVE_RANGE,
    MARKER_BELOW,
    MARKER_ABOVE,
    OPTION_A,
    OPTION_B,
    OPTION_C,
    OPTION_D,
    // Footer disclaimer (FR64).
    "Outil éducatif — ne constitue pas un conseil financier.",
    // The neutral render-failure message.
    "l'étude ne peut pas être mise en forme",
    // Verdict strings.
    VERDICT_FULL,
    VERDICT_PROVISIONAL,
    VERDICT_WITHHELD,
];

// ── the document builder: coordinate layout + simple pagination ──

/// The kinds of prose line a [`Block`] holds — the same three as [`Doc::line`],
/// [`Doc::indent_line`] and [`Doc::small_line`].
#[derive(Clone, Copy)]
enum ProseKind {
    Body,
    Indent,
    Small,
}

impl ProseKind {
    /// `(x, size, line_h)`, as the matching `Doc` method lays it out.
    fn metrics(self) -> (f32, f32, f32) {
        match self {
            ProseKind::Body => (MARGIN, FONT, LINE_H),
            ProseKind::Indent => (MARGIN + 18.0, FONT, LINE_H),
            ProseKind::Small => (MARGIN, SMALL, LINE_H - 2.0),
        }
    }
}

/// G1 F — a run of prose lines gathered before it is drawn, so a keep-together reserve can
/// measure it with every wrapped line ([`Doc::block_height`]) instead of guessing a line count.
#[derive(Default)]
struct Block {
    lines: Vec<(ProseKind, String)>,
}

impl Block {
    fn line(&mut self, s: &str) {
        self.lines.push((ProseKind::Body, s.to_string()));
    }
    fn indent_line(&mut self, s: &str) {
        self.lines.push((ProseKind::Indent, s.to_string()));
    }
    fn small_line(&mut self, s: &str) {
        self.lines.push((ProseKind::Small, s.to_string()));
    }
}

/// One header row of a grid, kept so it can be replayed on each continuation page (G1 C).
struct GridHeaderRow {
    cells: Vec<String>,
    numeric: std::ops::Range<usize>,
    font: f32,
}

/// Accumulates content across one or more A4 pages, tracking a top-origin cursor and starting a new
/// page when the next line would cross the bottom margin.
pub(crate) struct Doc {
    pages: Vec<Content>,
    cur: Content,
    y: f32,        // top-origin cursor (distance from the page top)
    grid_top: f32, // the top of the grid table currently being drawn (issue #104)
    // Issue #74 / G1 C: the current table's header rows — EVERY head row given before the first
    // body row (the comparison has two: « TICKER (CUR) », then the names). They are held back and
    // drawn together with the first body row (so they are never orphaned at a page foot), then
    // replayed, all of them, at the top of each continuation page when the table spans a break.
    grid_header: Vec<GridHeaderRow>,
    // Whether the current grid has started drawing (its header rows are on the page). A head row
    // given after that is an underlined body row (the quick screen's totals), never a header.
    grid_started: bool,
    // The font size of the grid being drawn (body, or caption for a wide table).
    grid_font: f32,
    /// The vertical extents of the current grid's full-width note rows ([`Doc::grid_note_row`]):
    /// the interior column rules are interrupted there, so they never cross the note's words.
    grid_spans: Vec<(f32, f32)>,
    // The page size (points). Portrait A4 by default; `landscape()` swaps them (Story 7.1 — the
    // five-column comparison). The text primitives flip y against PAGE_H, so a landscape page's
    // content stream starts with a translate that maps that flip onto its own height.
    page_w: f32,
    page_h: f32,
}

impl Doc {
    pub(crate) fn new() -> Self {
        Self::with_page(PAGE_W, PAGE_H)
    }

    /// A4 landscape (Story 7.1): the comparison's five columns need the width.
    pub(crate) fn landscape() -> Self {
        Self::with_page(PAGE_H, PAGE_W)
    }

    fn with_page(page_w: f32, page_h: f32) -> Self {
        Doc {
            pages: Vec::new(),
            cur: fresh_content(page_h),
            y: MARGIN,
            grid_top: MARGIN,
            grid_header: Vec::new(),
            grid_started: false,
            grid_font: FONT,
            grid_spans: Vec::new(),
            page_w,
            page_h,
        }
    }

    /// The usable width (the right margin's x) — layouts author against it, never the constant.
    pub(crate) fn right(&self) -> f32 {
        self.page_w - MARGIN
    }

    /// Ensure `need` points of vertical space remain on the current page; else start a new one.
    pub(crate) fn ensure(&mut self, need: f32) {
        if self.page_h - self.y - need < BOTTOM {
            self.new_page();
        }
    }

    /// Finish the page in progress and start a fresh one, resetting the cursor to the top margin.
    pub(crate) fn new_page(&mut self) {
        let finished = std::mem::replace(&mut self.cur, fresh_content(self.page_h));
        self.pages.push(finished);
        self.y = MARGIN;
    }

    /// The zero-based index of the page in progress (the finished pages before it).
    pub(crate) fn page_index(&self) -> usize {
        self.pages.len()
    }

    /// Issue #104: reserve `need` points as ONE block so a heading + its table/chart never split
    /// across a page break (the §4 zone bar was orphaning). A no-op when the block already fits.
    pub(crate) fn keep_together(&mut self, need: f32) {
        self.ensure(need);
    }

    /// G1 final (L10) — [`keep_together`] for a block that may be taller than a page: reserved as
    /// one when a fresh page can hold it, else left to break where it must (never chasing a
    /// too-tall block onto a new page for nothing).
    pub(crate) fn keep_together_if_it_fits(&mut self, need: f32) {
        if need <= self.page_h - MARGIN - BOTTOM {
            self.ensure(need);
        }
    }

    pub(crate) fn gap(&mut self, h: f32) {
        self.y += h;
    }

    pub(crate) fn title(&mut self, s: &str) {
        self.ensure(TITLE_FONT + 10.0); // reserve the true advance (font + rule + gap)
        self.y += TITLE_FONT;
        text_bold(&mut self.cur, MARGIN, self.y, TITLE_FONT, s);
        self.y += 6.0;
        hline(&mut self.cur, MARGIN, self.page_w - MARGIN, self.y, 1.0);
        self.y += 4.0;
    }

    pub(crate) fn section(&mut self, s: &str) {
        // Issue #104: a heading keeps room for its header + a few rows, so it never dangles alone at
        // a page foot with its table pushed to the next page.
        self.keep_together(HEAD_FONT + 5.0 * LINE_H);
        self.y += HEAD_FONT;
        text_bold(&mut self.cur, MARGIN, self.y, HEAD_FONT, s);
        self.y += 4.0;
        hline(&mut self.cur, MARGIN, self.page_w - MARGIN, self.y, 0.5);
        self.y += LINE_H - 4.0;
    }

    pub(crate) fn line(&mut self, s: &str) {
        let (x, size, line_h) = ProseKind::Body.metrics();
        self.prose(s, x, size, line_h);
    }

    /// A caption-sized line (the form's small print: formulas, footnotes).
    pub(crate) fn small_line(&mut self, s: &str) {
        let (x, size, line_h) = ProseKind::Small.metrics();
        self.prose(s, x, size, line_h);
    }

    /// A body line indented under its lettered parent (the §4 candidates, the zoning lines).
    pub(crate) fn indent_line(&mut self, s: &str) {
        let (x, size, line_h) = ProseKind::Indent.metrics();
        self.prose(s, x, size, line_h);
    }

    /// One line of prose from `x`, wrapped at the right margin (the 7.5 walk: a reader's long note
    /// ran off the page) — each further line takes another `line_h`.
    fn prose(&mut self, s: &str, x: f32, size: f32, line_h: f32) {
        for chunk in wrap_to_width(s, self.right() - x, size) {
            self.ensure(line_h);
            self.y += size;
            text(&mut self.cur, x, self.y, size, &chunk);
            self.y += line_h - size;
        }
    }

    /// The height a [`Block`] takes once laid out — every wrapped line counted (G1 F).
    fn block_height(&self, b: &Block) -> f32 {
        b.lines
            .iter()
            .map(|(kind, s)| {
                let (x, size, line_h) = kind.metrics();
                wrap_to_width(s, self.right() - x, size).len() as f32 * line_h
            })
            .sum()
    }

    /// Lay out a [`Block`]'s lines, in order.
    fn block(&mut self, b: &Block) {
        for (kind, s) in &b.lines {
            let (x, size, line_h) = kind.metrics();
            self.prose(s, x, size, line_h);
        }
    }

    /// Two facts on one line, at the left and at the page's middle (the form's paired growth lines).
    pub(crate) fn two_columns(&mut self, left: &str, right: &str) {
        self.ensure(LINE_H);
        self.y += FONT;
        text(&mut self.cur, MARGIN, self.y, FONT, left);
        text(&mut self.cur, self.page_w / 2.0 + 6.0, self.y, FONT, right);
        self.y += LINE_H - FONT;
    }

    /// Issue #207 — the form's identity block: a boxed grid of `label : value` pairs, `rows` rows
    /// of three pairs each. Labels in small print above the values, the box ruled between columns.
    /// G1 F: each label and value is fitted to its column's padded width at the real Helvetica
    /// widths (an over-long company or dossier name ends in « … »), never cut by a character
    /// count that lets a wide name run over the next cell.
    ///
    /// G1 final (L7): a value may carry an explicit line break (`\n`) — its row grows by a line
    /// (the « Données » value keeps « et saisie manuelle » on its own line when the providers
    /// fill the first).
    pub(crate) fn header_box(&mut self, rows: &[[(&str, &str); 3]]) {
        let value_step = FONT + 1.5;
        let row_h = |row: &[(&str, &str); 3]| {
            let lines = row
                .iter()
                .map(|(_, v)| v.split('\n').count())
                .max()
                .unwrap_or(1);
            LINE_H + SMALL + 2.0 + (lines - 1) as f32 * value_step
        };
        let h = rows.iter().map(row_h).sum::<f32>() + 4.0;
        self.ensure(h + 4.0);
        let (x0, x1) = (MARGIN, self.page_w - MARGIN);
        let col_w = (x1 - x0) / 3.0;
        let top = self.y;
        stroke_rect(&mut self.cur, x0, top, x1 - x0, h, 0.6);
        for c in 1..3 {
            vline(&mut self.cur, x0 + col_w * c as f32, top, top + h, 0.4);
        }
        let mut ry = top + 2.0;
        for (r, row) in rows.iter().enumerate() {
            if r > 0 {
                hline(&mut self.cur, x0, x1, ry - 1.0, 0.4);
            }
            let room = header_room(col_w);
            for (c, (label, value)) in row.iter().enumerate() {
                let x = x0 + col_w * c as f32 + CELL_PAD;
                text(
                    &mut self.cur,
                    x,
                    ry + SMALL,
                    SMALL,
                    &fit(label, room, SMALL),
                );
                for (k, line) in value.split('\n').enumerate() {
                    text(
                        &mut self.cur,
                        x,
                        ry + SMALL + value_step + k as f32 * value_step,
                        FONT,
                        &fit(line, room, FONT),
                    );
                }
            }
            ry += row_h(row);
        }
        self.y = top + h + 3.0;
    }

    /// A grid row whose cells from index `numeric_from` are RIGHT-aligned inside their column (the
    /// form's figures line up on their units); the cells before stay left-aligned (labels).
    pub(crate) fn grid_row_num(
        &mut self,
        cells: &[&str],
        edges: &[f32],
        head: bool,
        numeric_from: usize,
    ) {
        self.grid_font = FONT;
        self.grid_row_num_sized(cells, edges, head, numeric_from);
    }

    /// [`grid_row_num`] in the caption size (a wide table such as §2's ten year columns).
    pub(crate) fn grid_row_small(
        &mut self,
        cells: &[&str],
        edges: &[f32],
        head: bool,
        numeric_from: usize,
    ) {
        self.grid_font = SMALL;
        self.grid_row_num_sized(cells, edges, head, numeric_from);
    }

    /// A grid row whose cells in `numeric` are RIGHT-aligned (the figures) and the others
    /// left-aligned (labels, notes) — [`grid_row_num`] with an explicit range, for a table whose
    /// last column is prose (the review's « Remarque »).
    pub(crate) fn grid_row_range(
        &mut self,
        cells: &[&str],
        edges: &[f32],
        head: bool,
        numeric: std::ops::Range<usize>,
    ) {
        self.grid_font = FONT;
        self.grid_row_sized(cells, edges, head, numeric);
    }

    fn grid_row_num_sized(
        &mut self,
        cells: &[&str],
        edges: &[f32],
        head: bool,
        numeric_from: usize,
    ) {
        self.grid_row_sized(cells, edges, head, numeric_from..usize::MAX);
    }

    fn grid_row_sized(
        &mut self,
        cells: &[&str],
        edges: &[f32],
        head: bool,
        numeric: std::ops::Range<usize>,
    ) {
        if head && !self.grid_started {
            // A header row: held back and drawn with the first body row (G1 C).
            self.grid_header.push(GridHeaderRow {
                cells: cells.iter().map(|s| s.to_string()).collect(),
                numeric,
                font: self.grid_font,
            });
            return;
        }
        let height = self.grid_row_height(cells, edges);
        self.grid_break_before(height, edges);
        self.draw_grid_cells_aligned(cells, edges, head, numeric);
    }

    /// Make room for the next `need` points of grid rows (G1 C / D). Before the first body row the
    /// held-back header rows are drawn first, and they move to the next page WITH that row when
    /// both do not fit (a header row is never orphaned at a page foot). Later, a row that does not
    /// fit closes the box, starts a new page and replays EVERY header row there. A block taller
    /// than a whole page is not chased from page to page: at the top of a page it is drawn as is.
    fn grid_break_before(&mut self, need: f32, edges: &[f32]) {
        let at_top = self.y <= MARGIN + 0.5;
        if !self.grid_started {
            let head_h = self.grid_header_height(edges);
            if self.page_h - self.y - head_h - need < BOTTOM && !at_top {
                // Nothing of this grid is on the page yet: no box to close.
                self.new_page();
                self.grid_top = self.y;
                self.grid_spans.clear();
            }
            self.draw_grid_header(edges);
            self.grid_started = true;
            return;
        }
        if self.page_h - self.y - need < BOTTOM && !at_top {
            self.close_grid_box(edges);
            self.new_page();
            self.grid_top = self.y;
            self.draw_grid_header(edges);
        }
    }

    /// The height the current grid's header rows take together.
    fn grid_header_height(&mut self, edges: &[f32]) -> f32 {
        let font = self.grid_font;
        let header = std::mem::take(&mut self.grid_header);
        let mut h = 0.0;
        for row in &header {
            self.grid_font = row.font;
            let cells: Vec<&str> = row.cells.iter().map(String::as_str).collect();
            h += self.grid_row_height(&cells, edges);
        }
        self.grid_header = header;
        self.grid_font = font;
        h
    }

    /// Draw every header row of the current grid at the cursor, each in its own size.
    fn draw_grid_header(&mut self, edges: &[f32]) {
        let font = self.grid_font;
        let header = std::mem::take(&mut self.grid_header);
        for row in &header {
            self.grid_font = row.font;
            let cells: Vec<&str> = row.cells.iter().map(String::as_str).collect();
            self.draw_grid_cells_aligned(&cells, edges, true, row.numeric.clone());
        }
        self.grid_header = header;
        self.grid_font = font;
    }

    /// Each cell's lines and point size, laid out in its column by [`cell_layout`] (a cell never
    /// crosses a rule, and a figure is never cut).
    fn grid_cell_lines(&self, cells: &[&str], edges: &[f32]) -> Vec<(Vec<String>, f32)> {
        cells
            .iter()
            .enumerate()
            .map(|(i, s)| match edges.get(i + 1) {
                Some(right) => cell_layout(s, right - edges[i], self.grid_font),
                None => (vec![s.to_string()], self.grid_font),
            })
            .collect()
    }

    /// The extra height a wrapped cell's further lines take.
    fn grid_line_step(&self) -> f32 {
        self.grid_font + 2.5
    }

    fn grid_row_height(&self, cells: &[&str], edges: &[f32]) -> f32 {
        let lines = self
            .grid_cell_lines(cells, edges)
            .iter()
            .map(|(l, _)| l.len())
            .max()
            .unwrap_or(1)
            .max(1);
        LINE_H + (lines - 1) as f32 * self.grid_line_step()
    }

    /// The height one grid row will take at `font` — measured before a table is drawn (G1 final,
    /// L10: the §3 block is reserved whole).
    fn grid_rows_height(&mut self, cells: &[&str], edges: &[f32], font: f32) -> f32 {
        let saved = self.grid_font;
        self.grid_font = font;
        let h = self.grid_row_height(cells, edges);
        self.grid_font = saved;
        h
    }

    /// The body of a grid row: its cells at the cursor, wrapped within their columns, the ones in
    /// `numeric` right-aligned; a header row is underlined across the table width.
    fn draw_grid_cells_aligned(
        &mut self,
        cells: &[&str],
        edges: &[f32],
        head: bool,
        numeric: std::ops::Range<usize>,
    ) {
        let step = self.grid_line_step();
        let lines = self.grid_cell_lines(cells, edges);
        let rows = lines.iter().map(|(l, _)| l.len()).max().unwrap_or(1).max(1);
        let top = self.y + self.grid_font;
        for (i, (cell_lines, size)) in lines.iter().enumerate() {
            let size = *size;
            for (k, line) in cell_lines.iter().enumerate() {
                let y = top + k as f32 * step;
                let Some(right) = edges.get(i + 1) else {
                    text(&mut self.cur, edges[i] + CELL_PAD, y, size, line);
                    continue;
                };
                let w = text_width(line, size);
                let x = cell_x(numeric.contains(&i), edges[i], *right, w);
                text(&mut self.cur, x, y, size, line);
            }
        }
        self.y = top + (rows - 1) as f32 * step + (LINE_H - self.grid_font);
        if head {
            hline(
                &mut self.cur,
                edges[0],
                edges[edges.len() - 1],
                self.y - 1.5,
                0.6,
            );
        }
    }

    /// Issue #104 — start a boxed grid table. Reserve only the header + first row together (the
    /// section heading already reserved a few rows), and record the table top so [`grid_end`] can
    /// draw the outer box + column rules. Issue #74: a grid may SPAN page breaks — a body row that
    /// does not fit closes the box at a break and replays the header rows on the continuation page.
    pub(crate) fn grid_begin(&mut self, _rows: usize) {
        self.keep_together(2.0 * LINE_H + 4.0);
        self.grid_top = self.y;
        self.grid_header.clear();
        self.grid_started = false;
        self.grid_spans.clear();
    }

    /// Draw the grid's outer box from [`grid_top`] to the current cursor + a vertical rule at each
    /// interior column boundary. Called at each page break (for the portion on the closing page) and
    /// once more by [`grid_end`] for the final portion — the visible SSG grid (high-fidelity forms).
    fn close_grid_box(&mut self, edges: &[f32]) {
        let (top, bottom) = (self.grid_top - 1.0, self.y + 1.0);
        let left = edges[0];
        let right = edges[edges.len() - 1];
        stroke_rect(&mut self.cur, left, top, right - left, bottom - top, 0.6);
        // The interior rules run from the top down, skipping every note row's extent.
        let mut spans = std::mem::take(&mut self.grid_spans);
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        for e in &edges[1..edges.len() - 1] {
            let mut from = top;
            for (s_top, s_bottom) in &spans {
                if *s_top > from {
                    vline(&mut self.cur, *e, from, *s_top, 0.4);
                }
                from = from.max(*s_bottom);
            }
            if bottom > from {
                vline(&mut self.cur, *e, from, bottom, 0.4);
            }
        }
    }

    /// A body row followed by its small-print note, spanning from `note_left` to the table's
    /// right edge and wrapped there; the interior column rules stop above the note and resume
    /// below (the review's « Signaux · Données » line under each position). G1 D: the row and
    /// its note are ONE block — a page break never falls between them (the note would read as
    /// the next position's, or as nobody's).
    pub(crate) fn grid_row_num_with_note(
        &mut self,
        cells: &[&str],
        edges: &[f32],
        numeric_from: usize,
        note: &str,
        note_left: f32,
    ) {
        let right = edges[edges.len() - 1];
        let outer = [edges[0], note_left, right];
        self.grid_font = FONT;
        let row_h = self.grid_row_height(cells, edges);
        self.grid_font = SMALL;
        let note_h = self.grid_row_height(&["", note], &outer);
        self.grid_font = FONT;
        self.grid_break_before(row_h + note_h, edges);
        self.draw_grid_cells_aligned(cells, edges, false, numeric_from..usize::MAX);
        self.grid_font = SMALL;
        let from = self.y;
        self.draw_grid_cells_aligned(&["", note], &outer, false, usize::MAX..usize::MAX);
        self.grid_spans.push((from, self.y));
        self.grid_font = FONT;
    }

    /// Close the grid: draw its header rows if no body row did (a header-only table), box the
    /// final (or only) page's portion, then advance past it.
    pub(crate) fn grid_end(&mut self, edges: &[f32]) {
        if !self.grid_started {
            self.grid_break_before(0.0, edges);
        }
        self.close_grid_box(edges);
        self.grid_header.clear();
        self.grid_started = false;
        self.y += 2.0;
    }

    /// Issue #105 / #207 — the §1 semi-log growth chart, filling the rest of page 1 like the printed
    /// form. Sales / EPS / Price on log scales (each series its own — issue #25; the EPS scale is the
    /// labelled one), the yearly high–low PRICE as vertical bars, the est-high / est-low EPS
    /// projection over the forecast horizon, and the form's growth GUIDE lines (5–30 % compound
    /// from the last positive EPS point, light grey, labelled at their end). Greyscale-safe:
    /// weight + dash + shade, NEVER colour. Nothing is drawn when there is no plottable data (the
    /// annexe already carries the em-dashes).
    ///
    /// G1 F: the x axis is by YEAR (a gap year keeps its place, the years after it do not slide
    /// left); the guides and the projection run from their anchor year over exactly the horizon, so
    /// each is drawn at the slope it is labelled with; the plot's height is measured after any page
    /// break; a year with only one of its high / low prices still shows that price as a tick; and
    /// the form's quarterly box sits BELOW the plot (owner decision 7), never over plotted data.
    fn growth_chart(&mut self, frame: &crate::form::StudyFrame, nf: NumberStyle) {
        use steadyinvest_core::normalize::YearUsability;
        let series = &frame.series;
        let outputs = frame.snapshot.outputs();
        let pts_of = |get: &dyn Fn(&CanonicalYear) -> Option<Decimal>| {
            series
                .iter()
                .filter_map(|cy| {
                    get(cy)
                        .and_then(|d| d.to_f64())
                        .filter(|v| v.is_finite() && *v > 0.0)
                        .map(|v| (cy.year, v))
                })
                .collect::<Vec<(i32, f64)>>()
        };
        let sales = pts_of(&|cy| cy.sales);
        let eps = pts_of(&|cy| cy.eps);
        let highs = pts_of(&|cy| cy.high_price);
        let lows = pts_of(&|cy| cy.low_price);
        let est_high = outputs
            .growth
            .estimated_high_eps
            .and_then(|d| d.to_f64())
            .filter(|v| v.is_finite() && *v > 0.0);
        let est_low = outputs
            .growth
            .estimated_low_eps
            .and_then(|d| d.to_f64())
            .filter(|v| v.is_finite() && *v > 0.0);

        let (Some(first_year), Some(last_year)) = (
            series.iter().map(|y| y.year).min(),
            series.iter().map(|y| y.year).max(),
        ) else {
            return;
        };
        if sales.is_empty() && eps.is_empty() && highs.is_empty() && lows.is_empty() {
            return;
        }
        let horizon = FORECAST_HORIZON_YEARS as i32;
        // The guides start from the last POSITIVE EPS point (a log scale has no place for the
        // others), named in the scale note — it may lie years before the latest year.
        let anchor = eps.last().copied();
        // G1 F review — the projection starts where the estimates start: they are the EPS a
        // horizon after the latest usable year, compounded from THAT year's EPS. When that EPS is
        // not positive there is no honest start on a log scale — a line from an older positive
        // EPS would draw a path the estimates do not describe — so the projection is not drawn,
        // and the note says why.
        let base_year = series
            .iter()
            .filter(|y| matches!(y.usability, YearUsability::Usable))
            .map(|y| y.year)
            .max();
        let projection_start = base_year.and_then(|b| eps.iter().find(|p| p.0 == b).copied());
        let has_estimate = est_high.is_some() || est_low.is_some();
        let scale_note = chart_scale_note(
            anchor.map(|a| a.0),
            base_year,
            projection_start.is_some(),
            has_estimate,
        );

        // What goes under the plot: the year labels, the legend and the scale note (measured with
        // their wrapped lines), the quarterly box, then the caller's gap and four growth lines.
        let small_h = |s: &str| {
            let (x, size, line_h) = ProseKind::Small.metrics();
            wrap_to_width(s, self.right() - x, size).len() as f32 * line_h
        };
        let reserved_below = 13.0
            + small_h(CHART_LEGEND)
            + small_h(&scale_note)
            + QUARTER_BOX_GAP
            + QUARTER_BOX_H
            + QUARTER_BOX_GAP
            + 2.0
            + 2.0 * LINE_H;
        // Break first, THEN measure: after a page break the plot fills the new page too.
        self.ensure(CHART_MIN_H + reserved_below);
        let chart_h = (self.page_h - self.y - BOTTOM - reserved_below).max(CHART_MIN_H);
        let top = self.y;
        let x0 = MARGIN + CHART_AXIS_W;
        let x1 = self.page_w - MARGIN;
        let plot_w = x1 - x0;
        // The horizontal domain: the first year … the last year + the forecast horizon, with half a
        // year of room at each end (G1 final, L5: the first year's price bar is never drawn on the
        // frame's edge).
        let span = f64::from(last_year - first_year + horizon) + 2.0 * YEAR_PAD;
        let origin = f64::from(first_year) - YEAR_PAD;
        let px = |year: f64| year_x(year, origin, span, x0, plot_w);
        let py = |v: f64, lmin: f64, lmax: f64| {
            let t = ((v.max(1e-9).log10() - lmin) / (lmax - lmin)).clamp(0.0, 1.0);
            top + (f64::from(chart_h) * (1.0 - t)) as f32
        };

        // Issue #25 (multi-scale): each series on its OWN log range so none is crushed by another's
        // magnitude. The EPS scale (the decision series, its projection and the guides) is labelled.
        let vals = |pts: &[(i32, f64)]| pts.iter().map(|p| p.1).collect::<Vec<f64>>();
        let sales_b = series_log_bounds(&vals(&sales));
        let mut price_vals = vals(&highs);
        price_vals.extend(vals(&lows));
        let price_b = series_log_bounds(&price_vals);
        let mut eps_scale_vals = vals(&eps);
        eps_scale_vals.extend(est_high);
        eps_scale_vals.extend(est_low);
        // The steepest guide's end reserves headroom — kept only when finite and positive.
        if let Some((ly, lv)) = anchor {
            let (_, top_guide) = guide_end(ly, lv, GUIDE_RATES_PCT[GUIDE_RATES_PCT.len() - 1]);
            eps_scale_vals.extend(Some(top_guide).filter(|v| v.is_finite() && *v > 0.0));
        }
        let eps_b = series_log_bounds(&eps_scale_vals);

        stroke_rect(&mut self.cur, x0, top, plot_w, chart_h, 0.6);
        // Gridlines + labels on the EPS scale (nice 1/2/5×10^k).
        if let Some((lmin, lmax)) = eps_b {
            for (v, lbl) in nice_ticks(lmin, lmax, nf) {
                let gy = py(v, lmin, lmax);
                polyline(&mut self.cur, &[(x0, gy), (x1, gy)], 0.3, GRID_GRAY, &[]);
                text(&mut self.cur, MARGIN, gy + 2.5, 7.0, &lbl);
            }
        }
        // Issue #207 — the growth guide lines: from the last positive EPS point, each rate
        // compounded over the horizon and ending at the anchor year + the horizon, light grey,
        // labelled at their end (the printed form's fan).
        if let (Some((lmin, lmax)), Some((ly, lv))) = (eps_b, anchor) {
            let (ox, oy) = (px(f64::from(ly)), py(lv, lmin, lmax));
            for rate in GUIDE_RATES_PCT {
                let (ey_year, end) = guide_end(ly, lv, rate);
                let (ex, ey) = (px(f64::from(ey_year)), py(end, lmin, lmax));
                polyline(&mut self.cur, &[(ox, oy), (ex, ey)], 0.4, GUIDE_GRAY, &[]);
                text(
                    &mut self.cur,
                    ex.min(x1) - 19.0,
                    ey - 2.0,
                    5.5,
                    &format!("{rate} %"),
                );
            }
        }
        // The yearly high–low price bars (price scale): a vertical segment with short caps. A year
        // with only one of the two prices still shows it, as a lone cap (never dropped).
        if let Some((lmin, lmax)) = price_b {
            let mut years: Vec<i32> = highs.iter().chain(lows.iter()).map(|p| p.0).collect();
            years.sort_unstable();
            years.dedup();
            let cap = |cur: &mut Content, x: f32, y: f32| {
                polyline(cur, &[(x - 2.0, y), (x + 2.0, y)], 0.8, SERIES_GRAY, &[]);
            };
            for year in years {
                let x = px(f64::from(year));
                let hi = highs
                    .iter()
                    .find(|p| p.0 == year)
                    .map(|p| py(p.1, lmin, lmax));
                let lo = lows
                    .iter()
                    .find(|p| p.0 == year)
                    .map(|p| py(p.1, lmin, lmax));
                if let (Some(yh), Some(yl)) = (hi, lo) {
                    polyline(&mut self.cur, &[(x, yh), (x, yl)], 0.8, SERIES_GRAY, &[]);
                }
                for y in hi.into_iter().chain(lo) {
                    cap(&mut self.cur, x, y);
                }
            }
        }
        // The Sales (thin) and EPS (thick) lines, each on its own scale (greyscale: weight). G1
        // final (M2 / L5): each line is drawn run by run — it breaks at a missing year and at a
        // value the log scale cannot hold, and a lone point shows as a dot.
        let draw = |cur: &mut Content, runs: &[Vec<(i32, f64)>], b: Option<(f64, f64)>, w: f32| {
            let Some((lmin, lmax)) = b else {
                return;
            };
            for run in runs {
                let p: Vec<(f32, f32)> = run
                    .iter()
                    .map(|(year, v)| (px(f64::from(*year)), py(*v, lmin, lmax)))
                    .collect();
                match p.as_slice() {
                    [(x, y)] => {
                        let r = w + 0.6;
                        fill_rect(cur, x - r, y - r, 2.0 * r, 2.0 * r, SERIES_GRAY);
                    }
                    _ => polyline(cur, &p, w, SERIES_GRAY, &[]),
                }
            }
        };
        let runs_of = |get: &dyn Fn(&CanonicalYear) -> Option<Decimal>| {
            plot_runs(
                &series
                    .iter()
                    .map(|cy| (cy.year, get(cy).and_then(|d| d.to_f64())))
                    .collect::<Vec<_>>(),
            )
        };
        draw(&mut self.cur, &runs_of(&|cy| cy.sales), sales_b, 0.8);
        draw(&mut self.cur, &runs_of(&|cy| cy.eps), eps_b, 1.6);
        // Projection to est-high / est-low (dotted, EPS scale): from the latest usable year's EPS
        // (the estimates' base) to that year + the horizon — or not drawn (see `base_year`).
        if let (Some((lmin, lmax)), Some((by, bv))) = (eps_b, projection_start) {
            let (ox, oy) = (px(f64::from(by)), py(bv, lmin, lmax));
            let ex = px(f64::from(by + horizon));
            for (est, w) in [(est_high, 1.2), (est_low, 1.0)] {
                if let Some(v) = est {
                    polyline(
                        &mut self.cur,
                        &[(ox, oy), (ex, py(v, lmin, lmax))],
                        w,
                        SERIES_GRAY,
                        &[1.5, 2.0],
                    );
                }
            }
        }
        // Issue #104 — year labels along the x-axis (each historical year under its own place).
        for cy in series {
            text_centered(
                &mut self.cur,
                px(f64::from(cy.year)),
                top + chart_h + 9.0,
                6.5,
                &cy.year.to_string(),
            );
        }
        self.y = top + chart_h + 13.0;
        self.small_line(CHART_LEGEND);
        self.small_line(&scale_note);
        // Issue #207 — the form's « recent quarterly figures » box, under the plot (owner decision
        // 7: never over plotted data). v1 carries no quarterly data: the box states the absence
        // (em-dashes), never a guessed figure.
        self.y += QUARTER_BOX_GAP;
        let (bx, by, bw, bh) = (x0, self.y, QUARTER_BOX_W, QUARTER_BOX_H);
        stroke_rect(&mut self.cur, bx, by, bw, bh, 0.4);
        let mut ty = by + 4.0 + SMALL;
        text(&mut self.cur, bx + 4.0, ty, SMALL, QUARTER_BOX_TITLE);
        text_right(&mut self.cur, bx + bw - 44.0, ty, SMALL, "Ventes");
        text_right(&mut self.cur, bx + bw - 4.0, ty, SMALL, "BPA");
        for label in [QUARTER_LATEST, QUARTER_YEAR_AGO, QUARTER_CHANGE] {
            ty += SMALL + 3.0;
            text(&mut self.cur, bx + 4.0, ty, SMALL, label);
            text_right(&mut self.cur, bx + bw - 44.0, ty, SMALL, EM_DASH);
            text_right(&mut self.cur, bx + bw - 4.0, ty, SMALL, EM_DASH);
        }
        self.y = by + bh + QUARTER_BOX_GAP;
    }

    /// Issue #105 — the §4 zone bar. A horizontal band from forecast-low to forecast-high split into
    /// the three thirds (low / median / high), greyscale-shaded (light → dark) with a label in each,
    /// and a marker at the current price. Greyscale-safe: the bands read by shade + label + position,
    /// never hue. Nothing is drawn when the forecast is incomplete (the §4 text already says so).
    fn zone_bar(
        &mut self,
        zones: Option<&ZoneBounds>,
        current_price: Option<Decimal>,
        nf: NumberStyle,
    ) {
        let Some(z) = zones else {
            return;
        };
        let lo = z.forecast_low.to_f64().unwrap_or(0.0);
        let hi = z.forecast_high.to_f64().unwrap_or(0.0);
        let buy = z.buy_top.to_f64().unwrap_or(0.0);
        let neu = z.neutral_top.to_f64().unwrap_or(0.0);
        if hi <= lo {
            return;
        }
        self.ensure(ZONEBAR_H_RESERVE);
        let (x0, x1) = (MARGIN, self.page_w - MARGIN);
        let w = x1 - x0;
        let top = self.y + 10.0; // room above for the current-price marker label
        let fx =
            |price: f64| x0 + (((price - lo) / (hi - lo)).clamp(0.0, 1.0) * f64::from(w)) as f32;

        // Three thirds, greyscale shades (low third lightest, high third darkest).
        fill_rect(&mut self.cur, x0, top, fx(buy) - x0, ZONEBAR_H, 0.86);
        fill_rect(
            &mut self.cur,
            fx(buy),
            top,
            fx(neu) - fx(buy),
            ZONEBAR_H,
            0.70,
        );
        fill_rect(&mut self.cur, fx(neu), top, x1 - fx(neu), ZONEBAR_H, 0.52);
        stroke_rect(&mut self.cur, x0, top, w, ZONEBAR_H, 0.5);

        // Zone labels centered in each third.
        let mid_y = top + ZONEBAR_H / 2.0 + 3.0;
        text_centered(&mut self.cur, (x0 + fx(buy)) / 2.0, mid_y, 8.0, ZONE_LOW);
        text_centered(
            &mut self.cur,
            (fx(buy) + fx(neu)) / 2.0,
            mid_y,
            8.0,
            ZONE_MID,
        );
        text_centered(&mut self.cur, (fx(neu) + x1) / 2.0, mid_y, 8.0, ZONE_HIGH);

        // Boundary prices under the bar.
        let by = top + ZONEBAR_H + 9.0;
        text(&mut self.cur, x0, by, 7.0, &nf.money(Some(z.forecast_low)));
        text_centered(&mut self.cur, fx(buy), by, 7.0, &nf.money(Some(z.buy_top)));
        text_centered(
            &mut self.cur,
            fx(neu),
            by,
            7.0,
            &nf.money(Some(z.neutral_top)),
        );
        let hi_lbl = nf.money(Some(z.forecast_high));
        text_right(&mut self.cur, x1, by, 7.0, &hi_lbl);

        // Current-price marker: a vertical line through the bar + a caption above. G1 final (L9):
        // a price outside the range is never pinned on the edge as if it stood at the edge price —
        // an arrow at that edge points OUT of the bar and the caption says the price is outside.
        // No price → no marker (M1; the §4 text says it is absent).
        let caption = |outside: Option<&str>| match outside {
            None => format!("{CURRENT_PRICE} {}", nf.money(current_price)),
            Some(side) => format!("{CURRENT_PRICE} {} — {side}", nf.money(current_price)),
        };
        match price_place(z, current_price) {
            PricePlace::In(_) => {
                let cp = current_price.and_then(|d| d.to_f64()).unwrap_or(lo);
                let mx = fx(cp);
                polyline(
                    &mut self.cur,
                    &[(mx, top - 4.0), (mx, top + ZONEBAR_H + 2.0)],
                    1.3,
                    0.0,
                    &[],
                );
                // Kept inside the margins: a price near an edge never runs its caption off.
                let label = caption(None);
                let half = text_width(&label, 7.0) / 2.0;
                let cx = mx.min(x1 - half).max(x0 + half);
                text_centered(&mut self.cur, cx, top - 6.0, 7.0, &label);
            }
            PricePlace::Below => {
                edge_arrow(&mut self.cur, x0, top + ZONEBAR_H / 2.0, -1.0);
                let label = caption(Some(MARKER_BELOW));
                text(&mut self.cur, x0, top - 4.0, 7.0, &label);
            }
            PricePlace::Above => {
                edge_arrow(&mut self.cur, x1, top + ZONEBAR_H / 2.0, 1.0);
                let label = caption(Some(MARKER_ABOVE));
                text_right(&mut self.cur, x1, top - 4.0, 7.0, &label);
            }
            PricePlace::Absent => {}
        }
        self.y = by + 4.0;
    }

    /// Stamp the footer disclaimer on a page's content (FR64 — every page).
    fn footer(content: &mut Content, page_h: f32) {
        text(
            content,
            MARGIN,
            page_h - MARGIN,
            8.0,
            "Outil éducatif — ne constitue pas un conseil financier.",
        );
    }

    pub(crate) fn finish(mut self) -> Vec<u8> {
        // Close the page in progress.
        let last = std::mem::replace(&mut self.cur, Content::new());
        self.pages.push(last);

        let mut pdf = Pdf::new();
        let catalog = Ref::new(1);
        let tree = Ref::new(2);
        let font = Ref::new(3);
        let bold = Ref::new(4);
        // Two refs per page (page object + its content stream), after the four fixed refs.
        let page_ids: Vec<Ref> = (0..self.pages.len())
            .map(|i| Ref::new(5 + 2 * i as i32))
            .collect();
        let content_ids: Vec<Ref> = (0..self.pages.len())
            .map(|i| Ref::new(6 + 2 * i as i32))
            .collect();

        pdf.catalog(catalog).pages(tree);
        pdf.pages(tree)
            .kids(page_ids.iter().copied())
            .count(self.pages.len() as i32);
        // Helvetica + Helvetica-Bold (standard-14, no embedding) with WinAnsi so French accents render.
        pdf.type1_font(font)
            .base_font(Name(b"Helvetica"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.type1_font(bold)
            .base_font(Name(b"Helvetica-Bold"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));

        let (page_w, page_h) = (self.page_w, self.page_h);
        for (i, mut content) in self.pages.into_iter().enumerate() {
            Doc::footer(&mut content, page_h);
            {
                let mut page = pdf.page(page_ids[i]);
                page.parent(tree)
                    .media_box(Rect::new(0.0, 0.0, page_w, page_h))
                    .contents(content_ids[i]);
                let mut resources = page.resources();
                let mut fonts = resources.fonts();
                fonts.pair(Name(b"F0"), font);
                fonts.pair(Name(b"F1"), bold);
            }
            pdf.stream(content_ids[i], &content.finish());
        }
        pdf.finish()
    }
}

/// The padded room of a [`Doc::header_box`] value in a column `col_w` wide.
fn header_room(col_w: f32) -> f32 {
    col_w - 2.0 * CELL_PAD
}

/// A page's content stream: black text, mid-grey rules; on a non-A4-portrait page, a translate
/// that maps the primitives' PAGE_H y-flip onto the page's own height (see `Doc::page_h`).
fn fresh_content(page_h: f32) -> Content {
    let mut c = Content::new();
    if (page_h - PAGE_H).abs() > 0.5 {
        c.transform([1.0, 0.0, 0.0, 1.0, 0.0, page_h - PAGE_H]);
    }
    c.set_fill_gray(0.0); // black text
    c.set_stroke_gray(0.35); // mid-grey rules (greyscale only)
    c
}

/// Place `s` at top-origin `(x, top_y)` in Helvetica `size`, encoded as WinAnsi so French accents
/// render. PDF's origin is bottom-left, so the y is flipped here.
pub(crate) fn text(content: &mut Content, x: f32, top_y: f32, size: f32, s: &str) {
    content.begin_text();
    content.set_font(Name(b"F0"), size);
    content.set_text_matrix([1.0, 0.0, 0.0, 1.0, x, PAGE_H - top_y]);
    let bytes = winansi(s);
    content.show(Str(&bytes));
    content.end_text();
}

/// [`text`] in Helvetica-Bold (the headings).
pub(crate) fn text_bold(content: &mut Content, x: f32, top_y: f32, size: f32, s: &str) {
    content.begin_text();
    content.set_font(Name(b"F1"), size);
    content.set_text_matrix([1.0, 0.0, 0.0, 1.0, x, PAGE_H - top_y]);
    let bytes = winansi(s);
    content.show(Str(&bytes));
    content.end_text();
}

/// [`text`] with its RIGHT edge at `x_right` (the real Helvetica widths, [`text_width`]) — the
/// figures of a table line up on their units.
pub(crate) fn text_right(content: &mut Content, x_right: f32, top_y: f32, size: f32, s: &str) {
    text(content, x_right - text_width(s, size), top_y, size, s);
}

/// Helvetica advance widths for ASCII 32..=126, in 1/1000 em (Adobe's standard-14 AFM metrics —
/// the font every PDF reader ships, so the numbers are exact, not an estimate).
const HELVETICA_ASCII: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
    278, // ' '…'/'
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // '0'…'9'
    278, 278, 584, 584, 584, 556, 1015, // ':'…'@'
    667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, // 'A'…'M'
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, // 'N'…'Z'
    278, 278, 278, 469, 556, 333, // '['…'`'
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, // 'a'…'m'
    556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, // 'n'…'z'
    334, 260, 334, 584, // '{'…'~'
];

/// G1 F — Helvetica advance widths for the WinAnsi upper half 0x80..=0xFF, by code (the same AFM:
/// œ 944, Œ Æ ‰ ™ … 1000, ß ø 611, æ 889, © ® 737, • 350 …). The five codes WinAnsi leaves
/// undefined (0x81 0x8D 0x8F 0x90 0x9D) are never emitted by [`winansi_byte`]; they carry '?'s 556.
#[rustfmt::skip]
const HELVETICA_HIGH: [u16; 128] = [
    // 0x80 € ? ‚ ƒ „ … † ‡ ˆ ‰ Š ‹ Œ ? Ž ?
    556, 556, 222, 556, 333, 1000, 556, 556, 333, 1000, 667, 333, 1000, 556, 611, 556,
    // 0x90 ? ‘ ’ “ ” • – — ˜ ™ š › œ ? ž Ÿ
    556, 222, 222, 333, 333, 350, 556, 1000, 333, 1000, 500, 333, 944, 556, 500, 667,
    // 0xA0 nbsp ¡ ¢ £ ¤ ¥ ¦ § ¨ © ª « ¬ shy ® ¯
    278, 333, 556, 556, 556, 556, 260, 556, 333, 737, 370, 556, 584, 333, 737, 333,
    // 0xB0 ° ± ² ³ ´ µ ¶ · ¸ ¹ º » ¼ ½ ¾ ¿
    400, 584, 333, 333, 333, 556, 537, 278, 333, 333, 365, 556, 834, 834, 834, 611,
    // 0xC0 À Á Â Ã Ä Å Æ Ç È É Ê Ë Ì Í Î Ï
    667, 667, 667, 667, 667, 667, 1000, 722, 667, 667, 667, 667, 278, 278, 278, 278,
    // 0xD0 Ð Ñ Ò Ó Ô Õ Ö × Ø Ù Ú Û Ü Ý Þ ß
    722, 722, 778, 778, 778, 778, 778, 584, 778, 722, 722, 722, 722, 667, 667, 611,
    // 0xE0 à á â ã ä å æ ç è é ê ë ì í î ï
    556, 556, 556, 556, 556, 556, 889, 500, 556, 556, 556, 556, 278, 278, 278, 278,
    // 0xF0 ð ñ ò ó ô õ ö ÷ ø ù ú û ü ý þ ÿ
    556, 556, 556, 556, 556, 556, 556, 584, 611, 556, 556, 556, 556, 500, 556, 500,
];

/// One glyph's Helvetica width (1/1000 em), read off the byte [`winansi_byte`] writes for it — so
/// the measure is always the width of what is printed (anything unencodable prints as '?').
fn glyph_width(c: char) -> u16 {
    match winansi_byte(c) {
        b @ 0x20..=0x7E => HELVETICA_ASCII[usize::from(b) - 32],
        b @ 0x80..=0xFF => HELVETICA_HIGH[usize::from(b) - 0x80],
        _ => 0, // the C0 controls (and DEL) have no glyph
    }
}

/// The rendered width of `s` in Helvetica at `size` points.
pub(crate) fn text_width(s: &str, size: f32) -> f32 {
    s.chars().map(|c| f32::from(glyph_width(c))).sum::<f32>() * size / 1000.0
}

/// `s` cut to fit `width` points at `size`, ending with « … » when cut (never spilling over). When
/// not even the ellipsis fits (a zero or negative width), the result is empty — never wider than
/// asked.
pub(crate) fn fit(s: &str, width: f32, size: f32) -> String {
    if text_width(s, size) <= width {
        return s.to_string();
    }
    let room = width - text_width("…", size);
    if room < 0.0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0.0;
    for c in s.chars() {
        let w = f32::from(glyph_width(c)) * size / 1000.0;
        if used + w > room {
            break;
        }
        used += w;
        out.push(c);
    }
    format!("{}…", out.trim_end())
}

/// `s` broken at spaces into lines no wider than `width` points at `size`; a single word wider
/// than the line is cut with « … » ([`fit`]). An empty `s` is one empty line. A run of spaces
/// inside a line is kept (the « A   ·   B » separators); at a break it is dropped, so no line
/// starts or ends with the spaces it was broken at. A width ≤ 0 holds nothing: one empty line.
pub(crate) fn wrap_to_width(s: &str, width: f32, size: f32) -> Vec<String> {
    wrap(s, width, size, false)
}

/// Whether a word carries a figure (a digit) — such a word is never cut with « … » in a grid cell.
fn carries_figure(word: &str) -> bool {
    word.chars().any(|c| c.is_ascii_digit())
}

/// The shared line breaker of [`wrap_to_width`] and [`cell_layout`]. With `keep_figures` (a grid
/// cell), a line is never cut down to nothing and a line carrying a figure or the absence mark
/// « — » is never passed through [`fit`]: a figure wider than the line goes whole on its own line
/// (never split into pieces that would read as two numbers), and an absence is never erased.
fn wrap(s: &str, width: f32, size: f32, keep_figures: bool) -> Vec<String> {
    if !keep_figures && (width.is_nan() || width <= 0.0) {
        return vec![String::new()];
    }
    let push = |lines: &mut Vec<String>, line: &str| {
        let kept = if keep_figures && (carries_figure(line) || line == EM_DASH) {
            line.to_string()
        } else {
            match fit(line, width, size) {
                // A grid cell never shows nothing where its text was.
                cut if keep_figures && cut.is_empty() => line.to_string(),
                cut => cut,
            }
        };
        lines.push(kept);
    };
    // (spaces before, word): a run of spaces is remembered with the word it precedes.
    let mut tokens: Vec<(usize, &str)> = Vec::new();
    let mut spaces = 0;
    for (i, word) in s.split(' ').enumerate() {
        if i > 0 {
            spaces += 1;
        }
        if !word.is_empty() {
            tokens.push((spaces, word));
            spaces = 0;
        }
    }
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for (gap, word) in tokens {
        if line.is_empty() {
            line = word.to_string();
            continue;
        }
        let candidate = format!("{line}{}{word}", " ".repeat(gap.max(1)));
        if text_width(&candidate, size) <= width {
            line = candidate;
        } else {
            push(&mut lines, &line);
            line = word.to_string();
        }
    }
    push(&mut lines, &line);
    lines
}

/// The smallest type a grid figure is shrunk to (points). Below it the figure is printed whole at
/// this size, even across its column's rule — never split, never unreadable.
const MIN_FIGURE_FONT: f32 = 6.0;

/// One grid cell's lines and point size, for a column `col_w` wide, at the grid's `size`.
///
/// A text that fits between the rules at the grid's size stays whole on one line, even if it eats
/// into the padding (a narrow year column's « 2016 »); a longer one wraps at the padded width.
///
/// G1 F — a figure is shown whole, never cut to « 1… » (§2 over ten years, the annexe sales of a
/// JPY / KRW issuer): the widest word carrying a digit, when wider than the padded width, shrinks
/// the cell's type until it fits THAT width — so a shrunk figure ends on the column's normal right
/// edge and the column still reads on its units, the way a hand-filled form writes a long number
/// smaller. The type stops at [`MIN_FIGURE_FONT`]; a figure still too wide there is printed whole,
/// on its own line, across the rule if it must — never split into pieces. Only a word without a
/// digit (a label) may still end in « … »; a cell's text, and the absence mark « — », is never
/// reduced to nothing. A column whose padding leaves no room falls back on the room between the
/// rules; one with no room at all prints its text as is.
fn cell_layout(s: &str, col_w: f32, size: f32) -> (Vec<String>, f32) {
    let rule_w = col_w - 2.0 * GRID_INSET;
    if text_width(s, size) <= rule_w {
        return (vec![s.to_string()], size);
    }
    let pad_w = col_w - 2.0 * CELL_PAD;
    let room = if pad_w > 0.0 { pad_w } else { rule_w };
    if room.is_nan() || room <= 0.0 {
        return (vec![s.to_string()], size);
    }
    // A cell with a figure and no letter (« 1234,5 % », « 2,1 : 1 ») is ONE figure: it shrinks
    // whole, the unit kept on the number's line.
    let whole_figure = carries_figure(s) && !s.chars().any(char::is_alphabetic);
    let widest_figure = if whole_figure {
        text_width(s, size)
    } else {
        s.split(' ')
            .filter(|w| carries_figure(w))
            .map(|w| text_width(w, size))
            .fold(0.0_f32, f32::max)
    };
    let size = if widest_figure > room {
        // A hair under the exact ratio, so float rounding cannot tip it back over the room.
        (size * room / widest_figure * 0.999)
            .max(MIN_FIGURE_FONT)
            .min(size)
    } else {
        size
    };
    if whole_figure {
        return (vec![s.to_string()], size);
    }
    (wrap(s, room, size, true), size)
}

/// The x of a grid cell's text line `w` wide in the column `left..right`: a figure (`numeric`)
/// ends at the padded right edge, a label starts at the padded left edge; either is shifted back
/// inside the rules when it is wider than its padded room. A figure wider than the rules (printed
/// whole at the smallest type) keeps its right edge and crosses the LEFT rule, so the column
/// still reads on its units.
fn cell_x(numeric: bool, left: f32, right: f32, w: f32) -> f32 {
    if numeric {
        let x = right - CELL_PAD - w;
        if w <= right - left - 2.0 * GRID_INSET {
            x.max(left + GRID_INSET)
        } else {
            x
        }
    } else {
        (left + CELL_PAD)
            .min(right - GRID_INSET - w)
            .max(left + GRID_INSET)
    }
}

/// A horizontal rule at top-origin `top_y`, in mid-grey.
pub(crate) fn hline(content: &mut Content, x1: f32, x2: f32, top_y: f32, width: f32) {
    let y = PAGE_H - top_y;
    content.set_line_width(width);
    content.move_to(x1, y);
    content.line_to(x2, y);
    content.stroke();
}

/// A vertical rule between top-origin `top_y1` and `top_y2` (issue #104 — grid column separators).
pub(crate) fn vline(content: &mut Content, x: f32, top_y1: f32, top_y2: f32, width: f32) {
    content.set_line_width(width);
    content.move_to(x, PAGE_H - top_y1);
    content.line_to(x, PAGE_H - top_y2);
    content.stroke();
}

// ── vector-graphics primitives for the embedded charts (issue #105), all in top-origin coords ──

/// A polyline through top-origin `pts` in grey `gray`, weight `width`; `dash` (on, off) lengths make
/// it dashed (empty = solid). Fewer than two points draws nothing (a lone point has no line). The
/// stroke grey is restored to [`RULE_GRAY`] after, so later rules keep the default weight/tone.
fn polyline(content: &mut Content, pts: &[(f32, f32)], width: f32, gray: f32, dash: &[f32]) {
    if pts.len() < 2 {
        return;
    }
    content.set_stroke_gray(gray);
    content.set_line_width(width);
    if !dash.is_empty() {
        content.set_dash_pattern(dash.iter().copied(), 0.0);
    }
    for (i, (x, top_y)) in pts.iter().enumerate() {
        let y = PAGE_H - top_y;
        if i == 0 {
            content.move_to(*x, y);
        } else {
            content.line_to(*x, y);
        }
    }
    content.stroke();
    if !dash.is_empty() {
        content.set_dash_pattern(std::iter::empty(), 0.0);
    }
    content.set_stroke_gray(RULE_GRAY);
}

/// G1 final (L9) — the §4 marker of a price outside the zoning: a black arrow leaving the bar
/// through its `edge_x` at top-origin `y`, pointing outward (`dir` −1 = left, +1 = right), drawn
/// in the margin so it never reads as a position ON the bar.
fn edge_arrow(content: &mut Content, edge_x: f32, y: f32, dir: f32) {
    let tip = edge_x + dir * 14.0;
    polyline(content, &[(edge_x + dir * 2.0, y), (tip, y)], 1.3, 0.0, &[]);
    polyline(
        content,
        &[
            (tip - dir * 4.0, y - 3.5),
            (tip, y),
            (tip - dir * 4.0, y + 3.5),
        ],
        1.3,
        0.0,
        &[],
    );
}

/// A stroked rectangle outline at top-origin `(x, top_y)`, size `w × h`.
pub(crate) fn stroke_rect(content: &mut Content, x: f32, top_y: f32, w: f32, h: f32, width: f32) {
    content.set_line_width(width);
    content.rect(x, PAGE_H - top_y - h, w, h);
    content.stroke();
}

/// A grey-filled rectangle at top-origin `(x, top_y)`, size `w × h`. Restores the fill to black
/// (text) after — the greyscale zone fills are the only non-black fill in the document.
pub(crate) fn fill_rect(content: &mut Content, x: f32, top_y: f32, w: f32, h: f32, gray: f32) {
    content.set_fill_gray(gray);
    content.rect(x, PAGE_H - top_y - h, w, h);
    content.fill_nonzero();
    content.set_fill_gray(0.0);
}

/// [`text`] CENTERED on `cx` (the real Helvetica widths, [`text_width`]).
pub(crate) fn text_centered(content: &mut Content, cx: f32, top_y: f32, size: f32, s: &str) {
    text(content, cx - text_width(s, size) / 2.0, top_y, size, s);
}

/// Issue #25 (multi-scale): the log10 bounds of ONE series' own data range, padded by
/// [`SERIES_PAD_DECADES`] and widened to at least [`MIN_SERIES_DECADES`] so a flat series is not
/// stretched to fill. Each series gets its own bounds so none is crushed by another's magnitude
/// (AAPL: sales in hundreds of billions would otherwise flatten EPS/Price to invisible lines).
/// `None` when the series has no positive value.
fn series_log_bounds(values: &[f64]) -> Option<(f64, f64)> {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for v in values {
        if v.is_finite() && *v > 0.0 {
            lo = lo.min(v.log10());
            hi = hi.max(v.log10());
        }
    }
    if !(lo.is_finite() && hi.is_finite()) {
        return None;
    }
    if hi - lo < MIN_SERIES_DECADES {
        let mid = (lo + hi) / 2.0;
        lo = mid - MIN_SERIES_DECADES / 2.0;
        hi = mid + MIN_SERIES_DECADES / 2.0;
    }
    let pad = (hi - lo) * SERIES_PAD_DECADES;
    Some((lo - pad, hi + pad))
}

/// G1 F — the x of `year` on the §1 plot: the domain starts at `origin` (the first year less
/// [`YEAR_PAD`]) and spans `span` years across `plot_w` from `x0`. By year, never by index, so a
/// gap year keeps its place.
fn year_x(year: f64, origin: f64, span: f64, x0: f32, plot_w: f32) -> f32 {
    x0 + (((year - origin) / span.max(1.0)) * f64::from(plot_w)) as f32
}

/// G1 final (M2 / L5) — a series' drawable runs: consecutive years (by YEAR, `year + 1`) whose
/// value is plottable on a log scale (finite, positive). A missing year, an absent value or a value
/// the log scale cannot hold (zero, negative) BREAKS the line — never bridged, which would draw a
/// path through a year the data does not describe. A run of one point is kept (drawn as a dot).
fn plot_runs(points: &[(i32, Option<f64>)]) -> Vec<Vec<(i32, f64)>> {
    let mut sorted: Vec<(i32, Option<f64>)> = points.to_vec();
    sorted.sort_by_key(|p| p.0);
    let mut runs: Vec<Vec<(i32, f64)>> = Vec::new();
    let mut run: Vec<(i32, f64)> = Vec::new();
    for (year, value) in sorted {
        let plottable = value.filter(|v| v.is_finite() && *v > 0.0);
        let follows = run.last().is_some_and(|p| p.0 + 1 == year);
        match plottable {
            Some(v) if follows || run.is_empty() => run.push((year, v)),
            Some(v) => {
                runs.push(std::mem::take(&mut run));
                run.push((year, v));
            }
            None if !run.is_empty() => runs.push(std::mem::take(&mut run)),
            None => {}
        }
    }
    if !run.is_empty() {
        runs.push(run);
    }
    runs
}

/// G1 F — where a growth guide ends: `rate_pct` compounded from `(anchor_year, anchor_eps)` over
/// exactly the forecast horizon, so the drawn slope is the labelled rate.
fn guide_end(anchor_year: i32, anchor_eps: f64, rate_pct: u32) -> (i32, f64) {
    let horizon = FORECAST_HORIZON_YEARS as i32;
    (
        anchor_year + horizon,
        anchor_eps * (1.0 + f64::from(rate_pct) / 100.0).powi(horizon),
    )
}

/// G1 F review — the §1 scale note, stating where the guides and the projection really start:
/// the guides from the last positive EPS (`anchor_year`, possibly years before the latest year),
/// the projection from the latest usable year (`base_year`) — named when it differs from the
/// guides' start, or said not drawn when that year's EPS is not positive (`projection_drawn`
/// false) or there is no usable year. Nothing about the projection when there is no estimate.
fn chart_scale_note(
    anchor_year: Option<i32>,
    base_year: Option<i32>,
    projection_drawn: bool,
    has_estimate: bool,
) -> String {
    let mut parts = vec![CHART_SCALE.to_string()];
    parts.push(match anchor_year {
        Some(a) => {
            format!("{GUIDES_FROM} {a} {GUIDES_SPAN} {FORECAST_HORIZON_YEARS} {YEARS_OF_FORECAST}")
        }
        None => NO_POSITIVE_EPS.to_string(),
    });
    if has_estimate {
        match (base_year, projection_drawn) {
            (Some(b), true) if Some(b) != anchor_year => {
                parts.push(format!("{PROJECTION_FROM} {b}"));
            }
            (Some(_), true) => {}
            (Some(b), false) => parts.push(format!("{PROJECTION_NONE} {b} {NOT_POSITIVE}")),
            (None, _) => parts.push(PROJECTION_NO_BASE.to_string()),
        }
    }
    format!("{}.", parts.join(" ; "))
}

/// Nice `1 / 2 / 5 × 10^k` tick values (+ their compact labels) inside a log scale `[10^lmin, 10^lmax]`.
fn nice_ticks(lmin: f64, lmax: f64, nf: NumberStyle) -> Vec<(f64, String)> {
    let (min, max) = (10f64.powf(lmin), 10f64.powf(lmax));
    let mut out = Vec::new();
    for k in (min.log10().floor() as i32)..=(max.log10().ceil() as i32) {
        for m in [1.0, 2.0, 5.0] {
            let v = m * 10f64.powi(k);
            if v >= min && v <= max {
                out.push((v, compact_num(v, k, nf)));
            }
        }
    }
    out
}

/// A compact axis label: plain up to 999, then `k / M / Md` (French short scale) — data, not prose.
/// A tick below 1 (`m × 10^k`, `k < 0`) keeps its `-k` decimals, spelled in the reader's number
/// format (G1 I review: « 0,1 », never « 0 »).
fn compact_num(v: f64, k: i32, nf: NumberStyle) -> String {
    if k < 0 {
        let places = k.unsigned_abs() as usize;
        return Decimal::from_str_exact(&format!("{v:.places$}"))
            .map(|d| nf.spell(d))
            .unwrap_or_else(|_| format!("{v}"));
    }
    if v >= 1e9 {
        format!("{} Md", (v / 1e9).round() as i64)
    } else if v >= 1e6 {
        format!("{} M", (v / 1e6).round() as i64)
    } else if v >= 1e3 {
        format!("{} k", (v / 1e3).round() as i64)
    } else {
        format!("{}", v.round() as i64)
    }
}

/// Encode a UTF-8 string as WinAnsi (Latin-1 for 0xA0–0xFF, plus WinAnsi's own 0x80–0x9F range).
/// Characters outside the encoding fall back to '?', never panic.
fn winansi(s: &str) -> Vec<u8> {
    s.chars().map(winansi_byte).collect()
}

/// One character's WinAnsi byte. G1 F: the 0x80–0x9F range is WinAnsi's own (œ Œ “ ” ‘ ’ • ‰ ™
/// Š š Ž ž Ÿ … — Latin-1 has C1 controls there), so each is mapped by name rather than printed as
/// '?'; the Latin-1 letters (Æ æ Ø ø ß © ® …) are identity-mapped. [`glyph_width`] measures the
/// byte written here, so what is measured is what is printed.
fn winansi_byte(c: char) -> u8 {
    match c as u32 {
        n if n <= 0x7F => n as u8, // ASCII
        0x20AC => 0x80,            // € euro
        0x201A => 0x82,            // ‚ single low-9 quote
        0x0192 => 0x83,            // ƒ florin
        0x201E => 0x84,            // „ double low-9 quote
        0x2026 => 0x85,            // … horizontal ellipsis (issue #74 truncation)
        0x2020 => 0x86,            // † dagger
        0x2021 => 0x87,            // ‡ double dagger
        0x02C6 => 0x88,            // ˆ modifier circumflex
        0x2030 => 0x89,            // ‰ per mille
        0x0160 => 0x8A,            // Š
        0x2039 => 0x8B,            // ‹ single left guillemet
        0x0152 => 0x8C,            // Œ
        0x017D => 0x8E,            // Ž
        0x2018 => 0x91,            // ‘ left single quote
        0x2019 => 0x92,            // ’ right single quote
        0x201C => 0x93,            // “ left double quote
        0x201D => 0x94,            // ” right double quote
        0x2022 => 0x95,            // • bullet
        0x2013 => 0x96,            // – en dash
        0x2014 => 0x97,            // — em dash
        0x02DC => 0x98,            // ˜ small tilde
        0x2122 => 0x99,            // ™ trade mark
        0x0161 => 0x9A,            // š
        0x203A => 0x9B,            // › single right guillemet
        0x0153 => 0x9C,            // œ
        0x017E => 0x9E,            // ž
        0x0178 => 0x9F,            // Ÿ
        0x2212 => 0x2D,            // − minus sign → hyphen-minus (formulas)
        0x202F => 0xA0,            // narrow no-break space → no-break space (pasted figures)
        // Latin-1 high range == WinAnsi (é è à ç ° Æ ø ß © ® …). The C1 controls 0x80–0x9F are
        // NOT identity-mapped in WinAnsi, so they fall through to '?' rather than mis-render.
        n if (0xA0..=0xFF).contains(&n) => n as u8,
        _ => b'?',
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::{
        Cell, Coverage, ForecastLowOption, Freshness, Judgment, Money, Provenance, Review, Source,
        Study, Timestamp, YearData,
    };
    use uuid::Uuid;

    fn money_of(s: &str) -> Money {
        Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
    }

    fn cell(value: &str) -> Cell {
        Cell {
            value: Some(money_of(value)),
            source: Source::Manual,
            freshness: Freshness::Current,
            review: Review::Validated,
            coverage: Coverage::Present,
            provenance: Provenance {
                source: Source::Manual,
                logical_version: 1,
                timestamp: Timestamp("2026-03-09T00:00:00Z".to_string()),
                hash_of_dependencies: "manual".to_string(),
            },
            pending: None,
        }
    }

    fn year(y: i32, eps: &str) -> YearData {
        YearData {
            year: y,
            sales: cell("1000"),
            eps: cell(eps),
            high_price: cell("100"),
            low_price: cell("50"),
            dividend_per_share: Some(cell("2")),
            pre_tax_profit: Some(cell("200")),
            book_value_per_share: Some(cell("40")),
        }
    }

    fn demo_study() -> Study {
        let judgment = Judgment {
            estimated_high_eps: Some(money_of("9")),
            estimated_low_eps: Some(money_of("4")),
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: Some(money_of("18")),
            judged_avg_low_pe: Some(money_of("10")),
            forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
            recent_severe_low: None,
            current_price: Some(money_of("80")),
            present_full_year_dividend: Some(money_of("2")),
            ttm_eps: None,
        };
        let mut s = Study::new(
            Uuid::from_u128(0x5_6),
            Uuid::from_u128(0x1),
            "NESN",
            "CHF",
            judgment,
            Timestamp("2026-03-09T09:30:00Z".to_string()),
        );
        s.years = (2021..=2025).map(|y| year(y, "5")).collect();
        s
    }

    #[test]
    fn renders_a_nonempty_well_formed_pdf() {
        let bytes = render_study_pdf(&demo_study(), NumberStyle::Point)
            .expect("a normalizing study renders");
        assert!(
            bytes.starts_with(b"%PDF-"),
            "a PDF starts with the %PDF- header"
        );
        assert!(
            bytes.windows(5).any(|w| w == b"%%EOF"),
            "a PDF ends with the %%EOF trailer marker"
        );
        assert!(
            bytes.len() > 800,
            "the form has real content, got {}",
            bytes.len()
        );
    }

    /// The 7.5 walk: « NESN.SW (CHF) » measured at half an em per glyph came out ~20 % short, so
    /// the right-aligned head started too far left and ran over the rule. The widths are the AFM's.
    #[test]
    fn text_width_uses_the_real_helvetica_metrics() {
        // N722 E667 S667 N722 .278 S667 W944 space278 (333 C722 H722 F611 )333 = 7666
        assert!((text_width("NESN.SW (CHF)", 10.0) - 76.66).abs() < 1e-3);
        assert!(text_width("NESN.SW (CHF)", 10.0) > "NESN.SW (CHF)".len() as f32 * 5.0);
        // Accented letters take their base width; « i » is narrow, « î » too.
        assert_eq!(text_width("é", 10.0), text_width("e", 10.0));
        assert_eq!(text_width("î", 10.0), 2.78);
    }

    /// The 7.5 walk: a reader's long note ran off the page — a prose line now wraps at the margin.
    #[test]
    fn a_prose_line_longer_than_the_page_wraps() {
        let mut doc = Doc::new();
        let start = doc.y;
        doc.line("court");
        let one = doc.y - start;
        let long = "Le haut de 2021 tient à un exercice exceptionnel ; la moyenne ajustée serait plutôt autour de 20, à revoir après les résultats annuels 2026, et encore une fois après.";
        assert!(text_width(long, FONT) > doc.right() - MARGIN);
        let before = doc.y;
        doc.indent_line(long);
        assert!(
            doc.y - before >= 2.0 * one,
            "the note took more than one line"
        );
    }

    #[test]
    fn fit_and_wrap_never_exceed_the_width() {
        assert_eq!(fit("Nestlé", 100.0, 9.0), "Nestlé");
        let cut = fit("NVIDIA Corporation", 40.0, 9.0);
        assert!(cut.ends_with('…'));
        assert!(text_width(&cut, 9.0) <= 40.0);
        let note = "non classé : chiffre d'affaires indisponible";
        let lines = wrap_to_width(note, 120.0, 9.0);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| text_width(l, 9.0) <= 120.0));
        assert_eq!(lines.join(" "), note, "wrapping keeps every word");
        assert_eq!(wrap_to_width("", 50.0, 9.0), vec![String::new()]);
    }

    #[test]
    fn output_is_deterministic_same_study_same_bytes() {
        // No timestamp / file-id / randomness — a fixture renders byte-identically (testable, and
        // friendly to content-hash dedup).
        let a = render_study_pdf(&demo_study(), NumberStyle::Point).unwrap();
        let b = render_study_pdf(&demo_study(), NumberStyle::Point).unwrap();
        assert_eq!(a, b, "the same study must render identical bytes");
    }

    #[test]
    fn a_degenerate_study_renders_neutrally_without_panicking() {
        // A study with no years still normalizes (no usable data → unknown figures); the renderer must
        // produce a calm PDF with em-dashes, never panic. (Genuine normalize failures take the
        // `ReportError::Normalize` path — exercised by `core`'s own normalize tests.)
        let mut s = demo_study();
        s.years.clear();
        let bytes =
            render_study_pdf(&s, NumberStyle::Point).expect("a degenerate study still renders");
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(
            bytes.windows(5).any(|w| w == b"%%EOF"),
            "still a well-formed PDF"
        );
    }

    #[test]
    fn unknown_figures_format_as_the_em_dash_never_zero() {
        // The project's most-repeated rail, at the formatter level (the PDF hex-encodes the glyph).
        let nf = NumberStyle::Comma;
        assert_eq!(nf.money(None), EM_DASH);
        assert_eq!(nf.num(None), EM_DASH);
        assert_eq!(nf.pct(None), EM_DASH);
        assert_eq!(nf.cell(None, DisplayField::LargeMonetary), EM_DASH);
        assert_eq!(trend(None), EM_DASH);
        // A present value formats as its rounded decimal (no spurious zero-padding).
        assert_eq!(
            nf.money(Some(rust_decimal::Decimal::from_str_exact("80").unwrap())),
            "80"
        );
    }

    // ── G1 I (#237): the study PDF speaks the reader's number format ──

    #[test]
    fn a_number_is_spelled_in_the_readers_format() {
        let d = |s: &str| rust_decimal::Decimal::from_str_exact(s).unwrap();
        assert_eq!(NumberStyle::Comma.spell(d("128.9")), "128,9");
        assert_eq!(NumberStyle::Point.spell(d("128.9")), "128.9");
        assert_eq!(
            NumberStyle::Comma.spell(d("-1234567.5")),
            "-1\u{00A0}234\u{00A0}567,5"
        );
        assert_eq!(NumberStyle::Point.spell(d("-1234567.5")), "-1,234,567.5");
        assert_eq!(NumberStyle::Point.spell(d("123456")), "123,456");
        assert_eq!(NumberStyle::Comma.spell(d("999")), "999");
        assert_eq!(NumberStyle::Comma.pct(Some(d("15.84"))), "15,8 %");
        assert_eq!(NumberStyle::Point.pct(Some(d("15.84"))), "15.8 %");
        assert_eq!(
            upside(&UpsideDownside::Ratio(d("2.71")), NumberStyle::Comma),
            "2,7 : 1"
        );
        assert_eq!(NumberStyle::default(), NumberStyle::Comma);
    }

    #[test]
    fn an_axis_tick_below_one_keeps_its_decimals_in_the_readers_format() {
        let labels = |nf| {
            nice_ticks(-2.0, 0.5, nf)
                .into_iter()
                .map(|(_, l)| l)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(NumberStyle::Comma),
            ["0,01", "0,02", "0,05", "0,1", "0,2", "0,5", "1", "2"]
        );
        assert_eq!(
            labels(NumberStyle::Point),
            ["0.01", "0.02", "0.05", "0.1", "0.2", "0.5", "1", "2"]
        );
        assert_eq!(compact_num(5000.0, 3, NumberStyle::Comma), "5 k");
    }

    #[test]
    fn the_study_pdf_follows_the_number_format_and_stays_deterministic() {
        let mut s = demo_study();
        s.judgment.current_price = Some(money_of("77.94"));
        let comma = render_study_pdf(&s, NumberStyle::Comma).unwrap();
        let point = render_study_pdf(&s, NumberStyle::Point).unwrap();
        assert!(contains(&comma, "Cours actuel : 77,94"));
        assert!(!contains(&comma, "77.94"));
        assert!(contains(&point, "Cours actuel : 77.94"));
        assert!(!contains(&point, "77,94"));
        assert_ne!(comma, point);
        assert_eq!(comma, render_study_pdf(&s, NumberStyle::Comma).unwrap());
    }

    /// A token-wise neutrality check mirroring the app posture gate: split on non-alphanumerics and
    /// assert no token equals a banned verb (case-insensitive) — substrings never false-positive.
    fn assert_neutral(s: &str) {
        use steadyinvest_core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR};
        let lower = s.to_lowercase();
        for token in lower.split(|c: char| !c.is_alphanumeric()) {
            for banned in BANNED_VERBS_EN.iter().chain(BANNED_VERBS_FR.iter()) {
                assert_ne!(
                    token,
                    banned.to_lowercase(),
                    "report string {s:?} contains banned verb {banned:?}"
                );
            }
        }
    }

    #[test]
    fn report_strings_are_neutral_no_banned_verb() {
        // `report` is outside the app posture gate, so it guards its own neutrality (FR13) against the
        // SAME shared `core::method` catalogs. Covers the static inventory + the dynamic label fns
        // exhaustively over their enum variants.
        for s in REPORT_USER_FACING {
            assert_neutral(s);
        }
        for p in [
            PricePlace::In(Zone::Buy),
            PricePlace::In(Zone::Neutral),
            PricePlace::In(Zone::Sell),
            PricePlace::Below,
            PricePlace::Above,
            PricePlace::Absent,
        ] {
            assert_neutral(price_position(p));
        }
        for t in [Some(Trend::Up), Some(Trend::Even), Some(Trend::Down), None] {
            assert_neutral(trend(t));
        }
        for u in [
            UpsideDownside::Ratio(rust_decimal::Decimal::ONE),
            UpsideDownside::Undefined,
            UpsideDownside::Unknown,
        ] {
            assert_neutral(&upside(&u, NumberStyle::Comma));
        }
    }

    #[test]
    fn no_naic_wordmark_in_any_source_label() {
        // Stronger than a raw-byte scan of the (hex-encoded) PDF: assert the SOURCE strings carry no
        // NAIC mark / verbatim prose (open-source constraint).
        for s in REPORT_USER_FACING {
            for mark in ["NAIC", "Stock Selection Guide", "Better Investing", "SSG"] {
                assert!(
                    !s.contains(mark),
                    "report string {s:?} carries the NAIC mark {mark:?}"
                );
            }
        }
    }

    #[test]
    fn a_many_year_study_paginates_to_multiple_pages() {
        // Exercise the paginator's per-page object graph with n > 1 pages (the single-page fixtures
        // never hit the `4+2i / 5+2i` ref allocation across pages).
        let mut s = demo_study();
        s.years = (1970..=2025).map(|y| year(y, "5")).collect(); // 56 years → overflows one A4 page
        let bytes = render_study_pdf(&s, NumberStyle::Point).expect("a long study renders");
        assert!(bytes.starts_with(b"%PDF-") && bytes.windows(5).any(|w| w == b"%%EOF"));
        // The page tree's /Count must exceed 1 and the xref must stay well-formed.
        let count_pos = bytes
            .windows(7)
            .position(|w| w == b"/Count ")
            .expect("a page tree with a /Count");
        // Read every digit (a ≥10-page study must not be misread as its first digit).
        let n: u32 = bytes[count_pos + 7..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .fold(0, |acc, b| acc * 10 + u32::from(b - b'0'));
        assert!(n >= 2, "56 years must paginate to >1 page, got /Count {n}");
    }

    #[test]
    fn a_table_spanning_a_page_break_repeats_its_column_header() {
        // Issue #74: a §1 historical table long enough to cross a page break must re-emit its column
        // header on each continuation page (fidelity — a headerless continuation is confusing).
        let mut s = demo_study();
        s.years = (1900..=2025).map(|y| year(y, "5")).collect(); // 126 years → §1 spans several pages
        let bytes = render_study_pdf(&s, NumberStyle::Point).expect("a long study renders");
        // "Cours haut" is a §1-only header cell (WinAnsi = ASCII, so a contiguous byte run). One
        // occurrence per page the table touches → ≥ 2 proves the header was replayed.
        let header = b"Cours haut";
        let count = bytes.windows(header.len()).filter(|w| *w == header).count();
        assert!(
            count >= 2,
            "the §1 column header must repeat on continuation pages, saw {count}"
        );
    }

    // ── G1 F (#237) — the PDF engine and the study PDF of #216 ──

    /// The two ways the content stream can carry `s`: `pdf-writer` writes a string with a byte
    /// outside printable ASCII as hex (`<…>`), else as a literal with `( ) \` escaped — and a
    /// needle may sit inside a longer string written either way.
    fn encodings(s: &str) -> [Vec<u8>; 2] {
        let raw = winansi(s);
        let mut literal = Vec::new();
        for b in &raw {
            if matches!(b, b'(' | b')' | b'\\') {
                literal.push(b'\\');
            }
            literal.push(*b);
        }
        let hex = raw
            .iter()
            .flat_map(|b| format!("{b:02X}").into_bytes())
            .collect();
        [literal, hex]
    }

    fn contains(hay: &[u8], s: &str) -> bool {
        occurrences(hay, s) > 0
    }

    fn occurrences(hay: &[u8], s: &str) -> usize {
        encodings(s)
            .iter()
            .map(|needle| {
                hay.windows(needle.len())
                    .filter(|w| *w == needle.as_slice())
                    .count()
            })
            .sum()
    }

    /// The content streams, one per page, in page order (the only streams in the file).
    fn page_streams(bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut rest = bytes;
        while let Some(end) = rest.windows(9).position(|w| w == b"endstream") {
            out.push(rest[..end].to_vec());
            rest = &rest[end + 9..];
        }
        out
    }

    #[test]
    fn the_header_box_fits_each_value_to_its_column_by_width() {
        // G1 F: 48 characters of « W » are ~ 380 pt — the old character cap let a wide name run
        // over the next cell. The value written is the one fitted at the real glyph widths.
        let mut s = demo_study();
        let name = "W".repeat(48);
        s.company_name = Some(name.clone());
        let bytes = render_study_pdf(&s, NumberStyle::Point).unwrap();
        let room = (PAGE_W - 2.0 * MARGIN) / 3.0 - 2.0 * CELL_PAD;
        let fitted = fit(&name, room, FONT);
        assert!(fitted.ends_with('…') && text_width(&fitted, FONT) <= room);
        assert!(
            contains(&bytes, &fitted),
            "the fitted name is what is written"
        );
        assert!(!contains(&bytes, &name), "never the whole over-wide name");
        // A name that fits is written whole, no spurious ellipsis.
        let bytes = render_study_pdf(&demo_study(), NumberStyle::Point).unwrap();
        assert!(contains(&bytes, "NESN"));
    }

    #[test]
    fn a_grid_figure_is_never_cut_it_shrinks_whole_to_its_padded_room() {
        // The annexe sales of a JPY issuer: 14 digits in a 70 pt column at 9 pt.
        let col = 70.0;
        let (lines, size) = cell_layout("31234567890123", col, FONT);
        assert_eq!(lines, vec!["31234567890123".to_string()], "one line, whole");
        assert!((MIN_FIGURE_FONT..FONT).contains(&size));
        // Shrunk to the PADDED width: it ends on the column's normal right edge, like its
        // unshrunk neighbours (the units stay aligned).
        let w = text_width(&lines[0], size);
        assert!(w <= col - 2.0 * CELL_PAD);
        assert_eq!(cell_x(true, 0.0, col, w) + w, col - CELL_PAD);
        let short = text_width("180", FONT);
        assert_eq!(cell_x(true, 0.0, col, short) + short, col - CELL_PAD);
        // A figure with its unit shrinks whole, the unit on the number's line.
        let (lines, _) = cell_layout("1234,5 %", 25.0, SMALL);
        assert_eq!(lines, vec!["1234,5 %".to_string()]);
        // Past the smallest type the figure is printed whole at that size, on ONE line — never
        // split into pieces reading as two numbers — keeping its right edge (across the left rule).
        let long = "12345678901234567890";
        let (lines, size) = cell_layout(long, 20.0, FONT);
        assert_eq!(size, MIN_FIGURE_FONT);
        assert_eq!(lines, vec![long.to_string()]);
        let w = text_width(long, size);
        assert!(w > 20.0);
        assert_eq!(cell_x(true, 0.0, 20.0, w) + w, 20.0 - CELL_PAD);
        // A figure among words is never split either.
        let (lines, _) = cell_layout("soit 12345678901234567890 au total", 20.0, FONT);
        assert!(lines.contains(&long.to_string()), "{lines:?}");
        // A label without a digit may still end in « … ».
        let (lines, _) = cell_layout("Supercalifragilistique", 40.0, FONT);
        assert!(lines[0].ends_with('…'));
        // End to end: the study annexe prints the 14-digit figure whole, grouped in the reader's
        // format (G1 I) — the no-break space never breaks the figure.
        let mut s = demo_study();
        s.years[0].sales = cell("31234567890123");
        let bytes = render_study_pdf(&s, NumberStyle::Point).unwrap();
        assert!(contains(&bytes, "31,234,567,890,123"));
        let bytes = render_study_pdf(&s, NumberStyle::Comma).unwrap();
        assert!(contains(
            &bytes,
            "31\u{00A0}234\u{00A0}567\u{00A0}890\u{00A0}123"
        ));
    }

    #[test]
    fn a_degenerate_column_never_erases_a_cells_text_or_absence() {
        // No room at all: the text as is — the absence mark « — » above all.
        assert_eq!(cell_layout(EM_DASH, 1.0, FONT).0, vec![EM_DASH.to_string()]);
        assert_eq!(
            cell_layout(EM_DASH, -4.0, FONT).0,
            vec![EM_DASH.to_string()]
        );
        // The padding leaves no room (8 pt column): the room between the rules is used, and
        // neither a figure nor a word comes out empty.
        let (lines, _) = cell_layout("12 abc", 8.0, FONT);
        assert!(lines.iter().all(|l| !l.is_empty()), "{lines:?}");
        assert!(lines.contains(&"12".to_string()));
        // A word narrower than nothing but « … » is kept rather than blanked.
        let (lines, _) = cell_layout("en hausse", 12.0, SMALL);
        assert!(lines.iter().all(|l| !l.is_empty()), "{lines:?}");
        let (lines, _) = cell_layout("— · —", 6.0, FONT);
        assert!(lines.iter().all(|l| !l.is_empty()), "{lines:?}");
    }

    #[test]
    fn fit_and_wrap_hold_zero_widths_and_space_runs() {
        assert_eq!(fit("abc", 0.0, FONT), "");
        assert_eq!(fit("abc", -5.0, FONT), "");
        assert_eq!(fit("abcdef", 3.0, FONT), "", "narrower than « … » itself");
        assert_eq!(wrap_to_width("a b", 0.0, FONT), vec![String::new()]);
        assert_eq!(wrap_to_width("a b", -1.0, FONT), vec![String::new()]);
        // A run of spaces inside a line is kept …
        let sep = "A = x   ·   B = y";
        assert_eq!(wrap_to_width(sep, 1000.0, FONT), vec![sep.to_string()]);
        // … and dropped at a break: no line starts or ends with spaces, none is too wide.
        let long = "alpha   ·   beta   ·   gamma   ·   delta   ·   epsilon";
        let lines = wrap_to_width(long, 60.0, FONT);
        assert!(lines.len() > 1);
        for l in &lines {
            assert!(!l.starts_with(' ') && !l.ends_with(' '), "{l:?}");
            assert!(text_width(l, FONT) <= 60.0, "{l:?}");
        }
        // Leading spaces never produce an empty first line.
        assert_eq!(wrap_to_width("  mot", 100.0, FONT), vec!["mot".to_string()]);
    }

    #[test]
    fn winansi_maps_its_own_upper_range_and_measures_latin1_letters() {
        let s = "œŒ“”‘’•‰™ŠšŽžŸ…€–—";
        let bytes = winansi(s);
        assert!(
            !bytes.contains(&b'?'),
            "every one has a WinAnsi code: {bytes:?}"
        );
        assert_eq!(
            winansi("œŒ“”•‰™"),
            vec![0x9C, 0x8C, 0x93, 0x94, 0x95, 0x89, 0x99]
        );
        assert_eq!(
            winansi("ÆæØøß©®"),
            vec![0xC6, 0xE6, 0xD8, 0xF8, 0xDF, 0xA9, 0xAE]
        );
        assert_eq!(winansi("→"), vec![b'?'], "outside the encoding: '?'");
        // The standard Helvetica AFM widths (1/1000 em at 1000 pt = the AFM unit).
        for (c, w) in [
            ("œ", 944.0),
            ("Œ", 1000.0),
            ("Æ", 1000.0),
            ("æ", 889.0),
            ("Ø", 778.0),
            ("ø", 611.0),
            ("ß", 611.0),
            ("©", 737.0),
            ("®", 737.0),
            ("•", 350.0),
            ("‰", 1000.0),
            ("™", 1000.0),
            ("“", 333.0),
            ("é", 556.0),
            ("?", 556.0),
            ("→", 556.0),
        ] {
            assert_eq!(text_width(c, 1000.0), w, "{c}");
        }
    }

    #[test]
    fn a_section_3_total_is_the_whole_sum_or_absent_never_partial() {
        let d = |s: &str| Entry::Known(rust_decimal::Decimal::from_str_exact(s).unwrap());
        assert_eq!(
            column_total(vec![d("1.5"), d("2.5")]),
            Total::Sum(rust_decimal::Decimal::from(4))
        );
        assert_eq!(
            column_total(vec![d("1"), Entry::Unknown, d("2")]),
            Total::Absent {
                unknown: true,
                undefined: false
            }
        );
        assert_eq!(
            column_total(vec![Entry::Undefined, Entry::Unknown]),
            Total::Absent {
                unknown: true,
                undefined: true
            }
        );
        assert_eq!(column_total(Vec::new()), Total::Empty);
        // An overflow is absent — never restarted from the next year's figure.
        let max = Entry::Known(rust_decimal::Decimal::MAX);
        assert_eq!(column_total(vec![max, max, d("1")]), Total::Overflow);
        // End to end: a window year without a dividend → G and H totals absent, reason named;
        // never the undefined-ratio reason, which does not apply.
        let mut s = demo_study();
        s.years[4].dividend_per_share = None;
        let bytes = render_study_pdf(&s, NumberStyle::Point).unwrap();
        assert!(contains(&bytes, TOTAL_UNKNOWN_YEAR));
        assert!(!contains(&bytes, TOTAL_UNDEFINED));
        assert!(
            !contains(&bytes, "160.0 %") && !contains(&bytes, "160 %"),
            "no partial G total (4 × 40 %)"
        );
        let bytes = render_study_pdf(&demo_study(), NumberStyle::Point).unwrap();
        assert!(
            !contains(&bytes, TOTAL_UNKNOWN_YEAR),
            "no note when all is known"
        );
        assert!(
            contains(&bytes, "200.0 %"),
            "G total over five known years: 5 × 40 %"
        );
    }

    #[test]
    fn a_total_absent_for_a_non_positive_eps_names_the_undefined_ratio() {
        // Every figure is entered; the EPS of one window year is negative, so its P/E and payout
        // are undefined — not « missing ». The note names that cause, not a missing figure.
        let mut s = demo_study();
        s.years[2].eps = cell("-1");
        let bytes = render_study_pdf(&s, NumberStyle::Point).unwrap();
        assert!(contains(&bytes, TOTAL_UNDEFINED));
        assert!(
            !contains(&bytes, TOTAL_UNKNOWN_YEAR),
            "misattribution: no figure is missing"
        );
    }

    #[test]
    fn the_scale_note_names_where_the_guides_and_the_projection_start() {
        let h = FORECAST_HORIZON_YEARS;
        // The usual case: both start from the latest year's EPS — one start, named once.
        let n = chart_scale_note(Some(2025), Some(2025), true, true);
        assert!(n.contains(&format!(
            "{GUIDES_FROM} 2025 {GUIDES_SPAN} {h} {YEARS_OF_FORECAST}"
        )));
        assert!(!n.contains(PROJECTION_FROM) && !n.contains(PROJECTION_NONE));
        // The latest EPS not positive: the guides start years earlier (named); the projection,
        // whose base is the latest usable year, is not drawn, and the note says why.
        let n = chart_scale_note(Some(2023), Some(2025), false, true);
        assert!(n.contains(&format!("{GUIDES_FROM} 2023")));
        assert!(n.contains(&format!("{PROJECTION_NONE} 2025 {NOT_POSITIVE}")));
        // A projection drawn from a start other than the guides' is named.
        let n = chart_scale_note(Some(2025), Some(2024), true, true);
        assert!(n.contains(&format!("{PROJECTION_FROM} 2024")));
        // No estimate: nothing said about a projection; no positive EPS: no guides.
        let n = chart_scale_note(None, Some(2025), false, false);
        assert!(n.contains(NO_POSITIVE_EPS) && !n.contains("projection"));
        // End to end: the last two EPS negative.
        let mut s = demo_study();
        s.years[3].eps = cell("-1");
        s.years[4].eps = cell("-2");
        let bytes = render_study_pdf(&s, NumberStyle::Point).unwrap();
        assert!(contains(&bytes, &format!("{GUIDES_FROM} 2023")));
        assert!(contains(
            &bytes,
            &format!("{PROJECTION_NONE} 2025 {NOT_POSITIVE}")
        ));
    }

    #[test]
    fn the_section_2_average_says_how_many_years_it_covers() {
        let bytes = render_study_pdf(&demo_study(), NumberStyle::Point).unwrap();
        assert!(contains(&bytes, AVG_FIVE));
        assert!(!contains(&bytes, AVG_FEWER_NOTE));
        let mut s = demo_study();
        s.years.truncate(3);
        let bytes = render_study_pdf(&s, NumberStyle::Point).unwrap();
        assert!(
            !contains(&bytes, AVG_FIVE),
            "never « Moy. 5 ans » over three years"
        );
        assert!(contains(&bytes, AVG_FEWER_NOTE));
        assert!(contains(&bytes, "A : 3 ans, B : 3 ans."));
        assert_eq!(years_count(0), NO_YEAR);
        assert_eq!(years_count(1), "1 an");
    }

    #[test]
    fn the_data_source_reads_every_cell_and_never_calls_derived_manual() {
        let mut s = demo_study();
        assert_eq!(data_source(&s).phrase(), MANUAL_ENTRY);
        let provider = |c: &mut Cell| {
            c.source = Source::Provider;
            c.provenance.hash_of_dependencies = "eodhd:abc".to_string();
        };
        for y in &mut s.years {
            provider(&mut y.sales);
            provider(&mut y.eps);
            provider(&mut y.high_price);
            provider(&mut y.low_price);
            for c in [
                &mut y.dividend_per_share,
                &mut y.pre_tax_profit,
                &mut y.book_value_per_share,
            ]
            .into_iter()
            .flatten()
            {
                provider(c);
            }
        }
        assert_eq!(data_source(&s).phrase(), "fournisseur eodhd");
        // One manual cell elsewhere than the sales is named too.
        s.years[0].eps.source = Source::Manual;
        assert_eq!(
            data_source(&s).phrase(),
            "fournisseur eodhd et saisie manuelle"
        );
        // Only computed cells: « calculé », never « saisie manuelle ».
        let mut d = demo_study();
        for y in &mut d.years {
            for c in [
                &mut y.sales,
                &mut y.eps,
                &mut y.high_price,
                &mut y.low_price,
            ] {
                c.source = Source::Derived;
            }
            y.dividend_per_share = None;
            y.pre_tax_profit = None;
            y.book_value_per_share = None;
        }
        assert_eq!(data_source(&d).phrase(), COMPUTED);
        d.years.clear();
        assert_eq!(data_source(&d).phrase(), EM_DASH);
    }

    #[test]
    fn section_4_states_the_four_low_price_candidates() {
        // demo: judged low P/E 10 × est. low EPS 4 = 40; the five lows are 50; no severe low
        // entered; dividend 2 ÷ average high yield 4 % (2 ÷ 50) = 50.
        for nf in [NumberStyle::Point, NumberStyle::Comma] {
            let bytes = render_study_pdf(&demo_study(), nf).unwrap();
            // G1 final (L6): the yield keeps its decimal place, as on the screen (« 4,0 % »).
            let yield_4 = nf.spell(rust_decimal::Decimal::new(40, 1));
            for line in [
                "(a) PER bas moyen 10 × BPA estimé bas 4 = 40".to_string(),
                "(b) Prix bas moyen des 5 dernières années = 50".to_string(),
                "(c) Plus bas sévère récent = —".to_string(),
                format!(
                    "(d) Prix soutenu par le dividende : dividende 2 ÷ rendement haut moyen {yield_4} % = 50"
                ),
                "Prix bas retenu (PER bas × BPA bas) = 40".to_string(),
            ] {
                assert!(contains(&bytes, &line), "missing under {nf:?}: {line}");
            }
        }
        // A fractional candidate is spelled per format (G1 I review).
        let mut s = demo_study();
        s.judgment.judged_avg_low_pe = Some(money_of("10.5"));
        let comma = render_study_pdf(&s, NumberStyle::Comma).unwrap();
        let point = render_study_pdf(&s, NumberStyle::Point).unwrap();
        assert!(contains(
            &comma,
            "(a) PER bas moyen 10,5 × BPA estimé bas 4 = 42"
        ));
        assert!(contains(
            &point,
            "(a) PER bas moyen 10.5 × BPA estimé bas 4 = 42"
        ));
    }

    #[test]
    fn a_block_reserve_counts_its_wrapped_lines() {
        let doc = Doc::new();
        let mut b = Block::default();
        b.line("court");
        assert_eq!(doc.block_height(&b), LINE_H);
        b.line(&"mot ".repeat(60));
        assert!(doc.block_height(&b) >= 3.0 * LINE_H, "the long line wraps");
        b.small_line("note");
        assert!(doc.block_height(&b) >= 3.0 * LINE_H + LINE_H - 2.0);
    }

    #[test]
    fn the_study_is_the_forms_two_pages_then_the_annexe() {
        let bytes = render_study_pdf(&demo_study(), NumberStyle::Point).unwrap();
        let pages = page_streams(&bytes);
        assert_eq!(pages.len(), 3, "page 1, page 2, annexe");
        assert!(contains(
            &pages[0],
            "1. Analyse visuelle des ventes, bénéfices et cours"
        ));
        assert!(contains(&pages[0], QUARTER_BOX_TITLE));
        assert!(contains(&pages[0], "(4) Croissance estimée du BPA"));
        assert!(contains(&pages[1], "2. Évaluation de la gestion"));
        assert!(contains(&pages[1], "5. Potentiel à 5 ans"));
        assert!(contains(&pages[2], "Annexe — données historiques"));
    }

    #[test]
    fn the_guides_run_over_the_horizon_from_their_anchor_year() {
        let h = FORECAST_HORIZON_YEARS as i32;
        let (year, v) = guide_end(2023, 2.0, 10);
        assert_eq!(
            year,
            2023 + h,
            "ends a horizon after the LAST POSITIVE EPS year"
        );
        assert!((v - 2.0 * 1.1f64.powi(h)).abs() < 1e-9);
        // The drawn slope per year is the labelled rate.
        let per_year = (v / 2.0).log10() / f64::from(h);
        assert!((per_year - 1.1f64.log10()).abs() < 1e-12);
        // The x axis is by year: a gap year keeps its place.
        let x = |y: i32| year_x(f64::from(y), 2015.0, 15.0, 100.0, 300.0);
        assert!(((x(2019) - x(2017)) - 2.0 * (x(2018) - x(2017))).abs() < 1e-3);
        assert_eq!(x(2015), 100.0);
        assert_eq!(x(2030), 400.0);
        // Headroom and bounds ignore a non-finite value.
        assert_eq!(
            series_log_bounds(&[f64::INFINITY, f64::NAN]),
            None,
            "no bounds from non-finite values"
        );
        assert!(series_log_bounds(&[1.0, 10.0, f64::INFINITY]).is_some());
    }

    #[test]
    fn the_chart_fills_the_page_it_lands_on() {
        // A break before the plot measures its height on the NEW page, not the old remainder.
        let frame = crate::form::build_frame(&demo_study()).unwrap();
        let mut doc = Doc::new();
        doc.y = PAGE_H - BOTTOM - 120.0;
        doc.growth_chart(&frame, NumberStyle::Comma);
        assert_eq!(doc.page_index(), 1, "the plot moved to a new page");
        // What is left under it is the caller's gap + the four growth lines.
        assert!(
            doc.y >= PAGE_H - BOTTOM - 2.0 - 2.0 * LINE_H - 1.0,
            "the plot fills the new page, y = {}",
            doc.y
        );
    }

    #[test]
    fn the_quarterly_box_is_drawn_below_the_plot() {
        // Owner decision 7: the box leaves the plot — no opaque white fill is painted on page 1
        // (the zone bar's greys are on page 2; `1 g` is the white fill the box used to paint).
        let bytes = render_study_pdf(&demo_study(), NumberStyle::Point).unwrap();
        let pages = page_streams(&bytes);
        assert!(
            !contains(&pages[0], "\n1 g\n"),
            "no white box over the plot"
        );
        assert!(contains(&pages[0], QUARTER_BOX_TITLE));
    }

    #[test]
    fn every_header_row_repeats_after_a_break_and_none_is_orphaned() {
        let edges = [MARGIN, MARGIN + 100.0, PAGE_W - MARGIN];
        let mut doc = Doc::new();
        doc.grid_begin(0);
        doc.grid_row_small(&["", "HEADONE"], &edges, true, 1);
        doc.grid_row_small(&["", "HEADTWO"], &edges, true, 1);
        for i in 0..120 {
            doc.grid_row_small(&[&format!("r{i}"), "1"], &edges, false, 1);
        }
        // A head row after the body (the quick screen's totals) is an underlined row, not a
        // header: it never replaces the header rows on a continuation page.
        doc.grid_row_small(&["TOTALROW", "9"], &edges, true, 1);
        for i in 0..60 {
            doc.grid_row_small(&[&format!("s{i}"), "1"], &edges, false, 1);
        }
        doc.grid_end(&edges);
        let pages = doc.page_index() + 1;
        let bytes = doc.finish();
        assert!(pages >= 3);
        assert_eq!(occurrences(&bytes, "HEADONE"), pages);
        assert_eq!(occurrences(&bytes, "HEADTWO"), pages);
        assert_eq!(occurrences(&bytes, "TOTALROW"), 1);

        // Room for the two header rows but not for them + the first body row: all move on.
        let mut doc = Doc::new();
        doc.grid_begin(0);
        doc.y = PAGE_H - BOTTOM - 2.0 * LINE_H - 1.0;
        doc.grid_row_small(&["", "HEADONE"], &edges, true, 1);
        doc.grid_row_small(&["", "HEADTWO"], &edges, true, 1);
        doc.grid_row_small(&["BODYONE", "1"], &edges, false, 1);
        doc.grid_end(&edges);
        assert_eq!(doc.page_index(), 1);
        let pages = page_streams(&doc.finish());
        assert!(!contains(&pages[0], "HEADONE") && !contains(&pages[0], "HEADTWO"));
        assert!(contains(&pages[1], "HEADONE") && contains(&pages[1], "BODYONE"));
    }

    #[test]
    fn a_row_and_its_note_are_never_split_by_a_page_break() {
        let edges = [MARGIN, MARGIN + 80.0, PAGE_W - MARGIN];
        let mut doc = Doc::new();
        doc.grid_begin(0);
        doc.grid_row_num(&["HEAD", "X"], &edges, true, 1);
        doc.grid_row_num(&["FIRST", "1"], &edges, false, 1);
        // Room for the row alone, not for the row + its note.
        doc.y = PAGE_H - BOTTOM - LINE_H - 1.0;
        doc.grid_row_num_with_note(&["ROWX", "2"], &edges, 1, "NOTEX", edges[1]);
        doc.grid_end(&edges);
        assert_eq!(doc.page_index(), 1);
        let pages = page_streams(&doc.finish());
        assert!(!contains(&pages[0], "ROWX") && !contains(&pages[0], "NOTEX"));
        assert!(contains(&pages[1], "ROWX") && contains(&pages[1], "NOTEX"));
        assert!(
            contains(&pages[1], "HEAD"),
            "the header is replayed above them"
        );
    }

    #[test]
    fn a_pathological_ticker_does_not_reach_the_pdf_in_full() {
        // The 200-char ticker must be truncated before it is written into the content stream.
        let mut s = demo_study();
        s.security_ticker = "Z".repeat(200);
        let bytes = render_study_pdf(&s, NumberStyle::Point).expect("the study still renders");
        assert!(
            !bytes.windows(200).any(|w| w.iter().all(|b| *b == b'Z')),
            "the over-long ticker must be truncated, never written to the page in full"
        );
    }

    #[test]
    fn carries_no_naic_wordmark() {
        // The faithful layout must NOT embed NAIC marks/verbatim prose (open-source constraint). The
        // text is WinAnsi-encoded in the content streams; assert the wordmarks never appear.
        let bytes = render_study_pdf(&demo_study(), NumberStyle::Point).unwrap();
        for mark in [
            b"NAIC".as_slice(),
            b"Stock Selection Guide".as_slice(),
            b"Better Investing".as_slice(),
        ] {
            assert!(
                !bytes.windows(mark.len()).any(|w| w == mark),
                "the PDF must carry no NAIC wordmark"
            );
        }
    }

    // ── G1 final (#237) — the study PDF's remaining findings ──

    #[test]
    fn an_absent_current_price_is_said_absent_never_out_of_range() {
        // M1: the demo's zoning holds (40 … 162), the price is removed.
        let mut s = demo_study();
        s.judgment.current_price = None;
        let bytes = render_study_pdf(&s, NumberStyle::Comma).unwrap();
        assert!(contains(&bytes, PRICE_ABSENT));
        assert!(!contains(&bytes, "hors de la plage"));
        assert!(!contains(&bytes, BELOW_RANGE) && !contains(&bytes, ABOVE_RANGE));
        // No marker: its caption is never written.
        assert!(!contains(&bytes, &format!("{CURRENT_PRICE} {EM_DASH}")));
        // The place itself, read off the price and the bounds (never off the engine's `None`).
        let z = ZoneBounds {
            forecast_low: Decimal::from(40),
            buy_top: Decimal::from(80),
            neutral_top: Decimal::from(120),
            forecast_high: Decimal::from(160),
        };
        assert_eq!(price_place(&z, None), PricePlace::Absent);
        assert_eq!(price_place(&z, Some(Decimal::from(10))), PricePlace::Below);
        assert_eq!(price_place(&z, Some(Decimal::from(200))), PricePlace::Above);
        assert_eq!(
            price_place(&z, Some(Decimal::from(40))),
            PricePlace::In(Zone::Buy)
        );
        assert_eq!(
            price_place(&z, Some(Decimal::from(120))),
            PricePlace::In(Zone::Neutral)
        );
        assert_eq!(
            price_place(&z, Some(Decimal::from(160))),
            PricePlace::In(Zone::Sell)
        );
    }

    #[test]
    fn a_price_outside_the_zoning_is_named_and_marked_outside() {
        // L9: below the forecast low (40) and above the forecast high (162).
        let mut s = demo_study();
        s.judgment.current_price = Some(money_of("10"));
        let bytes = render_study_pdf(&s, NumberStyle::Point).unwrap();
        assert!(contains(
            &bytes,
            &format!("Le cours actuel 10 se situe : {BELOW_RANGE}")
        ));
        assert!(contains(
            &bytes,
            &format!("{CURRENT_PRICE} 10 — {MARKER_BELOW}")
        ));
        s.judgment.current_price = Some(money_of("500"));
        let bytes = render_study_pdf(&s, NumberStyle::Point).unwrap();
        assert!(contains(
            &bytes,
            &format!("Le cours actuel 500 se situe : {ABOVE_RANGE}")
        ));
        assert!(contains(
            &bytes,
            &format!("{CURRENT_PRICE} 500 — {MARKER_ABOVE}")
        ));
        // Inside: the plain caption, no « outside » word.
        let bytes = render_study_pdf(&demo_study(), NumberStyle::Point).unwrap();
        assert!(contains(&bytes, &format!("{CURRENT_PRICE} 80")));
        assert!(!contains(&bytes, MARKER_BELOW) && !contains(&bytes, MARKER_ABOVE));
    }

    #[test]
    fn a_chart_line_breaks_at_a_gap_and_at_a_non_plottable_value() {
        // M2 / L5: never bridged over a missing year, an absent value, a zero or a negative.
        let runs = plot_runs(&[
            (2016, Some(1.0)),
            (2015, Some(0.5)),
            (2017, Some(-1.0)), // negative: no place on a log scale
            (2018, Some(2.0)),
            (2019, Some(3.0)),
            // 2020 missing from the series
            (2021, Some(4.0)),
            (2022, None),
            (2023, Some(0.0)),
            (2024, Some(f64::NAN)),
        ]);
        assert_eq!(
            runs,
            vec![
                vec![(2015, 0.5), (2016, 1.0)],
                vec![(2018, 2.0), (2019, 3.0)],
                vec![(2021, 4.0)], // a lone point: kept, drawn as a dot
            ]
        );
        assert!(plot_runs(&[]).is_empty());
        assert!(plot_runs(&[(2020, Some(-2.0))]).is_empty());
    }

    #[test]
    fn the_first_year_is_not_drawn_on_the_frame_edge() {
        // L5: half a year of room at each end of the axis.
        let (first, last) = (2015, 2024);
        let span = f64::from(last - first + FORECAST_HORIZON_YEARS as i32) + 2.0 * YEAR_PAD;
        let origin = f64::from(first) - YEAR_PAD;
        let x = |y: i32| year_x(f64::from(y), origin, span, 100.0, 300.0);
        assert!(
            x(first) > 100.0 + 5.0,
            "the first bar clears the left frame"
        );
        let end = last + FORECAST_HORIZON_YEARS as i32;
        assert!(
            x(end) < 400.0 - 5.0,
            "the horizon's end clears the right frame"
        );
    }

    #[test]
    fn a_figure_keeps_its_trailing_zeros_as_on_the_screen() {
        // L6: the screen spells `round_for_display` as is — « 141,00 », « 4,0 % ».
        let d = |s: &str| Some(rust_decimal::Decimal::from_str_exact(s).unwrap());
        assert_eq!(NumberStyle::Comma.money(d("141.00")), "141,00");
        assert_eq!(NumberStyle::Point.pct(d("4.00")), "4.0 %");
        assert_eq!(NumberStyle::Comma.num(d("12.50")), "12,5");
        // Rounding still applies (never more places than the field's scale).
        assert_eq!(NumberStyle::Point.money(d("1.23456")), "1.23");
    }

    #[test]
    fn the_data_header_never_loses_the_manual_entry() {
        // L7: providers that fill the cell push « et saisie manuelle » to its own line, whole.
        let room = header_room((PAGE_W - 2.0 * MARGIN) / 3.0);
        let long = DataSource {
            head: format!(
                "{PROVIDER_MANY} {}",
                ["aaaaaaaaaaaaaaaaaaaaaaa"; 6].join(", ")
            ),
            and_manual: true,
        };
        let lines = long.header_lines(room, FONT);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1], AND_MANUAL);
        assert!(lines.iter().all(|l| text_width(l, FONT) <= room));
        // Two providers: the providers whole on the first line, the manual entry on the second.
        let two = DataSource {
            head: format!("{PROVIDER_MANY} eodhd, yahoo"),
            and_manual: true,
        };
        assert_eq!(
            two.header_lines(room, FONT),
            vec![
                "fournisseurs eodhd, yahoo".to_string(),
                AND_MANUAL.to_string()
            ]
        );
        // A short phrase stays on one line.
        let one = DataSource {
            head: format!("{PROVIDER_ONE} eodhd"),
            and_manual: true,
        };
        assert_eq!(one.header_lines(room, FONT).len(), 1);
        // End to end: two providers and a manual cell — the whole fact is in the PDF.
        let mut s = demo_study();
        for (i, y) in s.years.iter_mut().enumerate() {
            y.sales.source = Source::Provider;
            y.sales.provenance.hash_of_dependencies =
                format!("{}:x", if i % 2 == 0 { "eodhd" } else { "yahoo" });
        }
        let bytes = render_study_pdf(&s, NumberStyle::Comma).unwrap();
        assert!(contains(&bytes, AND_MANUAL));
    }

    #[test]
    fn the_header_date_is_labelled_as_the_creation_date() {
        // L11: the date shown is the study's creation, labelled as the screen labels it.
        let bytes = render_study_pdf(&demo_study(), NumberStyle::Comma).unwrap();
        assert!(contains(&bytes, CREATED_ON));
        assert!(contains(&bytes, "2026-03-09"));
    }

    #[test]
    fn a_block_that_fits_a_page_moves_whole_one_that_cannot_does_not() {
        // L10: the §3 block (heading, table, notes) is reserved as one when a page can hold it.
        let mut doc = Doc::new();
        doc.y = PAGE_H - BOTTOM - 100.0;
        doc.keep_together_if_it_fits(200.0);
        assert_eq!(doc.page_index(), 1, "moved whole to the next page");
        let mut doc = Doc::new();
        doc.y = PAGE_H - BOTTOM - 100.0;
        doc.keep_together_if_it_fits(PAGE_H);
        assert_eq!(
            doc.page_index(),
            0,
            "a block taller than a page is not chased"
        );
        // End to end: the demo's §3 notes sit on the page of its heading.
        let bytes = render_study_pdf(&demo_study(), NumberStyle::Point).unwrap();
        let pages = page_streams(&bytes);
        let with = |s: &str| pages.iter().position(|p| contains(p, s));
        assert_eq!(
            with("3. Historique cours / bénéfice"),
            with("8 · C/B moyen (D et E) :")
        );
    }
}

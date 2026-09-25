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
use steadyinvest_core::method::FORECAST_HORIZON_YEARS;
use steadyinvest_core::normalize::NormalizeError;
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
const SERIES_PAD_DECADES: f64 = 0.12; // per-series head/foot room (issue #25)
const MIN_SERIES_DECADES: f64 = 0.6; // a flat series still gets this much span (no false drama)
// Issue #207: the growth guide lines of the printed form — compound rates from the last EPS point.
const GUIDE_RATES_PCT: [u32; 6] = [5, 10, 15, 20, 25, 30];

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
pub fn render_study_pdf(study: &Study) -> Result<Vec<u8>, ReportError> {
    let frame = crate::form::build_frame(study).map_err(ReportError::Normalize)?;
    let outputs = frame.snapshot.outputs();
    let judgment = &study.judgment;
    let current_price = judgment.current_price.map(|m| m.as_decimal());

    let mut doc = Doc::new();

    // ── Page 1 — the header block (neutral — NOT the form's wordmark) ──
    doc.title("Analyse de sélection de titre");
    // Issue #74: a pathological identifier is truncated so the header cannot run past the A4
    // right edge (else it is silently clipped by the media box).
    let company = study
        .company_name
        .as_deref()
        .filter(|n| !n.trim().is_empty())
        .map(|n| truncate(n, 48))
        .unwrap_or_else(|| EM_DASH.to_string());
    doc.header_box(&[
        [
            ("Société", company.as_str()),
            ("Symbole", &truncate(&study.security_ticker, 24)),
            ("Date", &date_prefix(&study.created_at.0)),
        ],
        [
            ("Monnaie", &truncate(&study.native_currency, 16)),
            ("Données", &data_source(study)),
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
        fmt_dec(latest_bvps, DisplayField::PerShare),
    ));
    doc.gap(4.0);

    // ── §1 — the full-page semi-log plot + the four growth lines ──
    doc.section("1. Analyse visuelle des ventes, bénéfices et cours");
    doc.growth_chart(&frame);
    doc.gap(2.0);
    doc.two_columns(
        &format!(
            "(1) Croissance historique des ventes : {}",
            pct(outputs.growth.sales_cagr_pct)
        ),
        &format!(
            "(3) Croissance historique du BPA : {}",
            pct(outputs.growth.eps_cagr_pct)
        ),
    );
    doc.two_columns(
        &format!(
            "(2) Croissance estimée des ventes : {}",
            pct(judgment.projected_sales_growth_pct.map(|m| m.as_decimal()))
        ),
        &format!(
            "(4) Croissance estimée du BPA : {}",
            pct(judgment.projected_eps_growth_pct.map(|m| m.as_decimal()))
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
        head.push("Moy. 5 ans".to_string());
        head.push("Tendance".to_string());
        let head_refs: Vec<&str> = head.iter().map(String::as_str).collect();
        doc.grid_begin(2);
        doc.grid_row_small(&head_refs, &edges, true, 1);
        let mut ptp: Vec<String> = vec!["A · % marge avant impôt".to_string()];
        ptp.extend(rows.iter().map(|r| pct_bare(r.ptp_pct)));
        if rows.is_empty() {
            ptp.push(EM_DASH.to_string());
        }
        ptp.push(pct_bare(m.avg_ptp_pct));
        ptp.push(trend(m.ptp_trend).to_string());
        let refs: Vec<&str> = ptp.iter().map(String::as_str).collect();
        doc.grid_row_small(&refs, &edges, false, 1);
        let mut roe: Vec<String> = vec!["B · % rendement des c. propres".to_string()];
        roe.extend(rows.iter().map(|r| pct_bare(r.roe_pct)));
        if rows.is_empty() {
            roe.push(EM_DASH.to_string());
        }
        roe.push(pct_bare(m.avg_roe_pct));
        roe.push(trend(m.roe_trend).to_string());
        let refs: Vec<&str> = roe.iter().map(String::as_str).collect();
        doc.grid_row_small(&refs, &edges, false, 1);
        doc.grid_end(&edges);
    }
    doc.small_line("A = bénéfice avant impôt ÷ ventes × 100   ·   B = BPA ÷ valeur comptable par action × 100   ·   tendance = dernière année face à la moyenne");
    doc.gap(6.0);

    // ── §3 Price / earnings history — the form's columns A–H over the window, totals, averages,
    //    the average and current P/E. ──
    doc.section("3. Historique cours / bénéfice");
    {
        let v = &outputs.valuation;
        doc.grid_begin(v.per_year.len() + 2);
        doc.grid_row_num(
            &[
                "Année",
                "A · Haut",
                "B · Bas",
                "C · BPA",
                "D · A÷C",
                "E · B÷C",
                "F · Div.",
                "G · F÷C %",
                "H · F÷B %",
            ],
            &COLS9,
            true,
            1,
        );
        let (mut sum_d, mut sum_e, mut sum_g, mut sum_h) = (None, None, None, None);
        for row in &v.per_year {
            let cy = frame.series.iter().find(|y| y.year == row.year);
            let (hp, lp, ep, dv) = match cy {
                Some(y) => (y.high_price, y.low_price, y.eps, y.dividend_per_share),
                None => (None, None, None, None),
            };
            let cells = [
                row.year.to_string(),
                money(hp),
                money(lp),
                fmt_dec(ep, DisplayField::PerShare),
                num(row.high_pe),
                num(row.low_pe),
                fmt_dec(dv, DisplayField::PerShare),
                pct(row.payout_pct),
                pct(row.high_yield_pct),
            ];
            let refs: Vec<&str> = cells.iter().map(String::as_str).collect();
            doc.grid_row_num(&refs, &COLS9, false, 1);
            sum_d = add_known(sum_d, row.high_pe);
            sum_e = add_known(sum_e, row.low_pe);
            sum_g = add_known(sum_g, row.payout_pct);
            sum_h = add_known(sum_h, row.high_yield_pct);
        }
        let total = [
            "Total".to_string(),
            String::new(),
            String::new(),
            String::new(),
            num(sum_d),
            num(sum_e),
            String::new(),
            pct(sum_g),
            pct(sum_h),
        ];
        let refs: Vec<&str> = total.iter().map(String::as_str).collect();
        doc.grid_row_num(&refs, &COLS9, false, 1);
        let avg = [
            "Moyenne".to_string(),
            String::new(),
            String::new(),
            String::new(),
            num(v.avg_high_pe),
            num(v.avg_low_pe),
            String::new(),
            pct(v.avg_payout_pct),
            pct(v.avg_high_yield_pct),
        ];
        let refs: Vec<&str> = avg.iter().map(String::as_str).collect();
        doc.grid_row_num(&refs, &COLS9, false, 1);
        doc.grid_end(&COLS9);
        doc.line(&format!(
            "8 · C/B moyen (D et E) : {}   ·   9 · C/B actuel : {}   ·   valeur relative : {}",
            num(v.avg_pe),
            num(v.current_pe),
            pct(v.relative_value_pct),
        ));
        doc.line(&format!(
            "Cours actuel : {}   ·   plus haut de l'année en cours : {}   ·   plus bas de l'année en cours : {}",
            money(current_price),
            EM_DASH,
            EM_DASH,
        ));
    }
    doc.gap(6.0);

    // ── §4 Risk & reward — the form's A–E with every intermediate figure ──
    doc.keep_together(HEAD_FONT + 14.0 * LINE_H + ZONEBAR_H);
    doc.section("4. Risque et rendement sur 5 ans");
    {
        let r = &outputs.risk_reward;
        let c = &r.low_candidates;
        let est_high = outputs.growth.estimated_high_eps;
        let est_low = outputs.growth.estimated_low_eps;
        doc.line(&format!(
            "A · Prix haut à 5 ans : PER haut moyen {} × BPA estimé haut {} = {}",
            num(judgment.judged_avg_high_pe.map(|m| m.as_decimal())),
            fmt_dec(est_high, DisplayField::PerShare),
            money(r.forecast_high),
        ));
        doc.line("B · Prix bas à 5 ans, les quatre candidats :");
        doc.indent_line(&format!(
            "(a) PER bas moyen {} × BPA estimé bas {} = {}",
            num(judgment.judged_avg_low_pe.map(|m| m.as_decimal())),
            fmt_dec(est_low, DisplayField::PerShare),
            money(c.avg_low_pe_times_eps),
        ));
        doc.indent_line(&format!(
            "(b) Prix bas moyen des 5 dernières années = {}",
            money(c.avg_low_price_last_5y),
        ));
        doc.indent_line(&format!(
            "(c) Plus bas sévère récent = {}",
            money(c.recent_severe_low),
        ));
        doc.indent_line(&format!(
            "(d) Prix soutenu par le dividende : dividende {} ÷ rendement haut moyen {} = {}",
            fmt_dec(
                judgment.present_full_year_dividend.map(|m| m.as_decimal()),
                DisplayField::PerShare
            ),
            pct(outputs.valuation.avg_high_yield_pct),
            money(c.dividend_supported),
        ));
        doc.indent_line(&format!(
            "Prix bas retenu ({}) = {}",
            option_label(judgment.forecast_low_option),
            money(r.forecast_low),
        ));
        match &r.zones {
            Some(z) => {
                let range = z.forecast_high - z.forecast_low;
                let third = z.buy_top - z.forecast_low;
                doc.line(&format!(
                    "C · Zonage : étendue {} − {} = {}   ·   un tiers = {}",
                    money(Some(z.forecast_high)),
                    money(Some(z.forecast_low)),
                    money(Some(range)),
                    money(Some(third)),
                ));
                doc.indent_line(&format!(
                    "{} : {} à {}   ·   {} : {} à {}   ·   {} : {} à {}",
                    ZONE_LOW,
                    money(Some(z.forecast_low)),
                    money(Some(z.buy_top)),
                    ZONE_MID,
                    money(Some(z.buy_top)),
                    money(Some(z.neutral_top)),
                    ZONE_HIGH,
                    money(Some(z.neutral_top)),
                    money(Some(z.forecast_high)),
                ));
                doc.indent_line(&format!(
                    "Le cours actuel {} se situe : {}",
                    money(current_price),
                    zone_label(r.present_price_zone),
                ));
            }
            None => doc.line("C · Zonage : — (prévision incomplète ou plage dégénérée)"),
        }
        doc.line(&format!(
            "D · Ratio hausse / baisse : (prix haut {} − cours {}) ÷ (cours {} − prix bas {}) = {}",
            money(r.forecast_high),
            money(current_price),
            money(current_price),
            money(r.forecast_low),
            upside(&r.upside_downside),
        ));
        doc.line(&format!(
            "E · Objectif de cours : (prix haut {} ÷ cours {} × 100) − 100 = {} d'appréciation",
            money(r.forecast_high),
            money(current_price),
            pct(outputs.returns.projected_appreciation_pct),
        ));
        doc.gap(4.0);
        // Issue #105 — the zone bar (low/median/high thirds + the current-price marker).
        doc.zone_bar(r.zones.as_ref(), current_price);
    }
    doc.gap(6.0);

    // ── §5 Five-year potential — the form's A–C with the intermediate figures ──
    doc.keep_together(HEAD_FONT + 7.0 * LINE_H);
    doc.section("5. Potentiel à 5 ans");
    {
        let ret = &outputs.returns;
        doc.line(&format!(
            "A · Rendement présent : dividende {} ÷ cours {} × 100 = {}",
            fmt_dec(
                judgment.present_full_year_dividend.map(|m| m.as_decimal()),
                DisplayField::PerShare
            ),
            money(current_price),
            pct(ret.present_yield_pct),
        ));
        doc.line(&format!(
            "B · Rendement moyen sur 5 ans : BPA moyen projeté {} × % distribution moyen {} = dividende moyen {}",
            fmt_dec(ret.avg_annual_eps, DisplayField::PerShare),
            pct(outputs.valuation.avg_payout_pct),
            fmt_dec(ret.avg_annual_dividend, DisplayField::PerShare),
        ));
        doc.indent_line(&format!(
            "dividende moyen {} ÷ cours {} × 100 = {}",
            fmt_dec(ret.avg_annual_dividend, DisplayField::PerShare),
            money(current_price),
            pct(ret.avg_yield_pct),
        ));
        doc.line(&format!(
            "C · Rendement annuel total estimé : appréciation sur 5 ans {}, soit {} annualisée",
            pct(ret.projected_appreciation_pct),
            pct(ret.projected_annualized_appreciation_pct),
        ));
        doc.indent_line(&format!(
            "appréciation annualisée {} + rendement moyen {} = {}",
            pct(ret.projected_annualized_appreciation_pct),
            pct(ret.avg_yield_pct),
            total_return(ret),
        ));
        doc.small_line(
            "Les taux annualisés sont composés (et non simples) : (haut ÷ cours)^(1/5) − 1.",
        );
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
            cell(y.sales.value, DisplayField::LargeMonetary),
            cell(
                y.pre_tax_profit.as_ref().and_then(|c| c.value),
                DisplayField::LargeMonetary,
            ),
            cell(y.eps.value, DisplayField::PerShare),
            cell(y.high_price.value, DisplayField::Price),
            cell(y.low_price.value, DisplayField::Price),
            cell(
                y.dividend_per_share.as_ref().and_then(|c| c.value),
                DisplayField::PerShare,
            ),
            cell(
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

fn cell(v: Option<steadyinvest_contract::Money>, field: DisplayField) -> String {
    fmt_dec(v.map(|m| m.as_decimal()), field)
}

pub(crate) fn money(v: Option<Decimal>) -> String {
    fmt_dec(v, DisplayField::Price)
}

pub(crate) fn num(v: Option<Decimal>) -> String {
    fmt_dec(v, DisplayField::PeRatio)
}

pub(crate) fn pct(v: Option<Decimal>) -> String {
    match v {
        None => EM_DASH.to_string(),
        Some(d) => format!(
            "{} %",
            round_for_display(d, DisplayField::Percent).normalize()
        ),
    }
}

/// A percentage without its unit — for a table whose header already says « % » (the §2 columns).
fn pct_bare(v: Option<Decimal>) -> String {
    fmt_dec(v, DisplayField::Percent)
}

/// A running sum over KNOWN values only (the form's « Total » row sums the filled cells; an unknown
/// year is skipped, never counted as 0). `None` until the first known value.
fn add_known(acc: Option<Decimal>, v: Option<Decimal>) -> Option<Decimal> {
    match (acc, v) {
        (Some(a), Some(b)) => a.checked_add(b),
        (None, Some(b)) => Some(b),
        (acc, None) => acc,
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

/// Where the figures came from — the provider tag of the latest sales cell's provenance
/// (`"{tag}:{sha}"`, Story 6.9) or « saisie manuelle » (data, not prose; never a path or key).
fn data_source(study: &Study) -> String {
    use steadyinvest_contract::Source;
    let latest = study.years.iter().rev().find(|y| y.sales.value.is_some());
    match latest {
        Some(y) if y.sales.source == Source::Provider => y
            .sales
            .provenance
            .hash_of_dependencies
            .split(':')
            .next()
            .filter(|tag| !tag.is_empty() && tag.len() <= 24)
            .map(|tag| format!("fournisseur {tag}"))
            .unwrap_or_else(|| "fournisseur".to_string()),
        Some(_) => "saisie manuelle".to_string(),
        None => EM_DASH.to_string(),
    }
}

/// The §5 projected total (issue #189): the full total when both terms are known; when ONLY the
/// dividend history is missing (`ReturnOutputs::appreciation_only_potential`), the annualised
/// appreciation alone with the honest « (hors div.) » marker — mirrors `app`'s
/// `fmt_total_return`.
fn total_return(r: &steadyinvest_core::ssg::ReturnOutputs) -> String {
    match (
        r.projected_total_annualized_return_pct,
        r.appreciation_only_potential(),
    ) {
        (Some(total), _) => pct(Some(total)),
        (None, Some(appreciation)) => format!("{} (hors div.)", pct(Some(appreciation))),
        (None, None) => pct(None),
    }
}

pub(crate) fn fmt_dec(v: Option<Decimal>, field: DisplayField) -> String {
    match v {
        None => EM_DASH.to_string(),
        Some(d) => round_for_display(d, field).normalize().to_string(),
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

fn zone_label(z: Option<Zone>) -> &'static str {
    match z {
        Some(Zone::Buy) => "dans la zone basse",
        Some(Zone::Neutral) => "dans la zone médiane",
        Some(Zone::Sell) => "dans la zone haute",
        None => "hors de la plage prévue",
    }
}

fn upside(u: &UpsideDownside) -> String {
    match u {
        UpsideDownside::Ratio(d) => {
            format!(
                "{} : 1",
                round_for_display(*d, DisplayField::Ratio).normalize()
            )
        }
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

/// Truncate a display string to at most `max` characters, appending an ellipsis when cut (issue #74)
/// — so a pathological ticker/currency cannot run past the page's right edge. Char-based (never byte
/// slicing), so multibyte accents stay intact; `max` is assumed ≥ 1.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let kept: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{kept}…")
    }
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
const OPTION_A: &str = "PER bas × BPA bas";
const OPTION_B: &str = "prix bas moyen 5 ans";
const OPTION_C: &str = "plus bas sévère récent";
const OPTION_D: &str = "soutenu par le dividende";

// ── issue #105 / #207 — the embedded charts' neutral labels (greyscale legend + zone bands) ──
const CHART_LEGEND: &str = "BPA (trait épais)   ·   Ventes (trait fin)   ·   Cours haut–bas (barres)   ·   projection (pointillés)   ·   guides de croissance 5–30 % (gris clair)";
const CHART_SCALE_NOTE: &str = "Échelle logarithmique, propre à chaque série (l'axe gradué est celui du BPA) ; les guides partent du dernier BPA connu.";
const QUARTER_BOX_TITLE: &str = "Chiffres trimestriels récents";
const QUARTER_LATEST: &str = "Dernier trimestre";
const QUARTER_YEAR_AGO: &str = "Même trimestre, un an avant";
const QUARTER_CHANGE: &str = "Variation";
const ZONE_LOW: &str = "Zone basse";
const ZONE_MID: &str = "Zone médiane";
const ZONE_HIGH: &str = "Zone haute";
const CURRENT_PRICE: &str = "Cours actuel";

#[cfg(test)]
const REPORT_USER_FACING: &[&str] = &[
    // Header block.
    "Analyse de sélection de titre",
    "Société",
    "Symbole",
    "Date",
    "Monnaie",
    "Données",
    "Préparé par",
    "fournisseur",
    "saisie manuelle",
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
    "Moy. 5 ans",
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
    CHART_SCALE_NOTE,
    QUARTER_BOX_TITLE,
    QUARTER_LATEST,
    QUARTER_YEAR_AGO,
    QUARTER_CHANGE,
    ZONE_LOW,
    ZONE_MID,
    ZONE_HIGH,
    CURRENT_PRICE,
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

/// Accumulates content across one or more A4 pages, tracking a top-origin cursor and starting a new
/// page when the next line would cross the bottom margin.
pub(crate) struct Doc {
    pages: Vec<Content>,
    cur: Content,
    y: f32,        // top-origin cursor (distance from the page top)
    grid_top: f32, // the top of the grid table currently being drawn (issue #104)
    // Issue #74: the current table's column header, remembered on the header row so it can be
    // replayed at the top of each continuation page when the table spans a break.
    grid_header: Vec<String>,
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
        self.prose(s, MARGIN, FONT, LINE_H);
    }

    /// A caption-sized line (the form's small print: formulas, footnotes).
    pub(crate) fn small_line(&mut self, s: &str) {
        self.prose(s, MARGIN, SMALL, LINE_H - 2.0);
    }

    /// A body line indented under its lettered parent (the §4 candidates, the zoning lines).
    pub(crate) fn indent_line(&mut self, s: &str) {
        self.prose(s, MARGIN + 18.0, FONT, LINE_H);
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
    pub(crate) fn header_box(&mut self, rows: &[[(&str, &str); 3]]) {
        let row_h = LINE_H + SMALL + 2.0;
        let h = row_h * rows.len() as f32 + 4.0;
        self.ensure(h + 4.0);
        let (x0, x1) = (MARGIN, self.page_w - MARGIN);
        let col_w = (x1 - x0) / 3.0;
        let top = self.y;
        stroke_rect(&mut self.cur, x0, top, x1 - x0, h, 0.6);
        for c in 1..3 {
            vline(&mut self.cur, x0 + col_w * c as f32, top, top + h, 0.4);
        }
        for (r, row) in rows.iter().enumerate() {
            let ry = top + 2.0 + row_h * r as f32;
            if r > 0 {
                hline(&mut self.cur, x0, x1, ry - 1.0, 0.4);
            }
            for (c, (label, value)) in row.iter().enumerate() {
                let x = x0 + col_w * c as f32 + CELL_PAD;
                text(&mut self.cur, x, ry + SMALL, SMALL, label);
                text(&mut self.cur, x, ry + SMALL + FONT + 1.5, FONT, value);
            }
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
        let height = self.grid_row_height(cells, edges);
        if head {
            self.grid_header = cells.iter().map(|s| s.to_string()).collect();
        } else if self.page_h - self.y - height < BOTTOM {
            self.close_grid_box(edges);
            self.new_page();
            self.grid_top = self.y;
            let header = self.grid_header.clone();
            let refs: Vec<&str> = header.iter().map(String::as_str).collect();
            self.draw_grid_cells_aligned(&refs, edges, true, numeric.clone());
        }
        self.draw_grid_cells_aligned(cells, edges, head, numeric);
    }

    /// Each cell's lines, wrapped to its column (a cell never crosses a rule). A text that fits
    /// between the rules stays whole even if it eats into the padding (a narrow year column's
    /// « 2016 »); only a longer one wraps at the padded width.
    fn grid_cell_lines(&self, cells: &[&str], edges: &[f32]) -> Vec<Vec<String>> {
        let size = self.grid_font;
        cells
            .iter()
            .enumerate()
            .map(|(i, s)| match edges.get(i + 1) {
                Some(right) if text_width(s, size) > right - edges[i] - 2.0 * GRID_INSET => {
                    wrap_to_width(s, right - edges[i] - 2.0 * CELL_PAD, size)
                }
                _ => vec![s.to_string()],
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
            .map(Vec::len)
            .max()
            .unwrap_or(1)
            .max(1);
        LINE_H + (lines - 1) as f32 * self.grid_line_step()
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
        let size = self.grid_font;
        let step = self.grid_line_step();
        let lines = self.grid_cell_lines(cells, edges);
        let rows = lines.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let top = self.y + size;
        for (i, cell_lines) in lines.iter().enumerate() {
            for (k, line) in cell_lines.iter().enumerate() {
                let y = top + k as f32 * step;
                let Some(right) = edges.get(i + 1) else {
                    text(&mut self.cur, edges[i] + CELL_PAD, y, size, line);
                    continue;
                };
                // The padded position, shifted back inside the rules when the text is wider.
                let w = text_width(line, size);
                let x = if numeric.contains(&i) {
                    right - CELL_PAD - w
                } else {
                    (edges[i] + CELL_PAD).min(right - GRID_INSET - w)
                };
                text(&mut self.cur, x.max(edges[i] + GRID_INSET), y, size, line);
            }
        }
        self.y = top + (rows - 1) as f32 * step + (LINE_H - size);
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
    /// draw the outer box + column rules. Issue #74: a grid may SPAN page breaks — [`grid_row_num`]
    /// closes the box at a break and replays the header on the continuation page.
    pub(crate) fn grid_begin(&mut self, _rows: usize) {
        self.keep_together(2.0 * LINE_H + 4.0);
        self.grid_top = self.y;
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

    /// A small-print note under a grid row, spanning from `left` to the table's right edge and
    /// wrapped there; the interior column rules stop above it and resume below (the review's
    /// « Signaux · Données » line under each position).
    pub(crate) fn grid_note_row(&mut self, s: &str, left: f32, edges: &[f32]) {
        let right = edges[edges.len() - 1];
        let outer = [edges[0], left, right];
        self.grid_font = SMALL;
        let height = self.grid_row_height(&["", s], &outer);
        if self.page_h - self.y - height < BOTTOM {
            self.close_grid_box(edges);
            self.new_page();
            self.grid_top = self.y;
            let header = self.grid_header.clone();
            let refs: Vec<&str> = header.iter().map(String::as_str).collect();
            self.grid_font = FONT;
            self.draw_grid_cells_aligned(&refs, edges, true, 2..usize::MAX);
            self.grid_font = SMALL;
        }
        let from = self.y;
        self.draw_grid_cells_aligned(&["", s], &outer, false, usize::MAX..usize::MAX);
        self.grid_spans.push((from, self.y));
    }

    /// Close the grid: box the final (or only) page's portion, then advance past it.
    pub(crate) fn grid_end(&mut self, edges: &[f32]) {
        self.close_grid_box(edges);
        self.y += 2.0;
    }

    /// Issue #105 / #207 — the §1 semi-log growth chart, filling the rest of page 1 like the printed
    /// form. Sales / EPS / Price on log scales (each series its own — issue #25; the EPS scale is the
    /// labelled one), the yearly high–low PRICE as vertical bars, the est-high / est-low EPS
    /// projection from the last EPS point to the forecast horizon, and the form's growth GUIDE lines
    /// (5–30 % compound from the last EPS point, light grey, labelled at the right edge). Greyscale-
    /// safe: weight + dash + shade, NEVER colour. Nothing is drawn when there is no plottable data
    /// (the annexe already carries the em-dashes).
    fn growth_chart(&mut self, frame: &crate::form::StudyFrame) {
        let series = &frame.series;
        let outputs = frame.snapshot.outputs();
        let pts_of =
            |get: &dyn Fn(&steadyinvest_core::normalize::CanonicalYear) -> Option<Decimal>| {
                series
                    .iter()
                    .enumerate()
                    .filter_map(|(i, cy)| {
                        get(cy)
                            .and_then(|d| d.to_f64())
                            .filter(|v| *v > 0.0)
                            .map(|v| (i, v))
                    })
                    .collect::<Vec<(usize, f64)>>()
            };
        let sales = pts_of(&|cy| cy.sales);
        let eps = pts_of(&|cy| cy.eps);
        let highs = pts_of(&|cy| cy.high_price);
        let lows = pts_of(&|cy| cy.low_price);
        let est_high = outputs.growth.estimated_high_eps.and_then(|d| d.to_f64());
        let est_low = outputs.growth.estimated_low_eps.and_then(|d| d.to_f64());

        if series.is_empty() || (sales.is_empty() && eps.is_empty() && highs.is_empty()) {
            return;
        }

        // The plot takes what is left of the page above the four growth lines + the legend.
        let reserved_below = 5.0 * LINE_H + 16.0;
        let chart_h = (self.page_h - self.y - BOTTOM - reserved_below).max(CHART_MIN_H);
        self.ensure(chart_h + reserved_below);
        let top = self.y;
        let x0 = MARGIN + CHART_AXIS_W;
        let x1 = self.page_w - MARGIN;
        let plot_w = x1 - x0;
        let n = series.len();
        let span = ((n as f64 - 1.0) + f64::from(FORECAST_HORIZON_YEARS)).max(1.0);
        let px = |i: f64| x0 + ((i / span) * f64::from(plot_w)) as f32;
        let py = |v: f64, lmin: f64, lmax: f64| {
            let t = ((v.max(1e-9).log10() - lmin) / (lmax - lmin)).clamp(0.0, 1.0);
            top + (f64::from(chart_h) * (1.0 - t)) as f32
        };

        // Issue #25 (multi-scale): each series on its OWN log range so none is crushed by another's
        // magnitude. The EPS scale (the decision series, its projection and the guides) is labelled.
        let vals = |pts: &[(usize, f64)]| pts.iter().map(|p| p.1).collect::<Vec<f64>>();
        let sales_b = series_log_bounds(&vals(&sales));
        let mut price_vals = vals(&highs);
        price_vals.extend(vals(&lows));
        let price_b = series_log_bounds(&price_vals);
        let mut eps_scale_vals = vals(&eps);
        eps_scale_vals.extend(est_high.filter(|v| *v > 0.0));
        eps_scale_vals.extend(est_low.filter(|v| *v > 0.0));
        // The steepest guide (30 % over the horizon from the last EPS) reserves headroom.
        if let Some((_, lv)) = eps.last() {
            eps_scale_vals.push(lv * 1.30f64.powi(FORECAST_HORIZON_YEARS as i32));
        }
        let eps_b = series_log_bounds(&eps_scale_vals);

        stroke_rect(&mut self.cur, x0, top, plot_w, chart_h, 0.6);
        // Gridlines + labels on the EPS scale (nice 1/2/5×10^k).
        if let Some((lmin, lmax)) = eps_b {
            for (v, lbl) in nice_ticks(lmin, lmax) {
                let gy = py(v, lmin, lmax);
                polyline(&mut self.cur, &[(x0, gy), (x1, gy)], 0.3, GRID_GRAY, &[]);
                text(&mut self.cur, MARGIN, gy + 2.5, 7.0, &lbl);
            }
        }
        // Issue #207 — the growth guide lines: from the last historical EPS point, each rate compounded
        // over the horizon, light grey, labelled at the right edge (the printed form's fan).
        if let (Some((lmin, lmax)), Some((li, lv))) = (eps_b, eps.last().copied()) {
            let (ox, oy) = (px(li as f64), py(lv, lmin, lmax));
            for rate in GUIDE_RATES_PCT {
                let end = lv * (1.0 + f64::from(rate) / 100.0).powi(FORECAST_HORIZON_YEARS as i32);
                let ey = py(end, lmin, lmax);
                polyline(
                    &mut self.cur,
                    &[(ox, oy), (px(span), ey)],
                    0.4,
                    GUIDE_GRAY,
                    &[],
                );
                text(
                    &mut self.cur,
                    x1 - 19.0,
                    ey - 2.0,
                    5.5,
                    &format!("{rate} %"),
                );
            }
        }
        // The yearly high–low price bars (price scale): a vertical segment with short caps.
        if let Some((lmin, lmax)) = price_b {
            for (i, hv) in &highs {
                if let Some((_, lo)) = lows.iter().find(|(j, _)| j == i) {
                    let x = px(*i as f64);
                    let (yh, yl) = (py(*hv, lmin, lmax), py(*lo, lmin, lmax));
                    polyline(&mut self.cur, &[(x, yh), (x, yl)], 0.8, SERIES_GRAY, &[]);
                    polyline(
                        &mut self.cur,
                        &[(x - 2.0, yh), (x + 2.0, yh)],
                        0.8,
                        SERIES_GRAY,
                        &[],
                    );
                    polyline(
                        &mut self.cur,
                        &[(x - 2.0, yl), (x + 2.0, yl)],
                        0.8,
                        SERIES_GRAY,
                        &[],
                    );
                }
            }
        }
        // The Sales (thin) and EPS (thick) lines, each on its own scale (greyscale: weight).
        let draw = |cur: &mut Content,
                    pts: &[(usize, f64)],
                    b: Option<(f64, f64)>,
                    w: f32,
                    dash: &[f32]| {
            if let Some((lmin, lmax)) = b {
                let p: Vec<(f32, f32)> = pts
                    .iter()
                    .map(|(i, v)| (px(*i as f64), py(*v, lmin, lmax)))
                    .collect();
                polyline(cur, &p, w, SERIES_GRAY, dash);
            }
        };
        draw(&mut self.cur, &sales, sales_b, 0.8, &[]);
        draw(&mut self.cur, &eps, eps_b, 1.6, &[]);
        // Projection from the last EPS point to est-high / est-low at the horizon (dotted), EPS scale.
        if let (Some((lmin, lmax)), Some((li, lv))) = (eps_b, eps.last().copied()) {
            let (ox, oy) = (px(li as f64), py(lv, lmin, lmax));
            if let Some(h) = est_high.filter(|v| *v > 0.0) {
                polyline(
                    &mut self.cur,
                    &[(ox, oy), (px(span), py(h, lmin, lmax))],
                    1.2,
                    SERIES_GRAY,
                    &[1.5, 2.0],
                );
            }
            if let Some(l) = est_low.filter(|v| *v > 0.0) {
                polyline(
                    &mut self.cur,
                    &[(ox, oy), (px(span), py(l, lmin, lmax))],
                    1.0,
                    SERIES_GRAY,
                    &[1.5, 2.0],
                );
            }
        }
        // Issue #207 — the form's « recent quarterly figures » box, top-left inside the plot. v1
        // carries no quarterly data: the box states the absence (em-dashes), never a guessed figure.
        {
            let (bx, by, bw, bh) = (x0 + 6.0, top + 6.0, 200.0, 4.0 * (SMALL + 3.0) + 8.0);
            fill_rect(&mut self.cur, bx, by, bw, bh, 1.0);
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
        }
        // Issue #104 — year labels along the x-axis (each historical year under its column).
        for (i, cy) in series.iter().enumerate() {
            text_centered(
                &mut self.cur,
                px(i as f64),
                top + chart_h + 9.0,
                6.5,
                &cy.year.to_string(),
            );
        }
        self.y = top + chart_h + 13.0;
        self.small_line(CHART_LEGEND);
        self.small_line(CHART_SCALE_NOTE);
    }

    /// Issue #105 — the §4 zone bar. A horizontal band from forecast-low to forecast-high split into
    /// the three thirds (low / median / high), greyscale-shaded (light → dark) with a label in each,
    /// and a marker at the current price. Greyscale-safe: the bands read by shade + label + position,
    /// never hue. Nothing is drawn when the forecast is incomplete (the §4 text already says so).
    fn zone_bar(&mut self, zones: Option<&ZoneBounds>, current_price: Option<Decimal>) {
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
        self.ensure(ZONEBAR_H + 2.0 * LINE_H + 12.0);
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
        text(&mut self.cur, x0, by, 7.0, &money(Some(z.forecast_low)));
        text_centered(&mut self.cur, fx(buy), by, 7.0, &money(Some(z.buy_top)));
        text_centered(&mut self.cur, fx(neu), by, 7.0, &money(Some(z.neutral_top)));
        let hi_lbl = money(Some(z.forecast_high));
        text_right(&mut self.cur, x1, by, 7.0, &hi_lbl);

        // Current-price marker: a vertical line through the bar + a caption above.
        if let Some(cp) = current_price.and_then(|d| d.to_f64()) {
            let mx = fx(cp);
            polyline(
                &mut self.cur,
                &[(mx, top - 4.0), (mx, top + ZONEBAR_H + 2.0)],
                1.3,
                0.0,
                &[],
            );
            text_centered(
                &mut self.cur,
                mx,
                top - 6.0,
                7.0,
                &format!("{CURRENT_PRICE} {}", money(current_price)),
            );
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

/// One glyph's Helvetica width (1/1000 em) — accented letters take their base letter's width, as
/// in the AFM; anything [`winansi`] cannot encode renders as '?' (556).
fn glyph_width(c: char) -> u16 {
    match c {
        ' '..='~' => HELVETICA_ASCII[c as usize - 32],
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'è' | 'é' | 'ê' | 'ë' => 556,
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ù' | 'ú' | 'û' | 'ü' | 'ñ' => 556,
        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 278,
        'ç' | 'ý' | 'ÿ' => 500,
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'È' | 'É' | 'Ê' | 'Ë' => 667,
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 778,
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ç' | 'Ñ' => 722,
        '—' | '…' => 1000,
        '–' | '«' | '»' | '€' => 556,
        '’' => 222,
        '·' | '\u{a0}' => 278,
        '°' => 400,
        '×' | '÷' => 584,
        '−' => 333, // encoded as the hyphen-minus
        _ => 556,
    }
}

/// The rendered width of `s` in Helvetica at `size` points.
pub(crate) fn text_width(s: &str, size: f32) -> f32 {
    s.chars().map(|c| f32::from(glyph_width(c))).sum::<f32>() * size / 1000.0
}

/// `s` cut to fit `width` points at `size`, ending with « … » when cut (never spilling over).
pub(crate) fn fit(s: &str, width: f32, size: f32) -> String {
    if text_width(s, size) <= width {
        return s.to_string();
    }
    let room = width - text_width("…", size);
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
/// than the line is cut with « … » ([`fit`]). An empty `s` is one empty line.
pub(crate) fn wrap_to_width(s: &str, width: f32, size: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in s.split(' ') {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if line.is_empty() || text_width(&candidate, size) <= width {
            line = candidate;
        } else {
            lines.push(fit(&line, width, size));
            line = word.to_string();
        }
    }
    lines.push(fit(&line, width, size));
    lines
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
        if *v > 0.0 {
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

/// Nice `1 / 2 / 5 × 10^k` tick values (+ their compact labels) inside a log scale `[10^lmin, 10^lmax]`.
fn nice_ticks(lmin: f64, lmax: f64) -> Vec<(f64, String)> {
    let (min, max) = (10f64.powf(lmin), 10f64.powf(lmax));
    let mut out = Vec::new();
    for k in (min.log10().floor() as i32)..=(max.log10().ceil() as i32) {
        for m in [1.0, 2.0, 5.0] {
            let v = m * 10f64.powi(k);
            if v >= min && v <= max {
                out.push((v, compact_num(v)));
            }
        }
    }
    out
}

/// A compact axis label: plain up to 999, then `k / M / Md` (French short scale) — data, not prose.
fn compact_num(v: f64) -> String {
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

/// Encode a UTF-8 string as WinAnsi (Latin-1 for the accent range we use, plus a few WinAnsi-only
/// code points). Characters outside the encoding fall back to '?', never panic.
fn winansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c as u32 {
            0x2014 => 0x97,            // — em dash
            0x2013 => 0x96,            // – en dash
            0x2026 => 0x85,            // … horizontal ellipsis (issue #74 truncation)
            0x2019 => 0x92,            // ’ right single quote
            0x2212 => 0x2D,            // − minus sign → hyphen-minus (formulas)
            0x20AC => 0x80,            // € euro
            n if n <= 0x7F => n as u8, // ASCII
            // Latin-1 high range == WinAnsi (é è à ç ° …). The C1 controls 0x80–0x9F are NOT
            // identity-mapped in WinAnsi, so they fall through to '?' rather than mis-render.
            n if (0xA0..=0xFF).contains(&n) => n as u8,
            _ => b'?',
        })
        .collect()
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
        let bytes = render_study_pdf(&demo_study()).expect("a normalizing study renders");
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
        let a = render_study_pdf(&demo_study()).unwrap();
        let b = render_study_pdf(&demo_study()).unwrap();
        assert_eq!(a, b, "the same study must render identical bytes");
    }

    #[test]
    fn a_degenerate_study_renders_neutrally_without_panicking() {
        // A study with no years still normalizes (no usable data → unknown figures); the renderer must
        // produce a calm PDF with em-dashes, never panic. (Genuine normalize failures take the
        // `ReportError::Normalize` path — exercised by `core`'s own normalize tests.)
        let mut s = demo_study();
        s.years.clear();
        let bytes = render_study_pdf(&s).expect("a degenerate study still renders");
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(
            bytes.windows(5).any(|w| w == b"%%EOF"),
            "still a well-formed PDF"
        );
    }

    #[test]
    fn unknown_figures_format_as_the_em_dash_never_zero() {
        // The project's most-repeated rail, at the formatter level (the PDF hex-encodes the glyph).
        assert_eq!(money(None), EM_DASH);
        assert_eq!(num(None), EM_DASH);
        assert_eq!(pct(None), EM_DASH);
        assert_eq!(super::cell(None, DisplayField::LargeMonetary), EM_DASH);
        assert_eq!(trend(None), EM_DASH);
        // A present value formats as its rounded decimal (no spurious zero-padding).
        assert_eq!(
            money(Some(rust_decimal::Decimal::from_str_exact("80").unwrap())),
            "80"
        );
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
        for z in [Some(Zone::Buy), Some(Zone::Neutral), Some(Zone::Sell), None] {
            assert_neutral(zone_label(z));
        }
        for t in [Some(Trend::Up), Some(Trend::Even), Some(Trend::Down), None] {
            assert_neutral(trend(t));
        }
        for u in [
            UpsideDownside::Ratio(rust_decimal::Decimal::ONE),
            UpsideDownside::Undefined,
            UpsideDownside::Unknown,
        ] {
            assert_neutral(&upside(&u));
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
        let bytes = render_study_pdf(&s).expect("a long study renders");
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
        let bytes = render_study_pdf(&s).expect("a long study renders");
        // "Cours haut" is a §1-only header cell (WinAnsi = ASCII, so a contiguous byte run). One
        // occurrence per page the table touches → ≥ 2 proves the header was replayed.
        let header = b"Cours haut";
        let count = bytes.windows(header.len()).filter(|w| *w == header).count();
        assert!(
            count >= 2,
            "the §1 column header must repeat on continuation pages, saw {count}"
        );
    }

    #[test]
    fn an_over_long_identifier_is_truncated_with_an_ellipsis() {
        // Issue #74: the header truncates a pathological ticker/currency instead of running it off
        // the page (clipped by the media box).
        let long = "X".repeat(200);
        let t = truncate(&long, 40);
        assert_eq!(t.chars().count(), 40, "clamped to the max width");
        assert!(t.ends_with('…'), "a cut string ends with an ellipsis");
        // A string within budget is returned untouched — no spurious ellipsis.
        assert_eq!(truncate("NESN", 40), "NESN");
        assert_eq!(truncate("CHF", 16), "CHF");
    }

    #[test]
    fn a_pathological_ticker_does_not_reach_the_pdf_in_full() {
        // The 200-char ticker must be truncated before it is written into the content stream.
        let mut s = demo_study();
        s.security_ticker = "Z".repeat(200);
        let bytes = render_study_pdf(&s).expect("the study still renders");
        assert!(
            !bytes.windows(200).any(|w| w.iter().all(|b| *b == b'Z')),
            "the over-long ticker must be truncated, never written to the page in full"
        );
    }

    #[test]
    fn carries_no_naic_wordmark() {
        // The faithful layout must NOT embed NAIC marks/verbatim prose (open-source constraint). The
        // text is WinAnsi-encoded in the content streams; assert the wordmarks never appear.
        let bytes = render_study_pdf(&demo_study()).unwrap();
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
}

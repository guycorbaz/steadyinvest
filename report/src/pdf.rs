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
const PAGE_W: f32 = 595.0;
const PAGE_H: f32 = 842.0;
const MARGIN: f32 = 42.0;
const FONT: f32 = 9.0;
const TITLE_FONT: f32 = 15.0;
const HEAD_FONT: f32 = 11.0;
const LINE_H: f32 = 13.0;
const BOTTOM: f32 = MARGIN + 24.0; // keep clear of the footer disclaimer

// ── chart geometry (issue #105 — vector graphics into the PDF, greyscale-safe) ──
const CHART_H: f32 = 150.0; // §1 semi-log plot height (points)
const CHART_AXIS_W: f32 = 30.0; // left gutter for the y-axis decade labels
const ZONEBAR_H: f32 = 26.0; // §4 zone bar height (points)
const SERIES_PAD_DECADES: f64 = 0.12; // per-series head/foot room (issue #25)
const MIN_SERIES_DECADES: f64 = 0.6; // a flat series still gets this much span (no false drama)

// ── grid tables (issue #104 — visible SSG grid) ──
const CELL_PAD: f32 = 5.0; // left padding of text inside a grid cell
// Column boundaries (left … right) for the 5-column (year + four figures) and 3-column tables.
const COLS5: [f32; 6] = [
    MARGIN,
    MARGIN + 62.0,
    MARGIN + 182.0,
    MARGIN + 292.0,
    MARGIN + 402.0,
    PAGE_W - MARGIN,
];
const COLS3: [f32; 4] = [MARGIN, MARGIN + 150.0, MARGIN + 330.0, PAGE_W - MARGIN];
const RULE_GRAY: f32 = 0.35; // the default rule/grid grey (restored after a chart)
const GRID_GRAY: f32 = 0.75; // faint decade gridlines
const SERIES_GRAY: f32 = 0.0; // series strokes (black; told apart by weight + dash, never hue)

/// Render a study to a faithful, neutral, greyscale PDF (FR52). Read-only: it computes nothing the
/// engine does not already compute, writes no journal, and needs no provider.
pub fn render_study_pdf(study: &Study) -> Result<Vec<u8>, ReportError> {
    let frame = crate::form::build_frame(study).map_err(ReportError::Normalize)?;
    let outputs = frame.snapshot.outputs();

    let mut doc = Doc::new();

    // ── Header (neutral — NOT "Stock Selection Guide") ──
    doc.title("Analyse de sélection de titre");
    doc.line(&format!(
        "Titre : {}   ·   Monnaie : {}   ·   Décision : {}",
        study.security_ticker,
        study.native_currency,
        date_prefix(&study.created_at.0),
    ));
    doc.gap(6.0);

    // ── §1 Visual analysis (historical series) ──
    doc.section("1. Analyse visuelle (historique)");
    doc.grid_begin(study.years.len());
    doc.grid_row(&["Année", "Ventes", "BPA", "Cours haut", "Cours bas"], &COLS5, true);
    for y in &study.years {
        let (yr, sa, ep, hp, lp) = (
            y.year.to_string(),
            cell(y.sales.value, DisplayField::LargeMonetary),
            cell(y.eps.value, DisplayField::PerShare),
            cell(y.high_price.value, DisplayField::Price),
            cell(y.low_price.value, DisplayField::Price),
        );
        doc.grid_row(&[&yr, &sa, &ep, &hp, &lp], &COLS5, false);
    }
    doc.grid_end(&COLS5);
    doc.gap(3.0);
    doc.line(&format!(
        "Croissance annualisée — ventes : {}   ·   BPA : {}",
        pct(outputs.growth.sales_cagr_pct),
        pct(outputs.growth.eps_cagr_pct),
    ));
    doc.gap(4.0);
    // Issue #105 — the semi-log growth chart (Sales/EPS/Price + est-high/low EPS projection).
    doc.growth_chart(&frame);
    doc.gap(6.0);

    // ── §2 Management ──
    doc.section("2. Évaluation de la gestion");
    doc.grid_begin(outputs.management.per_year.len());
    doc.grid_row(&["Année", "% BAII / ventes", "% rendement c.p."], &COLS3, true);
    for r in &outputs.management.per_year {
        let (yr, ptp, roe) = (r.year.to_string(), pct(r.ptp_pct), pct(r.roe_pct));
        doc.grid_row(&[&yr, &ptp, &roe], &COLS3, false);
    }
    doc.grid_end(&COLS3);
    doc.gap(3.0);
    doc.line(&format!(
        "Moyennes — % BAII/ventes : {} ({})   ·   % rendement c.p. : {} ({})",
        pct(outputs.management.avg_ptp_pct),
        trend(outputs.management.ptp_trend),
        pct(outputs.management.avg_roe_pct),
        trend(outputs.management.roe_trend),
    ));
    doc.gap(6.0);

    // ── §3 Price / earnings history ──
    doc.section("3. Historique cours / bénéfice");
    doc.grid_begin(outputs.valuation.per_year.len());
    doc.grid_row(
        &["Année", "C/B haut", "C/B bas", "% distribution", "% rendement"],
        &COLS5,
        true,
    );
    for v in &outputs.valuation.per_year {
        let (yr, hpe, lpe, pay, yld) = (
            v.year.to_string(),
            num(v.high_pe),
            num(v.low_pe),
            pct(v.payout_pct),
            pct(v.high_yield_pct),
        );
        doc.grid_row(&[&yr, &hpe, &lpe, &pay, &yld], &COLS5, false);
    }
    doc.grid_end(&COLS5);
    doc.gap(3.0);
    doc.line(&format!(
        "Moyennes — C/B haut : {}   ·   C/B bas : {}   ·   C/B moyen : {}   ·   % distribution : {}",
        num(outputs.valuation.avg_high_pe),
        num(outputs.valuation.avg_low_pe),
        num(outputs.valuation.avg_pe),
        pct(outputs.valuation.avg_payout_pct),
    ));
    doc.gap(6.0);

    // ── §4 Risk & reward ── (issue #104: the forecast lines + the zone bar stay together)
    doc.keep_together(HEAD_FONT + 5.0 * LINE_H + ZONEBAR_H + 2.0 * LINE_H);
    doc.section("4. Risque et rendement");
    doc.line(&format!(
        "Prévision — haute : {}   ·   basse : {}",
        money(outputs.risk_reward.forecast_high),
        money(outputs.risk_reward.forecast_low),
    ));
    match &outputs.risk_reward.zones {
        Some(z) => {
            doc.line(&format!(
                "Zone basse : {} – {}   ·   Zone médiane : {} – {}   ·   Zone haute : {} – {}",
                money(Some(z.forecast_low)),
                money(Some(z.buy_top)),
                money(Some(z.buy_top)),
                money(Some(z.neutral_top)),
                money(Some(z.neutral_top)),
                money(Some(z.forecast_high)),
            ));
        }
        None => doc.line("Zones : — (prévision incomplète ou plage dégénérée)"),
    }
    doc.line(&format!(
        "Position du cours actuel : {}   ·   Rapport hausse / baisse : {}",
        zone_label(outputs.risk_reward.present_price_zone),
        upside(&outputs.risk_reward.upside_downside),
    ));
    doc.gap(4.0);
    // Issue #105 — the zone bar (low/median/high thirds + the current-price marker), greyscale-safe.
    doc.zone_bar(
        outputs.risk_reward.zones.as_ref(),
        study.judgment.current_price.map(|m| m.as_decimal()),
    );
    doc.gap(6.0);

    // ── §5 Five-year potential ──
    doc.section("5. Potentiel à 5 ans");
    doc.line(&format!(
        "Rendement présent : {}   ·   Appréciation projetée : {}",
        pct(outputs.returns.present_yield_pct),
        pct(outputs.returns.projected_appreciation_pct),
    ));
    doc.line(&format!(
        "Rendement annualisé total projeté : {}",
        pct(outputs.returns.projected_total_annualized_return_pct),
    ));
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

    Ok(doc.finish())
}

// ── neutral formatting helpers (None → em-dash, never 0; exact-decimal display rounding) ──

fn cell(v: Option<steadyinvest_contract::Money>, field: DisplayField) -> String {
    fmt_dec(v.map(|m| m.as_decimal()), field)
}

fn money(v: Option<Decimal>) -> String {
    fmt_dec(v, DisplayField::Price)
}

fn num(v: Option<Decimal>) -> String {
    fmt_dec(v, DisplayField::PeRatio)
}

fn pct(v: Option<Decimal>) -> String {
    match v {
        None => EM_DASH.to_string(),
        Some(d) => format!(
            "{} %",
            round_for_display(d, DisplayField::Percent).normalize()
        ),
    }
}

fn fmt_dec(v: Option<Decimal>, field: DisplayField) -> String {
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
        Some(Zone::Buy) => "Zone basse",
        Some(Zone::Neutral) => "Zone médiane",
        Some(Zone::Sell) => "Zone haute",
        None => "— (hors plage)",
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

const EM_DASH: &str = "—";

// ── neutral user-facing string inventory (FR13) ──
//
// `report` lives outside the `app` posture gate, so it guards its own neutrality: every static
// user-facing string the PDF emits is listed in [`REPORT_USER_FACING`] (or is a `zone_label` /
// `trend` / verdict const, all covered by the neutrality test). Add new strings here when you add
// them to the layout — the test scans them against `core::method::BANNED_VERBS_{FR,EN}` and asserts
// no NAIC wordmark, the same shared catalogs the app gate uses.
const VERDICT_FULL: &str = "Tous les critères validés et à jour";
const VERDICT_PROVISIONAL: &str = "Provisoire — données à revérifier ou confiance réduite";
const VERDICT_WITHHELD: &str = "En attente — au moins une donnée requise manque";

// ── issue #105 — the embedded charts' neutral labels (greyscale legend + zone bands) ──
const CHART_LEGEND: &str = "BPA (trait épais)   ·   Ventes (trait fin)   ·   Cours (tirets)   ·   projection (pointillés)   —   échelle propre par série (axe : BPA)";
const ZONE_LOW: &str = "Zone basse";
const ZONE_MID: &str = "Zone médiane";
const ZONE_HIGH: &str = "Zone haute";
const CURRENT_PRICE: &str = "Cours actuel";

#[cfg(test)]
const REPORT_USER_FACING: &[&str] = &[
    // Header.
    "Analyse de sélection de titre",
    "Titre :",
    "Monnaie :",
    "Décision :",
    // Section titles (all expanded).
    "1. Analyse visuelle (historique)",
    "2. Évaluation de la gestion",
    "3. Historique cours / bénéfice",
    "4. Risque et rendement",
    "5. Potentiel à 5 ans",
    "Synthèse",
    // Column headers.
    "Année",
    "Ventes",
    "BPA",
    "Cours haut",
    "Cours bas",
    "% BAII / ventes",
    "% rendement c.p.",
    "C/B haut",
    "C/B bas",
    "% distribution",
    "% rendement",
    // Static prose fragments + lines.
    "Croissance annualisée — ventes :",
    "Moyennes — % BAII/ventes :",
    "Moyennes — C/B haut :",
    "C/B bas :",
    "C/B moyen :",
    "Prévision — haute :",
    "basse :",
    "Zone basse :",
    "Zone médiane :",
    "Zone haute :",
    "Zones : — (prévision incomplète ou plage dégénérée)",
    "Position du cours actuel :",
    "Rapport hausse / baisse :",
    "Rendement présent :",
    "Appréciation projetée :",
    "Rendement annualisé total projeté :",
    "Position :",
    "Confiance réduite : moins d'années exploitables que le seuil de la méthode.",
    // Issue #105 — the embedded charts' labels.
    CHART_LEGEND,
    ZONE_LOW,
    ZONE_MID,
    ZONE_HIGH,
    CURRENT_PRICE,
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
struct Doc {
    pages: Vec<Content>,
    cur: Content,
    y: f32,        // top-origin cursor (distance from the page top)
    grid_top: f32, // the top of the grid table currently being drawn (issue #104)
}

impl Doc {
    fn new() -> Self {
        Doc {
            pages: Vec::new(),
            cur: new_page_content(),
            y: MARGIN,
            grid_top: MARGIN,
        }
    }

    /// Ensure `need` points of vertical space remain on the current page; else start a new one.
    fn ensure(&mut self, need: f32) {
        if PAGE_H - self.y - need < BOTTOM {
            let finished = std::mem::replace(&mut self.cur, new_page_content());
            self.pages.push(finished);
            self.y = MARGIN;
        }
    }

    /// Issue #104: reserve `need` points as ONE block so a heading + its table/chart never split
    /// across a page break (the §4 zone bar was orphaning). A no-op when the block already fits.
    fn keep_together(&mut self, need: f32) {
        self.ensure(need);
    }

    fn gap(&mut self, h: f32) {
        self.y += h;
    }

    fn title(&mut self, s: &str) {
        self.ensure(TITLE_FONT + 10.0); // reserve the true advance (font + rule + gap)
        self.y += TITLE_FONT;
        text(&mut self.cur, MARGIN, self.y, TITLE_FONT, s);
        self.y += 6.0;
        hline(&mut self.cur, MARGIN, PAGE_W - MARGIN, self.y, 1.0);
        self.y += 4.0;
    }

    fn section(&mut self, s: &str) {
        // Issue #104: a heading keeps room for its header + a few rows, so it never dangles alone at
        // a page foot with its table pushed to the next page.
        self.keep_together(HEAD_FONT + 5.0 * LINE_H);
        self.y += HEAD_FONT;
        text(&mut self.cur, MARGIN, self.y, HEAD_FONT, s);
        self.y += 4.0;
        hline(&mut self.cur, MARGIN, PAGE_W - MARGIN, self.y, 0.5);
        self.y += LINE_H - 4.0;
    }

    fn line(&mut self, s: &str) {
        self.ensure(LINE_H);
        self.y += FONT;
        text(&mut self.cur, MARGIN, self.y, FONT, s);
        self.y += LINE_H - FONT;
    }

    /// Issue #104 — start a boxed grid table: reserve `rows` × `LINE_H` (+ header) so the whole table
    /// stays on one page (a boxed table split mid-way would leave a dangling frame), and record its
    /// top so [`grid_end`] can draw the outer box + column rules.
    fn grid_begin(&mut self, rows: usize) {
        self.keep_together((rows as f32 + 1.0) * LINE_H + 4.0);
        self.grid_top = self.y;
    }

    /// One row of the current grid: each cell left-aligned inside its column (edges = column
    /// boundaries, len = cells + 1). A header row is underlined across the table width.
    fn grid_row(&mut self, cells: &[&str], edges: &[f32], head: bool) {
        self.y += FONT;
        for (i, s) in cells.iter().enumerate() {
            text(&mut self.cur, edges[i] + CELL_PAD, self.y, FONT, s);
        }
        self.y += LINE_H - FONT;
        if head {
            hline(&mut self.cur, edges[0], edges[edges.len() - 1], self.y - 1.5, 0.6);
        }
    }

    /// Close the grid: the outer box from [`grid_begin`]'s top to the current cursor + a vertical
    /// rule at each interior column boundary — the visible SSG grid (high-fidelity forms).
    fn grid_end(&mut self, edges: &[f32]) {
        let (top, bottom) = (self.grid_top - 1.0, self.y + 1.0);
        let left = edges[0];
        let right = edges[edges.len() - 1];
        stroke_rect(&mut self.cur, left, top, right - left, bottom - top, 0.6);
        for e in &edges[1..edges.len() - 1] {
            vline(&mut self.cur, *e, top, bottom, 0.4);
        }
        self.y = bottom + 1.0;
    }

    /// Issue #105 — the §1 semi-log growth chart. Sales / EPS / high-Price on ONE log scale (the
    /// classic SSG semilog view), plus the est-high / est-low EPS projection from the last EPS point
    /// to the forecast horizon. Greyscale-safe: the series are told apart by weight + dash (EPS thick
    /// solid, Sales thin solid, Price dashed, projection dotted), NEVER colour. Nothing is drawn when
    /// there is no plottable data (the tables already carry the em-dashes).
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
        let price = pts_of(&|cy| cy.high_price);
        let est_high = outputs.growth.estimated_high_eps.and_then(|d| d.to_f64());
        let est_low = outputs.growth.estimated_low_eps.and_then(|d| d.to_f64());

        if series.is_empty() || (sales.is_empty() && eps.is_empty() && price.is_empty()) {
            return;
        }

        self.ensure(CHART_H + 2.0 * LINE_H + 10.0);
        let top = self.y;
        let x0 = MARGIN + CHART_AXIS_W;
        let x1 = PAGE_W - MARGIN;
        let plot_w = x1 - x0;
        let n = series.len();
        let span = ((n as f64 - 1.0) + f64::from(FORECAST_HORIZON_YEARS)).max(1.0);
        let px = |i: f64| x0 + ((i / span) * f64::from(plot_w)) as f32;
        let py = |v: f64, lmin: f64, lmax: f64| {
            let t = ((v.max(1e-9).log10() - lmin) / (lmax - lmin)).clamp(0.0, 1.0);
            top + (f64::from(CHART_H) * (1.0 - t)) as f32
        };

        // Issue #25 (multi-scale): each series on its OWN log range so none is crushed by another's
        // magnitude. The EPS scale (the decision series + its projection) is the LABELLED one.
        let vals = |pts: &[(usize, f64)]| pts.iter().map(|p| p.1).collect::<Vec<f64>>();
        let sales_b = series_log_bounds(&vals(&sales));
        let price_b = series_log_bounds(&vals(&price));
        let mut eps_scale_vals = vals(&eps);
        eps_scale_vals.extend(est_high.filter(|v| *v > 0.0));
        eps_scale_vals.extend(est_low.filter(|v| *v > 0.0));
        let eps_b = series_log_bounds(&eps_scale_vals);

        stroke_rect(&mut self.cur, x0, top, plot_w, CHART_H, 0.6);
        // Gridlines + labels on the EPS scale (nice 1/2/5×10^k). Sales/Price live on their own scales
        // (shape/trend, not absolute height — the table carries the exact figures), told apart below.
        if let Some((lmin, lmax)) = eps_b {
            for (v, lbl) in nice_ticks(lmin, lmax) {
                let gy = py(v, lmin, lmax);
                polyline(&mut self.cur, &[(x0, gy), (x1, gy)], 0.3, GRID_GRAY, &[]);
                text(&mut self.cur, MARGIN, gy + 2.5, 7.0, &lbl);
            }
        }
        // Each series on its own scale (greyscale: weight + dash).
        let draw = |cur: &mut Content, pts: &[(usize, f64)], b: Option<(f64, f64)>, w: f32, dash: &[f32]| {
            if let Some((lmin, lmax)) = b {
                let p: Vec<(f32, f32)> =
                    pts.iter().map(|(i, v)| (px(*i as f64), py(*v, lmin, lmax))).collect();
                polyline(cur, &p, w, SERIES_GRAY, dash);
            }
        };
        draw(&mut self.cur, &sales, sales_b, 0.8, &[]);
        draw(&mut self.cur, &price, price_b, 0.8, &[3.0, 2.0]);
        draw(&mut self.cur, &eps, eps_b, 1.6, &[]);
        // Projection from the last EPS point to est-high / est-low at the horizon (dotted), EPS scale.
        if let (Some((lmin, lmax)), Some((li, lv))) = (eps_b, eps.last().copied()) {
            let (ox, oy) = (px(li as f64), py(lv, lmin, lmax));
            if let Some(h) = est_high.filter(|v| *v > 0.0) {
                polyline(&mut self.cur, &[(ox, oy), (px(span), py(h, lmin, lmax))], 1.2, SERIES_GRAY, &[1.5, 2.0]);
            }
            if let Some(l) = est_low.filter(|v| *v > 0.0) {
                polyline(&mut self.cur, &[(ox, oy), (px(span), py(l, lmin, lmax))], 1.0, SERIES_GRAY, &[1.5, 2.0]);
            }
        }
        // Issue #104 — year labels along the x-axis (each historical year under its column).
        for (i, cy) in series.iter().enumerate() {
            text_centered(&mut self.cur, px(i as f64), top + CHART_H + 9.0, 6.5, &cy.year.to_string());
        }
        self.y = top + CHART_H + 13.0;
        self.line(CHART_LEGEND);
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
        let (x0, x1) = (MARGIN, PAGE_W - MARGIN);
        let w = x1 - x0;
        let top = self.y + 10.0; // room above for the current-price marker label
        let fx = |price: f64| x0 + (((price - lo) / (hi - lo)).clamp(0.0, 1.0) * f64::from(w)) as f32;

        // Three thirds, greyscale shades (low third lightest, high third darkest).
        fill_rect(&mut self.cur, x0, top, fx(buy) - x0, ZONEBAR_H, 0.86);
        fill_rect(&mut self.cur, fx(buy), top, fx(neu) - fx(buy), ZONEBAR_H, 0.70);
        fill_rect(&mut self.cur, fx(neu), top, x1 - fx(neu), ZONEBAR_H, 0.52);
        stroke_rect(&mut self.cur, x0, top, w, ZONEBAR_H, 0.5);

        // Zone labels centered in each third.
        let mid_y = top + ZONEBAR_H / 2.0 + 3.0;
        text_centered(&mut self.cur, (x0 + fx(buy)) / 2.0, mid_y, 8.0, ZONE_LOW);
        text_centered(&mut self.cur, (fx(buy) + fx(neu)) / 2.0, mid_y, 8.0, ZONE_MID);
        text_centered(&mut self.cur, (fx(neu) + x1) / 2.0, mid_y, 8.0, ZONE_HIGH);

        // Boundary prices under the bar.
        let by = top + ZONEBAR_H + 9.0;
        text(&mut self.cur, x0, by, 7.0, &money(Some(z.forecast_low)));
        text_centered(&mut self.cur, fx(buy), by, 7.0, &money(Some(z.buy_top)));
        text_centered(&mut self.cur, fx(neu), by, 7.0, &money(Some(z.neutral_top)));
        let hi_lbl = money(Some(z.forecast_high));
        text(
            &mut self.cur,
            x1 - hi_lbl.chars().count() as f32 * 7.0 * 0.5,
            by,
            7.0,
            &hi_lbl,
        );

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
    fn footer(content: &mut Content) {
        text(
            content,
            MARGIN,
            PAGE_H - MARGIN,
            8.0,
            "Outil éducatif — ne constitue pas un conseil financier.",
        );
    }

    fn finish(mut self) -> Vec<u8> {
        // Close the page in progress.
        let last = std::mem::replace(&mut self.cur, Content::new());
        self.pages.push(last);

        let mut pdf = Pdf::new();
        let catalog = Ref::new(1);
        let tree = Ref::new(2);
        let font = Ref::new(3);
        // Two refs per page (page object + its content stream), after the three fixed refs.
        let page_ids: Vec<Ref> = (0..self.pages.len())
            .map(|i| Ref::new(4 + 2 * i as i32))
            .collect();
        let content_ids: Vec<Ref> = (0..self.pages.len())
            .map(|i| Ref::new(5 + 2 * i as i32))
            .collect();

        pdf.catalog(catalog).pages(tree);
        pdf.pages(tree)
            .kids(page_ids.iter().copied())
            .count(self.pages.len() as i32);
        // Helvetica (standard-14, no embedding) with WinAnsi so French accents render.
        pdf.type1_font(font)
            .base_font(Name(b"Helvetica"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));

        for (i, mut content) in self.pages.into_iter().enumerate() {
            Doc::footer(&mut content);
            {
                let mut page = pdf.page(page_ids[i]);
                page.parent(tree)
                    .media_box(Rect::new(0.0, 0.0, PAGE_W, PAGE_H))
                    .contents(content_ids[i]);
                page.resources().fonts().pair(Name(b"F0"), font);
            }
            pdf.stream(content_ids[i], &content.finish());
        }
        pdf.finish()
    }
}

fn new_page_content() -> Content {
    let mut c = Content::new();
    c.set_fill_gray(0.0); // black text
    c.set_stroke_gray(0.35); // mid-grey rules (greyscale only)
    c
}

/// Place `s` at top-origin `(x, top_y)` in Helvetica `size`, encoded as WinAnsi so French accents
/// render. PDF's origin is bottom-left, so the y is flipped here.
fn text(content: &mut Content, x: f32, top_y: f32, size: f32, s: &str) {
    content.begin_text();
    content.set_font(Name(b"F0"), size);
    content.set_text_matrix([1.0, 0.0, 0.0, 1.0, x, PAGE_H - top_y]);
    let bytes = winansi(s);
    content.show(Str(&bytes));
    content.end_text();
}

/// A horizontal rule at top-origin `top_y`, in mid-grey.
fn hline(content: &mut Content, x1: f32, x2: f32, top_y: f32, width: f32) {
    let y = PAGE_H - top_y;
    content.set_line_width(width);
    content.move_to(x1, y);
    content.line_to(x2, y);
    content.stroke();
}

/// A vertical rule between top-origin `top_y1` and `top_y2` (issue #104 — grid column separators).
fn vline(content: &mut Content, x: f32, top_y1: f32, top_y2: f32, width: f32) {
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
fn stroke_rect(content: &mut Content, x: f32, top_y: f32, w: f32, h: f32, width: f32) {
    content.set_line_width(width);
    content.rect(x, PAGE_H - top_y - h, w, h);
    content.stroke();
}

/// A grey-filled rectangle at top-origin `(x, top_y)`, size `w × h`. Restores the fill to black
/// (text) after — the greyscale zone fills are the only non-black fill in the document.
fn fill_rect(content: &mut Content, x: f32, top_y: f32, w: f32, h: f32, gray: f32) {
    content.set_fill_gray(gray);
    content.rect(x, PAGE_H - top_y - h, w, h);
    content.fill_nonzero();
    content.set_fill_gray(0.0);
}

/// Helvetica is ~0.5 em wide on average — enough to CENTER a short label at `cx` without embedding
/// font metrics (the labels are short and the box wide, so the estimate never overflows visibly).
fn text_centered(content: &mut Content, cx: f32, top_y: f32, size: f32, s: &str) {
    let w = s.chars().count() as f32 * size * 0.5;
    text(content, cx - w / 2.0, top_y, size, s);
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
            0x2019 => 0x92,            // ’ right single quote
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

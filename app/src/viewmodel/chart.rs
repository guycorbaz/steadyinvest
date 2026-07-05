//! The §1 interactive growth chart adapter (Story 2.8) — pure geometry, no decision math.
//!
//! Productionizes the GO'd Spike B (`docs/spikes/spike-b-native-slint-chart.md`): the native-Slint
//! semi-log chart is drawn from `Path` `commands` strings computed here against a viewbox whose
//! HEIGHT is rendered 1:1 (`CHART_H` logical px) while the width is free to stretch. Because the drag
//! is vertical only, a `TouchArea`'s `mouse-y` (px) equals the viewbox-y the mapping uses — the drag
//! inverse ([`value_for_y`]) is the exact mirror of the forward map ([`y_for`]).
//!
//! **Rendering ≠ the decision chain** (the spike's load-bearing rule): every pixel coordinate here is
//! `f32`/`f64`, but the only number the user *reads* (the est-high-EPS caption) is formatted from the
//! engine's `Decimal` via [`format_scaled`]. No zone / forecast / verdict math happens here (Cardinal
//! Rule) — the drag routes a new judgment value back through `core` (`engine::build_frame`), and this
//! module only maps the resulting series + forecast point to pixels. There are NO zones on the §1
//! chart (UX-DR10); the Buy/Neutral/Sell bands live in the §4 `ZoneBar`.

use rust_decimal::prelude::ToPrimitive;
use steadyinvest_contract::Money;
use steadyinvest_core::method::FORECAST_HORIZON_YEARS;

use crate::viewmodel::engine::StudyFrame;
use crate::viewmodel::format::{format_scaled, NumberFormat};
use crate::{AxisTick, GrowthChartState};
use steadyinvest_core::rounding::DisplayField;

/// The viewbox the chart is drawn (and dragged) against. The Slint `Path` scales this onto the
/// rendered element. The HEIGHT is rendered at exactly `CHART_H` logical px (so a vertical drag's
/// `mouse-y` maps directly to the viewbox-y); the WIDTH is free to stretch — the drag is vertical
/// only, so x-scaling does not affect the est-high-EPS the endpoint sets.
const CHART_W: f32 = 720.0;
const CHART_H: f32 = 420.0;
/// Fallback axis (log10 bounds) when a series has no plottable data — the faithful 1 → 200.
const AXIS_FALLBACK: (f64, f64) = (0.0, 2.30103); // (log10 1, log10 200)
/// Issue #25: each series is fit to its OWN log range spanning the data ± this padding (in DECADES),
/// so the curve fills the plot (no clamp, no compression) with a little breathing room top/bottom.
const RANGE_PAD_DECADES: f64 = 0.12;
/// …but a nearly-flat series is given at least this many decades of span, so a ~few-percent change is
/// NOT stretched to fill the whole height (honest: flat data reads flat, not dramatic).
const MIN_RANGE_DECADES: f64 = 0.6;
/// The 5–30 % growth-guide fan rates (UX-DR10), drawn from the last historical EPS point.
const FAN_RATES: [i32; 6] = [5, 10, 15, 20, 25, 30];
/// The per-share display scale the dragged est-high-EPS value snaps to (2 dp).
const EPS_DRAG_SCALE: u32 = 2;
/// Half-size (viewbox px) of the confront single-close marker box — drawn when there's exactly one
/// cached close, since a lone point can't form a visible polyline (Story 5.1).
const CONFRONT_MARKER: f32 = 4.0;

/// Issue #25: the log10 bounds `(lmin, lmax)` of the OWN scale for one series — the data's log range
/// padded by [`RANGE_PAD_DECADES`], widened to at least [`MIN_RANGE_DECADES`] so a flat series is not
/// stretched to fill. Empty / non-positive → the 1→200 fallback. Each series gets its own bounds so
/// none clamps off the top and none compresses to a sliver (the multi-scale fix).
fn series_bounds(values: impl IntoIterator<Item = f64>) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for v in values {
        if v.is_finite() && v > 0.0 {
            let l = v.log10();
            lo = lo.min(l);
            hi = hi.max(l);
        }
    }
    if !(lo.is_finite() && hi.is_finite()) {
        return AXIS_FALLBACK;
    }
    let mut span = hi - lo;
    if span < MIN_RANGE_DECADES {
        // Centre the flat data in the minimum span.
        let mid = (lo + hi) / 2.0;
        lo = mid - MIN_RANGE_DECADES / 2.0;
        hi = mid + MIN_RANGE_DECADES / 2.0;
        span = MIN_RANGE_DECADES;
    }
    let pad = span * RANGE_PAD_DECADES;
    (lo - pad, hi + pad)
}

/// Map a value to a viewbox-y (0 = top, `CHART_H` = bottom) on a log scale with log10 bounds
/// `lmin`/`lmax`. Clamped to the range so an off-scale figure pins to an edge.
pub fn y_for(value: f64, lmin: f64, lmax: f64) -> f32 {
    let lv = value.max(1e-12).log10().clamp(lmin, lmax);
    let t = (lv - lmin) / (lmax - lmin);
    (f64::from(CHART_H) * (1.0 - t)) as f32
}

/// Inverse of [`y_for`]: a viewbox-y (px) back to a value on the log scale `[lmin, lmax]`. `y` clamped
/// to `[0, CHART_H]` so a drag past an edge resolves to that edge's value.
pub fn value_for_y(y: f32, lmin: f64, lmax: f64) -> f64 {
    let t = (1.0 - (f64::from(y) / f64::from(CHART_H))).clamp(0.0, 1.0);
    10f64.powf(lmin + t * (lmax - lmin))
}

/// The judgment value a drag to viewbox-y `y` sets — the est-high-EPS on the EPS series' OWN scale
/// (`axis_min`/`axis_max`, read from the chart state), snapped to the per-share display scale so the
/// line and the exact-value field round-trip to the identical stored number.
pub fn judgment_value_for_y(y: f32, axis_min: f64, axis_max: f64) -> Money {
    let lmin = axis_min.max(1e-12).log10();
    let lmax = axis_max.max(axis_min * 1.0000001).log10();
    let value = value_for_y(y, lmin, lmax);
    // Always finite, so `from_f64_retain` is `Some`; fall back to 1 rather than `unwrap` (no panics).
    let dec = rust_decimal::Decimal::from_f64_retain(value)
        .unwrap_or(rust_decimal::Decimal::ONE)
        .round_dp(EPS_DRAG_SCALE);
    Money::from(dec)
}

/// A compact tick label for the EPS scale: plain up to 999, then `k / M / Md` (French) — data, not
/// prose (no `@tr`). The EPS scale is small ($ per share), so this is usually the plain number.
fn compact_axis_label(v: f64) -> String {
    let r = |x: f64| {
        // 2 significant-ish digits for a readable tick.
        if x >= 100.0 {
            format!("{}", x.round() as i64)
        } else if x >= 10.0 {
            format!("{:.0}", x)
        } else {
            format!("{:.1}", x)
        }
    };
    if v >= 1e9 {
        format!("{} Md", (v / 1e9).round() as i64)
    } else if v >= 1e6 {
        format!("{} M", (v / 1e6).round() as i64)
    } else if v >= 1e3 {
        format!("{} k", (v / 1e3).round() as i64)
    } else {
        r(v)
    }
}

/// Build a Slint `Path` `commands` string ("M x y L x y …") for `(x_px, value)` points on the log
/// scale `[lmin, lmax]`. An empty slice → "" (renders nothing); a single point → a lone "M".
fn path_commands(points: &[(f32, f64)], lmin: f64, lmax: f64) -> String {
    let mut s = String::with_capacity(points.len() * 16);
    for (i, (x, value)) in points.iter().enumerate() {
        let y = y_for(*value, lmin, lmax);
        s.push_str(if i == 0 { "M " } else { "L " });
        s.push_str(&format!("{x:.1} {y:.1} "));
    }
    s
}

/// The x (viewbox px) of a year at `offset` units from the first plotted year, across a total span of
/// `(plotted_years - 1) + FORECAST_HORIZON_YEARS` units (so the forecast point lands at the right edge).
fn x_for(offset: f64, span: f64) -> f32 {
    if span <= 0.0 {
        return 0.0;
    }
    ((offset / span) * f64::from(CHART_W)) as f32
}

/// The §1 chart geometry for the open study's coherent [`StudyFrame`]. Issue #25 (multi-scale): the
/// canonical Sales / EPS / high-Price series are each plotted on their OWN log scale (so none clamps
/// off the top and none compresses to a sliver), the 5–30 % guide fan + the judgment TREND LINE on
/// the EPS scale (anchored, origin fixed, at the last historical EPS → the draggable forecast
/// endpoint). The labelled side axis is the EPS scale; Sales/Price are told apart by colour + legend.
///
/// The endpoint sits at the engine's est-high-EPS (`outputs().growth.estimated_high_eps`, the user's
/// direct value or the value derived from the growth-% they typed). When neither is set the line is
/// **unset** (`judgment_y = -1`) — the chart never auto-places it (FR33).
pub fn growth_chart(frame: &StudyFrame, format: NumberFormat) -> GrowthChartState {
    let years = &frame.series;
    let n = years.len();
    if n == 0 {
        return unavailable();
    }
    // Total horizontal span in "year units": the historical years plus the forecast horizon, so the
    // forecast point sits at the right edge (x == CHART_W).
    let span = (n as f64 - 1.0) + f64::from(FORECAST_HORIZON_YEARS);
    let x_hist = |i: usize| x_for(i as f64, span);

    // Per-series (x, value) points.
    let series_pts =
        |get: fn(&steadyinvest_core::normalize::CanonicalYear) -> Option<rust_decimal::Decimal>| {
            years
                .iter()
                .enumerate()
                .filter_map(|(i, cy)| get(cy).and_then(|d| d.to_f64()).map(|v| (x_hist(i), v)))
                .collect::<Vec<(f32, f64)>>()
        };
    let sales_pts = series_pts(|cy| cy.sales);
    let eps_pts = series_pts(|cy| cy.eps);
    let price_pts = series_pts(|cy| cy.high_price);

    // The projection / fan origin: the last historical year that has an EPS value.
    let last_eps = years
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, cy)| cy.eps.and_then(|d| d.to_f64()).map(|v| (x_hist(i), v)));
    let est_high = frame.snapshot.outputs().growth.estimated_high_eps;

    // ── Issue #25 (multi-scale): each series on its OWN log scale so none clamps off the top and none
    //    compresses. Sales/Price fit their own data; the EPS scale ALSO reserves the forecast headroom
    //    (the fan at 30 % + the est-high) so the projection + the drag live comfortably on it. ──
    let (s_lmin, s_lmax) = series_bounds(sales_pts.iter().map(|p| p.1));
    let (p_lmin, p_lmax) = series_bounds(price_pts.iter().map(|p| p.1));
    // The EPS scale is STABLE during a drag: it spans the historical EPS + a FIXED forecast headroom
    // (the 30 % fan endpoint), but NOT the live `est_high` (including it would shift the scale as you
    // drag, so the line would "flee" the cursor). A drag beyond the headroom clamps at the top.
    let eps_range_vals: Vec<f64> = eps_pts
        .iter()
        .map(|p| p.1)
        .chain(last_eps.map(|(_, ov)| ov * 1.30f64.powi(FORECAST_HORIZON_YEARS as i32)))
        .collect();
    let (e_lmin, e_lmax) = series_bounds(eps_range_vals);

    let sales_commands = path_commands(&sales_pts, s_lmin, s_lmax);
    let price_commands = path_commands(&price_pts, p_lmin, p_lmax);
    let eps_commands = path_commands(&eps_pts, e_lmin, e_lmax);

    // The judgment TREND LINE + grip, on the EPS scale (origin FIXED at the last historical EPS →
    // the draggable forecast endpoint). FR33: unset when there is no forecast (never auto-placed).
    let (judgment_x, judgment_y, judgment_label, judgment_commands) = match est_high {
        Some(dec) => {
            let value = dec.to_f64().unwrap_or(1.0);
            let label = format_scaled(dec, DisplayField::PerShare, format);
            let line = match last_eps {
                Some((ox, ov)) => path_commands(&[(ox, ov), (CHART_W, value)], e_lmin, e_lmax),
                None => String::new(),
            };
            (CHART_W, y_for(value, e_lmin, e_lmax), label, line)
        }
        None => (CHART_W, -1.0, String::new(), String::new()),
    };

    // The 5–30 % guide fan from the last historical EPS point, on the EPS scale — pure guide geometry.
    let fan_commands: Vec<slint::SharedString> = match last_eps {
        Some((ox, ov)) => FAN_RATES
            .iter()
            .map(|r| {
                let endpoint =
                    ov * (1.0 + f64::from(*r) / 100.0).powi(FORECAST_HORIZON_YEARS as i32);
                path_commands(&[(ox, ov), (CHART_W, endpoint)], e_lmin, e_lmax).into()
            })
            .collect(),
        None => Vec::new(),
    };

    // The labelled axis is the EPS scale (issue #25 option a): Sales/Price are told apart by colour +
    // legend (their scales differ). Nice 1/2/5×10^k ticks that fall inside the EPS range.
    let axis_ticks = eps_ticks(e_lmin, e_lmax);

    GrowthChartState {
        available: true,
        chart_w: CHART_W,
        chart_h: CHART_H,
        sales_commands: sales_commands.into(),
        eps_commands: eps_commands.into(),
        price_commands: price_commands.into(),
        judgment_commands: judgment_commands.into(),
        fan_commands: slint::ModelRc::new(slint::VecModel::from(fan_commands)),
        axis_ticks: slint::ModelRc::new(slint::VecModel::from(axis_ticks)),
        judgment_x,
        judgment_y,
        judgment_label: judgment_label.into(),
        axis_min: 10f64.powf(e_lmin) as f32,
        axis_max: 10f64.powf(e_lmax) as f32,
    }
}

/// Nice 1/2/5×10^k ticks that fall inside the EPS scale `[10^lmin, 10^lmax]` (issue #25). Positions
/// via [`y_for`] on the same bounds, so the labels line up with the EPS curve.
fn eps_ticks(lmin: f64, lmax: f64) -> Vec<AxisTick> {
    let (min, max) = (10f64.powf(lmin), 10f64.powf(lmax));
    let mut ticks = Vec::new();
    let k0 = min.log10().floor() as i32;
    let k1 = max.log10().ceil() as i32;
    for k in k0..=k1 {
        for m in [1.0, 2.0, 5.0] {
            let v = m * 10f64.powi(k);
            if v >= min && v <= max {
                ticks.push(AxisTick {
                    label: compact_axis_label(v).into(),
                    y: y_for(v, lmin, lmax),
                });
            }
        }
    }
    ticks
}

/// The calm "no chart" state (no plottable years OR a transient normalize failure) — the chart area
/// keeps its size (constant form geometry), but nothing is drawn and the judgment line is unset
/// (`-1`, never auto-placed).
pub fn unavailable() -> GrowthChartState {
    GrowthChartState {
        available: false,
        chart_w: CHART_W,
        chart_h: CHART_H,
        sales_commands: Default::default(),
        eps_commands: Default::default(),
        price_commands: Default::default(),
        judgment_commands: Default::default(),
        fan_commands: slint::ModelRc::new(slint::VecModel::<slint::SharedString>::from(Vec::new())),
        axis_ticks: slint::ModelRc::new(slint::VecModel::<AxisTick>::from(Vec::new())),
        judgment_x: CHART_W,
        judgment_y: -1.0,
        judgment_label: Default::default(),
        axis_min: 1.0,
        axis_max: 200.0,
    }
}

// ── Story 5.1 — the confront overlay geometry (recorded band vs actual trajectory) ──

use crate::state::ConfrontView;
use crate::ConfrontState;

/// Build the confront chart (Story 5.1, FR50): the security's **actual** close trajectory since the
/// decision as a polyline, overlaid on the study's **recorded** §4 forecast band (a shaded rectangle).
/// Both are `Path` `commands` strings on a shared **linear price** viewbox (NOT the §1 semi-log axis —
/// confront plots absolute prices). No zones (UX-DR10), neutral. `available == false` → an empty state.
pub fn confront_chart(view: &ConfrontView, format: NumberFormat) -> ConfrontState {
    if !view.available {
        return ConfrontState {
            available: false,
            chart_w: CHART_W,
            chart_h: CHART_H,
            actual_commands: Default::default(),
            band_commands: Default::default(),
            high_label: Default::default(),
            low_label: Default::default(),
            decision_date: view.decision_date.clone().into(),
        };
    }
    let high = view.forecast_high.and_then(|d| d.to_f64()).unwrap_or(0.0);
    let low = view.forecast_low.and_then(|d| d.to_f64()).unwrap_or(0.0);
    let actuals: Vec<f64> = view.actual.iter().filter_map(|(_, c)| c.to_f64()).collect();

    // Price range = the band plus the actual closes, with a 5 % margin so nothing pins to an edge.
    let mut lo = low.min(high);
    let mut hi = low.max(high);
    for v in &actuals {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    let margin = ((hi - lo).abs() * 0.05).max(0.01);
    lo -= margin;
    hi += margin;
    let span = (hi - lo).max(f64::MIN_POSITIVE);
    let y_of = |price: f64| -> f32 { (f64::from(CHART_H) * (1.0 - (price - lo) / span)) as f32 };

    // Actual trajectory. The x-axis is spread evenly by index (oldest → newest), NOT by calendar
    // time — closes plot equidistantly regardless of weekend/holiday gaps (a known MVP limitation;
    // no date ticks are drawn). A lone close can't form a polyline (a bare `MoveTo` renders nothing),
    // so the single-point case draws a small visible marker box at the right edge (the "now" anchor)
    // instead of an invisible line.
    let n = actuals.len();
    let actual_cmd = if n == 1 {
        let cx = CHART_W - 2.0 * CONFRONT_MARKER; // inset so the box can't clip the right edge
        let cy = y_of(actuals[0]);
        let m = CONFRONT_MARKER;
        format!(
            "M {l:.1} {t:.1} L {r:.1} {t:.1} L {r:.1} {b:.1} L {l:.1} {b:.1} Z",
            l = cx - m,
            r = cx + m,
            t = cy - m,
            b = cy + m,
        )
    } else {
        let denom = (n - 1) as f64;
        let mut cmd = String::with_capacity(n * 16);
        for (i, v) in actuals.iter().enumerate() {
            let x = ((i as f64 / denom) * f64::from(CHART_W)) as f32;
            let y = y_of(*v);
            cmd.push_str(if i == 0 { "M " } else { "L " });
            cmd.push_str(&format!("{x:.1} {y:.1} "));
        }
        cmd
    };

    // Recorded band: a filled rectangle from forecast_high (top) to forecast_low (bottom).
    let hy = y_of(high.max(low));
    let ly = y_of(high.min(low));
    let band_cmd = format!(
        "M 0 {hy:.1} L {w:.1} {hy:.1} L {w:.1} {ly:.1} L 0 {ly:.1} Z",
        w = CHART_W
    );

    let label = |d: rust_decimal::Decimal| format_scaled(d, DisplayField::Price, format);
    ConfrontState {
        available: true,
        chart_w: CHART_W,
        chart_h: CHART_H,
        actual_commands: actual_cmd.into(),
        band_commands: band_cmd.into(),
        high_label: view.forecast_high.map(label).unwrap_or_default().into(),
        low_label: view.forecast_low.map(label).unwrap_or_default().into(),
        decision_date: view.decision_date.clone().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewmodel::engine::build_frame;
    use slint::Model;
    use steadyinvest_contract::{
        Cell, Coverage, ForecastLowOption, Freshness, Judgment, Provenance, Review, Source,
        Timestamp, YearData,
    };
    use uuid::Uuid;

    fn money(s: &str) -> Money {
        Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
    }

    fn prov() -> Provenance {
        Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: Timestamp("2026-03-09T00:00:00Z".to_string()),
            hash_of_dependencies: "manual".to_string(),
        }
    }

    fn cell(value: &str) -> Cell {
        Cell {
            value: Some(money(value)),
            source: Source::Manual,
            freshness: Freshness::Current,
            review: Review::Validated,
            coverage: Coverage::Present,
            provenance: prov(),
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

    fn judgment(est_high_eps: Option<Money>) -> Judgment {
        Judgment {
            estimated_high_eps: est_high_eps,
            estimated_low_eps: Some(money("3")),
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: Some(money("20")),
            judged_avg_low_pe: Some(money("10")),
            forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
            recent_severe_low: None,
            current_price: Some(money("60")),
            present_full_year_dividend: Some(money("2")),
        }
    }

    fn study(years: Vec<YearData>, judgment: Judgment) -> steadyinvest_contract::Study {
        let mut s = steadyinvest_contract::Study::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "NESN",
            "CHF",
            judgment,
            Timestamp("2026-06-14T09:30:00Z".to_string()),
        );
        s.years = years;
        s
    }

    /// The forward/inverse axis maps are mutual inverses to within the float tolerance, and both
    /// clamp at the axis bounds (a drag past an edge resolves to that edge, never beyond 1→200).
    #[test]
    fn axis_maps_round_trip_and_clamp_at_bounds() {
        let (lmin, lmax) = (1.0f64.log10(), 200.0f64.log10());
        for value in [1.0, 2.0, 8.0, 37.5, 150.0, 200.0] {
            let back = value_for_y(y_for(value, lmin, lmax), lmin, lmax);
            assert!(
                (back - value).abs() < 0.01,
                "value_for_y(y_for({value})) must round-trip, got {back}"
            );
        }
        // Bounds: top of the box → the range max, bottom → the range min.
        assert!(
            (value_for_y(0.0, lmin, lmax) - 200.0).abs() < 0.001,
            "top maps to the max"
        );
        assert!(
            (value_for_y(CHART_H, lmin, lmax) - 1.0).abs() < 0.001,
            "bottom maps to the min"
        );
        // Off-range values pin to an edge in the forward map.
        assert_eq!(
            y_for(0.5, lmin, lmax),
            y_for(1.0, lmin, lmax),
            "below-range clamps to the low edge"
        );
        assert_eq!(
            y_for(500.0, lmin, lmax),
            y_for(200.0, lmin, lmax),
            "above-range clamps to the high edge"
        );
        // A drag past the bottom edge resolves to the range floor, not beyond it.
        assert!(value_for_y(CHART_H + 50.0, lmin, lmax) >= 1.0 - 0.001);
    }

    /// `judgment_value_for_y` snaps to 2 dp and mirrors the EPS scale: the top of the box is the max.
    #[test]
    fn judgment_value_snaps_to_two_decimals() {
        let top = judgment_value_for_y(0.0, 1.0, 200.0);
        assert_eq!(top.as_decimal().scale(), EPS_DRAG_SCALE, "snapped to 2 dp");
        assert!(
            (top.as_decimal().to_f64().unwrap() - 200.0).abs() < 0.5,
            "the top of the chart maps to ~the scale max"
        );
    }

    /// `path_commands` emits one M then L's, with the y mapped through the given log range.
    #[test]
    fn path_commands_emits_move_then_lines() {
        let (lmin, lmax) = (1.0f64.log10(), 200.0f64.log10());
        let cmds = path_commands(&[(0.0, 1.0), (300.0, 200.0)], lmin, lmax);
        assert!(
            cmds.starts_with("M 0.0 "),
            "first point is a move, got {cmds}"
        );
        assert!(
            cmds.contains("L 300.0 "),
            "second point is a line, got {cmds}"
        );
        // y of the range min is the bottom (CHART_H), the range max the top (0).
        assert!(cmds.contains(&format!("M 0.0 {:.1}", CHART_H)));
        assert!(cmds.contains("L 300.0 0.0"));
        assert_eq!(
            path_commands(&[], lmin, lmax),
            "",
            "no points → empty commands"
        );
    }

    /// A populated study yields drawable series, the 5–30 % fan (six lines), the decade ticks, and a
    /// judgment line placed at the forecast est-high-EPS.
    #[test]
    fn growth_chart_plots_series_fan_ticks_and_a_judgment_line() {
        let years = (2021..=2025)
            .enumerate()
            .map(|(i, y)| year(y, &format!("{}", 4 + i)))
            .collect();
        let s = study(years, judgment(Some(money("9"))));
        let frame = build_frame(&s).expect("normalizes");
        let chart = growth_chart(&frame, NumberFormat::Comma);

        assert!(chart.available);
        assert!(!chart.eps_commands.is_empty(), "EPS series is drawn");
        assert!(!chart.sales_commands.is_empty(), "Sales series is drawn");
        assert!(!chart.price_commands.is_empty(), "Price series is drawn");
        assert_eq!(chart.fan_commands.row_count(), FAN_RATES.len());
        assert!(
            chart.axis_ticks.row_count() >= 1,
            "the EPS scale carries nice 1/2/5×10^k ticks (issue #25)"
        );
        assert!(chart.judgment_y >= 0.0, "the judgment line is placed");
        assert!(
            !chart.judgment_commands.is_empty(),
            "the judgment trend line is drawn from the last historical EPS to the forecast endpoint"
        );
        assert!(
            chart.judgment_commands.starts_with("M ")
                && chart.judgment_commands.matches("M ").count() == 1,
            "the trend line is one solid stroke (origin → endpoint), got {}",
            chart.judgment_commands
        );
        // The endpoint carries the grip handle at the right edge of the viewbox.
        assert_eq!(
            chart.judgment_x, CHART_W,
            "the endpoint sits at the future edge"
        );
        assert!(
            !chart.judgment_label.is_empty(),
            "the est-high-EPS caption is shown"
        );
        // The judgment line sits at the forecast EPS (9/share) on the chart's EPS scale.
        let (e_lmin, e_lmax) = (
            (chart.axis_min as f64).log10(),
            (chart.axis_max as f64).log10(),
        );
        assert!(
            (f64::from(chart.judgment_y) - f64::from(y_for(9.0, e_lmin, e_lmax))).abs() < 0.1,
            "the line is at y_for(9) on the EPS scale, got {}",
            chart.judgment_y
        );
    }

    /// FR33: with NO direct est-high-EPS and NO projected growth %, the engine derives no forecast →
    /// the chart leaves the judgment line UNSET (never auto-placed).
    #[test]
    fn growth_chart_never_auto_places_the_judgment_line() {
        let years = (2021..=2025).map(|y| year(y, "5")).collect();
        // Neither estimated_high_eps nor projected_eps_growth_pct → no forecast EPS.
        let s = study(years, judgment(None));
        let frame = build_frame(&s).expect("normalizes");
        let chart = growth_chart(&frame, NumberFormat::Comma);
        assert_eq!(chart.judgment_y, -1.0, "no forecast → no auto-placed line");
        assert!(chart.judgment_label.is_empty());
        assert!(chart.judgment_commands.is_empty());
    }

    /// The drag loop is self-consistent: a value dragged to viewbox-y `y`, applied to the study and
    /// re-mapped through the chart, returns the judgment line to (about) the same `y` — gesture →
    /// value → persist-equivalent → chart all agree (FR31; the line never jumps away from the cursor).
    #[test]
    fn drag_value_round_trips_through_persist_and_back_to_the_same_line() {
        let years = (2021..=2025).map(|y| year(y, "5")).collect();
        let mut s = study(years, judgment(None));
        for y in [40.0_f32, 120.0, 250.0] {
            // The EPS scale is stable (it excludes `est_high`), so read it from the current chart and
            // invert the drag against it — exactly what the drag wiring does (issue #25).
            let chart0 = growth_chart(&build_frame(&s).expect("normalizes"), NumberFormat::Comma);
            let value = judgment_value_for_y(y, chart0.axis_min as f64, chart0.axis_max as f64);
            crate::state::apply_judgment_field(&mut s.judgment, "est_high_eps", Some(value));
            let frame = build_frame(&s).expect("normalizes");
            let chart = growth_chart(&frame, NumberFormat::Comma);
            assert!(chart.judgment_y >= 0.0, "the line is now set");
            assert!(
                (chart.judgment_y - y.clamp(0.0, CHART_H)).abs() < 1.0,
                "the line returns to the dragged y {y}, got {}",
                chart.judgment_y
            );
        }
    }

    /// AC4 / FR12 on the LIVE drag path: dragging the EPS line on a study with a missing load-bearing
    /// input (current_price) recomputes a coherent frame whose §4 zone bar stays NON-full (withheld) —
    /// the recolor never paints full saturated colour beside a non-green input.
    #[test]
    fn dragging_never_paints_full_colour_when_a_load_bearing_input_is_missing() {
        let years = (2021..=2025).map(|y| year(y, "5")).collect();
        let mut j = judgment(Some(money("9")));
        j.current_price = None; // a load-bearing judgment input is Missing → Verdict::Withheld
        let mut s = study(years, j);
        crate::state::apply_judgment_field(
            &mut s.judgment,
            "est_high_eps",
            Some(judgment_value_for_y(80.0, 1.0, 200.0)),
        );
        let frame = build_frame(&s).expect("normalizes");
        let zone = crate::viewmodel::engine::zone_bar(&s, &frame.snapshot, NumberFormat::Comma);
        assert_eq!(
            zone.confidence.as_str(),
            "withheld",
            "a missing load-bearing input keeps the zone bar non-full during a drag (FR12)"
        );
    }

    // ── Story 5.1 — the confront overlay geometry ──

    fn confront_view(
        available: bool,
        high: Option<&str>,
        low: Option<&str>,
        closes: &[(&str, &str)],
    ) -> crate::state::ConfrontView {
        crate::state::ConfrontView {
            available,
            decision_date: "2026-03-09".to_string(),
            forecast_high: high.map(money).map(|m| m.as_decimal()),
            forecast_low: low.map(money).map(|m| m.as_decimal()),
            horizon_years: steadyinvest_core::method::FORECAST_HORIZON_YEARS,
            actual: closes
                .iter()
                .map(|(d, c)| (d.to_string(), money(c).as_decimal()))
                .collect(),
        }
    }

    #[test]
    fn confront_chart_unavailable_is_a_calm_empty_state() {
        let view = confront_view(false, None, None, &[]);
        let state = confront_chart(&view, NumberFormat::Comma);
        assert!(!state.available);
        assert!(state.actual_commands.is_empty(), "no trajectory is drawn");
        assert!(state.band_commands.is_empty(), "no band is drawn");
        assert!(
            state.chart_w > 0.0 && state.chart_h > 0.0,
            "geometry is constant"
        );
        assert_eq!(state.decision_date.as_str(), "2026-03-09");
    }

    #[test]
    fn confront_chart_draws_a_polyline_and_band_for_multiple_closes() {
        let view = confront_view(
            true,
            Some("120"),
            Some("80"),
            &[
                ("2026-03-10", "100"),
                ("2026-04-10", "108"),
                ("2026-05-10", "95"),
            ],
        );
        let state = confront_chart(&view, NumberFormat::Comma);
        assert!(state.available);
        assert!(
            state.actual_commands.starts_with("M ") && state.actual_commands.contains("L "),
            "three closes form a polyline (one MoveTo then LineTos), got {}",
            state.actual_commands
        );
        assert!(
            !state.band_commands.is_empty(),
            "the recorded band is drawn"
        );
        assert_eq!(state.high_label.as_str(), "120");
        assert_eq!(state.low_label.as_str(), "80");
    }

    /// Review fix (MED): a lone cached close must render a VISIBLE marker, not a bare `MoveTo` (which
    /// Slint draws as nothing). This is the common case — one refresh → one close.
    #[test]
    fn confront_chart_single_close_draws_a_visible_marker_not_a_bare_move() {
        let view = confront_view(true, Some("120"), Some("80"), &[("2026-03-10", "100")]);
        let state = confront_chart(&view, NumberFormat::Comma);
        assert!(state.available);
        assert!(
            state.actual_commands.contains('Z') && state.actual_commands.contains("L "),
            "a single close draws a closed marker box (not an invisible lone MoveTo), got {}",
            state.actual_commands
        );
    }

    /// An empty study still yields constant geometry (the chart area keeps its size) but draws nothing.
    #[test]
    fn growth_chart_empty_study_is_calm_and_unset() {
        let s = study(vec![], judgment(Some(money("9"))));
        let frame = build_frame(&s).expect("normalizes");
        let chart = growth_chart(&frame, NumberFormat::Comma);
        assert!(!chart.available);
        assert!(
            chart.chart_w > 0.0 && chart.chart_h > 0.0,
            "geometry is constant"
        );
        assert_eq!(chart.judgment_y, -1.0);
        assert!(chart.eps_commands.is_empty());
    }
}

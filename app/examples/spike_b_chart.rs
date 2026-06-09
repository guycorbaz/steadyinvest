//! Story 1.5 — Spike B (THROWAWAY). Native-Slint semi-log chart with a draggable judgment line and
//! a Buy/Neutral/Sell zone bar that recolours live. The deliverable is a GO/NO-GO decision (see
//! `docs/spikes/spike-b-native-slint-chart.md`), NOT production code. The real chart is Story 2.8.
//!
//! Run: `cargo run -p steadyinvest-app --example spike_b_chart` (needs a display) or `just spike`.
//! Each drag logs the **recompute latency** (µs) to stderr; the perceived <100 ms click-to-pixel
//! feel is judged visually on the target hardware.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::time::Instant;

slint::slint! {
    export component SpikeWindow inherits Window {
        title: "steadyinvest — Spike B (native Slint chart)";
        preferred-width: 760px;
        preferred-height: 540px;
        background: #0e0f12;

        in property <length> chart-w;
        in property <length> chart-h;
        in property <string> sales-commands;
        in property <string> eps-commands;
        in property <string> price-commands;
        in property <length> judgment-y;
        in property <string> signal-text;
        in property <color> buy-bg;
        in property <color> hold-bg;
        in property <color> sell-bg;
        in property <length> marker-y;
        callback judgment-dragged(length);

        VerticalLayout {
            padding: 12px;
            spacing: 8px;

            Text {
                text: "Spike B — drag the white judgment line; the zone bar recolours live.";
                color: #eceef2;
                font-size: 14px;
            }

            HorizontalLayout {
                spacing: 12px;
                alignment: start;

                // ── Semi-log growth chart (native Slint Path) ──
                Rectangle {
                    width: root.chart-w;
                    height: root.chart-h;
                    background: #16181d;
                    border-color: #2a2e37;
                    border-width: 1px;

                    Path {
                        commands: root.sales-commands;
                        stroke: #b8bdc7;
                        stroke-width: 2px;
                        viewbox-width: root.chart-w / 1px;
                        viewbox-height: root.chart-h / 1px;
                    }
                    Path {
                        commands: root.eps-commands;
                        stroke: #6da3ff;
                        stroke-width: 2px;
                        viewbox-width: root.chart-w / 1px;
                        viewbox-height: root.chart-h / 1px;
                    }
                    Path {
                        commands: root.price-commands;
                        stroke: #ecEEf2;
                        stroke-width: 1px;
                        viewbox-width: root.chart-w / 1px;
                        viewbox-height: root.chart-h / 1px;
                    }

                    // Draggable judgment line.
                    Rectangle {
                        x: 0;
                        y: root.judgment-y;
                        width: parent.width;
                        height: 2px;
                        background: #ffffff;
                    }

                    TouchArea {
                        moved => {
                            root.judgment-dragged(self.mouse-y);
                        }
                    }
                }

                // ── §4 zone bar (Buy/Neutral/Sell thirds) ──
                Rectangle {
                    width: 64px;
                    height: root.chart-h;
                    VerticalLayout {
                        Rectangle { background: root.sell-bg; }
                        Rectangle { background: root.hold-bg; }
                        Rectangle { background: root.buy-bg; }
                    }
                    // present-price marker
                    Rectangle {
                        x: 0;
                        y: root.marker-y;
                        width: parent.width;
                        height: 2px;
                        background: #8a8f98;
                    }
                }
            }

            Text {
                text: root.signal-text;
                color: #b8bdc7;
                font-size: 13px;
                wrap: word-wrap;
            }
        }
    }
}

const CHART_W: f32 = 560.0;
const CHART_H: f32 = 380.0;
const AXIS_MIN: f64 = 1.0;
const AXIS_MAX: f64 = 200.0;

// Fixed (synthetic) study context for the spike.
const CURRENT_PRICE: f64 = 150.0;
const AVG_HIGH_PE: i64 = 20;
const AVG_LOW_PE: i64 = 14;
const EST_LOW_EPS: i64 = 5;

/// Map a value on the 1→200 log axis to a y pixel (0 = top).
fn y_for(value: f64) -> f32 {
    let lmin = AXIS_MIN.log10();
    let lmax = AXIS_MAX.log10();
    let t = (value.clamp(AXIS_MIN, AXIS_MAX).log10() - lmin) / (lmax - lmin);
    (CHART_H as f64 * (1.0 - t)) as f32
}

/// Inverse of [`y_for`]: a y pixel back to a value on the log axis.
fn value_for_y(y: f32) -> f64 {
    let lmin = AXIS_MIN.log10();
    let lmax = AXIS_MAX.log10();
    let t = (1.0 - (y as f64 / CHART_H as f64)).clamp(0.0, 1.0);
    10f64.powf(lmin + t * (lmax - lmin))
}

/// Build a Slint `Path` `commands` string ("M x y L x y …") for a value series across the width.
fn path_commands(series: &[f64]) -> String {
    let n = series.len().max(2);
    let mut s = String::new();
    for (i, v) in series.iter().enumerate() {
        let x = (i as f32) / ((n - 1) as f32) * CHART_W;
        let y = y_for(*v);
        s.push_str(if i == 0 { "M " } else { "L " });
        s.push_str(&format!("{x:.1} {y:.1} "));
    }
    s
}

/// Result of the live recompute on a judgment-line drag.
struct Recompute {
    signal_text: String,
    marker_y: f32,
    /// (buy, hold, sell) backgrounds — the active zone is full saturation, others dimmed.
    zone_bg: (slint::Color, slint::Color, slint::Color),
}

/// The exact-decimal signal recompute (mirrors the method-spec §4 thirds zoning). Uses
/// `rust_decimal` for the decision values; only pixel mapping uses floats.
fn recompute(est_high_eps_value: f64) -> Recompute {
    let est_high_eps = Decimal::from_f64_retain(est_high_eps_value)
        .unwrap_or_default()
        .round_dp(2);
    let avg_high_pe = Decimal::from(AVG_HIGH_PE);
    let avg_low_pe = Decimal::from(AVG_LOW_PE);
    let est_low_eps = Decimal::from(EST_LOW_EPS);
    let current = Decimal::from_f64_retain(CURRENT_PRICE).unwrap_or_default();

    let fc_high = (avg_high_pe * est_high_eps).round_dp(2);
    let fc_low = (avg_low_pe * est_low_eps).round_dp(2);
    let range = fc_high - fc_low;
    let third = if range > Decimal::ZERO {
        range / Decimal::from(3)
    } else {
        Decimal::ZERO
    };
    let buy_top = fc_low + third;
    let hold_top = fc_low + third + third;

    let (zone, buy, hold, sell) = if range <= Decimal::ZERO || current <= buy_top {
        ("Buy", true, false, false)
    } else if current <= hold_top {
        ("Neutral", false, true, false)
    } else {
        ("Sell", false, false, true)
    };

    // Upside/downside ratio (undefined when current is at/below the forecast low).
    let ud = if current > fc_low && fc_high > current {
        Some(((fc_high - current) / (current - fc_low)).round_dp(1))
    } else {
        None
    };

    // Present-price marker on the (linear) zone bar.
    let marker_y = if range > Decimal::ZERO {
        let frac = ((current - fc_low) / range)
            .to_f64()
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        (CHART_H as f64 * (1.0 - frac)) as f32
    } else {
        CHART_H
    };

    let okabe = |on: bool, r: u8, g: u8, b: u8| {
        if on {
            slint::Color::from_argb_u8(255, r, g, b)
        } else {
            slint::Color::from_argb_u8(70, r, g, b)
        }
    };
    let zone_bg = (
        okabe(buy, 0x00, 0x9E, 0x73),  // Buy  #009E73
        okabe(hold, 0xE6, 0x9F, 0x00), // Hold #E69F00
        okabe(sell, 0xD5, 0x5E, 0x00), // Sell #D55E00
    );

    let ud_text = ud
        .map(|v| v.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let signal_text = format!(
        "est. high EPS {est_high_eps} → forecast high {fc_high} / low {fc_low} · \
         price {CURRENT_PRICE} in {zone} zone · U/D {ud_text}"
    );

    Recompute {
        signal_text,
        marker_y,
        zone_bg,
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = SpikeWindow::new()?;

    // Static synthetic series (≈10 years), plotted on the shared log axis.
    let sales = [12.0, 14.5, 17.0, 20.0, 24.0, 28.0, 33.0, 39.0, 46.0, 55.0];
    let eps = [3.0, 3.6, 4.2, 4.8, 5.6, 6.4, 7.0, 7.0, 7.0, 8.9];
    let price = [
        40.0, 55.0, 60.0, 70.0, 95.0, 120.0, 110.0, 130.0, 145.0, 150.0,
    ];

    window.set_chart_w(CHART_W);
    window.set_chart_h(CHART_H);
    window.set_sales_commands(path_commands(&sales).into());
    window.set_eps_commands(path_commands(&eps).into());
    window.set_price_commands(path_commands(&price).into());

    // Initial judgment line at the last EPS point's projected level.
    let initial_eps = 9.0;
    window.set_judgment_y(y_for(initial_eps));
    apply(&window, initial_eps);

    let weak = window.as_weak();
    window.on_judgment_dragged(move |y| {
        let Some(w) = weak.upgrade() else { return };
        let clamped_y = y.clamp(0.0, CHART_H);
        let est_eps = value_for_y(clamped_y);

        let t0 = Instant::now();
        w.set_judgment_y(clamped_y);
        apply(&w, est_eps);
        let micros = t0.elapsed().as_micros();
        eprintln!("[spike-b] recompute+property-set: {micros} µs (est. high EPS {est_eps:.2})");
    });

    window.run()
}

/// Run the recompute for a given estimated high EPS and push the result into the window.
fn apply(window: &SpikeWindow, est_high_eps: f64) {
    let r = recompute(est_high_eps);
    window.set_signal_text(r.signal_text.into());
    window.set_marker_y(r.marker_y);
    window.set_buy_bg(r.zone_bg.0);
    window.set_hold_bg(r.zone_bg.1);
    window.set_sell_bg(r.zone_bg.2);
}

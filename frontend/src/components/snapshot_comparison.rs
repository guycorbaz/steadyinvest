//! Snapshot Comparison Cards — side-by-side analysis comparison with metric deltas.
//!
//! Displays two analysis snapshots side by side with directional delta indicators
//! between them. Used in the Analysis view when a past analysis is selected from
//! the History Timeline Sidebar.

use crate::components::history_timeline::{MetricDelta, TimelineEntry};
use leptos::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive valuation zone from upside/downside ratio.
/// Same logic as `compact_analysis_card.rs`.
fn derive_valuation_zone(ratio: Option<f64>) -> (&'static str, &'static str) {
    match ratio {
        Some(r) if r >= 3.0 => ("zone-dot zone-buy", "Buy"),
        Some(r) if r >= 1.0 => ("zone-dot zone-hold", "Hold"),
        Some(_) => ("zone-dot zone-sell", "Sell"),
        None => ("zone-dot zone-none", "\u{2014}"),
    }
}

/// Format an optional f64 as a percentage string.
fn fmt_pct(val: Option<f64>) -> String {
    val.map(|v| format!("{:.1}%", v))
        .unwrap_or_else(|| "\u{2014}".to_string())
}

/// Format an optional f64 with one decimal.
fn fmt_f1(val: Option<f64>) -> String {
    val.map(|v| format!("{:.1}", v))
        .unwrap_or_else(|| "\u{2014}".to_string())
}

/// Format an optional f64 as a price.
fn fmt_price(val: Option<f64>, currency: Option<&str>) -> String {
    match val {
        Some(v) => format!("{} {:.2}", currency.unwrap_or(""), v),
        None => "\u{2014}".to_string(),
    }
}

/// Render a delta value with directional indicator.
fn delta_view(delta: Option<f64>, is_pct: bool) -> impl IntoView {
    match delta {
        Some(d) if d > 0.001 => {
            let text = if is_pct {
                format!("+{:.1}% \u{25B2}", d)
            } else {
                format!("+{:.1} \u{25B2}", d)
            };
            view! { <span class="delta delta-up">{text}</span> }.into_any()
        }
        Some(d) if d < -0.001 => {
            let text = if is_pct {
                format!("{:.1}% \u{25BC}", d)
            } else {
                format!("{:.1} \u{25BC}", d)
            };
            view! { <span class="delta delta-down">{text}</span> }.into_any()
        }
        Some(_) => {
            view! { <span class="delta delta-flat">"\u{2014}"</span> }.into_any()
        }
        None => {
            view! { <span class="delta delta-na">"\u{2014}"</span> }.into_any()
        }
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// Side-by-side snapshot comparison cards with metric deltas.
#[component]
pub fn SnapshotComparison(
    /// The current (most recent) analysis entry.
    current: TimelineEntry,
    /// The selected past analysis entry.
    past: TimelineEntry,
    /// Metric delta between the past and current entry (if available).
    delta: Option<MetricDelta>,
    /// Optional chart image URL for the past analysis.
    past_chart_image_url: Option<String>,
) -> impl IntoView {
    let current_date = current.captured_at.chars().take(10).collect::<String>();
    let past_date = past.captured_at.chars().take(10).collect::<String>();

    let (cur_zone_class, cur_zone_text) = derive_valuation_zone(current.upside_downside_ratio);
    let (past_zone_class, past_zone_text) = derive_valuation_zone(past.upside_downside_ratio);

    // Build metric rows with deltas
    let sales_delta = delta.as_ref().and_then(|d| d.sales_cagr_delta);
    let eps_delta = delta.as_ref().and_then(|d| d.eps_cagr_delta);
    let high_pe_delta = delta.as_ref().and_then(|d| d.high_pe_delta);
    let low_pe_delta = delta.as_ref().and_then(|d| d.low_pe_delta);
    let price_delta = delta.as_ref().and_then(|d| d.price_delta);
    let ud_delta = delta.as_ref().and_then(|d| d.upside_downside_delta);

    view! {
        <div class="snapshot-comparison">
            <div class="comparison-card-pair">
                // Past analysis card (left)
                <div class="comparison-card comparison-card-past">
                    <div class="comparison-card-header">
                        <span class="comparison-card-label">"Past Analysis"</span>
                        <span class="comparison-card-date">{past_date}</span>
                    </div>
                    {past_chart_image_url.map(|url| view! {
                        <div class="comparison-chart-thumb">
                            <img
                                src=url
                                alt="Past analysis chart"
                                class="chart-thumbnail"
                                on:error=|ev| {
                                    // Hide broken image if chart-image returns 404
                                    if let Some(parent) = leptos::prelude::event_target::<web_sys::HtmlElement>(&ev)
                                        .parent_element() {
                                        let _ = parent.set_attribute("style", "display:none");
                                    }
                                }
                            />
                        </div>
                    })}
                    <div class="comparison-card-metrics">
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"Sales CAGR"</span>
                            <span class="comp-metric-value">{fmt_pct(past.projected_sales_cagr)}</span>
                        </div>
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"EPS CAGR"</span>
                            <span class="comp-metric-value">{fmt_pct(past.projected_eps_cagr)}</span>
                        </div>
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"High P/E"</span>
                            <span class="comp-metric-value">{fmt_f1(past.projected_high_pe)}</span>
                        </div>
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"Low P/E"</span>
                            <span class="comp-metric-value">{fmt_f1(past.projected_low_pe)}</span>
                        </div>
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"Zone"</span>
                            <span class="comp-metric-value">
                                <span class=past_zone_class></span>
                                {past_zone_text}
                            </span>
                        </div>
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"U/D"</span>
                            <span class="comp-metric-value">{fmt_f1(past.upside_downside_ratio)}</span>
                        </div>
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"Price"</span>
                            <span class="comp-metric-value">{fmt_price(past.current_price, past.native_currency.as_deref())}</span>
                        </div>
                    </div>
                </div>

                // Delta column (center)
                <div class="comparison-deltas">
                    <div class="delta-header">"\u{0394}"</div>
                    <div class="delta-row">{delta_view(sales_delta, true)}</div>
                    <div class="delta-row">{delta_view(eps_delta, true)}</div>
                    <div class="delta-row">{delta_view(high_pe_delta, false)}</div>
                    <div class="delta-row">{delta_view(low_pe_delta, false)}</div>
                    <div class="delta-row"></div> // zone (no delta)
                    <div class="delta-row">{delta_view(ud_delta, false)}</div>
                    <div class="delta-row">{delta_view(price_delta, false)}</div>
                </div>

                // Current analysis card (right)
                <div class="comparison-card comparison-card-current">
                    <div class="comparison-card-header">
                        <span class="comparison-card-label">"Current Analysis"</span>
                        <span class="comparison-card-date">{current_date}</span>
                    </div>
                    <div class="comparison-card-metrics">
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"Sales CAGR"</span>
                            <span class="comp-metric-value">{fmt_pct(current.projected_sales_cagr)}</span>
                        </div>
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"EPS CAGR"</span>
                            <span class="comp-metric-value">{fmt_pct(current.projected_eps_cagr)}</span>
                        </div>
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"High P/E"</span>
                            <span class="comp-metric-value">{fmt_f1(current.projected_high_pe)}</span>
                        </div>
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"Low P/E"</span>
                            <span class="comp-metric-value">{fmt_f1(current.projected_low_pe)}</span>
                        </div>
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"Zone"</span>
                            <span class="comp-metric-value">
                                <span class=cur_zone_class></span>
                                {cur_zone_text}
                            </span>
                        </div>
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"U/D"</span>
                            <span class="comp-metric-value">{fmt_f1(current.upside_downside_ratio)}</span>
                        </div>
                        <div class="comp-metric-row">
                            <span class="comp-metric-label">"Price"</span>
                            <span class="comp-metric-value">{fmt_price(current.current_price, current.native_currency.as_deref())}</span>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

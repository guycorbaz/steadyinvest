//! Compact Analysis Card — condensed snapshot summary for Library and Comparison views.
//!
//! Displays ticker symbol, date, lock status, and key projected metrics
//! in a dense card layout following the Institutional HUD design system.

use leptos::prelude::*;

/// Data needed to render a Compact Analysis Card.
#[derive(Debug, Clone)]
pub struct CompactCardData {
    pub id: i32,
    pub ticker_symbol: String,
    pub captured_at: String,
    pub thesis_locked: bool,
    pub projected_sales_cagr: Option<f64>,
    pub projected_eps_cagr: Option<f64>,
    pub projected_high_pe: Option<f64>,
    pub projected_low_pe: Option<f64>,
}

/// Renders a compact summary card for a single analysis snapshot.
///
/// Used in the Library grid and (future) Comparison view.
/// Click triggers `on_click` with the snapshot ID for navigation.
#[component]
pub fn CompactAnalysisCard(
    data: CompactCardData,
    on_click: Callback<i32>,
) -> impl IntoView {
    let id = data.id;
    let lock_icon = if data.thesis_locked { "\u{1f512}" } else { "\u{1f513}" };
    let lock_class = if data.thesis_locked {
        "lock-badge locked"
    } else {
        "lock-badge unlocked"
    };

    let sales_cagr = data
        .projected_sales_cagr
        .map(|v| format!("{:.1}%", v))
        .unwrap_or_else(|| "—".to_string());
    let eps_cagr = data
        .projected_eps_cagr
        .map(|v| format!("{:.1}%", v))
        .unwrap_or_else(|| "—".to_string());
    let pe_range = match (data.projected_low_pe, data.projected_high_pe) {
        (Some(lo), Some(hi)) => format!("{:.1} — {:.1}", lo, hi),
        _ => "—".to_string(),
    };

    let aria = format!("Open analysis for {}", data.ticker_symbol);

    view! {
        <button
            class="compact-card"
            on:click=move |_| on_click.run(id)
            aria-label=aria
        >
            <div class="card-header">
                <span class="card-ticker">{data.ticker_symbol}</span>
                <span class=lock_class>{lock_icon}</span>
            </div>
            <div class="card-date">{data.captured_at}</div>
            <div class="card-metrics">
                <div class="metric-row">
                    <span class="metric-label">"Sales CAGR"</span>
                    <span class="metric-value">{sales_cagr}</span>
                </div>
                <div class="metric-row">
                    <span class="metric-label">"EPS CAGR"</span>
                    <span class="metric-value">{eps_cagr}</span>
                </div>
                <div class="metric-row">
                    <span class="metric-label">"PE Range"</span>
                    <span class="metric-value">{pe_range}</span>
                </div>
            </div>
        </button>
    }
}

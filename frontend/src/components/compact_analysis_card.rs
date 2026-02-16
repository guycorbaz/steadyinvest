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
    pub valuation_zone: Option<String>,
    pub upside_downside_ratio: Option<f64>,
    pub current_price: Option<f64>,
    pub target_high_price: Option<f64>,
    pub target_low_price: Option<f64>,
    pub display_currency: Option<String>,
}

/// Renders a compact summary card for a single analysis snapshot.
///
/// Used in the Library grid and Comparison view.
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
        .unwrap_or_else(|| "\u{2014}".to_string());
    let eps_cagr = data
        .projected_eps_cagr
        .map(|v| format!("{:.1}%", v))
        .unwrap_or_else(|| "\u{2014}".to_string());
    let pe_range = match (data.projected_low_pe, data.projected_high_pe) {
        (Some(lo), Some(hi)) => format!("{:.1} \u{2014} {:.1}", lo, hi),
        _ => "\u{2014}".to_string(),
    };

    // Valuation zone: colored dot + text
    let (zone_class, zone_text) = match data.valuation_zone.as_deref() {
        Some(z) => {
            let lower = z.to_lowercase();
            if lower.contains("under") || lower == "buy" {
                ("zone-dot zone-buy", "Buy")
            } else if lower.contains("over") || lower == "sell" {
                ("zone-dot zone-sell", "Sell")
            } else {
                ("zone-dot zone-hold", "Hold")
            }
        }
        None => ("zone-dot zone-none", "\u{2014}"),
    };

    // Upside/downside ratio: color-coded per NAIC 3:1 rule
    let (ud_class, ud_text) = match data.upside_downside_ratio {
        Some(r) if r >= 3.0 => ("ud-ratio ud-strong", format!("{:.1}", r)),
        Some(r) if r >= 1.0 => ("ud-ratio ud-marginal", format!("{:.1}", r)),
        Some(r) => ("ud-ratio ud-poor", format!("{:.1}", r)),
        None => ("ud-ratio ud-none", "\u{2014}".to_string()),
    };

    // Monetary values: current price + target range
    let currency_prefix = data
        .display_currency
        .as_deref()
        .unwrap_or("");
    let price_text = data
        .current_price
        .map(|p| format!("{} {:.2}", currency_prefix, p))
        .unwrap_or_else(|| "\u{2014}".to_string());
    let target_range = match (data.target_low_price, data.target_high_price) {
        (Some(lo), Some(hi)) => format!("{:.0} \u{2014} {:.0}", lo, hi),
        _ => "\u{2014}".to_string(),
    };
    let has_prices = data.current_price.is_some();

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
                <div class="metric-row">
                    <span class="metric-label">"Zone"</span>
                    <span class="metric-value">
                        <span class=zone_class></span>
                        {zone_text}
                    </span>
                </div>
                <div class="metric-row">
                    <span class="metric-label">"U/D"</span>
                    <span class=ud_class>{ud_text}</span>
                </div>
                {if has_prices {
                    view! {
                        <div class="metric-row metric-price-row">
                            <span class="metric-label">"Price"</span>
                            <span class="metric-value metric-price">{price_text}</span>
                        </div>
                        <div class="metric-row metric-price-row">
                            <span class="metric-label">"Target"</span>
                            <span class="metric-value metric-target">{target_range}</span>
                        </div>
                    }.into_any()
                } else {
                    view! {}.into_any()
                }}
            </div>
        </button>
    }
}

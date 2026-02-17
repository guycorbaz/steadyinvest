//! Evaluate Management component (NAIC Section 2).
//!
//! Renders a transposed table of management quality metrics per NAIC Figure 2.1:
//! rows = metrics (% Pre-Tax Profit on Sales, % Earned on Equity, % Debt to Capital),
//! columns = fiscal years + 5 Yr Avg + Trend indicator.

use leptos::prelude::*;
use steady_invest_logic::{HistoricalData, calculate_quality_analysis, TrendIndicator};

/// Transposed management quality table per NAIC Section 2.
///
/// Rows: % Pre-Tax Profit on Sales, % Earned on Equity, % Debt to Capital.
/// Columns: fiscal years (chronological) + 5-year average + overall trend arrow.
#[component]
pub fn QualityDashboard(data: HistoricalData) -> impl IntoView {
    let analysis = calculate_quality_analysis(&data);
    let pts = analysis.points;

    // Precompute all values before the view macro
    let year_headers: Vec<i32> = pts.iter().map(|p| p.year).collect();
    let profit_values: Vec<String> = pts.iter().map(|p| format!("{:.1}%", p.profit_on_sales)).collect();
    let roe_values: Vec<String> = pts.iter().map(|p| format!("{:.1}%", p.roe)).collect();
    let num_years = year_headers.len();

    // 5-year averages from the most recent 5 data points
    let (avg_profit, avg_roe) = if !pts.is_empty() {
        let last_5 = &pts[pts.len().saturating_sub(5)..];
        let ap = last_5.iter().map(|p| p.profit_on_sales).sum::<f64>() / last_5.len() as f64;
        let ar = last_5.iter().map(|p| p.roe).sum::<f64>() / last_5.len() as f64;
        (ap, ar)
    } else {
        (0.0, 0.0)
    };

    // Overall trend from the most recent data point
    let (profit_trend_class, profit_arrow) = pts.last()
        .map(|p| match p.profit_trend {
            TrendIndicator::Up => ("trend-up", "↑"),
            TrendIndicator::Down => ("trend-down", "↓"),
            TrendIndicator::Stable => ("trend-stable", "→"),
        })
        .unwrap_or(("trend-stable", "→"));

    let (roe_trend_class, roe_arrow) = pts.last()
        .map(|p| match p.roe_trend {
            TrendIndicator::Up => ("trend-up", "↑"),
            TrendIndicator::Down => ("trend-down", "↓"),
            TrendIndicator::Stable => ("trend-stable", "→"),
        })
        .unwrap_or(("trend-stable", "→"));

    view! {
        <div class="quality-dashboard">
            <div class="header-flex">
                <h3>"Evaluate Management"</h3>
                <span class="hud-subtitle">"% Earned on Equity & % Pre-Tax Profit on Sales"</span>
            </div>

            {if num_years == 0 {
                view! {
                    <p style="text-align: center; padding: 2rem; color: var(--text-secondary); font-style: italic;">
                        "No historical data available for quality analysis."
                    </p>
                }.into_any()
            } else {
                view! {
                    <div class="table-scroll-wrapper">
                        <table class="quality-grid">
                            <thead>
                                <tr>
                                    <th class="metric-col"></th>
                                    {year_headers.iter().map(|y| view! { <th>{*y}</th> }).collect_view()}
                                    <th class="summary-col">"5 Yr Avg"</th>
                                    <th class="summary-col">"Trend"</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr>
                                    <td class="metric-label">"% Pre-Tax Profit on Sales"</td>
                                    {profit_values.iter().map(|v| {
                                        view! { <td class="value-cell">{v.clone()}</td> }
                                    }).collect_view()}
                                    <td class="summary-col">{format!("{:.1}%", avg_profit)}</td>
                                    <td class=format!("summary-col trend-cell {}", profit_trend_class)>{profit_arrow}</td>
                                </tr>
                                <tr>
                                    <td class="metric-label">"% Earned on Equity"</td>
                                    {roe_values.iter().map(|v| {
                                        view! { <td class="value-cell">{v.clone()}</td> }
                                    }).collect_view()}
                                    <td class="summary-col">{format!("{:.1}%", avg_roe)}</td>
                                    <td class=format!("summary-col trend-cell {}", roe_trend_class)>{roe_arrow}</td>
                                </tr>
                                <tr class="debt-row">
                                    <td class="metric-label">"% Debt to Capital"</td>
                                    {(0..num_years).map(|_| {
                                        view! { <td class="value-cell na-cell">"N/A"</td> }
                                    }).collect_view()}
                                    <td class="summary-col na-cell">"N/A"</td>
                                    <td class="summary-col trend-cell trend-stable">"—"</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                }.into_any()
            }}
        </div>
    }
}

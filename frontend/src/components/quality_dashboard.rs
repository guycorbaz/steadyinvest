use leptos::prelude::*;
use naic_logic::{HistoricalData, calculate_quality_analysis, TrendIndicator};

#[component]
pub fn QualityDashboard(data: HistoricalData) -> impl IntoView {
    let analysis = move || calculate_quality_analysis(&data);

    view! {
        <div class="quality-dashboard">
            <div class="header-flex">
                <h3>"Quality Dashboard"</h3>
                <span class="hud-subtitle">"ROE & Profit on Sales Trends"</span>
            </div>
            
            <table class="quality-grid">
                <thead>
                    <tr>
                        <th>"Year"</th>
                        <th>"ROE %"</th>
                        <th>"Trend"</th>
                        <th>"Profit on Sales %"</th>
                        <th>"Trend"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || {
                        let pts = analysis().points;
                        if pts.is_empty() {
                            view! {
                                <tr>
                                    <td colspan="5" style="text-align: center; padding: 2rem; color: var(--text-secondary); font-style: italic;">
                                        "No historical data available for quality analysis."
                                    </td>
                                </tr>
                            }.into_any()
                        } else {
                            pts.into_iter().rev().map(|point| {
                                let roe_class = match point.roe_trend {
                                    TrendIndicator::Up => "trend-up",
                                    TrendIndicator::Down => "trend-down",
                                    TrendIndicator::Stable => "trend-stable",
                                };
                                let profit_class = match point.profit_trend {
                                    TrendIndicator::Up => "trend-up",
                                    TrendIndicator::Down => "trend-down",
                                    TrendIndicator::Stable => "trend-stable",
                                };

                                view! {
                                    <tr>
                                        <td class="year-cell">{point.year}</td>
                                        <td class=format!("value-cell {}", roe_class)>
                                            {format!("{:.1}%", point.roe)}
                                        </td>
                                        <td class=format!("trend-cell {}", roe_class)>
                                            {match point.roe_trend {
                                                TrendIndicator::Up => "↑",
                                                TrendIndicator::Down => "↓",
                                                TrendIndicator::Stable => "→",
                                            }}
                                        </td>
                                        <td class=format!("value-cell {}", profit_class)>
                                            {format!("{:.1}%", point.profit_on_sales)}
                                        </td>
                                        <td class=format!("trend-cell {}", profit_class)>
                                            {match point.profit_trend {
                                                TrendIndicator::Up => "↑",
                                                TrendIndicator::Down => "↓",
                                                TrendIndicator::Stable => "→",
                                            }}
                                        </td>
                                    </tr>
                                }
                            }).collect_view().into_any()
                        }
                    }}
                </tbody>
            </table>
        </div>
    }
}

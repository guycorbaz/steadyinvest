//! Read-only snapshot viewer.
//!
//! Renders a previously locked analysis in the same NAIC Figure 2.1 layout
//! as the live Analyst HUD, but with all controls disabled and projections frozen.
//! Includes "Save to File" and "Export PDF" actions.

use crate::components::ssg_chart::SSGChart;
use crate::components::quality_dashboard::QualityDashboard;
use crate::components::valuation_panel::ValuationPanel;
use leptos::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use steady_invest_logic::{TickerInfo, calculate_growth_analysis, project_forward};
use crate::types::LockedAnalysisModel;

/// Read-only view of a locked analysis snapshot (NAIC Figure 2.1 layout).
///
/// Layout: SSGChart → Fundamental Company Data → Evaluate Management → ValuationPanel.
/// All projections are frozen from the snapshot data.
#[component]
pub fn SnapshotHUD(
    ticker: TickerInfo,
    model: LockedAnalysisModel,
) -> impl IntoView {
    let snapshot = model.snapshot();
    let data = snapshot.historical_data;

    // Projections are fixed in snapshots
    let sales_projection_cagr = RwSignal::new(snapshot.projected_sales_cagr);
    let eps_projection_cagr = RwSignal::new(snapshot.projected_eps_cagr);
    let ptp_projection_cagr = RwSignal::new(snapshot.projected_ptp_cagr);
    let future_high_pe = RwSignal::new(snapshot.projected_high_pe);
    let future_low_pe = RwSignal::new(snapshot.projected_low_pe);

    // Precompute historical growth CAGRs for Fundamental Company Data table
    let raw_years: Vec<i32> = data.records.iter().map(|r| r.fiscal_year).collect();
    let sales_vals: Vec<f64> = data.records.iter().map(|r| r.sales.to_f64().unwrap_or(0.0)).collect();
    let eps_vals: Vec<f64> = data.records.iter().map(|r| r.eps.to_f64().unwrap_or(0.0)).collect();

    let sales_growth = calculate_growth_analysis(&raw_years, &sales_vals);
    let eps_growth = calculate_growth_analysis(&raw_years, &eps_vals);

    let ptp_valid: Vec<(i32, f64)> = data.records.iter()
        .filter_map(|r| r.pretax_income.map(|v| (r.fiscal_year, v.to_f64().unwrap_or(0.0))))
        .filter(|(_, v)| *v > 0.0)
        .collect();
    let ptp_years_vec: Vec<i32> = ptp_valid.iter().map(|(y, _)| *y).collect();
    let ptp_vals: Vec<f64> = ptp_valid.iter().map(|(_, v)| *v).collect();
    let ptp_growth = calculate_growth_analysis(&ptp_years_vec, &ptp_vals);

    let last_sales = sales_vals.last().copied().unwrap_or(0.0);
    let last_eps = eps_vals.last().copied().unwrap_or(0.0);
    let last_ptp = data.records.last()
        .and_then(|r| r.pretax_income)
        .map(|v| v.to_f64().unwrap_or(0.0))
        .unwrap_or(0.0);

    view! {
        <div class="data-ready snapshot-view">
            <div class="header-control-bar snapshot-header">
                <div class="title-group">
                    <span class="lock-icon">"🔒"</span>
                    <h2>"Historical Snapshot: " {ticker.name.clone()}</h2>
                </div>
                <div class="snapshot-meta">
                    "Captured on: " {model.captured_at.format("%Y-%m-%d %H:%M").to_string()}
                </div>
                <div class="header-actions">
                    <button
                        class="btn-secondary save-btn"
                        on:click={
                            let model = model.clone();
                            move |_| {
                                let snapshot = model.snapshot();
                                let _ = crate::persistence::save_snapshot(&snapshot);
                            }
                        }
                    >
                        <span class="btn-icon">"💾"</span>
                        "Save to File"
                    </button>
                    <button
                        class="btn-primary export-btn"
                        on:click={
                            let model = model.clone();
                            move |_| {
                                let id = model.id;
                                let url = format!("/api/analyses/export/{}", id);
                                let _ = window().location().set_href(&url);
                            }
                        }
                    >
                        <span class="btn-icon">"📄"</span>
                        "Export PDF"
                    </button>
                </div>
            </div>

            <div class="analyst-note-box standard-border">
                <h4>"Analyst Note"</h4>
                <div class="note-content">
                    {model.notes.clone().unwrap_or_default()}
                </div>
            </div>

            <div class="historical-indicator">
                "Note: You are viewing a historical snapshot. Projections and data are immutable."
            </div>

            // Section 1: Visual Analysis (SSG Chart)
            <SSGChart
                data=data.clone()
                sales_projection_cagr=sales_projection_cagr
                eps_projection_cagr=eps_projection_cagr
                ptp_projection_cagr=ptp_projection_cagr
            />

            // Section 1 continued: Fundamental Company Data table (read-only)
            <div class="fundamental-data-table standard-border read-only">
                <div class="header-flex">
                    <h4>"Fundamental Company Data"</h4>
                </div>
                <div class="table-scroll-wrapper">
                    <table>
                        <thead>
                            <tr>
                                <th class="metric-col"></th>
                                {data.records.iter().map(|r| {
                                    view! { <th>{r.fiscal_year}</th> }
                                }).collect_view()}
                                <th class="summary-col">"Growth %"</th>
                                <th class="summary-col">"Forecast %"</th>
                                <th class="summary-col">"5 Yr Est"</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr>
                                <td class="metric-label">"Historical Sales"</td>
                                {data.records.iter().map(|rec| {
                                    let ovr = rec.overrides.iter().find(|o| o.field_name == "sales").cloned();
                                    let is_overridden = ovr.is_some();
                                    view! {
                                        <td class=move || if is_overridden { "overridden-cell value-cell" } else { "value-cell" }>
                                            {rec.sales.to_string()}
                                            {if is_overridden { view! { <span class="override-mark">"*"</span> }.into_any() } else { ().into_any() }}
                                        </td>
                                    }
                                }).collect_view()}
                                <td class="summary-col">{format!("{:.1}%", sales_growth.cagr)}</td>
                                <td class="summary-col">{format!("{:.1}%", snapshot.projected_sales_cagr)}</td>
                                <td class="summary-col">{format!("{:.0}", project_forward(last_sales, snapshot.projected_sales_cagr, 5))}</td>
                            </tr>
                            <tr>
                                <td class="metric-label">"Historical Earnings"</td>
                                {data.records.iter().map(|rec| {
                                    let ovr = rec.overrides.iter().find(|o| o.field_name == "eps").cloned();
                                    let is_overridden = ovr.is_some();
                                    view! {
                                        <td class=move || if is_overridden { "overridden-cell value-cell" } else { "value-cell" }>
                                            {rec.eps.to_string()}
                                            {if is_overridden { view! { <span class="override-mark">"*"</span> }.into_any() } else { ().into_any() }}
                                        </td>
                                    }
                                }).collect_view()}
                                <td class="summary-col">{format!("{:.1}%", eps_growth.cagr)}</td>
                                <td class="summary-col">{format!("{:.1}%", snapshot.projected_eps_cagr)}</td>
                                <td class="summary-col">{format!("{:.2}", project_forward(last_eps, snapshot.projected_eps_cagr, 5))}</td>
                            </tr>
                            <tr>
                                <td class="metric-label">"Pre-Tax Profit"</td>
                                {data.records.iter().map(|rec| {
                                    let val_str = rec.pretax_income
                                        .map(|v| v.to_string())
                                        .unwrap_or_else(|| "—".to_string());
                                    view! { <td class="value-cell">{val_str}</td> }
                                }).collect_view()}
                                <td class="summary-col">{format!("{:.1}%", ptp_growth.cagr)}</td>
                                <td class="summary-col">{format!("{:.1}%", snapshot.projected_ptp_cagr)}</td>
                                <td class="summary-col">{
                                    if last_ptp > 0.0 {
                                        format!("{:.0}", project_forward(last_ptp, snapshot.projected_ptp_cagr, 5))
                                    } else {
                                        "—".to_string()
                                    }
                                }</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </div>

            // Section 2: Evaluate Management
            <QualityDashboard data=data.clone() />

            // Sections 3-5: P/E History, Risk & Reward, Five-Year Potential
            <ValuationPanel
                data=data.clone()
                projected_eps_cagr=eps_projection_cagr
                future_high_pe=future_high_pe
                future_low_pe=future_low_pe
            />
        </div>
    }
}

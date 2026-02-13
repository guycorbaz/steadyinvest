//! Read-only snapshot viewer.
//!
//! Renders a previously locked analysis in the same layout as the live
//! Analyst HUD, but with all controls disabled and projections frozen.
//! Includes "Save to File" and "Export PDF" actions.

use crate::components::ssg_chart::SSGChart;
use crate::components::quality_dashboard::QualityDashboard;
use crate::components::valuation_panel::ValuationPanel;
use leptos::prelude::*;
use steady_invest_logic::TickerInfo;
use crate::types::LockedAnalysisModel;

/// Read-only view of a locked analysis snapshot.
///
/// Displays the same chart, valuation panel, quality dashboard, and data grid
/// as the live HUD, but with frozen (non-interactive) projections.
///
/// # Props
///
/// * `ticker` — Identity info for the security.
/// * `model` — The locked analysis record including snapshot data.
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
    let future_high_pe = RwSignal::new(snapshot.projected_high_pe);
    let future_low_pe = RwSignal::new(snapshot.projected_low_pe);

    view! {
        <div class="data-ready snapshot-view">
            <div class="header-control-bar snapshot-header">
                <div class="title-group">
                    <span class="lock-icon">"🔒"</span>
                    <h2>"Historical Snapshot: " {ticker.name.clone()}</h2>
                </div>
                <div class="snapshot-meta">
                    "Captured on: " {model.created_at.format("%Y-%m-%d %H:%M").to_string()}
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
                    {model.analyst_note.clone()}
                </div>
            </div>

            <div class="historical-indicator">
                "Note: You are viewing a historical snapshot. Projections and data are immutable."
            </div>

            <SSGChart 
                data=data.clone() 
                sales_projection_cagr=sales_projection_cagr
                eps_projection_cagr=eps_projection_cagr
            />
            <ValuationPanel 
                data=data.clone()
                projected_eps_cagr=eps_projection_cagr
                future_high_pe=future_high_pe
                future_low_pe=future_low_pe
            />
            <QualityDashboard data=data.clone() />

            <div class="records-grid read-only">
                <table>
                    <thead>
                        <tr>
                             <th>"Year"</th>
                             <th>"Sales"</th>
                             <th>"EPS"</th>
                             <th>"High"</th>
                             <th>"Low"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {data.records.iter().map(|rec| {
                            let get_ovr = |field_name: &str| {
                                rec.overrides.iter().find(|o| o.field_name == field_name).cloned()
                            };

                            let render_cell = move |val: rust_decimal::Decimal, field: &str| {
                                let ovr = get_ovr(field);
                                let is_overridden = ovr.is_some();
                                
                                view! {
                                    <td class=move || if is_overridden { "overridden-cell" } else { "" }>
                                        {val.to_string()}
                                        {if is_overridden { view! { <span class="override-mark">"*"</span> }.into_any() } else { ().into_any() }}
                                    </td>
                                }
                            };

                            view! {
                                <tr>
                                    <td>{rec.fiscal_year}</td>
                                    {render_cell(rec.sales, "sales")}
                                    {render_cell(rec.eps, "eps")}
                                    {render_cell(rec.price_high, "price_high")}
                                    {render_cell(rec.price_low, "price_low")}
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
        </div>
    }
}

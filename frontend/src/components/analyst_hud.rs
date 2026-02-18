//! Main analysis workspace component.
//!
//! The Analyst HUD is the primary interactive view, combining the SSG chart,
//! Fundamental Company Data table, Evaluate Management dashboard, and valuation
//! panel into a single cohesive workspace per NAIC Figure 2.1 layout.

use crate::components::ssg_chart::SSGChart;
use crate::components::quality_dashboard::QualityDashboard;
use crate::components::valuation_panel::ValuationPanel;
use crate::components::override_modal::OverrideModal;
use crate::components::lock_thesis_modal::LockThesisModal;
use leptos::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use steady_invest_logic::{HistoricalData, TickerInfo, calculate_growth_analysis, project_forward};

/// Multi-panel analysis workspace for live data (NAIC Figure 2.1 layout).
///
/// Layout order: SSGChart → Fundamental Company Data → Evaluate Management → ValuationPanel.
/// Manages shared reactive signals for Sales/EPS/PTP CAGR and P/E projections.
#[component]
pub fn AnalystHUD(
    ticker: TickerInfo,
    data: HistoricalData,
    on_refetch: Callback<()>,
    on_locked: Callback<i32>,
) -> impl IntoView {
    // Shared projection signals for cross-component reactivity (AC 5)
    let sales_projection_cagr = RwSignal::new(0.0);
    let eps_projection_cagr = RwSignal::new(0.0);
    let ptp_projection_cagr = RwSignal::new(0.0);
    let future_high_pe = RwSignal::new(0.0);
    let future_low_pe = RwSignal::new(0.0);

    // Manual Override UI state
    #[derive(Clone, Debug)]
    struct ActiveOverride {
        year: i32,
        field: String,
        current_value: rust_decimal::Decimal,
        current_note: Option<String>,
    }
    let (active_override, set_active_override) = signal(Option::<ActiveOverride>::None);
    let (show_lock_modal, set_show_lock_modal) = signal(false);

    // Initialize P/E projections from historical averages
    Effect::new({
        let data = data.clone();
        move |_| {
            if let Some(pe) = &data.pe_range_analysis {
                future_high_pe.set(pe.avg_high_pe);
                future_low_pe.set(pe.avg_low_pe);
            }
        }
    });

    // Precompute historical growth CAGRs for Fundamental Company Data table
    let raw_years: Vec<i32> = data.records.iter().map(|r| r.fiscal_year).collect();
    let sales_vals: Vec<f64> = data.records.iter().map(|r| r.sales.to_f64().unwrap_or(0.0)).collect();
    let eps_vals: Vec<f64> = data.records.iter().map(|r| r.eps.to_f64().unwrap_or(0.0)).collect();

    let sales_growth = calculate_growth_analysis(&raw_years, &sales_vals);
    let eps_growth = calculate_growth_analysis(&raw_years, &eps_vals);

    // PTP growth: only include years with positive pretax_income
    let ptp_valid: Vec<(i32, f64)> = data.records.iter()
        .filter_map(|r| r.pretax_income.map(|v| (r.fiscal_year, v.to_f64().unwrap_or(0.0))))
        .filter(|(_, v)| *v > 0.0)
        .collect();
    let ptp_years_vec: Vec<i32> = ptp_valid.iter().map(|(y, _)| *y).collect();
    let ptp_vals: Vec<f64> = ptp_valid.iter().map(|(_, v)| *v).collect();
    let ptp_growth = calculate_growth_analysis(&ptp_years_vec, &ptp_vals);

    // Last values for 5-year estimates
    let last_sales = sales_vals.last().copied().unwrap_or(0.0);
    let last_eps = eps_vals.last().copied().unwrap_or(0.0);
    let last_ptp = data.records.last()
        .and_then(|r| r.pretax_income)
        .map(|v| v.to_f64().unwrap_or(0.0))
        .unwrap_or(0.0);

    view! {
        <div class="data-ready">
            <div class="header-control-bar shadow-nav">
                <div class="actions">
                    <button
                        class="primary-btn security-blue-bg"
                        on:click=move |_| set_show_lock_modal.set(true)
                    >
                        "🔒 Lock Thesis"
                    </button>
                    <button
                        class="btn-secondary save-btn"
                        on:click={
                            let data = data.clone();
                            move |_| {
                                let snapshot = steady_invest_logic::AnalysisSnapshot {
                                    historical_data: data.clone(),
                                    projected_sales_cagr: sales_projection_cagr.get(),
                                    projected_eps_cagr: eps_projection_cagr.get(),
                                    projected_ptp_cagr: ptp_projection_cagr.get(),
                                    projected_high_pe: future_high_pe.get(),
                                    projected_low_pe: future_low_pe.get(),
                                    analyst_note: String::new(),
                                    captured_at: chrono::Utc::now(),
                                };
                                let _ = crate::persistence::save_snapshot(&snapshot);
                            }
                        }
                    >
                        <span class="btn-icon">"💾"</span>
                        "Save to File"
                    </button>
                </div>
            </div>

            <div class="header-flex" style="margin-top: 1rem;">
                <h3>"10-Year Historicals Populated"</h3>
                <div class="badge-group">
                    {if data.is_split_adjusted {
                        view! { <span class="badge split-badge">"Split-Adjusted"</span> }.into_any()
                    } else {
                        ().into_any()
                    }}
                    {if let Some(display_cur) = data.display_currency.as_ref() {
                        let display_cur_str = display_cur.to_string();
                        view! { <span class="badge norm-badge">"Normalized to " {display_cur_str}</span> }.into_any()
                    } else {
                        ().into_any()
                    }}
                </div>
            </div>

            // Section 1: Visual Analysis (SSG Chart)
            <SSGChart
                data=data.clone()
                sales_projection_cagr=sales_projection_cagr
                eps_projection_cagr=eps_projection_cagr
                ptp_projection_cagr=ptp_projection_cagr
            />

            // Section 1 continued: Fundamental Company Data table (NAIC Figure 2.1)
            // Note: Price High/Low overrides are intentionally excluded from this table.
            // Per NAIC SSG methodology, only Sales and EPS are editable in Section 1.
            // Price data is display-only (shown in the chart's candlestick bars).
            <div class="fundamental-data-table standard-border">
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
                            // Historical Sales row
                            <tr>
                                <td class="metric-label">"Historical Sales"</td>
                                {data.records.iter().map(|rec| {
                                    let year = rec.fiscal_year;
                                    let val = rec.sales;
                                    let ovr = rec.overrides.iter().find(|o| o.field_name == "sales").cloned();
                                    let is_overridden = ovr.is_some();
                                    let note = ovr.and_then(|o| o.note);
                                    view! {
                                        <td
                                            class=move || if is_overridden { "overridden-cell value-cell" } else { "value-cell" }
                                            on:dblclick=move |_| {
                                                set_active_override.set(Some(ActiveOverride {
                                                    year,
                                                    field: "sales".to_string(),
                                                    current_value: val,
                                                    current_note: note.clone(),
                                                }));
                                            }
                                        >
                                            {val.to_string()}
                                            {if is_overridden { view! { <span class="override-mark">"*"</span> }.into_any() } else { ().into_any() }}
                                        </td>
                                    }
                                }).collect_view()}
                                <td class="summary-col">{format!("{:.1}%", sales_growth.cagr)}</td>
                                <td class="summary-col forecast-col">{move || format!("{:.1}%", sales_projection_cagr.get())}</td>
                                <td class="summary-col estimate-col">{move || format!("{:.0}", project_forward(last_sales, sales_projection_cagr.get(), 5))}</td>
                            </tr>
                            // Historical Earnings row
                            <tr>
                                <td class="metric-label">"Historical Earnings"</td>
                                {data.records.iter().map(|rec| {
                                    let year = rec.fiscal_year;
                                    let val = rec.eps;
                                    let ovr = rec.overrides.iter().find(|o| o.field_name == "eps").cloned();
                                    let is_overridden = ovr.is_some();
                                    let note = ovr.and_then(|o| o.note);
                                    view! {
                                        <td
                                            class=move || if is_overridden { "overridden-cell value-cell" } else { "value-cell" }
                                            on:dblclick=move |_| {
                                                set_active_override.set(Some(ActiveOverride {
                                                    year,
                                                    field: "eps".to_string(),
                                                    current_value: val,
                                                    current_note: note.clone(),
                                                }));
                                            }
                                        >
                                            {val.to_string()}
                                            {if is_overridden { view! { <span class="override-mark">"*"</span> }.into_any() } else { ().into_any() }}
                                        </td>
                                    }
                                }).collect_view()}
                                <td class="summary-col">{format!("{:.1}%", eps_growth.cagr)}</td>
                                <td class="summary-col forecast-col">{move || format!("{:.1}%", eps_projection_cagr.get())}</td>
                                <td class="summary-col estimate-col">{move || format!("{:.2}", project_forward(last_eps, eps_projection_cagr.get(), 5))}</td>
                            </tr>
                            // Pre-Tax Profit row
                            <tr>
                                <td class="metric-label">"Pre-Tax Profit"</td>
                                {data.records.iter().map(|rec| {
                                    let val_str = rec.pretax_income
                                        .map(|v| v.to_string())
                                        .unwrap_or_else(|| "—".to_string());
                                    view! {
                                        <td class="value-cell">{val_str}</td>
                                    }
                                }).collect_view()}
                                <td class="summary-col">{format!("{:.1}%", ptp_growth.cagr)}</td>
                                <td class="summary-col forecast-col">{move || format!("{:.1}%", ptp_projection_cagr.get())}</td>
                                <td class="summary-col estimate-col">{move || {
                                    if last_ptp > 0.0 {
                                        format!("{:.0}", project_forward(last_ptp, ptp_projection_cagr.get(), 5))
                                    } else {
                                        "—".to_string()
                                    }
                                }}</td>
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

            {let data = data.clone(); move || active_override.get().map(|ovr| {
                let data = data.clone();
                view! {
                    <OverrideModal
                        ticker=data.ticker
                        year=ovr.year
                        field=ovr.field
                        current_value=ovr.current_value
                        current_note=ovr.current_note
                        on_close=Callback::new(move |_| set_active_override.set(None))
                        on_save=Callback::new(move |_| on_refetch.run(()))
                    />
                }
            })}

            {let ticker = ticker.clone(); let data = data.clone(); move || show_lock_modal.get().then(|| {
                let data = data.clone();
                let ticker = ticker.clone();
                let chart_id = format!("ssg-chart-{}", ticker.ticker.to_lowercase());
                view! {
                    <LockThesisModal
                        ticker=ticker.ticker
                        chart_id=chart_id
                        historical_data=data
                        sales_projection_cagr=sales_projection_cagr.get()
                        eps_projection_cagr=eps_projection_cagr.get()
                        ptp_projection_cagr=ptp_projection_cagr.get()
                        future_high_pe=future_high_pe.get()
                        future_low_pe=future_low_pe.get()
                        on_close=Callback::new(move |_| set_show_lock_modal.set(false))
                        on_locked=Callback::new(move |id: i32| {
                            set_show_lock_modal.set(false);
                            on_refetch.run(());
                            on_locked.run(id);
                        })
                    />
                }
            })}
        </div>
    }
}

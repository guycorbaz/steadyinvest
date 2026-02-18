use leptos::prelude::*;
use steady_invest_logic::HistoricalData;
use rust_decimal::prelude::ToPrimitive;
use charming::{
    component::{Axis, Legend, Title},
    element::{AxisType, Orient, Tooltip, Trigger, LineStyle, LineStyleType},
    series::Line,
    Chart, WasmRenderer,
};

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
unsafe extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn setupDraggableHandles(chart_id: String, sales_start: f64, sales_years: f64, eps_start: f64, eps_years: f64, ptp_start: f64, ptp_years: f64);

    #[wasm_bindgen(js_namespace = window)]
    fn captureChartAsDataURL(chart_id: String) -> Option<String>;

    #[wasm_bindgen(js_namespace = window)]
    fn addPriceBars(chart_id: String, price_data_json: String);
}

/// Captures the current SSG chart as a base64-encoded PNG string.
///
/// Returns `Some(base64_string)` on success or `None` if the chart
/// element or ECharts instance is unavailable. Non-panicking — all
/// failures return `None` (AC #2).
pub fn capture_chart_image(chart_id: &str) -> Option<String> {
    let data_url = captureChartAsDataURL(chart_id.to_string())?;
    // Strip the data URL prefix to get raw base64
    Some(
        data_url
            .strip_prefix("data:image/png;base64,")
            .unwrap_or(&data_url)
            .to_string(),
    )
}

// Global signals for JS access
thread_local! {
    static SALES_SIGNAL: std::cell::Cell<Option<RwSignal<f64>>> = const { std::cell::Cell::new(None) };
    static EPS_SIGNAL: std::cell::Cell<Option<RwSignal<f64>>> = const { std::cell::Cell::new(None) };
    static PTP_SIGNAL: std::cell::Cell<Option<RwSignal<f64>>> = const { std::cell::Cell::new(None) };
}

/// Updates the Sales projection CAGR from JavaScript drag handle
#[wasm_bindgen]
pub fn rust_update_sales_cagr(val: f64) {
    web_sys::console::log_1(&format!("[Slider] Sales CAGR updated to: {:.2}%", val).into());
    SALES_SIGNAL.with(|s| {
        if let Some(sig) = s.get() {
            sig.set(val);
        }
    });
}

/// Updates the EPS projection CAGR from JavaScript drag handle
#[wasm_bindgen]
pub fn rust_update_eps_cagr(val: f64) {
    web_sys::console::log_1(&format!("[Slider] EPS CAGR updated to: {:.2}%", val).into());
    EPS_SIGNAL.with(|s| {
        if let Some(sig) = s.get() {
            sig.set(val);
        }
    });
}

/// Updates the Pre-Tax Profit projection CAGR from JavaScript drag handle
#[wasm_bindgen]
pub fn rust_update_ptp_cagr(val: f64) {
    web_sys::console::log_1(&format!("[Slider] PTP CAGR updated to: {:.2}%", val).into());
    PTP_SIGNAL.with(|s| {
        if let Some(sig) = s.get() {
            sig.set(val);
        }
    });
}

/// The Stock Selection Guide (SSG) Chart component.
/// 
/// Renders a logarithmic multi-series line chart (Sales, EPS, Price) with 
/// optional trendline overlays and CAGR labels.
/// 
/// Uses the `charming` library for ECharts-based rendering via WASM.
#[component]
pub fn SSGChart(
    data: HistoricalData,
    sales_projection_cagr: RwSignal<f64>,
    eps_projection_cagr: RwSignal<f64>,
    ptp_projection_cagr: RwSignal<f64>,
) -> impl IntoView {
    // Unique ID for the chart container to avoid conflicts
    let chart_id = format!("ssg-chart-{}", data.ticker.to_lowercase());

    // Reactive signal to toggle the visibility of trendlines and CAGR stats.
    let show_trends = RwSignal::new(true);

    // Projection state
    let is_projecting = RwSignal::new(false);

    // Store signals in thread-local for JS callbacks
    SALES_SIGNAL.with(|s| s.set(Some(sales_projection_cagr)));
    EPS_SIGNAL.with(|s| s.set(Some(eps_projection_cagr)));
    PTP_SIGNAL.with(|s| s.set(Some(ptp_projection_cagr)));

    let cid_for_effect = chart_id.clone();
    Effect::new(move |_| {
        // Triggered when trends are toggled, data changes, or projections change.
        let trends_active = show_trends.get();
        let s_cagr = sales_projection_cagr.get();
        let e_cagr = eps_projection_cagr.get();
        let p_cagr = ptp_projection_cagr.get();
        let projecting = is_projecting.get();
        let cid = cid_for_effect.clone();

        // Transform data for charming
        let mut years = Vec::with_capacity(data.records.len());
        let mut prices = Vec::with_capacity(data.records.len());
        let mut prices_low = Vec::with_capacity(data.records.len());

        let mut raw_years = Vec::with_capacity(data.records.len());
        let mut sales = Vec::with_capacity(data.records.len());
        let mut eps = Vec::with_capacity(data.records.len());
        let mut ptp = Vec::with_capacity(data.records.len());

        for record in &data.records {
            years.push(record.fiscal_year.to_string());
            raw_years.push(record.fiscal_year);

            sales.push(record.sales.to_f64().unwrap_or(0.0));
            eps.push(record.eps.to_f64().unwrap_or(0.0));
            prices.push(record.price_high.to_f64().unwrap_or(0.0));
            prices_low.push(record.price_low.to_f64().unwrap_or(0.0));

            // Pre-Tax Profit: use NAN for missing values to avoid log(0) on log-scale Y axis
            if let Some(ptp_val) = record.pretax_income {
                let v = ptp_val.to_f64().unwrap_or(0.0);
                ptp.push(if v > 0.0 { v } else { f64::NAN });
            } else {
                ptp.push(f64::NAN);
            }
        }

        let mut chart = Chart::new()
            .title(Title::new()
                .text(format!("Stock Selection Guide: {}", data.ticker))
                .text_style(charming::element::TextStyle::new()
                    .color("#E0E0E0")
                    .font_family("Inter")
                    .font_size(16)
                    .font_weight("600")))
            .legend(Legend::new()
                .bottom(0)
                .orient(Orient::Horizontal)
                .text_style(charming::element::TextStyle::new()
                    .color("#B0B0B0")
                    .font_family("Inter")
                    .font_size(12)))
            .tooltip(Tooltip::new().trigger(Trigger::Axis))
            .y_axis(Axis::new()
                .type_(AxisType::Log)
                .name("Value")
                .name_text_style(charming::element::TextStyle::new()
                    .color("#B0B0B0")
                    .font_family("Inter")
                    .font_size(12))
                .axis_label(charming::element::AxisLabel::new()
                    .color("#B0B0B0")
                    .font_size(11)));

        let mut sales_start = 0.0;
        let mut sales_years_f = 0.0;
        let mut eps_start = 0.0;
        let mut eps_years_f = 0.0;
        let mut ptp_start = 0.0;
        let mut ptp_years_f = 0.0;

        // Compute dimensions shared by both branches
        let hist_len = raw_years.len();
        if hist_len == 0 {
            return; // No records — skip rendering
        }
        let last_year = *raw_years.last().unwrap_or(&2023);
        let future_years: Vec<i32> = (1..=5).map(|i| last_year + i).collect();

        // Set x-axis once: extend with future years when trends are active
        {
            let x_data = if trends_active {
                let mut all = years.clone();
                for y in &future_years {
                    all.push(y.to_string());
                }
                all
            } else {
                years.clone()
            };
            chart = chart.x_axis(Axis::new()
                .type_(AxisType::Category)
                .data(x_data)
                .axis_label(charming::element::AxisLabel::new()
                    .color("#B0B0B0")
                    .font_size(11)));
        }

        if trends_active {
            let sales_trend = steady_invest_logic::calculate_growth_analysis(&raw_years, &sales);
            let eps_trend = steady_invest_logic::calculate_growth_analysis(&raw_years, &eps);

            // PTP trendline: filter out zero/None years for regression
            let ptp_valid_vals: Vec<f64> = ptp.iter()
                .zip(raw_years.iter())
                .filter(|&(&v, _)| v > 0.0)
                .map(|(&v, _)| v)
                .collect();
            let ptp_valid_years: Vec<i32> = ptp.iter()
                .zip(raw_years.iter())
                .filter(|&(&v, _)| v > 0.0)
                .map(|(_, &y)| y)
                .collect();
            let ptp_trend = steady_invest_logic::calculate_growth_analysis(&ptp_valid_years, &ptp_valid_vals);

            // Initialize projection signals if not yet set
            if !projecting {
                sales_projection_cagr.set(sales_trend.cagr);
                eps_projection_cagr.set(eps_trend.cagr);
                ptp_projection_cagr.set(ptp_trend.cagr);
                is_projecting.set(true);
            }

            // Historical trendline values
            let sales_trend_vals: Vec<f64> = sales_trend.trendline.iter().map(|p| p.value).collect();
            let eps_trend_vals: Vec<f64> = eps_trend.trendline.iter().map(|p| p.value).collect();

            // Last actual data values (projection starts from the solid data line)
            let sales_last_actual = sales.last().copied().unwrap_or(0.0);
            let eps_last_actual = eps.last().copied().unwrap_or(0.0);

            // JS bridge: projection period = 5 years, start = last actual value
            sales_start = sales_last_actual;
            sales_years_f = 5.0;
            eps_start = eps_last_actual;
            eps_years_f = 5.0;

            // Trendline series: historical values + NaN padding for future years
            let mut sales_trendline_data = sales_trend_vals.clone();
            let mut eps_trendline_data = eps_trend_vals.clone();
            for _ in 0..5 {
                sales_trendline_data.push(f64::NAN);
                eps_trendline_data.push(f64::NAN);
            }

            // Projection anchored from last actual data point (connects to solid line)
            let s_proj = steady_invest_logic::calculate_projected_trendline(
                last_year, sales_last_actual, s_cagr, &future_years
            );
            let e_proj = steady_invest_logic::calculate_projected_trendline(
                last_year, eps_last_actual, e_cagr, &future_years
            );

            // Projection series: NaN for historical years (except last for continuity)
            let mut s_proj_data: Vec<f64> = vec![f64::NAN; hist_len - 1];
            s_proj_data.push(sales_last_actual); // Connection point at last actual data point
            for p in &s_proj.trendline {
                s_proj_data.push(p.value);
            }
            let mut e_proj_data: Vec<f64> = vec![f64::NAN; hist_len - 1];
            e_proj_data.push(eps_last_actual);
            for p in &e_proj.trendline {
                e_proj_data.push(p.value);
            }

            // PTP trendline and projection
            // Map PTP trendline values back to the full year range (NaN where PTP was zero/missing)
            let mut ptp_trendline_data: Vec<f64> = Vec::with_capacity(hist_len + 5);
            let mut ptp_trend_idx = 0;
            for (i, &v) in ptp.iter().enumerate() {
                if v > 0.0 && ptp_trend_idx < ptp_trend.trendline.len() {
                    ptp_trendline_data.push(ptp_trend.trendline[ptp_trend_idx].value);
                    ptp_trend_idx += 1;
                } else {
                    // Interpolate from the trendline at this year's position if possible
                    let _ = i; // Use NaN for missing PTP years
                    ptp_trendline_data.push(f64::NAN);
                }
            }
            // Last actual PTP value (last finite positive value in the series)
            let ptp_last_actual = ptp.iter().rev()
                .find(|v| v.is_finite() && **v > 0.0)
                .copied()
                .unwrap_or(0.0);
            ptp_start = ptp_last_actual;
            ptp_years_f = 5.0;

            for _ in 0..5 {
                ptp_trendline_data.push(f64::NAN);
            }

            // Projection anchored from last actual PTP data point
            let p_proj = steady_invest_logic::calculate_projected_trendline(
                last_year, ptp_last_actual, p_cagr, &future_years
            );
            let mut p_proj_data: Vec<f64> = vec![f64::NAN; hist_len - 1];
            p_proj_data.push(if ptp_last_actual > 0.0 { ptp_last_actual } else { f64::NAN });
            for p in &p_proj.trendline {
                p_proj_data.push(p.value);
            }

            chart = chart
                // Sales data line
                .series(Line::new()
                    .name(format!("Sales Growth: {:.1}%", s_cagr))
                    .data(sales.clone())
                    .smooth(true)
                    .line_style(LineStyle::new().color("#1DB954")))
                // Sales historical trendline (dotted overlay)
                .series(Line::new()
                    .name("Sales Trend")
                    .data(sales_trendline_data)
                    .line_style(LineStyle::new().color("#1DB954").width(1).type_(LineStyleType::Dotted)))
                // Sales projection (dashed, starts at last historical year)
                .series(Line::new()
                    .name("Sales Est. Growth")
                    .data(s_proj_data)
                    .line_style(LineStyle::new().color("#1DB954").width(2).type_(LineStyleType::Dashed)))
                // EPS data line
                .series(Line::new()
                    .name(format!("EPS Growth: {:.1}%", e_cagr))
                    .data(eps.clone())
                    .smooth(true)
                    .line_style(LineStyle::new().color("#3498DB")))
                // EPS historical trendline
                .series(Line::new()
                    .name("EPS Trend")
                    .data(eps_trendline_data)
                    .line_style(LineStyle::new().color("#3498DB").width(1).type_(LineStyleType::Dotted)))
                // EPS projection
                .series(Line::new()
                    .name("EPS Est. Growth")
                    .data(e_proj_data)
                    .line_style(LineStyle::new().color("#3498DB").width(2).type_(LineStyleType::Dashed)))
                // Pre-Tax Profit data line (red/magenta per NAIC)
                .series(Line::new()
                    .name(format!("Pre-Tax Profit Growth: {:.1}%", p_cagr))
                    .data(ptp.clone())
                    .smooth(true)
                    .line_style(LineStyle::new().color("#E74C3C")))
                // PTP historical trendline
                .series(Line::new()
                    .name("PTP Trend")
                    .data(ptp_trendline_data)
                    .line_style(LineStyle::new().color("#E74C3C").width(1).type_(LineStyleType::Dotted)))
                // PTP projection
                .series(Line::new()
                    .name("PTP Est. Growth")
                    .data(p_proj_data)
                    .line_style(LineStyle::new().color("#E74C3C").width(2).type_(LineStyleType::Dashed)));
        } else {
            chart = chart
                .series(Line::new().name("Sales").data(sales).smooth(true).line_style(LineStyle::new().color("#1DB954")))
                .series(Line::new().name("EPS").data(eps).smooth(true).line_style(LineStyle::new().color("#3498DB")))
                .series(Line::new().name("Pre-Tax Profit").data(ptp.clone()).smooth(true).line_style(LineStyle::new().color("#E74C3C")));
        }

        // Price range bars are added via chart_bridge.js (addPriceBars) after
        // WasmRenderer renders, because charming's RawString (needed for Custom
        // series renderItem) doesn't work through serde_wasm_bindgen.
        let price_json = format!("[{}]",
            prices.iter().zip(prices_low.iter())
                .map(|(&high, &low)| format!("[{},{}]", low, high))
                .collect::<Vec<_>>()
                .join(",")
        );

        // Use requestAnimationFrame to defer rendering until after DOM update
        let window = web_sys::window().expect("no global window");
        let render_callback = Closure::once(Box::new(move || {
            // Get actual container dimensions dynamically for responsive chart sizing
            let container = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id(&cid));
            let container_width = container.as_ref().map(|e| e.client_width()).unwrap_or(800) as u32;
            let container_height = container.as_ref().map(|e| e.client_height()).unwrap_or(500) as u32;

            // Ensure minimum dimensions and use actual container size
            let chart_width = container_width.max(600).min(1400);
            let chart_height = container_height.max(300).min(700);
            let renderer = WasmRenderer::new(chart_width, chart_height);
            if let Err(e) = renderer.render(&cid, &chart) {
                web_sys::console::log_1(&format!("Chart render error: {:?}", e).into());
            }

            // Add price bars via JS bridge (Custom series renderItem needs real JS function)
            addPriceBars(cid.clone(), price_json);

            if trends_active && projecting {
                setup_handles_js(cid, sales_start, sales_years_f, eps_start, eps_years_f, ptp_start, ptp_years_f);
            }
        }) as Box<dyn FnOnce()>);

        window.request_animation_frame(render_callback.as_ref().unchecked_ref()).ok();
        render_callback.forget();
    });

    fn setup_handles_js(cid: String, s_start: f64, s_years: f64, e_start: f64, e_years: f64, p_start: f64, p_years: f64) {
        setupDraggableHandles(cid, s_start, s_years, e_start, e_years, p_start, p_years);
    }

    let cid_for_view = chart_id.clone();
    view! {
        <div class="ssg-chart-wrapper" style="
            background-color: var(--background);
            padding: var(--spacing-5);
            border-radius: var(--border-radius-sharp);
            margin-bottom: var(--spacing-8);
            border: var(--border-width) solid var(--surface);
            width: 100%;
            max-width: 100%;
            box-sizing: border-box;
            overflow: hidden;
        ">
            <div class="chart-control-bar">
                <div class="chart-slider-controls">
                    <div class="chart-slider-row">
                        <span style="
                            color: var(--text-secondary);
                            font-size: var(--text-sm);
                            font-family: 'JetBrains Mono', monospace;
                            min-width: 160px;
                        ">"Estimated Sales Growth Rate"</span>
                        <input
                            type="range" min="-20" max="50" step="0.1"
                            prop:value=move || sales_projection_cagr.get()
                            on:input=move |ev| {
                                if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                    sales_projection_cagr.set(val);
                                }
                            }
                            class="ssg-chart-slider"
                            style="
                                accent-color: var(--sales-color);
                                cursor: grab;
                            "
                        />
                        <span style="
                            color: var(--sales-color);
                            font-family: 'JetBrains Mono', monospace;
                            font-weight: bold;
                            width: 48px;
                            text-align: right;
                        ">{move || format!("{:.1}%", sales_projection_cagr.get())}</span>
                    </div>
                    <div class="chart-slider-row">
                        <span style="
                            color: var(--text-secondary);
                            font-size: var(--text-sm);
                            font-family: 'JetBrains Mono', monospace;
                            min-width: 160px;
                        ">"Estimated EPS Growth Rate"</span>
                        <input
                            type="range" min="-20" max="50" step="0.1"
                            prop:value=move || eps_projection_cagr.get()
                            on:input=move |ev| {
                                if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                    eps_projection_cagr.set(val);
                                }
                            }
                            class="ssg-chart-slider"
                            style="
                                accent-color: var(--eps-color);
                                cursor: grab;
                            "
                        />
                        <span style="
                            color: var(--eps-color);
                            font-family: 'JetBrains Mono', monospace;
                            font-weight: bold;
                            width: 48px;
                            text-align: right;
                        ">{move || format!("{:.1}%", eps_projection_cagr.get())}</span>
                    </div>
                    <div class="chart-slider-row">
                        <span style="
                            color: var(--text-secondary);
                            font-size: var(--text-sm);
                            font-family: 'JetBrains Mono', monospace;
                            min-width: 160px;
                        ">"Estimated Pre-Tax Profit Growth Rate"</span>
                        <input
                            type="range" min="-20" max="50" step="0.1"
                            prop:value=move || ptp_projection_cagr.get()
                            on:input=move |ev| {
                                if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                    ptp_projection_cagr.set(val);
                                }
                            }
                            class="ssg-chart-slider"
                            style="
                                accent-color: #E74C3C;
                                cursor: grab;
                            "
                        />
                        <span style="
                            color: #E74C3C;
                            font-family: 'JetBrains Mono', monospace;
                            font-weight: bold;
                            width: 48px;
                            text-align: right;
                        ">{move || format!("{:.1}%", ptp_projection_cagr.get())}</span>
                    </div>
                </div>
                <button
                    class="chart-trend-toggle"
                    on:click=move |_| show_trends.update(|v| *v = !*v)
                    style="
                        padding: var(--spacing-2) var(--spacing-4);
                        background: var(--surface);
                        color: var(--text-secondary);
                        font-size: var(--text-sm);
                        font-family: 'Inter', sans-serif;
                        font-weight: 500;
                        border-radius: var(--border-radius-sharp);
                        border: var(--border-width) solid rgba(255, 255, 255, 0.1);
                        transition: all var(--transition-fast);
                        cursor: pointer;
                        white-space: nowrap;
                    "
                    onmouseover="this.style.background='rgba(59, 130, 246, 0.1)'; this.style.color='var(--primary)'; this.style.borderColor='var(--primary)';"
                    onmouseout="this.style.background='var(--surface)'; this.style.color='var(--text-secondary)'; this.style.borderColor='rgba(255, 255, 255, 0.1)';"
                >
                    {move || if show_trends.get() { "Hide Trends" } else { "Show Trends" }}
                </button>
            </div>
            <div id=cid_for_view class="ssg-chart-container"></div>
            <div class="chart-cagr-mobile-summary">
                <div class="cagr-mobile-item">
                    <span class="cagr-mobile-label">"Sales Growth"</span>
                    <span class="cagr-mobile-value" style="color: var(--sales-color);">
                        {move || format!("{:.1}%", sales_projection_cagr.get())}
                    </span>
                </div>
                <div class="cagr-mobile-item">
                    <span class="cagr-mobile-label">"EPS Growth"</span>
                    <span class="cagr-mobile-value" style="color: var(--eps-color);">
                        {move || format!("{:.1}%", eps_projection_cagr.get())}
                    </span>
                </div>
                <div class="cagr-mobile-item">
                    <span class="cagr-mobile-label">"PTP Growth"</span>
                    <span class="cagr-mobile-value" style="color: #E74C3C;">
                        {move || format!("{:.1}%", ptp_projection_cagr.get())}
                    </span>
                </div>
            </div>
            <p class="chart-hint" style="
                color: var(--text-muted);
                font-size: var(--text-xs);
                margin-top: var(--spacing-2);
                font-style: italic;
                font-family: 'Inter', sans-serif;
            ">"Hint: Drag handles on the chart or use sliders for growth projections."</p>
        </div>
    }
}

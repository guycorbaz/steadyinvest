//! History Timeline Sidebar — vertical timeline of past analyses for a ticker.
//!
//! Displays a chronological list of analysis snapshots for the current ticker,
//! allowing the user to select a past analysis for side-by-side comparison.

use leptos::prelude::*;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// DTOs — matching backend HistoryResponse from GET /api/v1/snapshots/{id}/history
// ---------------------------------------------------------------------------

/// A single snapshot entry in the history timeline.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TimelineEntry {
    pub id: i32,
    pub captured_at: String,
    pub thesis_locked: bool,
    pub notes: Option<String>,
    pub projected_sales_cagr: Option<f64>,
    pub projected_eps_cagr: Option<f64>,
    pub projected_high_pe: Option<f64>,
    pub projected_low_pe: Option<f64>,
    pub current_price: Option<f64>,
    pub target_high_price: Option<f64>,
    pub target_low_price: Option<f64>,
    pub native_currency: Option<String>,
    pub upside_downside_ratio: Option<f64>,
}

/// Delta between consecutive snapshots.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct MetricDelta {
    pub from_snapshot_id: i32,
    pub to_snapshot_id: i32,
    pub sales_cagr_delta: Option<f64>,
    pub eps_cagr_delta: Option<f64>,
    pub high_pe_delta: Option<f64>,
    pub low_pe_delta: Option<f64>,
    pub price_delta: Option<f64>,
    pub upside_downside_delta: Option<f64>,
}

/// Full history response from `GET /api/v1/snapshots/{id}/history`.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct HistoryResponse {
    pub ticker_id: i32,
    pub ticker_symbol: String,
    pub snapshots: Vec<TimelineEntry>,
    pub metric_deltas: Vec<MetricDelta>,
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// History Timeline Sidebar component.
///
/// Renders a vertical timeline of past analyses for a ticker.
/// Clicking an entry emits `on_select(Some(id))` for comparison;
/// clicking the already-selected entry emits `on_select(None)` to deselect.
#[component]
pub fn HistoryTimeline(
    /// All snapshot entries for this ticker, ordered by captured_at ASC.
    entries: Vec<TimelineEntry>,
    /// The ID of the currently-displayed analysis (highlighted in timeline).
    current_snapshot_id: Option<i32>,
    /// The ID of the currently-selected past analysis (for comparison).
    selected_past_id: Option<i32>,
    /// Callback when user selects/deselects a past analysis.
    on_select: Callback<Option<i32>>,
) -> impl IntoView {
    if entries.is_empty() {
        return view! {
            <div class="timeline-empty">
                "No previous analyses"
            </div>
        }
        .into_any();
    }

    // Render entries in reverse chronological order (newest first) for the sidebar
    let mut display_entries = entries.clone();
    display_entries.reverse();

    let items = display_entries
        .into_iter()
        .map(|entry| {
            let id = entry.id;
            let is_current = current_snapshot_id == Some(id);
            let is_selected = selected_past_id == Some(id);

            let lock_icon = if entry.thesis_locked {
                "\u{1f512}"
            } else {
                "\u{1f513}"
            };

            let date_display = entry.captured_at.chars().take(10).collect::<String>();
            let aria_label = format!("Analysis from {}", date_display);

            let sales_cagr = entry
                .projected_sales_cagr
                .map(|v| format!("S: {:.1}%", v))
                .unwrap_or_else(|| "S: \u{2014}".to_string());
            let eps_cagr = entry
                .projected_eps_cagr
                .map(|v| format!("E: {:.1}%", v))
                .unwrap_or_else(|| "E: \u{2014}".to_string());

            let item_class = if is_current {
                "timeline-entry timeline-current"
            } else if is_selected {
                "timeline-entry timeline-selected"
            } else {
                "timeline-entry"
            };

            let on_select = on_select.clone();
            let on_click = move |_| {
                if is_current {
                    // Clicking current analysis does nothing
                    return;
                }
                if is_selected {
                    // Deselect
                    on_select.run(None);
                } else {
                    on_select.run(Some(id));
                }
            };

            let current_label = if is_current {
                Some(view! { <span class="timeline-current-label">"Current"</span> })
            } else {
                None
            };

            view! {
                <button
                    class=item_class
                    on:click=on_click
                    disabled=is_current
                    aria-label=aria_label
                >
                    <div class="timeline-entry-header">
                        <span class="timeline-date">{date_display}</span>
                        <span class="timeline-lock">{lock_icon}</span>
                        {current_label}
                    </div>
                    <div class="timeline-entry-metrics">
                        <span class="timeline-metric">{sales_cagr}</span>
                        <span class="timeline-metric">{eps_cagr}</span>
                    </div>
                </button>
            }
        })
        .collect_view();

    view! {
        <nav
            id="history-sidebar"
            class="timeline-sidebar"
            role="complementary"
            aria-label="Thesis history timeline"
        >
            <div class="timeline-header">
                <h3 class="timeline-title">"History"</h3>
                <span class="timeline-count">{entries.len().to_string()} " analyses"</span>
            </div>
            <div class="timeline-list">
                {items}
            </div>
        </nav>
    }
    .into_any()
}

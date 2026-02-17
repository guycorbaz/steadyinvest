//! Reusable UI component library.
//!
//! ## Layout Components
//! - [`command_strip`] — Persistent sidebar navigation
//! - [`footer`]        — Global footer with latency indicator
//!
//! ## Analysis Components
//! - [`search_bar`]            — Ticker search with autocomplete
//! - [`analyst_hud`]           — Main analysis workspace (chart + panels + data grid)
//! - [`ssg_chart`]             — Logarithmic SSG chart with draggable trendlines
//! - [`valuation_panel`]       — P/E slider controls and buy/sell zone display
//! - [`quality_dashboard`]     — ROE and Profit-on-Sales table with trend indicators
//! - [`snapshot_hud`]          — Read-only view of a locked analysis snapshot
//! - [`history_timeline`]      — Vertical timeline sidebar for thesis evolution
//! - [`snapshot_comparison`]   — Side-by-side comparison cards with metric deltas
//!
//! ## Library & Comparison Components
//! - [`compact_analysis_card`] — Compact card for library/comparison grid views
//!
//! ## Modal Dialogs
//! - [`override_modal`]     — Manual data override entry form
//! - [`lock_thesis_modal`]  — Thesis lock confirmation dialog

pub mod counter_btn;
pub mod search_bar;
pub mod ssg_chart;
pub mod quality_dashboard;
pub mod valuation_panel;
pub mod override_modal;
pub mod lock_thesis_modal;
pub mod analyst_hud;
pub mod snapshot_hud;
pub mod footer;
pub mod command_strip;
pub mod compact_analysis_card;
pub mod history_timeline;
pub mod snapshot_comparison;

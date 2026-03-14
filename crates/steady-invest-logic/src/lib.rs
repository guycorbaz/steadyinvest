//! # NAIC Stock Selection Guide — Core Business Logic
//!
//! This crate contains the shared financial analysis logic used by both the
//! backend API and the Leptos frontend (via WASM). It implements the key
//! calculations from the **NAIC Stock Selection Guide (SSG)** methodology:
//!
//! - **Growth analysis** — logarithmic trendline regression and CAGR calculation
//!   for Sales and EPS series ([`calculate_growth_analysis`])
//! - **P/E range analysis** — historical High/Low P/E ratios averaged over the
//!   last 5 years ([`calculate_pe_ranges`])
//! - **Quality metrics** — ROE and Profit-on-Sales with year-over-year trend
//!   indicators ([`calculate_quality_analysis`])
//! - **Projections** — CAGR-based future trendlines for valuation zone
//!   calculations ([`calculate_projected_trendline`])
//!
//! ## Key Types
//!
//! - [`HistoricalData`] — aggregated financial records with adjustment and
//!   normalization methods
//! - [`AnalysisSnapshot`] — point-in-time capture of an analyst's full thesis
//! - [`TrendAnalysis`] — CAGR value plus best-fit trendline points
//! - [`PeRangeAnalysis`] — per-year High/Low P/E with computed averages
//!
//! ## Design Principles
//!
//! All business logic lives in this crate — UI components consume results only.
//! Financial values use [`rust_decimal::Decimal`] for precision; intermediate
//! math (trendlines, CAGR) uses `f64` where acceptable.

mod adjustments;
mod calculations;
mod currency;
mod projections;
mod types;

pub use calculations::*;
pub use currency::*;
pub use projections::*;
pub use types::*;

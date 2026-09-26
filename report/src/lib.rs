//! steadyinvest-report — UI-independent PDF/print of the faithful SSG form.
//!
//! Renders a study to a faithful, neutral layout that reads in black and white (no NAIC
//! marks/logos or verbatim instructional text; colour only as a second channel — owner decision,
//! Guy 2026-09-26). Depends only on `core` and `contract` so it can run headless. Implemented
//! in Story 5.6 (study PDF) and extended in Epic 7 (other forms).
//!
//! - [`form`] owns the single `Study → core::StudySnapshot` construction path (relocated here so the
//!   live form in `app` and the PDF here share ONE construction — no drift). `app` re-exports it.
//! - [`pdf`] lays out the faithful, neutral, black-and-white-safe SSG form with `pdf-writer`.

pub mod comparison;
pub mod form;
pub mod pdf;
pub mod quick_screen;
pub mod review;

pub use comparison::{Comparison, ComparisonColumn, average_years, render_comparison};
pub use pdf::{JUDGED_NOTE, JUDGED_SIGIL, NumberStyle, ReportError, render_study_pdf};
pub use quick_screen::{QuickScreen, QuickScreenLadder, QuickScreenPriceRow, render_quick_screen};
pub use review::{DueLine, PortfolioReview, ReviewLine, ShareLine, render_portfolio_review};

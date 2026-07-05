//! steadyinvest-report — UI-independent PDF/print of the faithful SSG form.
//!
//! Renders a study to a faithful, neutral, grayscale-safe layout (no NAIC marks/logos or verbatim
//! instructional text). Depends only on `core` and `contract` so it can run headless. Implemented
//! in Story 5.6 (study PDF) and extended in Epic 7 (other forms).
//!
//! - [`form`] owns the single `Study → core::StudySnapshot` construction path (relocated here so the
//!   live form in `app` and the PDF here share ONE construction — no drift). `app` re-exports it.
//! - [`pdf`] lays out the faithful, neutral, greyscale-safe SSG form with `pdf-writer`.

pub mod form;
pub mod pdf;

pub use pdf::{ReportError, render_study_pdf};

//! Business logic service layer.
//!
//! Services encapsulate domain logic that controllers delegate to. They interact
//! with the database via SeaORM models and external APIs (financial data providers,
//! exchange rate sources).
//!
//! - [`harvest`]                 — Fetches and stores 10-year historical financial data
//! - [`exchange`]                — Currency conversion using cached exchange rates (harvest pipeline)
//! - [`exchange_rate_provider`]  — Current exchange rates via Frankfurter API with DB fallback
//! - [`audit_service`]           — Records data-integrity events and manual overrides
//! - [`provider_health`]         — Monitors API provider availability and rate limits
//! - [`reporting`]               — Generates PDF/image SSG report exports

pub mod audit_service;
pub mod harvest;
pub mod exchange;
pub mod exchange_rate_provider;
pub mod reporting;
pub mod provider_health;
#[cfg(test)]
mod reporting_test;

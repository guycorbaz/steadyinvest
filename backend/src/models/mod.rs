//! Database model layer (SeaORM).
//!
//! Each sub-module wraps an auto-generated entity from [`_entities`] with
//! domain-specific query helpers, validations, and conversion logic.
//!
//! - [`users`]                 — User accounts and authentication
//! - [`tickers`]               — Security ticker registry
//! - [`historicals`]           — 10-year historical financial records
//! - [`historicals_overrides`] — Manual data overrides per year/field
//! - [`exchange_rates`]        — Cached currency conversion rates
//! - [`audit_logs`]            — Data-integrity and override audit trail
//! - [`analysis_snapshots`]    — Persisted analysis snapshots (append-only)
//! - [`provider_rate_limits`]  — API provider rate-limit tracking

pub mod _entities;
pub mod users;
pub mod tickers;
pub mod audit_logs;
pub mod historicals;
pub mod exchange_rates;
pub mod historicals_overrides;
pub mod analysis_snapshots;
pub mod provider_rate_limits;

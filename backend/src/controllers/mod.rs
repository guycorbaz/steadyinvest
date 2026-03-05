//! REST API controller layer.
//!
//! Each sub-module registers Axum routes under `/api/v1/` and delegates
//! business logic to the [`services`](crate::services) layer.
//!
//! ## Endpoints
//!
//! - [`harvest`]         — Trigger and monitor 10-year data harvests
//! - [`tickers`]         — Ticker search and autocomplete
//! - [`overrides`]       — Manual data override CRUD
//! - [`analyses`]        — Analysis persistence (save / load / list / delete)
//! - [`snapshots`]       — Analysis snapshot CRUD (append-only, immutable)
//! - [`exchange_rates`]  — Current EUR/CHF/USD exchange rates
//! - [`auth`]            — User authentication (register, login, verify)
//! - [`comparisons`]     — Ad-hoc compare and persisted comparison sets
//! - [`system`]          — System health and provider status

pub mod analyses;
pub mod auth;
pub mod comparisons;
pub mod exchange_rates;
pub mod harvest;
pub mod overrides;
pub mod snapshot_metrics;
pub mod snapshots;
pub mod system;
pub mod tickers;

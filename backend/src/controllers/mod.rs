//! REST API controller layer.
//!
//! Each sub-module registers Axum routes under `/api/v1/` and delegates
//! business logic to the [`services`](crate::services) layer.
//!
//! ## Endpoints
//!
//! - [`harvest`]   — Trigger and monitor 10-year data harvests
//! - [`tickers`]   — Ticker search and autocomplete
//! - [`overrides`] — Manual data override CRUD
//! - [`analyses`]  — Analysis persistence (save / load / list / delete)
//! - [`auth`]      — User authentication (register, login, verify)
//! - [`system`]    — System health and provider status

pub mod auth;
pub mod tickers;
pub mod overrides;
pub mod harvest;
pub mod analyses;
pub mod snapshots;
pub mod system;
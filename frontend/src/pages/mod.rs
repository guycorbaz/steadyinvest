//! Page-level components mapped to routes.
//!
//! - [`home`]           — `/` — Main analysis workspace
//! - [`system_monitor`] — `/system-monitor` — API health dashboard
//! - [`audit_log`]      — `/audit-log` — Data integrity event log
//! - [`not_found`]      — Fallback 404 page

pub mod home;
pub mod not_found;
pub mod audit_log;
pub mod system_monitor;
pub mod library;
pub mod comparison;

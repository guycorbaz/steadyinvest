//! steadyinvest-ingestion — provider-agnostic acquisition + normalization layer.
//!
//! This is the **only** crate permitted to perform network I/O. The `MarketDataProvider` trait,
//! the first adapter (EODHD), the IFRS↔US-GAAP / split / fiscal-period / currency normalization,
//! and non-destructive reconciliation all arrive in Epic 3. Keys are injected by `app` (from the
//! OS keychain) and are never read inside this crate.

// Intentionally empty in the scaffold; types land in Epic 3.
#![allow(unused_crate_dependencies)]

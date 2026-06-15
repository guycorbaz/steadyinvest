//! steadyinvest-ingestion — provider-agnostic acquisition layer (FR15, FR21–27).
//!
//! This is the **only** crate that performs network I/O. The [`MarketDataProvider`] trait + the
//! first adapter ([`adapters::eodhd`]) fetch a ticker's raw annual series; [`fetch_canonical`]
//! routes it through `core::normalize` (Epic 1, unchanged) into [`CanonicalFinancials`] plus a
//! dependency digest. The raw ↔ `contract::Cell` stamping (provenance, source, freshness) is the
//! caller's job (`app`), which owns the `Clock` and the journal `logical_version`. Keys are
//! injected by `app` — never read here.

pub mod adapters;
pub mod error;
pub mod fetch;
pub mod provider;

pub use error::{IngestionError, ProviderError};
pub use fetch::{dependency_digest, fetch_canonical, FakeProvider, FetchedFinancials, Provider};
pub use provider::MarketDataProvider;

// Re-export the canonical types callers need to stamp cells, so `app` need not also reach into
// `core::normalize` for them.
pub use steadyinvest_core::normalize::{CanonicalFinancials, CanonicalYear, YearUsability};

/// Install the pure-Rust `ring` crypto provider for rustls **once**, before any HTTPS request
/// (Story 3.1; reqwest uses `rustls-no-provider`, so a provider must be installed in-process).
/// Idempotent — a second call (or a provider already installed) is a no-op. Call from
/// `app::main` at startup.
pub fn install_crypto_provider() {
    // `install_default` returns `Err` if one is already installed — that is fine.
    let _ = rustls::crypto::ring::default_provider().install_default();
}

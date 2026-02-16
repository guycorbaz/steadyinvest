//! Global application state signals.
//!
//! Provides context-scoped reactive signals that persist for the lifetime of
//! the application and can be read from any component via `use_context`.

use leptos::prelude::*;

/// Newtype wrapper for the global currency preference signal.
///
/// Prevents `use_context` collisions with other `RwSignal<String>` contexts.
/// Default value: `"CHF"` (user's primary market).
#[derive(Debug, Clone, Copy)]
pub struct CurrencyPreference(pub RwSignal<String>);

/// Provides global state signals at the app root.
pub fn provide_global_state() {
    let currency_pref = CurrencyPreference(RwSignal::new("CHF".to_string()));
    provide_context(currency_pref);
}

/// Convenience accessor for the global currency preference.
pub fn use_currency_preference() -> RwSignal<String> {
    use_context::<CurrencyPreference>()
        .expect("CurrencyPreference not provided — call provide_global_state() at app root")
        .0
}

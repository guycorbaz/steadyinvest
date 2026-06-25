//! OS secret-store access for provider API keys (Story 3.2, FR25 / NFR-S1).
//!
//! The **only** module that touches `keyring`. Keys are read here by `app` and injected into
//! `ingestion` at fetch time — `ingestion` never reads a key itself (FR63 / architecture
//! invariant), which keeps it offline-testable.
//!
//! **NFR-S1 by construction:** the secret lives *only* in the OS secret store. It is never written
//! to app-config, exports, or backups, and never logged. [`KeychainError`] is a unit enum — it has
//! **no field that could carry the key** — and the only thing ever logged is the (key-free)
//! `keyring` error at the boundary. The store is the persistent secret-service backend
//! (gnome-keyring/KWallet) via pure-Rust `zbus`; when no D-Bus secret agent is running the calls
//! fail with [`KeychainError::Unavailable`] and the caller degrades gracefully (AC6).

use crate::provider::ProviderChoice;

/// The keychain *service* name (one app-wide namespace); the *user* slot is per provider so
/// switching providers never clobbers another's key.
const SERVICE: &str = "steadyinvest";

/// A neutral, cause-named secret-store failure. Carries **no** detail by design (NFR-S1): the key
/// can never ride along, and the user-facing message is a fixed neutral notice. Diagnostic detail
/// (the key-free `keyring` error) is logged at the call boundary instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KeychainError {
    /// No OS secret store is reachable (e.g. no running D-Bus secret agent). Recoverable: the
    /// caller can fall back to the env-var key and keep working (AC6).
    #[error("the OS secret store is unavailable")]
    Unavailable,
    /// The secret store is reachable but rejected the operation.
    #[error("the OS secret store reported an error")]
    Backend,
}

/// The per-provider credential slot.
fn entry(provider: ProviderChoice) -> Result<keyring::Entry, KeychainError> {
    keyring::Entry::new(SERVICE, &format!("provider:{}", provider.wire())).map_err(|e| classify(&e))
}

/// Map a `keyring::Error` to our neutral cause. `NoEntry` is **not** an error — callers handle the
/// "absent" case via `Ok(None)`; it must never reach here.
fn classify(err: &keyring::Error) -> KeychainError {
    match err {
        keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_) => {
            KeychainError::Unavailable
        }
        _ => KeychainError::Backend,
    }
}

/// Store (add or replace) the API key for `provider`. The key crosses into `keyring` and nowhere
/// else. A blank key is rejected upstream (the caller treats blank as "delete"/no-op).
pub fn set_key(provider: ProviderChoice, key: &str) -> Result<(), KeychainError> {
    let entry = entry(provider)?;
    entry.set_password(key).map_err(|e| {
        // The `keyring` error never contains the password; log the cause, never the key.
        tracing::warn!(provider = provider.wire(), error = %e, "keychain set_key failed");
        classify(&e)
    })
}

/// Read the API key for `provider`. `Ok(None)` = no key stored (a normal state, distinct from a
/// store failure); `Err` = the store is unavailable or errored.
pub fn get_key(provider: ProviderChoice) -> Result<Option<String>, KeychainError> {
    let entry = entry(provider)?;
    match entry.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => {
            tracing::warn!(provider = provider.wire(), error = %e, "keychain get_key failed");
            Err(classify(&e))
        }
    }
}

/// Delete the stored key for `provider`. Deleting an absent key is a no-op success (idempotent).
pub fn delete_key(provider: ProviderChoice) -> Result<(), KeychainError> {
    let entry = entry(provider)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => {
            tracing::warn!(provider = provider.wire(), error = %e, "keychain delete_key failed");
            Err(classify(&e))
        }
    }
}

/// Whether a key is currently stored for `provider`. A store failure is reported as `Err` so the
/// caller can distinguish "no key" from "can't tell" (it surfaces a neutral notice for the latter).
pub fn has_key(provider: ProviderChoice) -> Result<bool, KeychainError> {
    Ok(get_key(provider)?.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_is_neutral_and_carries_no_secret() {
        // NFR-S1 structural guard: `KeychainError` is a unit enum, so no key value can be attached.
        // Its `Display` is a fixed neutral cause — assert it stays free of any value-like content.
        for err in [KeychainError::Unavailable, KeychainError::Backend] {
            let shown = err.to_string();
            assert!(shown.contains("secret store"), "neutral cause: {shown}");
            assert!(!shown.contains("provider:"), "no slot/key leakage: {shown}");
        }
    }

    // The live OS secret store needs a running D-Bus agent absent in headless CI; the keyring `mock`
    // backend (a dev-only in-memory store) lets us prove the store LOGIC here. The real backend is
    // exercised by the manual GO/NO-GO (Task 8).
    use std::sync::Once;

    /// Install the in-memory mock credential store once for the whole test binary (must happen
    /// before the first `Entry::new`). Only this test module calls `keyring::Entry` in tests.
    fn install_mock_store() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        });
    }

    #[test]
    fn absent_key_mappings_and_idempotent_delete_over_the_mock_store() {
        install_mock_store();
        let p = ProviderChoice::Eodhd;

        // keyring's mock builder hands each `Entry::new` a FRESH empty credential (it does not share
        // state across Entry instances for the same slot), so this proves the key-free ERROR-MAPPING
        // logic — not cross-call persistence (that needs the real backend → manual GO/NO-GO, Task 8).
        // These mappings gate AC3/AC5 ("absent" vs "unavailable") and AC1 (idempotent delete):
        assert_eq!(
            get_key(p).unwrap(),
            None,
            "a never-set slot maps NoEntry → Ok(None), not Err"
        );
        assert!(!has_key(p).unwrap(), "has_key is false for an absent key");
        delete_key(p).expect("delete of an absent key is an idempotent Ok, never an error");
        set_key(p, "any-key").expect("set over a reachable store succeeds");
    }
}

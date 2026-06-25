//! Provider + ingestion error model (FR13 neutral voice, cause-named per ADD15).
//!
//! [`ProviderError`] names the *cause* a later story (3.5) classifies into a banner
//! (network / quota / key / not-found). [`IngestionError`] wraps a provider error or a
//! `core::normalize` structural error. Messages are neutral, fact-stating — gated by the
//! crate-local posture test below against the **canonical** `core::method::BANNED_VERBS_*`
//! (ingestion depends on `core`, so unlike `persistence` it reuses the list, never copies it).

use thiserror::Error;

/// Everything a provider fetch can fail with — the cause is preserved for Story 3.5 to classify.
/// `Clone` so test doubles ([`crate::FakeProvider`]) can hand back a canned failure.
#[derive(Debug, Clone, Error)]
pub enum ProviderError {
    /// Connectivity / transport failure (DNS, TLS, timeout, reset).
    #[error("the provider request did not complete: {detail}")]
    Network { detail: String },

    /// Rate limit or plan quota reached; `retry_after` seconds when the provider states it.
    #[error("the provider reported a usage limit; retry is possible later")]
    Quota { retry_after_secs: Option<u64> },

    /// The API key was rejected or absent where one was required.
    #[error("the provider rejected the request as unauthenticated (key invalid or absent)")]
    InvalidOrAbsentKey,

    /// The key is valid but this resource is not authorized for the account (e.g. a plan that does
    /// not include this data). HTTP 403 — distinct from [`Self::InvalidOrAbsentKey`] (401): the
    /// credential works, the *subscription* does not cover the request. `detail` carries the
    /// provider's own reason (never the key — the body has no token).
    #[error("the provider refused access to this resource for the account: {detail}")]
    Forbidden { detail: String },

    /// The provider has no data for this ticker (404 or empty payload).
    #[error("the provider returned no data for ticker {ticker}")]
    TickerNotFound { ticker: String },

    /// The response body was not the expected shape.
    #[error("the provider response did not parse as the expected shape: {detail}")]
    Parse { detail: String },

    /// The provider or this adapter does not cover the request.
    #[error("this request is not covered by the adapter: {detail}")]
    Unsupported { detail: String },
}

/// The ingestion-level result: a provider failure, or a structural normalization error.
#[derive(Debug, Error)]
pub enum IngestionError {
    #[error(transparent)]
    Provider(#[from] ProviderError),

    /// `core::normalize` rejected the mapped raw input as structurally malformed.
    #[error("the fetched data did not normalize: {0}")]
    Normalize(steadyinvest_core::normalize::NormalizeError),
}

impl From<steadyinvest_core::normalize::NormalizeError> for IngestionError {
    fn from(e: steadyinvest_core::normalize::NormalizeError) -> Self {
        IngestionError::Normalize(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_core::method::{BANNED_VERBS_EN, BANNED_VERBS_FR};

    /// Same whole-word, case-insensitive matcher as `core::golden` / `app::posture`.
    fn contains_word(haystack: &str, needle: &str) -> bool {
        let h = haystack.to_lowercase();
        let n = needle.to_lowercase();
        h.match_indices(&n).any(|(i, _)| {
            let before_ok = i == 0
                || !h[..i]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric());
            let after = i + n.len();
            let after_ok = after == h.len()
                || !h[after..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric());
            before_ok && after_ok
        })
    }

    fn assert_neutral(text: &str) {
        for banned in BANNED_VERBS_EN.iter().chain(BANNED_VERBS_FR.iter()) {
            assert!(
                !contains_word(text, banned),
                "provider-facing message {text:?} contains banned verb {banned:?} (FR13)"
            );
        }
    }

    #[test]
    fn provider_error_messages_are_neutral_no_banned_verb() {
        let samples = [
            ProviderError::Network {
                detail: "example".into(),
            },
            ProviderError::Quota {
                retry_after_secs: Some(60),
            },
            ProviderError::InvalidOrAbsentKey,
            ProviderError::Forbidden {
                detail: "Only EOD data allowed for free users".into(),
            },
            ProviderError::TickerNotFound {
                ticker: "AAPL.US".into(),
            },
            ProviderError::Parse {
                detail: "example".into(),
            },
            ProviderError::Unsupported {
                detail: "example".into(),
            },
        ];
        for e in &samples {
            assert_neutral(&e.to_string());
        }
        assert_neutral(
            &IngestionError::Normalize(
                steadyinvest_core::normalize::NormalizeError::DuplicateYear { year: 2020 },
            )
            .to_string(),
        );
    }
}

//! Small crate-internal helpers shared by the storage modules.
//!
//! Two invariants live here so they are greppable in ONE place:
//! - [`parse_uuid`] — every TEXT id column round-trips through the same corruption-naming parse.
//! - [`bump_logical_version`] — **every mutating transaction on an exported table bumps
//!   `journal_meta.logical_version` exactly once** (NFR-R2). The single deliberate exception is
//!   `price_history` (a local, reconstructible cache excluded from the export snapshot — see
//!   `price_history.rs`), which is why that module does not call this helper. `fx_rates` (Story
//!   6.5) is the contrast case: an EXPORTED axis, so its writers DO bump.

use crate::error::{Error, Result};
use uuid::Uuid;

/// Parse a TEXT id column into a [`Uuid`], naming the field in the corruption error.
pub(crate) fn parse_uuid(text: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(text).map_err(|e| Error::CorruptPayload {
        detail: format!("{field} {text:?} is not a valid UUID: {e}"),
    })
}

/// Bump `journal_meta.logical_version` inside the caller's transaction (NFR-R2).
///
/// Call this exactly once per applied (non-no-op) mutation, in the same transaction as the
/// mutation itself — never outside one.
pub(crate) fn bump_logical_version(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    tx.execute(
        "UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1",
        [],
    )?;
    Ok(())
}

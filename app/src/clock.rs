//! Injected time + identity (ADD15) — the discipline Story 2.1 deferred to here.
//!
//! A single [`Clock`] (wall time → RFC3339 UTC [`contract::Timestamp`]) and [`IdGen`]
//! (`Uuid`) are the **only** sources of timestamps and UUIDs in the app. There is **no**
//! scattered `Uuid::new_v4()` or wall-clock call anywhere else: `Uuid::new_v4` lives only in
//! [`UuidGen`] here and the wall clock only in [`SystemClock`]. The real implementations back the
//! running app; tests inject the [`FixedClock`] / [`FixedIdGen`] doubles for deterministic
//! ids/timestamps — the same determinism rail `steadyinvest-persistence` relies on (it never calls
//! a clock or `new_v4` itself).
//!
//! RFC3339 formatting uses `chrono` (a vetted date crate) — no civil-time conversion is
//! hand-rolled. `to_rfc3339_opts(SecondsFormat::Secs, true)` emits e.g. `2026-06-13T10:30:00Z`,
//! the same `…Z` UTC shape the contract/persistence fixtures use.

use chrono::{SecondsFormat, Utc};
use steadyinvest_contract::Timestamp;
use uuid::Uuid;

/// The app's wall clock. The single source of "now" — injected so tests are deterministic.
pub trait Clock {
    /// Current instant as an RFC3339 UTC [`Timestamp`] (e.g. `2026-06-13T10:30:00Z`).
    fn now(&self) -> Timestamp;
}

/// The app's identity source. The single source of new UUIDs — injected so tests are deterministic.
pub trait IdGen {
    /// A fresh unique id.
    fn new_id(&self) -> Uuid;
}

/// Real clock: the host wall time, formatted as RFC3339 UTC (seconds precision, `Z` suffix).
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true))
    }
}

/// Real id source: a v4 (random) UUID. The ONLY `Uuid::new_v4` call site in the app.
#[derive(Debug, Default, Clone, Copy)]
pub struct UuidGen;

impl IdGen for UuidGen {
    fn new_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}

/// Test double: a clock pinned to a fixed instant (deterministic timestamps).
#[cfg(test)]
#[derive(Debug, Clone)]
pub struct FixedClock(pub Timestamp);

#[cfg(test)]
impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.0.clone()
    }
}

/// Test double: an id source returning a fixed UUID (deterministic ids).
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub struct FixedIdGen(pub Uuid);

#[cfg(test)]
impl IdGen for FixedIdGen {
    fn new_id(&self) -> Uuid {
        self.0
    }
}

/// Test double: a sequential id source returning **distinct** deterministic UUIDs from a counter —
/// for tests that create several entities in one state (e.g. the Story-4.1 watchlist).
#[cfg(test)]
#[derive(Debug)]
pub struct SeqIdGen(std::cell::Cell<u128>);

#[cfg(test)]
impl SeqIdGen {
    pub fn starting_at(seed: u128) -> Self {
        SeqIdGen(std::cell::Cell::new(seed))
    }
}

#[cfg(test)]
impl IdGen for SeqIdGen {
    fn new_id(&self) -> Uuid {
        let n = self.0.get();
        self.0.set(n + 1);
        Uuid::from_u128(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_emits_rfc3339_utc_with_z_suffix() {
        let ts = SystemClock.now();
        // Shape, not value: `YYYY-MM-DDTHH:MM:SSZ` (20 chars, ends in Z, has the T separator).
        assert!(ts.0.ends_with('Z'), "not UTC `Z`-suffixed: {:?}", ts.0);
        assert!(ts.0.contains('T'), "no date/time separator: {:?}", ts.0);
        assert_eq!(ts.0.len(), 20, "unexpected RFC3339 length: {:?}", ts.0);
        // It re-parses as an RFC3339 timestamp (chrono validates round-trip).
        assert!(
            chrono::DateTime::parse_from_rfc3339(&ts.0).is_ok(),
            "SystemClock emitted a non-RFC3339 string: {:?}",
            ts.0
        );
    }

    #[test]
    fn uuid_gen_produces_distinct_v4_ids() {
        let gen = UuidGen;
        let a = gen.new_id();
        let b = gen.new_id();
        assert_ne!(a, b, "two draws collided");
        assert_eq!(a.get_version_num(), 4, "not a v4 UUID");
    }

    #[test]
    fn fixed_doubles_are_deterministic() {
        let clock = FixedClock(Timestamp("2026-06-13T00:00:00Z".to_string()));
        assert_eq!(clock.now(), clock.now());
        assert_eq!(clock.now().0, "2026-06-13T00:00:00Z");

        let id = Uuid::from_u128(0xABCD);
        let gen = FixedIdGen(id);
        assert_eq!(gen.new_id(), id);
        assert_eq!(gen.new_id(), gen.new_id());
    }
}

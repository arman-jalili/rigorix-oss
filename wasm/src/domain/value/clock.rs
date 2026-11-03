//! Pinned time: `Timestamp`, `ClockReading` and the ordered `ClockTrack`.
//!
//! @canonical .pi/architecture/modules/portable-session.md#value-objects-entity-vs-vo-justification
//! Implements: Contract Freeze — Timestamp, ClockReading, ClockTrack
//! Issue: #1294 (contract freeze)
//!
//! Time is a port, never ambient (invariant 1). The session holds a *series* of
//! readings consumed in declared order; when the series is exhausted, the clock port
//! fails and the run is refused (invariant 2). There is deliberately no `now()`
//! anywhere in this crate.

use serde::{Deserialize, Serialize};

/// An RFC 3339 instant, as text.
///
/// # Contract (Frozen)
/// - Canonical form only: `YYYY-MM-DDTHH:MM:SSZ`, no fractional part unless the
///   engine emits one; the string is preserved byte for byte so envelope
///   timestamps match the CLI
/// - No `chrono`/`SystemTime` conversion in this crate
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(String);

impl Timestamp {
    /// Wrap an instant supplied by the clock port or the fixture.
    pub fn from_trusted(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// The canonical text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One pinned instant at a declared position in the series.
///
/// # Contract (Frozen)
/// - `index` is the port's declared ordering; it is what makes replay ordering
///   checkable rather than assumed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockReading {
    /// The pinned instant.
    pub instant: Timestamp,
    /// Zero-based position in the pinned series.
    pub index: u32,
}

/// The ordered series a session's time comes from.
///
/// # Contract (Frozen)
/// - Consumed strictly in order; never re-read, never reordered
/// - An empty series is a `SessionOpenError`; an exhausted one is a `PortFailure`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockTrack {
    readings: Vec<ClockReading>,
}

impl ClockTrack {
    /// Build a series from pinned readings.
    pub fn new(readings: Vec<ClockReading>) -> Self {
        Self { readings }
    }

    /// The pinned readings, in order.
    pub fn readings(&self) -> &[ClockReading] {
        &self.readings
    }

    /// Whether any reading remains unconsumed.
    pub fn is_empty(&self) -> bool {
        self.readings.is_empty()
    }
}

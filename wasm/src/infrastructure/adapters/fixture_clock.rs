//! `FixtureClock` — answers time from the pinned series.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! Implements: Contract Freeze — FixtureClock
//! Issue: #1294 (contract freeze)

use crate::domain::ports::Clock;
use crate::domain::value::{ClockReading, ClockTrack, PortFailure};

/// Answers from an ordered series of pinned readings.
///
/// # Contract (Frozen)
/// - Consumes strictly in order; an exhausted series is
///   `PortFailure { Clock, Exhausted }`
/// - Never falls back to `SystemTime`; there is no builder that would let it
pub struct FixtureClock {
    track: ClockTrack,
    cursor: usize,
}

impl FixtureClock {
    /// Wrap a pinned series.
    pub const fn new(track: ClockTrack) -> Self {
        Self { track, cursor: 0 }
    }
}

impl Clock for FixtureClock {
    fn advance(&mut self) -> Result<ClockReading, PortFailure> {
        let _ = (&self.track, self.cursor);
        todo!("contract freeze: FixtureClock::advance lands with #1298")
    }
}

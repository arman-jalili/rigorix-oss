//! `FixtureClock` — answers time from the pinned series.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! Implements: `Clock`
//! Issue: #1298 (infrastructure layer)
//!
//! # Contract
//! - Consumes strictly in order; an exhausted series is
//!   `PortFailure { Clock, Exhausted }`
//! - Never falls back to `SystemTime`. There is no constructor that would let it: the
//!   only way in is a `ClockTrack`

use crate::domain::ports::Clock;
use crate::domain::value::{ClockReading, ClockTrack, PortFailure, PortFailureKind, PortId};

/// Answers from an ordered series of pinned readings.
#[derive(Debug)]
pub struct FixtureClock {
    track: ClockTrack,
    cursor: usize,
}

impl FixtureClock {
    /// Wrap a pinned series.
    pub const fn new(track: ClockTrack) -> Self {
        Self { track, cursor: 0 }
    }

    /// How many readings remain unconsumed.
    pub fn remaining(&self) -> usize {
        self.track.readings().len().saturating_sub(self.cursor)
    }
}

impl Clock for FixtureClock {
    fn advance(&mut self) -> Result<ClockReading, PortFailure> {
        let reading = self.track.readings().get(self.cursor).cloned();
        match reading {
            Some(reading) => {
                self.cursor += 1;
                Ok(reading)
            }
            // Fail closed. A default instant here would be a fabricated timestamp in a
            // signed record, which is worse than a refusal.
            None => Err(PortFailure::new(PortId::Clock, PortFailureKind::Exhausted)),
        }
    }
}

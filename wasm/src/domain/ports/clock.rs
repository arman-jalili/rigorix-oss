//! `Clock` — the only source of time.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! Implements: Contract Freeze — Clock
//! Issue: #1294 (contract freeze)

use crate::domain::value::{ClockReading, PortFailure};

/// Answers "what time is it" from the session's pinned series.
///
/// # Contract (Frozen)
/// - Each call consumes the next reading, in the declared order
/// - An exhausted series is `PortFailure { port: Clock, kind: Exhausted }`; it is
///   **never** a cast of `0`, a default instant or a real clock read
pub trait Clock {
    /// Take the next pinned reading.
    fn advance(&mut self) -> Result<ClockReading, PortFailure>;
}

//! `MemoryBusinessSink` — records what a step would have written.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! Implements: Contract Freeze — MemoryBusinessSink
//! Issue: #1294 (contract freeze)

use crate::domain::ports::{BusinessSink, SinkWrite};
use crate::domain::value::PortFailure;

/// Keeps intended writes in memory so the host can compare them with its mock system.
///
/// # Contract (Frozen)
/// - Performs no I/O. A sink that would write is a host capability, not this crate's
/// - Recording order is preserved, so "what would have been written" is checkable
pub struct MemoryBusinessSink {
    writes: Vec<SinkWrite>,
}

impl MemoryBusinessSink {
    /// An empty sink.
    pub const fn new() -> Self {
        Self { writes: Vec::new() }
    }

    /// The writes recorded so far, in order.
    pub fn writes(&self) -> &[SinkWrite] {
        &self.writes
    }
}

impl Default for MemoryBusinessSink {
    fn default() -> Self {
        Self::new()
    }
}

impl BusinessSink for MemoryBusinessSink {
    fn record(&mut self, write: &SinkWrite) -> Result<(), PortFailure> {
        self.writes.push(write.clone());
        Ok(())
    }
}

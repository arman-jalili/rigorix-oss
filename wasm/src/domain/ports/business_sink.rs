//! `BusinessSink` — what a permitted step *would* have written.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! Implements: Contract Freeze — BusinessSink, SinkWrite
//! Issue: #1294 (contract freeze)
//!
//! The crate never performs the business action. It records the intended write so the
//! host can assert that the gate permitted exactly what its mock system expected — and
//! so a refusal can show what did not happen.

use serde::{Deserialize, Serialize};

use crate::domain::value::{Params, PortFailure};

/// One intended write.
///
/// # Contract (Frozen)
/// - Describes the effect, never performs it
/// - Carries no personal data beyond what the action parameters already held (NFR-003)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SinkWrite {
    /// Logical target the step would have written to.
    pub target: String,
    /// The effect, as the engine described it.
    pub effect: Params,
}

/// Records intended writes.
///
/// # Contract (Frozen)
/// - No I/O: the shipped adapter records into memory and can compare against the
///   fixture's expectation
/// - A sink that cannot record is `PortFailure { port: BusinessSink, .. }` and the run
///   is refused
pub trait BusinessSink {
    /// Record one intended write.
    fn record(&mut self, write: &SinkWrite) -> Result<(), PortFailure>;
}

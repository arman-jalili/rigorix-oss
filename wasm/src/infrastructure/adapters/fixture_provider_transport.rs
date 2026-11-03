//! `FixtureProviderTransport` — operator checks, answered offline.
//!
//! @canonical .pi/architecture/modules/portable-session.md#infrastructure-layer-infrastructure
//! Implements: Contract Freeze — FixtureProviderTransport
//! Issue: #1294 (contract freeze)

use crate::domain::ports::{ProviderCall, ProviderObservation, ProviderTransport};
use crate::domain::value::PortFailure;

/// Answers checks from the fixture transcript. **No network.**
///
/// # Contract (Frozen)
/// - An unanswered call is `Missing`; an answer that does not parse is `Malformed`
/// - Never `reqwest`, never a socket: the wasm target has none (NFR-002)
pub struct FixtureProviderTransport {
    transcript: serde_json::Value,
}

impl FixtureProviderTransport {
    /// Wrap the fixture transcript.
    pub const fn new(transcript: serde_json::Value) -> Self {
        Self { transcript }
    }
}

impl ProviderTransport for FixtureProviderTransport {
    fn call(&mut self, call: &ProviderCall) -> Result<ProviderObservation, PortFailure> {
        let _ = (&self.transcript, call);
        todo!("contract freeze: FixtureProviderTransport::call lands with #1298")
    }
}

//! `FixtureProviderTransport` — operator checks, answered offline.
//!
//! @canonical .pi/architecture/modules/portable-session.md#infrastructure-layer-infrastructure
//! Implements: `ProviderTransport`
//! Issue: #1298 (infrastructure layer)
//!
//! # Transcript shape
//!
//! ```json
//! { "providers": { "eligibility.check": { "determination": "passed", "payload": { } } } }
//! ```
//!
//! # Contract
//! - An unanswered call is `Missing`; an answer that does not parse is `Malformed`. The
//!   two are distinct because "we never asked" and "the answer was garbage" are different
//!   facts in a record
//! - Never `reqwest`, never a socket: the wasm target has none (NFR-002)

use crate::domain::ports::{Determination, ProviderCall, ProviderObservation, ProviderTransport};
use crate::domain::value::{PortFailure, PortFailureKind, PortId};

/// Answers checks from the fixture transcript.
#[derive(Debug)]
pub struct FixtureProviderTransport {
    transcript: serde_json::Value,
}

impl FixtureProviderTransport {
    /// Wrap the fixture transcript.
    pub const fn new(transcript: serde_json::Value) -> Self {
        Self { transcript }
    }

    /// The transcript key a call addresses.
    fn key(call: &ProviderCall) -> String {
        format!("{}.{}", call.provider, call.method)
    }
}

impl ProviderTransport for FixtureProviderTransport {
    fn call(&mut self, call: &ProviderCall) -> Result<ProviderObservation, PortFailure> {
        let entry = self
            .transcript
            .get("providers")
            .and_then(|providers| providers.get(Self::key(call)));

        let Some(entry) = entry else {
            return Err(PortFailure::new(
                PortId::ProviderTransport,
                PortFailureKind::Missing,
            ));
        };

        let determination = match entry.get("determination").and_then(|d| d.as_str()) {
            Some("passed") => Determination::Passed,
            Some("failed") => Determination::Failed,
            Some("error") => Determination::Error,
            // Anything else is a broken fixture, not a determination. Refusing here keeps
            // a typo from reading as a pass.
            _ => {
                return Err(PortFailure::new(
                    PortId::ProviderTransport,
                    PortFailureKind::Malformed,
                ));
            }
        };

        Ok(ProviderObservation {
            determination,
            payload: entry.get("payload").cloned(),
        })
    }
}

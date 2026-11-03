//! `TelemetryObserver` — where telemetry stops.
//!
//! @canonical .pi/architecture/decisions/ADR-003-cross-context-events.md
//! Implements: `TelemetryObserver` (AC 17)
//! Issue: #1310 (TelemetryObserver)
//!
//! # The rule in one line
//!
//! Telemetry may be observed; it may never be recorded. `RandomDrawConsumed` explains a
//! replay to a fixture author, and a signed artifact that carried it would be claiming
//! audit value for a diagnostic.
//!
//! # Why the split is at the type level, not in this observer
//!
//! This type sorts events for a *subscriber*. The guarantee that telemetry cannot reach a
//! record does not depend on it: `ArtifactSet` has no event field at all — evidence enters
//! only as `EvidenceRef` digests — so there is no slot for a `RandomDrawConsumed` even if
//! someone passed one here. The observer is the diagnostic path; the type is the
//! guarantee.
//!
//! # Release builds
//!
//! The `telemetry` feature is **off by default**: a release bundle drops the diagnostics
//! rather than shipping a channel that could be wired to something. A partner build that
//! wants them names the feature.

use crate::domain::events::{DomainEvent, EventClass};

/// What an observation produced.
#[derive(Debug, Default, PartialEq)]
pub struct Observed {
    /// Evidence class events, in the order they were raised.
    pub evidence: Vec<DomainEvent>,
    /// Telemetry class events, in the order they were raised. Empty in a release build.
    pub telemetry: Vec<DomainEvent>,
}

/// Sorts raised events into evidence and telemetry for a host subscriber.
#[derive(Debug, Clone, Copy)]
pub struct TelemetryObserver {
    keep_telemetry: bool,
}

impl TelemetryObserver {
    /// The observer for this build.
    ///
    /// TODO(#1318): wire the `telemetry` feature into the release profile once the
    /// packaging issue owns the feature list.
    pub const fn new() -> Self {
        Self {
            // A release bundle keeps none: an unused diagnostic channel is still a channel.
            keep_telemetry: cfg!(feature = "diagnostics"),
        }
    }

    /// Whether this build retains telemetry.
    pub const fn keeps_telemetry(&self) -> bool {
        self.keep_telemetry
    }

    /// Sort the events.
    pub fn observe(&self, events: Vec<DomainEvent>) -> Observed {
        let mut observed = Observed::default();
        for event in events {
            match event.class() {
                EventClass::Evidence => observed.evidence.push(event),
                EventClass::Telemetry => {
                    if self.keep_telemetry {
                        observed.telemetry.push(event);
                    }
                    // Otherwise dropped here, deliberately: no queue, no ring buffer, no
                    // "kept for debugging" that later gets serialised.
                }
            }
        }
        observed
    }
}

impl Default for TelemetryObserver {
    fn default() -> Self {
        Self::new()
    }
}

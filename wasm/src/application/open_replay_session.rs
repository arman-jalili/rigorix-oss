//! `OpenReplaySession` — assemble the replayable session (FR-001, FR-003, FR-011).
//!
//! @canonical .pi/architecture/modules/portable-session.md#application-services-from-exploration-use-cases
//! Implements: `OpenReplaySession` (AC 1, 2)
//! Issue: #1297 (application layer)
//!
//! # Contract
//! - Parses with the **engine's** types; this crate owns no scenario grammar (FR-001)
//! - Pins the clock series and seed before anything is evaluated; a session is immutable
//!   once opened
//! - Rejects incomplete or unparseable input with a typed refusal, never a default
//! - Consumes no capability port: opening consults neither the clock, the network nor the
//!   key source. That is `AttemptRun`'s and `ExportArtifactSet`'s business

use crate::application::dto::OpenSessionInput;
use crate::domain::errors::SessionOpenError;
use crate::domain::session::ReplaySession;

/// Opens a session from pinned inputs.
pub trait OpenReplaySession {
    /// Open.
    fn execute(&self, input: OpenSessionInput) -> Result<ReplaySession, SessionOpenError>;
}

/// The shipped implementation.
///
/// A unit struct because opening consults no capability. If that ever changes, the
/// dependency belongs in the constructor, not in a hidden default.
#[derive(Debug, Default, Clone, Copy)]
pub struct OpenReplaySessionService;

impl OpenReplaySessionService {
    /// Construct the service.
    pub const fn new() -> Self {
        Self
    }
}

impl OpenReplaySession for OpenReplaySessionService {
    fn execute(&self, input: OpenSessionInput) -> Result<ReplaySession, SessionOpenError> {
        // The whole use case: the aggregate owns the validation and the derivation, so
        // this service is a seam, not a second set of rules. Anything it added here would
        // be a rule the aggregate does not know about.
        ReplaySession::open(crate::domain::session::OpenInputs {
            scenario: input.scenario,
            fixtures: input.fixtures,
            clock: input.clock,
            seed: input.seed,
        })
    }
}

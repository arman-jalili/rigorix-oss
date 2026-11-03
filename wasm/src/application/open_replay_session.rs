//! `OpenReplaySession` — assemble the replayable session (FR-001, FR-003, FR-011).
//!
//! @canonical .pi/architecture/modules/portable-session.md#application-services-from-exploration-use-cases
//! Implements: Contract Freeze — OpenReplaySession, OpenReplaySessionService
//! Issue: #1294 (contract freeze)
//!
//! # Contract (Frozen)
//! - Parses with the **engine's** types; this crate owns no scenario grammar (FR-001)
//! - Pins the clock series and seed before anything is evaluated; a session is
//!   immutable once opened
//! - Rejects incomplete or unparseable input with a typed refusal, never a default
//! - Consumes no capability port: opening does not consult the clock, the network or
//!   the key source (that is `AttemptRun`'s and `ExportArtifactSet`'s business)

use crate::application::dto::OpenSessionInput;
use crate::domain::errors::SessionOpenError;
use crate::domain::session::ReplaySession;

/// Opens a session from pinned inputs.
pub trait OpenReplaySession {
    /// Open.
    fn execute(&self, input: OpenSessionInput) -> Result<ReplaySession, SessionOpenError>;
}

/// The shipped implementation (wired in #1295).
///
/// Held as a unit struct: it has no dependencies, because opening consults no
/// capability. If that ever changes, the dependency belongs in the constructor, not in
/// a hidden default.
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
        let _ = input;
        todo!("contract freeze: OpenReplaySession::execute lands with #1295")
    }
}

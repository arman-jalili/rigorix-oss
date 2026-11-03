//! `ArtifactQuery` — the read helper (not a catalog service).
//!
//! @canonical .pi/architecture/modules/portable-session.md#application-layer-application
//! Implements: Contract Freeze — ArtifactQuery
//! Issue: #1294 (contract freeze)
//!
//! # Contract (Frozen)
//! - Read-only: no method mutates a session, and no method can produce a new outcome
//! - Exposed separately from the commands so a UI can poll without risking a second run
//! - Deliberately not a catalog service: it has no use case in the exploration and its
//!   addition must not smuggle behaviour into the contract

use crate::domain::artifact::ArtifactSet;
use crate::domain::outcome::RunOutcome;
use crate::domain::session::ReplaySession;
use crate::domain::value::ReplaySessionId;

/// Read projections over a session.
pub trait ArtifactQuery {
    /// The session's identity.
    fn session_id(&self, session: &ReplaySession) -> &ReplaySessionId;

    /// The run's outcome, if it has run.
    fn outcome<'a>(&self, session: &'a ReplaySession) -> Option<&'a RunOutcome>;

    /// The sealed artifact set, if it has been sealed.
    fn artifact<'a>(&self, session: &'a ReplaySession) -> Option<&'a ArtifactSet>;
}

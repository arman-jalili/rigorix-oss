//! `ArtifactQuery` — the read helper (not a catalog service).
//!
//! @canonical .pi/architecture/modules/portable-session.md#application-layer-application
//! Implements: `ArtifactQuery`
//! Issue: #1297 (application layer)
//!
//! # Contract
//! - Read-only: no method mutates a session, and none can produce a new outcome
//! - Exposed separately from the commands so a UI can poll without risking a second run
//! - Deliberately not a catalog service: it has no use case in the exploration, and
//!   adding behaviour here would smuggle policy into a projection

use crate::domain::artifact::ArtifactSet;
use crate::domain::outcome::RunOutcome;
use crate::domain::session::ReplaySession;
use crate::domain::value::ReplaySessionId;

/// Read projections over a session.
pub trait ArtifactQuery {
    /// The session's identity.
    fn session_id<'a>(&self, session: &'a ReplaySession) -> &'a ReplaySessionId;

    /// The run's outcome, if it has run.
    fn outcome<'a>(&self, session: &'a ReplaySession) -> Option<&'a RunOutcome>;

    /// The sealed artifact set, if it has been sealed.
    fn artifact<'a>(&self, session: &'a ReplaySession) -> Option<&'a ArtifactSet>;
}

/// The shipped implementation: a pure projection over the aggregate.
#[derive(Debug, Default, Clone, Copy)]
pub struct ReplaySessionQuery;

impl ReplaySessionQuery {
    /// Construct the query.
    pub const fn new() -> Self {
        Self
    }
}

impl ArtifactQuery for ReplaySessionQuery {
    fn session_id<'a>(&self, session: &'a ReplaySession) -> &'a ReplaySessionId {
        session.id()
    }

    fn outcome<'a>(&self, session: &'a ReplaySession) -> Option<&'a RunOutcome> {
        session.outcome()
    }

    fn artifact<'a>(&self, session: &'a ReplaySession) -> Option<&'a ArtifactSet> {
        session.sealed()
    }
}

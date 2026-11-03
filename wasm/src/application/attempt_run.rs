//! `AttemptRun` — drive one attempt through the gate (FR-002, FR-004, FR-009, FR-013).
//!
//! @canonical .pi/architecture/modules/portable-session.md#application-services-from-exploration-use-cases
//! Implements: `AttemptRun` (AC 3, 4, 5, 10)
//! Issue: #1297 (application layer)
//!
//! # Contract
//! - Resolves **every** port answer from the transcript before handing the run to the
//!   engine; a missing or failed answer becomes a refusal (invariant 2)
//! - Returns the projected `RunOutcome` with ordered `StepOutcome`s and evidence
//!   references (criteria 9, 10)
//! - No parameter, flag or argument can request an allow: the only inputs are the session,
//!   the action, the ports and the gate (FR-009)
//! - An approval enters as a **claim** inside the fixture data; this service never grants
//!   one and never binds it (the engine binds it, FR-013)
//! - A run-level failure to evaluate is `AttemptError::EngineFailure`, which the interface
//!   layer renders as a refusal

use crate::domain::engine_seam::GateEvaluator;
use crate::domain::errors::AttemptError;
use crate::domain::outcome::RunOutcome;
use crate::domain::ports::Ports;
use crate::domain::services::EvidenceProjection;
use crate::domain::session::ReplaySession;
use crate::domain::value::GatedAction;

/// Runs the session's single attempt.
pub trait AttemptRun {
    /// Run.
    fn execute(
        &self,
        session: &mut ReplaySession,
        action: GatedAction,
        ports: Ports<'_>,
        gate: &mut dyn GateEvaluator,
    ) -> Result<RunOutcome, AttemptError>;
}

/// The shipped implementation.
///
/// The projection is injected because wording and ordering are policy, not plumbing: a
/// test double can assert the service passed the engine's findings through exactly once,
/// in the engine's order.
pub struct AttemptRunService<P: EvidenceProjection> {
    projection: P,
}

impl<P: EvidenceProjection> AttemptRunService<P> {
    /// Construct the service with its projection.
    pub const fn new(projection: P) -> Self {
        Self { projection }
    }

    /// The projection this service applies.
    pub const fn projection(&self) -> &P {
        &self.projection
    }
}

impl<P: EvidenceProjection> AttemptRun for AttemptRunService<P> {
    fn execute(
        &self,
        session: &mut ReplaySession,
        action: GatedAction,
        ports: Ports<'_>,
        gate: &mut dyn GateEvaluator,
    ) -> Result<RunOutcome, AttemptError> {
        // Orchestration only: the aggregate decides, this service hands it the pieces.
        // Deliberately no pre-check that could turn a refusal into a pass, and no catch
        // that could turn a failure into an allow.
        session.attempt(action, ports, gate, &self.projection)
    }
}

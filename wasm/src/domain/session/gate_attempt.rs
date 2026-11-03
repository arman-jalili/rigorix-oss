//! `GateAttempt` — the one proposed run a session may hold.
//!
//! @canonical .pi/architecture/modules/portable-session.md#aggregates
//! Implements: Contract Freeze — GateAttempt
//! Issue: #1294 (contract freeze)

use crate::domain::outcome::RunOutcome;
use crate::domain::value::{GateAttemptId, GatedAction};

/// One proposed run under evaluation.
///
/// # Contract (Frozen)
/// - Identity persists (`GateAttemptId`) and is assigned once, from the id sequence
/// - Never reopened and never rescored: `outcome` is set exactly once, inside
///   [`ReplaySession::attempt`](crate::domain::session::ReplaySession::attempt)
/// - It is **not** a business transaction and its outcome is **not** a business approval
#[derive(Debug, Clone, PartialEq)]
pub struct GateAttempt {
    id: GateAttemptId,
    action: GatedAction,
    outcome: RunOutcome,
}

impl GateAttempt {
    /// Record a completed attempt. Crate-internal: the aggregate is the only caller.
    #[allow(
        dead_code,
        reason = "the aggregate is the sole caller and lands with #1296"
    )]
    pub(crate) fn record(id: GateAttemptId, action: GatedAction, outcome: RunOutcome) -> Self {
        Self {
            id,
            action,
            outcome,
        }
    }

    /// The attempt's identity.
    pub fn id(&self) -> &GateAttemptId {
        &self.id
    }

    /// The action that was proposed.
    pub fn action(&self) -> &GatedAction {
        &self.action
    }

    /// The run's outcome.
    pub fn outcome(&self) -> &RunOutcome {
        &self.outcome
    }
}

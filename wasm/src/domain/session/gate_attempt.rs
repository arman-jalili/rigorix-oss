//! `GateAttempt` — the one proposed run a session may hold.
//!
//! @canonical .pi/architecture/modules/portable-session.md#aggregates
//! Implements: Contract Freeze — GateAttempt
//! Issue: #1294 (contract freeze), #1296 (domain layer)

use crate::domain::artifact::EnvelopeBytes;
use crate::domain::outcome::RunOutcome;
use crate::domain::value::{GateAttemptId, GatedAction, KeyRef};

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
    key_ref: KeyRef,
    envelopes: Vec<EnvelopeBytes>,
}

impl GateAttempt {
    /// Record a completed attempt. Crate-internal: the aggregate is the only caller.
    #[allow(
        dead_code,
        reason = "the aggregate is the sole caller and lands with #1296"
    )]
    pub(crate) fn record(
        id: GateAttemptId,
        action: GatedAction,
        outcome: RunOutcome,
        key_ref: KeyRef,
        envelopes: Vec<EnvelopeBytes>,
    ) -> Self {
        Self {
            id,
            action,
            outcome,
            key_ref,
            envelopes,
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

    /// The opaque reference of the key that signed this run's envelopes.
    pub fn key_ref(&self) -> &KeyRef {
        &self.key_ref
    }

    /// The engine's signed envelopes, byte for byte (never re-encoded).
    pub fn envelopes(&self) -> &[EnvelopeBytes] {
        &self.envelopes
    }
}

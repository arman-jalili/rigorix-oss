//! The engine seam — the one place the crate talks to the gate.
//!
//! @canonical .pi/architecture/decisions/ADR-001-architecture-pattern.md
//! @canonical .pi/architecture/decisions/ADR-011-crate-identity-and-release-train.md
//! Implements: Contract Freeze — GateEvaluator, EngineFinding, EngineFindingOutcome,
//!   EngineEvaluation, EngineBindingError
//! Issue: #1294 (contract freeze); replaced by concrete engine types under FR-011
//!
//! # Why this exists as a port
//!
//! The engine owns the gate. This crate must call it, must not re-model it, and — until
//! FR-011 lands — must not link it: `rigorix-engine` has no wasm-facing feature yet, so
//! a direct dependency would make this contract crate buildable only on a machine where
//! that feature has already been written.
//!
//! So the seam is a **carrier**, not a model. `EngineFinding` holds the fields the
//! record needs to name a decision (which gate, what outcome, which evidence) and puts
//! everything else in an opaque envelope. When the engine's `portable` feature lands,
//! the adapter implements this trait by calling the engine and filling the carrier from
//! the engine's own structs — the trait stays, the duplicate type goes (tracked by
//! ADR-011, not by a rewrite here).
//!
//! # Contract (Frozen)
//! - `evaluate` is the only way a run reaches the gate
//! - A binding failure is a refusal (`AttemptError::EngineFailure`), never an allow
//! - The carrier holds no engine logic: no predicate matching, no policy, no signing
//!   decisions. If a branch here would decide something, it belongs in the engine

use serde::{Deserialize, Serialize};

use crate::domain::outcome::EvidenceRef;
use crate::domain::ports::Ports;
use crate::domain::value::{GateAttemptId, GatedAction, PortFailure, ScenarioRef};

/// The engine's outcome for one gated step.
///
/// # Contract (Frozen)
/// - Four values, matching the engine's three gate outcomes plus "not evaluated"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineFindingOutcome {
    /// The step passed every check.
    Passed,
    /// A check ran and refused.
    Refused,
    /// A check could not run; the engine refuses on error.
    Error,
    /// The gate promoted the step: it awaits a human, and the run may not proceed.
    ///
    /// The third gate outcome had no home here until #1383, so a draft adapter mapped it to
    /// `Error` — the same two-valued slot that turned `Unknown` into `Failed` and `Promote`
    /// into `Passed`. A fail-closed vocabulary is only fail-closed if the third value has
    /// somewhere to go.
    Promoted,
    /// The engine never evaluated this step because an earlier one refused.
    NotReached,
}

/// One finding the engine produced, in evaluation order.
///
/// # Contract (Frozen)
/// - `outcome` and `gate_id` are the engine's; `evidence` is the engine's reference
/// - `opaque` carries the engine's own finding payload for evidence binding; the crate
///   never reads into it
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineFinding {
    /// Stable identifier of the gate that produced this finding.
    pub gate_id: String,
    /// What the engine concluded.
    pub outcome: EngineFindingOutcome,
    /// The engine's refusal kind, when it refused.
    pub refusal: Option<String>,
    /// The evidence the engine produced, if any.
    pub evidence: Option<EvidenceRef>,
    /// The engine's own finding payload, unread by this crate.
    pub opaque: Option<serde_json::Value>,
}

/// Everything one evaluation returned.
///
/// # Contract (Frozen)
/// - `findings` is ordered exactly as the engine evaluated
/// - `envelopes` are the engine's signed bytes, never re-encoded (ADR-010)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineEvaluation {
    /// Ordered findings.
    pub findings: Vec<EngineFinding>,
    /// Signed envelopes, byte for byte.
    pub envelopes: Vec<Vec<u8>>,
    /// Capabilities that could not answer during evaluation.
    ///
    /// The aggregate refuses on a non-empty list even if every finding says
    /// `Passed`: an unavailable check must never read as a passed one (invariant 2),
    /// and trusting the adapter to have handled it would put that guarantee in the
    /// adapter rather than in the aggregate.
    pub port_failures: Vec<PortFailure>,
}

/// What the crate asks the gate to evaluate.
///
/// Two lifetimes on purpose: the data being evaluated (`'a`) is built inside the run and
/// lives for the call, while the capability bundle (`'p`) is borrowed from the host and
/// outlives it. One lifetime would forbid the aggregate from naming its own attempt id.
#[derive(Debug)]
pub struct GateEvaluationRequest<'a, 'p> {
    /// Attempt identity, minted from the `IdSource` port.
    pub attempt_id: &'a GateAttemptId,
    /// The scenario the session pinned.
    pub scenario: &'a ScenarioRef,
    /// The proposed action.
    pub action: &'a GatedAction,
    /// Capabilities the engine may consult. The engine never reaches past these.
    pub ports: Ports<'p>,
}

/// Why the binding could not evaluate a run.
///
/// # Contract (Frozen)
/// - Carries a redacted description, never an engine debug dump with paths in it
/// - Always results in a refusal, never an allow
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineBindingError {
    /// The engine refused to evaluate at all (e.g. it could not bind the scenario).
    #[error("the gate rejected the run before evaluating it: {detail}")]
    Rejected {
        /// Redacted detail. No path, no parameter value.
        detail: String,
    },

    /// The engine panicked or aborted while evaluating.
    ///
    /// On `wasm32-unknown-unknown` this is not catchable (ADR-012 tier 3): the value is
    /// representable for the native test harness, and the browser surfaces a trap.
    #[error("the gate aborted while evaluating the run")]
    Aborted,
}

/// Calls the engine's gate.
///
/// # Contract (Frozen)
/// - Synchronous; the wasm target has no async runtime
/// - Must answer from `ports` for every capability it needs; no ambient access
/// - Must not sign, seal or export: those are the crate's, around the engine's verdict
pub trait GateEvaluator {
    /// Evaluate one run.
    fn evaluate(
        &mut self,
        request: GateEvaluationRequest<'_, '_>,
    ) -> Result<EngineEvaluation, EngineBindingError>;
}

//! `SeamGate` — the engine binding, and what it does before FR-011 exists.
//!
//! @canonical .pi/architecture/decisions/ADR-001-architecture-pattern.md
//! @canonical .pi/architecture/decisions/ADR-011-crate-identity-and-release-train.md
//! Implements: `GateEvaluator` (the engine binding)
//! Issue: #1309 (WasmContract); replaced by the engine call in FR-011
//!
//! # Two implementations, one trait
//!
//! When the engine's `portable` feature lands, this file holds the call into
//! `rigorix_engine`, converting its findings into the carriers `domain::engine_seam`
//! defines. Until it lands, `not_linked()` refuses every run — which is the correct
//! behaviour for a gate that cannot evaluate, and the opposite of an allow.
//!
//! The refusal is deliberately loud in the record: `EngineBindingError::Rejected` becomes
//! `AttemptError::EngineFailure` at the aggregate, which becomes a refusal DTO with kind
//! `gate_unavailable`. A host can see that the gate could not run, which is different from
//! "the gate said no" — the same distinction the engine draws between `unverified` and a
//! failed check.

use crate::domain::engine_seam::{
    EngineBindingError, EngineEvaluation, GateEvaluationRequest, GateEvaluator,
};

/// The engine binding.
#[derive(Debug, Default)]
pub struct SeamGate {
    /// Whether the engine slice is linked into this build.
    linked: bool,
}

impl SeamGate {
    /// A gate that refuses because the engine seam is not in this build (FR-011).
    pub const fn not_linked() -> Self {
        Self { linked: false }
    }

    /// Whether this gate can evaluate.
    pub const fn is_linked(&self) -> bool {
        self.linked
    }
}

impl GateEvaluator for SeamGate {
    fn evaluate(
        &mut self,
        request: GateEvaluationRequest<'_, '_>,
    ) -> Result<EngineEvaluation, EngineBindingError> {
        let _ = (request.attempt_id, request.scenario, request.action);
        if !self.linked {
            return Err(EngineBindingError::Rejected {
                detail: "the engine seam is not linked into this build (FR-011)".to_string(),
            });
        }
        // Unreachable until the seam is linked; kept explicit so the success path cannot
        // be mistaken for "no gate needed".
        Err(EngineBindingError::Aborted)
    }
}

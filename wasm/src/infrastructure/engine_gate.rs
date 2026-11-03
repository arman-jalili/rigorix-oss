//! `EngineGate` — the seam *linked* to the engine's portable slice (#1351, spike only).
//!
//! @canonical .pi/architecture/decisions/ADR-011-crate-identity-and-release-train.md
//! Implements: `GateEvaluator` over `rigorix_engine`'s gate slice
//! Issue: #1351 (M3 — "the bundle with the engine inside it")
//!
//! # This file is measurement apparatus, not the FR-011 adapter
//!
//! The real adapter is Phase B work: it must thread the seam through the gate-path
//! sites the inventory lists, and it must answer every capability from `Ports`. This
//! one exists so the linker cannot dead-code-eliminate the engine: it makes **one**
//! real engine decision — a `StepRequirement` evaluated against the proposed action —
//! and reports it in the carrier the aggregate already understands.
//!
//! It is compiled only under `--features engine`, and it is why the M3 number is the
//! size of a module that *calls* the engine rather than merely depending on it: a
//! declaration alone measured byte-identical to the empty seam (243 366 B), because
//! nothing referenced the crate.
//!
//! # What it proves, and what it does not
//!
//! Proves: the engine's `portable` slice links into a `cdylib` for
//! `wasm32-unknown-unknown` and evaluates a real gate decision there.
//!
//! Does not prove: that the four exported functions route through it. `attempt_run`
//! still builds `SeamGate::not_linked()`; wiring the aggregate is Phase B, and doing
//! it here would have meant editing the contract tests the spike must leave alone.

use serde_json::Value;

use crate::domain::engine_seam::{
    EngineBindingError, EngineEvaluation, EngineFinding, EngineFindingOutcome,
    GateEvaluationRequest, GateEvaluator,
};

use rigorix_engine::sequence_policy::domain::StepRequirement;

/// A `GateEvaluator` that asks the engine's sequence-policy requirement evaluator.
#[derive(Debug, Default)]
pub struct EngineGate;

impl EngineGate {
    /// A linked gate.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl GateEvaluator for EngineGate {
    fn evaluate(
        &mut self,
        request: GateEvaluationRequest<'_, '_>,
    ) -> Result<EngineEvaluation, EngineBindingError> {
        // The requirement arrives in the action's parameters: the spike evaluates
        // what the caller pinned, it does not invent a policy.
        let spec = request.action.params.0.get("requirement").ok_or_else(|| {
            EngineBindingError::Rejected {
                detail: "no `requirement` in the proposed action's parameters".to_string(),
            }
        })?;

        let requirement: StepRequirement =
            serde_json::from_value(spec.clone()).map_err(|error| EngineBindingError::Rejected {
                detail: format!("the requirement does not parse: {error}"),
            })?;

        let parameters: Value = serde_json::to_value(&request.action.params)
            .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));

        let finding = requirement
            .evaluate(
                &request.action.name,
                &request.action.name,
                &parameters,
                false,
            )
            .map_err(|error| EngineBindingError::Rejected {
                detail: format!("the engine refused to evaluate: {error}"),
            })?;

        let findings = match finding {
            Some(finding) => vec![EngineFinding {
                gate_id: "sequence_policy.requirement".to_string(),
                outcome: EngineFindingOutcome::Refused,
                refusal: Some(format!("{:?}", finding.action).to_lowercase()),
                evidence: None,
                opaque: serde_json::to_value(&finding).ok(),
            }],
            None => vec![EngineFinding {
                gate_id: "sequence_policy.requirement".to_string(),
                outcome: EngineFindingOutcome::Passed,
                refusal: None,
                evidence: None,
                opaque: None,
            }],
        };

        Ok(EngineEvaluation {
            findings,
            envelopes: Vec::new(),
            port_failures: Vec::new(),
        })
    }
}

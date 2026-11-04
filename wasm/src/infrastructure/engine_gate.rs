//! `EngineGate` — the crate's call into the engine's one gate composition.
//!
//! @canonical .pi/architecture/decisions/ADR-011-crate-identity-and-release-train.md
//! @canonical .pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md
//! Implements: `GateEvaluator` over `gate::evaluate_gate_sync`
//! Issue: #1383 (Phase C.3); replaces `SeamGate::not_linked()`
//!
//! # What the crate supplies, and what the engine does
//!
//! The crate supplies **ports**; the engine composes the decision. `gate::infrastructure::setup`
//! is a native composition root and stays outside the portable slice, so the four things it
//! builds are the four things a host must answer:
//!
//! | the slice drops | supplied here |
//! |---|---|
//! | `ProcessPreconditionRunner` | [`TranscriptRunner`] — a browser spawns no programs |
//! | `TomlPreconditionRepository` | [`FixturePreconditionRepository`] — rules as data |
//! | `TomlSequencePolicyRepository` | [`FixtureSequencePolicyRepository`] — rules as data |
//! | `HttpProviderClient` | [`TranscriptProviderClient`] — determinations from the fixture |
//!
//! The *services* stay the engine's: `PreconditionServiceImpl` and
//! `SequencePolicyServiceImpl` are in the slice ("one item excluded, not the module"), so
//! the crate supplies rule data and the engine still evaluates it. Calling
//! `PreconditionDispatchGate` plus a sequence service directly would reproduce the
//! composition C.1 moved, and make C.5's conformance claim a coincidence.
//!
//! # The third value keeps getting absorbed
//!
//! Three sites this month: `Unknown` → `Failed` (#1393), `Promote` → `Passed` (#1392), and a
//! draft of this file mapping `GateOutcome::Promoted` → `EngineFindingOutcome::Error`. One
//! pattern — a three-valued thing landing in a two-valued slot, absorbed by the permissive
//! or the terminal value. A fail-closed vocabulary is only fail-closed if the third value
//! has somewhere to go, so `Promoted` is a first-class carrier on both sides of the seam.

#[cfg(not(feature = "engine"))]
use crate::domain::engine_seam::{EngineBindingError, EngineEvaluation};
#[cfg(feature = "engine")]
use crate::domain::engine_seam::{
    EngineBindingError, EngineEvaluation, EngineFinding, EngineFindingOutcome,
    GateEvaluationRequest, GateEvaluator,
};
#[cfg(not(feature = "engine"))]
use crate::domain::engine_seam::{GateEvaluationRequest, GateEvaluator};

/// The crate's gate: one call, and the engine decides.
pub struct EngineGate {
    /// The transcript the four port adapters answer from.
    #[cfg(feature = "engine")]
    transcript: serde_json::Value,
}

impl EngineGate {
    /// Build the gate over the session's fixture world.
    #[cfg(feature = "engine")]
    pub const fn new(transcript: serde_json::Value) -> Self {
        Self { transcript }
    }

    /// The gate with no engine linked — every evaluation refuses, and says why.
    pub const fn unavailable() -> Self {
        Self {
            #[cfg(feature = "engine")]
            transcript: serde_json::Value::Null,
        }
    }
}

impl GateEvaluator for EngineGate {
    fn evaluate(
        &mut self,
        #[cfg_attr(not(feature = "engine"), allow(unused_variables))]
        request: GateEvaluationRequest<'_, '_>,
    ) -> Result<EngineEvaluation, EngineBindingError> {
        #[cfg(not(feature = "engine"))]
        {
            let _ = (
                request.attempt_id,
                request.scenario,
                request.action,
                request.ports,
            );
            Err(EngineBindingError::Rejected {
                detail: "this build was compiled without gate evaluation: the engine slice is not \
                     linked (FR-011)"
                    .to_string(),
            })
        }

        #[cfg(feature = "engine")]
        {
            evaluate_with_engine(self, request)
        }
    }
}

#[cfg(feature = "engine")]
fn evaluate_with_engine(
    gate: &EngineGate,
    request: GateEvaluationRequest<'_, '_>,
) -> Result<EngineEvaluation, EngineBindingError> {
    use rigorix_engine::audit::application::dto::PinnedInputs;
    use rigorix_engine::gate::application::{GatePorts, GateRequest, evaluate_gate_sync};
    use rigorix_engine::gate::domain::{GateError, GateOutcome, GateVerdict, GatedStep};

    let execution_id = uuid::Uuid::parse_str(request.attempt_id.as_str()).map_err(|_| {
        // The execution id is required, never defaulted: a gate that invents one puts a
        // guess inside signed evidence (#1381).
        EngineBindingError::Rejected {
            detail: "the attempt id is not an execution id".to_string(),
        }
    })?;

    let parameters = serde_json::Value::Object(request.action.params.0.clone());
    // The engine takes the three parts separately: the action's `name` is the step identity
    // and the tool it will invoke — this slice has one action per step, so they are equal
    // until a scenario names them apart.
    let step = GatedStep::new(
        request.action.name.clone(),
        request.action.name.clone(),
        parameters,
    );

    let ports = GatePorts {
        precondition: Some(super::engine_ports::precondition_service(&gate.transcript)),
        sequence: Some(super::engine_ports::sequence_service(&gate.transcript)),
    };

    let decision = evaluate_gate_sync(GateRequest {
        step: &step,
        proposed_action: &request.action.name,
        identity_attested: false,
        pins: PinnedInputs::none().with_execution_id(execution_id),
        ports,
        run_context: None,
    })
    .map_err(|error| match error {
        // A port that could not answer is a capability failure, not a binding failure: the
        // aggregate refuses on a port failure even when every finding passed (invariant 2).
        GateError::Precondition { .. } | GateError::Requirements { .. } => {
            EngineBindingError::Rejected {
                detail: error.to_string(),
            }
        }
        GateError::UnpinnedExecution => EngineBindingError::Rejected {
            detail: error.to_string(),
        },
        GateError::NotReady => EngineBindingError::Rejected {
            detail: format!("{error}: the slice's ports must answer on the first poll"),
        },
        GateError::Envelope { .. } => EngineBindingError::Rejected {
            detail: error.to_string(),
        },
    })?;

    let findings = decision
        .findings
        .iter()
        .map(|finding| EngineFinding {
            gate_id: finding.gate_id.as_str().to_string(),
            outcome: match finding.outcome {
                GateOutcome::Passed => EngineFindingOutcome::Passed,
                GateOutcome::Refused => EngineFindingOutcome::Refused,
                // The third value has its own carrier on both sides of the seam.
                GateOutcome::Promoted => EngineFindingOutcome::Promoted,
                // A check that could not answer is an error, and the aggregate refuses on
                // one — the one place `Error` is the honest projection.
                GateOutcome::CouldNotEvaluate => EngineFindingOutcome::Error,
            },
            refusal: match (&finding.outcome, &decision.verdict) {
                (GateOutcome::Refused, GateVerdict::Refuse { reason, .. }) => Some(reason.clone()),
                // Nothing was refused, so there is no refusal to name — in particular a
                // promotion carries none.
                _ => None,
            },
            evidence: None,
            opaque: serde_json::to_value(finding).ok(),
        })
        .collect();

    Ok(EngineEvaluation {
        findings,
        // The engine's signed bytes, never re-encoded (ADR-010). Absent when the host
        // supplied no run context: the engine does not assemble an envelope from nothing.
        envelopes: decision
            .envelope_evidence
            .map(|evidence| vec![evidence.canonical_bytes])
            .unwrap_or_default(),
        port_failures: Vec::new(),
    })
}

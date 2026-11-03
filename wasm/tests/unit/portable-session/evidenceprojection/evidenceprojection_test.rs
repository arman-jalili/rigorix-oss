//! Contract: findings project to steps in engine order, `NotReached` survives (AC 9).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::engine_seam::{
    EngineEvaluation, EngineFinding, EngineFindingOutcome,
};
use rigorix_gate_wasm::domain::outcome::StepDecision;
use rigorix_gate_wasm::domain::services::EvidenceProjection;

#[test]
fn projection_is_a_single_injected_dependency() {
    // One method: the projection cannot grow a second, divergent ordering rule.
    let _project: fn(
        &dyn EvidenceProjection,
        &[EngineFinding],
    ) -> Vec<rigorix_gate_wasm::domain::outcome::StepOutcome> = |p, findings| p.project(findings);
}

#[test]
fn not_reached_is_distinct_from_refused() {
    // Three decisions, not two. Collapsing them is how a record stops explaining itself.
    assert_ne!(StepDecision::NotReached, StepDecision::Refused);
    assert_ne!(StepDecision::NotReached, StepDecision::Allowed);
}

#[test]
fn engine_findings_carry_their_own_outcome_and_evidence() {
    let findings = vec![
        EngineFinding {
            gate_id: "precondition.a".into(),
            outcome: EngineFindingOutcome::Passed,
            refusal: None,
            evidence: None,
            opaque: None,
        },
        EngineFinding {
            gate_id: "precondition.b".into(),
            outcome: EngineFindingOutcome::Refused,
            refusal: Some("precondition_failed".into()),
            evidence: None,
            opaque: None,
        },
        EngineFinding {
            gate_id: "requirement.c".into(),
            outcome: EngineFindingOutcome::NotReached,
            refusal: None,
            evidence: None,
            opaque: None,
        },
    ];
    let evaluation = EngineEvaluation {
        findings,
        envelopes: vec![],
    };
    assert_eq!(evaluation.findings.len(), 3);
    assert_eq!(
        evaluation.findings[2].outcome,
        EngineFindingOutcome::NotReached
    );
    // The carrier is not a model: whatever the engine said is kept verbatim in `refusal`.
    assert_eq!(
        evaluation.findings[1].refusal.as_deref(),
        Some("precondition_failed")
    );
}

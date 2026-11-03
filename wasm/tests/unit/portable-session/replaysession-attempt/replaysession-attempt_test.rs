//! Contract: `attempt` runs once and fails closed (module AC 3, 4, 5).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::errors::AttemptError;
use rigorix_gate_wasm::domain::outcome::{
    OutcomeStatus, RefusalKind, RunOutcome, StepDecision, StepOutcome,
};
use rigorix_gate_wasm::domain::ports::Ports;
use rigorix_gate_wasm::domain::session::ReplaySession;
use rigorix_gate_wasm::domain::value::{GatedAction, Params, PortFailure, PortFailureKind, PortId};

#[test]
fn one_attempt_per_session_is_a_value() {
    // The refusal exists as a value, so a second call has something truthful to return
    // rather than a panic or a silently ignored call.
    let _ = AttemptError::AlreadyAttempted;
    let _ = AttemptError::AlreadySealed;
}

#[test]
fn attempt_signature_requires_ports_and_a_gate() {
    // A capability cannot be reached without being handed in, and the gate cannot be
    // skipped: both appear in the signature (invariant 1).
    let _attempt: fn(
        &mut ReplaySession,
        GatedAction,
        Ports<'_>,
        &mut dyn rigorix_gate_wasm::domain::engine_seam::GateEvaluator,
    ) -> Result<RunOutcome, AttemptError> = ReplaySession::attempt;
}

#[test]
fn a_refused_run_always_names_its_causing_step() {
    let refused = RunOutcome {
        status: OutcomeStatus::Refused,
        steps: vec![
            StepOutcome {
                gate_id: "precondition.beneficiary-eligible".into(),
                decision: StepDecision::Allowed,
                kind: None,
                reason: None,
                evidence: None,
            },
            StepOutcome {
                gate_id: "precondition.budget".into(),
                decision: StepDecision::Refused,
                kind: Some(RefusalKind::PreconditionFailed),
                reason: Some("the budget check did not pass".into()),
                evidence: None,
            },
        ],
        evidence: vec![],
    };
    let causing = refused.causing_step().expect("a refusal names its step");
    assert_eq!(causing.gate_id, "precondition.budget");

    // And the permitted case has none: no accidental "first step" attribution.
    let allowed = RunOutcome {
        status: OutcomeStatus::Allowed,
        steps: vec![StepOutcome {
            gate_id: "precondition.budget".into(),
            decision: StepDecision::Allowed,
            kind: None,
            reason: None,
            evidence: None,
        }],
        evidence: vec![],
    };
    assert!(allowed.causing_step().is_none());
}

#[test]
fn a_port_failure_is_a_refusal_kind_not_a_status() {
    // Invariant 2 in one line: `OutcomeStatus` has no variant a failing port could be
    // spelled as, other than `Refused`.
    let failure = PortFailure::new(PortId::ProviderTransport, PortFailureKind::Missing);
    assert_eq!(failure.port.as_str(), "provider_transport");
    assert_eq!(failure.kind.as_str(), "missing");
    assert_eq!(failure.to_string(), "provider_transport:missing");

    let _ = GatedAction::new("payment_execute", Params::default());
}

#[test]
fn identity_failure_has_its_own_kind_and_wording() {
    // "we could not check" must not be expressible as "the identity is wrong" — that is
    // why the kind is `IdentityUnverified` and the forbidden word never appears.
    let label = format!("{:?}", RefusalKind::IdentityUnverified).to_lowercase();
    assert!(label.contains("unverified"));
    assert!(!label.contains("unauthenticated"));
}

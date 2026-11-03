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
fn attempt_signature_requires_ports_a_gate_and_a_projection() {
    // A capability cannot be reached without being handed in; the gate cannot be skipped;
    // and the ordering rule cannot be re-invented per caller (invariant 1, criterion 9).
    type AttemptFn = fn(
        &mut ReplaySession,
        GatedAction,
        Ports<'_>,
        &mut dyn rigorix_gate_wasm::domain::engine_seam::GateEvaluator,
        &dyn rigorix_gate_wasm::domain::services::EvidenceProjection,
    ) -> Result<RunOutcome, AttemptError>;

    let _attempt: AttemptFn = ReplaySession::attempt;
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

// ---------------------------------------------------------------------------
// Behaviour (issue #1296): criteria 3, 4, 5, 10
// ---------------------------------------------------------------------------

use crate::support::{FailingKey, FakeGate, PortSet, action, digest, inputs};
use rigorix_gate_wasm::domain::outcome::RefusalKind as Kind;
use rigorix_gate_wasm::domain::services::{BusinessWording, EngineFindingsProjection};

fn projection() -> EngineFindingsProjection<BusinessWording> {
    EngineFindingsProjection::new(BusinessWording::new())
}

#[test]
fn an_allowed_run_names_no_cause_and_carries_evidence_order() {
    let mut session = ReplaySession::open(inputs()).expect("open");
    let mut gate = FakeGate::allowing();
    let mut set = PortSet::happy();

    let outcome = session
        .attempt(action(), set.ports(), &mut gate, &projection())
        .expect("attempt");

    assert_eq!(outcome.status, OutcomeStatus::Allowed);
    assert_eq!(outcome.steps.len(), 1);
    assert_eq!(outcome.steps[0].decision, StepDecision::Allowed);
    assert!(outcome.causing_step().is_none());
}

#[test]
fn a_refused_run_names_the_causing_step_and_preserves_not_reached() {
    let mut session = ReplaySession::open(inputs()).expect("open");
    let mut gate = FakeGate::refusing();
    let mut set = PortSet::happy();

    let outcome = session
        .attempt(action(), set.ports(), &mut gate, &projection())
        .expect("attempt");

    assert_eq!(outcome.status, OutcomeStatus::Refused);
    // Order is the engine's order, and `NotReached` was not collapsed into `Refused`.
    assert_eq!(
        outcome.steps.iter().map(|s| s.decision).collect::<Vec<_>>(),
        vec![
            StepDecision::Allowed,
            StepDecision::Refused,
            StepDecision::NotReached
        ]
    );
    let causing = outcome.causing_step().expect("names its cause");
    assert_eq!(causing.gate_id, "precondition.budget");
    assert_eq!(causing.kind, Some(Kind::PreconditionFailed));
    // Evidence travels with the outcome, in step order.
    assert_eq!(outcome.evidence.len(), 1);
    assert_eq!(outcome.evidence[0].digest, digest('7'));
}

#[test]
fn a_second_attempt_is_refused_and_the_first_outcome_is_unchanged() {
    let mut session = ReplaySession::open(inputs()).expect("open");
    let mut gate = FakeGate::allowing();
    let mut set = PortSet::happy();

    let first = session
        .attempt(action(), set.ports(), &mut gate, &projection())
        .expect("first attempt");
    let second = session.attempt(action(), set.ports(), &mut gate, &projection());
    assert_eq!(second.unwrap_err(), AttemptError::AlreadyAttempted);

    // The recorded outcome is still the first one, byte for byte.
    assert_eq!(session.outcome(), Some(&first));
}

#[test]
fn every_port_failure_kind_refuses_and_never_allows() {
    // Criterion 4 as a matrix: each of the four failure kinds, for the port that the
    // aggregate consults itself (the id source) and for a port the gate reports.
    let kinds = [
        PortFailureKind::Missing,
        PortFailureKind::Exhausted,
        PortFailureKind::Malformed,
        PortFailureKind::Failed,
    ];

    for kind in kinds {
        // (a) the gate reports the failure after answering every finding as passed
        let mut session = ReplaySession::open(inputs()).expect("open");
        let mut gate =
            FakeGate::with_port_failure(PortFailure::new(PortId::ProviderTransport, kind));
        let mut set = PortSet::happy();
        let outcome = session
            .attempt(action(), set.ports(), &mut gate, &projection())
            .expect("attempt");
        assert_eq!(
            outcome.status,
            OutcomeStatus::Refused,
            "a passed finding plus an unavailable check must still refuse ({kind:?})"
        );
        let causing = outcome.causing_step().expect("names its cause");
        assert_eq!(causing.gate_id, "port.provider_transport");
        assert_eq!(causing.kind, Some(Kind::PortUnavailable));

        // (b) the identity port cannot answer *before* the gate runs
        let mut session = ReplaySession::open(inputs()).expect("open");
        let mut gate = FakeGate::allowing();
        let mut set = PortSet::happy();
        set.key = FailingKey(kind).into_key();
        let outcome = session
            .attempt(action(), set.ports(), &mut gate, &projection())
            .expect("attempt");
        assert_eq!(outcome.status, OutcomeStatus::Refused);
        assert_eq!(
            outcome.causing_step().expect("cause").kind,
            Some(Kind::IdentityUnverified)
        );
    }
}

#[test]
fn an_engine_binding_failure_is_an_error_not_an_allow() {
    let mut session = ReplaySession::open(inputs()).expect("open");
    let mut gate = FakeGate::aborting();
    let mut set = PortSet::happy();

    let result = session.attempt(action(), set.ports(), &mut gate, &projection());
    assert!(matches!(result, Err(AttemptError::EngineFailure { .. })));
    // Nothing was recorded as an outcome: there is no allow to find.
    assert!(session.outcome().is_none());
}

// ---------------------------------------------------------------------------
// Fault matrix (issue #1300): criterion 4 — six ports × four kinds
// ---------------------------------------------------------------------------

#[test]
fn the_full_fault_matrix_refuses_every_case() {
    use crate::support::{AnyIds, FailingIds};
    use rigorix_gate_wasm::domain::value::PortFailure;

    let kinds = [
        PortFailureKind::Missing,
        PortFailureKind::Exhausted,
        PortFailureKind::Malformed,
        PortFailureKind::Failed,
    ];
    let ports = [
        PortId::Clock,
        PortId::IdSource,
        PortId::Randomness,
        PortId::KeySource,
        PortId::ProviderTransport,
        PortId::BusinessSink,
    ];

    let mut cases = 0;
    let mut allowed = Vec::new();

    for port in ports {
        for kind in kinds {
            cases += 1;

            // The port the aggregate consults itself: the id source.
            if port == PortId::IdSource {
                let mut session = ReplaySession::open(inputs()).expect("open");
                let mut gate = FakeGate::allowing();
                let mut set = PortSet::happy();
                set.ids = AnyIds::Failing(FailingIds(kind));
                let outcome = session
                    .attempt(action(), set.ports(), &mut gate, &projection())
                    .expect("attempt");
                if outcome.status != OutcomeStatus::Refused {
                    allowed.push(format!("{port:?}/{kind:?}"));
                }
                continue;
            }

            // The port the aggregate consults before the gate: the key source.
            if port == PortId::KeySource {
                let mut session = ReplaySession::open(inputs()).expect("open");
                let mut gate = FakeGate::allowing();
                let mut set = PortSet::happy();
                set.key = FailingKey(kind).into_key();
                let outcome = session
                    .attempt(action(), set.ports(), &mut gate, &projection())
                    .expect("attempt");
                if outcome.status != OutcomeStatus::Refused {
                    allowed.push(format!("{port:?}/{kind:?}"));
                }
                continue;
            }

            // Every other port is reported by the gate, which answers with all findings
            // passed — the strictest version of the case: an unavailable check plus a
            // green gate must still refuse.
            let mut session = ReplaySession::open(inputs()).expect("open");
            let mut gate = FakeGate::with_port_failure(PortFailure::new(port, kind));
            let mut set = PortSet::happy();
            let outcome = session
                .attempt(action(), set.ports(), &mut gate, &projection())
                .expect("attempt");
            if outcome.status != OutcomeStatus::Refused {
                allowed.push(format!("{port:?}/{kind:?}"));
            }
        }
    }

    assert_eq!(cases, 24, "six ports × four failure kinds");
    assert!(allowed.is_empty(), "these cases allowed a run: {allowed:?}");
}

#[test]
fn a_failed_first_attempt_leaves_the_outcome_readable_and_unsealed() {
    // A refused run is still a decision: it can be read and sealed (the record of a
    // refusal is the point of the crate). What it cannot be is re-run.
    let mut session = ReplaySession::open(inputs()).expect("open");
    let mut gate = FakeGate::refusing();
    let mut set = PortSet::happy();

    let outcome = session
        .attempt(action(), set.ports(), &mut gate, &projection())
        .expect("attempt");
    assert_eq!(outcome.status, OutcomeStatus::Refused);
    assert_eq!(session.outcome(), Some(&outcome));

    let mut gate_again = FakeGate::allowing();
    assert_eq!(
        session
            .attempt(action(), set.ports(), &mut gate_again, &projection())
            .unwrap_err(),
        AttemptError::AlreadyAttempted
    );
}

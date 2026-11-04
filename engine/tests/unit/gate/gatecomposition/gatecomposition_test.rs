//! The gate composition: one entry point, one order, one digest.
//!
//! Issue: #1381 (Phase C.1)
//!
//! These tests are the reason the ordering moved into the engine: they call
//! `evaluate_gate` directly with fixture ports and assert the *decision*, which is
//! something no caller-side composition could be held to.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use uuid::Uuid;

use rigorix_engine::audit::application::dto::PinnedInputs;
use rigorix_engine::gate::application::{
    GateDecision, GateError, GateFinding, GateId, GateOutcome, GatePorts, GateRequest, GateVerdict,
    GatedStep, evaluate_gate,
};
use rigorix_engine::precondition::application::dto::DispatchStep;
use rigorix_engine::precondition::application::service::PreconditionService;
use rigorix_engine::precondition::domain::{
    PreconditionError, PreconditionFinding, PreconditionOutcome, PreconditionVerdict,
};
use rigorix_engine::sequence_policy::application::dto::PlannedStep;
use rigorix_engine::sequence_policy::application::service::SequencePolicyService;
use rigorix_engine::sequence_policy::domain::{RequirementAction, RequirementFinding};

const EXEC: Uuid = Uuid::from_u128(0x018f_5b8e_0000_7000_8000_0000_0000_0001);

fn pins() -> PinnedInputs {
    PinnedInputs::none()
        .with_execution_id(EXEC)
        .with_timestamp(Utc.with_ymd_and_hms(2026, 11, 3, 12, 0, 0).unwrap())
}

fn step() -> GatedStep {
    GatedStep::new(
        "transfer",
        "payments.transfer",
        serde_json::json!({"amount": 10}),
    )
}

/// Records the order in which the composition consulted each port.
#[derive(Default)]
struct OrderLog(Mutex<Vec<&'static str>>);

#[derive(Clone, Copy)]
enum PreconditionAnswer {
    Pass,
    Deny,
}

struct StubPreconditions {
    answer: PreconditionAnswer,
    order: Arc<OrderLog>,
}

#[async_trait]
impl PreconditionService for StubPreconditions {
    async fn evaluate(
        &self,
        _execution_id: Uuid,
        _step: &DispatchStep,
    ) -> Result<PreconditionVerdict, PreconditionError> {
        Ok(match self.answer {
            PreconditionAnswer::Pass => PreconditionVerdict::Dispatch,
            PreconditionAnswer::Deny => PreconditionVerdict::Deny {
                precondition_id: "no-unguarded-transfer".to_string(),
                outcome: PreconditionOutcome::Failed,
            },
        })
    }

    async fn is_configured(&self) -> Result<bool, PreconditionError> {
        Ok(true)
    }

    async fn evaluate_with_findings(
        &self,
        _execution_id: Uuid,
        step: &DispatchStep,
    ) -> Result<(PreconditionVerdict, Vec<PreconditionFinding>), PreconditionError> {
        self.order.0.lock().unwrap().push("preconditions");
        let outcome = match self.answer {
            PreconditionAnswer::Pass => PreconditionOutcome::Passed,
            PreconditionAnswer::Deny => PreconditionOutcome::Failed,
        };
        let verdict = match self.answer {
            PreconditionAnswer::Pass => PreconditionVerdict::Dispatch,
            PreconditionAnswer::Deny => PreconditionVerdict::Deny {
                precondition_id: "no-unguarded-transfer".to_string(),
                outcome,
            },
        };
        Ok((
            verdict,
            vec![PreconditionFinding {
                precondition_id: "no-unguarded-transfer".to_string(),
                step: step.name.clone(),
                outcome,
                exit_code: Some(1),
                inputs_hash: "hash".to_string(),
                checked_at: Utc.with_ymd_and_hms(2026, 11, 3, 12, 0, 0).unwrap(),
                summary: Some("the transfer guard was not satisfied".to_string()),
                check_digest: None,
                authority_digest: None,
                attribution: Default::default(),
                check_writable: None,
            }],
        ))
    }
}

#[derive(Clone, Copy)]
enum RequirementAnswer {
    Promote,
    Deny,
}

struct StubRequirements {
    answer: RequirementAnswer,
    order: Arc<OrderLog>,
}

#[async_trait]
impl SequencePolicyService for StubRequirements {
    async fn evaluate_plan(
        &self,
        _steps: &[PlannedStep],
        _principal: Option<&str>,
    ) -> Result<
        Vec<rigorix_engine::sequence_policy::domain::SequenceMatch>,
        rigorix_engine::sequence_policy::domain::SequencePolicyError,
    > {
        Ok(Vec::new())
    }

    async fn evaluate_prefix(
        &self,
        _prefix: &[rigorix_engine::sequence_policy::application::dto::DispatchedStep],
        _next: &PlannedStep,
        _principal: Option<&str>,
    ) -> Result<
        Vec<rigorix_engine::sequence_policy::domain::SequenceMatch>,
        rigorix_engine::sequence_policy::domain::SequencePolicyError,
    > {
        Ok(Vec::new())
    }

    async fn evaluate_requirements(
        &self,
        steps: &[PlannedStep],
        _identity_attested: bool,
    ) -> Result<Vec<RequirementFinding>, rigorix_engine::sequence_policy::domain::SequencePolicyError>
    {
        self.order.0.lock().unwrap().push("requirements");
        Ok(vec![RequirementFinding {
            requirement_id: "require-approval".to_string(),
            requirement_name: "a human approves money movement".to_string(),
            step: steps[0].name.clone(),
            unmet_identity: false,
            unmet_params: vec!["approval_id".to_string()],
            unmet_companion: None,
            action: match self.answer {
                RequirementAnswer::Promote => RequirementAction::Promote,
                RequirementAnswer::Deny => RequirementAction::Deny,
            },
        }])
    }
}

fn fixture_ports(
    preconditions: PreconditionAnswer,
    requirements: RequirementAnswer,
) -> (GatePorts, Arc<OrderLog>) {
    let order = Arc::new(OrderLog::default());
    let ports = GatePorts {
        precondition: Some(Arc::new(StubPreconditions {
            answer: preconditions,
            order: Arc::clone(&order),
        })),
        sequence: Some(Arc::new(StubRequirements {
            answer: requirements,
            order: Arc::clone(&order),
        })),
    };
    (ports, order)
}

async fn decide(
    preconditions: PreconditionAnswer,
    requirements: RequirementAnswer,
) -> GateDecision {
    let (ports, _) = fixture_ports(preconditions, requirements);
    let step = step();
    evaluate_gate(GateRequest {
        step: &step,
        proposed_action: "move 10 units",
        identity_attested: true,
        pins: pins(),
        ports,
    })
    .await
    .expect("the gate answers")
}

#[tokio::test]
async fn a_passing_pair_dispatches_and_carries_both_findings() {
    let decision = decide(PreconditionAnswer::Pass, RequirementAnswer::Promote).await;

    assert!(decision.verdict.permits_dispatch());
    assert_eq!(decision.findings.len(), 2);
    assert_eq!(decision.findings[0].gate_id, GateId::Preconditions);
    assert_eq!(decision.findings[0].outcome, GateOutcome::Passed);
    assert_eq!(decision.findings[1].gate_id, GateId::Requirements);
    // A `promote` is evidence, not a refusal: the step still dispatches with the
    // operator's promotion recorded.
    assert_eq!(decision.findings[1].outcome, GateOutcome::Passed);
}

#[tokio::test]
async fn the_order_is_preconditions_then_requirements() {
    let (ports, order) = fixture_ports(PreconditionAnswer::Pass, RequirementAnswer::Promote);
    let step = step();
    let _ = evaluate_gate(GateRequest {
        step: &step,
        proposed_action: "move 10 units",
        identity_attested: true,
        pins: pins(),
        ports,
    })
    .await
    .expect("the gate answers");

    // SEED: swap the two blocks in `compose.rs` and this reads ["requirements",
    // "preconditions"] — the ordering is asserted, not incidental.
    assert_eq!(
        *order.0.lock().unwrap(),
        vec!["preconditions", "requirements"]
    );
}

#[tokio::test]
async fn a_refusal_precondition_is_not_displaced_by_a_requirement_promotion() {
    let decision = decide(PreconditionAnswer::Deny, RequirementAnswer::Promote).await;

    match &decision.verdict {
        GateVerdict::Refuse { gate, rule_id, .. } => {
            assert_eq!(*gate, GateId::Preconditions);
            assert_eq!(rule_id.as_deref(), Some("no-unguarded-transfer"));
        }
        GateVerdict::Dispatch => panic!("a denied precondition must refuse the step"),
    }
    // The requirements were never consulted: a step that cannot dispatch has no
    // obligations to satisfy, and asking would produce evidence about a run that
    // cannot happen.
    assert_eq!(decision.findings.len(), 1);
    assert_eq!(decision.findings[0].gate_id, GateId::Preconditions);
}

#[tokio::test]
async fn a_denied_requirement_refuses_after_the_precondition_passed() {
    let decision = decide(PreconditionAnswer::Pass, RequirementAnswer::Deny).await;

    match &decision.verdict {
        GateVerdict::Refuse { gate, rule_id, .. } => {
            assert_eq!(*gate, GateId::Requirements);
            assert_eq!(rule_id.as_deref(), Some("require-approval"));
        }
        GateVerdict::Dispatch => panic!("a denied requirement must refuse the step"),
    }
    assert_eq!(decision.findings.len(), 2);
}

#[tokio::test]
async fn the_same_request_twice_produces_the_same_digest() {
    // The determinism claim, as a value: no wall clock, no map iteration, no ids
    // minted inside the gate.
    let first: GateDecision = decide(PreconditionAnswer::Pass, RequirementAnswer::Deny).await;
    let second: GateDecision = decide(PreconditionAnswer::Pass, RequirementAnswer::Deny).await;

    assert_eq!(
        first.envelope.decision_digest,
        second.envelope.decision_digest
    );
    assert_eq!(first.envelope.execution_id, EXEC);
    assert!(first.envelope.pinned);
    assert_eq!(
        first.envelope.decision_digest.len(),
        64,
        "SHA-256 hex is 64 characters"
    );
    assert_eq!(first.findings, second.findings);
}

#[tokio::test]
async fn a_different_instant_still_produces_the_same_decision_digest_but_records_the_instant() {
    // The digest covers the decision, not the pins: the instant travels beside it.
    // Two decisions at different instants are the same decision; a verifier replays
    // with the recorded instant (spec 05-evidence-model).
    let step = step();
    let (ports, _) = fixture_ports(PreconditionAnswer::Pass, RequirementAnswer::Promote);
    let first = evaluate_gate(GateRequest {
        step: &step,
        proposed_action: "move 10 units",
        identity_attested: true,
        pins: pins(),
        ports: ports.clone(),
    })
    .await
    .expect("the gate answers");

    let (ports_later, _) = fixture_ports(PreconditionAnswer::Pass, RequirementAnswer::Promote);
    let later_pins = PinnedInputs::none()
        .with_execution_id(EXEC)
        .with_timestamp(Utc.with_ymd_and_hms(2026, 11, 3, 18, 51, 18).unwrap());
    let second = evaluate_gate(GateRequest {
        step: &step,
        proposed_action: "move 10 units",
        identity_attested: true,
        pins: later_pins,
        ports: ports_later,
    })
    .await
    .expect("the gate answers");

    assert_eq!(
        first.envelope.decision_digest,
        second.envelope.decision_digest
    );
    assert_ne!(
        first.envelope.pinned_timestamp,
        second.envelope.pinned_timestamp
    );
}

#[tokio::test]
async fn no_ports_means_dispatch_with_no_findings() {
    // Absence is a decision the operator made, not a missing wiring.
    let step = step();
    let decision = evaluate_gate(GateRequest {
        step: &step,
        proposed_action: "move 10 units",
        identity_attested: false,
        pins: pins(),
        ports: GatePorts::none(),
    })
    .await
    .expect("the gate answers");

    assert!(decision.verdict.permits_dispatch());
    assert!(decision.findings.is_empty());
}

#[tokio::test]
async fn an_unpinned_execution_is_refused_rather_than_defaulted() {
    // A nil-UUID default would collide two runs in one record.
    let step = step();
    let error = evaluate_gate(GateRequest {
        step: &step,
        proposed_action: "move 10 units",
        identity_attested: true,
        pins: PinnedInputs::none(),
        ports: GatePorts::none(),
    })
    .await
    .expect_err("an unpinned request has no reproducible identity");

    assert_eq!(error, GateError::UnpinnedExecution);
}

/// The evidence carries no parameter value (SpanPrivacy) and names the refusing rule.
#[tokio::test]
async fn a_refusal_finding_names_its_rule_and_leaks_no_parameters() {
    let decision = decide(PreconditionAnswer::Deny, RequirementAnswer::Promote).await;
    let finding: &GateFinding = &decision.findings[0];

    assert_eq!(finding.rule_id.as_deref(), Some("no-unguarded-transfer"));
    let serialised = serde_json::to_string(finding).expect("a finding serialises");
    assert!(
        !serialised.contains("amount"),
        "findings must not carry parameter values: {serialised}"
    );
}

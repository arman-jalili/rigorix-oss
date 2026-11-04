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
use rigorix_engine::gate::application::{GatePorts, GateRequest, evaluate_gate};
use rigorix_engine::gate::domain::{
    GateDecision, GateError, GateFinding, GateId, GateOutcome, GateVerdict, GatedStep,
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
        run_context: None,
    })
    .await
    .expect("the gate answers")
}

#[tokio::test]
async fn a_promotion_is_not_a_pass_and_is_not_a_dispatch() {
    // #1392: before this, a `promote` rule was recorded as `Passed` and the verdict stayed
    // `Dispatch` — the operator asked for human approval and got "it passed".
    let decision = decide(PreconditionAnswer::Pass, RequirementAnswer::Promote).await;

    assert!(
        !decision.verdict.permits_dispatch(),
        "a promoted step must not dispatch"
    );
    assert!(decision.verdict.awaits_approval());
    match &decision.verdict {
        GateVerdict::Promote { gate, rule_id, .. } => {
            assert_eq!(*gate, GateId::Requirements);
            assert_eq!(rule_id, "require-approval");
        }
        other => panic!("expected a promotion, got {other:?}"),
    }
    assert_eq!(decision.findings.len(), 2);
    assert_eq!(decision.findings[0].gate_id, GateId::Preconditions);
    assert_eq!(decision.findings[0].outcome, GateOutcome::Passed);
    assert_eq!(decision.findings[1].gate_id, GateId::Requirements);
    assert_eq!(
        decision.findings[1].outcome,
        GateOutcome::Promoted,
        "the finding names the third outcome, not `Passed`"
    );
}

#[tokio::test]
async fn a_promotion_changes_the_decision_digest() {
    // Two decisions that differ only by a promotion must not be the same decision: the
    // digest covers the verdict, and the verdict now has three values.
    let promoted = decide(PreconditionAnswer::Pass, RequirementAnswer::Promote).await;
    let denied = decide(PreconditionAnswer::Pass, RequirementAnswer::Deny).await;
    let plain = rigorix_engine::gate::application::evaluate_gate(GateRequest {
        step: &step(),
        proposed_action: "move 10 units",
        identity_attested: true,
        pins: pins(),
        ports: GatePorts::none(),
        run_context: None,
    })
    .await
    .expect("the gate answers");

    assert_ne!(
        promoted.envelope.decision_digest, plain.envelope.decision_digest,
        "a promotion must not digest like a plain dispatch"
    );
    assert_ne!(
        promoted.envelope.decision_digest,
        denied.envelope.decision_digest
    );
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
        run_context: None,
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
        // Both non-refusals are listed rather than caught by a wildcard: a fourth verdict
        // must fail to compile here too, or this test stops being a test.
        GateVerdict::Dispatch | GateVerdict::Promote { .. } => {
            panic!("a denied precondition must refuse the step")
        }
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
        GateVerdict::Dispatch | GateVerdict::Promote { .. } => {
            panic!("a denied requirement must refuse the step")
        }
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
        run_context: None,
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
        run_context: None,
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
        run_context: None,
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
        run_context: None,
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

// ---------------------------------------------------------------------------
// C.1b (#1389): the sync façade, and the envelope step
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::Poll;

use rigorix_engine::configuration::domain::Secret;
use rigorix_engine::gate::application::{RunContext, evaluate_gate_sync};

/// A port whose `evaluate` awaits something — the seed for `NotReady`.
struct AwaitingPreconditions {
    polls: Arc<AtomicUsize>,
}

#[async_trait]
impl PreconditionService for AwaitingPreconditions {
    async fn evaluate(
        &self,
        _execution_id: Uuid,
        _step: &DispatchStep,
    ) -> Result<PreconditionVerdict, PreconditionError> {
        Ok(PreconditionVerdict::Dispatch)
    }

    async fn is_configured(&self) -> Result<bool, PreconditionError> {
        Ok(true)
    }

    async fn evaluate_with_findings(
        &self,
        _execution_id: Uuid,
        _step: &DispatchStep,
    ) -> Result<(PreconditionVerdict, Vec<PreconditionFinding>), PreconditionError> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        // Yields at least once: this is what a port that awaits real I/O looks like from
        // the composition's side.
        tokio::task::yield_now().await;
        Ok((PreconditionVerdict::Dispatch, Vec::new()))
    }
}

#[tokio::test]
async fn the_sync_facade_agrees_with_the_async_composition() {
    let step = step();
    let (ports, _) = fixture_ports(PreconditionAnswer::Pass, RequirementAnswer::Deny);
    let awaited = rigorix_engine::gate::application::evaluate_gate(GateRequest {
        step: &step,
        proposed_action: "move 10 units",
        identity_attested: true,
        pins: pins(),
        ports,
        run_context: None,
    })
    .await
    .expect("the composition answers");

    let (ports, _) = fixture_ports(PreconditionAnswer::Pass, RequirementAnswer::Deny);
    let synced = evaluate_gate_sync(GateRequest {
        step: &step,
        proposed_action: "move 10 units",
        identity_attested: true,
        pins: pins(),
        ports,
        run_context: None,
    })
    .expect("the façade answers");

    assert_eq!(awaited.verdict, synced.verdict);
    assert_eq!(awaited.findings, synced.findings);
    assert_eq!(
        awaited.envelope.decision_digest,
        synced.envelope.decision_digest
    );
}

#[tokio::test]
async fn a_port_that_awaits_is_reported_not_ready() {
    // SEED: replace `fixture_ports` with this port and the façade must say NotReady —
    // not hang: the test would time out, and not panic: ADR-012 forbids the panic.
    let polls = Arc::new(AtomicUsize::new(0));
    let ports = GatePorts {
        precondition: Some(Arc::new(AwaitingPreconditions {
            polls: Arc::clone(&polls),
        })),
        sequence: None,
    };
    let step = step();
    let error = evaluate_gate_sync(GateRequest {
        step: &step,
        proposed_action: "move 10 units",
        identity_attested: true,
        pins: pins(),
        ports,
        run_context: None,
    })
    .expect_err("a port that awaits cannot answer on the first poll");

    assert_eq!(error, GateError::NotReady);
    assert_eq!(polls.load(Ordering::SeqCst), 1, "polled exactly once");
}

/// The property the crate's bridge rests on: every port the slice can construct resolves
/// on its first poll. Asserted by polling, not by awaiting.
#[test]
fn the_slice_ports_are_ready_on_the_first_poll() {
    use std::task::{Context, Waker};

    let step = step();
    let (ports, _) = fixture_ports(PreconditionAnswer::Pass, RequirementAnswer::Promote);
    let mut future = Box::pin(rigorix_engine::gate::application::evaluate_gate(
        GateRequest {
            step: &step,
            proposed_action: "move 10 units",
            identity_attested: true,
            pins: pins(),
            ports,
            run_context: None,
        },
    ));
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);

    match future.as_mut().poll(&mut cx) {
        // The property under test is *readiness*, not the verdict: these fixture ports
        // promote, so the decision is a promotion and dispatch is beside the point.
        Poll::Ready(Ok(decision)) => assert_ne!(
            decision.envelope.decision_digest.len(),
            0,
            "a first poll that returns a decision has evaluated everything"
        ),
        Poll::Ready(Err(error)) => panic!("the fixture ports must answer, got {error}"),
        Poll::Pending => panic!(
            "a port on the slice's gate path awaited I/O — evaluate_gate_sync would return \
             NotReady, and the browser has no executor to drive it"
        ),
    }
}

fn run_context(signing_key: Option<&str>) -> RunContext {
    RunContext {
        template_id: "pinned-refusal".to_string(),
        planning_prompt: "refuse the step: precondition beneficiary-eligible failed".to_string(),
        source: Some("rigorix_cli".to_string()),
        repository: Some("arman-jalili/rigorix-oss".to_string()),
        author: Some("m2".to_string()),
        identity: None,
        total_tokens: 0,
        duration_ms: 7,
        git_commit: Some("0000000000000000000000000000000000000000".to_string()),
        git_branch: Some("main".to_string()),
        signing_key: signing_key.map(Secret::new),
    }
}

#[tokio::test]
async fn a_run_context_produces_a_signed_envelope() {
    let step = step();
    let (ports, _) = fixture_ports(PreconditionAnswer::Deny, RequirementAnswer::Promote);
    let decision = rigorix_engine::gate::application::evaluate_gate(GateRequest {
        step: &step,
        proposed_action: "move 10 units",
        identity_attested: true,
        pins: pins(),
        ports,
        run_context: Some(run_context(Some("rigorix-dev-envelope-hmac-key"))),
    })
    .await
    .expect("the composition answers");

    let evidence = decision
        .envelope_evidence
        .as_ref()
        .expect("a run context produces a record");
    assert!(
        evidence.envelope.signature.is_some(),
        "a supplied key source signs the record"
    );
    assert_eq!(
        evidence.envelope_hash,
        rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl::envelope_hash(
            &evidence.envelope
        )
        .expect("the hash is computable"),
        "the recorded hash is the one a verifier recomputes"
    );
    assert_eq!(
        evidence.canonical_bytes,
        rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl::canonical_envelope_bytes(
            &evidence.envelope
        )
        .expect("the bytes are canonical"),
        "the recorded bytes are the bytes the signature covers (ADR-010)"
    );
    assert_eq!(evidence.envelope.execution_id, EXEC);
    // The refusal is recorded as the failure it is, not as a pass with findings.
    assert_eq!(evidence.envelope.events.len(), 1);
}

#[tokio::test]
async fn without_a_run_context_the_decision_carries_the_digest_alone() {
    let decision = decide(PreconditionAnswer::Pass, RequirementAnswer::Promote).await;
    assert!(decision.envelope_evidence.is_none());
    assert_eq!(decision.envelope.decision_digest.len(), 64);
}

#[tokio::test]
async fn both_paths_produce_the_same_bytes_and_the_same_digest() {
    // Two calls, one through the async composition and one through the façade, with the
    // same facts: identical canonical bytes. This is the property that makes "the CLI and
    // the crate assemble the record" one statement rather than two implementations.
    let step = step();
    let (ports, _) = fixture_ports(PreconditionAnswer::Deny, RequirementAnswer::Promote);
    let async_decision = rigorix_engine::gate::application::evaluate_gate(GateRequest {
        step: &step,
        proposed_action: "move 10 units",
        identity_attested: true,
        pins: pins(),
        ports,
        run_context: Some(run_context(Some("rigorix-dev-envelope-hmac-key"))),
    })
    .await
    .expect("the composition answers");

    let (ports, _) = fixture_ports(PreconditionAnswer::Deny, RequirementAnswer::Promote);
    let sync_decision = evaluate_gate_sync(GateRequest {
        step: &step,
        proposed_action: "move 10 units",
        identity_attested: true,
        pins: pins(),
        ports,
        run_context: Some(run_context(Some("rigorix-dev-envelope-hmac-key"))),
    })
    .expect("the façade answers");

    let a = async_decision.envelope_evidence.expect("a record");
    let b = sync_decision.envelope_evidence.expect("a record");
    assert_eq!(a.canonical_bytes, b.canonical_bytes);
    assert_eq!(a.envelope_hash, b.envelope_hash);
    assert_eq!(
        async_decision.envelope.decision_digest,
        sync_decision.envelope.decision_digest
    );
}

#[tokio::test]
async fn a_different_key_signs_the_same_decision_differently() {
    // The key's effect, isolated: the decision is identical (digest), the record differs
    // only where the signature is. A test that only compared two envelopes built with the
    // same key could not tell "signed" from "signed by this key".
    let step = step();
    let mut records = Vec::new();
    for key in ["rigorix-dev-envelope-hmac-key", "another-key-entirely"] {
        let (ports, _) = fixture_ports(PreconditionAnswer::Deny, RequirementAnswer::Promote);
        let decision = evaluate_gate_sync(GateRequest {
            step: &step,
            proposed_action: "move 10 units",
            identity_attested: true,
            pins: pins(),
            ports,
            run_context: Some(run_context(Some(key))),
        })
        .expect("the façade answers");
        records.push(decision);
    }

    assert_eq!(
        records[0].envelope.decision_digest, records[1].envelope.decision_digest,
        "the decision does not depend on the key"
    );
    let a = records[0].envelope_evidence.as_ref().expect("a record");
    let b = records[1].envelope_evidence.as_ref().expect("a record");
    assert_ne!(
        a.envelope.signature, b.envelope.signature,
        "the signature does depend on the key"
    );
}

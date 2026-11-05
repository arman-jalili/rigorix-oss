//! Contract: `AttemptRun` orchestrates; it cannot request an allow (AC 10; FR-009).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::application::AttemptRun;
use rigorix_gate_wasm::domain::engine_seam::GateEvaluator;
use rigorix_gate_wasm::domain::errors::AttemptError;
use rigorix_gate_wasm::domain::outcome::RunOutcome;
use rigorix_gate_wasm::domain::ports::Ports;
use rigorix_gate_wasm::domain::services::EvidenceProjection;
use rigorix_gate_wasm::domain::session::ReplaySession;
use rigorix_gate_wasm::domain::value::GatedAction;

struct NoProjection;

impl EvidenceProjection for NoProjection {
    fn project(
        &self,
        findings: &[rigorix_gate_wasm::domain::engine_seam::EngineFinding],
    ) -> Vec<rigorix_gate_wasm::domain::outcome::StepOutcome> {
        assert!(findings.is_empty(), "no findings in a contract test");
        Vec::new()
    }
}

#[test]
fn execute_has_no_way_to_ask_for_an_allow() {
    // Five parameters, none of which is a verdict, a flag, an override or an approval.
    // This is the structural half of "the host cannot influence a decision".
    fn assert_signature<S: AttemptRun>(
        service: &S,
        session: &mut ReplaySession,
        action: GatedAction,
        ports: Ports<'_>,
        gate: &mut dyn GateEvaluator,
    ) -> Result<RunOutcome, AttemptError> {
        service.execute(session, action, ports, gate)
    }
    let _ = assert_signature::<rigorix_gate_wasm::application::AttemptRunService<NoProjection>>;
}

#[test]
fn the_service_is_constructed_with_its_projection() {
    // The projection is injected once, at construction — not passed per call, so it
    // cannot vary between the run and its evidence.
    let service = rigorix_gate_wasm::application::AttemptRunService::new(NoProjection);
    let _ = service.projection();
}

#[test]
fn engine_failure_is_an_error_not_an_allow() {
    use rigorix_gate_wasm::domain::engine_seam::EngineBindingError;
    let failure = AttemptError::EngineFailure {
        source: EngineBindingError::Rejected {
            detail: "scenario version mismatch".into(),
        },
    };
    assert!(failure.to_string().contains("could not evaluate"));
    assert!(!failure.to_string().to_lowercase().contains("allowed"));
}

// ---------------------------------------------------------------------------
// Behaviour (issue #1297)
// ---------------------------------------------------------------------------

use crate::support::{
    AllowPolicy, FailingKey, PortSet, RunnerStubGate, TestStamp, action, digest, inputs,
};
use rigorix_gate_wasm::application::{
    ArtifactQuery, ExportArtifactSet, ExportArtifactSetService, OpenReplaySession,
    OpenReplaySessionService, ReplaySessionQuery,
};
use rigorix_gate_wasm::domain::outcome::{OutcomeStatus, RefusalKind};
use rigorix_gate_wasm::domain::services::{BusinessWording, EngineFindingsProjection};

fn projection() -> EngineFindingsProjection<BusinessWording> {
    EngineFindingsProjection::new(BusinessWording::new())
}

#[test]
fn open_then_run_then_export_is_one_straight_use_case_chain() {
    let opener = OpenReplaySessionService::new();
    let mut session = opener
        .execute(rigorix_gate_wasm::application::OpenSessionInput {
            scenario: inputs().scenario,
            fixtures: inputs().fixtures,
            clock: inputs().clock,
            seed: inputs().seed,
        })
        .expect("open");

    let runner = rigorix_gate_wasm::application::AttemptRunService::new(projection());
    let mut gate = RunnerStubGate::allowing();
    let mut set = PortSet::happy();
    let outcome = runner
        .execute(&mut session, action(), set.ports(), &mut gate)
        .expect("run");
    assert_eq!(outcome.status, OutcomeStatus::Allowed);

    let exporter = ExportArtifactSetService::new(TestStamp, AllowPolicy);
    let artifact = exporter.execute(&mut session).expect("export");
    assert_eq!(artifact.session_id, *session.id());

    // The read helper sees the same state without being able to change it.
    let query = ReplaySessionQuery::new();
    assert_eq!(query.session_id(&session), session.id());
    assert_eq!(query.outcome(&session), Some(&outcome));
    assert_eq!(query.artifact(&session), Some(&artifact));
}

#[test]
fn the_run_service_never_turns_a_refusal_into_a_pass() {
    let mut session = OpenReplaySessionService::new()
        .execute(rigorix_gate_wasm::application::OpenSessionInput {
            scenario: inputs().scenario,
            fixtures: inputs().fixtures,
            clock: inputs().clock,
            seed: inputs().seed,
        })
        .expect("open");

    let runner = rigorix_gate_wasm::application::AttemptRunService::new(projection());
    let mut gate = RunnerStubGate::refusing();
    let mut set = PortSet::happy();
    let outcome = runner
        .execute(&mut session, action(), set.ports(), &mut gate)
        .expect("run");

    assert_eq!(outcome.status, OutcomeStatus::Refused);
    assert!(outcome.causing_step().is_some());
}

#[test]
fn a_failed_identity_port_through_the_service_is_still_a_refusal() {
    let mut session = OpenReplaySessionService::new()
        .execute(rigorix_gate_wasm::application::OpenSessionInput {
            scenario: inputs().scenario,
            fixtures: inputs().fixtures,
            clock: inputs().clock,
            seed: inputs().seed,
        })
        .expect("open");

    let runner = rigorix_gate_wasm::application::AttemptRunService::new(projection());
    let mut gate = RunnerStubGate::allowing();
    let mut set = PortSet::happy();
    set.key = FailingKey(rigorix_gate_wasm::domain::value::PortFailureKind::Missing).into_key();

    let outcome = runner
        .execute(&mut session, action(), set.ports(), &mut gate)
        .expect("run");
    assert_eq!(outcome.status, OutcomeStatus::Refused);
    assert_eq!(
        outcome.causing_step().expect("cause").kind,
        Some(RefusalKind::IdentityUnverified)
    );

    // And it cannot be exported: there is no key to sign with.
    let exporter = ExportArtifactSetService::new(TestStamp, AllowPolicy);
    assert!(exporter.execute(&mut session).is_err());
}

#[test]
fn the_export_service_refuses_a_rejected_content_rule() {
    use crate::support::RejectPolicy;
    use rigorix_gate_wasm::domain::services::ContentRule;

    let mut session = OpenReplaySessionService::new()
        .execute(rigorix_gate_wasm::application::OpenSessionInput {
            scenario: inputs().scenario,
            fixtures: inputs().fixtures,
            clock: inputs().clock,
            seed: inputs().seed,
        })
        .expect("open");
    let runner = rigorix_gate_wasm::application::AttemptRunService::new(projection());
    let mut gate = RunnerStubGate::allowing();
    let mut set = PortSet::happy();
    runner
        .execute(&mut session, action(), set.ports(), &mut gate)
        .expect("run");

    let exporter =
        ExportArtifactSetService::new(TestStamp, RejectPolicy(ContentRule::AbsolutePath));
    let error = exporter.execute(&mut session).unwrap_err();
    assert!(error.to_string().contains("absolute_path"));
    assert!(session.sealed().is_none());
}

#[test]
fn the_open_service_refuses_the_same_way_the_aggregate_does() {
    use rigorix_gate_wasm::domain::errors::SessionOpenError;

    let mut bad = inputs();
    bad.clock = rigorix_gate_wasm::domain::value::ClockTrack::default();
    let error = OpenReplaySessionService::new()
        .execute(rigorix_gate_wasm::application::OpenSessionInput {
            scenario: bad.scenario,
            fixtures: bad.fixtures,
            clock: bad.clock,
            seed: bad.seed,
        })
        .unwrap_err();
    assert_eq!(error, SessionOpenError::ClockSeriesEmpty);
    let _ = digest('a');
}

// ---------------------------------------------------------------------------
// The run's event stream (issue #1304): criterion 10 in evidence form
// ---------------------------------------------------------------------------

use rigorix_gate_wasm::domain::events::DomainEvent;

fn open() -> rigorix_gate_wasm::domain::session::ReplaySession {
    OpenReplaySessionService::new()
        .execute(rigorix_gate_wasm::application::OpenSessionInput {
            scenario: inputs().scenario,
            fixtures: inputs().fixtures,
            clock: inputs().clock,
            seed: inputs().seed,
        })
        .expect("open")
}

#[test]
fn a_refused_run_raises_one_step_event_per_decision_and_one_run_event() {
    let mut session = open();
    let runner = rigorix_gate_wasm::application::AttemptRunService::new(projection());
    let mut gate = RunnerStubGate::refusing();
    let mut set = PortSet::happy();
    let outcome = runner
        .execute(&mut session, action(), set.ports(), &mut gate)
        .expect("run");

    let events = session.take_events();
    let steps: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            DomainEvent::StepDecisionRecorded(recorded) => Some(recorded),
            _ => None,
        })
        .collect();
    assert_eq!(
        steps.len(),
        outcome.steps.len(),
        "every decision must be recorded, and no more"
    );
    assert_eq!(
        steps
            .iter()
            .map(|s| s.decision.as_str())
            .collect::<Vec<_>>(),
        vec!["allowed", "refused", "not_reached"]
    );
    // Step indexes are positions, not a counter that could skip.
    assert_eq!(
        steps.iter().map(|s| s.step_index).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    let refused = events.iter().find_map(|e| match e {
        DomainEvent::RunRefused(refused) => Some(refused),
        _ => None,
    });
    let refused = refused.expect("a refused run raises RunRefused");
    // The event names the step and carries the business wording, not the engine's label.
    assert_eq!(refused.caused_by_step, "precondition.budget");
    assert_eq!(refused.reason, "the required check did not pass");
    assert!(refused.evidence.is_some());

    // A refused run raises no `RunAllowed`.
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, DomainEvent::RunAllowed(_)))
    );
}

#[test]
fn an_allowed_run_raises_run_allowed_with_the_action_and_evidence() {
    let mut session = open();
    let runner = rigorix_gate_wasm::application::AttemptRunService::new(projection());
    let mut gate = RunnerStubGate::allowing();
    let mut set = PortSet::happy();
    runner
        .execute(&mut session, action(), set.ports(), &mut gate)
        .expect("run");

    let events = session.take_events();
    let allowed = events
        .iter()
        .find_map(|e| match e {
            DomainEvent::RunAllowed(allowed) => Some(allowed),
            _ => None,
        })
        .expect("RunAllowed");
    assert_eq!(allowed.action.name, "payment_execute");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, DomainEvent::RunRefused(_)))
    );
}

#[test]
fn every_event_a_run_raises_is_evidence() {
    // The run path must not leak telemetry into the record: only `RandomDrawConsumed` is
    // telemetry, and the run does not raise it.
    let mut session = open();
    let runner = rigorix_gate_wasm::application::AttemptRunService::new(projection());
    let mut gate = RunnerStubGate::refusing();
    let mut set = PortSet::happy();
    runner
        .execute(&mut session, action(), set.ports(), &mut gate)
        .expect("run");

    for event in session.take_events() {
        assert!(
            event.is_evidence(),
            "{event:?} is telemetry on the run path"
        );
    }
}

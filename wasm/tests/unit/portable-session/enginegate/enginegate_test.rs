//! The four port adapters, and the composition they feed.
//!
//! Issue: #1383 (Phase C.3)
//!
//! Every assertion here is behavioural: the question is never "does the adapter construct"
//! but "does the engine, handed these answers, reach the right decision". The distinctions
//! that matter are the ones a two-valued slot would swallow — absent vs corrupt, permit vs
//! unknown, and a promotion that is neither a pass nor a refusal.

#![cfg(feature = "engine")]

use rigorix_engine::precondition::application::dto::PreconditionCheckInput;
use rigorix_engine::precondition::domain::{
    Precondition, PreconditionError, ProviderDetermination, ProviderMethod, ProviderSpec,
};
use rigorix_engine::precondition::infrastructure::provider_client::ProviderClient;
use rigorix_engine::precondition::infrastructure::repository::PreconditionRepository;
use rigorix_engine::precondition::infrastructure::runner::PreconditionRunner;
use rigorix_engine::sequence_policy::infrastructure::repository::SequencePolicyRepository;
use rigorix_gate_wasm::domain::engine_seam::{GateEvaluationRequest, GateEvaluator};
use rigorix_gate_wasm::domain::value::{
    GateAttemptId, GatedAction, Params, ScenarioFormatVersion, ScenarioRef,
};
use rigorix_gate_wasm::infrastructure::engine_gate::EngineGate;
use rigorix_gate_wasm::infrastructure::engine_ports::{
    FixturePreconditionRepository, FixtureSequencePolicyRepository, TranscriptProviderClient,
    TranscriptRunner, precondition_service, sequence_service,
};
use serde_json::json;

/// Poll a future to completion, asserting that it never yields.
///
/// The engine's synchronous entry point (`evaluate_gate_sync`) is built on exactly this
/// property: the slice's ports answer on the first poll, so a browser needs no executor and
/// no `block_on`. A test that hid a yield inside `block_on` would hide the defect this
/// asserts against, so a second poll is a failure here rather than a wait.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Waker};

    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = Box::pin(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!(
            "a port yielded: the engine's synchronous entry point requires every slice port \
             to answer on the first poll"
        ),
    }
}

fn provider_spec() -> ProviderSpec {
    ProviderSpec {
        id: "standing".to_string(),
        endpoint: "https://determine.example/v1/check?token=secret".to_string(),
        method: ProviderMethod::Post,
        timeout_ms: 1000,
        credential_ref: None,
        determination_field: "determination".to_string(),
        permit_values: vec![],
        deny_values: vec![],
    }
}

fn check_input() -> PreconditionCheckInput {
    PreconditionCheckInput {
        precondition_id: "standing".to_string(),
        execution_id: uuid::Uuid::nil(),
        step: "transfer".to_string(),
        tool: "transfer".to_string(),
        parameters: json!({}),
    }
}

fn transcript(determination: &str) -> serde_json::Value {
    json!({
        "providers": {
            "standing": {
                "determination": determination,
                "observed_at": "2026-01-01T00:00:00Z",
                "provider_version": "v3"
            }
        }
    })
}

#[test]
fn the_fixture_writes_the_engines_own_determination_vocabulary() {
    block_on(async {
        // The fixture format *is* `ProviderDetermination`, so there is no second vocabulary to
        // drift: `permit` parses, and a word the engine does not know is `Unknown` — never a
        // permit. This is the one bridge #1393 established, seen from the adapter side.
        for (token, expected) in [
            ("permit", ProviderDetermination::Permit),
            ("deny", ProviderDetermination::Deny),
            ("unknown", ProviderDetermination::Unknown),
            ("ALLOW", ProviderDetermination::Unknown),
        ] {
            let client = TranscriptProviderClient::new(transcript(token));
            let observation = client
                .determine(&provider_spec(), &check_input())
                .await
                .expect("the transcript answers");
            assert_eq!(observation.determination, expected, "token {token}");
            assert_eq!(observation.raw_value.as_deref(), Some(token));
        }
    })
}

#[test]
fn an_absent_determination_is_unknown_and_never_a_permit() {
    block_on(async {
        // The world declares when it was captured, so an absent provider is still an
        // observation — of nothing.
        let client = TranscriptProviderClient::new(json!({
            "observed_at": "2026-01-01T00:00:00Z",
            "providers": {}
        }));
        let observation = client
            .determine(&provider_spec(), &check_input())
            .await
            .expect("an absent provider is still an observation");
        assert_eq!(observation.determination, ProviderDetermination::Unknown);
        assert_eq!(observation.raw_value, None);
    })
}

#[test]
fn the_recorded_endpoint_carries_no_query_string() {
    block_on(async {
        let client = TranscriptProviderClient::new(transcript("permit"));
        let observation = client
            .determine(&provider_spec(), &check_input())
            .await
            .expect("answers");
        assert_eq!(
            observation.endpoint_redacted, "https://determine.example",
            "the record keeps scheme and host, and drops the path and the query — a query string \
         is where credentials live"
        );
        assert!(!observation.endpoint_redacted.contains("secret"));
    })
}

#[test]
fn a_provider_with_no_observation_time_is_a_refusal_not_a_guess() {
    block_on(async {
        // `observed_at` is a fact only the fixture has. Synthesising one would put an invented
        // instant inside signed evidence, so its absence fails closed and names the field.
        let client = TranscriptProviderClient::new(json!({
            "providers": { "standing": { "determination": "permit" } }
        }));
        let error = client
            .determine(&provider_spec(), &check_input())
            .await
            .expect_err("no observation time must refuse");
        assert!(error.to_string().contains("observed_at"), "{error}");
    })
}

#[test]
fn a_browser_runs_no_check_programs_and_says_so() {
    block_on(async {
        let runner = TranscriptRunner::new(json!({ "providers": {} }));
        let precondition = Precondition {
            id: "beneficiary-eligible".to_string(),
            r#match: serde_json::from_value(json!({ "tool": "transfer" })).expect("predicate"),
            require_params: vec![],
            command: vec!["check".to_string()],
            failure: rigorix_engine::precondition::domain::FailureAction::default(),
            timeout_ms: 1000,
            capture_output: false,
            provider: None,
            authority_path: None,
            require_immutable_check: false,
        };
        let error = runner
            .run(&precondition, &check_input())
            .await
            .expect_err("a browser cannot spawn a check");
        assert!(matches!(error, PreconditionError::Spawn { .. }), "{error}");
        // Fail closed, and the message names the way out — a host reading this should know
        // what to declare instead.
        assert!(
            error.to_string().contains("provider precondition"),
            "{error}"
        );
    })
}

#[test]
fn no_rule_document_means_no_gating_and_never_a_permit() {
    block_on(async {
        // `Ok(None)` is "the operator configured nothing", which is what the trait documents.
        // A repository that turned an empty document into a permit would make absence authority.
        let repository = FixturePreconditionRepository::new(None);
        assert!(repository.load_config().await.expect("answers").is_none());

        let sequence = FixtureSequencePolicyRepository::new(None);
        assert!(sequence.load_config().await.expect("answers").is_none());
    })
}

#[test]
fn a_corrupt_rule_document_refuses_rather_than_evaluating_part_of_it() {
    block_on(async {
        let repository = FixturePreconditionRepository::new(Some(json!({ "preconditions": "no" })));
        let error = repository
            .load_config()
            .await
            .expect_err("a document that does not parse must refuse");
        assert!(
            matches!(error, PreconditionError::ConfigInvalid(_)),
            "{error}"
        );

        let sequence = FixtureSequencePolicyRepository::new(Some(json!({ "rules": 3 })));
        assert!(
            sequence.load_config().await.is_err(),
            "a corrupt policy must refuse; failing open here would drop every requirement"
        );
    })
}

#[test]
fn the_engine_services_are_the_engines_over_fixture_data() {
    // The crate supplies rule *data*; the service types are the engine's own. If this
    // ever becomes a crate-side evaluator, C.5's conformance claim becomes a coincidence.
    let transcript = json!({ "providers": {} });
    let _: std::sync::Arc<
        dyn rigorix_engine::precondition::application::service::PreconditionService,
    > = precondition_service(&transcript);
    let _: std::sync::Arc<
        dyn rigorix_engine::sequence_policy::application::service::SequencePolicyService,
    > = sequence_service(&transcript);
}

#[test]
fn the_empty_fixture_world_evaluates_to_a_dispatch() {
    block_on(async {
        // End to end through the seam: no rules means no gating, and the composition still
        // answers on the first poll (the readiness property C.1b states where the composition
        // lives). A `NotReady` here would mean an adapter yields, which none of them do.
        use rigorix_engine::gate::application::{
            GateOutcome, GatePorts, GateRequest, GatedStep, evaluate_gate,
        };

        let decision = evaluate_gate(GateRequest {
            step: &GatedStep::new("transfer", "transfer", json!({})),
            proposed_action: "transfer",
            identity_attested: false,
            // The execution id is required and never defaulted (#1381); the test supplies one.
            pins: rigorix_engine::audit::application::dto::PinnedInputs::none()
                .with_execution_id(uuid::Uuid::nil()),
            ports: GatePorts {
                precondition: Some(precondition_service(&json!({ "providers": {} }))),
                sequence: Some(sequence_service(&json!({}))),
            },
            run_context: None,
        })
        .await
        .expect("the gate answers");

        assert!(decision.verdict.permits_dispatch());
        assert!(
            decision.findings.is_empty()
                || decision
                    .findings
                    .iter()
                    .all(|f| f.outcome == GateOutcome::Passed)
        );
        assert!(
            decision.envelope_evidence.is_none(),
            "no run context means no envelope: the engine assembles evidence from facts, never from nothing"
        );
    })
}

/// The unlinked build has no engine to call, so it refuses — and the kind says "could not
/// answer", not "answered no". This test exists only where that build does.
#[cfg(not(feature = "engine"))]
#[test]
fn the_unlinked_build_refuses_every_run_with_a_reason() {
    // The default build has no engine. It must refuse — and `gate_unavailable` must name
    // "could not answer", not "answered no".
    let mut ports = crate::support::PortSet::happy();
    let mut gate = EngineGate::unavailable();
    let attempt_id = GateAttemptId::from_trusted("attempt-1".to_string());
    let scenario = ScenarioRef::new(
        crate::support::digest('a'),
        rigorix_gate_wasm::domain::value::ScenarioFormatVersion(1),
    );
    let action = GatedAction::new("transfer", Params::default());
    let error = gate
        .evaluate(GateEvaluationRequest {
            attempt_id: &attempt_id,
            scenario: &scenario,
            action: &action,
            ports: ports.ports(),
        })
        .expect_err("an unlinked build must refuse");
    assert!(error.to_string().contains("gate evaluation"), "{error}");
}

/// The linked build never invents the execution id that pins a decision.
///
/// An attempt id that is not an execution id is a rejection, not a default: a defaulted id
/// inside signed evidence is a fabrication (#1381, and C.1's `UnpinnedExecution`).
#[cfg(feature = "engine")]
#[test]
fn an_attempt_id_that_is_not_an_execution_id_is_rejected_not_defaulted() {
    let mut ports = crate::support::PortSet::happy();
    let mut gate = EngineGate::new(json!({ "observed_at": "2026-01-01T00:00:00Z" }));
    let attempt_id = GateAttemptId::from_trusted("attempt-1".to_string());
    let scenario = ScenarioRef::new(crate::support::digest('a'), ScenarioFormatVersion(1));
    let action = GatedAction::new("transfer", Params::default());
    let error = gate
        .evaluate(GateEvaluationRequest {
            attempt_id: &attempt_id,
            scenario: &scenario,
            action: &action,
            ports: ports.ports(),
        })
        .expect_err("a non-uuid attempt id cannot pin an execution");
    assert!(error.to_string().contains("not an execution id"), "{error}");
}

//! Provider preconditions (ISSUE-PRECONDITION-PROVIDERS).
//!
//! The load-bearing property is **non-inheritance**: a provider's permitting
//! answer is evidence, not authority. These tests exercise both directions and
//! every fail-closed path, plus the dispatch-time-only invocation.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use rigorix_engine::precondition::application::{
    DispatchStep, PreconditionCheckInput, PreconditionService, PreconditionServiceImpl,
};
use rigorix_engine::precondition::domain::{
    AttributionReason, GatingMode, Precondition, PreconditionConfig, PreconditionError,
    PreconditionOutcome, PreconditionVerdict, ProviderDetermination, ProviderObservation,
    ProviderSpec,
};
use rigorix_engine::precondition::infrastructure::repository::PreconditionRepository;
use rigorix_engine::precondition::infrastructure::{
    PreconditionRun, PreconditionRunner, ProviderClient,
};
use serde_json::json;

struct StubRepository {
    outcome: Result<Option<PreconditionConfig>, PreconditionError>,
}

#[async_trait]
impl PreconditionRepository for StubRepository {
    async fn load_config(&self) -> Result<Option<PreconditionConfig>, PreconditionError> {
        self.outcome.clone()
    }
}

/// A provider stub that always answers with one determination.
struct StubProvider {
    calls: Arc<Mutex<Vec<String>>>,
    determination: ProviderDetermination,
}

#[async_trait]
impl ProviderClient for StubProvider {
    async fn determine(
        &self,
        spec: &ProviderSpec,
        _input: &PreconditionCheckInput,
    ) -> Result<ProviderObservation, PreconditionError> {
        self.calls.lock().unwrap().push(spec.id.clone());
        Ok(ProviderObservation {
            determination: self.determination,
            raw_value: Some(self.determination.as_str().to_string()),
            observed_at: Utc::now(),
            provider_version: Some("2026.10".to_string()),
            request_digest: "sha256:req".to_string(),
            response_digest: "sha256:resp".to_string(),
            endpoint_redacted: "https://determine.example/<path sha256:abc>".to_string(),
        })
    }
}

/// A provider stub that cannot be consulted (unreachable / timeout / unset
/// credential).
struct FailingProvider;

#[async_trait]
impl ProviderClient for FailingProvider {
    async fn determine(
        &self,
        spec: &ProviderSpec,
        _input: &PreconditionCheckInput,
    ) -> Result<ProviderObservation, PreconditionError> {
        Err(PreconditionError::Provider {
            precondition_id: spec.id.clone(),
            detail: "provider unreachable or timed out (see the operator log)".to_string(),
        })
    }
}

/// An argv check stub recording calls and returning a fixed outcome.
struct RecordingRunner {
    calls: Arc<Mutex<Vec<String>>>,
    outcome: PreconditionOutcome,
}

#[async_trait]
impl PreconditionRunner for RecordingRunner {
    async fn run(
        &self,
        precondition: &Precondition,
        _input: &PreconditionCheckInput,
    ) -> Result<PreconditionRun, PreconditionError> {
        self.calls.lock().unwrap().push(precondition.id.clone());
        Ok(PreconditionRun {
            outcome: self.outcome,
            exit_code: None,
            stdout: None,
            check_writable: None,
            authority_writable: None,
            check_digest: Some("sha256:check".to_string()),
            authority_digest: None,
        })
    }
}

fn provider_precondition(id: &str, provider_id: &str) -> Precondition {
    serde_json::from_value(json!({
        "id": id,
        "match": { "tool": "payment_execute" },
        "provider": { "id": provider_id, "endpoint": "https://determine.example/v1/check" }
    }))
    .expect("provider precondition parses")
}

fn command_precondition(id: &str) -> Precondition {
    serde_json::from_value(json!({
        "id": id,
        "match": { "tool": "payment_execute" },
        "command": ["/opt/rigorix/checks/rule"]
    }))
    .expect("command precondition parses")
}

fn config(preconditions: Vec<Precondition>) -> PreconditionConfig {
    PreconditionConfig {
        fail_closed: true,
        preconditions,
        gating: GatingMode::default(),
    }
}

fn service(
    config: PreconditionConfig,
    runner: Box<dyn PreconditionRunner>,
    provider: Box<dyn ProviderClient>,
) -> PreconditionServiceImpl {
    PreconditionServiceImpl::new(
        Box::new(StubRepository {
            outcome: Ok(Some(config)),
        }),
        runner,
    )
    .with_provider_client(provider)
}

fn step(tool: &str) -> DispatchStep {
    DispatchStep {
        name: "pay".to_string(),
        tool: tool.to_string(),
        parameters: json!({ "beneficiary": "acct-1" }),
    }
}

fn no_runner_calls() -> (Arc<Mutex<Vec<String>>>, Box<dyn PreconditionRunner>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let runner = Box::new(RecordingRunner {
        calls: calls.clone(),
        outcome: PreconditionOutcome::Passed,
    });
    (calls, runner)
}

// ---------------------------------------------------------------------------
// Core: the provider determination is honoured, and bound into the finding
// ---------------------------------------------------------------------------

#[tokio::test]
async fn provider_permit_dispatches_and_binds_the_digests() {
    let provider_calls = Arc::new(Mutex::new(Vec::new()));
    let (_, runner) = no_runner_calls();
    let svc = service(
        config(vec![provider_precondition("standing", "standing-svc")]),
        runner,
        Box::new(StubProvider {
            calls: provider_calls.clone(),
            determination: ProviderDetermination::Permit,
        }),
    );

    let (verdict, findings) = svc
        .evaluate_with_findings(uuid::Uuid::nil(), &step("payment_execute"))
        .await
        .expect("evaluate");

    assert_eq!(verdict, PreconditionVerdict::Dispatch);
    assert_eq!(provider_calls.lock().unwrap().as_slice(), ["standing-svc"]);
    let finding = &findings[0];
    assert_eq!(finding.outcome, PreconditionOutcome::Passed);
    // The signed record binds the request and response bytes.
    assert_eq!(finding.check_digest.as_deref(), Some("sha256:req"));
    assert_eq!(finding.authority_digest.as_deref(), Some("sha256:resp"));
    assert_eq!(finding.attribution, Some(AttributionReason::Recorded));
    let summary = finding.summary.as_deref().unwrap_or_default();
    assert!(summary.contains("standing-svc"));
    assert!(summary.contains("permit"));
    assert!(summary.contains("sha256:req"));
    assert!(summary.contains("sha256:resp"));
}

#[tokio::test]
async fn provider_deny_refuses() {
    let provider_calls = Arc::new(Mutex::new(Vec::new()));
    let (_, runner) = no_runner_calls();
    let svc = service(
        config(vec![provider_precondition("standing", "standing-svc")]),
        runner,
        Box::new(StubProvider {
            calls: provider_calls,
            determination: ProviderDetermination::Deny,
        }),
    );
    let verdict = svc
        .evaluate(uuid::Uuid::nil(), &step("payment_execute"))
        .await
        .expect("evaluate");
    assert_eq!(
        verdict,
        PreconditionVerdict::Deny {
            precondition_id: "standing".to_string(),
            outcome: PreconditionOutcome::Failed,
        }
    );
}

#[tokio::test]
async fn provider_unknown_vocabulary_refuses() {
    // Unknown must never become authority (spec §4.2).
    let (_, runner) = no_runner_calls();
    let svc = service(
        config(vec![provider_precondition("standing", "standing-svc")]),
        runner,
        Box::new(StubProvider {
            calls: Arc::new(Mutex::new(Vec::new())),
            determination: ProviderDetermination::Unknown,
        }),
    );
    let verdict = svc
        .evaluate(uuid::Uuid::nil(), &step("payment_execute"))
        .await
        .expect("evaluate");
    assert!(matches!(verdict, PreconditionVerdict::Deny { .. }));
}

#[tokio::test]
async fn provider_unreachable_refuses_as_error() {
    let (_, runner) = no_runner_calls();
    let svc = service(
        config(vec![provider_precondition("standing", "standing-svc")]),
        runner,
        Box::new(FailingProvider),
    );
    let (verdict, findings) = svc
        .evaluate_with_findings(uuid::Uuid::nil(), &step("payment_execute"))
        .await
        .expect("evaluate");
    assert_eq!(
        verdict,
        PreconditionVerdict::Deny {
            precondition_id: "standing".to_string(),
            outcome: PreconditionOutcome::Error,
        }
    );
    // Never reached → no digests, refused before the (provider) check.
    assert_eq!(
        findings[0].attribution,
        Some(AttributionReason::RefusedBeforeCheck)
    );
    assert!(findings[0].check_digest.is_none());
}

// ---------------------------------------------------------------------------
// Non-inheritance, BOTH directions (the load-bearing property)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn provider_permit_does_not_override_our_own_refusal() {
    // Direction 1: provider ALLOW + our own rule REFUSES → refused.
    let runner_calls = Arc::new(Mutex::new(Vec::new()));
    let provider_calls = Arc::new(Mutex::new(Vec::new()));
    let svc = service(
        config(vec![
            provider_precondition("standing", "standing-svc"),
            command_precondition("our-rule"),
        ]),
        Box::new(RecordingRunner {
            calls: runner_calls.clone(),
            outcome: PreconditionOutcome::Failed,
        }),
        Box::new(StubProvider {
            calls: provider_calls.clone(),
            determination: ProviderDetermination::Permit,
        }),
    );

    let verdict = svc
        .evaluate(uuid::Uuid::nil(), &step("payment_execute"))
        .await
        .expect("evaluate");

    assert_eq!(
        verdict,
        PreconditionVerdict::Deny {
            precondition_id: "our-rule".to_string(),
            outcome: PreconditionOutcome::Failed,
        },
        "a provider permit must not stand on its own"
    );
    assert_eq!(provider_calls.lock().unwrap().len(), 1);
    assert_eq!(runner_calls.lock().unwrap().as_slice(), ["our-rule"]);
}

#[tokio::test]
async fn provider_deny_overrides_our_own_permit() {
    // Direction 2: provider BLOCK + our own rule ALLOWS → refused.
    let runner_calls = Arc::new(Mutex::new(Vec::new()));
    let provider_calls = Arc::new(Mutex::new(Vec::new()));
    let svc = service(
        config(vec![
            command_precondition("our-rule"),
            provider_precondition("standing", "standing-svc"),
        ]),
        Box::new(RecordingRunner {
            calls: runner_calls.clone(),
            outcome: PreconditionOutcome::Passed,
        }),
        Box::new(StubProvider {
            calls: provider_calls.clone(),
            determination: ProviderDetermination::Deny,
        }),
    );

    let verdict = svc
        .evaluate(uuid::Uuid::nil(), &step("payment_execute"))
        .await
        .expect("evaluate");

    assert_eq!(
        verdict,
        PreconditionVerdict::Deny {
            precondition_id: "standing".to_string(),
            outcome: PreconditionOutcome::Failed,
        },
        "a passing own rule must not override a provider block"
    );
    assert_eq!(runner_calls.lock().unwrap().as_slice(), ["our-rule"]);
    assert_eq!(provider_calls.lock().unwrap().as_slice(), ["standing-svc"]);
}

// ---------------------------------------------------------------------------
// Dispatch-time only + no call for a non-matching step
// ---------------------------------------------------------------------------

#[tokio::test]
async fn provider_is_not_called_for_a_non_matching_step() {
    let provider_calls = Arc::new(Mutex::new(Vec::new()));
    let (_, runner) = no_runner_calls();
    let svc = service(
        config(vec![provider_precondition("standing", "standing-svc")]),
        runner,
        Box::new(StubProvider {
            calls: provider_calls.clone(),
            determination: ProviderDetermination::Permit,
        }),
    );
    // Not `payment_execute` → the provider must not be consulted at all.
    let verdict = svc
        .evaluate(uuid::Uuid::nil(), &step("payment_cancel"))
        .await
        .expect("evaluate");
    assert_eq!(verdict, PreconditionVerdict::Dispatch);
    assert!(provider_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn provider_verdict_is_deterministic() {
    let (_, runner) = no_runner_calls();
    let svc = service(
        config(vec![provider_precondition("standing", "standing-svc")]),
        runner,
        Box::new(StubProvider {
            calls: Arc::new(Mutex::new(Vec::new())),
            determination: ProviderDetermination::Permit,
        }),
    );
    let first = svc
        .evaluate(uuid::Uuid::nil(), &step("payment_execute"))
        .await
        .expect("evaluate");
    for _ in 0..20 {
        assert_eq!(
            svc.evaluate(uuid::Uuid::nil(), &step("payment_execute"))
                .await
                .expect("evaluate"),
            first
        );
    }
}

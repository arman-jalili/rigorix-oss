//! Envelope-v2 conformance (F-20260907-01): real engine output vs rigorix-sdk
//! `schemas/envelope.json` v1.
//!
//! Every envelope here is built through the REAL `AuditEnvelopeFactoryImpl`
//! with real event payloads (the same types the orchestrator wires on the run
//! path) — no hand-rolled JSON that bypasses engine serialization. The schema
//! asserts the frozen shape: EventStatus casing (`Success|Failure|Skipped|
//! Cancelled` — no rename_all), nulls for non-skipped Options, omitted
//! empty/None fields where `skip_serializing_if` is set, sorted map keys.

use rigorix_engine::audit::application::dto::BuildEnvelopeInput;
use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::application::factory::AuditEnvelopeFactory;
use rigorix_engine::audit::domain::{AuditEnvelope, EventStatus, ExecutionEventRef};
use rigorix_engine::identity::domain::{IdentityClaim, IdentityRef, IdentitySource};

use crate::{assert_valid, load_schema};

/// Fixed test-only HMAC key for the conformance corpus (documented in the SDK
/// `schemas/fixtures/envelope/README.md`). Never a live key.
pub(crate) const FIXTURE_KEY: &str = "fixture-hmac-key-2026";

fn schema() -> serde_json::Value {
    load_schema("envelope.json")
}

fn claim() -> IdentityClaim {
    IdentityClaim {
        subject: "user@org".to_string(),
        issuer: "https://idp.example.com".to_string(),
        authority: Some("admin".to_string()),
        source: IdentitySource::IdpToken,
        auth_method: Some("device_code".to_string()),
        issued_at: chrono::Utc::now() - chrono::Duration::minutes(5),
        expires_at: Some(chrono::Utc::now() + chrono::Duration::minutes(10)),
        token_ref: Some("keychain://default/rigorix/idp-token".to_string()),
    }
}

fn minimal_input() -> BuildEnvelopeInput {
    BuildEnvelopeInput {
        execution_id: uuid::Uuid::new_v4(),
        template_id: "conference-registration".to_string(),
        planning_prompt: "register alice for conf-2026".to_string(),
        events: vec![ExecutionEventRef {
            event_type: "tool_executed".to_string(),
            summary: "removed alice".to_string(),
            occurred_at: chrono::Utc::now(),
            correlation_id: None,
            status: EventStatus::Success,
            payload: None,
        }],
        source: Some("rigorix_action".to_string()),
        repository: None,
        author: None,
        identity: None,
        total_tokens: 1000,
        duration_ms: 5000,
        git_commit: None,
        git_branch: None,
        model_version: None,
        planning_prompt_content: None,
        file_paths: vec![],
        metadata: None,
        scoring_results: std::collections::HashMap::new(),
        sign: false,
    }
}

/// Event payloads mirror the real drained-event shapes the factory derives
/// approval_events / scope_violations / sequence_policy_findings /
/// decision_context_ref from (see `envelope_factory_impl.rs` population
/// logic + the audit module spec).
pub(crate) fn approval_event() -> ExecutionEventRef {
    ExecutionEventRef {
        event_type: "approval_recorded".to_string(),
        summary: "Approval recorded: risky_step by user@org".to_string(),
        occurred_at: chrono::Utc::now(),
        correlation_id: Some(uuid::Uuid::new_v4()),
        status: EventStatus::Success,
        payload: Some(serde_json::json!({
            "node_id": uuid::Uuid::new_v4().to_string(),
            "step_name": "risky_step",
            "intent_hash": "abc123",
            "approver_id": "user@org",
            "authority": "role:operator",
            "decided_at": chrono::Utc::now().to_rfc3339(),
            "decision_context_ref": "deploy to prod",
        })),
    }
}

pub(crate) fn scope_violation_event() -> ExecutionEventRef {
    ExecutionEventRef {
        event_type: "scope_violation_recorded".to_string(),
        summary: "Effects outside declared scope".to_string(),
        occurred_at: chrono::Utc::now(),
        correlation_id: Some(uuid::Uuid::new_v4()),
        status: EventStatus::Failure,
        payload: Some(serde_json::json!({
            "node_id": uuid::Uuid::new_v4().to_string(),
            "step_name": "run_command",
            "out_of_scope": ["src/injected.rs"],
            "detected_at": chrono::Utc::now().to_rfc3339(),
        })),
    }
}

pub(crate) fn sequence_rule_event() -> ExecutionEventRef {
    ExecutionEventRef {
        event_type: "sequence_rule_matched".to_string(),
        summary: "remove-then-reassign matched".to_string(),
        occurred_at: chrono::Utc::now(),
        correlation_id: None,
        status: EventStatus::Skipped,
        payload: Some(serde_json::json!({
            "rule_id": "no-remove-then-reassign",
            "later_step": "registration_add",
            "action": "promote",
            "matched_indices": [0, 3],
            "summary": "step 3 promoted for human approval",
        })),
    }
}

/// A minimal unsigned envelope (schema shape (a)). The factory marks unsigned
/// runs `evidence_degraded=true` (GAP-M-12) — both the degraded engine output
/// AND the pre-GAP-M-12 `false` shape must conform (schema describes shape;
/// the GAP-M-12 policy lives in the verifier).
#[tokio::test]
async fn minimal_unsigned_envelope_conforms() {
    let factory = AuditEnvelopeFactoryImpl::new(None);
    let envelope = factory.build_envelope(minimal_input()).await.unwrap();
    assert!(envelope.signature.is_none());
    assert!(
        envelope.evidence_degraded,
        "unsigned must be marked degraded"
    );

    let schema = schema();
    let value = serde_json::to_value(&envelope).expect("engine serde");
    assert_valid(&schema, &value, "minimal unsigned (evidence_degraded=true)");

    // Schema shape (a): `evidence_degraded=false` + `signature: null` is a
    // valid envelope shape (verifier policy, not schema, rejects it).
    let legacy = AuditEnvelope {
        evidence_degraded: false,
        ..envelope
    };
    let value = serde_json::to_value(&legacy).expect("engine serde");
    assert_valid(
        &schema,
        &value,
        "minimal unsigned (evidence_degraded=false)",
    );
}

/// An HMAC-signed envelope (schema shape (b)) — fixed fixture key, signature
/// non-null, engine verify round-trips.
#[tokio::test]
async fn hmac_signed_envelope_conforms_and_verifies() {
    let factory = AuditEnvelopeFactoryImpl::new(Some(FIXTURE_KEY.to_string()));
    let mut input = minimal_input();
    input.sign = true;
    let envelope = factory.build_envelope(input).await.unwrap();

    assert!(envelope.signature.is_some(), "signed envelope carries HMAC");
    assert!(!envelope.evidence_degraded);

    let value = serde_json::to_value(&envelope).expect("engine serde");
    assert_valid(&schema(), &value, "hmac-signed envelope");

    // Engine-side verify (byte-exact reproduction with the same key).
    factory
        .verify_signature(&envelope)
        .await
        .expect("engine verifies own HMAC");
}

/// The RICH envelope (schema shape (c)): identity + approval_events +
/// scope_violations + sequence_policy_findings + scoring_results +
/// decision_context_ref — all built through the real factory derivation from
/// real drained event payloads (no hand-rolled JSON).
#[tokio::test]
async fn rich_envelope_conforms_and_is_deterministic() {
    let factory = AuditEnvelopeFactoryImpl::new(Some(FIXTURE_KEY.to_string()));
    let mut input = minimal_input();
    input.identity = Some(IdentityRef::from_claim(&claim()));
    input.events = vec![
        approval_event(),
        scope_violation_event(),
        sequence_rule_event(),
    ];
    input.scoring_results = {
        let mut m = std::collections::HashMap::new();
        let mut dims = std::collections::HashMap::new();
        dims.insert(
            "code_quality".to_string(),
            rigorix_engine::audit::domain::ScoreDimensionRef {
                score: 0.9,
                max: 1.0,
                label: "Code quality".to_string(),
                passed: true,
            },
        );
        dims.insert(
            "coverage".to_string(),
            rigorix_engine::audit::domain::ScoreDimensionRef {
                score: 0.8,
                max: 1.0,
                label: "Coverage".to_string(),
                passed: true,
            },
        );
        m.insert(
            "node-1".to_string(),
            rigorix_engine::audit::domain::ScoringResultRef {
                passed: true,
                backend: "llm-judge".to_string(),
                dimensions: dims,
                duration_ms: 1200,
            },
        );
        m.insert(
            "node-2".to_string(),
            rigorix_engine::audit::domain::ScoringResultRef {
                passed: false,
                backend: "llm-judge".to_string(),
                dimensions: std::collections::HashMap::new(),
                duration_ms: 800,
            },
        );
        m
    };
    input.file_paths = vec!["src/lib.rs".to_string()];
    input.planning_prompt_content = Some("opt-in prompt content".to_string());
    input.git_commit = Some("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2".to_string());
    input.git_branch = Some("main".to_string());
    input.sign = true;

    let envelope = factory.build_envelope(input).await.unwrap();

    // Factory derivation must have populated the rich additive blocks.
    assert_eq!(
        envelope.approval_events.len(),
        1,
        "approval derived from events"
    );
    assert_eq!(
        envelope.scope_violations.len(),
        1,
        "scope violation derived"
    );
    assert_eq!(
        envelope.sequence_policy_findings.len(),
        1,
        "sequence finding derived"
    );
    assert_eq!(envelope.scoring_results.len(), 2, "scoring map populated");
    assert_eq!(
        envelope.decision_context_ref.as_deref(),
        Some("deploy to prod"),
        "decision context ref derived from approval payload"
    );
    assert!(envelope.identity.is_some(), "identity ref carried");

    let value = serde_json::to_value(&envelope).expect("engine serde");
    assert_valid(&schema(), &value, "rich envelope");

    // Deterministic canonical bytes: two serializations (fresh HashMap state)
    // must be byte-identical — the conformance map fix (sorted keys).
    let again = serde_json::to_value(&envelope).expect("engine serde");
    assert_eq!(
        serde_json::to_string(&value).unwrap(),
        serde_json::to_string(&again).unwrap(),
        "canonical serialization must be stable (sorted map keys)"
    );

    // EventStatus casing is serde-exact: schema (which asserts the enum) must
    // reject a lowercase variant — proving the engine emits the frozen casing.
    let mut broken = value.clone();
    broken["events"][1]["status"] = serde_json::json!("failure"); // lowercase
    let compiled = jsonschema::validator_for(&schema()).expect("schema compiles");
    assert!(
        !compiled.is_valid(&broken),
        "lowercase EventStatus must violate the schema"
    );

    // Engine verify round-trips on the rich envelope too.
    factory
        .verify_signature(&envelope)
        .await
        .expect("engine verifies rich HMAC");
}

//! #1222: an envelope carrying `scope_check` must verify with the published
//! verifier, and a chain containing it must verify end to end.
//!
//! @canonical .pi/architecture/modules/audit.md#envelope
//!
//! The 0.8.0 verifier struct lacked the field, so serde dropped it while
//! re-serializing and produced different canonical bytes — signature mismatch
//! on the envelope, `prev_hash` mismatch on its successor. 0.8.1 admits the
//! field in the engine's declared position.

use rigorix_engine::audit::application::dto::BuildEnvelopeInput;
use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::application::factory::AuditEnvelopeFactory;
use rigorix_engine::audit::domain::{EventStatus, ExecutionEventRef};
use uuid::Uuid;

const KEY: &str = "scope-check-verifier-test-key";

fn input(execution_id: Uuid, sequence: u64, prev_hash: Option<String>) -> BuildEnvelopeInput {
    BuildEnvelopeInput {
        execution_id,
        template_id: "scope-check".to_string(),
        planning_prompt: "verify scope_check".to_string(),
        events: vec![ExecutionEventRef {
            event_type: "effect_scope_checked".to_string(),
            summary: "effect scope checked".to_string(),
            occurred_at: chrono::Utc::now(),
            correlation_id: None,
            status: EventStatus::Success,
            payload: Some(serde_json::json!({ "status": "checked" })),
        }],
        source: Some("rigorix_mcp".to_string()),
        repository: None,
        author: None,
        identity: None,
        effect_key: None,
        producer_id: Some("local".to_string()),
        sequence: Some(sequence),
        prev_hash,
        history_policy: None,
        total_tokens: 0,
        duration_ms: 0,
        git_commit: None,
        git_branch: None,
        model_version: None,
        planning_prompt_content: None,
        file_paths: vec![],
        metadata: None,
        sign: true,
        scoring_results: std::collections::HashMap::new(),
    }
}

#[tokio::test]
async fn scope_check_envelopes_verify_and_chain_end_to_end() {
    let factory = AuditEnvelopeFactoryImpl::new(Some(KEY.to_string()));

    let first = factory
        .build_envelope(input(Uuid::new_v4(), 0, None))
        .await
        .expect("build genesis");
    assert_eq!(
        first.scope_check,
        Some(rigorix_engine::audit::domain::ScopeCheckStatus::Checked),
        "the envelope must actually carry the field under test"
    );
    let first_prev = AuditEnvelopeFactoryImpl::envelope_hash(&first).expect("hash genesis");

    let second = factory
        .build_envelope(input(Uuid::new_v4(), 1, Some(first_prev)))
        .await
        .expect("build successor");

    let first_json = serde_json::to_string(&first).expect("serialize genesis");
    let second_json = serde_json::to_string(&second).expect("serialize successor");

    rigorix_verifier::verify_signature(&first_json, KEY).expect("genesis signature verifies");
    rigorix_verifier::verify_signature(&second_json, KEY).expect("successor signature verifies");

    let parsed = vec![
        serde_json::from_str::<rigorix_verifier::AuditEnvelope>(&first_json)
            .expect("parse genesis"),
        serde_json::from_str::<rigorix_verifier::AuditEnvelope>(&second_json)
            .expect("parse successor"),
    ];
    rigorix_verifier::verify_chain(&parsed).expect("the chain containing scope_check verifies");
}

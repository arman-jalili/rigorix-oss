//! Integration test: execution produces audit trail.
//!
//! Tests audit envelope building and HMAC signature verification.

use rigorix::audit::application::dto::BuildEnvelopeInput;
use rigorix::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix::audit::application::factory::AuditEnvelopeFactory;
use rigorix::audit::domain::envelope::ExecutionEventRef;
use std::collections::HashMap;

#[tokio::test]
async fn test_audit_envelope_has_hmac_signature() {
    let factory = AuditEnvelopeFactoryImpl::new(Some("test-signing-key".to_string()));

    let input = BuildEnvelopeInput {
        execution_id: uuid::Uuid::new_v4(),
        template_id: "test-template".to_string(),
        planning_prompt: "Read src/lib.rs".to_string(),
        events: vec![
            ExecutionEventRef {
                event_type: "execution_started".to_string(),
                summary: "Execution started".to_string(),
                occurred_at: chrono::Utc::now(),
                correlation_id: None,
                status: rigorix::audit::domain::envelope::EventStatus::Success,
            },
        ],
        metadata: Some(HashMap::from([
            ("intent".to_string(), "test intent".to_string()),
            ("llm_calls".to_string(), "2".to_string()),
        ])),
        sign: true,
    };

    let envelope = factory.build_envelope(input).await.unwrap();
    assert!(envelope.signature.is_some(), "HMAC signature should be present");
}

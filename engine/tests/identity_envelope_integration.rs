//! Integration test: identity flows into the envelope identity block (redacted).
//!
//! Covers identity module acceptance criterion #6:
//! "RunInput.identity flows into envelope identity block (redacted)".
//!
//! Uses the REAL envelope factory (`AuditEnvelopeFactoryImpl`) and the real
//! redaction mapping (`IdentityRef::from_claim`) — the same types the
//! orchestrator wires on the run path. The full orchestrator run-path
//! equivalent lives in `orchestrator_impl.rs` tests (crate-internal mocks).

use rigorix_engine::audit::application::dto::BuildEnvelopeInput;
use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::application::factory::AuditEnvelopeFactory;
use rigorix_engine::identity::domain::{IdentityClaim, IdentityRef, IdentitySource};

/// Attested sample claim — token preserved only by reference.
fn sample_claim() -> IdentityClaim {
    IdentityClaim {
        subject: "user@org".to_string(),
        issuer: "https://idp.example.com".to_string(),
        authority: Some("admin".to_string()),
        source: IdentitySource::IdpToken,
        auth_method: Some("device_code".to_string()),
        issued_at: chrono::Utc::now(),
        expires_at: Some(chrono::Utc::now() + chrono::Duration::minutes(15)),
        token_ref: Some("keychain://default/rigorix/idp-token".to_string()),
    }
}

fn envelope_input(identity: Option<IdentityRef>) -> BuildEnvelopeInput {
    BuildEnvelopeInput {
        execution_id: uuid::Uuid::new_v4(),
        template_id: "test-template".to_string(),
        planning_prompt: "Read src/lib.rs".to_string(),
        events: vec![],
        source: None,
        repository: None,
        author: Some("legacy-author".to_string()),
        identity,
        total_tokens: 0,
        duration_ms: 0,
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

#[tokio::test]
async fn test_envelope_factory_populates_identity_block_from_input() {
    let factory = AuditEnvelopeFactoryImpl::new(None);
    let claim = sample_claim();

    let envelope = factory
        .build_envelope(envelope_input(Some(IdentityRef::from_claim(&claim))))
        .await
        .expect("envelope builds");

    let identity_ref = envelope
        .identity
        .expect("envelope identity block must be populated");
    assert_eq!(identity_ref.subject, "user@org");
    assert_eq!(identity_ref.issuer, "https://idp.example.com");
    assert_eq!(identity_ref.source, IdentitySource::IdpToken);
    assert_eq!(identity_ref.authority, Some("admin".to_string()));
}

#[tokio::test]
async fn test_envelope_without_identity_keeps_block_absent() {
    let factory = AuditEnvelopeFactoryImpl::new(None);
    let envelope = factory
        .build_envelope(envelope_input(None))
        .await
        .expect("envelope builds");
    assert!(
        envelope.identity.is_none(),
        "no identity => no identity block"
    );
    // Backward compatible: field is serde-defaulted and skipped when absent.
    let json = serde_json::to_string(&envelope).expect("serialize envelope");
    assert!(
        !json.contains("\"identity\""),
        "absent block must be skipped: {json}"
    );
}

#[test]
fn test_identity_ref_redaction_never_leaks_token() {
    let claim = sample_claim();
    let identity_ref = IdentityRef::from(&claim);

    let ref_json = serde_json::to_string(&identity_ref).expect("serialize ref");
    assert!(
        !ref_json.contains("token_ref"),
        "identity ref must not carry token_ref: {ref_json}"
    );
    assert!(
        !ref_json.contains("keychain"),
        "token locator leaked: {ref_json}"
    );
    assert!(
        !ref_json.contains("eyJhbGci"),
        "raw token payload leaked: {ref_json}"
    );

    // Full envelope serialization with the identity block present.
    let envelope_json = serde_json::to_string(&serde_json::json!({
        "execution_id": uuid::Uuid::new_v4(),
        "identity": identity_ref,
    }))
    .expect("serialize envelope shape");
    assert!(!envelope_json.contains("keychain"));
    assert!(!envelope_json.contains("token_ref"));
}

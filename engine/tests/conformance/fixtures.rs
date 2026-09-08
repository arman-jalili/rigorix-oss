//! Real signed envelope fixture generation (F-20260907-01 DoD).
//!
//! Produces 4–5 REAL engine-signed envelope JSON files (fixed test HMAC key
//! `fixture-hmac-key-2026`, documented in rigorix-sdk
//! `schemas/fixtures/envelope/README.md`) from actual engine runs — envelopes
//! built through the real factory with real event payloads, then committed to
//! rigorix-sdk `schemas/fixtures/envelope/` via the SDK PR.
//!
//! Only runs when `RIGORIX_FIXTURE_OUT` is set (default CI never sets it):
//! ```bash
//! RIGORIX_FIXTURE_OUT=<dir> cargo test -p rigorix-engine --features conformance \
//!   --test conformance write_real_signed_fixtures -- --ignored
//! ```

use rigorix_engine::audit::application::dto::BuildEnvelopeInput;
use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::application::factory::AuditEnvelopeFactory;
use rigorix_engine::audit::domain::{EventStatus, ExecutionEventRef};
use rigorix_engine::identity::domain::{IdentityClaim, IdentityRef, IdentitySource};

use crate::envelope::{FIXTURE_KEY, approval_event, scope_violation_event, sequence_rule_event};

fn identity_claim() -> IdentityClaim {
    IdentityClaim {
        subject: "user@org".to_string(),
        issuer: "https://idp.example.com".to_string(),
        authority: Some("admin".to_string()),
        source: IdentitySource::IdpToken,
        auth_method: Some("device_code".to_string()),
        issued_at: chrono::Utc::now() - chrono::Duration::minutes(30),
        expires_at: Some(chrono::Utc::now() + chrono::Duration::minutes(30)),
        token_ref: Some("keychain://default/rigorix/idp-token".to_string()),
    }
}

fn base_input() -> BuildEnvelopeInput {
    BuildEnvelopeInput {
        execution_id: uuid::Uuid::new_v4(),
        template_id: "conference-registration".to_string(),
        planning_prompt: "register jeff for conf-2026".to_string(),
        events: vec![ExecutionEventRef {
            event_type: "tool_executed".to_string(),
            summary: "add jeff to conf-2026".to_string(),
            occurred_at: chrono::Utc::now(),
            correlation_id: Some(uuid::Uuid::new_v4()),
            status: EventStatus::Success,
            payload: Some(serde_json::json!({ "tool": "add_attendance" })),
        }],
        source: Some("rigorix_action".to_string()),
        repository: Some("arman-jalili/conference-demo".to_string()),
        author: Some("user@org".to_string()),
        identity: None,
        total_tokens: 3200,
        duration_ms: 48200,
        git_commit: Some("1f6a4c9e2b8d4f0a3c5e7b9d1a2c4e6f8a0b2d4c6".to_string()),
        git_branch: Some("main".to_string()),
        model_version: Some("claude-sonnet-4-20250514".to_string()),
        planning_prompt_content: None,
        file_paths: vec!["data/registrations.json".to_string()],
        metadata: None,
        scoring_results: std::collections::HashMap::new(),
        sign: true,
    }
}

fn rich_input() -> BuildEnvelopeInput {
    let mut input = base_input();
    input.identity = Some(IdentityRef::from_claim(&identity_claim()));
    input.events = vec![
        approval_event(),
        scope_violation_event(),
        sequence_rule_event(),
    ];
    input.file_paths = vec![
        "data/registrations.json".to_string(),
        "src/injected.rs".to_string(),
    ];
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
    input.scoring_results.insert(
        "node-1".to_string(),
        rigorix_engine::audit::domain::ScoringResultRef {
            passed: true,
            backend: "llm-judge".to_string(),
            dimensions: dims,
            duration_ms: 1200,
        },
    );
    input
}

/// Write 5 real signed fixtures (one per variant) to `RIGORIX_FIXTURE_OUT`.
#[tokio::test]
#[ignore = "env-gated fixture generation (RIGORIX_FIXTURE_OUT); run explicitly"]
async fn write_real_signed_fixtures() {
    let Ok(out_dir) = std::env::var("RIGORIX_FIXTURE_OUT") else {
        eprintln!("RIGORIX_FIXTURE_OUT unset — skipping fixture generation");
        return;
    };
    let factory = AuditEnvelopeFactoryImpl::new(Some(FIXTURE_KEY.to_string()));

    // 1. Signed minimal (single tool event).
    let signed = factory
        .build_envelope(base_input())
        .await
        .expect("signed envelope");
    // 2. Unsigned degraded (evidence_degraded=true, no signature).
    let mut unsigned_input = base_input();
    unsigned_input.sign = false;
    let degraded = factory
        .build_envelope(unsigned_input)
        .await
        .expect("degraded envelope");
    // 3. Rich (identity + approval + scope + findings + scoring).
    let rich = factory
        .build_envelope(rich_input())
        .await
        .expect("rich envelope");
    // 4. Sequence-policy finding variant (deny recorded).
    let mut denied_input = base_input();
    denied_input.events = vec![
        sequence_rule_event(),
        ExecutionEventRef {
            event_type: "sequence_policy_denied".to_string(),
            summary: "dispatch denied by rule".to_string(),
            occurred_at: chrono::Utc::now(),
            correlation_id: None,
            status: EventStatus::Failure,
            payload: Some(serde_json::json!({
                "rule_id": "no-remove-then-reassign",
                "later_step": "run_command",
                "action": "deny",
                "matched_indices": [0, 2],
                "summary": "composed remove→add denied before dispatch",
            })),
        },
    ];
    let denied = factory
        .build_envelope(denied_input)
        .await
        .expect("denied envelope");
    // 5. Rich without identity (pre-identity envelope, additive block absent).
    let mut pre_identity_input = rich_input();
    pre_identity_input.identity = None;
    let pre_identity = factory
        .build_envelope(pre_identity_input)
        .await
        .expect("pre-identity envelope");

    let variants: [(&str, &rigorix_engine::audit::domain::AuditEnvelope); 5] = [
        ("envelope-minimal-signed", &signed),
        ("envelope-unsigned-degraded", &degraded),
        ("envelope-rich-signed", &rich),
        ("envelope-sequence-denied-signed", &denied),
        ("envelope-rich-pre-identity", &pre_identity),
    ];

    let dir = std::path::Path::new(&out_dir);
    std::fs::create_dir_all(dir).expect("create fixture out dir");
    for (name, envelope) in variants {
        let json = serde_json::to_string_pretty(envelope).expect("serialize fixture");
        let path = dir.join(format!("{name}.json"));
        std::fs::write(&path, format!("{json}\n")).expect("write fixture");
        eprintln!(
            "wrote {} (signature={})",
            path.display(),
            envelope.signature.is_some()
        );
    }
}

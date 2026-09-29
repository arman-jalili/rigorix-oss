//! Live anchored-path wire E2E (ADR-016 Phase C, #912).
//!
//! Fixture parity is not wire parity: this drives the **real**
//! [`HttpAnchorClient`] over HTTP against a server that returns a genuinely
//! Ed25519-signed `HistorySlice` on the enterprise's exact wire
//! (`GET /v1/history?scope=&since=`, `Authorization: Bearer <credential>`,
//! bare signed slice). It asserts the four outcomes the issue requires:
//!
//! 1. a valid signed slice verifies and is consumed (deny-class run evaluated);
//! 2. an unreachable / erroring anchor **fails closed**;
//! 3. a forged slice **fails closed** (signature verified before matching);
//! 4. `local_unanchored` is unchanged.
//!
//! It also asserts the verified head is bound into the envelope
//! (`history_integrity = anchored`, `anchor_head`) and that the bearer
//! credential is presented.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use rigorix_engine::audit::application::factory::AuditEnvelopeFactory as _;
use rigorix_engine::audit::application::{AuditEnvelopeFactoryImpl, BuildEnvelopeInput};
use rigorix_engine::audit::domain::{
    EventStatus, ExecutionEventRef, HistoryIntegrity, anchor::AnchorMode,
};
use rigorix_engine::audit::infrastructure::anchor::{AnchorRuntime, HttpAnchorClient};
use rigorix_engine::sequence_policy::application::PlannedStep;
use rigorix_engine::sequence_policy::application::SequencePolicyServiceImpl;
use rigorix_engine::sequence_policy::application::service::SequencePolicyService as _;
use rigorix_engine::sequence_policy::domain::{
    HistoryPredicate, ParamMatchKind, ParamPredicate, RuleAction, SequencePolicyConfig,
    SequencePolicyError, SequenceRule, StepPredicate,
};
use rigorix_engine::sequence_policy::infrastructure::{
    AnchoredHistoryAdapter, SequencePolicyRepository,
};

const SCOPE: &str = "local";
const HEAD: &str = "abababababababababababababababababababababababababababababababab";
const TOKEN: &str = "rgx_e2e_anchor_key";

fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
}

fn action(
    node: &str,
    principal: &str,
    secs_ago: i64,
) -> rigorix_engine::audit::domain::anchor::HistoryActionRef {
    rigorix_engine::audit::domain::anchor::HistoryActionRef {
        node: node.into(),
        principal: Some(principal.into()),
        at: Utc::now() - chrono::Duration::seconds(secs_ago),
        effect_key: None,
    }
}

fn signed_slice(
    actions: Vec<rigorix_engine::audit::domain::anchor::HistoryActionRef>,
) -> rigorix_engine::audit::domain::anchor::HistorySlice {
    let mut slice = rigorix_engine::audit::domain::anchor::HistorySlice {
        scope: SCOPE.into(),
        since: Utc::now() - chrono::Duration::days(1),
        actions,
        head_hash: HEAD.into(),
        sig: None,
    };
    let sig = signing_key().sign(&slice.signing_bytes().unwrap());
    slice.sig = Some(hex::encode(sig.to_bytes()));
    slice
}

/// A runtime wired to `base_url` with the test signing key's public half.
fn runtime(base_url: &str) -> Arc<AnchorRuntime> {
    AnchorRuntime::anchored(
        Arc::new(HttpAnchorClient::new_with_token(
            base_url,
            Some(TOKEN.into()),
        )),
        hex::encode(signing_key().verifying_key().to_bytes()),
        "e2e-anchor",
        SCOPE,
    )
}

async fn mount_slice(
    server: &MockServer,
    slice: &rigorix_engine::audit::domain::anchor::HistorySlice,
) {
    Mock::given(method("GET"))
        .and(path("/v1/history"))
        .and(query_param("scope", SCOPE))
        .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(slice))
        .mount(server)
        .await;
}

fn deny_history_config() -> SequencePolicyConfig {
    SequencePolicyConfig {
        fail_closed: true,
        requirements: Vec::new(),
        rules: vec![SequenceRule {
            id: "no-cross-run-remove-reassign".into(),
            name: "No cross-run remove-then-reassign".into(),
            description: "d".into(),
            steps: vec![StepPredicate {
                tool: "registration_add".into(),
                params: vec![ParamPredicate {
                    pointer: "/event_id".into(),
                    kind: ParamMatchKind::Exact,
                    value: Some("conf-2026".into()),
                    step: None,
                }],
            }],
            window: None,
            action: RuleAction::Deny,
            history: Some(HistoryPredicate {
                prior_node: "registration_remove".into(),
                same_principal: true,
                window_secs: 900,
                effect_key: false,
            }),
        }],
    }
}

struct FixedRepo(Option<SequencePolicyConfig>);

#[async_trait::async_trait]
impl SequencePolicyRepository for FixedRepo {
    async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
        Ok(self.0.clone())
    }
}

/// The deny-class service over the real HTTP-backed anchored adapter.
fn anchored_service(runtime: Arc<AnchorRuntime>) -> SequencePolicyServiceImpl {
    SequencePolicyServiceImpl::new(Box::new(FixedRepo(Some(deny_history_config()))))
        .with_history(Arc::new(AnchoredHistoryAdapter::new(runtime)))
}

fn add_step() -> PlannedStep {
    PlannedStep {
        name: "add".into(),
        tool: "registration_add".into(),
        parameters: json!({ "event_id": "conf-2026" }),
    }
}

async fn eval_deny(svc: &SequencePolicyServiceImpl) -> Result<usize, SequencePolicyError> {
    let matches = svc.evaluate_plan(&[add_step()], Some("jeff@corp")).await?;
    Ok(matches.len())
}

// ── 1. valid signed slice over the wire ─────────────────────────────

#[tokio::test]
async fn valid_signed_slice_is_verified_over_http_and_head_recorded() {
    let server = MockServer::start().await;
    let slice = signed_slice(vec![action("registration_remove", "jeff@corp", 120)]);
    mount_slice(&server, &slice).await;

    let rt = runtime(&server.uri());
    let actions = rt
        .fetch_verified(Utc::now() - chrono::Duration::hours(1))
        .await
        .expect("a validly-signed slice verifies");
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].node, "registration_remove");
    assert_eq!(rt.head(), Some(HEAD.to_string()));

    // The deny-class service consumes the authenticated history.
    let svc = anchored_service(rt);
    assert_eq!(eval_deny(&svc).await.unwrap(), 1);
}

// ── 2. unreachable / erroring anchor fails closed ───────────────────

#[tokio::test]
async fn unreachable_anchor_fails_closed() {
    // Nothing listening on this port → connection refused.
    let rt = runtime("http://127.0.0.1:1");
    let svc = anchored_service(rt);
    assert!(
        eval_deny(&svc).await.is_err(),
        "an unreachable anchor must refuse a consequential (deny-class) run"
    );
}

#[tokio::test]
async fn erroring_anchor_fails_closed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let svc = anchored_service(runtime(&server.uri()));
    assert!(eval_deny(&svc).await.is_err());
}

// ── 3. forged slice fails closed (signature verified before matching) ─

#[tokio::test]
async fn forged_slice_fails_closed() {
    let server = MockServer::start().await;
    let mut slice = signed_slice(vec![action("registration_remove", "jeff@corp", 120)]);
    slice.head_hash = "ff".repeat(32); // tamper AFTER signing
    mount_slice(&server, &slice).await;

    let svc = anchored_service(runtime(&server.uri()));
    assert!(
        eval_deny(&svc).await.is_err(),
        "a forged anchor slice must fail closed"
    );
}

#[tokio::test]
async fn scope_mismatch_fails_closed() {
    let server = MockServer::start().await;
    let mut slice = signed_slice(vec![action("registration_remove", "jeff@corp", 120)]);
    // Re-sign a slice whose scope is *not* the configured one.
    slice.scope = "other-scope".into();
    let sig = signing_key().sign(&slice.signing_bytes().unwrap());
    slice.sig = Some(hex::encode(sig.to_bytes()));
    Mock::given(method("GET"))
        .and(path("/v1/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&slice))
        .mount(&server)
        .await;

    assert!(
        eval_deny(&anchored_service(runtime(&server.uri())))
            .await
            .is_err()
    );
}

// ── 4. local_unanchored unchanged ───────────────────────────────────

#[tokio::test]
async fn local_unanchored_is_unchanged() {
    let rt = AnchorRuntime::disabled();
    assert_eq!(rt.mode(), AnchorMode::LocalUnanchored);
    assert_eq!(rt.status().anchor_id, None);
    assert!(rt.status().head.is_none());
    // No anchor ⇒ an anchored read is a hard error (never fabricated history).
    assert_eq!(
        rt.fetch_verified(Utc::now()).await.unwrap_err(),
        rigorix_engine::audit::infrastructure::anchor::AnchorError::NotConfigured
    );
}

// ── verified head is bound into the envelope ─────────────────────────

fn envelope_input() -> BuildEnvelopeInput {
    BuildEnvelopeInput {
        execution_id: uuid::Uuid::new_v4(),
        template_id: "e2e".into(),
        planning_prompt: "plan".into(),
        events: vec![ExecutionEventRef {
            event_type: "task_completed".into(),
            summary: "done".into(),
            occurred_at: Utc::now(),
            correlation_id: None,
            status: EventStatus::Success,
            payload: None,
        }],
        source: None,
        repository: None,
        author: None,
        identity: None,
        effect_key: None,
        producer_id: None,
        sequence: None,
        prev_hash: None,
        history_policy: None,
        total_tokens: 0,
        duration_ms: 0,
        git_commit: None,
        git_branch: None,
        model_version: None,
        planning_prompt_content: None,
        file_paths: Vec::new(),
        metadata: None,
        sign: false,
        scoring_results: std::collections::HashMap::new(),
    }
}

#[tokio::test]
async fn anchored_envelope_records_mode_and_verified_head() {
    let server = MockServer::start().await;
    mount_slice(
        &server,
        &signed_slice(vec![action("registration_remove", "jeff@corp", 120)]),
    )
    .await;
    let rt = runtime(&server.uri());
    let _ = rt
        .fetch_verified(Utc::now() - chrono::Duration::hours(1))
        .await
        .unwrap();

    let envelope = AuditEnvelopeFactoryImpl::default()
        .with_anchor(rt)
        .build_envelope(envelope_input())
        .await
        .unwrap();
    assert_eq!(envelope.history_integrity, Some(HistoryIntegrity::Anchored));
    assert_eq!(envelope.anchor_head, Some(HEAD.to_string()));

    // No anchor ⇒ unchanged `local_unanchored` tagging.
    let local = AuditEnvelopeFactoryImpl::default()
        .build_envelope(envelope_input())
        .await
        .unwrap();
    assert_eq!(
        local.history_integrity,
        Some(HistoryIntegrity::LocalUnanchored)
    );
    assert!(local.anchor_head.is_none());
}

// Keep the unused-import checker honest about `DateTime`.
#[allow(dead_code)]
fn _assert_dt(_: DateTime<Utc>) {}

//! Live anchored-path E2E against a real enterprise anchor (#912).
//!
//! This is the **wire** counterpart to `anchored_wire_e2e.rs`: it drives the
//! real [`HttpAnchorClient`] at `RIGORIX_E2E_ANCHOR_URL` (the enterprise's
//! `GET /v1/history`, bare signed slice, Bearer auth) and asserts the four
//! outcomes. It is **env-gated** — with `RIGORIX_E2E_ANCHOR_URL` unset the
//! test prints a skip notice and returns (so the normal suite stays green
//! without an enterprise).
//!
//! Required env (set by `demo/anchor-e2e/run.sh`):
//! - `RIGORIX_E2E_ANCHOR_URL`      e.g. `http://127.0.0.1:3000`
//! - `RIGORIX_E2E_ANCHOR_PUBLIC_KEY` hex Ed25519 public key of the anchor
//! - `RIGORIX_E2E_ANCHOR_TOKEN`    bearer credential (`rgx_...` API key)
//! - `RIGORIX_E2E_ANCHOR_SCOPE`    producer id (default `local`)
//! - `RIGORIX_E2E_PRIOR_NODE`      prior node to expect (default
//!   `registration_remove`)

use std::sync::Arc;

use chrono::Utc;
use serde_json::json;

use rigorix_engine::audit::application::{AuditEnvelopeFactoryImpl, BuildEnvelopeInput};
use rigorix_engine::audit::domain::anchor::AnchorMode;
use rigorix_engine::audit::domain::{EventStatus, ExecutionEventRef, HistoryIntegrity};
use rigorix_engine::audit::infrastructure::anchor::{AnchorRuntime, HttpAnchorClient};
use rigorix_engine::sequence_policy::application::PlannedStep;
use rigorix_engine::sequence_policy::application::SequencePolicyServiceImpl;
use rigorix_engine::sequence_policy::domain::{
    HistoryPredicate, ParamMatchKind, ParamPredicate, RuleAction, SequencePolicyConfig,
    SequencePolicyError, SequenceRule, StepPredicate,
};
use rigorix_engine::sequence_policy::infrastructure::{
    AnchoredHistoryAdapter, SequencePolicyRepository,
};

use rigorix_engine::audit::application::factory::AuditEnvelopeFactory as _;
use rigorix_engine::sequence_policy::application::service::SequencePolicyService as _;

struct LiveEnv {
    url: String,
    public_key: String,
    token: String,
    scope: String,
    prior_node: String,
}

fn live_env() -> Option<LiveEnv> {
    let url = std::env::var("RIGORIX_E2E_ANCHOR_URL").ok()?;
    let public_key = std::env::var("RIGORIX_E2E_ANCHOR_PUBLIC_KEY")
        .expect("RIGORIX_E2E_ANCHOR_PUBLIC_KEY is required when RIGORIX_E2E_ANCHOR_URL is set");
    let token = std::env::var("RIGORIX_E2E_ANCHOR_TOKEN")
        .expect("RIGORIX_E2E_ANCHOR_TOKEN is required when RIGORIX_E2E_ANCHOR_URL is set");
    Some(LiveEnv {
        url,
        public_key,
        token,
        scope: std::env::var("RIGORIX_E2E_ANCHOR_SCOPE").unwrap_or_else(|_| "local".into()),
        prior_node: std::env::var("RIGORIX_E2E_PRIOR_NODE")
            .unwrap_or_else(|_| "registration_remove".into()),
    })
}

fn runtime(env: &LiveEnv, public_key: &str) -> Arc<AnchorRuntime> {
    AnchorRuntime::anchored(
        Arc::new(HttpAnchorClient::new_with_token(
            env.url.clone(),
            Some(env.token.clone()),
        )),
        public_key.to_string(),
        "e2e-enterprise",
        env.scope.clone(),
    )
}

fn deny_config(prior_node: &str) -> SequencePolicyConfig {
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
                prior_node: prior_node.into(),
                same_principal: true,
                window_secs: 86_400,
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

fn service(runtime: Arc<AnchorRuntime>, prior_node: &str) -> SequencePolicyServiceImpl {
    SequencePolicyServiceImpl::new(Box::new(FixedRepo(Some(deny_config(prior_node)))))
        .with_history(Arc::new(AnchoredHistoryAdapter::new(runtime)))
}

async fn deny_matches(svc: &SequencePolicyServiceImpl) -> Result<usize, SequencePolicyError> {
    let step = PlannedStep {
        name: "add".into(),
        tool: "registration_add".into(),
        parameters: json!({ "event_id": "conf-2026" }),
    };
    let m = svc.evaluate_plan(&[step], Some("jeff@corp")).await?;
    Ok(m.len())
}

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

macro_rules! skip_if_no_anchor {
    () => {
        match live_env() {
            Some(env) => env,
            None => {
                eprintln!("RIGORIX_E2E_ANCHOR_URL unset — skipping live anchor E2E");
                return;
            }
        }
    };
}

/// 1. A real enterprise-signed slice verifies and drives the deny-class run.
#[tokio::test]
async fn live_valid_slice_verifies_and_enforces() {
    let env = skip_if_no_anchor!();
    let rt = runtime(&env, &env.public_key);
    let actions = rt
        .fetch_verified(Utc::now() - chrono::Duration::hours(24))
        .await
        .expect("the enterprise's signed slice must verify");
    assert!(
        actions.iter().any(|a| a.node == env.prior_node),
        "seeded prior '{}' must be present: {actions:?}",
        env.prior_node
    );
    assert!(rt.head().is_some(), "verified head recorded");

    let svc = service(rt.clone(), &env.prior_node);
    assert_eq!(
        deny_matches(&svc).await.expect("evaluated"),
        1,
        "the deny-class rule fires on the authenticated history"
    );

    // The verified head + mode are bound into the next envelope.
    let envelope = AuditEnvelopeFactoryImpl::default()
        .with_anchor(rt)
        .build_envelope(envelope_input())
        .await
        .unwrap();
    assert_eq!(envelope.history_integrity, Some(HistoryIntegrity::Anchored));
    assert!(envelope.anchor_head.is_some());
}

/// 2. Anchor down ⇒ consequential run fails closed.
#[tokio::test]
async fn live_anchor_down_fails_closed() {
    let env = skip_if_no_anchor!();
    let dead = LiveEnv {
        url: "http://127.0.0.1:1".into(),
        ..env
    };
    let svc = service(runtime(&dead, &dead.public_key), &dead.prior_node);
    assert!(deny_matches(&svc).await.is_err());
}

/// 3. A slice the configured key cannot authenticate ⇒ fails closed (forged).
#[tokio::test]
async fn live_forged_slice_fails_closed() {
    let env = skip_if_no_anchor!();
    // A genuine slice verified against the WRONG public key is indistinguishable
    // from a forged anchor identity — the guard must refuse.
    let wrong_key = "00".repeat(32);
    let svc = service(runtime(&env, &wrong_key), &env.prior_node);
    assert!(deny_matches(&svc).await.is_err());
}

/// 4. `local_unanchored` (no anchor configured) is unchanged.
#[tokio::test]
async fn live_local_unanchored_is_unchanged() {
    let _env = skip_if_no_anchor!();
    let rt = AnchorRuntime::disabled();
    assert_eq!(rt.mode(), AnchorMode::LocalUnanchored);
    assert!(rt.status().anchor_id.is_none());
    assert!(rt.status().head.is_none());
    // A no-anchor envelope stays `local_unanchored` with no head.
    let envelope = AuditEnvelopeFactoryImpl::default()
        .build_envelope(envelope_input())
        .await
        .unwrap();
    assert_eq!(
        envelope.history_integrity,
        Some(HistoryIntegrity::LocalUnanchored)
    );
    assert!(envelope.anchor_head.is_none());
}

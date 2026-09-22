//! R9 operator-controlled step requirements (ADR-015) — unit contract tests.
//!
//! Acceptance criteria covered:
//! - AC 18: `require_identity` refuses an unauthenticated plan even when the
//!   matched step does not set any flag; identity is never promotable.
//! - AC 19: `require_params` denies a step missing a pointer / allows it when
//!   present; `promote` sets the promotion action for parameter obligations.
//! - AC 22: malformed requirements fail closed; absent = status quo; caps
//!   bound the count and the required pointers.
//! - Determinism: same plan + config → same finding set.

use async_trait::async_trait;
use rigorix_engine::sequence_policy::application::{
    PlannedStep, SequencePolicyService, SequencePolicyServiceImpl,
};
use rigorix_engine::sequence_policy::domain::{
    ParamMatchKind, ParamPredicate, RequirementAction, SafetyCaps, SequencePolicyConfig,
    SequencePolicyError, StepPredicate, StepRequirement,
};
use rigorix_engine::sequence_policy::infrastructure::{
    SequencePolicyRepository, TomlSequencePolicyRepository,
};

use serde_json::json;

/// In-memory repository serving a fixed config / outcome.
struct StubRepository {
    outcome: Result<Option<SequencePolicyConfig>, SequencePolicyError>,
}

#[async_trait]
impl SequencePolicyRepository for StubRepository {
    async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
        self.outcome.clone()
    }
}

fn requirement() -> StepRequirement {
    StepRequirement {
        id: "payout-guard".to_string(),
        name: "Payout commands must be attested and carry canonical effect data".to_string(),
        description: "d".to_string(),
        r#match: StepPredicate {
            tool: "run_command".to_string(),
            params: vec![ParamPredicate {
                pointer: "/command".to_string(),
                kind: ParamMatchKind::Glob,
                value: Some("*execute_payout.sh*".to_string()),
                step: None,
            }],
        },
        require_identity: true,
        require_params: vec!["/beneficiary".to_string(), "/effect_key".to_string()],
        action: RequirementAction::Deny,
    }
}

fn config_with(requirements: Vec<StepRequirement>) -> SequencePolicyConfig {
    SequencePolicyConfig {
        fail_closed: true,
        requirements,
        rules: Vec::new(),
    }
}

fn service(config: SequencePolicyConfig) -> SequencePolicyServiceImpl {
    SequencePolicyServiceImpl::new(Box::new(StubRepository {
        outcome: Ok(Some(config)),
    }))
}

fn payout_step(params: serde_json::Value) -> PlannedStep {
    PlannedStep {
        name: "pay".to_string(),
        tool: "run_command".to_string(),
        parameters: params,
    }
}

fn payout_command() -> serde_json::Value {
    json!({ "command": "bash .rigorix/scripts/execute_payout.sh acct 100" })
}

// ── AC 18: identity obligation ────────────────────────────────────────────

#[tokio::test]
async fn require_identity_refuses_unauthenticated_composed_step() {
    // The plan step carries no `require_identity` flag (PlannedStep has no
    // such field — the point of ADR-015: the operator obligation is
    // independent of the plan). The requirement still refuses it.
    let svc = service(config_with(vec![requirement()]));
    let findings = svc
        .evaluate_requirements(&[payout_step(payout_command())], false)
        .await
        .expect("evaluate");
    assert_eq!(findings.len(), 1, "one requirement fired: {findings:?}");
    let f = &findings[0];
    assert_eq!(f.requirement_id, "payout-guard");
    assert_eq!(f.step, "pay");
    assert!(f.unmet_identity, "identity obligation unmet");
    assert_eq!(f.action, RequirementAction::Deny);
}

#[tokio::test]
async fn require_identity_is_never_promotable() {
    // Even a `promote` requirement refuses when the identity is unmet.
    let mut req = requirement();
    req.action = RequirementAction::Promote;
    let svc = service(config_with(vec![req]));
    let findings = svc
        .evaluate_requirements(&[payout_step(payout_command())], false)
        .await
        .expect("evaluate");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].action,
        RequirementAction::Deny,
        "identity failures are never promotable"
    );
}

// ── AC 19: parameter obligations ──────────────────────────────────────────

#[tokio::test]
async fn require_params_denies_when_pointer_missing_and_allows_when_present() {
    let mut req = requirement();
    req.require_identity = false; // isolate the parameter obligation
    let svc = service(config_with(vec![req]));

    // Missing both pointers → deny finding naming both.
    let findings = svc
        .evaluate_requirements(&[payout_step(payout_command())], true)
        .await
        .expect("evaluate");
    assert_eq!(findings.len(), 1);
    assert!(!findings[0].unmet_identity);
    assert_eq!(
        findings[0].unmet_params,
        vec!["/beneficiary", "/effect_key"]
    );
    assert_eq!(findings[0].action, RequirementAction::Deny);

    // Present + non-null → no finding.
    let mut present = payout_command();
    present["beneficiary"] = json!("acct-1");
    present["effect_key"] = json!("eff-1");
    let findings = svc
        .evaluate_requirements(&[payout_step(present)], true)
        .await
        .expect("evaluate");
    assert!(findings.is_empty(), "satisfied requirement: {findings:?}");
}

#[tokio::test]
async fn require_params_promote_action_reports_promote() {
    let mut req = requirement();
    req.require_identity = false;
    req.action = RequirementAction::Promote;
    let svc = service(config_with(vec![req]));
    let findings = svc
        .evaluate_requirements(&[payout_step(payout_command())], true)
        .await
        .expect("evaluate");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].action, RequirementAction::Promote);
}

// ── Determinism property ──────────────────────────────────────────────────

#[tokio::test]
async fn same_plan_and_config_produce_the_same_finding_set() {
    let svc = service(config_with(vec![requirement()]));
    let steps = [payout_step(payout_command())];
    let first = svc
        .evaluate_requirements(&steps, false)
        .await
        .expect("evaluate");
    for _ in 0..50 {
        let again = svc
            .evaluate_requirements(&steps, false)
            .await
            .expect("evaluate");
        assert_eq!(first, again, "requirement evaluation must be deterministic");
    }
}

// ── AC 22: absent / malformed ─────────────────────────────────────────────

#[tokio::test]
async fn absent_requirements_is_status_quo() {
    let svc = service(config_with(Vec::new()));
    let findings = svc
        .evaluate_requirements(&[payout_step(payout_command())], false)
        .await
        .expect("evaluate");
    assert!(findings.is_empty());

    // No config file → fail-open-absent, still no findings.
    let no_file = SequencePolicyServiceImpl::new(Box::new(StubRepository { outcome: Ok(None) }));
    assert!(
        no_file
            .evaluate_requirements(&[payout_step(payout_command())], false)
            .await
            .expect("evaluate")
            .is_empty()
    );
}

#[tokio::test]
async fn malformed_config_fails_closed() {
    let svc = SequencePolicyServiceImpl::new(Box::new(StubRepository {
        outcome: Err(SequencePolicyError::InvalidConfig(
            "bad requirements".to_string(),
        )),
    }));
    let err = svc
        .evaluate_requirements(&[payout_step(payout_command())], true)
        .await
        .expect_err("fail closed");
    assert!(!err.is_retriable());
}

#[test]
fn safety_caps_bound_requirements_count_and_pointers() {
    let caps = SafetyCaps {
        max_rules_per_file: 100,
        max_steps_per_rule: 8,
        max_window: 5,
        max_regex_predicates_per_file: 8,
        max_history_window_secs: 604_800,
        max_requirements_per_file: 1,
        max_required_params_per_requirement: 1,
    };
    let over_count = config_with(vec![
        requirement(),
        StepRequirement {
            id: "second".to_string(),
            ..requirement()
        },
    ]);
    assert!(over_count.validate(&caps).is_err());

    let mut over_pointers = requirement();
    over_pointers.require_params = vec!["/a".to_string(), "/b".to_string()]; // cap = 1
    assert!(config_with(vec![over_pointers]).validate(&caps).is_err());
}

// ── Repository: `[[requirements]]` parse → validate ───────────────────────

#[tokio::test]
async fn toml_requirements_block_parses_and_validates() {
    let toml = r#"
fail_closed = true

[[requirements]]
id = "payout-guard"
name = "Payout commands must be attested and carry canonical effect data"
description = "raw run_command must not reach the payout script"
match = { tool = "run_command", params = [{ pointer = "/command", kind = "glob", value = "*execute_payout.sh*" }] }
require_identity = true
require_params = ["/beneficiary", "/effect_key"]
action = "deny"
"#;
    let path = std::env::temp_dir().join(format!("rigorix-r9-{}.toml", uuid::Uuid::new_v4()));
    std::fs::write(&path, toml).expect("write");
    let repo = TomlSequencePolicyRepository::new(&path);
    let config = repo
        .load_config()
        .await
        .expect("load")
        .expect("config present");
    assert_eq!(config.requirements.len(), 1);
    let req = &config.requirements[0];
    assert_eq!(req.id, "payout-guard");
    assert!(req.require_identity);
    assert_eq!(req.require_params, vec!["/beneficiary", "/effect_key"]);
    assert_eq!(req.action, RequirementAction::Deny);
    assert_eq!(req.r#match.params[0].kind, ParamMatchKind::Glob);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn toml_malformed_requirement_fails_closed() {
    // No obligation at all is a fail-closed config error.
    let toml = r#"
[[requirements]]
id = "no-op"
name = "n"
match = { tool = "run_command" }
"#;
    let path = std::env::temp_dir().join(format!("rigorix-r9-bad-{}.toml", uuid::Uuid::new_v4()));
    std::fs::write(&path, toml).expect("write");
    let repo = TomlSequencePolicyRepository::new(&path);
    let err = repo.load_config().await.expect_err("fail closed");
    assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
    let _ = std::fs::remove_file(&path);
}

//! #889 (OSS-C5) — bundle ingestion + local-vs-bundle precedence, exercised
//! through the public API (runs under the default `cargo test`, no SDK
//! checkout required).
//!
//! The full schema-validated round-trip lives in
//! `tests/conformance/policy_bundle.rs`; this file proves the same behavior is
//! covered by the normal suite.

use rigorix_engine::sequence_policy::application::{
    PlannedStep, SequencePolicyService, SequencePolicyServiceImpl,
};
use rigorix_engine::sequence_policy::domain::{
    RequirementAction, RuleAction, SequencePolicyConfig, SequencePolicyError,
};
use rigorix_engine::sequence_policy::infrastructure::{
    BundleSequencePolicyRepository, PrecedenceSequencePolicyRepository, SequencePolicyPrecedence,
    SequencePolicyRepository, TomlSequencePolicyRepository,
};

fn fixture_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/conformance/fixtures")
}

async fn local_config() -> SequencePolicyConfig {
    TomlSequencePolicyRepository::new(fixture_dir().join("enterprise-policy-bundle.toml"))
        .load_config()
        .await
        .expect("local TOML loads")
        .expect("local TOML present")
}

async fn bundle_config() -> SequencePolicyConfig {
    BundleSequencePolicyRepository::from_file(fixture_dir().join("enterprise-policy-bundle.json"))
        .load_config()
        .await
        .expect("bundle loads")
        .expect("bundle present")
}

struct FixedRepo(SequencePolicyConfig);

#[async_trait::async_trait]
impl SequencePolicyRepository for FixedRepo {
    async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
        Ok(Some(self.0.clone()))
    }
}

fn planned(name: &str, params: serde_json::Value) -> PlannedStep {
    PlannedStep {
        name: name.to_string(),
        tool: "run_command".to_string(),
        parameters: params,
    }
}

#[tokio::test]
async fn exported_bundle_and_local_toml_parse_identically() {
    assert_eq!(local_config().await, bundle_config().await);
}

#[tokio::test]
async fn exported_bundle_rejects_malformed_and_over_cap() {
    // Malformed shape → fail closed.
    let err = BundleSequencePolicyRepository::parse_bundle(&serde_json::json!({
        "rules": [{ "name": "missing id", "description": "d", "steps": [{ "tool": "t" }] }]
    }))
    .expect_err("must fail closed");
    assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));

    // Over safety caps → fail closed.
    let steps: Vec<serde_json::Value> = (0..9)
        .map(|i| serde_json::json!({ "tool": format!("t{i}") }))
        .collect();
    let err = BundleSequencePolicyRepository::parse_bundle(&serde_json::json!({
        "rules": [{ "id": "over", "name": "n", "description": "d", "steps": steps }]
    }))
    .expect_err("must fail closed");
    assert!(matches!(
        &err,
        SequencePolicyError::RuleExceedsCaps { rule, .. } if rule == "over"
    ));
}

#[tokio::test]
async fn precedence_refuses_conflicting_sources_and_picks_bundle_by_default() {
    let local = local_config().await;
    let bundle = bundle_config().await;

    // A deliberately different local config.
    let mut different = local.clone();
    different.rules[0].id = "local-only-rule".to_string();

    let refuse = PrecedenceSequencePolicyRepository::new(
        Some(Box::new(FixedRepo(different.clone()))),
        Some(Box::new(FixedRepo(bundle.clone()))),
        SequencePolicyPrecedence::RefuseOnConflict,
    );
    let err = refuse
        .load_config()
        .await
        .expect_err("conflict fails closed");
    assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));

    let enterprise = PrecedenceSequencePolicyRepository::new(
        Some(Box::new(FixedRepo(different))),
        Some(Box::new(FixedRepo(bundle.clone()))),
        SequencePolicyPrecedence::EnterpriseWins,
    );
    assert_eq!(
        enterprise.load_config().await.expect("ok").expect("some"),
        bundle,
        "enterprise bundle is the source of truth by default"
    );
}

#[tokio::test]
async fn bundle_and_toml_produce_identical_verdicts() {
    let local = SequencePolicyServiceImpl::new(Box::new(FixedRepo(local_config().await)));
    let bundle = SequencePolicyServiceImpl::new(Box::new(FixedRepo(bundle_config().await)));

    // deny
    let deny = vec![
        planned(
            "remove",
            serde_json::json!({ "command": "bash remove_attendance.sh", "beneficiary": "alice" }),
        ),
        planned(
            "add",
            serde_json::json!({ "command": "bash add_attendance.sh", "beneficiary": "alice" }),
        ),
    ];
    let l = local.evaluate_plan(&deny, None).await.expect("local");
    let b = bundle.evaluate_plan(&deny, None).await.expect("bundle");
    assert_eq!(l, b);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].rule_id, "seat-remove-then-reassign");
    assert_eq!(l[0].action, RuleAction::Deny);
    assert_eq!(l[0].later_step, "add");

    // promote
    let promote = vec![
        planned(
            "verify",
            serde_json::json!({ "command": "bash verify_waitlist.sh" }),
        ),
        planned(
            "transfer",
            serde_json::json!({ "command": "bash transfer_seat.sh" }),
        ),
    ];
    let l = local.evaluate_plan(&promote, None).await.expect("local");
    let b = bundle.evaluate_plan(&promote, None).await.expect("bundle");
    assert_eq!(l, b);
    assert_eq!(l[0].action, RuleAction::Promote);

    // allow
    let allow = vec![planned(
        "backup",
        serde_json::json!({ "command": "bash backup.sh" }),
    )];
    assert_eq!(
        local.evaluate_plan(&allow, None).await.expect("local"),
        bundle.evaluate_plan(&allow, None).await.expect("bundle")
    );
}

#[tokio::test]
async fn requirements_findings_identical_across_sources() {
    let local = SequencePolicyServiceImpl::new(Box::new(FixedRepo(local_config().await)));
    let bundle = SequencePolicyServiceImpl::new(Box::new(FixedRepo(bundle_config().await)));

    let payout = vec![planned(
        "pay",
        serde_json::json!({ "command": "bash execute_payout.sh acct-1" }),
    )];
    let l = local
        .evaluate_requirements(&payout, false)
        .await
        .expect("local");
    let b = bundle
        .evaluate_requirements(&payout, false)
        .await
        .expect("bundle");
    assert_eq!(l, b);
    assert_eq!(l[0].requirement_id, "payout-guard");
    assert_eq!(l[0].action, RequirementAction::Deny);
    assert!(l[0].unmet_identity);
}

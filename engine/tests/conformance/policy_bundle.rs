//! OSS-C5 (#889) conformance: an enterprise-exported policy bundle and the
//! equivalent local operator TOML parse to the same `SequencePolicyConfig` and
//! produce **identical** verdicts.
//!
//! This is the OSS half of enterprise C3's AC #6 ("what the org exported" ==
//! "what the agent enforces"). It exercises R7/R8/R9 together:
//! - an ADR-014 `equals_step` predicate (value identity across steps),
//! - R9 `[[requirements]]` in both `deny` (attestation) and `promote`
//!   (parameter obligation) shapes,
//! - and the `deny` / `promote` / `allow` rule actions.
//!
//! Fixtures (provenance in `fixtures/`):
//! - `fixtures/enterprise-policy-bundle.json` — the `policy.json` v1 export shape
//! - `fixtures/enterprise-policy-bundle.toml` — its `sequence-policy.toml` twin

use rigorix_engine::sequence_policy::application::PlannedStep;
use rigorix_engine::sequence_policy::application::SequencePolicyService;
use rigorix_engine::sequence_policy::application::SequencePolicyServiceImpl;
use rigorix_engine::sequence_policy::domain::{
    RequirementAction, RuleAction, SequenceMatch, SequencePolicyConfig,
};
use rigorix_engine::sequence_policy::infrastructure::{
    BundleSequencePolicyRepository, SequencePolicyRepository, TomlSequencePolicyRepository,
};

use crate::{assert_valid, load_schema};

fn fixture_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/conformance/fixtures")
}

async fn local_config() -> SequencePolicyConfig {
    let repo =
        TomlSequencePolicyRepository::new(fixture_dir().join("enterprise-policy-bundle.toml"));
    repo.load_config()
        .await
        .expect("local TOML loads")
        .expect("local TOML present")
}

async fn bundle_config() -> SequencePolicyConfig {
    let repo = BundleSequencePolicyRepository::from_file(
        fixture_dir().join("enterprise-policy-bundle.json"),
    );
    repo.load_config()
        .await
        .expect("bundle loads")
        .expect("bundle present")
}

fn planned(name: &str, params: serde_json::Value) -> PlannedStep {
    PlannedStep {
        name: name.to_string(),
        tool: "run_command".to_string(),
        parameters: params,
    }
}

fn service(config: SequencePolicyConfig) -> SequencePolicyServiceImpl {
    SequencePolicyServiceImpl::new(Box::new(FixedRepo(config)))
}

/// In-memory repository wrapping a parsed config (the service only needs a
/// repository to re-read; both sources are compared as whole configs).
struct FixedRepo(SequencePolicyConfig);

#[async_trait::async_trait]
impl SequencePolicyRepository for FixedRepo {
    async fn load_config(
        &self,
    ) -> Result<
        Option<SequencePolicyConfig>,
        rigorix_engine::sequence_policy::domain::SequencePolicyError,
    > {
        Ok(Some(self.0.clone()))
    }
}

fn fingerprint(matches: &[SequenceMatch]) -> Vec<(String, RuleAction, String)> {
    matches
        .iter()
        .map(|m| (m.rule_id.clone(), m.action, m.later_step.clone()))
        .collect()
}

#[tokio::test]
async fn bundle_and_local_toml_parse_identically_and_conform_to_schema() {
    let local = local_config().await;
    let bundle = bundle_config().await;

    assert_eq!(
        local, bundle,
        "the enterprise bundle and the equivalent local TOML must parse to the same config"
    );

    // The bundle is a policy.json v1 export → validate it with engine serde.
    let value = serde_json::to_value(&bundle).expect("SequencePolicyConfig serializes");
    assert_valid(
        &load_schema("policy.json"),
        &value,
        "enterprise policy bundle",
    );
}

#[tokio::test]
async fn deny_verdict_is_identical_and_carries_rule_id_and_step() {
    // remove(alice) → add(alice) with the SAME beneficiary (ADR-014
    // equals_step) matches the deny rule.
    let steps = vec![
        planned(
            "remove",
            serde_json::json!({ "command": "bash remove_attendance.sh conf-2026", "beneficiary": "alice" }),
        ),
        planned(
            "add",
            serde_json::json!({ "command": "bash add_attendance.sh conf-2026", "beneficiary": "alice" }),
        ),
    ];

    let local = service(local_config().await);
    let bundle = service(bundle_config().await);
    let local_v = local.evaluate_plan(&steps, None).await.expect("local eval");
    let bundle_v = bundle
        .evaluate_plan(&steps, None)
        .await
        .expect("bundle eval");

    assert_eq!(fingerprint(&local_v), fingerprint(&bundle_v));
    assert_eq!(local_v.len(), 1, "exactly the remove-then-reassign rule");
    let m = &local_v[0];
    assert_eq!(m.rule_id, "seat-remove-then-reassign");
    assert_eq!(m.action, RuleAction::Deny);
    // `denied_by_sequence` surfaces carry these two fields (AC #7).
    assert_eq!(m.later_step, "add");
    assert!(!m.rule_id.is_empty() && !m.later_step.is_empty());
}

#[tokio::test]
async fn different_beneficiary_does_not_match_the_equals_step_rule() {
    // Removing alice and adding bob is NOT the forbidden pair — value
    // identity is structural, so no deny.
    let steps = vec![
        planned(
            "remove",
            serde_json::json!({ "command": "bash remove_attendance.sh conf-2026", "beneficiary": "alice" }),
        ),
        planned(
            "add",
            serde_json::json!({ "command": "bash add_attendance.sh conf-2026", "beneficiary": "bob" }),
        ),
    ];
    let bundle = service(bundle_config().await);
    let matches = bundle.evaluate_plan(&steps, None).await.expect("eval");
    assert!(
        matches.is_empty(),
        "equals_step must not fire for different beneficiaries: {matches:?}"
    );
}

#[tokio::test]
async fn promote_verdict_is_identical() {
    let steps = vec![
        planned(
            "verify",
            serde_json::json!({ "command": "bash verify_waitlist.sh" }),
        ),
        planned(
            "transfer",
            serde_json::json!({ "command": "bash transfer_seat.sh" }),
        ),
    ];
    let local = service(local_config().await);
    let bundle = service(bundle_config().await);
    let local_v = local.evaluate_plan(&steps, None).await.expect("local eval");
    let bundle_v = bundle
        .evaluate_plan(&steps, None)
        .await
        .expect("bundle eval");

    assert_eq!(fingerprint(&local_v), fingerprint(&bundle_v));
    assert_eq!(local_v.len(), 1);
    assert_eq!(local_v[0].rule_id, "transfer-seat-is-critical");
    assert_eq!(local_v[0].action, RuleAction::Promote);
    assert_eq!(local_v[0].later_step, "transfer");
}

#[tokio::test]
async fn allow_verdict_is_identical() {
    let steps = vec![planned(
        "backup",
        serde_json::json!({ "command": "bash backup.sh" }),
    )];
    let local = service(local_config().await);
    let bundle = service(bundle_config().await);
    assert!(
        local
            .evaluate_plan(&steps, None)
            .await
            .expect("local")
            .is_empty(),
        "unrelated plan is allowed"
    );
    assert!(
        bundle
            .evaluate_plan(&steps, None)
            .await
            .expect("bundle")
            .is_empty(),
        "unrelated plan is allowed"
    );
}

#[tokio::test]
async fn requirements_deny_and_promote_findings_are_identical() {
    let local = service(local_config().await);
    let bundle = service(bundle_config().await);

    // Unattested payout command missing required params → deny finding.
    let payout = vec![planned(
        "pay",
        serde_json::json!({ "command": "bash execute_payout.sh acct-1" }),
    )];
    let local_deny = local
        .evaluate_requirements(&payout, false)
        .await
        .expect("local");
    let bundle_deny = bundle
        .evaluate_requirements(&payout, false)
        .await
        .expect("bundle");
    assert_eq!(local_deny, bundle_deny, "requirement findings must match");
    let deny = local_deny
        .iter()
        .find(|f| f.requirement_id == "payout-guard")
        .expect("payout-guard fires on an unattested payout command");
    assert!(deny.unmet_identity);
    assert_eq!(deny.action, RequirementAction::Deny);
    assert_eq!(deny.unmet_params, vec!["/beneficiary", "/effect_key"]);

    // Transfer without `/amount` → promote finding (parameter obligation only).
    let transfer = vec![planned(
        "transfer",
        serde_json::json!({ "command": "bash transfer_seat.sh" }),
    )];
    let local_promote = local
        .evaluate_requirements(&transfer, true)
        .await
        .expect("local");
    let bundle_promote = bundle
        .evaluate_requirements(&transfer, true)
        .await
        .expect("bundle");
    assert_eq!(local_promote, bundle_promote);
    let promote = local_promote
        .iter()
        .find(|f| f.requirement_id == "review-large-transfer")
        .expect("large-transfer requirement fires");
    assert_eq!(promote.action, RequirementAction::Promote);
    assert_eq!(promote.unmet_params, vec!["/amount"]);
    assert!(!promote.unmet_identity);
}

use super::*;
use crate::identity::domain::{IdentityRef, IdentitySource};
use crate::orchestrator::application::dto::TemplateStepDef;

fn step(name: &str, require_identity: bool) -> TemplateStepDef {
    TemplateStepDef {
        name: name.to_string(),
        tool: "run_command".into(),
        description: name.into(),
        parameters: serde_json::json!({}),
        requires_approval: false,
        require_identity,
        timeout_secs: None,
        evaluate_score: false,
    }
}

fn ref_with(source: IdentitySource) -> IdentityRef {
    IdentityRef {
        subject: "demo@corp.demo".into(),
        issuer: "local".into(),
        source,
        authority: None,
        expires_at: None,
    }
}

#[test]
fn no_identity_refuses_flagged_step() {
    let err = check_identity_gate(&[step("registration_remove", true)], None).unwrap_err();
    assert!(
        matches!(err, OrchestratorError::IdentityRequired { ref step, ref status } if step == "registration_remove" && status == "unauthenticated"),
        "unexpected: {err:?}"
    );
}

#[test]
fn unverified_identity_refuses_flagged_step() {
    let err = check_identity_gate(
        &[step("registration_add", true)],
        Some(&ref_with(IdentitySource::Unverified)),
    )
    .unwrap_err();
    assert!(
        matches!(err, OrchestratorError::IdentityRequired { ref status, .. } if status == "unverified")
    );
}

#[test]
fn attested_identity_allows_flagged_step() {
    for src in [IdentitySource::IdpToken, IdentitySource::LocalPrincipal] {
        assert!(
            check_identity_gate(&[step("registration_remove", true)], Some(&ref_with(src))).is_ok()
        );
    }
}

#[test]
fn unflaggged_steps_run_without_identity() {
    assert!(check_identity_gate(&[step("verify_attendance", false)], None).is_ok());
}

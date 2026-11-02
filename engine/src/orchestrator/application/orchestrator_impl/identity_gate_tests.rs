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
    let err = check_identity_gate(&[step("registration_remove", true)], None, None).unwrap_err();
    assert!(
        matches!(err, OrchestratorError::IdentityRequired { ref step, ref status, .. } if step == "registration_remove" && status == "unauthenticated"),
        "unexpected: {err:?}"
    );
}

#[test]
fn unverified_identity_refuses_flagged_step() {
    let err = check_identity_gate(
        &[step("registration_add", true)],
        Some(&ref_with(IdentitySource::Unverified)),
        None,
    )
    .unwrap_err();
    assert!(
        matches!(err, OrchestratorError::IdentityRequired { ref status, .. } if status == "unverified")
    );
}

/// #1277 — the refusal names the recovery that can help. A caller whose claim
/// is present but unchecked already logged in; `rigorix_auth_login` is the one
/// action that cannot fix it.
///
/// #1282 — and it states the *condition*, not a history. The gate cannot know
/// when discovery failed, so it may not say "when the server started": with the
/// cooldown retry that failure can happen mid-session, minutes after a healthy
/// start, and a restart is not required either way.
///
/// #1285 — and it names no stage. The gate sees an `IdentityRef`, not the
/// verifier: "discovery failed" was wrong for the common case (a session whose
/// IdP went away after discovery succeeded, so the JWKS fetch failed).
#[test]
fn unverified_refusal_states_the_condition_not_a_startup_history() {
    let err = check_identity_gate(
        &[step("registration_add", true)],
        Some(&ref_with(IdentitySource::Unverified)),
        None,
    )
    .unwrap_err();
    let rendered = err.to_string();
    assert!(
        rendered.contains("unverified"),
        "the refusal must use the same word as the status: {rendered}"
    );
    assert!(
        !rendered.contains("discovery"),
        "the gate cannot know which stage failed, so it must name neither: {rendered}"
    );
    assert!(
        rendered.contains("could not verify it"),
        "the refusal must name the condition that holds: {rendered}"
    );
    assert!(
        !rendered.contains("when the server started"),
        "the gate cannot know when discovery failed, so it must not assert it: {rendered}"
    );
    assert!(
        !rendered.contains("restart"),
        "a restart is not required — attestation re-attempts discovery: {rendered}"
    );
    assert!(
        rendered.contains("RIGORIX_IDP_ISSUER"),
        "the refusal must name the recovery that can help: {rendered}"
    );
    assert!(
        rendered.contains("rigorix_auth_status"),
        "the refusal must say where the recorded error is: {rendered}"
    );
    assert!(
        !rendered.contains("rigorix_auth_login"),
        "the refusal must not point at the action that cannot help: {rendered}"
    );
}

/// #1285 — when the calling process recorded why verification failed, the
/// refusal quotes it and still names no stage. Measured case: discovery
/// succeeded, the IdP went away, the JWKS fetch failed.
#[test]
fn unverified_refusal_quotes_the_recorded_cause() {
    let err = check_identity_gate(
        &[step("registration_add", true)],
        Some(&ref_with(IdentitySource::Unverified)),
        Some("the JWKS fetch failed: connection refused"),
    )
    .unwrap_err();
    let rendered = err.to_string();
    assert!(
        rendered.contains("the JWKS fetch failed: connection refused"),
        "the refusal must quote the cause the verifier recorded: {rendered}"
    );
    assert!(
        rendered.contains("last credential-verification attempt failed"),
        "the condition is verification, whichever stage failed: {rendered}"
    );
    assert!(
        !rendered.contains("discovery"),
        "the refusal must not blame a stage: {rendered}"
    );
    assert!(
        rendered.contains("verification is re-attempted on the next call"),
        "the recovery follows what actually recovers: {rendered}"
    );
    assert!(
        !rendered.contains("re-attempts discovery"),
        "a cached verifier does not rediscover: {rendered}"
    );
    assert!(
        !rendered.contains("rigorix_auth_login"),
        "the caller did log in: {rendered}"
    );
}

/// An anonymous caller has no verification attempt to blame, so a cause is
/// never attached to `unauthenticated` (the gate drops it).
#[test]
fn unauthenticated_ignores_a_recorded_cause() {
    let err = check_identity_gate(
        &[step("registration_remove", true)],
        None,
        Some("stale reason from another session"),
    )
    .unwrap_err();
    let rendered = err.to_string();
    assert!(
        !rendered.contains("stale reason"),
        "no claim means no verification attempt to quote: {rendered}"
    );
    assert!(rendered.contains("rigorix_auth_login"), "{rendered}");
}

/// The anonymous case keeps the login guidance — that is the recovery there.
#[test]
fn unauthenticated_refusal_keeps_the_login_recovery() {
    let err = check_identity_gate(&[step("registration_remove", true)], None, None).unwrap_err();
    let rendered = err.to_string();
    assert!(
        rendered.contains("unauthenticated") && rendered.contains("rigorix_auth_login"),
        "unexpected refusal text: {rendered}"
    );
}

#[test]
fn attested_identity_allows_flagged_step() {
    for src in [IdentitySource::IdpToken, IdentitySource::LocalPrincipal] {
        assert!(
            check_identity_gate(
                &[step("registration_remove", true)],
                Some(&ref_with(src)),
                None
            )
            .is_ok()
        );
    }
}

#[test]
fn unflaggged_steps_run_without_identity() {
    assert!(check_identity_gate(&[step("verify_attendance", false)], None, None).is_ok());
}

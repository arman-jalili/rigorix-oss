//! Integration test: attestation with an unreachable IdP degrades to
//! `IdentitySource::Unverified` — explicit marker, no error.
//!
//! Covers identity module acceptance criterion #4:
//! "attest with unreachable IdP → `IdentitySource::Unverified`, explicit marker,
//! no error".
//!
//! Offline-first policy (ADR-012): an unreachable IdP (or the offline default
//! `NullVerifier`) degrades the claim to `Unverified` — the run/approval never
//! fails because the IdP is down.

use async_trait::async_trait;
use base64::Engine as _;

use rigorix_engine::identity::application::dto::AttestInput;
use rigorix_engine::identity::application::service::{
    IdentityAttestationService, VerificationOutcome,
};
use rigorix_engine::identity::application::service_impl::IdentityAttestationServiceImpl;
use rigorix_engine::identity::domain::{IdentityClaim, IdentityError, IdentitySource};
use rigorix_engine::identity::infrastructure::verifier::{NullVerifier, TokenVerifier};

/// Build a well-formed (unsigned) JWT with standard claims — signature
/// verification is a separate concern (best-effort).
fn make_jwt() -> String {
    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&serde_json::json!({
            "sub": "user@org",
            "iss": "https://idp.example.com",
            "exp": chrono::Utc::now().timestamp() + 600,
            "roles": ["admin"],
            "auth_method": "device_code",
        }))
        .expect("claims JSON"),
    );
    format!("{header}.{payload}.signature-placeholder")
}

/// Simulates an IdP that is unreachable on the network — verification is
/// attempted and cannot run, so the outcome is `Unverified` with a reason.
struct UnreachableIdpVerifier;

#[async_trait]
impl TokenVerifier for UnreachableIdpVerifier {
    async fn verify(
        &self,
        _token: &str,
        _claim: &IdentityClaim,
    ) -> Result<VerificationOutcome, IdentityError> {
        Ok(VerificationOutcome::Unverified {
            reason: "idp unreachable: connection refused".to_string(),
        })
    }
}

#[tokio::test]
async fn test_attest_with_unreachable_idp_degrades_to_unverified_no_error() {
    let service = IdentityAttestationServiceImpl::with_verifier(Box::new(UnreachableIdpVerifier));

    let result = service
        .attest(AttestInput {
            token: Some(make_jwt()),
            principal: None,
            issuer: Some("https://idp.example.com".to_string()),
            auth_method: Some("device_code".to_string()),
        })
        .await;

    // Explicit marker, no error — attestation never fail-closes on identity.
    let claim = result.expect("attest must NOT error when the IdP is unreachable");
    assert_eq!(
        claim.source,
        IdentitySource::Unverified,
        "claim must carry the explicit Unverified marker"
    );
    // Roles presented in an unverified token are not evidence.
    assert_eq!(claim.authority, None);
}

#[tokio::test]
async fn test_attest_offline_default_degrades_to_unverified() {
    // The offline default (NullVerifier) means no IdP is reachable.
    let service = IdentityAttestationServiceImpl::new();

    let claim = service
        .attest(AttestInput {
            token: Some(make_jwt()),
            principal: None,
            issuer: Some("https://idp.example.com".to_string()),
            auth_method: None,
        })
        .await
        .expect("attest must not error");

    assert_eq!(
        claim.source,
        IdentitySource::Unverified,
        "offline default marks the IdP-token claim Unverified explicitly"
    );
    assert_eq!(claim.subject, "user@org");
    assert_eq!(claim.authority, None, "unverified roles are not evidence");
}

#[tokio::test]
async fn test_attest_local_principal_stays_local_principal() {
    // A local principal is attributed locally — no IdP verification applies,
    // so the honest marker is LocalPrincipal (not Unverified).
    let service = IdentityAttestationServiceImpl::new();
    let claim = service
        .attest(AttestInput {
            token: None,
            principal: Some("os-user".to_string()),
            issuer: Some("local".to_string()),
            auth_method: None,
        })
        .await
        .expect("attest must not error");
    assert_eq!(claim.source, IdentitySource::LocalPrincipal);
    assert_eq!(claim.subject, "os-user");
}

#[tokio::test]
async fn test_attest_without_any_credential_is_an_error() {
    let service = IdentityAttestationServiceImpl::new();
    let result = service
        .attest(AttestInput {
            token: None,
            principal: None,
            issuer: None,
            auth_method: None,
        })
        .await;
    assert!(
        matches!(result, Err(IdentityError::MissingClaim(_))),
        "no presented identity => MissingClaim"
    );
}

#[test]
fn test_null_verifier_is_offline_default_and_returns_unverified() {
    // The offline default must be constructible and must yield Unverified
    // (never panic, never error) — this is what keeps attestation offline-capable.
    let verifier = NullVerifier::new();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let outcome = runtime
        .block_on(verifier.verify(
            "token",
            &IdentityClaim {
                subject: "user@org".to_string(),
                issuer: "https://idp.example.com".to_string(),
                authority: None,
                source: IdentitySource::IdpToken,
                auth_method: None,
                issued_at: chrono::Utc::now(),
                expires_at: None,
                token_ref: None,
            },
        ))
        .expect("NullVerifier never errors");
    assert!(matches!(outcome, VerificationOutcome::Unverified { .. }));
}

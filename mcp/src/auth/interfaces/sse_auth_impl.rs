//! SseAuthGateImpl — optional transport gate for non-localhost SSE binds.
//!
//! @canonical .pi/architecture/modules/auth.md#authhandler--sse-auth-interfaces
//! Implements: ISSUE-AUTH-5 — SSE Auth (Interfaces)
//! Issue: #825
//! ADR-008 §4, ADR-005 (amended): SSE transport auth for non-localhost binds
//!
//! Concrete implementation of the frozen [`SseAuthGate`] contract:
//!
//! - `none` — inert (default; localhost-only binds require no auth)
//! - `api_key` — constant-time comparison of the `X-API-Key` header against
//!   the configured static key
//! - `idp` — validates an RFC 6750 `Bearer` access token against the IdP
//!   JWKS (via OIDC discovery + the engine `JwksVerifier`). **Fail-closed**
//!   by design: unlike attestation, a network-exposed gateway must reject
//!   when it cannot verify — an unreachable IdP is an `SseAuthError`, a
//!   failed/absent verification is a `Deny` decision
//!
//! The gate is framework-agnostic (no axum types) — the SSE transport
//! adapts its middleware to this trait.

use async_trait::async_trait;
use std::sync::Arc;

use rigorix_engine::identity::{IdentityAttestationService, JwksVerifier, TokenVerifier};

use crate::auth::domain::error::{AuthError, SseAuthError};
use crate::auth::domain::value::Secret;
use crate::auth::infrastructure::IdpClient;
use crate::auth::interfaces::sse_auth::{SseAuthDecision, SseAuthGate, SseAuthMode};

/// Concrete [`SseAuthGate`] implementation.
pub struct SseAuthGateImpl {
    /// Enforcement mode.
    mode: SseAuthMode,

    /// Static key for `api_key` mode (constant-time compared).
    api_key: Option<Secret<String>>,

    /// IdP client used for OIDC discovery in `idp` mode.
    idp: Option<Arc<dyn IdpClient>>,

    /// Engine claim extraction (`idp` mode: bearer JWT → claim).
    attestation: Option<Arc<dyn IdentityAttestationService>>,

    /// JWKS verifier (overridable for tests; default built per request from
    /// the discovered `jwks_uri`).
    verifier_override: Option<Arc<dyn TokenVerifier>>,
}

impl SseAuthGateImpl {
    /// An inert gate (`mode = none` — localhost-only binds, ADR-005 default).
    pub fn disabled() -> Self {
        Self {
            mode: SseAuthMode::None,
            api_key: None,
            idp: None,
            attestation: None,
            verifier_override: None,
        }
    }

    /// A gate validating a configured static API key (`X-API-Key` header).
    pub fn api_key(key: Secret<String>) -> Self {
        Self {
            mode: SseAuthMode::ApiKey,
            api_key: Some(key),
            idp: None,
            attestation: None,
            verifier_override: None,
        }
    }

    /// A gate validating OIDC bearer access tokens against the IdP JWKS.
    ///
    /// `idp` supplies OIDC discovery (the `jwks_uri`); `attestation` supplies
    /// JWT claim extraction.
    pub fn idp(idp: Arc<dyn IdpClient>, attestation: Arc<dyn IdentityAttestationService>) -> Self {
        Self {
            mode: SseAuthMode::Idp,
            api_key: None,
            idp: Some(idp),
            attestation: Some(attestation),
            verifier_override: None,
        }
    }

    /// Test seam: pin the verifier instead of building one from discovery.
    pub fn idp_with_verifier(
        idp: Arc<dyn IdpClient>,
        attestation: Arc<dyn IdentityAttestationService>,
        verifier: Arc<dyn TokenVerifier>,
    ) -> Self {
        Self {
            mode: SseAuthMode::Idp,
            api_key: None,
            idp: Some(idp),
            attestation: Some(attestation),
            verifier_override: Some(verifier),
        }
    }

    /// Parse an RFC 6750 `Authorization: Bearer <token>` header value.
    fn parse_bearer(authorization: Option<&str>) -> Option<&str> {
        let value = authorization?.trim();
        let token = value.strip_prefix("Bearer ")?;
        let token = token.trim();
        if token.is_empty() { None } else { Some(token) }
    }

    /// Verify a bearer token in `idp` mode (fail-closed).
    async fn verify_idp_bearer(&self, token: &str) -> Result<SseAuthDecision, SseAuthError> {
        let idp = self
            .idp
            .as_deref()
            .ok_or_else(|| SseAuthError::NotConfigured("idp mode without IdpClient".into()))?;
        let attestation = self.attestation.as_deref().ok_or_else(|| {
            SseAuthError::NotConfigured("idp mode without attestation service".into())
        })?;

        // Bearer → claim (client-fault on unparseable tokens: a Deny, not an
        // error — the gate must not leak whether a token is merely invalid).
        let claim = match attestation.extract_claims(token) {
            Ok(claim) => claim,
            Err(_) => {
                return Ok(SseAuthDecision::Deny {
                    reason: "invalid bearer token".into(),
                });
            }
        };

        let verifier: Arc<dyn TokenVerifier> = match self.verifier_override.as_ref() {
            Some(v) => v.clone(),
            None => {
                // Discover jwks_uri from the IdP.
                let meta = idp.discover().await.map_err(|e| match e {
                    AuthError::Discovery { .. } | AuthError::Transport(_) => {
                        SseAuthError::IdpUnreachable(format!("IdP discovery failed: {e}"))
                    }
                    other => SseAuthError::Internal(other.to_string()),
                })?;
                let jwks_uri = meta.jwks_uri.ok_or_else(|| {
                    SseAuthError::NotConfigured(
                        "IdP discovery returned no jwks_uri — cannot verify bearer tokens".into(),
                    )
                })?;
                Arc::new(JwksVerifier::new(jwks_uri))
            }
        };

        let outcome = verifier
            .verify(token, &claim)
            .await
            .map_err(|e| SseAuthError::Internal(format!("token verification fault: {e}")))?;
        match outcome {
            rigorix_engine::identity::VerificationOutcome::Verified => Ok(SseAuthDecision::Allow),
            rigorix_engine::identity::VerificationOutcome::Unverified { reason } => {
                Ok(SseAuthDecision::Deny { reason })
            }
        }
    }
}

#[async_trait]
impl SseAuthGate for SseAuthGateImpl {
    fn mode(&self) -> SseAuthMode {
        self.mode
    }

    async fn authorize(
        &self,
        authorization_header: Option<&str>,
        api_key_header: Option<&str>,
    ) -> Result<SseAuthDecision, SseAuthError> {
        match self.mode {
            SseAuthMode::None => Ok(SseAuthDecision::Allow),
            SseAuthMode::ApiKey => {
                let configured = self.api_key.as_ref().ok_or_else(|| {
                    SseAuthError::NotConfigured("api_key mode without a key".into())
                })?;
                let presented = api_key_header.unwrap_or_default();
                if constant_time_eq(presented.as_bytes(), configured.expose().as_bytes()) {
                    Ok(SseAuthDecision::Allow)
                } else {
                    Ok(SseAuthDecision::Deny {
                        reason: "invalid or missing X-API-Key".into(),
                    })
                }
            }
            SseAuthMode::Idp => match Self::parse_bearer(authorization_header) {
                Some(token) => self.verify_idp_bearer(token).await,
                None => Ok(SseAuthDecision::Deny {
                    reason: "missing or malformed Authorization: Bearer header".into(),
                }),
            },
        }
    }
}

/// Constant-time byte comparison (no early exit on mismatch).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use rigorix_engine::identity::{
        IdentityClaim, IdentityError, IdentitySource, VerificationOutcome,
    };

    fn claim_for(token: &str) -> IdentityClaim {
        IdentityClaim {
            subject: format!("sub-{token}"),
            issuer: "https://idp.example.com".into(),
            authority: None,
            source: IdentitySource::IdpToken,
            auth_method: None,
            issued_at: chrono::Utc::now(),
            expires_at: None,
            token_ref: None,
        }
    }

    /// Stub attestation producing claims from any token.
    #[derive(Default)]
    struct StubAttestation;

    #[async_trait]
    impl IdentityAttestationService for StubAttestation {
        async fn attest(
            &self,
            _input: rigorix_engine::identity::AttestInput,
        ) -> Result<IdentityClaim, IdentityError> {
            Ok(claim_for("attested"))
        }

        fn extract_claims(&self, token: &str) -> Result<IdentityClaim, IdentityError> {
            if token.contains("invalid") {
                return Err(IdentityError::InvalidToken("bad token".into()));
            }
            Ok(claim_for(token))
        }

        async fn verify(
            &self,
            _claim: &IdentityClaim,
            _token: &str,
        ) -> Result<VerificationOutcome, IdentityError> {
            Ok(VerificationOutcome::Verified)
        }
    }

    /// Verifier scripted to a fixed outcome.
    struct ScriptedVerifier(VerificationOutcome);

    #[async_trait]
    impl TokenVerifier for ScriptedVerifier {
        async fn verify(
            &self,
            _token: &str,
            _claim: &IdentityClaim,
        ) -> Result<VerificationOutcome, IdentityError> {
            Ok(self.0.clone())
        }
    }

    /// Stub IdP (never reached when a verifier override is pinned).
    struct NeverIdp;

    #[async_trait]
    impl IdpClient for NeverIdp {
        async fn discover(&self) -> Result<crate::auth::infrastructure::IdpMetadata, AuthError> {
            Err(AuthError::Transport("unreachable".into()))
        }
        async fn device_authorization(
            &self,
            _client_id: &str,
        ) -> Result<crate::auth::infrastructure::DeviceAuthorization, AuthError> {
            unreachable!()
        }
        async fn poll_token(
            &self,
            _d: &Secret<String>,
            _c: &str,
        ) -> Result<crate::auth::infrastructure::TokenPoll, AuthError> {
            unreachable!()
        }
        async fn refresh_token(
            &self,
            _d: &Secret<String>,
            _c: &str,
        ) -> Result<crate::auth::infrastructure::TokenResponse, AuthError> {
            unreachable!()
        }
        async fn revoke_token(&self, _d: &Secret<String>, _c: &str) -> Result<(), AuthError> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn disabled_gate_always_allows() {
        let gate = SseAuthGateImpl::disabled();
        assert_eq!(gate.mode(), SseAuthMode::None);
        assert_eq!(
            gate.authorize(None, None).await.unwrap(),
            SseAuthDecision::Allow
        );
    }

    #[tokio::test]
    async fn api_key_gate_allows_matching_and_denies_others() {
        let gate = SseAuthGateImpl::api_key(Secret::new("sekret-key-123".into()));
        assert_eq!(gate.mode(), SseAuthMode::ApiKey);
        assert_eq!(
            gate.authorize(None, Some("sekret-key-123")).await.unwrap(),
            SseAuthDecision::Allow
        );
        assert!(matches!(
            gate.authorize(None, Some("wrong-key")).await.unwrap(),
            SseAuthDecision::Deny { .. }
        ));
        assert!(matches!(
            gate.authorize(None, None).await.unwrap(),
            SseAuthDecision::Deny { .. }
        ));
        // Authorization header is ignored in api_key mode.
        assert!(matches!(
            gate.authorize(Some("Bearer whatever"), None).await.unwrap(),
            SseAuthDecision::Deny { .. }
        ));
    }

    #[tokio::test]
    async fn idp_gate_allows_verified_bearer() {
        let gate = SseAuthGateImpl::idp_with_verifier(
            Arc::new(NeverIdp),
            Arc::new(StubAttestation),
            Arc::new(ScriptedVerifier(VerificationOutcome::Verified)),
        );
        assert_eq!(gate.mode(), SseAuthMode::Idp);
        assert_eq!(
            gate.authorize(Some("Bearer eyJ.abc.sig"), None)
                .await
                .unwrap(),
            SseAuthDecision::Allow
        );
    }

    #[tokio::test]
    async fn idp_gate_denies_unverified_bearer_fail_closed() {
        let gate = SseAuthGateImpl::idp_with_verifier(
            Arc::new(NeverIdp),
            Arc::new(StubAttestation),
            Arc::new(ScriptedVerifier(VerificationOutcome::Unverified {
                reason: "signature mismatch".into(),
            })),
        );
        let decision = gate
            .authorize(Some("Bearer eyJ.abc.sig"), None)
            .await
            .unwrap();
        match decision {
            SseAuthDecision::Deny { reason } => assert!(reason.contains("signature mismatch")),
            SseAuthDecision::Allow => panic!("unverified bearer must be denied"),
        }
    }

    #[tokio::test]
    async fn idp_gate_denies_missing_or_malformed_header() {
        let gate = SseAuthGateImpl::idp_with_verifier(
            Arc::new(NeverIdp),
            Arc::new(StubAttestation),
            Arc::new(ScriptedVerifier(VerificationOutcome::Verified)),
        );
        assert!(matches!(
            gate.authorize(None, None).await.unwrap(),
            SseAuthDecision::Deny { .. }
        ));
        assert!(matches!(
            gate.authorize(Some("Basic dXNlcjpwYXNz"), None)
                .await
                .unwrap(),
            SseAuthDecision::Deny { .. }
        ));
        assert!(matches!(
            gate.authorize(Some("Bearer "), None).await.unwrap(),
            SseAuthDecision::Deny { .. }
        ));
    }

    #[tokio::test]
    async fn idp_gate_denies_unparseable_token() {
        let gate = SseAuthGateImpl::idp_with_verifier(
            Arc::new(NeverIdp),
            Arc::new(StubAttestation),
            Arc::new(ScriptedVerifier(VerificationOutcome::Verified)),
        );
        let decision = gate
            .authorize(Some("Bearer invalid-token"), None)
            .await
            .unwrap();
        match decision {
            SseAuthDecision::Deny { reason } => assert_eq!(reason, "invalid bearer token"),
            SseAuthDecision::Allow => panic!("unparseable token must be denied"),
        }
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"", b"x"));
    }

    #[test]
    fn bearer_parsing() {
        assert_eq!(
            SseAuthGateImpl::parse_bearer(Some("Bearer tok-1")),
            Some("tok-1")
        );
        assert_eq!(
            SseAuthGateImpl::parse_bearer(Some("  Bearer tok-1  ")),
            Some("tok-1")
        );
        assert_eq!(SseAuthGateImpl::parse_bearer(Some("Basic x")), None);
        assert_eq!(SseAuthGateImpl::parse_bearer(Some("Bearer ")), None);
        assert_eq!(SseAuthGateImpl::parse_bearer(None), None);
    }
}

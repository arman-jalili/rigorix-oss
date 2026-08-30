//! IdentityAttestationServiceImpl — converts presented credentials into attested claims.
//!
//! @canonical .pi/architecture/modules/identity.md#identityattestationservice
//! Implements: ISSUE-IDENTITY-2 — attestation with explicit `Unverified` degradation
//! Issue: #702 (identity epic)
//!
//! Converts a presented IdP token (or local principal) into a structured,
//! time-bound `IdentityClaim`, with best-effort signature verification.
//!
//! # Degradation Contract (AC#4)
//!
//! Attestation **never fails closed on identity** (ADR-012): when the IdP is
//! unreachable (or verification is unavailable/disabled), the claim degrades
//! to `IdentitySource::Unverified` — an explicit, never-silent marker — and
//! `attest` returns `Ok(claim)`, **not an error**. The default verifier
//! (`NullVerifier`) is the offline default and yields `Unverified`.
//!
//! # OSS Attests / Enterprise Authorizes (ADR-012)
//!
//! This implementation makes **no authorization judgment** — it only records
//! who was presented as acting and marks the verification outcome.

use async_trait::async_trait;
use base64::Engine as _;

use crate::identity::application::dto::AttestInput;
use crate::identity::application::service::{IdentityAttestationService, VerificationOutcome};
use crate::identity::domain::{IdentityClaim, IdentityError, IdentitySource};
use crate::identity::infrastructure::verifier::{NullVerifier, TokenVerifier};

/// Default implementation of `IdentityAttestationService`.
///
/// Verification is best-effort via an injected `TokenVerifier`; the default is
/// `NullVerifier` (offline — attestation proceeds without network verification).
pub struct IdentityAttestationServiceImpl {
    /// Best-effort signature verifier (JWKS-backed when online).
    verifier: Box<dyn TokenVerifier>,
}

impl IdentityAttestationServiceImpl {
    /// Create the service with the offline default verifier (`NullVerifier`).
    pub fn new() -> Self {
        Self {
            verifier: Box::new(NullVerifier::new()),
        }
    }

    /// Create the service with a custom verifier (e.g. JWKS-backed).
    pub fn with_verifier(verifier: Box<dyn TokenVerifier>) -> Self {
        Self { verifier }
    }
}

impl Default for IdentityAttestationServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IdentityAttestationService for IdentityAttestationServiceImpl {
    /// Attest from a presented token/principal → `IdentityClaim`.
    ///
    /// # Observability (SpanPrivacy)
    /// The raw token is never logged — it is skipped from the tracing span
    /// (`.pi/architecture/modules/observability.md#privacy`). The redacted
    /// claim summary is emitted as the `subject` span field.
    #[tracing::instrument(skip_all, fields(subject = tracing::field::Empty))]
    async fn attest(&self, input: AttestInput) -> Result<IdentityClaim, IdentityError> {
        // 1. Build the initial claim from the presented credential.
        let mut claim = match (input.token.as_deref(), input.principal.as_deref()) {
            (Some(token), _) => self.extract_claims(token)?,
            (None, Some(principal)) => IdentityClaim {
                subject: principal.to_string(),
                issuer: input.issuer.clone().unwrap_or_else(|| "local".to_string()),
                authority: None,
                source: IdentitySource::LocalPrincipal,
                auth_method: input.auth_method.clone(),
                issued_at: chrono::Utc::now(),
                expires_at: None,
                token_ref: None,
            },
            (None, None) => {
                return Err(IdentityError::MissingClaim(
                    "presented identity: token or principal".to_string(),
                ));
            }
        };

        // 2. Best-effort verification — an unreachable IdP degrades the claim,
        //    it never turns attestation into an error (ADR-012 offline policy).
        if let Some(token) = input.token.as_deref() {
            match self.verify(&claim, token).await {
                Ok(VerificationOutcome::Verified) => {}
                Ok(VerificationOutcome::Unverified { .. }) => {
                    // Explicit degraded marker — never silent.
                    claim.source = IdentitySource::Unverified;
                    // Roles presented in an unverified token are not evidence.
                    claim.authority = None;
                }
                Err(_) => {
                    // Non-fatal for attestation: verification unavailability
                    // degrades the claim instead of failing the run/approval.
                    claim.source = IdentitySource::Unverified;
                    claim.authority = None;
                }
            }
        }

        tracing::Span::current()
            .record("subject", tracing::field::display(claim.redacted_summary()));

        Ok(claim)
    }

    /// Extract standard JWT claims (`sub`, `iss`, `exp`, `roles`) WITHOUT
    /// signature verification. Used for attestation; verification is separate
    /// and best-effort.
    ///
    /// # Observability (SpanPrivacy)
    /// The raw token is skipped from the tracing span — it never reaches logs.
    #[tracing::instrument(skip_all)]
    fn extract_claims(&self, token: &str) -> Result<IdentityClaim, IdentityError> {
        // JWT shape: header.payload.signature (all base64url segments).
        let segments: Vec<&str> = token.split('.').collect();
        if segments.len() != 3 {
            return Err(IdentityError::InvalidToken(format!(
                "expected 3 JWT segments, found {}",
                segments.len()
            )));
        }

        // Decode the payload segment (base64url, unpadded).
        let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(segments[1])
            .map_err(|e| IdentityError::InvalidToken(format!("payload decode: {e}")))?;

        let claims: serde_json::Value = serde_json::from_slice(&payload_bytes)
            .map_err(|e| IdentityError::InvalidToken(format!("payload JSON: {e}")))?;

        let subject = claims
            .get("sub")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| IdentityError::MissingClaim("sub".to_string()))?
            .to_string();

        let issuer = claims
            .get("iss")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| IdentityError::MissingClaim("iss".to_string()))?
            .to_string();

        // `exp` is a numeric date (seconds since epoch). Expired claims are
        // rejected at extraction — re-authentication is required.
        let expires_at = claims
            .get("exp")
            .and_then(serde_json::Value::as_i64)
            .map(|ts| chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0));
        if let Some(Some(exp)) = expires_at
            && exp <= chrono::Utc::now()
        {
            return Err(IdentityError::Expired);
        }

        // `roles` (standard JWT claim) → authority (captured fact, not judgment).
        let authority = match claims.get("roles") {
            Some(serde_json::Value::Array(roles)) => {
                let rendered: Vec<String> = roles
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(String::from)
                    .collect();
                if rendered.is_empty() {
                    None
                } else {
                    Some(rendered.join(", "))
                }
            }
            Some(serde_json::Value::String(role)) => Some(role.clone()),
            _ => None,
        };

        let issued_at = claims
            .get("iat")
            .and_then(serde_json::Value::as_i64)
            .and_then(|ts| chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0))
            .unwrap_or_else(chrono::Utc::now);

        Ok(IdentityClaim {
            subject,
            issuer,
            authority,
            source: IdentitySource::IdpToken,
            auth_method: claims
                .get("auth_method")
                .and_then(serde_json::Value::as_str)
                .map(String::from),
            issued_at,
            expires_at: expires_at.flatten(),
            token_ref: None,
        })
    }

    /// Best-effort signature verification against the IdP JWKS.
    ///
    /// Delegates to the injected `TokenVerifier` (default `NullVerifier` =
    /// offline). An unreachable IdP yields `VerificationOutcome::Unverified`,
    /// never an error.
    ///
    /// # Observability (SpanPrivacy)
    /// The raw token is skipped from the tracing span — it never reaches logs.
    #[tracing::instrument(skip_all, fields(claim = tracing::field::Empty))]
    async fn verify(
        &self,
        claim: &IdentityClaim,
        token: &str,
    ) -> Result<VerificationOutcome, IdentityError> {
        tracing::Span::current().record("claim", tracing::field::display(claim.redacted_summary()));
        self.verifier.verify(token, claim).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an unsigned JWT string from a claims payload (for extraction tests).
    fn make_jwt(claims: serde_json::Value) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(r#"{"alg":"none","typ":"JWT"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&claims).expect("claims JSON"));
        format!("{header}.{payload}.signature-placeholder")
    }

    #[test]
    fn extract_claims_decodes_standard_claims() {
        let token = make_jwt(serde_json::json!({
            "sub": "user@org",
            "iss": "https://idp.example.com",
            "exp": chrono::Utc::now().timestamp() + 600,
            "roles": ["admin", "dev"],
            "auth_method": "device_code",
        }));
        let service = IdentityAttestationServiceImpl::new();
        let claim = service.extract_claims(&token).expect("extract claims");

        assert_eq!(claim.subject, "user@org");
        assert_eq!(claim.issuer, "https://idp.example.com");
        assert_eq!(claim.source, IdentitySource::IdpToken);
        assert_eq!(claim.authority.as_deref(), Some("admin, dev"));
        assert_eq!(claim.auth_method.as_deref(), Some("device_code"));
        assert!(claim.is_valid());
    }

    #[test]
    fn extract_claims_rejects_malformed_token() {
        let service = IdentityAttestationServiceImpl::new();
        assert!(matches!(
            service.extract_claims("not-a-jwt"),
            Err(IdentityError::InvalidToken(_))
        ));
    }

    #[test]
    fn extract_claims_rejects_missing_required_claims() {
        let token = make_jwt(serde_json::json!({ "sub": "user@org" })); // no iss
        let service = IdentityAttestationServiceImpl::new();
        assert!(matches!(
            service.extract_claims(&token),
            Err(IdentityError::MissingClaim(ref c)) if c == "iss"
        ));
    }

    #[test]
    fn extract_claims_rejects_expired_token() {
        let token = make_jwt(serde_json::json!({
            "sub": "user@org",
            "iss": "https://idp.example.com",
            "exp": chrono::Utc::now().timestamp() - 60,
        }));
        let service = IdentityAttestationServiceImpl::new();
        assert!(matches!(
            service.extract_claims(&token),
            Err(IdentityError::Expired)
        ));
    }
}

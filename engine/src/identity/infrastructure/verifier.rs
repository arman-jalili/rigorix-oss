//! TokenVerifier — best-effort signature verification against the IdP JWKS.
//!
//! @canonical .pi/architecture/modules/identity.md#tokenverifier
//! Implements: ISSUE-IDENTITY-3 — JWKS-backed RS256 verification (AC#5)
//! Issue: #703 (identity epic)
//!
//! Best-effort signature verification keeps attestation fully offline-capable
//! (ADR-012 option c): an unreachable IdP yields
//! `VerificationOutcome::Unverified`, never an error. The `NullVerifier`
//! struct is the offline default — attestation proceeds without network
//! verification. The `JwksVerifier` fetches the IdP JWKS and validates the
//! token signature (RS256) when online.
//!
//! # Contract (Frozen)
//! - `verify` never errors on an unreachable IdP — it records `Unverified`
//! - `NullVerifier` is the offline default and is constructible via
//!   `NullVerifier::new()`
//! - `JwksVerifier` (JWKS-backed, RS256) lands in ISSUE-IDENTITY-3

use async_trait::async_trait;
use base64::Engine as _;
// RUSTSEC-2023-0071 (rsa, Marvin): verification-only usage — no private-key
// operation exists here. Replaced rsa with ring (maintained, BoringSSL-derived)
// which exposes no private-key path at all.
use ring::signature::{RSA_PKCS1_2048_8192_SHA256, RsaPublicKeyComponents};

use crate::identity::application::service::VerificationOutcome;
use crate::identity::domain::{IdentityClaim, IdentityError};

/// Best-effort signature verification against the IdP JWKS.
///
/// # Contract
/// - IdP reachable + valid signature → `VerificationOutcome::Verified`
/// - IdP reachable + tampered signature → `VerificationOutcome::Unverified`
/// - IdP unreachable → `VerificationOutcome::Unverified`, **never an error**
#[async_trait]
pub trait TokenVerifier: Send + Sync {
    /// Verify a token signature against the IdP JWKS.
    ///
    /// # Errors
    /// - `IdentityError::InvalidToken` — token is not a parseable JWT
    /// - `IdentityError::Internal` — unexpected verification failure
    ///
    /// An unreachable IdP is **not** an error — it yields
    /// `VerificationOutcome::Unverified`.
    async fn verify(
        &self,
        token: &str,
        claim: &IdentityClaim,
    ) -> Result<VerificationOutcome, IdentityError>;
}

/// Offline default — attestation proceeds without network verification
/// (ADR-012 option c).
///
/// # TODO
/// The `TokenVerifier` implementation for `NullVerifier` lands in
/// ISSUE-IDENTITY-4 (TokenVerifier). Contract stub — the type exists so
/// downstream code can depend on the offline default.
pub struct NullVerifier;

impl NullVerifier {
    /// Construct the offline default verifier.
    pub const fn new() -> Self {
        Self
    }
}

impl Default for NullVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TokenVerifier for NullVerifier {
    /// Offline default (ADR-012 option c): no IdP is reachable, so verification
    /// is unavailable and the outcome is explicitly `Unverified` — never an
    /// error. Attestation degrades the claim to `IdentitySource::Unverified`.
    /// Implemented in ISSUE-IDENTITY-2 (the offline default's stated contract).
    async fn verify(
        &self,
        _token: &str,
        _claim: &IdentityClaim,
    ) -> Result<VerificationOutcome, IdentityError> {
        Ok(VerificationOutcome::Unverified {
            reason: "verification disabled — offline default (NullVerifier)".to_string(),
        })
    }
}

/// JWKS-backed best-effort token verifier (RS256).
///
/// Fetches the IdP's JSON Web Key Set (`/.well-known/jwks.json`), resolves the
/// signing key by `kid`, and validates the RS256 signature. Best-effort: an
/// unreachable IdP, unknown `kid`, or verification failure yields
/// `VerificationOutcome::Unverified` — never an error (ADR-012 option c).
pub struct JwksVerifier {
    /// IdP JWKS endpoint (e.g. `https://idp.example.com/.well-known/jwks.json`).
    jwks_url: String,
    /// HTTP client with a bounded timeout.
    client: reqwest::Client,
}

impl JwksVerifier {
    /// Create a JWKS-backed verifier for the given IdP JWKS endpoint.
    pub fn new(jwks_url: impl Into<String>) -> Self {
        Self {
            jwks_url: jwks_url.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Parse the JWT header and extract the `kid` (and expected `alg`).
    fn header_kid_alg(token: &str) -> Result<(String, String), IdentityError> {
        let header_segment = token
            .split('.')
            .next()
            .ok_or_else(|| IdentityError::InvalidToken("missing header segment".to_string()))?;
        let header_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(header_segment)
            .map_err(|e| IdentityError::InvalidToken(format!("header decode: {e}")))?;
        let header: serde_json::Value = serde_json::from_slice(&header_bytes)
            .map_err(|e| IdentityError::InvalidToken(format!("header JSON: {e}")))?;
        let kid = header
            .get("kid")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| IdentityError::MissingClaim("kid".to_string()))?
            .to_string();
        let alg = header
            .get("alg")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("RS256")
            .to_string();
        Ok((kid, alg))
    }

    /// Fetch the JWKS document from the IdP.
    ///
    /// A fetch failure (unreachable IdP, non-200, timeout) is a best-effort
    /// outcome, NOT an error — the claim degrades to `Unverified`.
    async fn fetch_jwks(&self) -> Result<serde_json::Value, IdentityError> {
        let response = self
            .client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|e| IdentityError::VerificationUnavailable(format!("jwks fetch: {e}")))?;
        if !response.status().is_success() {
            return Err(IdentityError::VerificationUnavailable(format!(
                "jwks http {}",
                response.status()
            )));
        }
        response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| IdentityError::VerificationUnavailable(format!("jwks parse: {e}")))
    }
}

#[async_trait]
impl TokenVerifier for JwksVerifier {
    /// Verify a token signature against the IdP JWKS.
    ///
    /// # Outcomes
    /// - Valid signature for the `kid`'s key → `VerificationOutcome::Verified`
    /// - Tampered/mismatched signature → `VerificationOutcome::Unverified`
    /// - IdP unreachable / unknown kid → `VerificationOutcome::Unverified`, **never an error**
    ///
    /// # Errors
    /// - `IdentityError::InvalidToken` — token is not a parseable JWT
    async fn verify(
        &self,
        token: &str,
        _claim: &IdentityClaim,
    ) -> Result<VerificationOutcome, IdentityError> {
        // 1. Parse header (kid + alg). Malformed tokens are contract errors.
        let (kid, alg) = Self::header_kid_alg(token)?;
        if alg != "RS256" {
            return Ok(VerificationOutcome::Unverified {
                reason: format!("unsupported algorithm: {alg}"),
            });
        }

        // 2. Fetch the JWKS. Unreachable IdP → Unverified, never an error.
        let jwks = match self.fetch_jwks().await {
            Ok(jwks) => jwks,
            Err(e) => {
                return Ok(VerificationOutcome::Unverified {
                    reason: e.to_string(),
                });
            }
        };

        // 3. Resolve the signing key by kid.
        let key = match jwks
            .get("keys")
            .and_then(serde_json::Value::as_array)
            .and_then(|keys| {
                keys.iter().find(|k| {
                    k.get("kid").and_then(serde_json::Value::as_str) == Some(kid.as_str())
                })
            }) {
            Some(key) => key,
            None => {
                return Ok(VerificationOutcome::Unverified {
                    reason: format!("no JWKS key for kid: {kid}"),
                });
            }
        };

        // 4. Build the RSA public key from n/e (base64url, big-endian bytes —
        //    ring consumes the big-endian representation directly).
        let decode_rsa_param = |field: &str| -> Result<Vec<u8>, IdentityError> {
            let encoded = key
                .get(field)
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    IdentityError::VerificationUnavailable(format!("jwks missing {field}"))
                })?;
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(encoded)
                .map_err(|e| IdentityError::VerificationUnavailable(format!("jwks {field}: {e}")))
        };
        let n = decode_rsa_param("n")?;
        let e = decode_rsa_param("e")?;
        let public_key = RsaPublicKeyComponents {
            n: n.as_slice(),
            e: e.as_slice(),
        };

        // 5. Split the JWT into signed-content and signature segments.
        let segments: Vec<&str> = token.split('.').collect();
        if segments.len() != 3 {
            return Err(IdentityError::InvalidToken(
                "expected 3 JWT segments".to_string(),
            ));
        }
        let signed_content = format!("{}.{}", segments[0], segments[1]);
        let signature_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(segments[2])
            .map_err(|e| IdentityError::InvalidToken(format!("signature decode: {e}")))?;

        // 6. Verify RS256 over the signed content (ring hashes internally).
        match public_key.verify(
            &RSA_PKCS1_2048_8192_SHA256,
            signed_content.as_bytes(),
            &signature_bytes,
        ) {
            Ok(()) => Ok(VerificationOutcome::Verified),
            Err(_) => Ok(VerificationOutcome::Unverified {
                reason: "signature mismatch (tampered token)".to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_verifier_is_constructible() {
        let verifier = NullVerifier::new();
        let _offline_default: &dyn TokenVerifier = &verifier;
    }

    #[test]
    fn jwks_verifier_is_constructible_and_trait_object_safe() {
        let verifier = JwksVerifier::new("https://idp.example.com/.well-known/jwks.json");
        let _as_trait: &dyn TokenVerifier = &verifier;
    }

    #[test]
    fn header_kid_alg_extracts_kid_and_alg() {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(r#"{"alg":"RS256","kid":"key-123","typ":"JWT"}"#);
        let (kid, alg) =
            JwksVerifier::header_kid_alg(&format!("{header}.payload.sig")).expect("header parse");
        assert_eq!(kid, "key-123");
        assert_eq!(alg, "RS256");
    }

    #[test]
    fn header_kid_alg_defaults_alg_when_absent() {
        let header =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"kid":"key-123"}"#);
        let (kid, alg) =
            JwksVerifier::header_kid_alg(&format!("{header}.payload.sig")).expect("header parse");
        assert_eq!(kid, "key-123");
        assert_eq!(alg, "RS256"); // RS256 is the supported default
    }

    #[test]
    fn header_kid_alg_rejects_malformed_header() {
        // Not base64url → InvalidToken.
        assert!(matches!(
            JwksVerifier::header_kid_alg("!!!.payload.sig"),
            Err(IdentityError::InvalidToken(_))
        ));
        // Valid base64 but not JSON → InvalidToken.
        let not_json = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("not-json");
        assert!(matches!(
            JwksVerifier::header_kid_alg(&format!("{not_json}.payload.sig")),
            Err(IdentityError::InvalidToken(_))
        ));
        // Missing kid → MissingClaim.
        let no_kid = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256"}"#);
        assert!(matches!(
            JwksVerifier::header_kid_alg(&format!("{no_kid}.payload.sig")),
            Err(IdentityError::MissingClaim(ref c)) if c == "kid"
        ));
    }
}

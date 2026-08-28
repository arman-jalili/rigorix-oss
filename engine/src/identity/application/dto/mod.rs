//! Data Transfer Objects for the Identity Attestation module.
//!
//! @canonical .pi/architecture/modules/identity.md#application
//! Implements: Contract Freeze — AttestInput / AttestOutput DTO schemas
//! Issue: #700 (identity epic — contract freeze)
//!
//! DTOs define the input/output contracts for service operations. They carry
//! validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API and persistence)
//! - Field names and types are frozen — implementation issues depend on them
//! - The raw token may appear in `AttestInput` (presentation), but never in
//!   `AttestOutput` — outputs carry only the `token_ref` locator

use serde::{Deserialize, Serialize};

use crate::identity::application::service::VerificationOutcome;
use crate::identity::domain::IdentityClaim;

/// Input for attesting an identity from a presented token/principal.
///
/// At least one of `token` or `principal` should be present:
/// - `token` — IdP-issued JWT / OIDC access token (best-effort verified when online)
/// - `principal` — local principal (OS user, configured approver)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestInput {
    /// Raw IdP token (JWT / OIDC access token) presented for attestation.
    pub token: Option<String>,
    /// Local principal (OS user, configured approver) — no IdP token.
    pub principal: Option<String>,
    /// Issuer hint (IdP issuer URL, or "local") used for verification routing.
    pub issuer: Option<String>,
    /// Auth method from the token (e.g. "device_code", "client_credentials").
    pub auth_method: Option<String>,
}

/// Output of the attestation flow: the attested claim plus its verification marker.
///
/// The `verification` field makes degradation explicit — consumers can see
/// `VerificationOutcome::Unverified { reason }` and record it in the envelope.
/// The raw token is never part of this DTO; only the claim's `token_ref`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestOutput {
    /// The attested, time-bound identity claim.
    pub claim: IdentityClaim,
    /// Best-effort verification marker (explicit, never silent).
    pub verification: VerificationOutcome,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn sample_claim() -> IdentityClaim {
        IdentityClaim {
            subject: "user@org".to_string(),
            issuer: "https://idp.example.com".to_string(),
            authority: None,
            source: crate::identity::domain::IdentitySource::LocalPrincipal,
            auth_method: None,
            issued_at: Utc.with_ymd_and_hms(2026, 8, 28, 10, 0, 0).unwrap(),
            expires_at: None,
            token_ref: None,
        }
    }

    #[test]
    fn attest_input_serde_round_trip() {
        let input = AttestInput {
            token: Some("eyJhbGciOiJSUzI1NiJ9.payload".to_string()),
            principal: None,
            issuer: Some("https://idp.example.com".to_string()),
            auth_method: Some("device_code".to_string()),
        };
        let json = serde_json::to_string(&input).expect("serialize input");
        let restored: AttestInput = serde_json::from_str(&json).expect("deserialize input");
        assert_eq!(input, restored);
    }

    #[test]
    fn attest_input_supports_local_principal_only() {
        let input = AttestInput {
            token: None,
            principal: Some("os-user".to_string()),
            issuer: Some("local".to_string()),
            auth_method: None,
        };
        let json = serde_json::to_string(&input).expect("serialize input");
        let restored: AttestInput = serde_json::from_str(&json).expect("deserialize input");
        assert_eq!(input, restored);
    }

    #[test]
    fn attest_output_serde_round_trip() {
        let output = AttestOutput {
            claim: sample_claim(),
            verification: VerificationOutcome::Verified,
        };
        let json = serde_json::to_string(&output).expect("serialize output");
        let restored: AttestOutput = serde_json::from_str(&json).expect("deserialize output");
        assert_eq!(output, restored);
    }

    #[test]
    fn attest_output_verification_serializes_snake_case() {
        let output = AttestOutput {
            claim: sample_claim(),
            verification: VerificationOutcome::Unverified {
                reason: "idp unreachable".to_string(),
            },
        };
        let json = serde_json::to_string(&output).expect("serialize output");
        assert!(
            json.contains("\"unverified\""),
            "expected snake_case variant, got {json}"
        );
        assert!(json.contains("idp unreachable"));
    }
}

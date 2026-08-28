//! IdentityClaim — the shared, serde-stable identity value type.
//!
//! @canonical .pi/architecture/modules/identity.md#identityclaim
//! Implements: Contract Freeze — IdentityClaim value object + IdentitySource enum
//! Issue: #700 (identity epic — contract freeze)
//!
//! Every identity-bearing surface uses this type — runs (`author`), approvals
//! (`approver_id`), and the signed audit envelope (redacted `identity` block).
//! It is an **attributed, time-bound statement**: evidence of who was presented
//! as authorizing, not proof of who the person is.
//!
//! # Contract (Frozen)
//! - Serde-stable: field names and types are frozen; round-trip must preserve
//!   every field (verified by unit test in ISSUE-IDENTITY-1)
//! - `token_ref` is a *reference* to the raw token — it never contains the raw
//!   token value itself in serialized form
//! - `redacted_summary()` must never render the raw token (SpanPrivacy pattern)
//! - `is_valid()` is true only while the claim is within its lifetime
//! - Degradation is explicit: `IdentitySource::Unverified` is a first-class
//!   state, never a silent fallback

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The shared identity claim value object.
///
/// Attributed, time-bound evidence of who was presented as acting. Consumed by
/// the orchestrator (run author), the approval service (approver identity), the
/// audit envelope (redacted identity block), and the MCP auth module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityClaim {
    /// Subject — the human's unique identifier at the IdP (e.g. "user@org" or `sub`).
    pub subject: String,
    /// Issuer — who vouches for the subject (IdP issuer URL, or "local").
    pub issuer: String,
    /// Roles / authority presented (captured fact, not judgment).
    pub authority: Option<String>,
    /// How the identity was established: IdP token, local principal, or unverified.
    pub source: IdentitySource,
    /// Auth method from the token (e.g. "device_code", "client_credentials").
    pub auth_method: Option<String>,
    /// When the claim was issued.
    pub issued_at: DateTime<Utc>,
    /// When the claim expires. `None` means no expiry.
    pub expires_at: Option<DateTime<Utc>>,
    /// Reference to the raw token, preserved for off-band verification.
    /// Never contains the raw token value itself in serialized form.
    pub token_ref: Option<String>,
}

impl IdentityClaim {
    /// True when the claim is still within its lifetime.
    ///
    /// # Contract
    /// - `true` when `expires_at` is `None` (no expiry) or in the future
    /// - `false` once `expires_at` is in the past (expired claims must be
    ///   rejected at approval binding)
    ///
    /// Implemented in ISSUE-IDENTITY-1 (IdentityClaim).
    pub fn is_valid(&self) -> bool {
        match self.expires_at {
            None => true,
            Some(expires_at) => expires_at > Utc::now(),
        }
    }

    /// Redacted rendering for logs and envelope summaries.
    ///
    /// # Contract
    /// - Renders subject, issuer, source, and lifetime
    /// - Never contains the raw token, its reference locator, or the
    ///   `token_ref` field name
    /// - Follows the SpanPrivacy pattern (api_key/token/secret fields never logged)
    ///
    /// Implemented in ISSUE-IDENTITY-1 (IdentityClaim).
    pub fn redacted_summary(&self) -> String {
        format!(
            "identity[subject={}, issuer={}, source={}, authority={}, auth_method={}, expires_at={}]",
            self.subject,
            self.issuer,
            self.source,
            self.authority.as_deref().unwrap_or("none"),
            self.auth_method.as_deref().unwrap_or("none"),
            self.expires_at
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| "none".to_string()),
        )
    }
}

/// How the identity was established — drives the attestation marker.
///
/// The marker is explicit, never silent: an unreachable IdP or absent
/// credential degrades to [`IdentitySource::Unverified`], and consumers can see
/// that degradation in every serialized claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentitySource {
    /// IdP token (OIDC access token / JWT) — best-effort verified when online.
    IdpToken,
    /// Local principal (OS user, configured approver) — attributed, not IdP-anchored.
    LocalPrincipal,
    /// No identity presented or IdP unreachable — explicitly degraded.
    Unverified,
}

impl std::fmt::Display for IdentitySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Matches the frozen serde snake_case representation.
        let rendered = match self {
            IdentitySource::IdpToken => "idp_token",
            IdentitySource::LocalPrincipal => "local_principal",
            IdentitySource::Unverified => "unverified",
        };
        f.write_str(rendered)
    }
}

/// Redacted envelope representation of an identity claim.
///
/// The audit envelope carries this ref in its `identity` block (see
/// `.pi/architecture/modules/audit.md`): subject, issuer, source, authority,
/// and expiry — **redacted, never the raw token**. Raw tokens are preserved by
/// reference (`token_ref`) in the full claim, never embedded here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityRef {
    /// Subject — the human's unique identifier at the IdP.
    pub subject: String,
    /// Issuer — who vouches for the subject.
    pub issuer: String,
    /// How the identity was established (degradation marker preserved).
    pub source: IdentitySource,
    /// Roles / authority presented (captured fact, not judgment).
    pub authority: Option<String>,
    /// When the claim expires (`None` = no expiry).
    pub expires_at: Option<DateTime<Utc>>,
}

impl IdentityRef {
    /// Build a redacted ref from a claim.
    ///
    /// Deliberately copies only the summary fields — `token_ref`, `auth_method`
    /// and the raw token are never carried into the envelope.
    pub fn from_claim(claim: &IdentityClaim) -> Self {
        Self {
            subject: claim.subject.clone(),
            issuer: claim.issuer.clone(),
            source: claim.source.clone(),
            authority: claim.authority.clone(),
            expires_at: claim.expires_at,
        }
    }
}

impl From<&IdentityClaim> for IdentityRef {
    fn from(claim: &IdentityClaim) -> Self {
        Self::from_claim(claim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// Builds a frozen-shape sample claim for contract tests.
    fn sample_claim() -> IdentityClaim {
        IdentityClaim {
            subject: "user@org".to_string(),
            issuer: "https://idp.example.com".to_string(),
            authority: Some("admin".to_string()),
            source: IdentitySource::IdpToken,
            auth_method: Some("device_code".to_string()),
            issued_at: Utc.with_ymd_and_hms(2026, 8, 28, 10, 0, 0).unwrap(),
            expires_at: Some(Utc.with_ymd_and_hms(2026, 8, 28, 10, 15, 0).unwrap()),
            token_ref: Some("keychain://default/rigorix/idp-token".to_string()),
        }
    }

    #[test]
    fn claim_serde_round_trip_preserves_all_fields() {
        let claim = sample_claim();
        let json = serde_json::to_string(&claim).expect("serialize claim");
        let restored: IdentityClaim = serde_json::from_str(&json).expect("deserialize claim");
        assert_eq!(claim, restored);
    }

    #[test]
    fn claim_json_field_names_are_frozen() {
        let claim = sample_claim();
        let value: serde_json::Value = serde_json::to_value(&claim).expect("claim to value");
        let obj = value.as_object().expect("claim object");
        for field in [
            "subject",
            "issuer",
            "authority",
            "source",
            "auth_method",
            "issued_at",
            "expires_at",
            "token_ref",
        ] {
            assert!(obj.contains_key(field), "missing frozen field {field}");
        }
    }

    #[test]
    fn claim_token_ref_is_a_reference_not_the_token() {
        // Contract: token_ref may carry an opaque locator but never the token value.
        let claim = sample_claim();
        let json = serde_json::to_string(&claim).expect("serialize claim");
        assert!(
            !json.contains("eyJhbGciOi"),
            "serialized claim leaked a JWT payload"
        );
    }

    #[test]
    fn identity_source_serializes_snake_case() {
        let values = [
            (IdentitySource::IdpToken, "idp_token"),
            (IdentitySource::LocalPrincipal, "local_principal"),
            (IdentitySource::Unverified, "unverified"),
        ];
        for (source, expected) in values {
            let json = serde_json::to_string(&source).expect("serialize source");
            assert_eq!(json, format!("\"{expected}\""));
        }
    }

    #[test]
    fn identity_source_round_trip() {
        for source in [
            IdentitySource::IdpToken,
            IdentitySource::LocalPrincipal,
            IdentitySource::Unverified,
        ] {
            let json = serde_json::to_string(&source).expect("serialize source");
            let restored: IdentitySource = serde_json::from_str(&json).expect("deserialize source");
            assert_eq!(source, restored);
        }
    }

    // ── ISSUE-IDENTITY-1 behavior tests (Red → Green) ────────────────────────

    #[test]
    fn is_valid_none_expiry_is_valid() {
        let claim = IdentityClaim {
            expires_at: None,
            ..sample_claim()
        };
        assert!(claim.is_valid(), "no expiry => always valid");
    }

    #[test]
    fn is_valid_future_expiry_is_valid() {
        let claim = IdentityClaim {
            expires_at: Some(Utc::now() + chrono::Duration::seconds(60)),
            ..sample_claim()
        };
        assert!(claim.is_valid(), "future expiry => valid");
    }

    #[test]
    fn is_valid_past_expiry_is_invalid() {
        let claim = IdentityClaim {
            expires_at: Some(Utc::now() - chrono::Duration::seconds(60)),
            ..sample_claim()
        };
        assert!(!claim.is_valid(), "past expiry => invalid (expired)");
    }

    #[test]
    fn redacted_summary_contains_subject_but_never_raw_token() {
        let raw_token = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJ1c2VyQG9yZyJ9.signature";
        let claim = IdentityClaim {
            token_ref: Some("keychain://default/rigorix/idp-token".to_string()),
            ..sample_claim()
        };
        let summary = claim.redacted_summary();
        assert!(summary.contains("user@org"), "subject rendered");
        assert!(
            !summary.contains("keychain"),
            "token_ref locator must never appear: {summary}"
        );
        assert!(
            !summary.contains("token_ref"),
            "token_ref field name must never appear: {summary}"
        );
        assert!(
            !summary.contains(&raw_token[0..20]),
            "raw token payload must never appear: {summary}"
        );
    }

    #[test]
    fn identity_ref_from_claim_redacts_token_ref() {
        let claim = sample_claim(); // token_ref = Some("keychain://.../idp-token")
        let reference = IdentityRef::from(&claim);
        assert_eq!(reference.subject, "user@org");
        assert_eq!(reference.issuer, "https://idp.example.com");
        assert_eq!(reference.source, IdentitySource::IdpToken);
        assert_eq!(reference.authority, Some("admin".to_string()));
        assert_eq!(
            reference.expires_at,
            Some(Utc.with_ymd_and_hms(2026, 8, 28, 10, 15, 0).unwrap())
        );
        // Redacted: no token_ref field, no token locator in the serialized ref.
        let json = serde_json::to_string(&reference).expect("serialize identity ref");
        assert!(
            !json.contains("token_ref"),
            "IdentityRef must not carry token_ref: {json}"
        );
        assert!(!json.contains("keychain"));
    }

    #[test]
    fn identity_ref_serde_round_trip() {
        let reference = IdentityRef::from(&sample_claim());
        let json = serde_json::to_string(&reference).expect("serialize identity ref");
        let restored: IdentityRef = serde_json::from_str(&json).expect("deserialize identity ref");
        assert_eq!(reference, restored);
    }
}

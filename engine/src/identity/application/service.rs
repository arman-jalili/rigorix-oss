//! IdentityAttestationService — converts presented credentials into attested claims.
//!
//! @canonical .pi/architecture/modules/identity.md#identityattestationservice
//! Implements: Contract Freeze — IdentityAttestationService trait
//! Issue: #700 (identity epic — contract freeze)
//!
//! The application service that converts a presented IdP token (or local
//! principal) into a structured, time-bound `IdentityClaim`, with best-effort
//! signature verification against the IdP JWKS.
//!
//! # Contract (Frozen)
//! - `attest`: full attestation entry point (extract → mark → best-effort verify)
//! - `extract_claims`: decodes standard JWT claims (sub, iss, exp, roles)
//!   WITHOUT verification — used for attestation; verification is separate
//! - `verify`: best-effort signature verification; an unreachable IdP yields
//!   `VerificationOutcome::Unverified`, never an error (offline-first, ADR-012)
//! - All methods are trait-object safe (`Send + Sync`)
//! - No authorization judgment — OSS attests, Enterprise authorizes

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::identity::domain::{IdentityClaim, IdentityError};

use super::dto::AttestInput;

/// Result of best-effort signature verification.
///
/// The outcome marker is explicit, never silent: `Unverified` carries the
/// reason so consumers can distinguish "IdP unreachable" from "unknown kid"
/// and record it in the envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationOutcome {
    /// Signature validated against the IdP JWKS.
    Verified,
    /// Not verified — IdP unreachable, unknown kid, or verification disabled.
    Unverified {
        /// Human-readable reason for the degraded outcome.
        reason: String,
    },
}

/// Application service for identity attestation.
///
/// The default deployment is offline-first: `verify` degrades to
/// `Unverified` when the IdP is unreachable (see `NullVerifier` in
/// `infrastructure/verifier.rs` and ADR-012 option c).
#[async_trait]
pub trait IdentityAttestationService: Send + Sync {
    /// Attest from a presented token/principal → `IdentityClaim`.
    ///
    /// # Errors
    /// - `IdentityError::InvalidToken` — token is not a parseable JWT
    /// - `IdentityError::MissingClaim` — required claim absent
    /// - `IdentityError::Expired` — token claims past expiry
    ///
    /// An unreachable IdP is **not** an error — the claim degrades to
    /// `IdentitySource::Unverified` and the outcome is recorded.
    async fn attest(&self, input: AttestInput) -> Result<IdentityClaim, IdentityError>;

    /// Extract claims from a JWT WITHOUT verification (decode claims).
    ///
    /// Decodes standard JWT claims (`sub`, `iss`, `exp`, roles) into an
    /// [`IdentityClaim`]. Verification is separate and best-effort.
    fn extract_claims(&self, token: &str) -> Result<IdentityClaim, IdentityError>;

    /// Best-effort signature verification against the IdP JWKS.
    ///
    /// # Contract
    /// - IdP reachable + valid signature → `VerificationOutcome::Verified`
    /// - IdP reachable + tampered signature → `VerificationOutcome::Unverified`
    /// - IdP unreachable → `VerificationOutcome::Unverified`, **never an error**
    async fn verify(
        &self,
        claim: &IdentityClaim,
        token: &str,
    ) -> Result<VerificationOutcome, IdentityError>;
}

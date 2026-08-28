//! TokenVerifier — best-effort signature verification against the IdP JWKS.
//!
//! @canonical .pi/architecture/modules/identity.md#tokenverifier
//! Implements: Contract Freeze — TokenVerifier trait + NullVerifier (offline default)
//! Issue: #700 (identity epic — contract freeze)
//!
//! Best-effort signature verification keeps attestation fully offline-capable
//! (ADR-012 option c): an unreachable IdP yields
//! `VerificationOutcome::Unverified`, never an error. The `NullVerifier`
//! struct is the offline default — attestation proceeds without network
//! verification.
//!
//! # Contract (Frozen)
//! - `verify` never errors on an unreachable IdP — it records `Unverified`
//! - `NullVerifier` is the offline default and is constructible via
//!   `NullVerifier::new()`
//! - Concrete JWKS-backed implementations land in ISSUE-IDENTITY-4

use async_trait::async_trait;

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
    /// # TODO
    /// Offline default: returns `VerificationOutcome::Unverified` with an
    /// explicit reason (no IdP reachable). Implemented in ISSUE-IDENTITY-4
    /// (TokenVerifier). Contract stub — behavior lands with the implementation.
    async fn verify(
        &self,
        _token: &str,
        _claim: &IdentityClaim,
    ) -> Result<VerificationOutcome, IdentityError> {
        todo!("ISSUE-IDENTITY-4 (TokenVerifier): implement NullVerifier offline default")
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
}

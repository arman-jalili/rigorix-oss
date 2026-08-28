//! IdentityError — typed error enum for all identity attestation failure modes.
//!
//! @canonical .pi/architecture/modules/identity.md#error-handling
//! Implements: Contract Freeze — IdentityError enum
//! Issue: #700 (identity epic — contract freeze)
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `IdentityError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level
//!
//! # Recovery
//! - `InvalidToken` / `MissingClaim`: not retriable — reject the presented
//!   identity, degrade to `IdentitySource::Unverified`
//! - `Expired`: not retriable — re-authenticate
//! - `VerificationUnavailable`: **non-fatal for attestation** — degrade to
//!   `IdentitySource::Unverified`, record the outcome

use thiserror::Error;

/// Errors that can occur during identity attestation and verification.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdentityError {
    /// The presented token was not a parseable JWT.
    #[error("Invalid token format: {0}")]
    InvalidToken(String),

    /// The presented token's claims are past their expiry.
    #[error("Token expired")]
    Expired,

    /// A required claim (sub, iss, exp) was absent from the token.
    #[error("Missing required claim: {0}")]
    MissingClaim(String),

    /// Signature verification could not run (IdP unreachable, unknown kid, …).
    ///
    /// Non-fatal for attestation — the claim degrades to
    /// `IdentitySource::Unverified` and the outcome is recorded.
    #[error("Verification unavailable: {0}")]
    VerificationUnavailable(String),

    /// Unexpected internal failure.
    #[error("Internal error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages_are_frozen() {
        assert_eq!(
            IdentityError::InvalidToken("not a jwt".to_string()).to_string(),
            "Invalid token format: not a jwt"
        );
        assert_eq!(IdentityError::Expired.to_string(), "Token expired");
        assert_eq!(
            IdentityError::MissingClaim("sub".to_string()).to_string(),
            "Missing required claim: sub"
        );
        assert_eq!(
            IdentityError::VerificationUnavailable("idp unreachable".to_string()).to_string(),
            "Verification unavailable: idp unreachable"
        );
        assert_eq!(
            IdentityError::Internal("boom".to_string()).to_string(),
            "Internal error: boom"
        );
    }

    #[test]
    fn error_implements_std_error() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<IdentityError>();
    }
}

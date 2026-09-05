//! Error types for the Auth bounded context.
//!
//! @canonical .pi/architecture/modules/auth.md#errors
//! Implements: Contract Freeze — AuthError, SseAuthError
//!
//! Structured error types for auth operations. Each variant carries
//! sufficient context for programmatic handling (retry, user-facing
//! messages, event emission).
//!
//! # Contract (Frozen)
//!
//! - All public variants and their fields are frozen
//! - New variants require ADR approval and interface review
//! - Every error has a user-readable Display message
//! - Errors never embed raw token material (SpanPrivacy)

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// AuthError — root error type for the auth module
// ---------------------------------------------------------------------------

/// Root error type for all AuthService, IdpClient, KeychainStore, and
/// TokenProvider operations.
///
/// Covers configuration problems, OIDC discovery/transport failures, device
/// flow outcomes surfaced as errors, custody (keychain) failures, and token
/// validation problems.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum AuthError {
    /// Configuration error (missing/invalid IdP settings).
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// OIDC discovery failed (`.well-known/openid-configuration`).
    #[error("OIDC discovery failed for {issuer}: {reason}")]
    Discovery {
        /// IdP issuer URL.
        issuer: String,
        /// Underlying reason.
        reason: String,
    },

    /// Network transport failure (IdP unreachable, TLS, timeout).
    #[error("Transport error: {0}")]
    Transport(String),

    /// IdP rejected the device authorization request (bad client, denied).
    #[error("Device authorization rejected: {0}")]
    DeviceAuthorizationRejected(String),

    /// User (or IdP policy) denied the device flow — `access_denied`.
    #[error("Device flow denied: {0}")]
    AccessDenied(String),

    /// Device code expired before authorization — `expired_token`.
    #[error("Device flow expired")]
    Expired,

    /// Token endpoint returned a malformed/undeserializable response.
    #[error("Invalid token response: {0}")]
    InvalidTokenResponse(String),

    /// Refresh-token exchange failed.
    #[error("Token refresh failed: {0}")]
    RefreshFailed(String),

    /// Keychain custody failure (store/read/delete).
    #[error("Keychain error: {0}")]
    Keychain(String),

    /// No active authenticated session for the requested operation.
    #[error("Not authenticated")]
    NotAuthenticated,

    /// Attestation delegation to the engine identity service failed.
    #[error("Attestation failed: {0}")]
    Attestation(String),

    /// Internal synchronization or state error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl AuthError {
    /// True when the operation is safe to retry after a short backoff.
    ///
    /// # Contract (Frozen)
    /// - `Transport` (IdP unreachable, transient) → retriable
    /// - `Discovery` (endpoint temporarily unavailable) → retriable
    /// - Everything else (denials, expiry, config, custody) → not retriable —
    ///   the human or config must intervene
    pub fn is_retriable(&self) -> bool {
        matches!(self, AuthError::Transport(_) | AuthError::Discovery { .. })
    }
}

// ---------------------------------------------------------------------------
// SseAuthError — errors from the optional SSE transport gate
// ---------------------------------------------------------------------------

/// Errors produced by the SSE auth gate (non-localhost binds only).
///
/// The gate (ADR-008) is an access-control point for a network-exposed
/// gateway. A rejected credential is a *decision* (`SseAuthDecision::Deny`),
/// not an error — errors here mean the gate cannot evaluate at all
/// (misconfiguration, IdP outage) and the transport must refuse to start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum SseAuthError {
    /// Gate mode is set but required infrastructure is not configured.
    #[error("SSE auth not configured: {0}")]
    NotConfigured(String),

    /// IdP could not be reached to validate the bearer token.
    #[error("SSE auth IdP unreachable: {0}")]
    IdpUnreachable(String),

    /// Internal error while evaluating the gate.
    #[error("SSE auth internal error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Device flow outcome helpers (RFC 8628 §3.5 token endpoint errors)
// ---------------------------------------------------------------------------

/// The `error` code returned by the token endpoint during device polling.
///
/// RFC 8628 §3.5 defines these as non-fatal in-progress markers — the poll
/// loop translates them into `DeviceFlowStatus` transitions.
///
/// # Contract (Frozen)
///
/// - `AuthorizationPending` → keep polling at the configured interval
/// - `SlowDown` → increase the polling interval by 5 seconds
/// - `AccessDenied` → `DeviceFlowStatus::Denied`
/// - `ExpiredToken` → `DeviceFlowStatus::Expired`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceFlowPollError {
    /// The user has not yet authorized — continue polling.
    AuthorizationPending,
    /// Poll too frequently — increase the interval by 5 seconds.
    SlowDown,
    /// The user denied the request.
    AccessDenied,
    /// The device code expired.
    ExpiredToken,
}

impl DeviceFlowPollError {
    /// Parse an RFC 8628 token-endpoint `error` value.
    pub fn from_rfc_code(code: &str) -> Option<Self> {
        match code {
            "authorization_pending" => Some(Self::AuthorizationPending),
            "slow_down" => Some(Self::SlowDown),
            "access_denied" => Some(Self::AccessDenied),
            "expired_token" => Some(Self::ExpiredToken),
            _ => None,
        }
    }
}

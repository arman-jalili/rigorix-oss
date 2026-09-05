//! IdpClient — OIDC device-flow client port.
//!
//! @canonical .pi/architecture/modules/auth.md#idpclient-infrastructure
//! Implements: Contract Freeze — IdpClient trait, IdpMetadata,
//! DeviceAuthorization, TokenResponse contracts
//! ADR-008: OIDC device flow (RFC 8628) over HTTPS
//!
//! Port interface for any OIDC provider the dev or org configures (Keycloak,
//! Entra ID, Okta, …). The concrete implementation (HTTP via reqwest,
//! discovery caching) lands in its implementation issue.
//!
//! # Contract (Frozen)
//!
//! - HTTPS enforced; TLS verification mandatory (matches enterprise proxy
//!   policy) — no HTTP issuer is ever accepted
//! - `discover()` reads `.well-known/openid-configuration` for the
//!   device-authorization, token, and revocation endpoints
//! - `device_authorization()` performs RFC 8628 §3.1
//! - `poll_token()` performs RFC 8628 §3.5; RFC 8628 in-progress codes
//!   (`authorization_pending`, `slow_down`, `access_denied`, `expired_token`)
//!   are surfaced via `crate::auth::domain::DeviceFlowPollError`, genuine
//!   faults via `AuthError`
//! - Secrets never leave the client except as `Secret<T>`

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::domain::{AuthError, Secret};

/// Default polling interval in seconds when the IdP omits `interval`
/// (RFC 8628 §3.2 default is 5).
pub const DEFAULT_POLL_INTERVAL_SECS: u64 = 5;

/// OIDC provider metadata discovered from
/// `.well-known/openid-configuration` (RFC 8414).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdpMetadata {
    /// Issuer URL (must match the configured issuer).
    pub issuer: String,

    /// Device-authorization endpoint (RFC 8628 §3.1).
    pub device_authorization_endpoint: String,

    /// Token endpoint.
    pub token_endpoint: String,

    /// Revocation endpoint (RFC 7009) — optional.
    pub revocation_endpoint: Option<String>,

    /// JWKS URI for best-effort signature verification.
    pub jwks_uri: Option<String>,
}

/// Response to a device-authorization request (RFC 8628 §3.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceAuthorization {
    /// Device code — exchanged for tokens by `poll_token` (redacted).
    pub device_code: Secret<String>,

    /// Human-readable code entered at the verification URI.
    pub user_code: String,

    /// URL the human opens to authorize.
    pub verification_uri: String,

    /// Seconds until the device code expires.
    pub expires_in: u64,

    /// Minimum polling interval in seconds (RFC 8628 §3.3).
    pub interval_secs: u64,

    /// When the device code was issued.
    pub issued_at: DateTime<Utc>,
}

/// Successful token-endpoint response (RFC 6749 §5.1 / RFC 8628 §3.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenResponse {
    /// Short-TTL access token (redacted; cached in-memory by TokenProvider).
    pub access_token: Secret<String>,

    /// Long-lived refresh token — present on first exchange (redacted;
    /// persisted to the keychain by the service).
    pub refresh_token: Option<Secret<String>>,

    /// Access-token lifetime in seconds.
    pub expires_in: u64,

    /// Token type (RFC 6749 §5.1 — typically "Bearer").
    pub token_type: String,

    /// Scope granted by the IdP (optional).
    pub scope: Option<String>,
}

/// Outcome of a single token-endpoint poll (RFC 8628 §3.3–3.5).
///
/// RFC 8628 in-progress responses are HTTP errors with a typed `error`
/// body — they are expected poll states, not faults, so they surface as
/// variants here rather than `AuthError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenPoll {
    /// Token exchange succeeded — tokens in the response.
    Succeeded(TokenResponse),

    /// User has not yet authorized — poll again after `retry_after_secs`
    /// (covers `authorization_pending` and RFC `slow_down` backoff).
    Pending {
        /// Seconds to wait before the next poll.
        retry_after_secs: Option<u64>,
    },

    /// User or IdP policy denied the flow (`access_denied`).
    AccessDenied {
        /// Human-readable reason when the IdP provided one.
        reason: String,
    },

    /// Device code expired before authorization (`expired_token`).
    Expired,
}

/// OIDC device-flow client port (RFC 8628).
///
/// Implementations are HTTP clients (reqwest-backed); all endpoints are
/// HTTPS-enforced.
#[async_trait::async_trait]
pub trait IdpClient: Send + Sync {
    /// Fetch and cache OIDC provider metadata from
    /// `{issuer}/.well-known/openid-configuration`.
    ///
    /// # Errors
    /// - `AuthError::Discovery` — endpoint unreachable or malformed
    async fn discover(&self) -> Result<IdpMetadata, AuthError>;

    /// Start a device authorization grant (RFC 8628 §3.1).
    ///
    /// # Errors
    /// - `AuthError::DeviceAuthorizationRejected` — IdP refused the request
    /// - `AuthError::Transport` — IdP unreachable (retriable)
    async fn device_authorization(&self, client_id: &str)
    -> Result<DeviceAuthorization, AuthError>;

    /// Poll the token endpoint with the device code (RFC 8628 §3.5).
    ///
    /// RFC 8628 in-progress outcomes are returned as [`TokenPoll`] variants
    /// (pending/denied/expired) — only genuine faults error here.
    ///
    /// # Errors
    /// - `AuthError::Transport` — IdP unreachable (retriable)
    /// - `AuthError::InvalidTokenResponse` — undecodable token response
    async fn poll_token(
        &self,
        device_code: &Secret<String>,
        client_id: &str,
    ) -> Result<TokenPoll, AuthError>;

    /// Exchange a refresh token for a new access token (RFC 6749 §6).
    ///
    /// # Errors
    /// - `AuthError::RefreshFailed` — IdP rejected the refresh token
    async fn refresh_token(
        &self,
        refresh_token: &Secret<String>,
        client_id: &str,
    ) -> Result<TokenResponse, AuthError>;

    /// Revoke a refresh token at the IdP (RFC 7009) — best-effort on logout.
    ///
    /// # Errors
    /// - `AuthError::Transport` — IdP unreachable
    async fn revoke_token(
        &self,
        refresh_token: &Secret<String>,
        client_id: &str,
    ) -> Result<(), AuthError>;
}

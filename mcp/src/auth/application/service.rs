//! Service interfaces (use cases) for the Auth bounded context.
//!
//! @canonical .pi/architecture/modules/auth.md#services
//! Implements: Contract Freeze — AuthService use-case trait
//! ADR-008: device flow lifecycle; ADR-012: attestation seam
//!
//! These traits define the application-level operations for the identity
//! lifecycle: login (device flow), poll (RFC 8628 completion), refresh
//! (silent), status, logout, and attest (token → `IdentityClaim`).
//!
//! # Contract (Frozen)
//!
//! - Every use case has a corresponding trait method
//! - Input types are DTOs; output types are DTOs (redacted)
//! - All methods are async (use `async-trait` for trait object safety)
//! - All methods return domain error types (`AuthError`)
//! - No implementation — only contract signatures
//! - Services are thread-safe (Send + Sync)

use async_trait::async_trait;
use rigorix_engine::identity::IdentityClaim;

use crate::auth::domain::error::AuthError;

use super::dto::{
    LoginInput, LoginOutput, LogoutInput, LogoutOutput, PollInput, PollOutput, RefreshInput,
    RefreshOutput, StatusInput, StatusOutput,
};

/// Application service orchestrating the identity lifecycle.
///
/// Composes the OIDC device flow ([`IdpClient`]), keychain custody
/// ([`KeychainStore`]), in-memory short-TTL access tokens ([`TokenProvider`]),
/// and engine attestation (ADR-012).
///
/// # Contract (Frozen)
///
/// - `login` initiates the device flow and returns user-facing verification
///   info — it never blocks awaiting the human
/// - `poll` advances an active flow; on `authorized` it persists the refresh
///   token to the keychain and caches the access token in memory
/// - `refresh` is silent and background-safe (returns `NotAuthenticated` when
///   no refresh token exists)
/// - `logout` clears keychain + memory and revokes at the IdP when possible
/// - `attest` degrades explicitly via the engine claim (never silently fails
///   closed for local dev — ADR-008)
/// - No authorization judgment — OSS attests, Enterprise authorizes
#[async_trait]
pub trait AuthService: Send + Sync {
    /// Initiate the OIDC device authorization grant (RFC 8628 §3.1–3.2).
    ///
    /// 1. Discovers IdP endpoints (`.well-known/openid-configuration`)
    /// 2. Requests a device code (client_id + optional overrides)
    /// 3. Returns `verification_uri` + `user_code` + `expires_in` for display
    ///
    /// Emits `AuthEvent::AuthLoginStarted`.
    ///
    /// # Errors
    /// - `AuthError::Configuration` — IdP not configured
    /// - `AuthError::Discovery` — IdP discovery failed
    /// - `AuthError::DeviceAuthorizationRejected` — IdP refused the request
    async fn login(&self, input: &LoginInput) -> Result<LoginOutput, AuthError>;

    /// Advance an active device flow by polling the token endpoint
    /// (RFC 8628 §3.3–3.5).
    ///
    /// On `DeviceFlowStatus::Authorized`: the refresh token is stored in the
    /// keychain and the short-TTL access token cached in the TokenProvider.
    /// Emits `AuthLoginSucceeded` or `AuthLoginFailed`.
    ///
    /// # Errors
    /// - `AuthError::NotAuthenticated` — no active flow to poll
    /// - `AuthError::Transport` — IdP unreachable (retriable)
    async fn poll(&self, input: &PollInput) -> Result<PollOutput, AuthError>;

    /// Report identity status (`TokenStatus` + redacted claim summary).
    ///
    /// Emits `AuthEvent::AuthStatusChecked`.
    async fn status(&self, input: &StatusInput) -> Result<StatusOutput, AuthError>;

    /// Silently exchange the keychain refresh token for a new access token.
    ///
    /// Background-safe (log/tracing only, no user interaction).
    ///
    /// # Errors
    /// - `AuthError::NotAuthenticated` — no refresh token in the keychain
    /// - `AuthError::RefreshFailed` — IdP rejected the refresh
    async fn refresh(&self, input: &RefreshInput) -> Result<RefreshOutput, AuthError>;

    /// Clear all identity material: revoke + delete the keychain refresh
    /// token and clear the in-memory access token.
    ///
    /// Emits `AuthEvent::AuthLoggedOut`.
    async fn logout(&self, input: &LogoutInput) -> Result<LogoutOutput, AuthError>;

    /// Attest the current in-memory access token → `IdentityClaim`.
    ///
    /// Delegates to the engine `IdentityAttestationService` (ADR-012). When
    /// the access token is expired, a silent refresh is attempted first.
    ///
    /// # Errors
    /// - `AuthError::NotAuthenticated` — no access token to attest
    /// - `AuthError::Attestation` — engine could not produce a claim
    async fn attest(&self) -> Result<IdentityClaim, AuthError>;
}

//! TokenProvider — short-TTL access-token custody port.
//!
//! @canonical .pi/architecture/modules/auth.md#tokenprovider-infrastructure
//! Implements: Contract Freeze — TokenProvider trait
//! ADR-008: access tokens are short-TTL (5–15 min) and in-memory only
//!
//! Port interface for the in-memory access token — the only identity material
//! the agent-visible surface can present. The concrete implementation (a
//! thread-safe in-memory cell with expiry tracking) lands in its
//! implementation issue.
//!
//! # Contract (Frozen)
//!
//! - Never persists access tokens to disk
//! - TTL is bounded (5–15 min per ADR-008) — enforced at the expiry boundary
//! - Tokens cross the boundary only as `Secret<String>`
//! - The provider never refreshes on its own — the AuthService drives
//!   refresh via the keychain refresh token

use chrono::{DateTime, Utc};

use crate::auth::domain::{AuthError, Secret};

/// Provider for the current short-TTL access token.
#[async_trait::async_trait]
pub trait TokenProvider: Send + Sync {
    /// The currently cached access token, if any.
    ///
    /// A token past its recorded expiry must NOT be returned as usable —
    /// implementations return `None` once expired (the service silently
    /// refreshes instead).
    async fn current_access_token(&self) -> Option<Secret<String>>;

    /// Cache a fresh access token with its absolute expiry.
    ///
    /// # Errors
    /// - `AuthError::Internal` — interior state poisoned/locked
    async fn set_access_token(
        &self,
        token: Secret<String>,
        expires_at: DateTime<Utc>,
    ) -> Result<(), AuthError>;

    /// Absolute expiry of the cached token (`None` when empty).
    async fn access_token_expires_at(&self) -> Option<DateTime<Utc>>;

    /// Clear the cached token (logout / revocation).
    async fn clear(&self);
}

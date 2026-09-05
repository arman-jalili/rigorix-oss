//! KeychainStore — long-lived credential custody port.
//!
//! @canonical .pi/architecture/modules/auth.md#keychainstore-infrastructure
//! Implements: Contract Freeze — KeychainStore trait
//! ADR-008: the refresh token lives HERE, never in readable files
//!
//! Port interface for refresh-token custody. The concrete implementation
//! (OS keychain via `keyring` — macOS Keychain, Windows Credential Manager,
//! Linux Secret Service — with an explicit opt-in plaintext-file fallback for
//! CI environments without a keychain) lands in its implementation issue.
//!
//! # Contract (Frozen)
//!
//! - The refresh token is the crown jewel: never written to `.rigorix/` or
//!   any agent-readable file by the default path
//! - Plaintext fallback is always explicit opt-in (documented degraded mode)
//! - Tokens cross the boundary only as `Secret<String>`
//! - `account` disambiguates credentials (e.g. per-issuer)

use crate::auth::domain::{AuthError, Secret};

/// Default keychain service name under which Rigorix credentials are stored.
pub const RIGORIX_KEYCHAIN_SERVICE: &str = "rigorix";

/// Default account name for the refresh token.
pub const REFRESH_TOKEN_ACCOUNT: &str = "refresh_token";

/// Long-lived credential store (refresh-token custody).
///
/// `service` defaults to [`RIGORIX_KEYCHAIN_SERVICE`]; `account` defaults to
/// [`REFRESH_TOKEN_ACCOUNT`] and can vary per issuer.
#[async_trait::async_trait]
pub trait KeychainStore: Send + Sync {
    /// Persist the refresh token in the OS keychain.
    ///
    /// # Errors
    /// - `AuthError::Keychain` — keychain unavailable or write failed
    async fn store_refresh_token(
        &self,
        service: &str,
        account: &str,
        token: &Secret<String>,
    ) -> Result<(), AuthError>;

    /// Read the refresh token from the OS keychain.
    ///
    /// Returns `Ok(None)` when no credential exists for the account.
    ///
    /// # Errors
    /// - `AuthError::Keychain` — keychain unavailable
    async fn get_refresh_token(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<Secret<String>>, AuthError>;

    /// Delete the refresh token from the OS keychain.
    ///
    /// Deleting a missing credential is not an error (idempotent).
    ///
    /// # Errors
    /// - `AuthError::Keychain` — keychain unavailable
    async fn delete_refresh_token(&self, service: &str, account: &str) -> Result<(), AuthError>;

    /// True when this store is the explicit plaintext-file fallback
    /// (degraded mode — only ever opt-in for CI).
    ///
    /// Consumers log a prominent warning when true.
    fn uses_plaintext_fallback(&self) -> bool {
        false
    }
}

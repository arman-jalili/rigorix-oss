//! Factory interfaces for the Auth bounded context.
//!
//! @canonical .pi/architecture/modules/auth.md#factories
//! Implements: Contract Freeze — AuthServiceFactory interface
//!
//! Factory interfaces encapsulate the construction of the composed
//! [`AuthService`]: wiring the OIDC device-flow client, keychain custody,
//! in-memory token provider, and the engine attestation service together.
//!
//! # Contract (Frozen)
//!
//! - Factory methods validate inputs before constructing
//! - All factory methods return Result for fallible construction
//! - Construction is async (I/O-bound: keychain probe, IdP reachability)

use async_trait::async_trait;
use rigorix_engine::identity::IdentityAttestationService;
use std::sync::Arc;

use crate::auth::domain::{AuthError, IdpConfig};
use crate::auth::infrastructure::{IdpClient, KeychainStore, TokenProvider};

use super::service::AuthService;

/// Factory for composing an [`AuthService`] from its ports.
///
/// All four ports are injected (interface-first) so the factory stays
/// implementation-agnostic — the concrete IdP client, keychain, token
/// provider, and attestation service are supplied by the composition root.
#[async_trait]
pub trait AuthServiceFactory: Send + Sync {
    /// Compose an [`AuthService`].
    ///
    /// # Errors
    /// - `AuthError::Configuration` — IdP config invalid or ports inconsistent
    /// - `AuthError::Keychain` — keychain not reachable for custody
    async fn create(
        &self,
        config: IdpConfig,
        idp_client: Arc<dyn IdpClient>,
        keychain: Arc<dyn KeychainStore>,
        tokens: Arc<dyn TokenProvider>,
        attestation: Arc<dyn IdentityAttestationService>,
    ) -> Result<Arc<dyn AuthService>, AuthError>;
}

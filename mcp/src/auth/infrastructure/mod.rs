//! Infrastructure layer for the Auth bounded context.
//!
//! @canonical .pi/architecture/modules/auth.md#infrastructure
//! Implements: Contract Freeze — IdpClient, KeychainStore, TokenProvider
//! interface traits
//!
//! This module defines the port interfaces for the auth module's external
//! dependencies. Implementations land in their own issues:
//!
//! - `IdpClient` — OIDC discovery + device flow over HTTP (RFC 8628)
//! - `KeychainStore` — long-lived credential custody (OS keychain / `keyring`,
//!   explicit plaintext fallback for CI)
//! - `TokenProvider` — short-TTL access-token custody (in-memory only)
//!
//! # Contract (Frozen)
//!
//! - All interfaces are traits (no concrete impls in this freeze)
//! - All I/O-bound methods are async
//! - All methods return `AuthError` (domain error type)
//! - Thread-safe (Send + Sync)
//! - Secrets cross boundaries only as `Secret<T>` wrappers

pub mod idp_client;
pub mod keychain_store;
pub mod token_provider;

pub use idp_client::{DeviceAuthorization, IdpClient, IdpMetadata, TokenPoll, TokenResponse};
pub use keychain_store::KeychainStore;
pub use token_provider::TokenProvider;

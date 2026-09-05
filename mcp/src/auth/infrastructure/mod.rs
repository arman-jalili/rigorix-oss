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
pub mod idp_client_impl;
pub mod keychain_store;
pub mod keychain_store_impl;
pub mod token_provider;
pub mod token_provider_impl;

pub use idp_client::{DeviceAuthorization, IdpClient, IdpMetadata, TokenPoll, TokenResponse};
pub use idp_client_impl::HttpIdpClient;
pub use keychain_store::KeychainStore;
pub use keychain_store_impl::KeychainStoreImpl;
pub use token_provider::TokenProvider;
pub use token_provider_impl::InMemoryTokenProvider;

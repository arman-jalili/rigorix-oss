//! Repository interface for the Enterprise Proxy bounded context.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#repository
//! Implements: Contract Freeze — repository interfaces
//!
//! Repository interfaces abstract all persistence concerns for the
//! enterprise proxy. Since the proxy primarily communicates via HTTP
//! without local persistence, this module defines minimal interfaces
//! for any state that needs storage (e.g., cached schema persistence
//! for recovery after restart).
//!
//! # Contract (Frozen)
//!
//! - Repository types are pure interfaces with no behavior
//! - All methods are async (async-trait)
//! - All methods return domain error types
//! - Repository implementations are in infrastructure/
//! - Thread-safe (Send + Sync)

use async_trait::async_trait;

use crate::enterprise_proxy::domain::error::ProxyError;
use crate::enterprise_proxy::domain::value::EnterpriseMetadata;

// ---------------------------------------------------------------------------
// SchemaCacheRepository — persistence for schema cache state
// ---------------------------------------------------------------------------

/// Repository for persisting schema cache state across restarts.
///
/// Enables the enterprise proxy to recover cached tool schemas
/// without re-fetching from the API on every restart.
///
/// # Contract (Frozen)
///
/// - `save` serializes and persists the metadata
/// - `load` deserializes and returns the last saved metadata (if any)
/// - `clear` removes all persisted cache data
/// - Default implementation is in-memory (no persistence)
#[async_trait]
pub trait SchemaCacheRepository: Send + Sync {
    /// Save enterprise metadata to persistent storage.
    ///
    /// # Errors
    /// - `ProxyError::Internal` if serialization or write fails
    async fn save(&self, metadata: &EnterpriseMetadata) -> Result<(), ProxyError>;

    /// Load enterprise metadata from persistent storage.
    ///
    /// Returns `None` if no cached data exists.
    ///
    /// # Errors
    /// - `ProxyError::Internal` if read or deserialization fails
    async fn load(&self) -> Result<Option<EnterpriseMetadata>, ProxyError>;

    /// Clear all persisted cache data.
    ///
    /// # Errors
    /// - `ProxyError::Internal` if delete fails
    async fn clear(&self) -> Result<(), ProxyError>;
}

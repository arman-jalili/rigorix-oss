//! Aggregate root and domain service traits for Enterprise Proxy.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#entities
//! Implements: Contract Freeze — EnterpriseProxy trait, SchemaCache struct
//!
//! # EnterpriseProxy (Aggregate Root)
//!
//! Proxies `rigorix_enterprise_*` tool calls to the Rigorix Enterprise API
//! via HTTP JSON-RPC. Dynamically discovers available enterprise tools
//! during initialization.
//!
//! # SchemaCache (Domain Service)
//!
//! Caches enterprise tool schemas for capability negotiation during
//! MCP initialization. Supports TTL-based staleness checks.
//!
//! # Contract (Frozen)
//!
//! - All methods are async (use async-trait)
//! - All methods return Result with ProxyError
//! - No implementation logic — pure interface
//! - Thread-safe (Send + Sync)
//! - EnterpriseProxy is conditionally available (feature-gated)

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::error::ProxyError;
use super::value::{EnterpriseMetadata, ToolSchema};

// ---------------------------------------------------------------------------
// EnterpriseProxy — Aggregate Root
// ---------------------------------------------------------------------------

/// Aggregate root that forwards `rigorix_enterprise_*` MCP tool calls to
/// the Rigorix Enterprise API via HTTP JSON-RPC.
///
/// Dynamically discovers available enterprise tools during initialization
/// and caches their schemas for capability negotiation.
///
/// # Invariants (Frozen)
///
/// - Zero enterprise code loaded when `enterprise.api_url` is not configured
/// - Enterprise API key is stored as `Secret` type — never logged, always redacted
/// - Failures never cascade to OSS tools — clear diagnostic errors returned
/// - Tool schemas are cached for server lifetime (with optional TTL-based refresh)
/// - Proxy is forward-compatible — new enterprise tools require no OSS changes
#[async_trait]
pub trait EnterpriseProxy: Send + Sync {
    /// Check if the enterprise proxy is enabled (configured and active).
    fn is_enabled(&self) -> bool;

    /// Get the list of currently available enterprise tools.
    ///
    /// Returns schemas for all tools discovered during initialization.
    /// Returns empty vec if not initialized or no tools available.
    fn available_tools(&self) -> Vec<ToolSchema>;

    /// Get enterprise metadata if available.
    fn metadata(&self) -> Option<EnterpriseMetadata>;

    /// Handle an enterprise tool call by proxying it to the enterprise API.
    ///
    /// Forwards the method and parameters as a JSON-RPC request to the
    /// enterprise API and returns the response.
    ///
    /// # Errors
    /// - `ProxyError::NotEnabled` if the proxy is not enabled
    /// - `ProxyError::Transport` if the enterprise API is unreachable
    /// - `ProxyError::Timeout` if the request times out
    /// - `ProxyError::ApiError` if the API returns an error status
    /// - `ProxyError::Authentication` if the API key is invalid/expired
    async fn handle(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, ProxyError>;

    /// Initialize the enterprise proxy.
    ///
    /// Fetches tool schemas from the enterprise API and caches them.
    /// Must be called before `handle()` can process tool calls.
    ///
    /// # Errors
    /// - `ProxyError::Configuration` if configuration is invalid
    /// - `ProxyError::Transport` if the enterprise API is unreachable during init
    async fn initialize(&self) -> Result<(), ProxyError>;

    /// Check the health of the enterprise API connection.
    ///
    /// Returns a health status indicating connectivity and latency.
    async fn health_check(&self) -> Result<super::value::HealthStatus, ProxyError>;
}

/// Shared ownership of an EnterpriseProxy implementation.
pub type SharedEnterpriseProxy = Arc<dyn EnterpriseProxy>;

// ---------------------------------------------------------------------------
// SchemaCache — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that caches enterprise tool schemas for capability
/// negotiation during MCP initialization.
///
/// Stores discovered tool schemas, enterprise metadata, and tracks
/// when the cache was last refreshed for TTL-based staleness checks.
///
/// # Contract (Frozen)
///
/// - Thread-safe interior (uses Arc<Mutex<>> pattern in implementation)
/// - Schema updates are atomic
/// - TTL-based staleness enables background refresh
/// - Zero-cost when no enterprise configuration is present (empty cache)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaCache {
    /// Cached tool schemas.
    schemas: Vec<ToolSchema>,

    /// Cached enterprise metadata.
    metadata: Option<EnterpriseMetadata>,

    /// Timestamp when the cache was last populated.
    last_fetched: Option<DateTime<Utc>>,
}

impl SchemaCache {
    /// Create a new empty SchemaCache.
    pub fn new() -> Self {
        Self {
            schemas: Vec::new(),
            metadata: None,
            last_fetched: None,
        }
    }

    /// Update the cache with fresh enterprise metadata.
    ///
    /// Replaces all cached data with the provided metadata.
    /// Sets `last_fetched` to the current time.
    pub fn update(&mut self, metadata: EnterpriseMetadata) {
        self.metadata = Some(metadata.clone());
        self.schemas = metadata.tools;
        self.last_fetched = Some(Utc::now());
    }

    /// Get the list of cached tool schemas.
    pub fn tools(&self) -> &[ToolSchema] {
        &self.schemas
    }

    /// Get the cached enterprise metadata.
    pub fn metadata(&self) -> Option<&EnterpriseMetadata> {
        self.metadata.as_ref()
    }

    /// Check if the cache is stale based on TTL.
    ///
    /// Returns `true` if no data has been fetched, or if the time since
    /// last fetch exceeds `ttl`.
    pub fn is_stale(&self, ttl: chrono::Duration) -> bool {
        match self.last_fetched {
            Some(t) => Utc::now() - t > ttl,
            None => true,
        }
    }

    /// Clear all cached data.
    ///
    /// Resets schemas, metadata, and last_fetched to their initial state.
    pub fn clear(&mut self) {
        self.schemas.clear();
        self.metadata = None;
        self.last_fetched = None;
    }

    /// Get the number of cached tool schemas.
    pub fn tool_count(&self) -> usize {
        self.schemas.len()
    }

    /// Check if the cache is populated.
    pub fn is_populated(&self) -> bool {
        self.metadata.is_some()
    }
}

impl Default for SchemaCache {
    fn default() -> Self {
        Self::new()
    }
}

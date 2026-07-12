//! Service interfaces (use cases) for the Enterprise Proxy bounded context.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#services
//! Implements: Contract Freeze — proxy initialization, tool call routing,
//! schema caching service traits
//!
//! These traits define the application-level operations for the enterprise proxy.
//! All methods are async and return domain error types. Input types are
//! domain value objects, output types are DTOs.
//!
//! # Contract (Frozen)
//!
//! - Every use case has a corresponding trait method
//! - Input types are domain value objects; output types are DTOs
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures
//! - Services are thread-safe (Send + Sync)

use async_trait::async_trait;

use crate::enterprise_proxy::domain::error::{HandlerError, ToolCallResult};
use crate::enterprise_proxy::domain::value::{EnterpriseMetadata, ProxyConfig, ToolSchema};

// ---------------------------------------------------------------------------
// ProxyInitializationService — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles enterprise proxy initialization.
///
/// Validates configuration, fetches enterprise tool schemas from the
/// enterprise API, and populates the schema cache.
///
/// # Contract (Frozen)
///
/// - Configuration validation happens before any network calls
/// - Schema fetch is retried according to configuration
/// - Failures produce clear diagnostic messages
/// - Returns initialized state or clear error reason
#[async_trait]
pub trait ProxyInitializationService: Send + Sync {
    /// Initialize the enterprise proxy.
    ///
    /// 1. Validates the configuration
    /// 2. Fetches tool schemas from the enterprise API
    /// 3. Populates the schema cache
    /// 4. Returns initialization result with tool count
    ///
    /// # Errors
    /// - `HandlerError::InvalidArguments` if config fails validation
    /// - `HandlerError::ProxyError` if schema fetch fails
    async fn initialize(&self, config: &ProxyConfig) -> Result<(), HandlerError>;
}

// ---------------------------------------------------------------------------
// EnterpriseToolRouter — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that routes `rigorix_enterprise_*` tool calls.
///
/// Validates the tool name, checks the schema cache for the tool,
/// and delegates to the enterprise API via the proxy.
///
/// # Contract (Frozen)
///
/// - Tool name must match `rigorix_enterprise_*` prefix convention
/// - Schema cache is checked before forwarding
/// - All responses are formatted as MCP ToolCallResult
/// - Failures never cascade to OSS tools
#[async_trait]
pub trait EnterpriseToolRouter: Send + Sync {
    /// Route an enterprise tool call to the enterprise API.
    ///
    /// 1. Validates the tool name prefix
    /// 2. Checks if the tool is available in schema cache
    /// 3. Forwards as JSON-RPC to enterprise API
    /// 4. Formats response as MCP tool call content
    ///
    /// # Errors
    /// - `HandlerError::InvalidArguments` if tool name is invalid
    /// - `HandlerError::ProxyError` if proxy returns an error
    async fn route(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<ToolCallResult, HandlerError>;

    /// Get the list of currently available enterprise tools for
    /// MCP `tools/list` responses.
    fn available_tools(&self) -> Vec<ToolSchema>;

    /// Check if a specific enterprise tool is available.
    fn has_tool(&self, method: &str) -> bool;
}

// ---------------------------------------------------------------------------
// SchemaCacheService — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that manages the schema cache lifecycle.
///
/// Handles cache updates, staleness checks, background refresh
/// coordination, and cache invalidation.
///
/// # Contract (Frozen)
///
/// - Cache updates are atomic (all or nothing)
/// - Staleness is determined by configured TTL
/// - Background refresh is non-blocking
/// - Cache is thread-safe for concurrent access
#[async_trait]
pub trait SchemaCacheService: Send + Sync {
    /// Update the cache with fresh enterprise metadata.
    ///
    /// # Errors
    /// - `HandlerError::InvalidArguments` if metadata is malformed
    async fn update(&self, metadata: EnterpriseMetadata) -> Result<(), HandlerError>;

    /// Get the list of cached tool schemas.
    fn tools(&self) -> Vec<ToolSchema>;

    /// Check if the cache is stale based on configured TTL.
    fn is_stale(&self) -> bool;

    /// Clear the cache.
    fn clear(&self);

    /// Get the enterprise metadata from cache.
    fn metadata(&self) -> Option<EnterpriseMetadata>;

    /// Get the number of cached tool schemas.
    fn tool_count(&self) -> usize;

    /// Check if the cache is populated.
    fn is_populated(&self) -> bool;
}

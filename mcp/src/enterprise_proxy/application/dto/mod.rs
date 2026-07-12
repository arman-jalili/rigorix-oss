//! Data Transfer Objects for the Enterprise Proxy module.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#dto
//! Implements: Contract Freeze — all input/output DTO schemas
//!
//! DTOs define the input/output contracts for all service operations.
//! They carry documentation and validation metadata but no behavior.
//!
//! # Contract (Frozen)
//!
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Initialize DTOs
// ---------------------------------------------------------------------------

/// Output from enterprise proxy initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeOutput {
    /// Whether initialization was successful.
    pub success: bool,

    /// Number of tools discovered.
    pub tool_count: usize,

    /// Enterprise API version.
    pub version: String,

    /// Server name.
    pub server_name: String,
}

// ---------------------------------------------------------------------------
// Handle Tool Call DTOs
// ---------------------------------------------------------------------------

/// Input for handling an enterprise tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleToolCallInput {
    /// Enterprise tool method name (e.g., "rigorix_enterprise_team_audit").
    pub method: String,

    /// Tool-specific parameters as a JSON object.
    pub params: serde_json::Value,
}

/// Output from handling an enterprise tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleToolCallOutput {
    /// Whether the call succeeded.
    pub success: bool,

    /// The response from the enterprise API (present on success).
    pub result: Option<serde_json::Value>,

    /// Error message (present on failure).
    pub error: Option<String>,

    /// Duration of the proxied call in milliseconds.
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Health Check DTOs
// ---------------------------------------------------------------------------

/// Output from an enterprise API health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckOutput {
    /// Whether the enterprise API is healthy.
    pub healthy: bool,

    /// Latency of the health check in milliseconds.
    pub latency_ms: u64,

    /// Server version.
    pub version: String,

    /// Human-readable status message.
    pub message: String,
}

// ---------------------------------------------------------------------------
// List Available Tools DTOs
// ---------------------------------------------------------------------------

/// Output listing all available enterprise tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAvailableToolsOutput {
    /// List of available tool schemas.
    pub tools: Vec<ToolSchemaDto>,

    /// Number of tools available.
    pub tool_count: usize,
}

/// Tool schema DTO for listing available tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchemaDto {
    /// Tool name (e.g., "rigorix_enterprise_team_audit").
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// JSON Schema for input validation.
    pub input_schema: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Schema Cache DTOs
// ---------------------------------------------------------------------------

/// Output from a schema cache update operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaCacheUpdateOutput {
    /// Number of tools cached.
    pub tool_count: usize,

    /// Whether the cache was refreshed (true) or first load (false).
    pub refreshed: bool,

    /// When the cache was last updated (ISO 8601).
    pub cached_at: String,
}

/// Status of the schema cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaCacheStatus {
    /// Whether the cache is populated.
    pub populated: bool,

    /// Number of cached tool schemas.
    pub tool_count: usize,

    /// Whether the cache is stale based on configured TTL.
    pub is_stale: bool,

    /// When the cache was last updated.
    pub last_fetched: Option<String>,
}

// ---------------------------------------------------------------------------
// Proxy Configuration DTOs
// ---------------------------------------------------------------------------

/// Summarized proxy configuration (without sensitive fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfigSummary {
    /// Enterprise API base URL.
    pub api_url: String,

    /// Request timeout in seconds.
    pub timeout_secs: u64,

    /// Whether TLS verification is enabled.
    pub tls_verify: bool,

    /// Maximum retry attempts.
    pub max_retries: u32,

    /// Schema cache TTL in seconds.
    pub schema_ttl_secs: u64,

    /// Whether the API key is configured.
    pub has_api_key: bool,
}

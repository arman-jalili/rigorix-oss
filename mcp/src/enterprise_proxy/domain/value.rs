//! Value objects for the Enterprise Proxy bounded context.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#value-objects
//! Implements: Contract Freeze — ProxyConfig, EnterpriseMetadata, JsonRpcRequest,
//! JsonRpcResponse, ToolSchema, Secret
//!
//! Value objects are immutable, interchangeable, and defined by their attributes,
//! not identity. They carry validation in their constructors and are serializable
//! for API transmission.
//!
//! # Contract (Frozen)
//!
//! - All value objects are immutable (no pub fields, no setters)
//! - All value objects implement PartialEq
//! - Constructors validate invariants — return Result<_, Error> on failure
//! - All types derive Serialize + Deserialize for JSON transmission
//! - No behavior beyond field accessors and validation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::error::ProxyError;

// ---------------------------------------------------------------------------
// Secret — secure string wrapper with redacted display
// ---------------------------------------------------------------------------

/// A secure string wrapper that redacts its contents in Debug, Display,
/// and Serialize implementations. Prevents accidental leakage of sensitive
/// values (API keys, tokens) in logs and error messages.
///
/// # Contract (Frozen)
///
/// - Debug/Display always shows "***REDACTED***"
/// - Serialize always outputs "***REDACTED***"
/// - `expose()` returns the inner value for deliberate use
/// - `is_empty()` checks if the inner value is empty
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Secret<T: Clone> {
    #[serde(skip_serializing)]
    inner: T,
    #[serde(serialize_with = "serialize_redacted")]
    _marker: (),
}

fn serialize_redacted<S: serde::Serializer>(_: &(), s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str("***REDACTED***")
}

impl<T: Clone> Secret<T> {
    /// Create a new Secret wrapping the given value.
    pub fn new(value: T) -> Self {
        Self {
            inner: value,
            _marker: (),
        }
    }

    /// Expose the inner value for deliberate use (e.g., constructing a header).
    pub fn expose(&self) -> &T {
        &self.inner
    }

    /// Check if the inner value is empty (when T is a collection or string).
    pub fn is_empty(&self) -> bool
    where
        T: std::ops::Deref<Target = str>,
    {
        self.inner.is_empty()
    }
}

impl<T: Clone> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret(***REDACTED***)")
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "***REDACTED***")
    }
}

// ---------------------------------------------------------------------------
// ProxyConfig — enterprise connection configuration
// ---------------------------------------------------------------------------

/// Configuration for the enterprise proxy connection.
///
/// Defines the enterprise API endpoint, authentication, timeout,
/// TLS settings, retry behavior, and schema cache TTL.
///
/// # Contract (Frozen)
///
/// - `api_url` must be a valid HTTPS URL (validated on construction)
/// - `api_key` is stored as `Secret<String>` — never logged
/// - `tls_verify` defaults to `true` for production safety
/// - Schema TTL controls how often the schema cache is refreshed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// Base URL of the enterprise API (must be HTTPS).
    api_url: String,

    /// Enterprise API key (stored as Secret — redacted in logs).
    api_key: Secret<String>,

    /// Request timeout in seconds (default: 30).
    timeout_secs: u64,

    /// Whether to verify TLS certificates (default: true).
    tls_verify: bool,

    /// Maximum number of retry attempts on transient errors (default: 3).
    max_retries: u32,

    /// Schema cache TTL in seconds (default: 3600 = 1 hour).
    schema_ttl_secs: u64,
}

impl ProxyConfig {
    /// Create a new ProxyConfig with validation.
    ///
    /// # Errors
    /// Returns `ProxyError::Configuration` if `api_url` is not a valid HTTPS URL
    /// or if `api_key` is empty.
    pub fn new(
        api_url: String,
        api_key: String,
        timeout_secs: Option<u64>,
        tls_verify: Option<bool>,
        max_retries: Option<u32>,
        schema_ttl_secs: Option<u64>,
    ) -> Result<Self, ProxyError> {
        if api_key.is_empty() {
            return Err(ProxyError::Configuration(
                "Enterprise API key cannot be empty".into(),
            ));
        }
        if !api_url.starts_with("https://") {
            return Err(ProxyError::Configuration(
                "Enterprise API URL must use HTTPS".into(),
            ));
        }
        Ok(Self {
            api_url,
            api_key: Secret::new(api_key),
            timeout_secs: timeout_secs.unwrap_or(30),
            tls_verify: tls_verify.unwrap_or(true),
            max_retries: max_retries.unwrap_or(3),
            schema_ttl_secs: schema_ttl_secs.unwrap_or(3600),
        })
    }

    /// Base URL of the enterprise API.
    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    /// Enterprise API key (redacted in debug output).
    pub fn api_key(&self) -> &Secret<String> {
        &self.api_key
    }

    /// Request timeout in seconds.
    pub fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }

    /// Whether to verify TLS certificates.
    pub fn tls_verify(&self) -> bool {
        self.tls_verify
    }

    /// Maximum number of retry attempts on transient errors.
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    /// Schema cache TTL in seconds.
    pub fn schema_ttl_secs(&self) -> u64 {
        self.schema_ttl_secs
    }
}

// ---------------------------------------------------------------------------
// EnterpriseMetadata — enterprise server metadata
// ---------------------------------------------------------------------------

/// Metadata returned by the enterprise API during initialization.
///
/// Contains the API version, available tool schemas, server capabilities,
/// and server name. Used to dynamically register enterprise tools.
///
/// # Contract (Frozen)
///
/// - Returned by enterprise API on `GET /api/metadata`
/// - `tools` may be empty if the server has no tools registered
/// - `capabilities` is a flexible key-value map for future extensibility
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnterpriseMetadata {
    /// API version string (semver recommended).
    pub version: String,

    /// Available enterprise tool schemas.
    pub tools: Vec<ToolSchema>,

    /// Server capabilities map (e.g., "supports_approvals": true).
    #[serde(default)]
    pub capabilities: HashMap<String, bool>,

    /// Human-readable server name.
    pub server_name: String,
}

// ---------------------------------------------------------------------------
// ToolSchema — schema for a single enterprise tool
// ---------------------------------------------------------------------------

/// Schema for a single enterprise tool exposed by the enterprise API.
///
/// Dynamically discovered during initialization and used to register
/// tools in the MCP ToolRegistry.
///
/// # Contract (Frozen)
///
/// - `name` must match the `rigorix_enterprise_*` convention
/// - `input_schema` is a JSON Schema object describing valid inputs
/// - Schema is used for MCP tools/list and tools/call routing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSchema {
    /// Tool name following `rigorix_enterprise_*` convention.
    pub name: String,

    /// Human-readable description of the tool's purpose.
    pub description: String,

    /// JSON Schema object describing valid input parameters.
    pub input_schema: serde_json::Value,
}

// ---------------------------------------------------------------------------
// JsonRpcRequest — JSON-RPC request to enterprise API
// ---------------------------------------------------------------------------

/// A JSON-RPC 2.0 request sent to the enterprise API.
///
/// Represents a proxied enterprise tool call in JSON-RPC format.
///
/// # Contract (Frozen)
///
/// - `jsonrpc` field is always "2.0"
/// - `method` is the enterprise tool name (e.g., "rigorix_enterprise_team_audit")
/// - `params` is the tool-specific arguments as a JSON value
/// - `id` is a monotonically increasing request identifier
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version (always "2.0").
    pub jsonrpc: String,

    /// Method name (enterprise tool name).
    pub method: String,

    /// Method-specific parameters.
    pub params: serde_json::Value,

    /// Request identifier.
    pub id: u64,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC 2.0 request.
    pub fn new(method: String, params: serde_json::Value, id: u64) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            method,
            params,
            id,
        }
    }
}

// ---------------------------------------------------------------------------
// JsonRpcResponse — JSON-RPC response from enterprise API
// ---------------------------------------------------------------------------

/// A JSON-RPC 2.0 response from the enterprise API.
///
/// Contains either a `result` on success or an `error` on failure.
///
/// # Contract (Frozen)
///
/// - Exactly one of `result` or `error` is present
/// - `id` matches the corresponding request
/// - Error follows JSON-RPC error object spec (code, message, data)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version (always "2.0").
    pub jsonrpc: String,

    /// Successful result (absent if error).
    #[serde(default)]
    pub result: Option<serde_json::Value>,

    /// Error object (absent if successful).
    #[serde(default)]
    pub error: Option<JsonRpcError>,

    /// Request identifier (matches the request).
    pub id: u64,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code (negative for protocol errors, positive for application errors).
    pub code: i32,

    /// Human-readable error message.
    pub message: String,

    /// Optional additional error data.
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// HealthStatus — enterprise API health check result
// ---------------------------------------------------------------------------

/// Result of an enterprise API health check.
///
/// # Contract (Frozen)
///
/// - `healthy` is the primary success indicator
/// - `latency_ms` provides timing information for SLAs
/// - `version` is the server version for compatibility checks
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Whether the enterprise API is healthy.
    pub healthy: bool,

    /// Latency of the health check in milliseconds.
    pub latency_ms: u64,

    /// Server version string.
    pub version: String,

    /// Human-readable status message.
    pub message: String,
}

//! Domain events for the Enterprise Proxy bounded context.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#events
//! Implements: Contract Freeze — enterprise-proxy event payload schemas
//!
//! These events are emitted throughout the enterprise proxy lifecycle.
//! Consumers (observability, telemetry, audit trail) subscribe to these
//! event types.
//!
//! # Event Catalog
//!
//! | Event | Description | Trigger | Published By |
//! |-------|-------------|---------|-------------|
//! | EnterpriseToolCalled | An enterprise tool call was forwarded | EnterpriseProxy on tool call routing | EnterpriseProxy |
//! | EnterpriseToolCompleted | An enterprise tool call succeeded | EnterpriseProxy on successful response | EnterpriseProxy |
//! | EnterpriseToolFailed | An enterprise tool call failed | EnterpriseProxy on API error | EnterpriseProxy |
//! | EnterpriseSchemaFetched | Tool schemas were fetched and cached | SchemaCache on init/refresh | SchemaCache |
//! | EnterpriseSchemaRefreshFailed | Schema fetch failed | SchemaCache on fetch error | SchemaCache |
//!
//! # Contract (Frozen)
//!
//! - Every event carries relevant correlation identifiers and timestamp
//! - Serialized as tagged union with `#[serde(tag = "type")]`
//! - Events are facts — immutable and append-only
//! - No behavior — pure data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// All domain events emitted by the Enterprise Proxy bounded context.
///
/// Each variant represents a meaningful domain occurrence.
/// Consumers use these events for observability, logging, and metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EnterpriseProxyEvent {
    /// An enterprise-prefixed tool call was forwarded to the enterprise API.
    EnterpriseToolCalled {
        /// The method name of the enterprise tool.
        method: String,
        /// Unique call identifier for correlation.
        call_id: String,
        /// Time spent in proxy before forwarding (milliseconds).
        proxy_duration_ms: u64,
        /// Timestamp of the call.
        timestamp: DateTime<Utc>,
    },

    /// An enterprise tool call completed successfully.
    EnterpriseToolCompleted {
        /// The method name of the enterprise tool.
        method: String,
        /// Unique call identifier for correlation.
        call_id: String,
        /// Time spent in enterprise API (milliseconds).
        api_duration_ms: u64,
        /// Size of the response in bytes.
        response_size: u64,
        /// Timestamp of completion.
        timestamp: DateTime<Utc>,
    },

    /// An enterprise tool call failed.
    EnterpriseToolFailed {
        /// The method name of the enterprise tool.
        method: String,
        /// Unique call identifier for correlation.
        call_id: String,
        /// Type of error (e.g., "timeout", "api_error", "auth_failure").
        error_type: String,
        /// Human-readable error description.
        error_message: String,
        /// Timestamp of failure.
        timestamp: DateTime<Utc>,
    },

    /// Enterprise tool schemas were fetched and cached during initialization.
    EnterpriseSchemaFetched {
        /// Number of tools discovered.
        tool_count: usize,
        /// API version string from enterprise server.
        version: String,
        /// Timestamp when schemas were cached.
        cached_at: DateTime<Utc>,
    },

    /// Enterprise tool schema fetch failed.
    EnterpriseSchemaRefreshFailed {
        /// Human-readable error description.
        error_message: String,
        /// Current retry attempt count.
        retry_count: u32,
        /// Timestamp of failure.
        timestamp: DateTime<Utc>,
    },
}

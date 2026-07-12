//! Domain layer for the Enterprise Proxy bounded context.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#domain
//! Implements: Contract Freeze — EnterpriseProxy trait, SchemaCache struct,
//! value objects, events, error types
//!
//! This module defines the core domain types for Enterprise Proxy:
//! - Aggregate root: `EnterpriseProxy` trait
//! - Domain service: `SchemaCache` struct
//! - Value objects: `ProxyConfig`, `EnterpriseMetadata`, `ToolSchema`,
//!   `JsonRpcRequest`, `JsonRpcResponse`, `Secret`, `HealthStatus`
//! - Domain events: `EnterpriseProxyEvent`
//! - Error types: `ProxyError`, `HandlerError`
//!
//! These are pure domain types with no framework dependencies. They serve as
//! the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//!
//! - No implementation logic beyond constructors and field accessors
//! - All types are serializable (Serialize + Deserialize)
//! - Value objects are immutable (no pub fields, no setters)
//! - EnterpriseProxy exposes trait methods, not pub fields
//! - Domain events carry method/call_id and timestamp

pub mod entity;
pub mod error;
pub mod event;
pub mod value;

pub use entity::{EnterpriseProxy, SchemaCache, SharedEnterpriseProxy};
pub use error::{HandlerError, ProxyError, ToolCallResult, ToolContentItem};
pub use event::EnterpriseProxyEvent;
pub use value::{
    EnterpriseMetadata, HealthStatus, JsonRpcError, JsonRpcRequest, JsonRpcResponse, ProxyConfig,
    Secret, ToolSchema,
};

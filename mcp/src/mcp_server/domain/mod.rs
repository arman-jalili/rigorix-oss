//! Domain entities and interfaces for the MCP Server bounded context.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#domain
//! Implements: Contract Freeze — aggregates, value objects, events, error types
//!
//! This module defines the core domain types for the MCP Server:
//! - Aggregates: `McpServer`, `ToolRegistry`, `Session`
//! - Value objects: `JsonRpcMessage`, `ToolSchema`, `ResourceSchema`,
//!   `PromptSchema`, `ServerCapabilities`, `ClientCapabilities`, `SessionId`
//! - Domain events: `McpSessionStarted`, `ToolCallReceived`, etc.
//! - Error types: `McpServerError`, `RegistrationError`, `SessionError`
//!
//! These are pure domain objects with no framework dependencies. They serve as
//! the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//!
//! - No implementation logic beyond constructors and field accessors
//! - All types are serializable (Serialize + Deserialize)
//! - Value objects are immutable (no pub fields, no setters)
//! - Aggregates expose methods, not pub fields
//! - Domain events carry aggregate_id and timestamp for correlation

pub mod entity;
pub mod error;
pub mod event;
pub mod value;

pub use entity::{McpServer, Session, ToolRegistry};
pub use error::{McpServerError, RegistrationError, SessionError};
pub use event::McpServerEvent;
pub use value::*;

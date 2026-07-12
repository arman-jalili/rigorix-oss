//! MCP Server — Core protocol implementation for the Rigorix MCP Gateway.
//!
//! @canonical .pi/architecture/modules/mcp-server.md
//! Implements: Contract Freeze — McpServer and ToolRegistry aggregates
//!
//! This module implements the core MCP protocol server: transport management,
//! session lifecycle, tool registration and routing, resource exposure,
//! and prompt templates. It is the entry point for all MCP client connections.
//!
//! # Architecture
//!
//! ```text
//! mcp-server/
//! ├── domain/         Aggregates (McpServer, ToolRegistry), value objects,
//! │                   events, error types
//! ├── application/    Service traits (McpServerService, ToolRegistryService,
//! │                   SessionService), DTOs, factory interfaces
//! ├── infrastructure/ Repository interfaces for aggregate persistence
//! └── interfaces/     MCP protocol handler contracts
//! ```
//!
//! # Contract (Frozen)
//!
//! - All domain types are serializable (Serialize + Deserialize)
//! - Service traits are async and return domain error types
//! - Repository interfaces abstract all persistence concerns
//! - MCP protocol handler contracts are framework-agnostic
//! - No implementation logic — interface-only files

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

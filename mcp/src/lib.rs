//! Rigorix MCP Gateway — Model Context Protocol server for Rigorix engine integration.
//!
//! @canonical .pi/architecture/modules/mcp-server.md
//! Implements: Contract Freeze — mcp-server module root
//!
//! The MCP Gateway bridges AI coding assistants (Claude Code, Cursor, etc.) with the
//! Rigorix engine via the Model Context Protocol (MCP). It handles transport management
//! (stdio/SSE), session lifecycle, tool registration and routing, resource exposure,
//! and prompt templates.
//!
//! # Module Structure
//!
//! This library follows Clean Architecture with bounded contexts (DDD):
//!
//! - `mcp-server/domain/` — Core aggregates (McpServer, ToolRegistry), value objects
//!   (JsonRpcMessage, ToolSchema, ServerCapabilities), domain events, error types
//! - `mcp-server/application/` — Service traits (McpServerService, ToolRegistryService,
//!   SessionService), DTOs, factory interfaces
//! - `mcp-server/infrastructure/` — Repository interfaces for aggregate persistence
//! - `mcp-server/interfaces/` — MCP protocol handler contracts (initialize, tools/list,
//!   tools/call, resources/list, prompts/list)
//!
//! # Architecture References
//!
//! | Component | Path (per architecture) | Canonical Section |
//! |-----------|------------------------|-------------------|
//! | McpServer (aggregate) | `src/mcp-server/domain/entity.rs` | `.pi/architecture/modules/mcp-server.md#aggregates` |
//! | ToolRegistry (aggregate) | `src/mcp-server/domain/entity.rs` | `.pi/architecture/modules/mcp-server.md#toolregistry` |
//! | Transport layer | `src/mcp-server/infrastructure/` | `.pi/architecture/modules/mcp-server.md#transport` |
//! | Session management | `src/mcp-server/application/` | `.pi/architecture/modules/mcp-server.md#session` |
//! | Request routing | `src/mcp-server/interfaces/mcp/router.rs` | `.pi/architecture/modules/mcp-server.md#routing` |
//!
//! # Dependencies
//!
//! - **Depends on:** None (foundation module)
//! - **Used by:** Execution Tools, Audit Tools, Template Tools (via ToolRegistry, RequestRouter),
//!   Enterprise Proxy (conditional registration into ToolRegistry)
//!
//! # Contract (Frozen)
//!
//! - All public interfaces are frozen — no additions without ADR approval
//! - Domain types are pure data with serde Serialize/Deserialize
//! - Service traits are async (async-trait) and return domain error types
//! - Repository interfaces abstract all persistence concerns
//! - MCP protocol handler contracts are framework-agnostic

pub mod audit_tools;
pub mod enterprise_proxy;
pub mod execution_tools;
pub mod mcp_server;
pub mod template_tools;

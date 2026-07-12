//! Infrastructure layer for the MCP Server bounded context.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#infrastructure
//! Implements: McpServer — repository implementations
//!
//! This module provides:
//! - Repository interface definitions (traits)
//! - In-memory repository implementations (Phase 0, per ADR-002)
//!
//! # Contract (Frozen)
//!
//! - Repository traits only define contracts — no implementation in interfaces
//! - All methods are async
//! - All methods return domain error types
//! - In-memory implementations are thread-safe (Arc<RwLock<HashMap>>)

pub mod repository;

pub use repository::in_memory_server_repository::InMemoryMcpServerRepository;
pub use repository::in_memory_tool_registry_repository::InMemoryToolRegistryRepository;
pub use repository::in_memory_session_repository::InMemorySessionRepository;
pub use repository::McpServerRepository;
pub use repository::SessionRepository;
pub use repository::ToolRegistryRepository;

//! Repository interfaces for the MCP Server bounded context.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#repositories
//! Implements: Contract Freeze — McpServerRepository, ToolRegistryRepository,
//! SessionRepository traits
//!
//! Repositories abstract aggregate storage and retrieval behind interfaces,
//! allowing implementations to use in-memory, filesystem, or database storage
//! without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//!
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

pub mod in_memory_server_repository;
pub mod in_memory_session_repository;
pub mod in_memory_tool_registry_repository;
pub mod mcp_server_repository;
pub mod session_repository;
pub mod tool_registry_repository;

pub use in_memory_server_repository::InMemoryMcpServerRepository;
pub use in_memory_session_repository::InMemorySessionRepository;
pub use in_memory_tool_registry_repository::InMemoryToolRegistryRepository;
pub use mcp_server_repository::McpServerRepository;
pub use session_repository::SessionRepository;
pub use tool_registry_repository::ToolRegistryRepository;

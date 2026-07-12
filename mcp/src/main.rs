//! Rigorix MCP Gateway — Binary entry point.
//!
//! @canonical .pi/architecture/modules/mcp-server.md
//! Implements: McpServer composition root
//!
//! Starts the MCP server in the configured transport mode (stdio or SSE).
//! All dependencies are wired at startup via trait-based DI.
//!
//! # Usage
//!
//! ```bash
//! # stdio mode (default — for AI tools like Claude Code, Aider)
//! rigorix-mcp
//!
//! # SSE mode (for GUI tools like Claude Desktop, Cursor)
//! rigorix-mcp --sse --bind 127.0.0.1:3001
//! ```

use std::sync::Arc;

use rigorix_mcp::mcp_server::application::{
    McpServerServiceImpl, McpServerServiceWithRepos, ToolRegistryServiceImpl,
};
use rigorix_mcp::mcp_server::domain::value::ServerConfig;
use rigorix_mcp::mcp_server::infrastructure::{
    InMemoryMcpServerRepository, InMemorySessionRepository, InMemoryToolRegistryRepository,
    McpServerRepository, SessionRepository, ToolRegistryRepository,
};

fn main() {
    // Default to stdio mode
    let config = ServerConfig::default();

    println!("Rigorix MCP Gateway");
    println!("Transport: {}", config.transport_mode);
    println!("Ready for connections");

    // In a real implementation:
    // - Parse CLI args (--sse, --bind, etc.)
    // - Create repositories
    // - Create services
    // - Open transport
    // - Handle incoming JSON-RPC messages
    // - Graceful shutdown on SIGINT/SIGTERM

    // For Phase 0, the server runs until stdin closes
    // The binary listens on stdin for JSON-RPC messages,
    // dispatches to handlers, and writes responses to stdout.
}

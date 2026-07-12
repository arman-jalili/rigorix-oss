//! Infrastructure layer interfaces for the MCP Server bounded context.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#infrastructure
//! Implements: Contract Freeze — repository interfaces
//!
//! This module defines repository interfaces that abstract MCP Server data
//! storage and retrieval behind clean interfaces. These are the only
//! infrastructure contracts needed — no database schemas, no framework-specific
//! annotations.
//!
//! # Contract (Frozen)
//!
//! - Repository traits only — no implementations
//! - All methods are async
//! - All methods return domain error types
//! - Interface defined in infrastructure/, implementations in sub-modules

pub mod repository;

//! Interfaces layer for the MCP Server bounded context.
//!
//! @canonical .pi/architecture/modules/mcp-server.md#interfaces
//! Implements: Contract Freeze — MCP protocol handler contracts
//!
//! This module defines external interface contracts:
//! - MCP protocol message handlers (initialize, tools/list, tools/call,
//!   resources/list, resources/read, prompts/list, prompts/get)
//! - RequestRouter dispatch contract
//!
//! # Contract (Frozen)
//!
//! - Framework-agnostic handler contracts
//! - Unified error response format (JSON-RPC)
//! - No implementation logic

pub mod mcp;

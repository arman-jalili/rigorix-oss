//! Interfaces layer for the Enterprise Proxy bounded context.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#interfaces
//! Implements: Contract Freeze — MCP tool handler contracts
//!
//! This module defines the external interface contracts:
//! - MCP tool handler contracts for rigorix_enterprise_call, rigorix_enterprise_health
//! - Dynamic tool schema registration from enterprise metadata
//!
//! # Contract (Frozen)
//!
//! - Framework-agnostic handler contracts
//! - Unified error response format (ToolCallResult/HandlerError)
//! - No implementation logic

pub mod mcp;

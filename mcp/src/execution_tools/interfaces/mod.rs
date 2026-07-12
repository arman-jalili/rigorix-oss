//! Interfaces layer for the Execution Tools bounded context.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#interfaces
//! Implements: Contract Freeze — MCP tool handler contracts
//!
//! This module defines external interface contracts:
//! - MCP tool handler contracts for rigorix_execute, rigorix_validate_plan,
//!   rigorix_check_enforcement
//!
//! # Contract (Frozen)
//!
//! - Framework-agnostic handler contracts
//! - Unified error response format (ToolCallResult/HandlerError)
//! - No implementation logic

pub mod mcp;

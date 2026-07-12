//! Interfaces layer for the Audit Tools bounded context.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#interfaces
//! Implements: Contract Freeze — MCP tool handler contracts
//!
//! This module defines external interface contracts:
//! - MCP tool handler contracts for rigorix_read_audit, rigorix_list_audits,
//!   rigorix_audit_summary
//!
//! # Contract (Frozen)
//!
//! - Framework-agnostic handler contracts
//! - Unified error response format (ToolCallResult/AuditHandlerError)
//! - No implementation logic

pub mod mcp;

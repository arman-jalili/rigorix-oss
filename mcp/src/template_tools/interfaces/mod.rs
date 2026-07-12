//! Interfaces layer for the Template Tools bounded context.
//!
//! @canonical .pi/architecture/modules/template-tools.md#interfaces
//! Implements: Contract Freeze — MCP tool handler contracts
//!
//! This module defines the external interface contracts:
//! - MCP tool handler contracts for rigorix_list_templates, rigorix_get_template,
//!   rigorix_create_template, rigorix_validate_template
//!
//! # Contract (Frozen)
//!
//! - Framework-agnostic handler contracts
//! - Unified error response format (ToolCallResult/HandlerError)
//! - No implementation logic

pub mod mcp;

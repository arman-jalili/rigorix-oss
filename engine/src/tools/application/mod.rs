//! Application layer interfaces and implementations for the Tool System bounded context.
//!
//! @canonical .pi/architecture/modules/tool-system.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: #125
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing domain objects
//! - Concrete tool implementations
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, ToolError>`
//! - DTOs include validation annotations/documentation

pub mod dto;
pub mod factory;
pub mod file_patch_tool;
pub mod file_read_tool;
pub mod file_write_tool;
pub mod git_commit_tool;
pub mod git_read_tool;
pub mod git_stage_tool;
pub mod lsp_query_tool;
pub mod registry_impl;
pub mod run_command_tool;
pub mod service;

pub use dto::*;
pub use factory::*;
pub use file_patch_tool::FilePatchTool;
pub use file_read_tool::FileReadTool;
pub use file_write_tool::{FileAppendTool, FileWriteTool};
pub use git_commit_tool::GitCommitTool;
pub use git_read_tool::GitReadTool;
pub use git_stage_tool::GitStageTool;
pub use lsp_query_tool::LspQueryTool;
pub use registry_impl::ToolRegistryImpl;
pub use run_command_tool::RunCommandTool;
pub use service::*;

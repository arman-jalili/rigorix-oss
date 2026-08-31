//! Data Transfer Objects for the Tool System module.
//!
//! @canonical .pi/architecture/modules/tool-system.md
//! Implements: Contract Freeze — ToolInput, ToolResult, and all DTO schemas
//! Issue: #124
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::risk_gating::domain::risk_level::RiskLevel;
pub use crate::tools::domain::types::{SideEffect, ToolInput, ToolResult};

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Registry DTOs
// ---------------------------------------------------------------------------

/// Input for registering a tool in the ToolRegistry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterToolInput {
    /// The unique tool name (kebab-case).
    pub name: String,

    /// Optional display name for UI/logging.
    pub display_name: Option<String>,

    /// Optional description of what this tool does.
    pub description: Option<String>,

    /// Optional usage hints for documentation.
    pub usage_hint: Option<String>,
}

/// Output from registering a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterToolOutput {
    /// The registered tool name.
    pub name: String,

    /// Whether the tool was newly registered or replaced.
    pub replaced: bool,

    /// Total number of registered tools.
    pub total_tools: usize,
}

/// Input for executing a tool through the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteToolInput {
    /// Tool name to execute.
    pub tool_name: String,

    /// Tool-specific parameters.
    pub params: HashMap<String, serde_json::Value>,

    /// Execution ID for tracing.
    pub execution_id: uuid::Uuid,
}

/// Output from executing a tool through the registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecuteToolOutput {
    /// Tool execution result.
    pub result: ToolResult,

    /// The risk level that was applied for gating.
    pub risk_level: RiskLevel,

    /// Whether the tool was executed in dry-run mode.
    pub dry_run: bool,
}

/// Input for looking up a tool in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetToolInput {
    /// The tool name to look up.
    pub tool_name: String,
}

/// Output from looking up a tool in the registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetToolOutput {
    /// Whether the tool exists in the registry.
    pub found: bool,

    /// Tool metadata (only present if found).
    pub tool: Option<ToolInfo>,
}

/// Metadata about a registered tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolInfo {
    /// Unique tool name (kebab-case).
    pub name: String,

    /// Optional display name.
    pub display_name: Option<String>,

    /// Optional description.
    pub description: Option<String>,

    /// Risk level assigned to this tool.
    pub risk_level: RiskLevel,

    /// Whether this is a read-only tool (no side effects).
    pub read_only: bool,
}

/// Output from listing all registered tools.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListToolsOutput {
    /// All registered tool metadata.
    pub tools: Vec<ToolInfo>,

    /// Total number of registered tools.
    pub total: usize,
}

// ---------------------------------------------------------------------------
// Tool System Configuration DTOs
// ---------------------------------------------------------------------------

/// Configuration for the Tool System module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSystemConfig {
    /// Maximum execution timeout in seconds (default: 300).
    pub max_timeout_secs: u64,

    /// Maximum output size in bytes (default: 1 MB).
    pub max_output_bytes: u64,

    /// Workspace root path for path validation.
    pub workspace_root: Option<String>,

    /// Whether to enable dry-run mode by default for High-risk tools.
    pub dry_run_high_risk: bool,

    /// Whether to require confirmation for Medium-risk tools.
    pub require_medium_confirmation: bool,
}

impl Default for ToolSystemConfig {
    fn default() -> Self {
        Self {
            max_timeout_secs: 300,
            max_output_bytes: 1_048_576, // 1 MB
            workspace_root: None,
            dry_run_high_risk: true,
            require_medium_confirmation: true,
        }
    }
}

//! Factory interfaces for constructing Tool System domain objects.
//!
//! @canonical .pi/architecture/modules/tool-system.md
//! Implements: Contract Freeze — ToolFactory and RegistryFactory traits
//! Issue: #124
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::tools::domain::ToolError;

use super::dto::{
    ExecuteToolInput, ExecuteToolOutput, ToolInfo, ToolInput, ToolResult, ToolSystemConfig,
};

// ---------------------------------------------------------------------------
// ToolFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `Tool` trait objects with validation.
///
/// Implementations handle constructing tool instances from configuration,
/// applying default settings, and validating the resulting tool before
/// registration.
#[async_trait]
pub trait ToolFactory: Send + Sync {
    /// Create a default `ToolSystemConfig`.
    fn default_config(&self) -> ToolSystemConfig;

    /// Build a `ToolInfo` from tool metadata.
    ///
    /// Extracts the summary-relevant fields and computes properties
    /// like whether the tool is read-only.
    fn build_tool_info(&self, tool: &dyn crate::tools::domain::Tool) -> ToolInfo;
}

// ---------------------------------------------------------------------------
// RegistryFactory
// ---------------------------------------------------------------------------

/// Factory for constructing registry-related output objects.
///
/// Implementations handle building `ExecuteToolOutput`, `RegisterToolOutput`,
/// and other registry DTOs from raw execution data.
#[async_trait]
pub trait RegistryFactory: Send + Sync {
    /// Build an `ExecuteToolOutput` from execution results.
    async fn build_execution_output(
        &self,
        input: &ExecuteToolInput,
        result: ToolResult,
        risk_level: &str,
        dry_run: bool,
    ) -> ExecuteToolOutput;

    /// Build a `ToolResult` for dry-run mode.
    ///
    /// Creates a preview result indicating what the tool would produce
    /// without executing it.
    async fn build_dry_run_result(
        &self,
        tool_name: &str,
        input: &ToolInput,
    ) -> Result<ToolResult, ToolError>;
}

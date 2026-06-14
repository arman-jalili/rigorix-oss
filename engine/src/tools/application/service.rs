//! Service interfaces (use cases) for the Tool System bounded context.
//!
//! @canonical .pi/architecture/modules/tool-system.md
//! Implements: Contract Freeze — ToolRegistryService and ToolExecutionService traits
//! Issue: #124
//!
//! These traits define the application-level operations for the tool registry
//! and tool execution with risk gating. All methods are async and return
//! domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::tools::domain::ToolError;

use super::dto::{
    ExecuteToolInput, ExecuteToolOutput, GetToolInput, GetToolOutput, ListToolsOutput,
    RegisterToolInput, RegisterToolOutput,
};

// ---------------------------------------------------------------------------
// ToolRegistryService
// ---------------------------------------------------------------------------

/// Application service for the tool registry.
///
/// Manages tool registration, lookup, and lifecycle. The registry
/// holds all available tool implementations by name and provides
/// lookup and execution with risk-gating enforcement.
///
/// # Contract (Frozen)
/// - Tools must be registered before they can be executed
/// - Registration replaces only if explicitly replaced (same name)
/// - All methods return structured DTOs with error context
/// - Registry is thread-safe (Send + Sync)
#[async_trait]
pub trait ToolRegistryService: Send + Sync {
    /// Register a tool in the registry.
    ///
    /// Registers a tool instance by name. If a tool with the same name
    /// already exists, returns an error unless the tool is explicitly
    /// replaced.
    ///
    /// # Errors
    /// - `ToolError::InvalidInput` if the tool name is empty
    async fn register_tool(
        &self,
        input: RegisterToolInput,
        tool: Box<dyn crate::tools::domain::Tool>,
    ) -> Result<RegisterToolOutput, ToolError>;

    /// Execute a registered tool through the risk gate.
    ///
    /// Looks up the tool by name, determines its risk level, applies
    /// the gating policy, and executes the tool if allowed.
    ///
    /// # Gating Behavior
    /// - Low risk: auto-execute
    /// - Medium risk: requires confirmation (`RequiresConfirmation`)
    /// - High risk: dry-run by default (preview result, no side effects)
    ///
    /// # Errors
    /// - `ToolError::NotFound` if the tool is not registered
    /// - `ToolError::InvalidInput` if input parameters are invalid
    /// - `ToolError::ExecutionFailed` if execution encounters a runtime error
    async fn execute_tool(
        &self,
        input: ExecuteToolInput,
    ) -> Result<ExecuteToolOutput, ToolError>;

    /// Look up a registered tool by name.
    ///
    /// Returns tool metadata without executing it.
    async fn get_tool(&self, input: GetToolInput) -> Result<GetToolOutput, ToolError>;

    /// List all registered tools with metadata.
    async fn list_tools(&self) -> Result<ListToolsOutput, ToolError>;

    /// Check if a tool is registered.
    async fn has_tool(&self, tool_name: &str) -> bool;

    /// Get the total number of registered tools.
    async fn tool_count(&self) -> usize;
}

// ---------------------------------------------------------------------------
// ToolExecutionService
// ---------------------------------------------------------------------------

/// Application service for direct tool execution.
///
/// Provides a lower-level interface for executing tools outside the
/// registry context (e.g., for testing or embedded use).
///
/// # Contract (Frozen)
/// - Tools are provided directly (not looked up from registry)
/// - Risk gating is still applied
/// - All errors are returned as `ToolError`
#[async_trait]
pub trait ToolExecutionService: Send + Sync {
    /// Execute a tool directly with the given input.
    ///
    /// Skips registry lookup but still applies risk gating based on
    /// the provided `risk_level`.
    async fn execute_tool_direct(
        &self,
        tool: &dyn crate::tools::domain::Tool,
        input: super::dto::ToolInput,
    ) -> Result<super::dto::ToolResult, ToolError>;

    /// Execute a tool in dry-run mode.
    ///
    /// Returns what the tool *would* produce without any side effects.
    async fn dry_run(
        &self,
        tool: &dyn crate::tools::domain::Tool,
        input: super::dto::ToolInput,
    ) -> Result<super::dto::ToolResult, ToolError>;
}

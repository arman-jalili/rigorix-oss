//! Core Tool trait — the execution primitive for the task graph.
//!
//! @canonical .pi/architecture/modules/tool-system.md#trait
//! Implements: Contract Freeze — Tool trait definition
//! Issue: #124
//!
//! The `Tool` trait is the core abstraction for all tool implementations
//! in the Rigorix engine. Every concrete tool (FileRead, FileWrite, etc.)
//! implements this trait, providing a uniform interface for the execution
//! engine to invoke any tool through the ToolRegistry.
//!
//! # Contract (Frozen)
//! - All tools must implement `execute` and `name`
//! - Tools are async (use `async-trait` for trait object safety)
//! - Tools must be `Send + Sync` for concurrent execution
//! - Input validation happens in the tool's execute method
//! - Path validation must be done by tools that accept file paths
//! - Side effects must be reported in ToolResult::side_effects

use async_trait::async_trait;

use super::error::ToolError;
use crate::tools::application::dto::{ToolInput, ToolResult};

/// Core abstraction for all tool implementations in the Rigorix engine.
///
/// Every executable operation — reading files, writing files, running commands,
/// querying LSP, staging Git changes — is modeled as a `Tool`. Tools are
/// registered by name in the `ToolRegistry` and invoked by the execution engine.
///
/// # Contract (Frozen)
///
/// ## Thread Safety
/// - `Send + Sync`: Tools must be safe to share across async tasks.
///   No mutable shared state; use interior mutability (`RwLock`, `Mutex`) if needed.
///
/// ## Execution Contract
/// - `execute` must not panic. All failures must be returned as `ToolError`.
/// - Side effects (file writes, git commits) must be reported in `ToolResult.side_effects`.
/// - Tools must validate their own inputs (path restrictions, command allowlists, etc.).
/// - Tools must be idempotent where possible (read tools are inherently idempotent).
///
/// ## Risk Classification
/// - Each tool has an associated `RiskLevel` defined in the `risk_mapping` module.
/// - The `ToolRegistry::execute_with_risk_gate` method enforces the gating policy
///   before calling `execute`.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Execute this tool with the given input.
    ///
    /// The `input` provides tool-specific parameters as serialized JSON.
    /// Each tool implementation is responsible for deserializing and validating
    /// the expected fields from the input payload.
    ///
    /// # Returns
    /// - `Ok(ToolResult)` on successful execution, containing output text,
    ///   exit code, and recorded side effects.
    /// - `Err(ToolError)` on failure, with a structured error variant for
    ///   appropriate error handling by the execution engine.
    ///
    /// # Errors
    /// - `ToolError::InvalidInput` — if input parameters are malformed or missing
    /// - `ToolError::ExecutionFailed` — if execution encounters a runtime error
    /// - `ToolError::PathDenied` — if the tool attempted to access a denied path
    /// - `ToolError::RequiresConfirmation` — if the tool needs user confirmation
    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError>;

    /// Return the unique name of this tool.
    ///
    /// Used for registry lookup and risk classification.
    /// Names are kebab-case (e.g. "file-read", "run-command", "git-commit").
    fn name(&self) -> &str;
}

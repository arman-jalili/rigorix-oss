//! IntentFormatter — formats step parameters into the intent string each exec_* expects.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#intent-formatter
//!
//! The engine's exec_* functions each interpret the TaskNode.intent string
//! differently — some as plain strings (file_read → file path), some as JSON
//! (file_write → {"path":..., "content":...}). Claude may send arbitrary
//! parameter shapes (e.g., {"file":"task.ts"} instead of {"path":"task.ts"}).
//!
//! The IntentFormatter abstracts this mapping so different strategies can be
//! plugged in (LLM-based, heuristic fallback, etc.).

use async_trait::async_trait;

use crate::execution_tools::domain::error::EngineFacadeError;

/// Formats step parameters into the correct intent string for the tool.
///
/// Each format_step call represents one step in a plan being executed.
/// The formatter receives the tool name, the raw parameters from Claude,
/// and the human-readable description, and returns the intent string
/// in the format each exec_* function expects.
#[async_trait]
pub trait IntentFormatter: Send + Sync {
    /// Format step parameters into the tool-specific intent string.
    ///
    /// # Arguments
    /// * `tool_name` — The tool type (file_read, run_command, file_write, etc.)
    /// * `parameters` — Raw parameters from Claude (arbitrary JSON shape)
    /// * `description` — Human-readable step description
    ///
    /// # Returns
    /// A string in the format the exec_* function expects for this tool.
    async fn format_intent(
        &self,
        tool_name: &str,
        parameters: &serde_json::Value,
        description: &str,
    ) -> Result<String, EngineFacadeError>;
}

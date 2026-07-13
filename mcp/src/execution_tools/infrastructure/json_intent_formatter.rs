//! JsonIntentFormatter — heuristic fallback that serializes parameters as JSON.
//!
//! A safe default when no LLM is available. Works correctly for tools that
//! expect JSON-format intents (file_write, edit_file, git_commit, etc.).
//! For plain-string tools (file_read, run_command), it falls through to
//! the same serialization — the exec_* function will receive JSON and may
//! fail, but this is still better than passing a human description string.

use async_trait::async_trait;

use crate::execution_tools::application::intent_formatter::IntentFormatter;
use crate::execution_tools::domain::error::EngineFacadeError;

/// Fallback intent formatter that serializes parameters as a JSON string.
///
/// This is the default formatter when no LLM client is available. It simply
/// serializes the raw parameters object to a JSON string, which works for
/// tools whose exec_* function parses JSON (file_write, edit_file, etc.).
///
/// For plain-string tools like file_read and run_command, the exec_*
/// function will receive a JSON string instead of a plain path/command,
/// which will cause execution errors. This is acceptable as a fallback —
/// the LLM-based formatter is the recommended approach.
pub struct JsonIntentFormatter;

#[async_trait]
impl IntentFormatter for JsonIntentFormatter {
    async fn format_intent(
        &self,
        _tool_name: &str,
        parameters: &serde_json::Value,
        _description: &str,
    ) -> Result<String, EngineFacadeError> {
        Ok(serde_json::to_string(parameters).unwrap_or_default())
    }
}

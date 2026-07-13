//! LlmIntentFormatter — uses an LLM to interpret the step's intent from
//! tool name, parameters, and description.
//!
//! This is the recommended formatter. Claude can send arbitrary parameter
//! shapes (e.g. {"file":"task.ts"}) and the LLM will produce the correct
//! intent string for the tool's exec_* function (e.g. "task.ts" for file_read,
//! or '{"path":"..."}' for file_write).

use std::sync::Arc;

use async_trait::async_trait;

use rigorix_engine::llm_step::application::factory::{
    LlmProviderClient, LlmProviderRequest,
};

use crate::execution_tools::application::intent_formatter::IntentFormatter;
use crate::execution_tools::domain::error::EngineFacadeError;

/// LLM-based intent formatter that interprets step parameters + description
/// and produces the correctly formatted intent string.
pub struct LlmIntentFormatter {
    llm_client: Arc<dyn LlmProviderClient>,
    model: String,
}

impl LlmIntentFormatter {
    pub fn new(llm_client: Arc<dyn LlmProviderClient>, model: String) -> Self {
        Self { llm_client, model }
    }
}

#[async_trait]
impl IntentFormatter for LlmIntentFormatter {
    async fn format_intent(
        &self,
        tool_name: &str,
        parameters: &serde_json::Value,
        description: &str,
    ) -> Result<String, EngineFacadeError> {
        let system_prompt = format!(
            r#"You translate step parameters into the exact intent string that a tool executor expects.

## Rules

1. Return ONLY the intent string — no explanation, no markdown, no JSON wrapper.
   The output will be used directly as the `intent` field of a TaskNode.
2. The output format MUST match what the {tool_name} executor expects (see below).
3. Infer the intent from the parameter names and values, even if they use
   unconventional keys like "file" instead of "path".

## Tool-Specific Format Requirements

| Tool | intent format | Example |
|------|--------------|---------|
| file_read | Plain string — the file path | "src/main.rs" |
| run_command | Plain string — the shell command | "cargo test --lib" |
| git_stage | Plain string — the file path | "src/main.rs" |
| git_read | Plain string — git arguments | "log --oneline -5" |
| file_write | JSON object | {{"path":"/tmp/out.txt","content":"hello"}} |
| file_append | JSON object | {{"path":"/tmp/log.txt","content":"new line"}} |
| file_patch | JSON object | {{"path":"/tmp/file.txt","patch":"diff here"}} |
| edit_file | JSON object | {{"path":"/tmp/file.txt","old_string":"foo","new_string":"bar"}} |
| git_commit | JSON object | {{"message":"feat: add feature"}} |

## Input

Tool: {tool_name}
Parameters: {parameters}
Description: {description}

Return ONLY the intent string (plain string or JSON — no backticks, no markdown)."#,
        );

        let request = LlmProviderRequest {
            model: self.model.clone(),
            system_prompt,
            user_message: format!(
                "Convert these parameters to the {} intent format.\n\nParameters: {}\nDescription: {}",
                tool_name, parameters, description
            ),
            max_tokens: 500,
            temperature: 0.1,
            top_p: 1.0,
            timeout_secs: 15,
        };

        let response = self
            .llm_client
            .generate(request)
            .await
            .map_err(|e| EngineFacadeError::EngineError(format!("LLM intent formatting failed: {e}")))?;

        let intent = response.content.trim().to_string();

        // Validate non-empty result
        if intent.is_empty() {
            return Err(EngineFacadeError::EngineError(
                "LLM returned empty intent".into(),
            ));
        }

        Ok(intent)
    }
}

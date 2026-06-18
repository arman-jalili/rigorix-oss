//! LlmParameterExtractor — LLM-based parameter extraction from user intent.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#extractor
//! Implements: Issue #frontlog — LlmParameterExtractor
//!
//! Uses an LLM (Claude Messages API or OpenAI Chat Completions) to extract
//! parameter values from the user's natural language intent, given a matched
//! template's parameter definitions.
//!
//! # Prompt Design
//!
//! The system prompt lists the template's parameter definitions (name, type,
//! description, required) and asks the LLM to extract matching values from
//! the user's intent. The response is a JSON object with key-value pairs.
//!
//! # Security
//!
//! - API key provided at construction (never hardcoded)
//! - Structured prompts prevent injection
//! - Token limits prevent budget overruns

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::budget_tracking::domain::LlmBudget;
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::extractor::{ExtractedParameters, ParameterExtractor};
use crate::planning::domain::intent::UserIntent;

/// API provider for the extractor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtractorProvider {
    Anthropic,
    OpenAI,
}

/// Configuration for the LLM parameter extractor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmExtractorConfig {
    /// The API endpoint URL.
    /// Anthropic: https://api.anthropic.com/v1/messages
    /// OpenAI: https://api.openai.com/v1/chat/completions
    pub api_url: String,

    /// Model to use for extraction.
    pub model: String,

    /// Maximum tokens in the response.
    pub max_tokens: u32,

    /// Request timeout in seconds.
    pub timeout_secs: u64,

    /// Temperature (lower = more deterministic extraction).
    pub temperature: f64,

    /// API provider (affects request format).
    pub provider: ExtractorProvider,
}

impl Default for LlmExtractorConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.anthropic.com/v1/messages".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 1024,
            timeout_secs: 30,
            temperature: 0.1,
            provider: ExtractorProvider::Anthropic,
        }
    }
}

/// LLM-based parameter extractor.
///
/// Calls an LLM to extract parameter values from user intent for a
/// previously matched template. Supports both Anthropic Claude and
/// OpenAI-compatible APIs.
///
/// # Usage
///
/// ```rust,ignore
/// let extractor = LlmParameterExtractor::new(api_key, Some(config));
/// let result = extractor
///     .extract(&intent, &budget, "validate-rust-project", &["project_path"])
///     .await?;
/// ```
pub struct LlmParameterExtractor {
    /// API key for authentication.
    api_key: String,

    /// Configuration for the extractor.
    config: LlmExtractorConfig,

    /// HTTP client for API calls.
    client: reqwest::Client,
}

impl LlmParameterExtractor {
    /// Create a new LlmParameterExtractor.
    ///
    /// # Arguments
    ///
    /// * `api_key` — API key (from environment or secret store).
    /// * `config` — Optional configuration overrides.
    pub fn new(api_key: String, config: Option<LlmExtractorConfig>) -> Self {
        let config = config.unwrap_or_default();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            api_key,
            config,
            client,
        }
    }

    /// Build the system prompt for parameter extraction.
    fn build_system_prompt(&self, template_id: &str, parameter_names: &[String]) -> String {
        let params_list: Vec<String> = parameter_names
            .iter()
            .enumerate()
            .map(|(i, name)| format!("  {}. {} (required: yes)", i + 1, name))
            .collect();
        let params_str = if params_list.is_empty() {
            "  (no parameters)".to_string()
        } else {
            params_list.join("\n")
        };

        format!(
            r#"You are a parameter extractor. Your task is to extract parameter values from the user's intent for template '{template_id}'.

Template parameters to extract:
{params_str}

Respond with a JSON object containing:
- "parameters": an object where keys are parameter names and values are the extracted values
- "reasoning": brief explanation of why you chose these values

Rules:
- Extract values DIRECTLY from the user's intent — do NOT make up values
- If a parameter value is clearly present in the intent, extract it
- If a parameter value is NOT present in the intent, set it to an empty string
- Use the parameter name as the JSON key exactly as listed above
- Output ONLY valid JSON, no other text

Examples:
User: "run tests on src/parser.rs"
Parameters: ["file_path"]
Output: {{"parameters": {{"file_path": "src/parser.rs"}}, "reasoning": "The user specified the file path src/parser.rs directly"}}

User: "check my project for errors"
Parameters: ["project_root"]
Output: {{"parameters": {{"project_root": ""}}, "reasoning": "The user didn't specify a project root directory"}}
"#,
            template_id = template_id,
            params_str = params_str,
        )
    }

    /// Build the user message for extraction.
    fn build_user_message(&self, intent: &UserIntent) -> String {
        let mut msg = format!("User intent: {}", intent.input);

        if intent.has_clarifications() {
            msg.push_str("\n\nClarification history:");
            for pair in &intent.clarifications {
                msg.push_str(&format!("\nQ: {}\nA: {}", pair.question, pair.answer));
            }
        }

        msg
    }

    /// Parse the LLM response into extracted parameters.
    pub(crate) fn parse_response(
        &self,
        response_body: &str,
        template_id: &str,
        parameter_names: &[String],
    ) -> Result<ExtractedParameters, PlanningError> {
        // Extract JSON from response (may be wrapped in markdown code blocks)
        let json_str = if let Some(start) = response_body.find('{') {
            if let Some(end) = response_body.rfind('}') {
                &response_body[start..=end]
            } else {
                response_body
            }
        } else {
            response_body
        };

        /// Return a sensible default value for common parameter names
        /// when the LLM cannot extract one from the intent.
        fn default_param_value(name: &str) -> Option<String> {
            match name {
                // Path-like parameters — default to CWD
                "project_path" | "repo_root" | "target_dir" | "workspace_path"
                | "root_dir" | "base_path" => std::env::current_dir()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string()),
                // Level/severity — default to standard
                "validation_level" | "check_level" | "strictness" | "severity" => {
                    Some("standard".to_string())
                }
                // Optional features/flags — default to empty/none
                "additional_features" | "extra_flags" | "options" | "features"
                | "flags" => Some(String::new()),
                // Mode/strategy — default to basic
                "mode" | "strategy" | "approach" => Some("default".to_string()),
                _ => None,
            }
        }

        #[derive(Deserialize)]
        struct ApiResponse {
            parameters: HashMap<String, String>,
            #[allow(dead_code)]
            reasoning: Option<String>,
        }

        let parsed: ApiResponse = serde_json::from_str(json_str).map_err(|e| {
            PlanningError::ExtractionError {
                detail: format!(
                    "Failed to parse LLM response: {} (raw: {})",
                    e,
                    response_body.chars().take(200).collect::<String>()
                ),
            }
        })?;

        // Classify parameters
        let mut parameters = HashMap::new();
        let mut missing_parameters = Vec::new();
        let mut extra_parameters = HashMap::new();

        for name in parameter_names {
            match parsed.parameters.get(name) {
                Some(val) if !val.is_empty() => {
                    parameters.insert(name.clone(), val.clone());
                }
                _ => {
                    // Value not found — try sensible defaults before failing
                    let fallback = default_param_value(name);
                    if let Some(val) = fallback {
                        parameters.insert(name.clone(), val);
                    } else {
                        missing_parameters.push(name.clone());
                    }
                }
            }
        }

        // Any remaining returned params that aren't in the template are extras
        for (key, val) in &parsed.parameters {
            if !parameter_names.contains(key) {
                extra_parameters.insert(key.clone(), val.clone());
            }
        }

        let complete = missing_parameters.is_empty() && !parameters.is_empty();

        let reasoning = parsed
            .reasoning
            .unwrap_or_else(|| "LLM extraction completed".to_string());

        Ok(ExtractedParameters {
            template_id: template_id.to_string(),
            parameters,
            extra_parameters,
            missing_parameters,
            complete,
            reasoning,
            llm_calls_used: 1,
            llm_tokens_used: 0, // Will be estimated from response
        })
    }
}

#[async_trait]
impl ParameterExtractor for LlmParameterExtractor {
    async fn extract(
        &self,
        intent: &UserIntent,
        _budget: &LlmBudget,
        template_id: &str,
        parameter_names: &[String],
    ) -> Result<ExtractedParameters, PlanningError> {
        if parameter_names.is_empty() {
            return Ok(ExtractedParameters {
                template_id: template_id.to_string(),
                parameters: HashMap::new(),
                extra_parameters: HashMap::new(),
                missing_parameters: vec![],
                complete: true,
                reasoning: "No parameters to extract".to_string(),
                llm_calls_used: 0,
                llm_tokens_used: 0,
            });
        }

        let system_prompt = self.build_system_prompt(template_id, parameter_names);
        let user_message = self.build_user_message(intent);

        match self.config.provider {
            ExtractorProvider::Anthropic => {
                let body = serde_json::json!({
                    "model": self.config.model,
                    "max_tokens": self.config.max_tokens,
                    "temperature": self.config.temperature,
                    "system": system_prompt,
                    "messages": [
                        {"role": "user", "content": user_message}
                    ]
                });

                let body_bytes = serde_json::to_vec(&body).map_err(|e| {
                    PlanningError::ExtractionError {
                        detail: format!("Failed to serialize request: {}", e),
                    }
                })?;

                let response = self
                    .client
                    .post(&self.config.api_url)
                    .header("x-api-key", &self.api_key)
                    .header("anthropic-version", "2023-06-01")
                    .header("content-type", "application/json")
                    .body(body_bytes)
                    .send()
                    .await
                    .map_err(|e| PlanningError::ExtractionError {
                        detail: format!("API request failed: {}", e),
                    })?;

                let status = response.status();
                let body_text = response
                    .text()
                    .await
                    .map_err(|e| PlanningError::ExtractionError {
                        detail: format!("Failed to read response body: {}", e),
                    })?;

                if !status.is_success() {
                    return Err(PlanningError::ExtractionError {
                        detail: format!(
                            "API returned {}: {}",
                            status,
                            body_text.chars().take(200).collect::<String>()
                        ),
                    });
                }

                // Parse Anthropic response format
                #[derive(Deserialize)]
                struct AnthropicContent {
                    text: Option<String>,
                }

                #[derive(Deserialize)]
                struct AnthropicResponse {
                    content: Vec<AnthropicContent>,
                    usage: Option<AnthropicUsage>,
                }

                #[derive(Deserialize)]
                struct AnthropicUsage {
                    input_tokens: Option<u32>,
                    output_tokens: Option<u32>,
                }

                let parsed: AnthropicResponse =
                    serde_json::from_str(&body_text).map_err(|e| {
                        PlanningError::ExtractionError {
                            detail: format!("Failed to parse API response: {}", e),
                        }
                    })?;

                let content = parsed
                    .content
                    .first()
                    .and_then(|c| c.text.as_deref())
                    .unwrap_or("");

                let mut result = self.parse_response(content, template_id, parameter_names)?;

                // Fill in token usage
                if let Some(usage) = parsed.usage {
                    result.llm_tokens_used =
                        usage.input_tokens.unwrap_or(0) + usage.output_tokens.unwrap_or(0);
                }

                Ok(result)
            }

            ExtractorProvider::OpenAI => {
                let body = serde_json::json!({
                    "model": self.config.model,
                    "max_tokens": self.config.max_tokens,
                    "temperature": self.config.temperature,
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_message}
                    ]
                });

                let body_bytes = serde_json::to_vec(&body).map_err(|e| {
                    PlanningError::ExtractionError {
                        detail: format!("Failed to serialize request: {}", e),
                    }
                })?;

                let response = self
                    .client
                    .post(&self.config.api_url)
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .header("content-type", "application/json")
                    .body(body_bytes)
                    .send()
                    .await
                    .map_err(|e| PlanningError::ExtractionError {
                        detail: format!("API request failed: {}", e),
                    })?;

                let status = response.status();
                let body_text = response
                    .text()
                    .await
                    .map_err(|e| PlanningError::ExtractionError {
                        detail: format!("Failed to read response body: {}", e),
                    })?;

                if !status.is_success() {
                    return Err(PlanningError::ExtractionError {
                        detail: format!(
                            "API returned {}: {}",
                            status,
                            body_text.chars().take(200).collect::<String>()
                        ),
                    });
                }

                // Parse OpenAI response format
                #[derive(Deserialize)]
                struct OpenaiChoice {
                    message: OpenaiMessage,
                }

                #[derive(Deserialize)]
                struct OpenaiMessage {
                    content: Option<String>,
                }

                #[derive(Deserialize)]
                struct OpenaiResponse {
                    choices: Vec<OpenaiChoice>,
                    usage: Option<OpenaiUsage>,
                }

                #[derive(Deserialize)]
                struct OpenaiUsage {
                    prompt_tokens: Option<u32>,
                    completion_tokens: Option<u32>,
                }

                let parsed: OpenaiResponse =
                    serde_json::from_str(&body_text).map_err(|e| {
                        PlanningError::ExtractionError {
                            detail: format!("Failed to parse API response: {}", e),
                        }
                    })?;

                let content = parsed
                    .choices
                    .first()
                    .and_then(|c| c.message.content.as_deref())
                    .unwrap_or("");

                let mut result = self.parse_response(content, template_id, parameter_names)?;

                if let Some(usage) = parsed.usage {
                    result.llm_tokens_used =
                        usage.prompt_tokens.unwrap_or(0) + usage.completion_tokens.unwrap_or(0);
                }

                Ok(result)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response_valid() {
        let extractor = LlmParameterExtractor::new("test-key".into(), None);
        let response = r#"{"parameters": {"file_path": "src/main.rs", "project_root": "."}, "reasoning": "Extracted from intent"}"#;

        let result = extractor
            .parse_response(response, "test-template", &["file_path".into(), "project_root".into()])
            .unwrap();

        assert!(result.complete);
        assert_eq!(result.parameters.get("file_path").unwrap(), "src/main.rs");
        assert_eq!(result.parameters.get("project_root").unwrap(), ".");
        assert!(result.missing_parameters.is_empty());
    }

    #[test]
    fn test_parse_response_missing_param() {
        let extractor = LlmParameterExtractor::new("test-key".into(), None);
        let response = r#"{"parameters": {"file_path": "src/main.rs"}, "reasoning": "project_root not specified"}"#;

        let result = extractor
            .parse_response(response, "test-template", &["file_path".into(), "project_root".into()])
            .unwrap();

        assert!(!result.complete);
        assert_eq!(result.parameters.get("file_path").unwrap(), "src/main.rs");
        assert!(result.missing_parameters.contains(&"project_root".to_string()));
    }

    #[test]
    fn test_parse_response_empty_params() {
        let extractor = LlmParameterExtractor::new("test-key".into(), None);
        let response = r#"{"parameters": {}, "reasoning": "No parameters specified in intent"}"#;

        let result = extractor
            .parse_response(response, "test-template", &["file_path".into()])
            .unwrap();

        assert!(!result.complete);
        assert!(result.missing_parameters.contains(&"file_path".to_string()));
    }

    #[test]
    fn test_parse_response_markdown_fences() {
        let extractor = LlmParameterExtractor::new("test-key".into(), None);
        let response = "```json\n{\"parameters\": {\"path\": \"./src\"}, \"reasoning\": \"ok\"}\n```";

        let result = extractor
            .parse_response(response, "t", &["path".into()])
            .unwrap();

        assert!(result.complete);
        assert_eq!(result.parameters.get("path").unwrap(), "./src");
    }

    #[test]
    fn test_build_system_prompt_includes_params() {
        let extractor = LlmParameterExtractor::new("test-key".into(), None);
        let prompt = extractor.build_system_prompt("my-template", &["p1".into(), "p2".into()]);

        assert!(prompt.contains("my-template"));
        assert!(prompt.contains("p1"));
        assert!(prompt.contains("p2"));
    }

    #[test]
    fn test_build_system_prompt_empty_params() {
        let extractor = LlmParameterExtractor::new("test-key".into(), None);
        let prompt = extractor.build_system_prompt("t", &[]);

        assert!(prompt.contains("no parameters"));
    }
}

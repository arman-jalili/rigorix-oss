//! TemplateGenerator trait — fallback template generation from user intent.
//!
//! @canonical .pi/architecture/modules/template-generation.md#generator
//! Implements: Contract Freeze — TemplateGenerator trait, ClaudeTemplateGenerator,
//! GeneratorError, RepoContext, GeneratedTemplate
//! Issue: issue-contract-freeze
//!
//! TemplateGenerator is the fallback path in the planning pipeline.
//! When the Classifier finds no good match (confidence < 0.3 for all
//! templates), the pipeline falls back to the TemplateGenerator to
//! create a new template definition on-the-fly from the user intent.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

/// Generates a new template definition from user intent.
///
/// Used as a fallback when no existing template matches the user's
/// intent. The generator creates a complete TOML template definition
/// that can be parsed and registered for immediate use.
#[async_trait]
pub trait TemplateGenerator: Send + Sync {
    /// Generate a template definition from user intent.
    async fn generate(
        &self,
        intent: &crate::planning::domain::UserIntent,
        repo_context: &RepoContext,
        budget: &crate::budget_tracking::domain::LlmBudget,
    ) -> Result<GeneratedTemplate, GeneratorError>;

    /// Estimate the token cost of generating a template.
    fn estimate_cost(&self, intent: &crate::planning::domain::UserIntent) -> GeneratedTemplateCost;
}

/// A template generated on-the-fly by the TemplateGenerator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTemplate {
    /// The TOML string of the generated template definition.
    pub toml_content: String,
    /// Suggested template ID.
    pub suggested_id: String,
    /// Suggested human-readable name.
    pub suggested_name: String,
    /// Brief description of what this template does.
    pub description: String,
    /// Number of LLM calls used.
    pub llm_calls_used: u32,
    /// Number of LLM tokens consumed.
    pub llm_tokens_used: u32,
}

/// Estimated cost of generating a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTemplateCost {
    /// Estimated number of LLM calls.
    pub estimated_calls: u32,
    /// Estimated number of LLM tokens.
    pub estimated_tokens: u32,
}

// ---------------------------------------------------------------------------
// RepoContext — Repository snapshot for generation context
// ---------------------------------------------------------------------------

/// Snapshot of repository structure used as context for template generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoContext {
    /// Working directory being operated on.
    pub root_dir: PathBuf,
    /// Detected project type.
    pub project_type: String,
    /// Flat list of relevant file paths.
    pub directory_tree: Vec<String>,
    /// External dependencies.
    pub dependencies: Vec<String>,
    /// Public type, function, and trait names.
    pub public_api: Vec<String>,
    /// Optional symbol graph subset for Phase 3 validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_graph_snapshot: Option<serde_json::Value>,
}

impl RepoContext {
    /// Create a new empty RepoContext for a given directory.
    pub fn new(root_dir: PathBuf, project_type: String) -> Self {
        Self {
            root_dir,
            project_type,
            directory_tree: Vec::new(),
            dependencies: Vec::new(),
            public_api: Vec::new(),
            symbol_graph_snapshot: None,
        }
    }

    /// Check if this context has any file entries.
    pub fn has_files(&self) -> bool {
        !self.directory_tree.is_empty()
    }

    /// Check if this context has any public API entries.
    pub fn has_public_api(&self) -> bool {
        !self.public_api.is_empty()
    }
}

// ---------------------------------------------------------------------------
// GeneratorError — Typed error enum for generation failures
// ---------------------------------------------------------------------------

/// Errors specific to the template generation process.
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum GeneratorError {
    /// The LLM returned content that is not valid TOML.
    InvalidToml {
        raw_response: String,
        parse_error: String,
        attempt: u8,
    },
    /// The generated template failed structural validation.
    ValidationFailed {
        template_id: String,
        errors: Vec<String>,
        attempt: u8,
    },
    /// Phase 3: Generated template references symbols that don't exist.
    SymbolValidation {
        template_id: String,
        invalid_references: Vec<InvalidSymbolReference>,
        attempt: u8,
    },
    /// The LLM budget was exhausted before generation completed.
    BudgetExhausted { calls_used: u32, max_calls: u32 },
    /// The LLM API call failed.
    ApiError {
        detail: String,
        status_code: Option<u16>,
        retry_after: Option<u64>,
    },
    /// Maximum retry attempts exhausted.
    MaxRetriesExhausted { attempts: u8, errors: Vec<String> },
    /// The repository context could not be built.
    ContextBuildFailed { detail: String },
}

/// An invalid symbol reference found during Phase 3 validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvalidSymbolReference {
    pub symbol: String,
    pub usage: String,
    pub reason: String,
    pub is_any_type: bool,
}

impl fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeneratorError::InvalidToml {
                raw_response,
                parse_error,
                attempt,
            } => write!(
                f,
                "Invalid TOML (attempt {}): {} - response: {}...",
                attempt,
                parse_error,
                &raw_response[..raw_response.len().min(100)]
            ),
            GeneratorError::ValidationFailed {
                template_id,
                errors,
                attempt,
            } => write!(
                f,
                "Validation failed for '{}' (attempt {}): {}",
                template_id,
                attempt,
                errors.join("; ")
            ),
            GeneratorError::SymbolValidation {
                template_id,
                invalid_references,
                attempt,
            } => write!(
                f,
                "Symbol validation failed for '{}' (attempt {}): {} invalid references",
                template_id,
                attempt,
                invalid_references.len()
            ),
            GeneratorError::BudgetExhausted {
                calls_used,
                max_calls,
            } => write!(
                f,
                "Budget exhausted: used {}/{} calls",
                calls_used, max_calls
            ),
            GeneratorError::ApiError {
                detail,
                status_code,
                retry_after,
            } => write!(
                f,
                "API error (status: {:?}, retry_after: {:?}): {}",
                status_code, retry_after, detail
            ),
            GeneratorError::MaxRetriesExhausted { attempts, errors } => write!(
                f,
                "Max retries exhausted after {} attempts: {}",
                attempts,
                errors.join("; ")
            ),
            GeneratorError::ContextBuildFailed { detail } => {
                write!(f, "Context build failed: {}", detail)
            }
        }
    }
}

impl GeneratorError {
    /// Returns `true` if this error is transient and the operation may succeed on retry.
    pub fn is_retriable(&self) -> bool {
        matches!(self, GeneratorError::ApiError { .. })
    }
}

// ---------------------------------------------------------------------------
// ClaudeTemplateGenerator — Anthropic Messages API Implementation
// ---------------------------------------------------------------------------

/// Configuration for the Claude template generator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeGeneratorConfig {
    pub api_url: String,
    pub model: String,
    pub max_tokens: u32,
    pub timeout_secs: u64,
    pub temperature: f64,
    pub max_retries: u8,
}

impl Default for ClaudeGeneratorConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.anthropic.com/v1/messages".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 4096,
            timeout_secs: 120,
            temperature: 0.3,
            max_retries: 3,
        }
    }
}

/// Production template generator using Anthropic's Claude Messages API.
pub struct ClaudeTemplateGenerator {
    api_key: String,
    config: ClaudeGeneratorConfig,
    client: reqwest::Client,
}

impl ClaudeTemplateGenerator {
    /// Create a new ClaudeTemplateGenerator.
    pub fn new(api_key: String, config: Option<ClaudeGeneratorConfig>) -> Self {
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

    /// Build the system prompt for template generation.
    fn build_system_prompt(&self, ctx: &RepoContext) -> String {
        let public_api_list = if ctx.public_api.is_empty() {
            "(none available)".to_string()
        } else {
            ctx.public_api.join(", ")
        };
        let dependencies_list = if ctx.dependencies.is_empty() {
            "(none)".to_string()
        } else {
            ctx.dependencies.join(", ")
        };
        let file_tree = if ctx.directory_tree.is_empty() {
            "(no files scanned)".to_string()
        } else {
            ctx.directory_tree.join("\n")
        };
        format!(
            r##"You are a template generator for the Rigorix workflow engine. Your task is to
create a valid TOML template definition that matches the user's intent.

## Repository Context

**Project type:** {project_type}

**File tree:**
{file_tree}

**Existing dependencies:** {dependencies_list}

**PUBLIC API SURFACE (only use these):**
{public_api_list}

**IMPORTANT CONSTRAINTS:**
- DO NOT invent type names, function names, or field names that are not in the PUBLIC API SURFACE above
- Only use the action types: file_read, file_write, file_append, file_patch, run_command, lsp_query, git_read, git_stage, git_commit
- Every action must have all required fields
- Use snake_case for all IDs and field names
- DO NOT wrap your response in markdown code fences - output raw TOML only
- The template must include an id, name, description, version, parameters section, and at least one node
- Include appropriate parameters for any placeholder values used in node actions
- Add meaningful retry configuration for nodes that may fail transiently

## Template Schema Reference

```toml
id = "kebab-case-id"
name = "Human Readable Name"
description = "What this template does"
version = "1.0.0"

[[parameters]]
name = "param_name"
description = "What this parameter is for"
required = true
param_type = "path"

[[parameters]]
name = "optional_param"
description = "Optional setting"
required = false
param_type = "string"
default = "default_value"

[[nodes]]
id = "step-1"
name = "Step one"
depends_on = []
[nodes.action]
type = "file_read"
path = "{{ param_name }}"

[[nodes]]
id = "step-2"
name = "Step two"
depends_on = ["step-1"]
[nodes.action]
type = "git_commit"
message = "{{ commit_message }}"
auto_stage = true
[nodes.retry]
max_retries = 3
retry_on = ["transient"]
strategy = "same_operation"
backoff_ms = 1000

[[nodes]]
id = "step-3"
name = "Validate result"
depends_on = ["step-2"]
[nodes.action]
type = "run_command"
command = "cargo build"
timeout_secs = 120
```

Now generate a template for the following user intent."##,
            project_type = ctx.project_type,
            file_tree = file_tree,
            dependencies_list = dependencies_list,
            public_api_list = public_api_list,
        )
    }

    /// Build the user message for template generation.
    fn build_user_message(&self, intent: &crate::planning::domain::UserIntent) -> String {
        let mut msg = format!("## User Intent\n\n{}", intent.input);
        if intent.has_clarifications() {
            msg.push_str("\n\n## Clarification History\n");
            for pair in &intent.clarifications {
                msg.push_str(&format!("\n- Q: {}\n- A: {}", pair.question, pair.answer));
            }
        }
        msg.push_str("\n\n## Response Format\n");
        msg.push_str("Output ONLY valid TOML. No markdown fences. No explanations.");
        msg
    }

    /// Strip markdown code fences from the LLM response.
    pub(crate) fn strip_code_fences(response: &str) -> String {
        let trimmed = response.trim();
        let first_fence = trimmed.find("```");
        let content_after_open = match first_fence {
            Some(open_pos) => {
                let after_fence = &trimmed[open_pos + 3..];
                if let Some(newline) = after_fence.find('\n') {
                    &after_fence[newline + 1..]
                } else {
                    trimmed
                }
            }
            None => trimmed,
        };
        if content_after_open.is_empty() && first_fence.is_some() {
            return trimmed
                .trim_end_matches("```")
                .trim_end_matches('`')
                .trim()
                .to_string();
        }
        if let Some(close_pos) = content_after_open.rfind("```") {
            content_after_open[..close_pos].trim().to_string()
        } else {
            content_after_open.trim().to_string()
        }
    }

    /// Parse the Anthropic API response and extract the text content.
    fn parse_api_response(response_text: &str) -> Result<String, GeneratorError> {
        #[derive(Deserialize)]
        struct AnthropicMessage {
            content: Vec<AnthropicContent>,
        }
        #[derive(Deserialize)]
        struct AnthropicContent {
            #[serde(rename = "type")]
            content_type: String,
            text: Option<String>,
        }
        let message: AnthropicMessage =
            serde_json::from_str(response_text).map_err(|e| GeneratorError::ApiError {
                detail: format!("Failed to parse Claude API response: {}", e),
                status_code: None,
                retry_after: None,
            })?;
        let text = message
            .content
            .into_iter()
            .find(|c| c.content_type == "text")
            .and_then(|c| c.text)
            .ok_or_else(|| GeneratorError::ApiError {
                detail: "Claude response has no text content block".to_string(),
                status_code: None,
                retry_after: None,
            })?;
        Ok(text)
    }

    /// Extract Retry-After header value from response headers.
    fn extract_retry_after(response: &reqwest::Response) -> Option<u64> {
        response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
    }
}

#[async_trait]
impl TemplateGenerator for ClaudeTemplateGenerator {
    async fn generate(
        &self,
        intent: &crate::planning::domain::UserIntent,
        repo_context: &RepoContext,
        _budget: &crate::budget_tracking::domain::LlmBudget,
    ) -> Result<GeneratedTemplate, GeneratorError> {
        let max_retries = self.config.max_retries;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            let system_prompt = self.build_system_prompt(repo_context);
            let user_message = self.build_user_message(intent);

            let message_content = if attempt > 0 {
                format!(
                    "{}\n\n## Previous Attempt Failed\n\n{}",
                    user_message, last_error
                )
            } else {
                user_message.clone()
            };

            let body = serde_json::json!({
                "model": self.config.model,
                "max_tokens": self.config.max_tokens,
                "temperature": self.config.temperature,
                "system": system_prompt,
                "messages": [{"role": "user", "content": message_content}]
            });

            let body_bytes = serde_json::to_vec(&body).map_err(|e| GeneratorError::ApiError {
                detail: format!("Failed to serialize request: {}", e),
                status_code: None,
                retry_after: None,
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
                .map_err(|e| GeneratorError::ApiError {
                    detail: format!("HTTP request failed: {}", e),
                    status_code: None,
                    retry_after: None,
                })?;

            let status = response.status();
            let retry_after = Self::extract_retry_after(&response);

            if !status.is_success() {
                let response_text = response.text().await.unwrap_or_default();
                if status.as_u16() == 429 || status.as_u16() >= 500 {
                    last_error = format!(
                        "API returned status {}: {}",
                        status.as_u16(),
                        response_text.chars().take(200).collect::<String>()
                    );
                    if let Some(seconds) = retry_after {
                        tokio::time::sleep(Duration::from_secs(seconds)).await;
                    }
                    continue;
                }
                return Err(GeneratorError::ApiError {
                    detail: format!(
                        "API returned status {}: {}",
                        status.as_u16(),
                        response_text.chars().take(200).collect::<String>()
                    ),
                    status_code: Some(status.as_u16()),
                    retry_after,
                });
            }

            let response_text = response
                .text()
                .await
                .map_err(|e| GeneratorError::ApiError {
                    detail: format!("Failed to read response body: {}", e),
                    status_code: None,
                    retry_after: None,
                })?;

            let raw_toml = Self::parse_api_response(&response_text)?;
            let toml_content = Self::strip_code_fences(&raw_toml);

            let template_result: Result<crate::templates::domain::Template, _> =
                toml::from_str(&toml_content);

            match template_result {
                Ok(template) => {
                    return Ok(GeneratedTemplate {
                        toml_content,
                        suggested_id: template.id.clone(),
                        suggested_name: template.name.clone(),
                        description: template.description.clone(),
                        llm_calls_used: attempt as u32 + 1,
                        llm_tokens_used: 0,
                    });
                }
                Err(e) => {
                    last_error = format!("TOML parse error: {}", e);
                }
            }
        }

        Err(GeneratorError::MaxRetriesExhausted {
            attempts: max_retries,
            errors: vec![last_error],
        })
    }

    fn estimate_cost(
        &self,
        _intent: &crate::planning::domain::UserIntent,
    ) -> GeneratedTemplateCost {
        GeneratedTemplateCost {
            estimated_calls: self.config.max_retries as u32,
            estimated_tokens: self.config.max_tokens,
        }
    }
}

// ── OpenAI-compatible TemplateGenerator ─────────────────────────────────

/// Template generator for OpenAI-compatible APIs (OpenAI, DeepSeek, local).
/// Sends requests using the OpenAI chat completions format:
/// POST /v1/chat/completions with Authorization: Bearer header.
pub struct OpenaiTemplateGenerator {
    api_key: String,
    config: ClaudeGeneratorConfig,
    client: reqwest::Client,
}

impl OpenaiTemplateGenerator {
    pub fn new(api_key: String, config: Option<ClaudeGeneratorConfig>) -> Self {
        let config = config.unwrap_or_default();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");
        Self { api_key, config, client }
    }

    fn build_system_prompt(&self, ctx: &RepoContext) -> String {
        ClaudeTemplateGenerator::new("".into(), None).build_system_prompt(ctx)
    }

    fn build_user_message(&self, intent: &crate::planning::domain::UserIntent) -> String {
        ClaudeTemplateGenerator::new("".into(), None).build_user_message(intent)
    }

    fn strip_code_fences(response: &str) -> String {
        ClaudeTemplateGenerator::strip_code_fences(response)
    }

    /// Parse the OpenAI API response and extract the text content.
    fn parse_api_response(response_text: &str) -> Result<String, GeneratorError> {
        #[derive(Deserialize)]
        struct OpenaiResponse {
            choices: Vec<OpenaiChoice>,
        }
        #[derive(Deserialize)]
        struct OpenaiChoice {
            message: OpenaiMessage,
        }
        #[derive(Deserialize)]
        struct OpenaiMessage {
            content: Option<String>,
        }
        let resp: OpenaiResponse =
            serde_json::from_str(response_text).map_err(|e| GeneratorError::ApiError {
                detail: format!("Failed to parse OpenAI API response: {}", e),
                status_code: None,
                retry_after: None,
            })?;
        resp.choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| GeneratorError::ApiError {
                detail: "OpenAI response has no content".to_string(),
                status_code: None,
                retry_after: None,
            })
    }

    fn extract_retry_after(response: &reqwest::Response) -> Option<u64> {
        response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
    }
}

#[async_trait]
impl TemplateGenerator for OpenaiTemplateGenerator {
    async fn generate(
        &self,
        intent: &crate::planning::domain::UserIntent,
        repo_context: &RepoContext,
        _budget: &crate::budget_tracking::domain::LlmBudget,
    ) -> Result<GeneratedTemplate, GeneratorError> {
        let max_retries = self.config.max_retries;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            let system_prompt = self.build_system_prompt(repo_context);
            let user_message = self.build_user_message(intent);

            let message_content = if attempt > 0 {
                format!( "{}\n\n## Previous Attempt Failed\n\n{}", user_message, last_error )
            } else {
                user_message.clone()
            };

            // OpenAI format: system is a message with role "system"
            let body = serde_json::json!({
                "model": self.config.model,
                "max_tokens": self.config.max_tokens,
                "temperature": self.config.temperature,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": message_content}
                ]
            });

            let body_bytes = serde_json::to_vec(&body).map_err(|e| GeneratorError::ApiError {
                detail: format!("Failed to serialize request: {}", e),
                status_code: None, retry_after: None,
            })?;

            let response = self
                .client
                .post(&self.config.api_url)
                .header("Authorization", format!("Bearer {}", &self.api_key))
                .header("content-type", "application/json")
                .body(body_bytes)
                .send()
                .await
                .map_err(|e| GeneratorError::ApiError {
                    detail: format!("HTTP request failed: {}", e),
                    status_code: None, retry_after: None,
                })?;

            let status = response.status();
            let retry_after = Self::extract_retry_after(&response);

            if !status.is_success() {
                let response_text = response.text().await.unwrap_or_default();
                if status.as_u16() == 429 || status.as_u16() >= 500 {
                    last_error = format!(
                        "API returned status {}: {}",
                        status.as_u16(),
                        response_text.chars().take(200).collect::<String>()
                    );
                    if let Some(seconds) = retry_after {
                        tokio::time::sleep(Duration::from_secs(seconds)).await;
                    }
                    continue;
                }
                return Err(GeneratorError::ApiError {
                    detail: format!(
                        "API returned status {}: {}",
                        status.as_u16(),
                        response_text.chars().take(200).collect::<String>()
                    ),
                    status_code: Some(status.as_u16()),
                    retry_after,
                });
            }

            let response_text = response.text().await.map_err(|e| GeneratorError::ApiError {
                detail: format!("Failed to read response body: {}", e),
                status_code: None, retry_after: None,
            })?;

            let raw_toml = Self::parse_api_response(&response_text)?;
            let toml_content = Self::strip_code_fences(&raw_toml);

            let template_result: Result<crate::templates::domain::Template, _> =
                toml::from_str(&toml_content);

            match template_result {
                Ok(template) => {
                    return Ok(GeneratedTemplate {
                        toml_content,
                        suggested_id: template.id.clone(),
                        suggested_name: template.name.clone(),
                        description: template.description.clone(),
                        llm_calls_used: attempt as u32 + 1,
                        llm_tokens_used: 0,
                    });
                }
                Err(e) => {
                    last_error = format!("TOML parse error: {}", e);
                }
            }
        }

        Err(GeneratorError::MaxRetriesExhausted {
            attempts: max_retries,
            errors: vec![last_error],
        })
    }

    fn estimate_cost(&self, _intent: &crate::planning::domain::UserIntent) -> GeneratedTemplateCost {
        GeneratedTemplateCost {
            estimated_calls: self.config.max_retries as u32,
            estimated_tokens: self.config.max_tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_code_fences_no_fences() {
        let input = "simple toml content";
        assert_eq!(
            ClaudeTemplateGenerator::strip_code_fences(input),
            "simple toml content"
        );
    }

    #[test]
    fn test_strip_code_fences_with_language() {
        let input = "```toml\nname = \"test\"\n```";
        assert_eq!(
            ClaudeTemplateGenerator::strip_code_fences(input),
            "name = \"test\""
        );
    }

    #[test]
    fn test_strip_code_fences_no_language() {
        let input = "```\nname = \"test\"\n```";
        assert_eq!(
            ClaudeTemplateGenerator::strip_code_fences(input),
            "name = \"test\""
        );
    }

    #[test]
    fn test_strip_code_fences_trailing_content() {
        let input = "```toml\nname = \"test\"\n```\nsome trailing text";
        assert_eq!(
            ClaudeTemplateGenerator::strip_code_fences(input),
            "name = \"test\""
        );
    }

    #[test]
    fn test_strip_code_fences_only_closing() {
        let input = "name = \"test\"\n```";
        assert_eq!(
            ClaudeTemplateGenerator::strip_code_fences(input),
            "name = \"test\""
        );
    }

    #[test]
    fn test_strip_code_fences_whitespace() {
        let input = "\n  ```toml\n  name = \"test\"\n  ```  \n";
        assert_eq!(
            ClaudeTemplateGenerator::strip_code_fences(input),
            "name = \"test\""
        );
    }

    #[test]
    fn test_generator_error_display_invalid_toml() {
        let err = GeneratorError::InvalidToml {
            raw_response: "{{{bad toml".to_string(),
            parse_error: "expected a value".to_string(),
            attempt: 0,
        };
        let display = format!("{}", err);
        assert!(display.contains("Invalid TOML"));
    }

    #[test]
    fn test_generator_error_display_budget_exhausted() {
        let err = GeneratorError::BudgetExhausted {
            calls_used: 5,
            max_calls: 3,
        };
        let display = format!("{}", err);
        assert!(display.contains("Budget exhausted"));
    }

    #[test]
    fn test_repo_context_default() {
        let ctx = RepoContext::new(PathBuf::from("/test"), "rust".to_string());
        assert_eq!(ctx.project_type, "rust");
        assert!(!ctx.has_files());
    }

    #[test]
    fn test_claude_config_defaults() {
        let config = ClaudeGeneratorConfig::default();
        assert_eq!(config.model, "claude-sonnet-4-20250514");
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_generated_template_serde() {
        let t = GeneratedTemplate {
            toml_content: "id = \"test\"".to_string(),
            suggested_id: "test".to_string(),
            suggested_name: "Test".to_string(),
            description: "A test".to_string(),
            llm_calls_used: 1,
            llm_tokens_used: 100,
        };
        let json = serde_json::to_string(&t).unwrap();
        let deserialized: GeneratedTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.suggested_id, "test");
    }

    // -- ClaudeTemplateGenerator unit tests (L-04) --

    #[test]
    fn test_strip_code_fences_with_markdown_json() {
        let input = "```json\n{\"name\": \"test\"}\n```";
        let result = ClaudeTemplateGenerator::strip_code_fences(input);
        assert_eq!(result, "{\"name\": \"test\"}");
    }

    #[test]
    fn test_parse_api_response_valid_anthropic_format() {
        let input = r#"{"content": [{"type": "text", "text": "template: {\"name\": \"test\"}"}]}"#;
        let result = ClaudeTemplateGenerator::parse_api_response(input);
        assert!(result.is_ok(), "Should parse valid Anthropic response");
        assert!(result.unwrap().contains("template:"));
    }

    #[test]
    fn test_parse_api_response_invalid_json() {
        let result = ClaudeTemplateGenerator::parse_api_response("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_api_response_missing_content() {
        let input = r#"{"content": []}"#;
        let result = ClaudeTemplateGenerator::parse_api_response(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_system_prompt_contains_context() {
        let config = ClaudeGeneratorConfig::default();
        let generator = ClaudeTemplateGenerator::new("test-key".to_string(), Some(config));
        let ctx = RepoContext {
            root_dir: std::path::PathBuf::from("/test"),
            project_type: "rust".to_string(),
            directory_tree: vec!["src/".to_string()],
            dependencies: vec!["tokio".to_string()],
            public_api: vec!["pub fn run()".to_string()],
            symbol_graph_snapshot: None,
        };
        let prompt = generator.build_system_prompt(&ctx);
        assert!(prompt.contains("rust"), "Prompt should mention project type");
        assert!(prompt.contains("tokio"), "Prompt should mention dependencies");
        assert!(
            prompt.contains("pub fn run()"),
            "Prompt should mention public API"
        );
    }

    #[test]
    fn test_build_user_message_contains_intent() {
        let config = ClaudeGeneratorConfig::default();
        let generator = ClaudeTemplateGenerator::new("test-key".to_string(), Some(config));
        let intent = crate::planning::domain::intent::UserIntent::new(
            "read the file".to_string(),
            None,
        );
        let message = generator.build_user_message(&intent);
        assert!(message.contains("read the file"));
    }
}

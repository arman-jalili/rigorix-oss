//! TemplateGenerator trait — fallback template generation from user intent.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#generator
//! Implements: Contract Freeze — TemplateGenerator trait
//! Issue: issue-contract-freeze
//!
//! TemplateGenerator is the fallback path in the planning pipeline.
//! When the Classifier finds no good match (confidence < 0.3 for all
//! templates), the pipeline falls back to the TemplateGenerator to
//! create a new template definition on-the-fly from the user intent.
//!
//! # Contract (Frozen)
//! - Generates a TOML template string from user intent
//! - The generated template must be parseable by TemplateParserService
//! - The generated template is registered in the TemplateEngine before re-classifying
//! - Implementations must be deterministic (same intent → same template structure)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

use crate::budget_tracking::domain::LlmBudget;
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::intent::UserIntent;

/// Generates a new template definition from user intent.
///
/// Used as a fallback when no existing template matches the user's
/// intent. The generator creates a complete TOML template definition
/// that can be parsed and registered for immediate use.
///
/// # Contract (Frozen)
/// - `generate` returns a TOML string matching the template schema
/// - The output must be parseable by `TemplateParserService::parse_str`
/// - Budget is consumed via `LlmBudget` reservation
/// - Implementations should include appropriate node structure and parameters
#[async_trait]
pub trait TemplateGenerator: Send + Sync {
    /// Generate a template definition from user intent.
    ///
    /// Creates a TOML template string that the TemplateEngine can
    /// parse and register. The generated template should match the
    /// user's intent as closely as possible.
    ///
    /// # Arguments
    ///
    /// * `intent` — The user's raw intent (with optional clarifications).
    /// * `repo_context` — Repository snapshot with file tree, public API,
    ///   dependencies, and optional symbol graph for validation.
    /// * `budget` — LLM budget for tracking generation cost.
    ///
    /// # Returns
    ///
    /// A `GeneratedTemplate` containing the TOML string and metadata.
    async fn generate(
        &self,
        intent: &UserIntent,
        repo_context: &RepoContext,
        budget: &LlmBudget,
    ) -> Result<GeneratedTemplate, GeneratorError>;

    /// Estimate the token cost of generating a template.
    ///
    /// Provides a rough estimate for budget pre-checking before
    /// the actual generation call.
    fn estimate_cost(&self, intent: &UserIntent) -> GeneratedTemplateCost;
}

/// A template generated on-the-fly by the TemplateGenerator.
///
/// Carries the TOML content, metadata, and any validation/symbol
/// validation results for the generated template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTemplate {
    /// The TOML string of the generated template definition.
    pub toml_content: String,

    /// Suggested template ID (used for registration).
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
///
/// Provides the LLM with knowledge of the codebase structure,
/// public API surface, and existing dependencies to prevent
/// hallucinated types, fields, or method references.
///
/// # Contract (Frozen)
/// - `directory_tree` is a flat or nested listing of relevant files
/// - `public_api` lists public types, functions, and traits
/// - `dependencies` lists external crate/package references
/// - `symbol_graph_snapshot` is an optional subset of the indexed symbol graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoContext {
    /// Working directory being operated on.
    pub root_dir: PathBuf,

    /// Detected project type (e.g. "rust", "python", "typescript").
    pub project_type: String,

    /// Flat list of relevant file paths (relative to root_dir).
    pub directory_tree: Vec<String>,

    /// External dependencies (crate names, packages, etc.).
    pub dependencies: Vec<String>,

    /// Public type, function, and trait names available in the codebase.
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
///
/// Separate from `PlanningError` because generation has distinct
/// failure modes (TOML parse, symbol validation, budget, LLM API)
/// that don't apply to the broader planning pipeline.
///
/// # Contract (Frozen)
/// - Every failure mode has a dedicated variant with structured context
/// - Errors carry enough information for meaningful retry feedback to the LLM
/// - Implements `std::error::Error` for library compatibility
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum GeneratorError {
    /// The LLM returned content that is not valid TOML.
    #[serde(rename = "invalid_toml")]
    InvalidToml {
        /// The raw LLM response that failed to parse.
        raw_response: String,
        /// The TOML parser error message.
        parse_error: String,
        /// Retry attempt number (0-based).
        attempt: u8,
    },

    /// The generated template failed structural validation.
    #[serde(rename = "validation_failed")]
    ValidationFailed {
        /// Template ID that failed validation.
        template_id: String,
        /// Validation error messages.
        errors: Vec<String>,
        /// Retry attempt number.
        attempt: u8,
    },

    /// Phase 3: Generated template references symbols that don't exist.
    #[serde(rename = "symbol_validation")]
    SymbolValidation {
        /// Template ID being validated.
        template_id: String,
        /// List of invalid symbol references found.
        invalid_references: Vec<InvalidSymbolReference>,
        /// Retry attempt number.
        attempt: u8,
    },

    /// The LLM budget was exhausted before generation completed.
    #[serde(rename = "budget_exhausted")]
    BudgetExhausted {
        /// Number of LLM calls consumed.
        calls_used: u32,
        /// Maximum allowed calls.
        max_calls: u32,
    },

    /// The LLM API call failed (network, auth, rate limit).
    #[serde(rename = "api_error")]
    ApiError {
        /// Human-readable error detail.
        detail: String,
        /// HTTP status code (if applicable).
        status_code: Option<u16>,
        /// Retry-after seconds (if rate limited).
        retry_after: Option<u64>,
    },

    /// Maximum retry attempts exhausted without generating a valid template.
    #[serde(rename = "max_retries_exhausted")]
    MaxRetriesExhausted {
        /// Number of attempts made.
        attempts: u8,
        /// Errors from each attempt.
        errors: Vec<String>,
    },

    /// The repository context could not be built.
    #[serde(rename = "context_build_failed")]
    ContextBuildFailed {
        /// Details about the failure.
        detail: String,
    },
}

/// An invalid symbol reference found during Phase 3 validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvalidSymbolReference {
    /// The symbol name that was referenced (e.g. "MyStruct", "some_field").
    pub symbol: String,

    /// How the symbol was used in the template (e.g. "type", "field_access").
    pub usage: String,

    /// The specific reason this reference is invalid.
    pub reason: String,

    /// Whether this reference uses `any` type (LLM escape hatch).
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
            GeneratorError::MaxRetriesExhausted { attempts, errors } => {
                write!(
                    f,
                    "Max retries exhausted after {} attempts: {}",
                    attempts,
                    errors.join("; ")
                )
            }
            GeneratorError::ContextBuildFailed { detail } => {
                write!(f, "Context build failed: {}", detail)
            }
        }
    }
}

impl From<GeneratorError> for PlanningError {
    fn from(err: GeneratorError) -> Self {
        match err {
            GeneratorError::BudgetExhausted {
                calls_used,
                max_calls,
            } => PlanningError::BudgetExhausted {
                used_calls: calls_used,
                max_calls,
                used_tokens: 0,
                max_tokens: 0,
            },
            _ => PlanningError::TemplateEngineError {
                detail: err.to_string(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// ClaudeTemplateGenerator — Anthropic Messages API Implementation
// ---------------------------------------------------------------------------

/// Configuration for the Claude template generator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeGeneratorConfig {
    /// The Anthropic API endpoint (default: https://api.anthropic.com/v1/messages).
    pub api_url: String,

    /// Claude model to use (default: claude-sonnet-4-20250514).
    pub model: String,

    /// Maximum tokens in the response.
    pub max_tokens: u32,

    /// Request timeout in seconds.
    pub timeout_secs: u64,

    /// Temperature for generation (higher = more creative).
    pub temperature: f64,

    /// Maximum number of retry attempts on parse/validation failure.
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
///
/// Communicates with the Claude API to generate a new TOML template
/// definition from user intent. Supports:
/// - Structured prompt engineering with template schema and repo context
/// - Up to 3 retry attempts on TOML parse/validation errors
/// - Markdown code fence stripping from LLM response
/// - Rate limit handling with Retry-After header support
/// - PUBLIC API SURFACE and EXISTING DEPENDENCIES constraints
///
/// # Prompt Structure
///
/// The generator builds a structured prompt that includes:
/// - Template schema documentation (valid action types, parameter types, etc.)
/// - Repository context (file tree, public API, dependencies)
/// - Existing template IDs (to avoid naming conflicts)
/// - The user intent with clarification history
///
/// # Security
///
/// - API key is provided at construction time, not hardcoded
/// - Structured prompts prevent prompt injection
/// - Token limits are respected to prevent budget overruns
/// - PUBLIC API SURFACE constraint prevents hallucinated type references
pub struct ClaudeTemplateGenerator {
    /// API key for authentication.
    api_key: String,

    /// Configuration for the generator.
    config: ClaudeGeneratorConfig,

    /// HTTP client for API calls.
    client: reqwest::Client,
}

impl ClaudeTemplateGenerator {
    /// Create a new ClaudeTemplateGenerator.
    ///
    /// # Arguments
    ///
    /// * `api_key` — Anthropic API key (from environment or secret store).
    /// * `config` — Optional configuration overrides (defaults used if None).
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
    fn build_user_message(&self, intent: &UserIntent) -> String {
        let mut msg = format!(
            "## User Intent\n\n{}",
            intent.input
        );

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
    ///
    /// Handles:
    /// - ```toml ... ``` (language identifier)
    /// - ``` ... ``` (no identifier)
    /// - Only closing ``` (content is preserved)
    /// - No fences (content returned as-is)
    /// - Whitespace and trailing content
    pub(crate) fn strip_code_fences(response: &str) -> String {
        let trimmed = response.trim();

        // Find the first opening fence
        let first_fence = trimmed.find("```");
        let content_after_open = match first_fence {
            Some(open_pos) => {
                let after_fence = &trimmed[open_pos + 3..];
                // Skip past the language identifier line (if any)
                if let Some(newline) = after_fence.find('\n') {
                    &after_fence[newline + 1..]
                } else {
                    // No content after opening fence - might be a closing fence that
                    // was detected as opening; treat as no opening fence
                    trimmed
                }
            }
            None => trimmed,
        };

        // If the opening fence detection consumed everything, check if we actually
        // had a closing-only fence scenario
        if content_after_open.is_empty() && first_fence.is_some() {
            // The only ``` was treated as opening but had no content after it.
            // This could be a closing fence without an opening fence, or no fence at all.
            return trimmed
                .trim_end_matches("```")
                .trim_end_matches(|c: char| c == '`')
                .trim()
                .to_string();
        }

        // Find the LAST closing fence and remove everything from it onward
        if let Some(close_pos) = content_after_open.rfind("```") {
            let content = content_after_open[..close_pos].trim();
            content.to_string()
        } else {
            content_after_open.trim().to_string()
        }
    }

    /// Parse the Anthropic Messages API response and extract the text content.
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

        let message: AnthropicMessage = serde_json::from_str(response_text).map_err(|e| {
            GeneratorError::ApiError {
                detail: format!("Failed to parse Claude API response: {}", e),
                status_code: None,
                retry_after: None,
            }
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
        intent: &UserIntent,
        repo_context: &RepoContext,
        _budget: &LlmBudget,
    ) -> Result<GeneratedTemplate, GeneratorError> {
        let max_retries = self.config.max_retries;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            // Build the prompt
            let system_prompt = self.build_system_prompt(repo_context);
            let user_message = self.build_user_message(intent);

            // Add retry feedback after the first attempt
            let message_content = if attempt > 0 {
                format!(
                    "{}\n\n## Previous Attempt Failed\n\n{}",
                    user_message, last_error
                )
            } else {
                user_message.clone()
            };

            // Build the request body
            let body = serde_json::json!({
                "model": self.config.model,
                "max_tokens": self.config.max_tokens,
                "temperature": self.config.temperature,
                "system": system_prompt,
                "messages": [
                    {"role": "user", "content": message_content}
                ]
            });

            let body_bytes = serde_json::to_vec(&body).map_err(|e| {
                GeneratorError::ApiError {
                    detail: format!("Failed to serialize request: {}", e),
                    status_code: None,
                    retry_after: None,
                }
            })?;

            // Make the API call
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
                    // Retryable error
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

            let response_text = response.text().await.map_err(|e| {
                GeneratorError::ApiError {
                    detail: format!("Failed to read response body: {}", e),
                    status_code: None,
                    retry_after: None,
                }
            })?;

            // Parse the API response and extract TOML
            let raw_toml = Self::parse_api_response(&response_text)?;
            let toml_content = Self::strip_code_fences(&raw_toml);

            // Quick validation: try to parse as TOML to check basic validity
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
                    last_error = format!(
                        "TOML parse error: {}",
                        e
                    );
                }
            }
        }

        Err(GeneratorError::MaxRetriesExhausted {
            attempts: max_retries,
            errors: vec![last_error],
        })
    }

    fn estimate_cost(&self, _intent: &UserIntent) -> GeneratedTemplateCost {
        // Rough estimate: 1-3 API calls depending on retries
        GeneratedTemplateCost {
            estimated_calls: self.config.max_retries as u32,
            estimated_tokens: self.config.max_tokens,
        }
    }
}

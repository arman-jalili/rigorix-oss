//! LlmGenerateNode — DAG node that wraps LLM generation.
//!
//! @canonical .pi/architecture/modules/llm-step.md#llmgeneratenode
//! Implements: Contract Freeze — LlmGenerateNode domain entity
//! Issue: issue-contract-freeze
//!
//! The execution engine sees LlmGenerateNode as just another node type —
//! but internally it calls an LLM to generate code, fix errors, or
//! produce structured output during DAG execution.
//!
//! # Lifecycle
//!
//! 1. **Created** — A new LlmGenerateNode is instantiated with model config,
//!    prompt template, and output expectations.
//! 2. **Context Built** — The LlmStepContext assembles source code context
//!    and failure analysis.
//! 3. **Generation** — The LLM is called with the assembled prompt.
//! 4. **Output Parsed** — The LLM response is parsed into the expected format.
//! 5. **Completed/Failed** — The node reaches a terminal state.
//!
//! # Contract (Frozen)
//! - Pure domain entity with no framework dependencies
//! - All state transitions are validated by the application layer
//! - Config carries all parameters needed for LLM generation

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A DAG node that generates content via an LLM call.
///
/// LlmGenerateNode is a domain entity that represents a single LLM
/// generation step within an execution DAG. It carries all configuration
/// needed to call an LLM provider, the context to build the prompt, and
/// the generated output once complete.
///
/// The execution engine treats LlmGenerateNode as a regular DAG node —
/// it participates in the standard lifecycle (queued → executing →
/// completed/failed) — but the "execution" step involves an LLM call
/// rather than a local computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmGenerateNode {
    /// Unique identifier for this generation node.
    pub id: Uuid,

    /// Human-readable name for display and logging.
    pub name: String,

    /// The LLM model configuration for this generation.
    pub model_config: LlmModelConfig,

    /// The prompt template to use for generation.
    ///
    /// Placeholders in the template (e.g., `{source_code}`, `{error_context}`)
    /// are filled by the LlmStepContext at generation time.
    pub prompt_template: String,

    /// The expected output format from the LLM.
    pub output_schema: LlmOutputSchema,

    /// Current state of this generation node.
    pub state: LlmGenerateNodeState,

    /// The generated output (populated when generation completes).
    pub output: Option<LlmGenerationOutput>,

    /// The error that occurred during generation (if any).
    pub error: Option<String>,

    /// ISO 8601 timestamp when this node was created.
    pub created_at: DateTime<Utc>,

    /// ISO 8601 timestamp when generation started.
    pub started_at: Option<DateTime<Utc>>,

    /// ISO 8601 timestamp when generation completed.
    pub completed_at: Option<DateTime<Utc>>,
}

/// Configuration for the LLM model to use for generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModelConfig {
    /// The LLM provider (e.g., "anthropic", "openai").
    pub provider: String,

    /// The model identifier (e.g., "claude-sonnet-4-20250514", "gpt-4o").
    pub model: String,

    /// Maximum tokens for the response.
    pub max_tokens: u32,

    /// Temperature for generation (0.0 = deterministic, 1.0 = creative).
    pub temperature: f64,

    /// Top-p sampling parameter.
    pub top_p: f64,

    /// Request timeout in seconds.
    pub timeout_secs: u64,

    /// Maximum retries for transient provider errors.
    pub max_retries: u8,
}

impl Default for LlmModelConfig {
    fn default() -> Self {
        Self {
            provider: String::from("anthropic"),
            model: String::from("claude-sonnet-4-20250514"),
            max_tokens: 4096,
            temperature: 0.7,
            top_p: 0.9,
            timeout_secs: 120,
            max_retries: 3,
        }
    }
}

/// The expected output format from the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmOutputSchema {
    /// The output format type.
    pub format: LlmOutputFormat,

    /// A JSON Schema or description of the expected output structure.
    ///
    /// For `Json` format, this should be a valid JSON Schema string.
    /// For `Text` format, this can be a natural language description.
    /// For `Code` format, this should describe the expected code structure.
    pub schema: String,

    /// Whether the schema is required (strict mode).
    pub strict: bool,
}

/// Supported LLM output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmOutputFormat {
    /// Plain text output.
    Text,
    /// JSON structured output.
    Json,
    /// Source code output (language inferred from context).
    Code,
    /// Markdown formatted output.
    Markdown,
}

/// The lifecycle state of an LlmGenerateNode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmGenerateNodeState {
    /// Node has been created but not yet queued for execution.
    Created,
    /// Context is being assembled for this node.
    BuildingContext,
    /// The LLM call is in progress.
    Generating,
    /// Generation completed successfully.
    Completed,
    /// Generation failed with an error.
    Failed,
    /// Generation was cancelled.
    Cancelled,
}

/// The output produced by a successful LLM generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmGenerationOutput {
    /// The raw text output from the LLM.
    pub raw_output: String,

    /// The parsed output (for JSON format, this is the parsed JSON;
    /// for other formats, this mirrors `raw_output`).
    pub parsed_output: serde_json::Value,

    /// Total tokens used in the generation (prompt + completion).
    pub total_tokens: u32,

    /// Tokens used in the prompt.
    pub prompt_tokens: u32,

    /// Tokens used in the completion.
    pub completion_tokens: u32,

    /// The model that was used for generation.
    pub model_used: String,

    /// ISO 8601 timestamp when the output was produced.
    pub generated_at: DateTime<Utc>,

    /// Optional metadata returned by the provider.
    pub provider_metadata: Option<serde_json::Value>,
}

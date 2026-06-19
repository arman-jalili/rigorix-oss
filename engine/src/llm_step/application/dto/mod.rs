//! Data Transfer Objects for the LLM Step module.
//!
//! @canonical .pi/architecture/modules/llm-step.md
//! Implements: Contract Freeze — DTO schemas for LLM step operations
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::llm_step::domain::{
    FailureContext, LlmGenerateNode, LlmGenerationOutput, LlmModelConfig, LlmStepContext,
    SourceContext,
};

// ---------------------------------------------------------------------------
// Create Node DTOs
// ---------------------------------------------------------------------------

/// Input for creating a new LlmGenerateNode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNodeInput {
    /// Human-readable name for the generation node.
    pub name: String,

    /// The LLM model configuration.
    pub model_config: LlmModelConfig,

    /// The prompt template with placeholders for context.
    pub prompt_template: String,

    /// The expected output schema.
    pub output_schema: crate::llm_step::domain::LlmOutputSchema,
}

/// Output from creating a new LlmGenerateNode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNodeOutput {
    /// The created LlmGenerateNode.
    pub node: LlmGenerateNode,

    /// ISO 8601 timestamp of creation.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Build Context DTOs
// ---------------------------------------------------------------------------

/// Input for assembling context for an LLM generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildContextInput {
    /// The generation node ID.
    pub node_id: Uuid,

    /// The execution ID this node belongs to.
    pub execution_id: Uuid,

    /// The DAG ID for execution context.
    pub dag_id: Uuid,

    /// Target file path for the generated code (if known).
    pub target_file_path: Option<String>,

    /// Source file paths to include in context.
    pub source_file_paths: Vec<String>,

    /// Whether to attempt to gather failure context from the execution.
    pub include_failure_context: bool,
}

/// Output from assembling context for an LLM generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildContextOutput {
    /// The assembled LLM step context.
    pub context: LlmStepContext,

    /// Number of source files included.
    pub source_file_count: u32,

    /// Number of symbol definitions included.
    pub symbol_count: u32,

    /// Whether failure context was gathered.
    pub has_failure_context: bool,

    /// ISO 8601 timestamp of assembly.
    pub assembled_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Generate DTOs
// ---------------------------------------------------------------------------

/// Input for executing an LLM generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateInput {
    /// The generation node to execute.
    pub node: LlmGenerateNode,

    /// The assembled context for this generation.
    pub context: LlmStepContext,

    /// The LLM API key (injected at runtime, never stored).
    ///
    /// This is a sensitive field. It must never be logged or persisted.
    pub api_key: String,
}

/// Output from executing an LLM generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateOutput {
    /// The node ID that was executed.
    pub node_id: Uuid,

    /// The generated output.
    pub output: LlmGenerationOutput,

    /// Duration of the generation in milliseconds.
    pub duration_ms: u64,

    /// ISO 8601 timestamp of generation.
    pub generated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Execute Step DTOs (orchestrated flow)
// ---------------------------------------------------------------------------

/// Input for executing a full LLM step (context build + generate).
///
/// This is the primary entry point that orchestrates:
/// 1. Context assembly (source code + failure analysis)
/// 2. LLM generation
/// 3. Output parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteStepInput {
    /// The generation node to execute.
    pub node: LlmGenerateNode,

    /// The execution ID this node belongs to.
    pub execution_id: Uuid,

    /// The DAG ID for execution context.
    pub dag_id: Uuid,

    /// Target file path for the generated code (if known).
    pub target_file_path: Option<String>,

    /// Source file paths to include in context.
    pub source_file_paths: Vec<String>,

    /// Whether to attempt to gather failure context.
    pub include_failure_context: bool,

    /// The LLM API key (injected at runtime, never stored).
    pub api_key: String,
}

/// Output from executing a full LLM step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteStepOutput {
    /// The node ID that was executed.
    pub node_id: Uuid,

    /// The generated output.
    pub output: LlmGenerationOutput,

    /// The assembled context (for audit and debugging).
    pub context: LlmStepContext,

    /// Duration of the full step in milliseconds.
    pub total_duration_ms: u64,

    /// Duration of context assembly in milliseconds.
    pub context_duration_ms: u64,

    /// Duration of LLM generation in milliseconds.
    pub generation_duration_ms: u64,

    /// Total tokens used across all LLM calls.
    pub total_tokens_used: u32,

    /// Whether the step was a retry.
    pub is_retry: bool,

    /// The retry attempt number (0 = first attempt).
    pub retry_attempt: u8,

    /// ISO 8601 timestamp of completion.
    pub completed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Context Query DTOs
// ---------------------------------------------------------------------------

/// Input for querying available source files for context building.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSourceContextInput {
    /// The execution ID.
    pub execution_id: Uuid,

    /// The file paths to include in the context.
    pub file_paths: Vec<String>,

    /// Maximum context size in characters (for token budget management).
    pub max_context_size: Option<usize>,
}

/// Output from querying source context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSourceContextOutput {
    /// The assembled source context.
    pub source_context: SourceContext,

    /// Total characters in the context.
    pub total_size: usize,

    /// ISO 8601 timestamp.
    pub retrieved_at: DateTime<Utc>,
}

/// Input for querying failure context for a recovery generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFailureContextInput {
    /// The execution ID.
    pub execution_id: Uuid,

    /// The node ID that failed.
    pub node_id: Uuid,

    /// Maximum number of previous attempts to include.
    pub max_previous_attempts: Option<u8>,
}

/// Output from querying failure context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFailureContextOutput {
    /// The assembled failure context.
    pub failure_context: FailureContext,

    /// Number of previous attempts included.
    pub previous_attempt_count: u8,

    /// ISO 8601 timestamp.
    pub retrieved_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Retry DTOs
// ---------------------------------------------------------------------------

/// Input for retrying a failed LLM generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryGenerationInput {
    /// The node to retry.
    pub node: LlmGenerateNode,

    /// The previously assembled context (may be updated with new failure context).
    pub context: LlmStepContext,

    /// The retry attempt number (1-indexed).
    pub attempt: u8,

    /// The updated failure context from the previous attempt.
    pub updated_failure_context: FailureContext,

    /// The LLM API key (injected at runtime, never stored).
    pub api_key: String,
}

/// Output from retrying a failed LLM generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryGenerationOutput {
    /// The generated output.
    pub output: LlmGenerationOutput,

    /// Duration of the retry generation in milliseconds.
    pub duration_ms: u64,

    /// ISO 8601 timestamp.
    pub generated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Validation DTOs
// ---------------------------------------------------------------------------

/// Input for validating an LlmGenerateNode configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateNodeConfigInput {
    /// The node configuration to validate.
    pub node: LlmGenerateNode,
}

/// Output from validating an LlmGenerateNode configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateNodeConfigOutput {
    /// Whether the configuration is valid.
    pub is_valid: bool,

    /// List of validation errors (empty if valid).
    pub errors: Vec<String>,

    /// List of validation warnings.
    pub warnings: Vec<String>,

    /// Estimated token cost for this configuration.
    pub estimated_token_cost: Option<u32>,
}

// ---------------------------------------------------------------------------
// Status DTOs
// ---------------------------------------------------------------------------

/// Summary of an LLM step for display and listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStepSummary {
    /// The generation node ID.
    pub node_id: Uuid,

    /// The node name.
    pub name: String,

    /// The current state.
    pub state: String,

    /// The model being used.
    pub model: String,

    /// The provider being used.
    pub provider: String,

    /// Number of retries attempted.
    pub retries_attempted: u8,

    /// Total tokens used so far.
    pub total_tokens_used: u32,

    /// ISO 8601 timestamp of creation.
    pub created_at: DateTime<Utc>,

    /// ISO 8601 timestamp when generation started.
    pub started_at: Option<DateTime<Utc>>,

    /// ISO 8601 timestamp when generation completed.
    pub completed_at: Option<DateTime<Utc>>,
}

impl LlmStepSummary {
    /// Create an LlmStepSummary from an LlmGenerateNode.
    pub fn from_node(node: &LlmGenerateNode) -> Self {
        Self {
            node_id: node.id,
            name: node.name.clone(),
            state: format!("{:?}", node.state),
            model: node.model_config.model.clone(),
            provider: node.model_config.provider.clone(),
            retries_attempted: 0,
            total_tokens_used: node.output.as_ref().map_or(0, |o| o.total_tokens),
            created_at: node.created_at,
            started_at: node.started_at,
            completed_at: node.completed_at,
        }
    }
}

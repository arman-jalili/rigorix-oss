//! Event payload schemas for the LLM Step bounded context.
//!
//! @canonical .pi/architecture/modules/llm-step.md#events
//! Implements: Contract Freeze — LlmStepEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted on the `EventBus` whenever significant LLM step
//! lifecycle events occur — generation started, completed, failed, context
//! built, token budget exceeded. Consumers (orchestrator, audit, TUI)
//! subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `node_id` and `execution_id` correlate to the originating context

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Events emitted by the LLM Step module.
///
/// Wrapped in `ExecutionEvent::llm_step_event(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LlmStepEvent {
    /// Context assembly has started for a generation node.
    ///
    /// Emitted when the LlmStepContext begins gathering source code
    /// and failure analysis data.
    ContextAssemblyStarted {
        /// The generation node ID.
        node_id: Uuid,
        /// The execution ID this node belongs to.
        execution_id: Uuid,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Context assembly completed successfully.
    ///
    /// Emitted when all context sources have been gathered and the
    /// prompt is ready for generation.
    ContextAssemblyCompleted {
        /// The generation node ID.
        node_id: Uuid,
        /// The execution ID this node belongs to.
        execution_id: Uuid,
        /// Number of source files included in the context.
        source_file_count: u32,
        /// Number of symbol definitions included.
        symbol_count: u32,
        /// Whether failure context is included.
        has_failure_context: bool,
        /// Total character length of the assembled prompt.
        prompt_length: u32,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Context assembly failed.
    ///
    /// Emitted when the context builder encounters an error gathering
    /// source code or failure analysis data.
    ContextAssemblyFailed {
        /// The generation node ID.
        node_id: Uuid,
        /// The execution ID this node belongs to.
        execution_id: Uuid,
        /// The source that failed to provide context.
        source: String,
        /// Details about the failure.
        message: String,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// LLM generation has started.
    ///
    /// Emitted when the LlmStepService sends the request to the LLM provider.
    GenerationStarted {
        /// The generation node ID.
        node_id: Uuid,
        /// The execution ID this node belongs to.
        execution_id: Uuid,
        /// The model being used for generation.
        model: String,
        /// The provider being called.
        provider: String,
        /// Number of tokens in the assembled prompt.
        /// ISO 8601 timestamp.
        prompt_token_count: u32,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// LLM generation completed successfully.
    ///
    /// Emitted when the LLM provider returns a valid response.
    GenerationCompleted {
        /// The generation node ID.
        node_id: Uuid,
        /// The execution ID this node belongs to.
        execution_id: Uuid,
        /// The total tokens used (prompt + completion).
        total_tokens: u32,
        /// Tokens used in the prompt.
        prompt_tokens: u32,
        /// Tokens used in the completion.
        completion_tokens: u32,
        /// The model that was used.
        model_used: String,
        /// Duration of the generation in milliseconds.
        duration_ms: u64,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// LLM generation failed.
    ///
    /// Emitted when the LLM provider returns an error or the response
    /// cannot be parsed.
    GenerationFailed {
        /// The generation node ID.
        node_id: Uuid,
        /// The execution ID this node belongs to.
        execution_id: Uuid,
        /// The error type (provider_error, parse_error, timeout, etc.).
        error_type: String,
        /// The error message.
        message: String,
        /// Number of retries remaining before permanent failure.
        retries_remaining: u8,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// A generation was retried after a transient failure.
    ///
    /// Emitted when the retry policy triggers a new attempt.
    GenerationRetried {
        /// The generation node ID.
        node_id: Uuid,
        /// The execution ID this node belongs to.
        execution_id: Uuid,
        /// The retry attempt number (1-indexed).
        attempt: u8,
        /// The reason for the retry.
        reason: String,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// The token budget was exceeded during generation.
    ///
    /// Emitted when generation is aborted because the token limit
    /// was reached. This may trigger a retry with a higher budget
    /// or a different model.
    TokenBudgetExceeded {
        /// The generation node ID.
        node_id: Uuid,
        /// The execution ID this node belongs to.
        execution_id: Uuid,
        /// Tokens used before the limit was reached.
        used: u32,
        /// The maximum allowed tokens.
        max: u32,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// LLM generation output was parsed successfully.
    ///
    /// Emitted after the raw LLM response is parsed into the expected
    /// output format (JSON, code, etc.).
    OutputParsed {
        /// The generation node ID.
        node_id: Uuid,
        /// The execution ID this node belongs to.
        execution_id: Uuid,
        /// The output format that was parsed.
        format: String,
        /// Size of the parsed output in characters.
        output_size: u32,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// LLM generation output could not be parsed.
    OutputParseFailed {
        /// The generation node ID.
        node_id: Uuid,
        /// The execution ID this node belongs to.
        execution_id: Uuid,
        /// The expected format.
        expected_format: String,
        /// Details about why parsing failed.
        message: String,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },
}

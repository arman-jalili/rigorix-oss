//! Error types for the LLM Step bounded context.
//!
//! @canonical .pi/architecture/modules/llm-step.md#errors
//! Implements: Contract Freeze — LlmStepError enum
//! Issue: issue-contract-freeze
//!
//! Defines domain-specific errors for LLM step operations. These errors
//! are wrapped into `CoreOrchestratorError::LlmStep` at the orchestration
//! layer for consistent error propagation.
//!
//! # Contract (Frozen)
//! - Every error variant has a clear semantic meaning
//! - Errors carry enough context for diagnosis and retry decisions
//! - Each variant implements `is_retriable()` for the retry subsystem

use thiserror::Error;

/// Error type for all LLM Step operations.
///
/// These errors are domain-specific. At the orchestration layer they
/// are wrapped into `CoreOrchestratorError::LlmStep` for uniform
/// error handling.
#[derive(Debug, Error)]
pub enum LlmStepError {
    /// The LLM provider returned an error (network, auth, rate limit).
    ///
    /// Carries the provider name, HTTP status code, and the raw error
    /// message from the provider response.
    #[error("LLM provider error ({provider}): {message} (status: {status})")]
    ProviderError {
        /// The LLM provider that returned the error (e.g., "anthropic", "openai").
        provider: String,
        /// HTTP status code from the provider API.
        status: u16,
        /// Human-readable error message.
        message: String,
    },

    /// The LLM response could not be parsed into the expected format.
    ///
    /// This typically happens when the LLM returns malformed JSON or
    /// a response that doesn't match the expected schema.
    #[error("Failed to parse LLM response: {message}")]
    ParseError {
        /// Details about the parse failure.
        message: String,
        /// The raw response text that failed to parse.
        raw_response: String,
    },

    /// The context builder could not gather the required context.
    ///
    /// This happens when the source code context or failure analysis
    /// data is unavailable or incomplete.
    #[error("Context build failed: {message} (from: {context_source})")]
    ContextBuildFailed {
        /// Details about why context collection failed.
        message: String,
        /// The component that failed to provide context.
        context_source: String,
    },

    /// The generation node configuration is invalid.
    ///
    /// This happens when an LlmGenerateNode has missing or invalid
    /// configuration fields.
    #[error("Invalid node configuration: {message}")]
    InvalidConfiguration {
        /// Details about the configuration issue.
        message: String,
        /// The field or aspect of the configuration that is invalid.
        field: String,
    },

    /// The generation exceeded the configured token budget.
    ///
    /// Carries both the limit and the actual usage so the caller can
    /// adjust budget allocation.
    #[error("Token budget exceeded: used {used}, max {max}")]
    TokenBudgetExceeded {
        /// Number of tokens used in the generation.
        used: u32,
        /// Maximum allowed tokens.
        max: u32,
    },

    /// The generation was cancelled.
    #[error("LLM step cancelled: {reason}")]
    Cancelled {
        /// Why the step was cancelled.
        reason: String,
    },

    /// The generation timed out.
    #[error("LLM step timed out after {duration_secs}s")]
    Timeout {
        /// The timeout duration in seconds.
        duration_secs: u64,
    },

    /// The requested model is not supported or not available.
    #[error("Unsupported model: {model}")]
    UnsupportedModel {
        /// The model identifier that is not supported.
        model: String,
        /// Available models on this provider.
        available_models: Vec<String>,
    },

    /// A required dependency was not found or not configured.
    #[error("Missing dependency: {dependency}")]
    MissingDependency {
        /// The name of the missing dependency.
        dependency: String,
        /// Guidance on how to resolve the missing dependency.
        resolution: Option<String>,
    },
}

impl LlmStepError {
    /// Check if this error represents a transient failure that can be retried.
    ///
    /// Transient errors include provider errors (network, rate limits),
    /// timeouts, and token budget issues (which may resolve with a different
    /// model or budget allocation).
    pub fn is_retriable(&self) -> bool {
        match self {
            // Provider errors may succeed on retry (network blips, rate limit expiry)
            LlmStepError::ProviderError { status, .. } if *status >= 429 => true,
            LlmStepError::ProviderError { .. } => false,
            // Parse errors are unlikely to succeed on retry — the prompt or
            // output format needs adjustment
            LlmStepError::ParseError { .. } => false,
            // Context build failures may succeed on retry if caused by
            // transient infrastructure issues
            LlmStepError::ContextBuildFailed { .. } => true,
            // Configuration errors are permanent — fix the config
            LlmStepError::InvalidConfiguration { .. } => false,
            // Token budget may resolve with a different budget allocation
            LlmStepError::TokenBudgetExceeded { .. } => true,
            // Cancellation is intentional
            LlmStepError::Cancelled { .. } => false,
            // Timeouts may succeed on retry
            LlmStepError::Timeout { .. } => true,
            // Model unsupported is permanent
            LlmStepError::UnsupportedModel { .. } => false,
            // Missing dependency is permanent
            LlmStepError::MissingDependency { .. } => false,
        }
    }
}

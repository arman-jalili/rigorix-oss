//! Factory interfaces for constructing LLM Step service instances.
//!
//! @canonical .pi/architecture/modules/llm-step.md
//! Implements: Contract Freeze — LlmStepFactory and LlmContextBuilderFactory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of LlmStepService and
//! LlmContextBuilderService instances with appropriate LLM provider
//! configuration, storage paths, and event bus integration.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured service instance
//! - Configuration is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::llm_step::domain::LlmStepError;

use super::service::{LlmContextBuilderService, LlmStepService};

/// Factory for constructing `LlmStepService` instances.
///
/// Handles creation of the LLM step service with appropriate provider
/// configuration, event bus integration, and retry policies.
#[async_trait]
pub trait LlmStepFactory: Send + Sync {
    /// Create an `LlmStepService` instance.
    ///
    /// Configures the service with the LLM provider, event bus,
    /// and retry policy for generation operations.
    async fn create(
        &self,
        config: LlmStepFactoryConfig,
    ) -> Result<Box<dyn LlmStepService>, LlmStepError>;
}

/// Configuration for creating an `LlmStepService` instance.
#[derive(Debug, Clone)]
pub struct LlmStepFactoryConfig {
    /// The default LLM provider to use.
    pub default_provider: LlmProviderConfig,

    /// Whether to emit audit events for generations.
    pub emit_audit_events: bool,

    /// Whether to emit LlmStepEvent events.
    pub emit_step_events: bool,

    /// Maximum number of retries for transient failures.
    pub max_retries: u8,

    /// Default timeout for LLM calls in seconds.
    pub default_timeout_secs: u64,

    /// Whether to validate configurations before execution.
    pub validate_before_execution: bool,
}

impl Default for LlmStepFactoryConfig {
    fn default() -> Self {
        Self {
            default_provider: LlmProviderConfig::default(),
            emit_audit_events: true,
            emit_step_events: true,
            max_retries: 3,
            default_timeout_secs: 120,
            validate_before_execution: true,
        }
    }
}

/// Configuration for an LLM provider connection.
#[derive(Debug, Clone)]
pub struct LlmProviderConfig {
    /// The provider name (e.g., "anthropic", "openai").
    pub provider_name: String,

    /// The default model to use.
    pub default_model: String,

    /// The API endpoint URL.
    pub api_url: String,

    /// Maximum tokens for the response.
    pub max_tokens: u32,

    /// Default temperature for generation.
    pub temperature: f64,
}

impl Default for LlmProviderConfig {
    fn default() -> Self {
        Self {
            provider_name: String::from("anthropic"),
            default_model: String::from("claude-sonnet-4-20250514"),
            api_url: String::from("https://api.anthropic.com/v1/messages"),
            max_tokens: 4096,
            temperature: 0.7,
        }
    }
}

/// Factory for constructing `LlmContextBuilderService` instances.
///
/// Handles creation of the context builder service with appropriate
/// repo engine integration, code graph access, and failure analysis
/// module connections.
#[async_trait]
pub trait LlmContextBuilderFactory: Send + Sync {
    /// Create an `LlmContextBuilderService` instance.
    ///
    /// Configures the context builder with repo engine, code graph,
    /// and failure classification module dependencies.
    async fn create(
        &self,
        config: LlmContextBuilderFactoryConfig,
    ) -> Result<Box<dyn LlmContextBuilderService>, LlmStepError>;
}

/// Configuration for creating an `LlmContextBuilderService` instance.
#[derive(Debug, Clone)]
pub struct LlmContextBuilderFactoryConfig {
    /// Maximum number of source files to include in context.
    pub max_source_files: u32,

    /// Maximum context size in characters.
    pub max_context_size: usize,

    /// Whether to include symbol definitions in context.
    pub include_symbols: bool,

    /// Maximum number of previous attempts to include in failure context.
    pub max_previous_attempts: u8,

    /// Whether to include full file contents or just snippets.
    pub use_snippets: bool,
}

impl Default for LlmContextBuilderFactoryConfig {
    fn default() -> Self {
        Self {
            max_source_files: 20,
            max_context_size: 100_000,
            include_symbols: true,
            max_previous_attempts: 3,
            use_snippets: true,
        }
    }
}

/// Factory for constructing LLM provider client instances.
///
/// Abstracts the creation of HTTP clients for different LLM providers.
/// Supports Anthropic and OpenAI providers with configurable auth.
#[async_trait]
pub trait LlmProviderClientFactory: Send + Sync {
    /// Create an LLM provider client.
    ///
    /// Returns a client configured for the specified provider.
    /// The returned client can be used to make LLM API calls.
    async fn create_client(
        &self,
        config: LlmProviderConfig,
    ) -> Result<Box<dyn LlmProviderClient>, LlmStepError>;
}

/// A client for making LLM API calls.
///
/// Abstracts the HTTP request/response for different LLM providers.
/// Implementations handle provider-specific request formats, auth,
/// and response parsing.
#[async_trait]
pub trait LlmProviderClient: Send + Sync + std::fmt::Debug {
    /// Send a generation request to the LLM provider.
    ///
    /// Returns the raw provider response. The caller is responsible
    /// for parsing the response into the expected output format.
    async fn generate(
        &self,
        request: LlmProviderRequest,
    ) -> Result<LlmProviderResponse, LlmStepError>;

    /// Check if the provider is healthy and the API key is valid.
    async fn health_check(&self) -> Result<bool, LlmStepError>;
}

/// A request to an LLM provider.
#[derive(Debug, Clone)]
pub struct LlmProviderRequest {
    /// The model to use for generation.
    pub model: String,

    /// The system prompt.
    pub system_prompt: String,

    /// The user message / prompt.
    pub user_message: String,

    /// Maximum tokens for the response.
    pub max_tokens: u32,

    /// Temperature for generation.
    pub temperature: f64,

    /// Top-p sampling parameter.
    pub top_p: f64,

    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

/// A response from an LLM provider.
#[derive(Debug, Clone)]
pub struct LlmProviderResponse {
    /// The generated text content.
    pub content: String,

    /// The model that was used.
    pub model: String,

    /// Tokens used in the prompt.
    pub prompt_tokens: u32,

    /// Tokens used in the completion.
    pub completion_tokens: u32,

    /// The stop reason (e.g., "end_turn", "max_tokens").
    pub stop_reason: String,

    /// Provider-specific metadata.
    pub provider_metadata: Option<serde_json::Value>,
}

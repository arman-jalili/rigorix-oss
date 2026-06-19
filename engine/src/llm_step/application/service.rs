//! Service interfaces (use cases) for the LLM Step bounded context.
//!
//! @canonical .pi/architecture/modules/llm-step.md
//! Implements: Contract Freeze — LlmStepService trait
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for LLM-based
//! code generation during DAG execution:
//! - `LlmStepService`: Orchestrates context building, LLM generation, and retries
//! - `LlmContextBuilderService`: Handles source code and failure context assembly
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::llm_step::domain::LlmStepError;

use super::dto::{
    BuildContextInput, BuildContextOutput, CreateNodeInput, CreateNodeOutput, ExecuteStepInput,
    ExecuteStepOutput, GenerateInput, GenerateOutput, GetFailureContextInput,
    GetFailureContextOutput, GetSourceContextInput, GetSourceContextOutput, RetryGenerationInput,
    RetryGenerationOutput, ValidateNodeConfigInput, ValidateNodeConfigOutput,
};

/// Central LLM step service for orchestrating LLM generation during execution.
///
/// The LlmStepService orchestrates the full lifecycle of an LLM generation
/// within a DAG execution:
///
/// 1. **Context Building** — Assembles source code context and failure analysis
/// 2. **Generation** — Calls the LLM provider with the assembled prompt
/// 3. **Output Parsing** — Parses the LLM response into the expected format
/// 4. **Retry** — Handles transient failures with configurable retry policy
///
/// # Integration Points
///
/// - **Repo Engine** — For source code context (symbol graph, file contents)
/// - **Failure Classification** — For failure analysis context during recovery
/// - **Execution Engine** — For DAG execution state and previous node outputs
/// - **Budget Tracking** — For LLM token budget enforcement
/// - **Event System** — For emitting LlmStepEvents
/// - **Audit** — For recording generation audits
///
/// # Lifecycle
///
/// 1. `create_node` — Instantiate a new LlmGenerateNode with configuration
/// 2. `build_context` — Assemble source code and failure context
/// 3. `execute_step` — End-to-end: build context + generate + parse
/// 4. `generate` — Execute the LLM call with the assembled context
/// 5. `retry_generation` — Retry a failed generation with updated context
/// 6. `validate_node_config` — Validate node configuration before execution
///
/// # Cancellation Integration
///
/// The step service cooperates with the Cancellation module:
/// - LLM calls should check for cancellation signals before starting
/// - Context assembly should be interruptible without data corruption
/// - Token budget should be rolled back on cancellation
#[async_trait]
pub trait LlmStepService: Send + Sync {
    /// Create a new LlmGenerateNode with the given configuration.
    ///
    /// Validates the configuration and returns the created node.
    /// The node is in `Created` state and ready for context assembly.
    async fn create_node(&self, input: CreateNodeInput) -> Result<CreateNodeOutput, LlmStepError>;

    /// Assemble context for an LLM generation.
    ///
    /// Gathers source code context from the repo engine and failure
    /// analysis from the execution state. The assembled context
    /// fills the prompt template placeholders.
    async fn build_context(
        &self,
        input: BuildContextInput,
    ) -> Result<BuildContextOutput, LlmStepError>;

    /// Execute a full LLM step end-to-end.
    ///
    /// Orchestrates:
    /// 1. Context assembly (source code + failure analysis)
    /// 2. LLM generation
    /// 3. Output parsing
    ///
    /// This is the primary entry point for executing an LLM step
    /// within a DAG execution.
    async fn execute_step(
        &self,
        input: ExecuteStepInput,
    ) -> Result<ExecuteStepOutput, LlmStepError>;

    /// Execute an LLM generation with an already-assembled context.
    ///
    /// Sends the assembled prompt to the LLM provider and parses
    /// the response into the expected output format.
    async fn generate(&self, input: GenerateInput) -> Result<GenerateOutput, LlmStepError>;

    /// Retry a failed generation with updated failure context.
    ///
    /// Updates the context with the new failure information from
    /// the previous attempt and retries the generation.
    async fn retry_generation(
        &self,
        input: RetryGenerationInput,
    ) -> Result<RetryGenerationOutput, LlmStepError>;

    /// Validate an LlmGenerateNode configuration before execution.
    ///
    /// Checks model availability, prompt template validity, output
    /// schema validity, and estimated token cost.
    async fn validate_node_config(
        &self,
        input: ValidateNodeConfigInput,
    ) -> Result<ValidateNodeConfigOutput, LlmStepError>;
}

/// Service for assembling source code and failure context for LLM generation.
///
/// The LlmContextBuilderService handles the gathering and assembly of
/// context from multiple sources:
/// - Repo engine (source files, symbol definitions)
/// - Failure classification (error type, retry strategy)
/// - Execution engine (DAG state, previous node outputs)
///
/// This service is consumed by LlmStepService but is also available
/// as a standalone service for use cases that need context without
/// immediate generation (e.g., context preview, audit logging).
#[async_trait]
pub trait LlmContextBuilderService: Send + Sync {
    /// Build source code context from the repo engine.
    ///
    /// Gathers the specified source files, symbol definitions, and
    /// repository structure from the code graph.
    async fn get_source_context(
        &self,
        input: GetSourceContextInput,
    ) -> Result<GetSourceContextOutput, LlmStepError>;

    /// Build failure analysis context.
    ///
    /// Gathers failure type, error messages, and retry strategy
    /// from the failure classification module.
    async fn get_failure_context(
        &self,
        input: GetFailureContextInput,
    ) -> Result<GetFailureContextOutput, LlmStepError>;

    /// Assemble the final prompt from a template and context.
    ///
    /// Fills template placeholders with the gathered context values
    /// and produces the final system and user prompts for the LLM.
    async fn assemble_prompt(
        &self,
        template: String,
        source_context: crate::llm_step::domain::SourceContext,
        failure_context: Option<crate::llm_step::domain::FailureContext>,
    ) -> Result<String, LlmStepError>;
}

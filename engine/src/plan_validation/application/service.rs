//! ValidationLoopService — application service interface for the plan validation loop.
//!
//! @canonical .pi/architecture/modules/plan-validation.md#service
//! Implements: Contract Freeze — ValidationLoopService trait, NodeClassification
//! Issue: issue-contract-freeze
//!
//! The ValidationLoopService is the central application service for the
//! self-correcting plan→execute→verify→fix loop. It orchestrates the
//! validation of a template through iterative retries, where only
//! generative (llm_generate) nodes are retried with augmented context
//! from failure analysis, while deterministic node outputs are cached
//! and reused across iterations.
//!
//! # Flow
//!
//! 1. `validate` — End-to-end validation loop: plan → execute → verify
//!    → [if fail: parse → augment → retry generative] → return
//! 2. `classify_nodes` — Separate template nodes into generative vs deterministic
//! 3. `retry_generative_nodes` — Retry only llm_generate nodes with augmented context
//!
//! # Selective Retry
//!
//! The core insight: deterministic steps (file_read, file_patch with AST anchors,
//! compile-check, test-run) execute identically every time and their outputs
//! are cached. Only `llm_generate` nodes are retried with augmented context.
//! This avoids wasted LLM calls and preserves template stability.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::plan_validation::domain::error::ValidationLoopError;

use super::dto::{
    ClassifyNodesInput, ClassifyNodesOutput, EvaluateIterationInput, EvaluateIterationOutput,
    RetryGenerativeNodesInput, RetryGenerativeNodesOutput,
    ValidateInput, ValidateOutput,
};

/// Central validation loop service that orchestrates the self-correcting
/// plan→execute→verify→fix cycle.
///
/// The ValidationLoopService wraps the planning pipeline, execution engine,
/// failure parser, and quality gates into a cohesive retry loop that:
///
/// 1. Plans and executes a template from user intent
/// 2. Verifies the output against quality gates
/// 3. On failure: parses errors, augments context, retries only LLM steps
/// 4. Returns the validated template or a structured failure report
///
/// # Lifecycle
///
/// 1. `validate` — Full end-to-end validation loop
/// 2. `classify_nodes` — Separate generative from deterministic nodes
/// 3. `retry_generative_nodes` — Targeted retry of only LLM-generated content
///
/// # Cancellation Integration
///
/// The validation loop cooperates with the Cancellation module:
/// - Long-running LLM calls check for cancellation signals
/// - State is preserved for graceful resumption after interruption
/// - Budget reservations are rolled back on cancellation
///
/// # Error Recovery
///
/// - Planning failures → retry with reduced template set
/// - Execution failures → parse, augment intent, retry generative nodes
/// - Quality gate failures → same as execution failures
/// - Repeated identical failures → escalate (LLM not learning)
#[async_trait]
pub trait ValidationLoopService: Send + Sync {
    /// Execute the full validation loop for a given intent.
    ///
    /// Flow: plan → execute → verify → [if fail: parse → augment →
    /// retry_generative → verify] → return
    ///
    /// Returns `ValidationOutcome::Validated` with the validated template
    /// on success. Returns `ValidationOutcome::Failed` with the failure
    /// history if all retries are exhausted. Returns
    /// `ValidationOutcome::BudgetExhausted` if the cumulative token
    /// budget is exceeded.
    ///
    /// # Arguments
    ///
    /// * `input` — The validate input containing intent, config, and
    ///   optional existing template.
    ///
    /// # Errors
    ///
    /// Returns `ValidationLoopError` for infrastructure or system errors
    /// (not validation failures — those are captured in the outcome).
    async fn validate(
        &self,
        input: ValidateInput,
    ) -> Result<ValidateOutput, ValidationLoopError>;

    /// Classify template nodes into generative and deterministic categories.
    ///
    /// Generative nodes produce LLM-generated content (llm_generate).
    /// Deterministic nodes perform fixed operations (file_read, file_patch,
    /// run_command, compile_check, test_run) and can be safely cached
    /// across retry iterations.
    ///
    /// # Contract
    ///
    /// - All `llm_generate` action type nodes → generative
    /// - All other action types → deterministic
    /// - Returns both lists even if one is empty
    async fn classify_nodes(
        &self,
        input: ClassifyNodesInput,
    ) -> Result<ClassifyNodesOutput, ValidationLoopError>;

    /// Retry only the generative nodes with augmented context.
    ///
    /// Takes the previous template and the failures from the last
    /// iteration. Re-executes only the `llm_generate` nodes with
    /// augmented context (failure analysis appended to the LLM prompt).
    /// Deterministic node outputs from the previous iteration are
    /// reused (cached).
    ///
    /// This is the core selective retry mechanism. It avoids:
    /// - Wasted LLM calls for steps that were correct
    /// - Template corruption from re-applying deterministic patches
    /// - Excessive iteration time from re-running the full template
    ///
    /// # Contract
    ///
    /// - Only `llm_generate` nodes are re-executed
    /// - Deterministic node outputs are carried over
    /// - The augmented context includes failure analysis
    /// - The returned template is a valid, executable template
    async fn retry_generative_nodes(
        &self,
        input: RetryGenerativeNodesInput,
    ) -> Result<RetryGenerativeNodesOutput, ValidationLoopError>;
}

// ---------------------------------------------------------------------------
// QualityGateEvaluationService
// ---------------------------------------------------------------------------

/// Application service for evaluating quality gates during validation.
///
/// Evaluates whether a template execution met the required quality
/// level. Called by the validation loop after each iteration to
/// determine if validation succeeded or should retry.
///
/// # Contract (Frozen)
/// - All methods are async
/// - Returns structured evaluation results with failures if gate not met
/// - No implementation — only contract signatures
#[async_trait]
pub trait QualityGateEvaluationService: Send + Sync {
    /// Evaluate whether a template execution met the required quality.
    ///
    /// Returns a structured result with any failures found. The
    /// validation loop uses this to decide between success and retry.
    async fn evaluate_iteration(
        &self,
        input: EvaluateIterationInput,
    ) -> Result<EvaluateIterationOutput, ValidationLoopError>;

    /// Check if the cumulative budget has been exceeded.
    async fn check_budget(
        &self,
        cumulative_tokens: u64,
        max_tokens: u64,
    ) -> Result<bool, ValidationLoopError>;
}

// ---------------------------------------------------------------------------
// NodeClassification (data struct)
// ---------------------------------------------------------------------------

/// Classification of template nodes for selective retry.
///
/// Returned by `ValidationLoopService::classify_nodes()` to identify
/// which nodes produce generative content and which are deterministic.
///
/// # Contract (Frozen)
/// - generative: nodes that produce LLM-generated content
/// - deterministic: nodes that perform fixed operations (cached across retries)
#[derive(Debug, Clone)]
pub struct NodeClassification {
    /// IDs of nodes that produce generative content (llm_generate).
    pub generative: Vec<String>,

    /// IDs of nodes that are deterministic (file_read, file_patch,
    /// run_command, compile_check, test_run, etc.).
    pub deterministic: Vec<String>,
}

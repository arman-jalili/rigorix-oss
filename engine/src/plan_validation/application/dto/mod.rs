//! Data Transfer Objects for the Plan Validation module.
//!
//! @canonical .pi/architecture/modules/plan-validation.md
//! Implements: Contract Freeze — DTO schemas for plan validation operations
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

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::failure_parser::domain::TemplateFailure;
use crate::plan_validation::domain::loop_config::ValidationLoopConfig;
use crate::plan_validation::domain::outcome::ValidationOutcome;
use crate::planning::domain::intent::UserIntent;
use crate::templates::domain::Template;

// ---------------------------------------------------------------------------
// Validate DTOs
// ---------------------------------------------------------------------------

/// Input for the validation loop's full `validate()` operation.
///
/// Encapsulates the user intent (possibly augmented from previous
/// validation attempts), the validation loop configuration, and
/// optional initial template (for re-validation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateInput {
    /// The user intent to validate against (may be augmented with
    /// failure analysis from previous attempts).
    pub intent: UserIntent,

    /// The validation loop configuration (max iterations, quality, etc.).
    pub config: ValidationLoopConfig,

    /// Optional execution ID for correlation.
    /// Generated if not provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<Uuid>,

    /// Optional initial template to validate (for re-validation
    /// of an existing template, e.g., after manual fix).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub existing_template: Option<Template>,
}

/// Output from the validation loop's full `validate()` operation.
///
/// Contains the final outcome, the structured validation report,
/// and the validated template (if successful).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateOutput {
    /// The execution ID for correlation.
    pub execution_id: Uuid,

    /// The final outcome of the validation loop.
    pub outcome: ValidationOutcome,

    /// The validated template (present only if validated successfully).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validated_template: Option<Template>,

    /// Number of iterations executed.
    pub iterations: u32,

    /// Cumulative LLM tokens consumed.
    pub cumulative_tokens: u64,

    /// Total duration in milliseconds.
    pub total_duration_ms: u64,

    /// Number of failures across all iterations.
    pub total_failures: u32,
}

// ---------------------------------------------------------------------------
// ClassifyNodes DTOs
// ---------------------------------------------------------------------------

/// Input for classifying template nodes as generative or deterministic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyNodesInput {
    /// The template whose nodes to classify.
    pub template: Template,
}

/// Output from node classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyNodesOutput {
    /// IDs of nodes that produce generative content (llm_generate).
    pub generative: Vec<String>,

    /// IDs of nodes that are deterministic (file_read, file_patch,
    /// run_command, compile_check, etc.).
    pub deterministic: Vec<String>,

    /// Total node count.
    pub total_nodes: u32,
}

// ---------------------------------------------------------------------------
// RetryGenerativeNodes DTOs
// ---------------------------------------------------------------------------

/// Input for retrying only the generative nodes with augmented context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryGenerativeNodesInput {
    /// The execution ID for correlation.
    pub execution_id: Uuid,

    /// The template from the previous iteration containing the
    /// generative nodes to retry.
    pub previous_template: Template,

    /// Failures from the previous iteration that inform the retry.
    pub failures: Vec<TemplateFailure>,

    /// Source context for generating fix suggestions.
    pub source_context: crate::failure_parser::domain::SourceContext,
}

/// Output from retrying generative nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryGenerativeNodesOutput {
    /// The updated template with retried generative nodes.
    pub template: Template,

    /// Number of nodes that were retried.
    pub retried_count: u32,

    /// Number of deterministic nodes that were skipped (cached).
    pub skipped_count: u32,
}

// ---------------------------------------------------------------------------
// AugmentIntent DTOs
// ---------------------------------------------------------------------------

/// Input for augmenting a user intent with failure analysis context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AugmentIntentInput {
    /// The original user intent.
    pub intent: UserIntent,

    /// Failures from the most recent iteration.
    pub failures: Vec<TemplateFailure>,

    /// Full failure history from all previous iterations.
    /// Empty on the first failure.
    #[serde(default)]
    pub failure_history: Vec<Vec<TemplateFailure>>,

    /// The current iteration number (1-indexed).
    pub iteration: u32,

    /// Maximum allowed iterations.
    pub max_iterations: u32,
}

/// Output from augmenting a user intent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AugmentIntentOutput {
    /// The augmented user intent with failure context appended.
    pub augmented_intent: UserIntent,

    /// Whether the augmentation detected repeated failures
    /// (LLM not learning from previous feedback).
    pub has_repeated_failures: bool,

    /// Number of unique failure types in the augmented context.
    pub unique_failure_types: u32,
}

// ---------------------------------------------------------------------------
// CheckRepeatedFailure DTOs
// ---------------------------------------------------------------------------

/// Input for checking if a failure repeats a previous failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRepeatedFailureInput {
    /// The failure to check.
    pub failure: TemplateFailure,

    /// The full failure history.
    pub failure_history: Vec<Vec<TemplateFailure>>,
}

/// Output from repeated failure check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRepeatedFailureOutput {
    /// Whether this is a repeated failure.
    pub is_repeated: bool,

    /// The iteration number where this failure first appeared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_seen_iteration: Option<u32>,

    /// How many times this failure has been seen.
    pub repeat_count: u32,
}

// ---------------------------------------------------------------------------
// ValidateInput / ValidateOutput for the inner per-iteration operation
// ---------------------------------------------------------------------------

/// Input for validating a single iteration's quality gate result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateIterationInput {
    /// The execution ID for correlation.
    pub execution_id: Uuid,

    /// The current template state.
    pub template: Template,

    /// The required quality level from config.
    pub required_quality: String,

    /// The iteration number for reporting.
    pub iteration: u32,
}

/// Output from evaluating a single iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateIterationOutput {
    /// Whether this iteration passed the quality gate.
    pub passed: bool,

    /// Failures detected in this iteration (empty if passed).
    #[serde(default)]
    pub failures: Vec<TemplateFailure>,

    /// LLM tokens used in this iteration.
    pub llm_tokens_used: u64,

    /// Duration of this iteration in milliseconds.
    pub duration_ms: u64,

    /// Fixes applied from the previous iteration.
    #[serde(default)]
    pub fixes_applied: Vec<String>,
}

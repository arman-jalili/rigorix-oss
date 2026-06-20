//! Data Transfer Objects for the Action Output module.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md
//! Implements: Contract Freeze — DTO schemas for output formatting, annotation writing,
//! step summary writing, and variable setting operations
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for event processing)
//! - Validation constraints are documented in field docs

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::action_output::domain::{
    ExecutionContext, FormattedOutput, StepSummary, WorkflowAnnotation,
};

// ---------------------------------------------------------------------------
// Write Run Output DTOs
// ---------------------------------------------------------------------------

/// Input for writing all outputs for a successful engine run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteRunOutputInput {
    /// The execution context to format and write.
    pub context: ExecutionContext,

    /// Whether to post a PR comment (requires GitHub token).
    pub post_pr_comment: bool,
}

/// Output from writing all outputs for a successful engine run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteRunOutputOutput {
    /// The aggregated formatted output that was written.
    pub output: FormattedOutput,

    /// Number of bytes written to the step summary.
    pub summary_bytes: u64,

    /// Number of annotations emitted.
    pub annotation_count: u32,

    /// Number of output variables set.
    pub variable_count: u32,

    /// Whether a PR comment was posted.
    pub pr_comment_posted: bool,
}

// ---------------------------------------------------------------------------
// Write Validation Failure DTOs
// ---------------------------------------------------------------------------

/// Input for writing outputs for a failed validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteValidationFailureInput {
    /// The execution context for the validation.
    pub context: ExecutionContext,

    /// Flat list of template failures/validation errors as annotations.
    pub failures: Vec<WorkflowAnnotation>,

    /// The execution ID for this validation run.
    pub execution_id: Uuid,

    /// Whether to post a PR comment (requires GitHub token).
    pub post_pr_comment: bool,
}

/// Output from writing validation failure outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteValidationFailureOutput {
    /// The step summary that was written.
    pub summary: StepSummary,

    /// Number of annotations emitted.
    pub annotation_count: u32,

    /// Whether a PR comment was posted.
    pub pr_comment_posted: bool,
}

// ---------------------------------------------------------------------------
// Format Summary DTOs
// ---------------------------------------------------------------------------

/// Input for formatting a summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatSummaryInput {
    /// The execution context to format.
    pub context: ExecutionContext,

    /// Whether to include detailed template content in collapsible sections.
    pub include_details: bool,

    /// Maximum number of failure entries to include inline.
    pub max_inline_failures: Option<u32>,
}

/// Output from formatting a summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatSummaryOutput {
    /// The formatted step summary.
    pub summary: StepSummary,

    /// Length of the rendered markdown in bytes.
    pub rendered_length: u64,
}

// ---------------------------------------------------------------------------
// Format Annotations DTOs
// ---------------------------------------------------------------------------

/// Input for formatting a failure into workflow annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatAnnotationInput {
    /// The context describing the failure.
    pub context: String,

    /// The failure type identifier.
    pub failure_type: String,

    /// Target file path (if applicable).
    pub file: Option<String>,

    /// Target line number (if applicable, 1-indexed).
    pub line: Option<usize>,

    /// Additional structured context for the annotation.
    pub details: Option<serde_json::Value>,
}

/// Output from formatting an annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatAnnotationOutput {
    /// The formatted workflow annotation.
    pub annotation: WorkflowAnnotation,

    /// The rendered workflow command string (for stdout emission).
    pub workflow_command: String,
}

// ---------------------------------------------------------------------------
// Write Annotation DTOs
// ---------------------------------------------------------------------------

/// Input for writing a single annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteAnnotationInput {
    /// The annotation to write.
    pub annotation: WorkflowAnnotation,
}

/// Output from writing an annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteAnnotationOutput {
    /// Number of bytes written to stdout.
    pub bytes_written: u64,
}

// ---------------------------------------------------------------------------
// Write Summary DTOs
// ---------------------------------------------------------------------------

/// Input for writing a step summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteSummaryInput {
    /// The step summary to write.
    pub summary: StepSummary,

    /// Whether to append (true) or overwrite (false).
    /// Appending is useful for accumulating output from multiple formatters.
    /// Overwriting is useful for the final summary.
    pub append: bool,
}

/// Output from writing a step summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteSummaryOutput {
    /// Number of bytes written.
    pub bytes_written: u64,

    /// Number of sections in the summary.
    pub section_count: u32,
}

// ---------------------------------------------------------------------------
// Set Variable DTOs
// ---------------------------------------------------------------------------

/// Input for setting a single output variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetVariableInput {
    /// The variable name.
    pub name: String,

    /// The variable value.
    pub value: String,

    /// Maximum allowed value length (default: 10240 bytes).
    pub max_length: Option<usize>,
}

/// Output from setting an output variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetVariableOutput {
    /// Number of bytes written to `GITHUB_OUTPUT`.
    pub bytes_written: u64,
}

// ---------------------------------------------------------------------------
// Write PR Comment DTOs
// ---------------------------------------------------------------------------

/// Input for posting a PR comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostPrCommentInput {
    /// The PR number to post on.
    pub pr_number: u64,

    /// The comment body (markdown).
    pub body: String,

    /// GitHub token for authentication.
    pub github_token: String,

    /// Repository owner/name (e.g., "owner/repo").
    pub repo: String,

    /// Optional: reply to a specific comment ID.
    pub reply_to_comment_id: Option<u64>,
}

/// Output from posting a PR comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostPrCommentOutput {
    /// The comment ID assigned by GitHub.
    pub comment_id: u64,

    /// The HTML URL of the posted comment.
    pub html_url: String,
}

// ---------------------------------------------------------------------------
// Output Variables DTOs
// ---------------------------------------------------------------------------

/// Input for setting all standard output variables from an execution context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetOutputVariablesInput {
    /// The execution context to extract variables from.
    pub context: ExecutionContext,
}

/// Output from setting all standard output variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetOutputVariablesOutput {
    /// Number of variables set.
    pub variable_count: u32,

    /// Names of the variables that were set.
    pub variable_names: Vec<String>,
}

// ---------------------------------------------------------------------------
// Render DTOs
// ---------------------------------------------------------------------------

/// Input for rendering a step summary to markdown string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderSummaryInput {
    /// The step summary to render.
    pub summary: StepSummary,
}

/// Output from rendering a step summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderSummaryOutput {
    /// The rendered markdown string.
    pub markdown: String,

    /// Length of the rendered markdown in bytes.
    pub length: u64,
}

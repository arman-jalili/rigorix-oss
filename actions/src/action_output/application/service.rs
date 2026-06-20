//! Service interfaces (use cases) for the Action Output bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md
//! Implements: Contract Freeze — OutputFormattingService, AnnotationWritingService,
//! StepSummaryWritingService, OutputVariableService, PrCommentService traits
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for formatting engine
//! results into GitHub Actions-native outputs. All methods are async and
//! return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::action_output::domain::ActionOutputError;

use super::dto::{
    FormatAnnotationInput, FormatAnnotationOutput, FormatSummaryInput, FormatSummaryOutput,
    PostPrCommentInput, PostPrCommentOutput, RenderSummaryInput, RenderSummaryOutput,
    SetOutputVariablesInput, SetOutputVariablesOutput, SetVariableInput, SetVariableOutput,
    WriteAnnotationInput, WriteAnnotationOutput, WriteRunOutputInput, WriteRunOutputOutput,
    WriteSummaryInput, WriteSummaryOutput, WriteValidationFailureInput,
    WriteValidationFailureOutput,
};

/// Top-level service for formatting engine results into GitHub Actions outputs.
///
/// Implements the `OutputFormatter` component from the architecture doc.
/// Orchestrates all output channels: step summary, annotations, output
/// variables, and PR comments. This is the main public interface for the
/// action-output module.
///
/// # Contract (Frozen)
/// - `write_run_output()` is the primary entry point for successful runs
/// - `write_validation_failure()` handles validation failure output
/// - Delegates to specialized services for each output channel
/// - Produces a `FormattedOutput` that aggregates all channels
#[async_trait]
pub trait OutputFormattingService: Send + Sync {
    /// Write all outputs for a successful engine run.
    ///
    /// Produces:
    /// - Step summary with execution plan, file changes, and status
    /// - Output variables for downstream steps
    /// - Optional PR comment (if configured)
    async fn write_run_output(
        &self,
        input: WriteRunOutputInput,
    ) -> Result<WriteRunOutputOutput, ActionOutputError>;

    /// Write all outputs for a failed validation with failure details.
    ///
    /// Produces:
    /// - Step summary with validation report
    /// - Workflow annotations for each failure
    /// - Output variables (status=failed, failure_count=N)
    /// - Optional PR comment with failure summary (if configured)
    async fn write_validation_failure(
        &self,
        input: WriteValidationFailureInput,
    ) -> Result<WriteValidationFailureOutput, ActionOutputError>;

    /// Format an execution context into a step summary (without writing).
    ///
    /// Useful for previewing the summary before writing, or for
    /// producing content for a PR comment body.
    async fn format_summary(
        &self,
        input: FormatSummaryInput,
    ) -> Result<FormatSummaryOutput, ActionOutputError>;

    /// Render a step summary to a markdown string.
    async fn render_summary(
        &self,
        input: RenderSummaryInput,
    ) -> Result<RenderSummaryOutput, ActionOutputError>;

    /// Set all standard output variables from an execution context.
    async fn set_output_variables(
        &self,
        input: SetOutputVariablesInput,
    ) -> Result<SetOutputVariablesOutput, ActionOutputError>;
}

/// Service for writing GitHub Actions workflow annotations.
///
/// Implements the `AnnotationWriter` component from the architecture doc.
/// Emits annotations via workflow commands on stdout:
/// `::<level> file=<path>,line=<line>,col=<col>::<message>`
///
/// # Contract (Frozen)
/// - Annotations are written to stdout (not stderr)
/// - GitHub Actions runner parses workflow commands from stdout
/// - Supported levels: error, warning, notice
/// - Each call writes a single annotation
#[async_trait]
pub trait AnnotationWritingService: Send + Sync {
    /// Write a single workflow annotation to stdout.
    ///
    /// Emits a workflow command in the format:
    /// `::<level> file=<path>,line=<line>,col=<col>,title=<title>::<message>`
    async fn write_annotation(
        &self,
        input: WriteAnnotationInput,
    ) -> Result<WriteAnnotationOutput, ActionOutputError>;

    /// Format a failure description into an annotation (without writing).
    ///
    /// Produces a structured `WorkflowAnnotation` and its rendered
    /// workflow command string.
    async fn format_annotation(
        &self,
        input: FormatAnnotationInput,
    ) -> Result<FormatAnnotationOutput, ActionOutputError>;

    /// Write multiple annotations in batch.
    ///
    /// More efficient than calling `write_annotation` in a loop.
    /// All annotations are written to stdout sequentially.
    async fn write_annotations(
        &self,
        annotations: &[crate::action_output::domain::WorkflowAnnotation],
    ) -> Result<u32, ActionOutputError>;
}

/// Service for writing GitHub Actions step summaries.
///
/// Implements the `StepSummaryWriter` component from the architecture doc.
/// Writes markdown-formatted summaries to the `GITHUB_STEP_SUMMARY` file,
/// which renders in the GitHub Actions UI.
///
/// # Contract (Frozen)
/// - Writes to `GITHUB_STEP_SUMMARY` environment variable path
/// - Supports append and overwrite modes
/// - Content is markdown formatted
/// - Large content is wrapped in `<details>` HTML tags
#[async_trait]
pub trait StepSummaryWritingService: Send + Sync {
    /// Write a step summary to `GITHUB_STEP_SUMMARY`.
    ///
    /// Opens the file at `GITHUB_STEP_SUMMARY` in append or overwrite mode
    /// and writes the rendered markdown content.
    async fn write_summary(
        &self,
        input: WriteSummaryInput,
    ) -> Result<WriteSummaryOutput, ActionOutputError>;

    /// Render a step summary to a markdown string without writing.
    ///
    /// Useful for including the summary content in other outputs
    /// (e.g., PR comment body).
    async fn render_markdown(
        &self,
        summary: &crate::action_output::domain::StepSummary,
    ) -> Result<String, ActionOutputError>;

    /// Check if `GITHUB_STEP_SUMMARY` is available.
    async fn is_available(&self) -> bool;

    /// Get the path to the step summary file.
    async fn get_summary_path(&self) -> Result<String, ActionOutputError>;
}

/// Service for setting GitHub Actions output variables.
///
/// Implements the `OutputVariableWriter` component from the architecture doc.
/// Writes `name=value` pairs to `$GITHUB_OUTPUT` for downstream workflow steps.
///
/// # Contract (Frozen)
/// - Writes to `GITHUB_OUTPUT` environment variable path
/// - Variable names must match `[a-z_][a-z0-9_]*`
/// - Values are sanitized (newlines stripped)
/// - Values are capped at 10KB
#[async_trait]
pub trait OutputVariableService: Send + Sync {
    /// Set a single output variable.
    async fn set_variable(
        &self,
        input: SetVariableInput,
    ) -> Result<SetVariableOutput, ActionOutputError>;

    /// Set all standard output variables from an execution context.
    ///
    /// Sets: execution_id, status, iterations, template_id, quality_level,
    /// failure_count, cumulative_tokens, duration_ms.
    async fn set_from_context(
        &self,
        input: SetOutputVariablesInput,
    ) -> Result<SetOutputVariablesOutput, ActionOutputError>;

    /// Check if `GITHUB_OUTPUT` is available.
    async fn is_available(&self) -> bool;

    /// Get the path to the output file.
    async fn get_output_path(&self) -> Result<String, ActionOutputError>;
}

/// Service for posting PR comments via the GitHub API.
///
/// Implements the `PrCommentWriter` component from the architecture doc.
/// Posts markdown-formatted comments to pull request threads using the
/// GitHub REST API.
///
/// # Contract (Frozen)
/// - Requires a valid GitHub token with `pull_requests: write` scope
/// - Uses GitHub Issues API (`POST /repos/{owner}/{repo}/issues/{number}/comments`)
/// - Supports replying to existing comments (threaded)
/// - Token is never logged
#[async_trait]
pub trait PrCommentService: Send + Sync {
    /// Post a comment to a pull request.
    async fn post_comment(
        &self,
        input: PostPrCommentInput,
    ) -> Result<PostPrCommentOutput, ActionOutputError>;

    /// Format an execution summary as a PR comment body.
    ///
    /// Produces a concise markdown summary suitable for a PR comment.
    /// Full template content is placed in `<details>` sections.
    async fn format_execution_summary(
        &self,
        context: &crate::action_output::domain::ExecutionContext,
    ) -> Result<String, ActionOutputError>;

    /// Format a validation failure summary as a PR comment body.
    async fn format_failure_summary(
        &self,
        context: &crate::action_output::domain::ExecutionContext,
        execution_id: &uuid::Uuid,
    ) -> Result<String, ActionOutputError>;

    /// Check if the GitHub API is accessible with the given token.
    async fn is_api_accessible(&self, token: &str) -> bool;
}

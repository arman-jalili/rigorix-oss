//! Domain types for GitHub Actions output formatting.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md#types
//! Implements: Contract Freeze — FormattedOutput, WorkflowAnnotation, StepSummary,
//! OutputVariable, PrComment, OutputLevel, AnnotationLevel
//! Issue: issue-contract-freeze
//!
//! These are the core domain types that represent formatted outputs,
//! workflow annotations, step summaries, and output variables for
//! GitHub Actions-native display. They serve as the frozen contract
//! that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All formatting logic must live in the application layer (service traits)
//! - All I/O must happen behind repository interfaces
//! - All types are serializable (Serialize + Deserialize) where applicable

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// OutputLevel
// ---------------------------------------------------------------------------

/// The severity level for a workflow command or annotation.
///
/// Maps to GitHub Actions workflow command levels:
/// - `Error` → `::error` — fails the step
/// - `Warning` → `::warning` — creates a warning annotation
/// - `Notice` → `::notice` — creates a notice annotation
/// - `Debug` → (not a workflow command, logged via tracing)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputLevel {
    /// Fatal — fails the step with an error annotation.
    Error,
    /// Non-fatal warning annotation.
    Warning,
    /// Informational notice annotation.
    Notice,
    /// Debug-level information (traced, not annotated).
    Debug,
}

// ---------------------------------------------------------------------------
// WorkflowAnnotation
// ---------------------------------------------------------------------------

/// A single GitHub Actions workflow annotation.
///
/// Annotations appear inline in the GitHub Actions UI at the specified
/// file location. They are emitted via workflow commands on stdout:
/// `::<level> file=<path>,line=<line>,col=<col>::<message>`
///
/// ## Location Semantics
/// - `file` is required — relative path from repo root
/// - `line` is 1-indexed (GitHub Actions convention)
/// - `col` is optional — if omitted, highlights the entire line
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowAnnotation {
    /// The annotation severity level.
    pub level: OutputLevel,

    /// File path relative to repository root.
    pub file: String,

    /// Line number (1-indexed). Use `1` for file-level annotations.
    pub line: usize,

    /// Column number (optional, 1-indexed).
    pub column: Option<usize>,

    /// The annotation title (appears bolded in UI).
    pub title: Option<String>,

    /// Human-readable message describing the issue.
    pub message: String,
}

impl WorkflowAnnotation {
    /// Create a new error annotation.
    pub fn error(file: impl Into<String>, line: usize, message: impl Into<String>) -> Self {
        Self {
            level: OutputLevel::Error,
            file: file.into(),
            line,
            column: None,
            title: None,
            message: message.into(),
        }
    }

    /// Create a new warning annotation.
    pub fn warning(file: impl Into<String>, line: usize, message: impl Into<String>) -> Self {
        Self {
            level: OutputLevel::Warning,
            file: file.into(),
            line,
            column: None,
            title: None,
            message: message.into(),
        }
    }

    /// Create a new notice annotation.
    pub fn notice(file: impl Into<String>, line: usize, message: impl Into<String>) -> Self {
        Self {
            level: OutputLevel::Notice,
            file: file.into(),
            line,
            column: None,
            title: None,
            message: message.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// StepSummary
// ---------------------------------------------------------------------------

/// A GitHub Actions step summary rendered as markdown.
///
/// Step summaries appear in the GitHub Actions UI in a dedicated
/// "Summary" section of the workflow run. They provide a rich,
/// formatted view of execution results.
///
/// ## Sections
/// A summary is composed of multiple sections, each rendered in order.
/// Sections can be collapsed (via `<details>` HTML tags) for large content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepSummary {
    /// The summary title (rendered as H2 heading).
    pub title: String,

    /// Ordered list of summary sections.
    pub sections: Vec<SummarySection>,

    /// Optional footer (rendered after all sections).
    pub footer: Option<String>,
}

/// A single section within a step summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarySection {
    /// Section heading (rendered as H3).
    pub heading: String,

    /// Section body content (markdown).
    pub body: String,

    /// If `true`, render body inside a `<details>` collapsible.
    pub collapsible: bool,

    /// If collapsible, the summary label for the `<details>` tag.
    pub collapsible_label: Option<String>,
}

impl StepSummary {
    /// Create a new step summary with a title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            sections: Vec::new(),
            footer: None,
        }
    }

    /// Add a section to this summary.
    pub fn add_section(&mut self, section: SummarySection) {
        self.sections.push(section);
    }

    /// Set the footer text.
    pub fn set_footer(&mut self, footer: impl Into<String>) {
        self.footer = Some(footer.into());
    }
}

impl SummarySection {
    /// Create a new visible (non-collapsible) section.
    pub fn new(heading: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            heading: heading.into(),
            body: body.into(),
            collapsible: false,
            collapsible_label: None,
        }
    }

    /// Create a new collapsible section.
    pub fn collapsible(
        heading: impl Into<String>,
        body: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            heading: heading.into(),
            body: body.into(),
            collapsible: true,
            collapsible_label: Some(label.into()),
        }
    }
}

// ---------------------------------------------------------------------------
// OutputVariable
// ---------------------------------------------------------------------------

/// A single GitHub Actions output variable.
///
/// Output variables are written to `$GITHUB_OUTPUT` and become available
/// to downstream workflow steps via `steps.<step_id>.outputs.<name>`.
///
/// ## Security
/// - Values are sanitized: newlines are stripped, length is capped at 10KB
/// - Sensitive values (tokens, secrets) MUST NOT be set as output variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputVariable {
    /// The variable name (lowercase, underscores, alphanumeric).
    pub name: String,

    /// The variable value.
    pub value: String,
}

impl OutputVariable {
    /// Create a new output variable.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// Predefined output variable names for downstream workflow steps.
pub mod output_variable_names {
    /// UUID of the execution.
    pub const EXECUTION_ID: &str = "execution_id";
    /// Final execution status: completed, failed, partial_failure.
    pub const STATUS: &str = "status";
    /// Number of validation loop iterations.
    pub const ITERATIONS: &str = "iterations";
    /// ID of the generated/used template.
    pub const TEMPLATE_ID: &str = "template_id";
    /// Achieved quality level.
    pub const QUALITY_LEVEL: &str = "quality_level";
    /// Number of failures (0 on success).
    pub const FAILURE_COUNT: &str = "failure_count";
    /// Total LLM tokens used.
    pub const CUMULATIVE_TOKENS: &str = "cumulative_tokens";
    /// Total execution duration in milliseconds.
    pub const DURATION_MS: &str = "duration_ms";
}

// ---------------------------------------------------------------------------
// PrComment
// ---------------------------------------------------------------------------

/// A PR comment to post via the GitHub API.
///
/// Used for posting execution summaries, validation failure reports,
/// and status updates on pull request threads.
///
/// ## Security
/// - The GitHub token passed via `secrets.GITHUB_TOKEN` is never logged
/// - Full template content is placed in collapsed `<details>` sections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrComment {
    /// The PR number to post on.
    pub pr_number: u64,

    /// The comment body (markdown).
    pub body: String,

    /// Optional: reply to a specific comment ID (thread).
    pub reply_to_comment_id: Option<u64>,
}

impl PrComment {
    /// Create a new PR comment.
    pub fn new(pr_number: u64, body: impl Into<String>) -> Self {
        Self {
            pr_number,
            body: body.into(),
            reply_to_comment_id: None,
        }
    }

    /// Set the comment as a reply to a specific comment.
    pub fn reply_to(mut self, comment_id: u64) -> Self {
        self.reply_to_comment_id = Some(comment_id);
        self
    }
}

// ---------------------------------------------------------------------------
// FormattedOutput
// ---------------------------------------------------------------------------

/// Aggregated output container for a single execution.
///
/// Produced by the top-level `OutputFormatter` and consumed by
/// `action-entrypoint` after engine dispatch completes. Contains
/// all output channels in one struct for atomic writing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattedOutput {
    /// The step summary to write.
    pub summary: Option<StepSummary>,

    /// Workflow annotations to emit (template failures, validation errors).
    pub annotations: Vec<WorkflowAnnotation>,

    /// Output variables for downstream workflow steps.
    pub variables: Vec<OutputVariable>,

    /// PR comment to post (if applicable).
    pub pr_comment: Option<PrComment>,
}

impl FormattedOutput {
    /// Create an empty formatted output.
    pub fn empty() -> Self {
        Self {
            summary: None,
            annotations: Vec::new(),
            variables: Vec::new(),
            pr_comment: None,
        }
    }

    /// Whether this output has any content.
    pub fn is_empty(&self) -> bool {
        self.summary.is_none()
            && self.annotations.is_empty()
            && self.variables.is_empty()
            && self.pr_comment.is_none()
    }
}

// ---------------------------------------------------------------------------
// ExecutionStatus
// ---------------------------------------------------------------------------

/// Final execution status of a rigorix-engine run.
///
/// Used in step summaries and output variables to communicate the
/// outcome of an execution to the user and downstream steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Execution completed successfully.
    Completed,
    /// Execution completed with failures.
    Failed,
    /// Partial success (some steps succeeded, some failed).
    PartialFailure,
}

impl ExecutionStatus {
    /// Human-readable string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ExecutionStatus::Completed => "completed",
            ExecutionStatus::Failed => "failed",
            ExecutionStatus::PartialFailure => "partial_failure",
        }
    }
}

// ---------------------------------------------------------------------------
// ExecutionContext
// ---------------------------------------------------------------------------

/// Context about an execution that needs to be formatted.
///
/// Carries the metadata needed by output formatters to produce
/// properly formatted results. The actual engine result types
/// (`RunOutput`, `ValidationReport`, `TemplateFailure`) are
/// transformed into this context by the application layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Unique execution identifier.
    pub execution_id: uuid::Uuid,

    /// Final execution status.
    pub status: ExecutionStatus,

    /// Number of validation iterations performed.
    pub iterations: u32,

    /// Maximum allowed validation iterations.
    pub max_iterations: u32,

    /// Total LLM tokens consumed.
    pub cumulative_tokens: u64,

    /// Total execution duration in milliseconds.
    pub duration_ms: u64,

    /// Achieved quality level (if applicable).
    pub quality_level: Option<String>,

    /// ID of the template used (if applicable).
    pub template_id: Option<String>,

    /// Number of failures encountered (0 on success).
    pub failure_count: u32,

    /// List of file changes made during execution.
    pub file_changes: Vec<FileChange>,

    /// Step-by-step execution plan with outcomes.
    pub execution_steps: Vec<ExecutionStep>,

    /// Key-value metadata for additional context.
    pub metadata: HashMap<String, String>,
}

/// A single file change made during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// File path relative to workspace root.
    pub path: String,

    /// Type of change.
    pub change_type: FileChangeType,
}

/// Type of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileChangeType {
    /// File was created.
    Created,
    /// File was modified.
    Modified,
    /// File was deleted.
    Deleted,
}

/// A single step in the execution plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Step identifier.
    pub id: String,

    /// Human-readable description.
    pub description: String,

    /// Whether the step succeeded.
    pub success: bool,

    /// Duration of this step in milliseconds.
    pub duration_ms: u64,

    /// Optional error message if step failed.
    pub error: Option<String>,
}

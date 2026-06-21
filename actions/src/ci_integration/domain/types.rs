//! Domain types for the CI Integration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md#types
//! Implements: Contract Freeze — StatusCheckState, PrComment, GitHubStatus, etc.
//! Issue: issue-contract-freeze
//!
//! Core domain types that represent GitHub CI/CD primitives: commit status checks,
//! PR comments, issue labels, and execution tracking. These are the frozen contract
//! that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All types are serializable (Serialize + Deserialize) where applicable

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// StatusCheckState
// ---------------------------------------------------------------------------

/// The state of a GitHub commit status check.
///
/// Maps to GitHub's status check state values:
/// - `Pending` → "pending" (execution in progress)
/// - `Success` → "success" (execution completed, validated)
/// - `Failure` → "failure" (execution completed, failed)
/// - `Error` → "error" (partial recovery or system error)
///
/// ## Source
///
/// Produced by mapping engine execution states to GitHub status check states
/// in `StatusCheckService`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StatusCheckState {
    /// Execution is in progress (maps to GitHub "pending").
    Pending,
    /// Execution completed and validated successfully (maps to GitHub "success").
    Success,
    /// Execution completed but failed validation (maps to GitHub "failure").
    Failure,
    /// Partial recovery or system error (maps to GitHub "error").
    Error,
}

impl StatusCheckState {
    /// Returns the GitHub API string representation of this state.
    pub fn as_github_state(&self) -> &'static str {
        match self {
            StatusCheckState::Pending => "pending",
            StatusCheckState::Success => "success",
            StatusCheckState::Failure => "failure",
            StatusCheckState::Error => "error",
        }
    }

    /// Returns `true` if the status represents a terminal (final) state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, StatusCheckState::Success | StatusCheckState::Failure | StatusCheckState::Error)
    }
}

// ---------------------------------------------------------------------------
// GitHubStatus
// ---------------------------------------------------------------------------

/// Represents a GitHub commit status to be created or updated via the API.
///
/// Sent to `POST /repos/{owner}/{repo}/statuses/{sha}`.
///
/// ## Contract
/// - `state` must be one of: "pending", "success", "failure", "error"
/// - `context` identifies the status check (e.g., "rigorix/execution")
/// - `target_url` links back to the execution details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubStatus {
    /// The status state: "pending", "success", "failure", "error".
    pub state: String,
    /// URL linking to detailed execution output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_url: Option<String>,
    /// Short description of the status.
    pub description: String,
    /// A string label to differentiate this status from others (e.g., "rigorix/execution").
    pub context: String,
}

impl GitHubStatus {
    /// Create a new GitHub status check payload.
    pub fn new(state: impl Into<String>, context: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            state: state.into(),
            target_url: None,
            description: description.into(),
            context: context.into(),
        }
    }

    /// Set a target URL for the status.
    pub fn with_target_url(mut self, url: String) -> Self {
        self.target_url = Some(url);
        self
    }
}

// ---------------------------------------------------------------------------
// PrComment
// ---------------------------------------------------------------------------

/// Type of PR/issue comment managed by `PrCommentService`.
///
/// Distinguishes between execution summary comments and annotation comments
/// that are attached to specific lines of the diff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrCommentType {
    /// Full execution summary comment (sticky — updated in-place).
    ExecutionSummary,
    /// Annotations on specific lines of the PR diff.
    Annotation,
}

// ---------------------------------------------------------------------------
// PrComment
// ---------------------------------------------------------------------------

/// A comment on a GitHub PR or issue.
///
/// Used by `PrCommentService` for the "sticky comment" pattern:
/// identifies the existing rigorix bot comment and updates it in-place
/// rather than posting multiple comments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrComment {
    /// The GitHub comment ID (assigned by GitHub when created).
    pub id: u64,
    /// The issue or PR number this comment belongs to.
    pub issue_number: u64,
    /// The markdown body of the comment.
    pub body: String,
    /// The username of the comment author.
    pub user: String,
    /// The type of comment.
    pub comment_type: PrCommentType,
    /// Whether this is the rigorix bot's own comment.
    pub is_bot_comment: bool,
}

impl PrComment {
    /// Create a new PrComment with the given fields.
    pub fn new(
        id: u64,
        issue_number: u64,
        body: String,
        user: String,
        comment_type: PrCommentType,
    ) -> Self {
        Self {
            id,
            issue_number,
            body,
            user,
            comment_type,
            is_bot_comment: false,
        }
    }

    /// Mark this comment as a bot comment.
    pub fn mark_as_bot(mut self) -> Self {
        self.is_bot_comment = true;
        self
    }
}

// ---------------------------------------------------------------------------
// ExecutionSummary
// ---------------------------------------------------------------------------

/// Structured execution summary posted as a PR comment.
///
/// Contains the execution ID, status, plan steps, validation details,
/// and follow-up instructions. Follows the format defined in the
/// ci-integration architecture doc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// The execution UUID.
    pub execution_id: Uuid,
    /// Overall execution status.
    pub status: ExecutionStatus,
    /// Quality level achieved.
    pub quality: Option<String>,
    /// Per-step execution results.
    pub steps: Vec<ExecutionStep>,
    /// Validation iteration information.
    pub validation: Option<ValidationInfo>,
    /// Follow-up instructions (e.g., retry command).
    pub follow_up: Option<String>,
}

/// Overall execution status for the summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Execution is running.
    Running,
    /// Execution passed all validations.
    Passed,
    /// Execution failed.
    Failed,
    /// Execution partially recovered.
    PartialRecovery,
}

/// A single step in the execution plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Step name.
    pub name: String,
    /// Step status (pass/fail/running).
    pub status: StepStatus,
    /// Duration in seconds.
    pub duration_secs: Option<f64>,
}

/// Status of a single execution step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StepStatus {
    /// Step completed successfully.
    Completed,
    /// Step failed.
    Failed,
    /// Step is currently running.
    Running,
    /// Step was skipped.
    Skipped,
}

/// Validation information for the execution summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationInfo {
    /// Number of validation iterations.
    pub iterations: u32,
    /// Total tokens consumed.
    pub tokens: u64,
    /// Template used for generation.
    pub template: Option<String>,
}

// ---------------------------------------------------------------------------
// BotIdentifier
// ---------------------------------------------------------------------------

/// Marker used to identify rigorix bot comments.
///
/// Embedded as an HTML comment in the markdown body so the bot can
/// find its own comments for the "sticky comment" pattern:
/// `<!-- rigorix-bot -->`
pub const BOT_IDENTIFIER: &str = "<!-- rigorix-bot -->";

// ---------------------------------------------------------------------------
// StatusCheckContext
// ---------------------------------------------------------------------------

/// Well-known context names for status checks.
///
/// Used as the `context` field in `GitHubStatus` to differentiate
/// status checks created by different parts of rigorix.
pub mod status_context {
    /// Status check for engine execution progress.
    pub const EXECUTION: &str = "rigorix/execution";
    /// Status check for validation results.
    pub const VALIDATION: &str = "rigorix/validation";
}

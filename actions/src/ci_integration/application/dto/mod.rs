//! Data Transfer Objects for the CI Integration module.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md
//! Implements: Contract Freeze — DTO schemas for status checks, PR comments,
//! and label management operations
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

use crate::ci_integration::domain::{ExecutionSummary, PrComment, StatusCheckState};

// ---------------------------------------------------------------------------
// Status Check DTOs
// ---------------------------------------------------------------------------

/// Input for creating a pending commit status check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePendingStatusInput {
    /// The full SHA of the commit to attach the status to.
    pub commit_sha: String,
    /// The execution UUID for linking in the target URL.
    pub execution_id: Uuid,
    /// Human-readable status description (e.g., "Rigorix execution in progress").
    pub description: String,
    /// Optional override for the status check context (default: "rigorix/execution").
    pub context_override: Option<String>,
}

/// Output from creating a pending commit status check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePendingStatusOutput {
    /// The context string used for the status check.
    pub context: String,
    /// The state set (always "pending").
    pub state: StatusCheckState,
}

/// Input for updating a commit status check on execution completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatusInput {
    /// The full SHA of the commit.
    pub commit_sha: String,
    /// The execution UUID (for target URL resolution).
    pub execution_id: Uuid,
    /// Serialized representation of the execution outcome.
    /// Concrete type depends on whether the outcome is from the engine
    /// (ValidationReport) or a custom outcome format.
    pub outcome: ExecutionOutcomeDto,
    /// Optional override for the status check context (default: "rigorix/execution").
    pub context_override: Option<String>,
}

/// Serialized execution outcome for status check state determination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOutcomeDto {
    /// Whether execution completed with validated results.
    pub is_validated: bool,
    /// Whether execution failed after all retries.
    pub is_failed: bool,
    /// Whether execution partially recovered.
    pub is_partial_recovery: bool,
    /// Number of validation iterations performed.
    pub iterations: u32,
    /// Human-readable outcome description.
    pub description: String,
}

/// Output from updating a commit status check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatusOutput {
    /// The new status check state.
    pub state: StatusCheckState,
    /// The GitHub API state string ("success", "failure", "error").
    pub github_state: String,
    /// The description used in the status.
    pub description: String,
    /// The context string used.
    pub context: String,
}

// ---------------------------------------------------------------------------
// PR Comment DTOs
// ---------------------------------------------------------------------------

/// Input for upserting an execution summary comment on a PR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertCommentInput {
    /// The GitHub issue or PR number.
    pub issue_number: u64,
    /// The structured execution summary to post as markdown.
    pub summary: ExecutionSummary,
    /// Optional existing comment ID to update (bypasses find-bot-comment step).
    pub existing_comment_id: Option<u64>,
}

/// Output from upserting a PR comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertCommentOutput {
    /// The GitHub comment ID.
    pub comment_id: u64,
    /// Whether a new comment was created (true) or an existing one updated (false).
    pub created: bool,
}

/// Input for finding the rigorix bot comment on a PR/issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindBotCommentInput {
    /// The GitHub issue or PR number.
    pub issue_number: u64,
}

/// Output from searching for the bot comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindBotCommentOutput {
    /// The existing bot comment, if found.
    pub comment: Option<PrComment>,
    /// Whether a bot comment was found.
    pub found: bool,
}

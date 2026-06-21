//! Service interfaces (use cases) for the CI Integration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md#services
//! Implements: Contract Freeze — StatusCheckService, PrCommentService service traits
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for managing GitHub
//! commit status checks, posting PR comments, and applying issue labels.
//! All methods are async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::ci_integration::domain::CiIntegrationError;

use super::dto::{
    CreatePendingStatusInput, CreatePendingStatusOutput, FindBotCommentInput, FindBotCommentOutput,
    UpdateStatusInput, UpdateStatusOutput, UpsertCommentInput, UpsertCommentOutput,
};

// ---------------------------------------------------------------------------
// StatusCheckService
// ---------------------------------------------------------------------------

/// Application service for managing GitHub commit status checks.
///
/// Implements the `StatusCheckManager` contract from the architecture doc.
/// Maps engine execution states to GitHub status check states:
/// - Pending/Running → "pending"
/// - Completed/Validated → "success"
/// - Failed/Exhausted → "failure"
/// - PartialFailure → "error" (with annotations)
///
/// # Contract (Frozen)
/// - `create_pending()` sets status to "pending" when execution starts
/// - `update_status()` transitions to a terminal state on completion
/// - Status check context uses the format `rigorix/{suffix}` (e.g., `rigorix/execution`)
/// - Each status includes a `target_url` linking back to execution details
#[async_trait]
pub trait StatusCheckService: Send + Sync {
    /// Create a pending status check when execution starts.
    ///
    /// Sets the GitHub commit status to "pending" with a descriptive
    /// context label and a link to the execution details.
    ///
    /// # Parameters
    ///
    /// - `commit_sha`: The full SHA of the commit to attach the status to.
    /// - `execution_id`: The execution UUID for linking.
    /// - `description`: Human-readable status description (e.g., "Rigorix execution in progress").
    ///
    /// # Returns
    ///
    /// `CreatePendingStatusOutput` containing the context string used.
    async fn create_pending(
        &self,
        input: CreatePendingStatusInput,
    ) -> Result<CreatePendingStatusOutput, CiIntegrationError>;

    /// Update the status check on completion.
    ///
    /// Transitions the status from "pending" to the appropriate terminal
    /// state based on the execution outcome:
    /// - Validated → "success"
    /// - Failed → "failure"
    /// - PartialRecovery → "error"
    /// - BudgetExhausted → "failure"
    ///
    /// # Parameters
    ///
    /// - `commit_sha`: The full SHA of the commit.
    /// - `execution_id`: The execution UUID (for target URL).
    /// - `outcome_json`: Serialized execution outcome for determining state.
    ///
    /// # Returns
    ///
    /// `UpdateStatusOutput` containing the new state and description.
    async fn update_status(
        &self,
        input: UpdateStatusInput,
    ) -> Result<UpdateStatusOutput, CiIntegrationError>;

    /// Generate the execution details URL for a target_url link.
    ///
    /// Returns a URL that points to execution details (e.g., GitHub Actions run URL).
    async fn execution_url(&self, execution_id: &str) -> String;
}

// ---------------------------------------------------------------------------
// PrCommentService
// ---------------------------------------------------------------------------

/// Application service for posting structured PR review comments.
///
/// Implements the `PrCommentManager` contract from the architecture doc.
/// Uses a "sticky comment" pattern: identifies the existing rigorix bot
/// comment and updates it in-place rather than posting multiple comments.
///
/// # Contract (Frozen)
/// - `upsert()` posts a new comment or updates an existing one
/// - Bot comments are identified by the `BOT_IDENTIFIER` marker
/// - Comments follow a structured markdown format with execution summary
/// - Supports annotation comments for specific diff lines
#[async_trait]
pub trait PrCommentService: Send + Sync {
    /// Post or update the execution summary comment on a PR.
    ///
    /// First searches for an existing rigorix bot comment on the issue/PR.
    /// If found, updates it in-place. If not found, creates a new comment.
    ///
    /// # Parameters
    ///
    /// - `issue_number`: The GitHub issue or PR number.
    /// - `body`: The markdown body to post.
    ///
    /// # Returns
    ///
    /// `UpsertCommentOutput` containing the comment ID and whether it was created or updated.
    async fn upsert(
        &self,
        input: UpsertCommentInput,
    ) -> Result<UpsertCommentOutput, CiIntegrationError>;

    /// Find the existing rigorix bot comment on a PR/issue.
    ///
    /// Searches issue comments for one that contains the `BOT_IDENTIFIER` marker.
    ///
    /// # Returns
    ///
    /// The GitHub comment ID if found, `None` otherwise.
    async fn find_bot_comment(
        &self,
        input: FindBotCommentInput,
    ) -> Result<FindBotCommentOutput, CiIntegrationError>;

    /// Post annotation comments on specific lines of the PR diff.
    ///
    /// Used for inline annotations when validation finds issues.
    async fn post_annotation(
        &self,
        issue_number: u64,
        body: &str,
        commit_sha: &str,
        path: &str,
        line: u32,
    ) -> Result<UpsertCommentOutput, CiIntegrationError>;
}

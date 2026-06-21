//! Event payload schemas for the CI Integration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md
//! Implements: Contract Freeze — CiIntegrationEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted on the EventBus whenever status checks are created,
//! PR comments are posted, or labels are applied. Consumers (audit, console printer)
//! subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - Events are serializable for audit logging

use serde::{Deserialize, Serialize};

/// Events emitted by the CI Integration module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CiIntegrationEvent {
    /// A commit status check was created or updated.
    StatusCheckUpdated {
        /// The commit SHA.
        commit_sha: String,
        /// The status check context (e.g., "rigorix/execution").
        context: String,
        /// The new state (pending, success, failure, error).
        state: String,
        /// Short description of the status.
        description: String,
    },

    /// A PR execution summary comment was posted or updated.
    CommentUpserted {
        /// The issue or PR number.
        issue_number: u64,
        /// The GitHub comment ID.
        comment_id: u64,
        /// Whether this was a new comment (true) or an update (false).
        created: bool,
    },

    /// Labels were applied to an issue or PR based on execution outcome.
    LabelsApplied {
        /// The issue or PR number.
        issue_number: u64,
        /// Labels that were added.
        labels_added: Vec<String>,
        /// Labels that were removed.
        labels_removed: Vec<String>,
    },

    /// A follow-up workflow was triggered.
    WorkflowTriggered {
        /// The execution ID that triggered the workflow.
        execution_id: String,
        /// The workflow file name.
        workflow: String,
        /// Inputs passed to the workflow.
        inputs: std::collections::HashMap<String, serde_json::Value>,
    },

    /// An execution was tracked for idempotency.
    ExecutionTracked {
        /// The execution UUID.
        execution_id: String,
        /// The PR number this execution belongs to.
        pr_number: u64,
        /// Whether this was a duplicate (rejected).
        duplicate: bool,
    },

    /// A CI integration error occurred (non-fatal warning).
    IntegrationWarning {
        /// Operation that produced the warning.
        operation: String,
        /// Warning message.
        message: String,
    },
}

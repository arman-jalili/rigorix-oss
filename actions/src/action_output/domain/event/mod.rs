//! Event payload schemas for the Action Output bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md
//! Implements: Contract Freeze — ActionOutputEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted on the EventBus whenever outputs are written,
//! annotations are emitted, or PR comments are posted. Consumers
//! (audit, console printer) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - Events are serializable for audit logging

use serde::{Deserialize, Serialize};

use crate::action_output::domain::OutputLevel;

/// Events emitted by the Action Output module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionOutputEvent {
    /// Step summary was written to `GITHUB_STEP_SUMMARY`.
    StepSummaryWritten {
        /// Title of the summary.
        title: String,
        /// Number of sections in the summary.
        section_count: u32,
        /// Size of the written content in bytes.
        bytes_written: u64,
    },

    /// Workflow annotation was emitted to stdout.
    AnnotationEmitted {
        /// Annotation severity level.
        level: OutputLevel,
        /// Target file path.
        file: String,
        /// Target line number.
        line: usize,
        /// Whether the annotation had a column scope.
        has_column: bool,
        /// Title of the annotation (if present).
        title: Option<String>,
    },

    /// Output variable was set via `GITHUB_OUTPUT`.
    OutputVariableSet {
        /// Variable name.
        name: String,
        /// Value length in bytes (value itself redacted from logs).
        value_length: u32,
    },

    /// PR comment was posted via GitHub API.
    PrCommentPosted {
        /// The PR number.
        pr_number: u64,
        /// Comment body length in bytes.
        body_length: u32,
        /// Whether this was a reply to an existing comment.
        is_reply: bool,
    },

    /// Full output was written for an execution.
    OutputWritten {
        /// The execution ID.
        execution_id: String,
        /// Execution status.
        status: String,
        /// Whether a step summary was written.
        has_summary: bool,
        /// Number of annotations emitted.
        annotation_count: u32,
        /// Number of output variables set.
        variable_count: u32,
        /// Whether a PR comment was posted.
        has_pr_comment: bool,
    },

    /// An output formatting error occurred (non-fatal warning).
    OutputWarning {
        /// The context where the warning occurred.
        context: String,
        /// Warning message.
        message: String,
    },
}

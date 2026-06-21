//! Event payload schemas for the Action Entrypoint bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md
//! Implements: Contract Freeze — ActionEntrypointEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted on the EventBus whenever the entrypoint dispatches
//! an action, resolves a mode, or encounters an error. Consumers (audit, console
//! printer) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - Events are serializable for audit logging

use serde::{Deserialize, Serialize};

use crate::action_entrypoint::domain::ActionMode;

/// Events emitted by the Action Entrypoint module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionEntrypointEvent {
    /// Action context was built from the environment.
    ContextBuilt {
        /// The resolved execution mode.
        mode: ActionMode,
        /// The triggering event type.
        event_type: String,
        /// Whether a GitHub token was available.
        has_token: bool,
        /// The workspace root path.
        workspace_root: String,
    },

    /// A dispatch was initiated.
    DispatchInitiated {
        /// The execution mode being dispatched.
        mode: ActionMode,
        /// The event type that triggered the dispatch.
        event_type: String,
        /// `Some` if dispatching to the validation loop.
        is_validation: bool,
    },

    /// A dispatch completed successfully.
    DispatchCompleted {
        /// The execution mode that was dispatched.
        mode: ActionMode,
        /// The dispatch status.
        status: String,
        /// Execution ID if one was produced.
        execution_id: Option<String>,
        /// Duration in milliseconds.
        duration_ms: u64,
    },

    /// A dispatch failed with an error.
    DispatchFailed {
        /// The execution mode that was attempted.
        mode: ActionMode,
        /// Error message.
        error: String,
        /// Duration in milliseconds before failure.
        duration_ms: u64,
        /// Whether the error is retriable.
        retriable: bool,
    },

    /// Mode was resolved from inputs/event context.
    ModeResolved {
        /// The resolved mode.
        mode: ActionMode,
        /// The source of resolution (input, event, default).
        source: String,
    },

    /// An unsupported event type was received.
    UnsupportedEventReceived {
        /// The unsupported event name.
        event_name: String,
        /// Available event types for routing.
        supported_types: Vec<String>,
    },
}

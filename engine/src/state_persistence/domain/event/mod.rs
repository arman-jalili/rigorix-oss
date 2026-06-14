//! Event payload schemas for the State Persistence bounded context.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#events
//! Implements: Contract Freeze — StateEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted on the `EventBus` whenever state changes occur
//! — execution state saved, node state changed, graph persisted.
//! Consumers (orchestrator, TUI, audit) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `execution_id` correlates to the originating execution

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Events emitted by the State Persistence module.
///
/// Wrapped in `ExecutionEvent::state_persisted(...)` or similar at the
/// orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StateEvent {
    /// The execution state was saved to disk.
    ///
    /// Emitted after every successful `StateManager::save_state` call.
    StateSaved {
        /// Globally unique execution identifier.
        execution_id: String,
        /// The execution status at the time of save.
        status: String,
        /// Number of node states included in this snapshot.
        node_count: u32,
        /// ISO 8601 timestamp of the save.
        timestamp: DateTime<Utc>,
    },

    /// A node's state was updated within the execution state.
    ///
    /// Emitted when a node transitions status (e.g., Pending → InProgress).
    NodeStateUpdated {
        /// Globally unique execution identifier.
        execution_id: String,
        /// The node ID whose state changed.
        node_id: String,
        /// The previous node status.
        previous_status: String,
        /// The new node status.
        new_status: String,
        /// ISO 8601 timestamp of the update.
        timestamp: DateTime<Utc>,
    },

    /// An execution graph was persisted for TUI history view.
    ///
    /// Emitted when an `ExecutionGraph` is saved to the graph store.
    GraphPersisted {
        /// The execution ID associated with this graph.
        execution_id: String,
        /// Unique identifier for the graph record.
        graph_id: String,
        /// Number of nodes in the persisted graph.
        node_count: u32,
        /// ISO 8601 timestamp of the persistence.
        timestamp: DateTime<Utc>,
    },

    /// An execution record was finalised.
    ///
    /// Emitted when the full `ExecutionRecord` (state + drained events + graph)
    /// has been built and persisted at the end of execution.
    ExecutionRecordFinalised {
        /// Globally unique execution identifier.
        execution_id: String,
        /// Number of events in the record.
        event_count: u32,
        /// The final execution status.
        status: String,
        /// ISO 8601 timestamp of finalisation.
        timestamp: DateTime<Utc>,
    },

    /// A state file was found to be corrupted.
    ///
    /// Emitted when a state file cannot be deserialised, usually during
    /// `StateManager::load_state`.
    StateCorrupted {
        /// The execution ID that was being loaded.
        execution_id: String,
        /// The path to the corrupted state file.
        path: String,
        /// Details about the corruption.
        detail: String,
        /// ISO 8601 timestamp of the error.
        timestamp: DateTime<Utc>,
    },
}

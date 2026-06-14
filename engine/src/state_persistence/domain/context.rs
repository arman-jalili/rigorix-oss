//! ExecutionRecord domain entity — complete execution record.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#record
//! Implements: Contract Freeze — ExecutionRecord schema
//! Issue: issue-contract-freeze
//!
//! The `ExecutionRecord` is the complete record of an execution, built at
//! the end of execution by combining the final `ExecutionState`, all drained
//! events from the `EventBus`, and the `ExecutionGraph`. It is persisted for
//! audit, history, and replay purposes.
//!
//! # Contract (Frozen)
//! - `ExecutionRecord` aggregates state, events, and graph for a single execution
//! - Built by the orchestrator at execution completion
//! - Persisted via a repository for audit trail and TUI history

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::graph::ExecutionGraph;
use super::state::ExecutionStatus;

/// Complete record of an execution, including final state, all events, and graph.
///
/// Built by the orchestrator at execution end by combining:
/// 1. The final `ExecutionState` snapshot
/// 2. All drained `PersistedEvent`s from the `EventBus`
/// 3. The `ExecutionGraph` for TUI history view
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Unique identifier for this record.
    pub record_id: Uuid,

    /// The execution ID this record corresponds to.
    pub execution_id: Uuid,

    /// The human-readable name or description of this execution.
    pub name: String,

    /// The final execution status.
    pub status: ExecutionStatus,

    /// ISO 8601 timestamp when the execution started.
    pub started_at: DateTime<Utc>,

    /// ISO 8601 timestamp when the execution completed.
    pub completed_at: Option<DateTime<Utc>>,

    /// Total execution duration in milliseconds.
    pub total_duration_ms: u64,

    /// SHA-256 hash of the symbol graph at execution start.
    pub symbol_graph_hash: String,

    /// Number of persisted events in this record.
    pub event_count: u32,

    /// Number of nodes in the execution.
    pub node_count: u32,

    /// Number of nodes that completed successfully.
    pub completed_node_count: u32,

    /// Number of nodes that failed.
    pub failed_node_count: u32,

    /// Number of nodes that were skipped.
    pub skipped_node_count: u32,

    /// The execution graph for TUI display.
    pub graph: ExecutionGraph,

    /// ISO 8601 timestamp when this record was created.
    pub created_at: DateTime<Utc>,
}

impl ExecutionRecord {
    /// Create a new ExecutionRecord.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        execution_id: Uuid,
        name: String,
        status: ExecutionStatus,
        started_at: DateTime<Utc>,
        completed_at: Option<DateTime<Utc>>,
        total_duration_ms: u64,
        symbol_graph_hash: String,
        event_count: u32,
        node_count: u32,
        completed_node_count: u32,
        failed_node_count: u32,
        skipped_node_count: u32,
        graph: ExecutionGraph,
    ) -> Self {
        Self {
            record_id: Uuid::new_v4(),
            execution_id,
            name,
            status,
            started_at,
            completed_at,
            total_duration_ms,
            symbol_graph_hash,
            event_count,
            node_count,
            completed_node_count,
            failed_node_count,
            skipped_node_count,
            graph,
            created_at: Utc::now(),
        }
    }
}

//! Event payload schemas for the Code Graph bounded context.
//!
//! @canonical .pi/architecture/modules/code-graph.md#events
//! Implements: Contract Freeze — CodeGraphEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted whenever significant Code Graph lifecycle
//! events occur — graph constructed, node added, graph sealed, analysis
//! completed. Consumers subscribe to these event types for audit,
//! visualization, and integration purposes.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `graph_id` correlates to the originating CodeGraph

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::code_graph::domain::{EdgeKind, NodeKind};

/// Events emitted by the Code Graph module.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CodeGraphEvent {
    /// A new CodeGraph was constructed.
    ///
    /// Emitted after `CodeGraph::new()` is called with metadata.
    GraphCreated {
        /// Globally unique identifier for this graph.
        graph_id: Uuid,
        /// Name of the graph.
        name: String,
        /// The tool or process that created the graph.
        source: String,
        /// Number of modules analyzed to build this graph.
        total_modules_scanned: u64,
        /// ISO 8601 timestamp of creation.
        timestamp: DateTime<Utc>,
    },

    /// A module node was added to the graph.
    ///
    /// Emitted when `CodeGraph::add_node()` succeeds.
    NodeAdded {
        /// Globally unique identifier for this graph.
        graph_id: Uuid,
        /// The ID of the added node.
        node_id: Uuid,
        /// The name of the added module.
        module_name: String,
        /// The kind of node that was added.
        node_kind: NodeKind,
        /// ISO 8601 timestamp of the addition.
        timestamp: DateTime<Utc>,
    },

    /// An edge was added to the graph.
    ///
    /// Emitted when `CodeGraph::add_edge()` succeeds.
    EdgeAdded {
        /// Globally unique identifier for this graph.
        graph_id: Uuid,
        /// The source node ID.
        source_id: Uuid,
        /// The target node ID.
        target_id: Uuid,
        /// The relationship kind.
        edge_kind: EdgeKind,
        /// ISO 8601 timestamp of the addition.
        timestamp: DateTime<Utc>,
    },

    /// The graph was sealed (frozen for analysis).
    ///
    /// Emitted when `CodeGraph::seal()` completes successfully.
    GraphSealed {
        /// Globally unique identifier for this graph.
        graph_id: Uuid,
        /// Number of nodes in the sealed graph.
        node_count: u32,
        /// Number of edges in the sealed graph.
        edge_count: u32,
        /// ISO 8601 timestamp of sealing.
        timestamp: DateTime<Utc>,
    },

    /// Graph analysis was completed.
    ///
    /// Emitted when analysis passes (cycle detection, dependency
    /// resolution, impact analysis) complete.
    AnalysisCompleted {
        /// Globally unique identifier for this graph.
        graph_id: Uuid,
        /// The type of analysis performed.
        analysis_type: String,
        /// Whether the analysis found any issues.
        has_issues: bool,
        /// Number of issues found (if any).
        issue_count: u32,
        /// ISO 8601 timestamp of analysis completion.
        timestamp: DateTime<Utc>,
    },

    /// A cycle was detected during analysis.
    ///
    /// Emitted when cycle detection finds one or more cycles
    /// in the dependency graph.
    CycleDetected {
        /// Globally unique identifier for this graph.
        graph_id: Uuid,
        /// Number of cycles detected.
        cycle_count: u32,
        /// Number of nodes involved in cycles.
        affected_node_count: u32,
        /// ISO 8601 timestamp of detection.
        timestamp: DateTime<Utc>,
    },

    /// The graph was formatted for output.
    ///
    /// Emitted when `CodeGraphFormatter` produces output in
    /// any format (Mermaid, DOT, JSON, text).
    GraphFormatted {
        /// Globally unique identifier for this graph.
        graph_id: Uuid,
        /// The output format used.
        format: String,
        /// Size of the output in characters.
        output_size: u64,
        /// ISO 8601 timestamp of formatting.
        timestamp: DateTime<Utc>,
    },

    /// The graph was persisted to storage.
    ///
    /// Emitted after a successful save operation.
    GraphPersisted {
        /// Globally unique identifier for this graph.
        graph_id: Uuid,
        /// The storage backend used.
        storage_backend: String,
        /// ISO 8601 timestamp of persistence.
        timestamp: DateTime<Utc>,
    },

    /// The graph was loaded from storage.
    ///
    /// Emitted after a successful load operation.
    GraphLoaded {
        /// Globally unique identifier for this graph.
        graph_id: Uuid,
        /// The storage backend used.
        storage_backend: String,
        /// Number of nodes in the loaded graph.
        node_count: u32,
        /// Number of edges in the loaded graph.
        edge_count: u32,
        /// ISO 8601 timestamp of loading.
        timestamp: DateTime<Utc>,
    },

    /// An error occurred during graph operations.
    ///
    /// Emitted when a non-fatal error occurs during graph
    /// construction, analysis, persistence, or formatting.
    GraphError {
        /// Globally unique identifier for this graph (if available).
        graph_id: Option<Uuid>,
        /// The operation that failed.
        operation: String,
        /// The error message.
        error_message: String,
        /// ISO 8601 timestamp of the error.
        timestamp: DateTime<Utc>,
    },
}

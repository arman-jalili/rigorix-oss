//! ViewModel — double-buffered state model for the TUI render loop.
//!
//! @canonical .pi/architecture/modules/tui.md#viewmodel
//! Implements: Contract Freeze — ViewModel component
//! Issue: issue-tui-contract-freeze
//!
//! # Contract (Frozen)
//!
//! Two copies of the ViewModel to eliminate RwLock contention:
//! - `write_buffer`: `tokio::sync::RwLock<TuiViewModel>` — EventBridge writes here
//! - `read_buffer`: `parking_lot::RwLock<TuiViewModel>` — Render loop reads here
//! - Swap happens once per event batch, not per frame

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Execution phase
// ---------------------------------------------------------------------------

/// The current phase of execution the TUI is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionPhase {
    /// No execution loaded — waiting for user input.
    Idle,
    /// Showing plan preview after user typed intent.
    Planning,
    /// Executing the plan — nodes running.
    Executing,
    /// Execution completed successfully.
    Completed,
    /// Execution failed.
    Failed,
    /// Execution was cancelled.
    Cancelled,
}

// ---------------------------------------------------------------------------
// Node state
// ---------------------------------------------------------------------------

/// Status of a single DAG node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Not yet started.
    Pending,
    /// Currently running.
    InProgress,
    /// Completed successfully.
    Completed,
    /// Failed.
    Failed,
    /// Retrying.
    Retrying,
    /// Skipped (dependency failed).
    Skipped,
}

/// A single node in the DAG execution tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeViewModel {
    /// Unique node identifier.
    pub id: String,
    /// Human-readable node name.
    pub name: String,
    /// Tool/operation name.
    pub tool_name: String,
    /// Current node status.
    pub status: NodeStatus,
    /// Dependencies (parent node IDs).
    pub dependencies: Vec<String>,
    /// Dependents (child node IDs).
    pub dependents: Vec<String>,
    /// Execution timing in milliseconds.
    pub timing_ms: Option<u64>,
    /// Truncated output preview.
    pub output_preview: Option<String>,
    /// Error message if failed.
    pub error: Option<String>,
    /// Current retry count.
    pub retry_count: u32,
    /// Risk level string.
    pub risk_level: Option<String>,
}

// ---------------------------------------------------------------------------
// View models
// ---------------------------------------------------------------------------

/// Root TUI state model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiViewModel {
    /// Current execution ID, if any.
    pub execution_id: Option<Uuid>,
    /// Current execution phase.
    pub phase: ExecutionPhase,
    /// Current intent text (from command bar).
    pub intent: Option<String>,
    /// Template ID from planning.
    pub template_id: Option<String>,
    /// DAG nodes keyed by node ID.
    pub nodes: HashMap<String, NodeViewModel>,
    /// Ordered execution event log.
    pub event_log: Vec<EventLogEntry>,
    /// Live metrics counters.
    pub metrics: MetricsViewModel,
    /// LLM budget state.
    pub llm_budget: LlmBudgetViewModel,
    /// Active view identifier.
    pub active_view: ActiveView,
    /// Error message if phase is Failed.
    pub error: Option<String>,
    /// Command bar history (previous intents).
    pub command_bar_history: Vec<String>,
}

impl Default for TuiViewModel {
    fn default() -> Self {
        Self {
            execution_id: None,
            phase: ExecutionPhase::Idle,
            intent: None,
            template_id: None,
            nodes: HashMap::new(),
            event_log: Vec::new(),
            metrics: MetricsViewModel::default(),
            llm_budget: LlmBudgetViewModel::default(),
            active_view: ActiveView::Dashboard,
            error: None,
            command_bar_history: Vec::new(),
        }
    }
}

/// DAG tree structure for node tree rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagViewModel {
    /// All nodes keyed by node ID.
    pub nodes: HashMap<String, NodeViewModel>,
    /// Root node IDs (no dependencies).
    pub root_ids: Vec<String>,
    /// Execution order (topological).
    pub exec_order: Vec<String>,
}

/// Live execution metrics counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsViewModel {
    /// Total LLM API calls made.
    pub llm_calls: u64,
    /// Total tokens consumed.
    pub tokens: u64,
    /// Number of completed nodes.
    pub nodes_completed: u32,
    /// Number of failed nodes.
    pub nodes_failed: u32,
    /// Number of total nodes.
    pub nodes_total: u32,
    /// Throughput (nodes per second).
    pub throughput: f64,
    /// Per-tool execution counts.
    pub tool_counts: HashMap<String, u32>,
}

/// LLM budget state with max/used bars.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmBudgetViewModel {
    /// Maximum allowed LLM calls.
    pub max_calls: Option<u64>,
    /// LLM calls used so far.
    #[serde(default)]
    pub used_calls: u64,
    /// Maximum allowed tokens.
    pub max_tokens: Option<u64>,
    /// Tokens used so far.
    #[serde(default)]
    pub used_tokens: u64,
}

// ---------------------------------------------------------------------------
// Event log
// ---------------------------------------------------------------------------

/// A single entry in the TUI event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogEntry {
    /// Timestamp (millis since epoch).
    pub timestamp_ms: u64,
    /// Event type string.
    pub event_type: String,
    /// Human-readable summary.
    pub summary: String,
    /// Optional detail payload.
    pub detail: Option<String>,
}

// ---------------------------------------------------------------------------
// Active view
// ---------------------------------------------------------------------------

/// Which view is currently active in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ActiveView {
    /// Main execution dashboard (DAG tree + details + metrics).
    #[default]
    Dashboard,
    /// Node list table.
    Nodes,
    /// Event timeline.
    Events,
    /// Plan preview.
    Plan,
    /// Past execution browser.
    History,
    /// Configuration panel.
    Settings,
    /// Template list/show.
    Templates,
    /// LLM clarification requests.
    Clarification,
    /// Plan diff comparison.
    Diff,
}

// ---------------------------------------------------------------------------
// ViewModel mutations (EventBridge output)
// ---------------------------------------------------------------------------

/// Mutation operations on the TuiViewModel.
///
/// The EventBridge converts engine `ExecutionEvent`s into these mutations
/// and applies them to the write buffer.
#[derive(Debug, Clone)]
pub enum ViewModelMutation {
    /// Set the execution phase.
    SetPhase(ExecutionPhase),
    /// Set the execution ID.
    SetExecutionId(Uuid),
    /// Set the intent string.
    SetIntent(String),
    /// Set the template ID.
    SetTemplateId(String),
    /// Add or update a DAG node.
    UpsertNode(NodeViewModel),
    /// Remove a DAG node.
    RemoveNode(String),
    /// Append an event log entry.
    AppendEvent(EventLogEntry),
    /// Update metrics counters.
    UpdateMetrics(MetricsViewModel),
    /// Update LLM budget.
    UpdateLlmBudget(LlmBudgetViewModel),
    /// Set the active view.
    SetActiveView(ActiveView),
    /// Set error message.
    SetError(String),
    /// Add to command bar history.
    PushCommandHistory(String),
    /// Clear all state (reset to defaults).
    Reset,
}

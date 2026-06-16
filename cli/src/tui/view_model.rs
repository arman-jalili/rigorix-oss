//! ViewModel — double-buffered state model for the TUI render loop.
//!
//! @canonical .pi/architecture/modules/tui.md#viewmodel
//! Implements: ViewModel component
//! Issue: issue-viewmodel

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Execution phase
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionPhase {
    Idle,
    Planning,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

// ---------------------------------------------------------------------------
// Node state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Retrying,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeViewModel {
    pub id: String,
    pub name: String,
    pub tool_name: String,
    pub status: NodeStatus,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
    pub timing_ms: Option<u64>,
    pub output_preview: Option<String>,
    pub error: Option<String>,
    pub retry_count: u32,
    pub risk_level: Option<String>,
}

// ---------------------------------------------------------------------------
// View models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiViewModel {
    pub execution_id: Option<Uuid>,
    pub phase: ExecutionPhase,
    pub intent: Option<String>,
    pub template_id: Option<String>,
    pub nodes: HashMap<String, NodeViewModel>,
    pub event_log: Vec<EventLogEntry>,
    pub metrics: MetricsViewModel,
    pub llm_budget: LlmBudgetViewModel,
    pub active_view: ActiveView,
    pub error: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagViewModel {
    pub nodes: HashMap<String, NodeViewModel>,
    pub root_ids: Vec<String>,
    pub exec_order: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsViewModel {
    pub llm_calls: u64,
    pub tokens: u64,
    pub nodes_completed: u32,
    pub nodes_failed: u32,
    pub nodes_total: u32,
    pub throughput: f64,
    pub tool_counts: HashMap<String, u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmBudgetViewModel {
    pub max_calls: Option<u64>,
    #[serde(default)]
    pub used_calls: u64,
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub used_tokens: u64,
}

// ---------------------------------------------------------------------------
// Event log
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogEntry {
    pub timestamp_ms: u64,
    pub event_type: String,
    pub summary: String,
    pub detail: Option<String>,
}

// ---------------------------------------------------------------------------
// Active view
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ActiveView {
    #[default]
    Dashboard,
    Nodes,
    Events,
    Plan,
    History,
    Settings,
    Templates,
    Clarification,
    Diff,
}

// ---------------------------------------------------------------------------
// ViewModel mutations (EventBridge output)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ViewModelMutation {
    SetPhase(ExecutionPhase),
    SetExecutionId(Uuid),
    SetIntent(String),
    SetTemplateId(String),
    UpsertNode(NodeViewModel),
    RemoveNode(String),
    AppendEvent(EventLogEntry),
    UpdateMetrics(MetricsViewModel),
    UpdateLlmBudget(LlmBudgetViewModel),
    SetActiveView(ActiveView),
    SetError(String),
    PushCommandHistory(String),
    Reset,
}

/// Apply a single mutation to the ViewModel in place.
pub fn apply_mutation(vm: &mut TuiViewModel, mutation: ViewModelMutation) {
    match mutation {
        ViewModelMutation::SetPhase(phase) => vm.phase = phase,
        ViewModelMutation::SetExecutionId(id) => vm.execution_id = Some(id),
        ViewModelMutation::SetIntent(intent) => vm.intent = Some(intent),
        ViewModelMutation::SetTemplateId(id) => vm.template_id = Some(id),
        ViewModelMutation::UpsertNode(node) => {
            vm.nodes.insert(node.id.clone(), node);
        }
        ViewModelMutation::RemoveNode(id) => {
            vm.nodes.remove(&id);
        }
        ViewModelMutation::AppendEvent(entry) => {
            vm.event_log.push(entry);
        }
        ViewModelMutation::UpdateMetrics(metrics) => vm.metrics = metrics,
        ViewModelMutation::UpdateLlmBudget(budget) => vm.llm_budget = budget,
        ViewModelMutation::SetActiveView(view) => vm.active_view = view,
        ViewModelMutation::SetError(err) => {
            vm.error = Some(err);
            vm.phase = ExecutionPhase::Failed;
        }
        ViewModelMutation::PushCommandHistory(cmd) => {
            vm.command_bar_history.push(cmd);
        }
        ViewModelMutation::Reset => {
            *vm = TuiViewModel::default();
        }
    }
}

/// Apply a batch of mutations atomically.
pub fn apply_batch(vm: &mut TuiViewModel, mutations: Vec<ViewModelMutation>) {
    for mutation in mutations {
        apply_mutation(vm, mutation);
    }
}

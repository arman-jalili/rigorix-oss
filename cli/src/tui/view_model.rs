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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_phase_is_idle() {
        let vm = TuiViewModel::default();
        assert_eq!(vm.phase, ExecutionPhase::Idle);
    }

    #[test]
    fn test_default_active_view_is_dashboard() {
        let vm = TuiViewModel::default();
        assert_eq!(vm.active_view, ActiveView::Dashboard);
    }

    #[test]
    fn test_apply_set_phase() {
        let mut vm = TuiViewModel::default();
        apply_mutation(
            &mut vm,
            ViewModelMutation::SetPhase(ExecutionPhase::Executing),
        );
        assert_eq!(vm.phase, ExecutionPhase::Executing);
    }

    #[test]
    fn test_apply_set_execution_id() {
        let mut vm = TuiViewModel::default();
        let id = Uuid::new_v4();
        apply_mutation(&mut vm, ViewModelMutation::SetExecutionId(id));
        assert_eq!(vm.execution_id, Some(id));
    }

    #[test]
    fn test_apply_set_intent() {
        let mut vm = TuiViewModel::default();
        apply_mutation(&mut vm, ViewModelMutation::SetIntent("add auth".into()));
        assert_eq!(vm.intent, Some("add auth".to_string()));
    }

    #[test]
    fn test_apply_set_template_id() {
        let mut vm = TuiViewModel::default();
        apply_mutation(
            &mut vm,
            ViewModelMutation::SetTemplateId("add-endpoint".into()),
        );
        assert_eq!(vm.template_id, Some("add-endpoint".to_string()));
    }

    #[test]
    fn test_apply_upsert_node() {
        let mut vm = TuiViewModel::default();
        let node = NodeViewModel {
            id: "n1".into(),
            name: "read-file".into(),
            tool_name: "file_read".into(),
            status: NodeStatus::InProgress,
            dependencies: vec![],
            dependents: vec![],
            timing_ms: None,
            output_preview: None,
            error: None,
            retry_count: 0,
            risk_level: None,
        };
        apply_mutation(&mut vm, ViewModelMutation::UpsertNode(node));
        assert_eq!(vm.nodes.len(), 1);
        assert_eq!(vm.nodes["n1"].name, "read-file");
    }

    #[test]
    fn test_apply_upsert_node_overwrites() {
        let mut vm = TuiViewModel::default();
        let n1 = NodeViewModel {
            id: "n1".into(),
            name: "old".into(),
            tool_name: "t".into(),
            status: NodeStatus::Pending,
            dependencies: vec![],
            dependents: vec![],
            timing_ms: None,
            output_preview: None,
            error: None,
            retry_count: 0,
            risk_level: None,
        };
        let n2 = NodeViewModel {
            id: "n1".into(),
            name: "new".into(),
            tool_name: "t".into(),
            status: NodeStatus::Completed,
            dependencies: vec![],
            dependents: vec![],
            timing_ms: None,
            output_preview: None,
            error: None,
            retry_count: 0,
            risk_level: None,
        };
        apply_mutation(&mut vm, ViewModelMutation::UpsertNode(n1));
        apply_mutation(&mut vm, ViewModelMutation::UpsertNode(n2));
        assert_eq!(vm.nodes["n1"].name, "new");
        assert_eq!(vm.nodes["n1"].status, NodeStatus::Completed);
    }

    #[test]
    fn test_apply_remove_node() {
        let mut vm = TuiViewModel::default();
        let node = NodeViewModel {
            id: "n1".into(),
            name: "x".into(),
            tool_name: "t".into(),
            status: NodeStatus::Pending,
            dependencies: vec![],
            dependents: vec![],
            timing_ms: None,
            output_preview: None,
            error: None,
            retry_count: 0,
            risk_level: None,
        };
        apply_mutation(&mut vm, ViewModelMutation::UpsertNode(node));
        assert_eq!(vm.nodes.len(), 1);
        apply_mutation(&mut vm, ViewModelMutation::RemoveNode("n1".into()));
        assert!(vm.nodes.is_empty());
    }

    #[test]
    fn test_apply_append_event() {
        let mut vm = TuiViewModel::default();
        let entry = EventLogEntry {
            timestamp_ms: 1000,
            event_type: "NodeStarted".into(),
            summary: "n1".into(),
            detail: None,
        };
        apply_mutation(&mut vm, ViewModelMutation::AppendEvent(entry));
        assert_eq!(vm.event_log.len(), 1);
        assert_eq!(vm.event_log[0].event_type, "NodeStarted");
    }

    #[test]
    fn test_apply_update_metrics() {
        let mut vm = TuiViewModel::default();
        let metrics = MetricsViewModel {
            llm_calls: 5,
            tokens: 1000,
            nodes_completed: 3,
            nodes_failed: 0,
            nodes_total: 5,
            throughput: 2.0,
            tool_counts: HashMap::new(),
        };
        apply_mutation(&mut vm, ViewModelMutation::UpdateMetrics(metrics));
        assert_eq!(vm.metrics.llm_calls, 5);
        assert_eq!(vm.metrics.tokens, 1000);
    }

    #[test]
    fn test_apply_update_llm_budget() {
        let mut vm = TuiViewModel::default();
        let budget = LlmBudgetViewModel {
            max_calls: Some(100),
            used_calls: 10,
            max_tokens: Some(10000),
            used_tokens: 500,
        };
        apply_mutation(&mut vm, ViewModelMutation::UpdateLlmBudget(budget));
        assert_eq!(vm.llm_budget.used_calls, 10);
        assert_eq!(vm.llm_budget.max_tokens, Some(10000));
    }

    #[test]
    fn test_apply_set_active_view() {
        let mut vm = TuiViewModel::default();
        apply_mutation(
            &mut vm,
            ViewModelMutation::SetActiveView(ActiveView::Events),
        );
        assert_eq!(vm.active_view, ActiveView::Events);
    }

    #[test]
    fn test_apply_set_error_sets_phase_failed() {
        let mut vm = TuiViewModel::default();
        apply_mutation(
            &mut vm,
            ViewModelMutation::SetError("something broke".into()),
        );
        assert_eq!(vm.phase, ExecutionPhase::Failed);
        assert_eq!(vm.error, Some("something broke".to_string()));
    }

    #[test]
    fn test_apply_push_command_history() {
        let mut vm = TuiViewModel::default();
        apply_mutation(
            &mut vm,
            ViewModelMutation::PushCommandHistory("add auth".into()),
        );
        apply_mutation(
            &mut vm,
            ViewModelMutation::PushCommandHistory("plan deploy".into()),
        );
        assert_eq!(vm.command_bar_history.len(), 2);
        assert_eq!(vm.command_bar_history[0], "add auth");
    }

    #[test]
    fn test_apply_reset_clears_all() {
        let mut vm = TuiViewModel::default();
        apply_mutation(
            &mut vm,
            ViewModelMutation::SetPhase(ExecutionPhase::Completed),
        );
        apply_mutation(&mut vm, ViewModelMutation::SetIntent("test".into()));
        apply_mutation(&mut vm, ViewModelMutation::Reset);
        assert_eq!(vm.phase, ExecutionPhase::Idle);
        assert_eq!(vm.intent, None);
        assert!(vm.nodes.is_empty());
    }

    #[test]
    fn test_apply_batch_multiple_mutations() {
        let mut vm = TuiViewModel::default();
        let id = Uuid::new_v4();
        apply_batch(
            &mut vm,
            vec![
                ViewModelMutation::SetExecutionId(id),
                ViewModelMutation::SetPhase(ExecutionPhase::Executing),
                ViewModelMutation::SetIntent("test".into()),
            ],
        );
        assert_eq!(vm.execution_id, Some(id));
        assert_eq!(vm.phase, ExecutionPhase::Executing);
        assert_eq!(vm.intent, Some("test".to_string()));
    }

    #[test]
    fn test_metrics_defaults() {
        let m = MetricsViewModel::default();
        assert_eq!(m.llm_calls, 0);
        assert_eq!(m.nodes_total, 0);
        assert_eq!(m.throughput, 0.0);
    }

    #[test]
    fn test_llm_budget_defaults() {
        let b = LlmBudgetViewModel::default();
        assert_eq!(b.used_calls, 0);
        assert_eq!(b.max_calls, None);
    }

    #[test]
    fn test_event_log_entry_creation() {
        let entry = EventLogEntry {
            timestamp_ms: 42,
            event_type: "Test".into(),
            summary: "hello".into(),
            detail: Some("world".into()),
        };
        assert_eq!(entry.summary, "hello");
        assert_eq!(entry.detail, Some("world".into()));
    }

    #[test]
    fn test_node_view_model_creation() {
        let node = NodeViewModel {
            id: "n1".into(),
            name: "test-node".into(),
            tool_name: "bash".into(),
            status: NodeStatus::Pending,
            dependencies: vec!["n0".into()],
            dependents: vec![],
            timing_ms: Some(150),
            output_preview: Some("output".into()),
            error: None,
            retry_count: 1,
            risk_level: Some("low".into()),
        };
        assert_eq!(node.name, "test-node");
        assert_eq!(node.timing_ms, Some(150));
        assert_eq!(node.retry_count, 1);
    }

    #[test]
    fn test_execution_phase_ordering() {
        assert!(ExecutionPhase::Idle != ExecutionPhase::Executing);
        assert!(ExecutionPhase::Executing != ExecutionPhase::Completed);
        assert!(ExecutionPhase::Completed != ExecutionPhase::Failed);
    }

    #[test]
    fn test_active_view_default() {
        assert_eq!(ActiveView::default(), ActiveView::Dashboard);
    }
}

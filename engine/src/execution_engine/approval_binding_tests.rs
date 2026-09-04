//! ADR-011 approval-binding wiring tests (opt-in dispatch choke point).
//!
//! @canonical .pi/architecture/modules/approval.md#approvalservice
//! Implements: ISSUE slice — ApprovalService invoked by the runtime dispatch
//!   path: approve captures a record, dispatch verifies the intent hash
//!   (HALT on mismatch — tool never called), and the record is consumed once
//!   on terminal outcome.
//! Issue: #792 wiring (ADR-011 acceptance slice)

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use crate::approval::application::{ApprovalServiceImpl, NodeIntentResolver, ResolvedNode};
use crate::approval::domain::{ApprovalStatus, ExecutionIntent};
use crate::approval::infrastructure::repository::{
    ApprovalRepository, FileBackedApprovalRepository, InMemoryApprovalRepository,
};
use crate::dag_engine::domain::{TaskGraph, TaskNode};
use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
use crate::execution_engine::application::dto::{
    ApproveNodeInput, ExecuteGraphInput, GetExecutionStateInput, HydrateExecutionInput,
    ResumeExecutionInput,
};
use crate::execution_engine::application::service::ParallelExecutionService;
use crate::execution_engine::application::service_impl::{
    ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
};
use crate::execution_engine::domain::{NodeStatus, ParallelExecutorConfig};

// ── Harness ─────────────────────────────────────────────────────────────────

/// Resolver over a fixed (sealed) graph — mirrors what the orchestrator wires
/// for a run: step name / node id → canonical `ExecutionIntent`.
struct GraphResolver {
    by_name: HashMap<String, (Uuid, ExecutionIntent)>,
}

impl GraphResolver {
    fn from_graph(graph: &TaskGraph) -> Self {
        let mut by_name = HashMap::new();
        for node in graph.nodes() {
            by_name.insert(
                node.name.clone(),
                (node.id, ExecutionIntent::from_node(node)),
            );
        }
        Self { by_name }
    }
}

#[async_trait::async_trait]
impl NodeIntentResolver for GraphResolver {
    async fn resolve_by_step_name(&self, step_name: &str) -> Option<ResolvedNode> {
        self.by_name
            .get(step_name)
            .map(|(id, intent)| ResolvedNode {
                node_id: *id,
                step_name: step_name.to_string(),
                intent: intent.clone(),
            })
    }
    async fn resolve_by_node_id(&self, node_id: Uuid) -> Option<ResolvedNode> {
        self.by_name
            .iter()
            .find(|(_, (id, _))| *id == node_id)
            .map(|(name, (id, intent))| ResolvedNode {
                node_id: *id,
                step_name: name.clone(),
                intent: intent.clone(),
            })
    }
}

fn executor_with_binding(
    repo: Arc<dyn ApprovalRepository>,
    resolver: Arc<dyn NodeIntentResolver>,
) -> ParallelExecutionServiceImpl {
    let service = ApprovalServiceImpl::new(
        repo,
        resolver,
        b"engine-run-key".to_vec(),
        Duration::from_secs(3600),
    );
    let executor = ParallelExecutionServiceImpl::new(
        ParallelExecutorConfig::default(),
        Box::new(RetryEvaluationServiceImpl::new()),
        Arc::new(EventBusServiceImpl::default()),
    );
    executor.with_approval_service(Arc::new(service))
}

fn gated_run_node(id: Uuid, name: &str, command: &str) -> TaskNode {
    let intent = serde_json::json!({ "command": command });
    TaskNode::new(id, name, "run_command", vec![], intent.to_string()).with_requires_approval(true)
}

fn marker_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("approval-bound-{tag}.marker"))
}

fn touch_cmd(path: &Path) -> String {
    format!("touch {}", path.display())
}

/// ADR-011 AC #4: approve → identical intent → verified at the choke point →
/// dispatches (tool runs) → single-use record consumed on terminal outcome.
#[tokio::test]
async fn test_bound_approval_dispatches_verified_intent_and_consumes() {
    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let marker = marker_path("dispatch-a");
    let _ = std::fs::remove_file(&marker);
    let graph = {
        let mut g = TaskGraph::new();
        g.add_unchecked(gated_run_node(node_id, "risky_step", &touch_cmd(&marker)))
            .unwrap();
        g.seal().unwrap();
        g
    };

    let repo = Arc::new(InMemoryApprovalRepository::new()) as Arc<dyn ApprovalRepository>;
    let resolver = Arc::new(GraphResolver::from_graph(&graph)) as Arc<dyn NodeIntentResolver>;
    let executor = executor_with_binding(repo.clone(), resolver);

    // First execute pauses at the approval gate (legacy pause semantics).
    let out = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph.clone()),
            config_override: None,
        })
        .await
        .unwrap();
    assert!(out.approval_pending, "gated run pauses before dispatch");

    // Human approves with captured identity → record persisted, node released.
    let approve = executor
        .approve_node(ApproveNodeInput {
            dag_id,
            step_names: vec!["risky_step".to_string()],
            approver_id: Some("tester@org".into()),
            authority: Some("role:operator".into()),
            decision_context: None,
            token_claims_ref: Some("tok-ref".into()),
        })
        .await
        .unwrap();
    assert_eq!(approve.approved, vec!["risky_step".to_string()]);
    let record = repo
        .load(node_id)
        .await
        .expect("repo")
        .expect("record present");
    assert_eq!(record.status, ApprovalStatus::Pending);

    // Resume → dispatch choke point verifies (Matched) → tool runs → consume.
    executor
        .resume_execution(ResumeExecutionInput { dag_id })
        .await
        .unwrap();
    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(
        state.is_complete,
        "verified intent dispatches and completes"
    );
    assert!(
        marker.exists(),
        "tool must execute when the approved intent is verified"
    );
    let record = repo
        .load(node_id)
        .await
        .expect("repo")
        .expect("record present");
    assert_eq!(
        record.status,
        ApprovalStatus::Consumed,
        "single-use record consumed once on terminal outcome"
    );
    let _ = std::fs::remove_file(&marker);
}

/// ADR-011 AC #5: intent mutated between approve and dispatch (cross-process
/// resume against a tampered persisted graph) → HALT before dispatch, node
/// `IntentMismatch`, tool never called; re-approval recovers.
#[tokio::test]
async fn test_tampered_intent_halts_before_dispatch_and_reapproval_recovers() {
    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let store = std::env::temp_dir().join(format!("approval-bound-store-{dag_id}.json"));
    let _ = std::fs::remove_file(&store);

    // ── Process A: original graph, approve, pause (never dispatched) ──
    let marker_a = marker_path(&format!("tamper-{dag_id}-a"));
    let _ = std::fs::remove_file(&marker_a);
    let graph_a = {
        let mut g = TaskGraph::new();
        g.add_unchecked(gated_run_node(node_id, "deploy", &touch_cmd(&marker_a)))
            .unwrap();
        g.seal().unwrap();
        g
    };
    let repo_a = Arc::new(FileBackedApprovalRepository::open(&store).unwrap())
        as Arc<dyn ApprovalRepository>;
    let executor_a = executor_with_binding(
        repo_a,
        Arc::new(GraphResolver::from_graph(&graph_a)) as Arc<dyn NodeIntentResolver>,
    );
    executor_a
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph_a.clone()),
            config_override: None,
        })
        .await
        .unwrap();
    let approve_a = executor_a
        .approve_node(ApproveNodeInput {
            dag_id,
            step_names: vec!["deploy".to_string()],
            approver_id: Some("tester@org".into()),
            authority: None,
            decision_context: None,
            token_claims_ref: None,
        })
        .await
        .unwrap();
    assert_eq!(approve_a.approved, vec!["deploy".to_string()]);
    let state_a = executor_a
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    // A pauses with the approval granted but the node never dispatched.

    // ── Process B: persisted store + a TAMPERED graph (upstream change) ──
    let marker_b = marker_path(&format!("tamper-{dag_id}-b"));
    let _ = std::fs::remove_file(&marker_b);
    let graph_tampered = {
        let mut g = TaskGraph::new();
        g.add_unchecked(gated_run_node(
            node_id,
            "deploy",
            &touch_cmd(&marker_b), // mutated intent
        ))
        .unwrap();
        g.seal().unwrap();
        g
    };
    let repo_b = Arc::new(FileBackedApprovalRepository::open(&store).unwrap())
        as Arc<dyn ApprovalRepository>;
    let executor_b = executor_with_binding(
        repo_b.clone(),
        Arc::new(GraphResolver::from_graph(&graph_tampered)) as Arc<dyn NodeIntentResolver>,
    );
    // Cross-process resume: B hydrates with the persisted approved set and the
    // record file written by A — the OLD approval now must NOT authorize the
    // tampered dispatch.
    executor_b
        .hydrate_execution(HydrateExecutionInput {
            dag_id,
            graph: graph_tampered.clone(),
            node_states: state_a.node_states.clone(),
            approved: HashSet::from([node_id]),
            started_at: state_a.started_at.unwrap_or_else(chrono::Utc::now),
        })
        .await
        .unwrap();

    let resume = executor_b
        .resume_execution(ResumeExecutionInput { dag_id })
        .await
        .unwrap();
    assert_eq!(resume.dag_id, dag_id);

    // HALT: node in IntentMismatch, nothing dispatched.
    let state_b = executor_b
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    let node_state = state_b.node_states.get(&node_id).expect("node present");
    assert_eq!(
        node_state.status,
        NodeStatus::IntentMismatch,
        "tampered intent must halt the node into IntentMismatch"
    );
    assert!(
        !state_b.is_complete,
        "mismatched intent must not complete the run"
    );
    assert!(
        !marker_a.exists(),
        "tool must never be called (marker A absent)"
    );
    assert!(
        !marker_b.exists(),
        "tool must never be called (marker B absent)"
    );

    // Re-approval (new record bound to the CURRENT graph) → dispatch proceeds.
    let reapprove = executor_b
        .approve_node(ApproveNodeInput {
            dag_id,
            step_names: vec!["deploy".to_string()],
            approver_id: Some("tester@org".into()),
            authority: None,
            decision_context: None,
            token_claims_ref: None,
        })
        .await
        .unwrap();
    assert_eq!(reapprove.approved, vec!["deploy".to_string()]);
    executor_b
        .resume_execution(ResumeExecutionInput { dag_id })
        .await
        .unwrap();
    let state_c = executor_b
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(
        state_c.is_complete,
        "re-approval against current intent completes"
    );
    assert!(
        marker_b.exists(),
        "re-approved intent dispatches (marker B present)"
    );

    let _ = std::fs::remove_file(&store);
    let _ = std::fs::remove_file(&marker_a);
    let _ = std::fs::remove_file(&marker_b);
}

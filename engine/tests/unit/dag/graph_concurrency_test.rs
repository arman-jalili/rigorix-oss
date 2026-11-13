#![cfg(not(feature = "portable"))]
//! The two graph properties #1526 did not prove (#1537).
//!
//! @canonical .pi/architecture/modules/dag-engine.md#parallelism
//! @canonical .pi/architecture/modules/dag-engine.md#join
//!
//! # The seam
//!
//! The executor does not consult `ToolRegistryService`. `service_impl/tools.rs:187-211` dispatches
//! over a fixed `match tool_name` whose arms are private `exec_*` methods and whose fallback is
//! `unknown_tool`, and `grep -rn "ToolRegistry" engine/src/execution_engine/` returns nothing. A
//! `Tool` registered through the registry is never reached from `execute_graph`, so a `RecordingTool`
//! would record nothing — the seam this test uses is the **event stream the executor already
//! publishes**, and the work is done by a real `run_command` step whose `sleep` gives the interval
//! something to intersect.
//!
//! # What makes the concurrency assertion evidence
//!
//! Two nodes can look concurrent if a test compares only start times, and an assertion that passes
//! on a chain certifies nothing. So every test here is paired: the diamond must **intersect**, and
//! the same four steps as a chain — what a plan with no `depends_on` produces (#1526's frozen
//! sequential contract) — must **not**. The chain case is not a comment describing what a failure
//! would look like; it is a test that runs, and the mutation this factory was shown to catch is
//! "the graph was flattened into a chain".
//!
//! Durations are never asserted. The only facts used are the order and overlap of the events'
//! timestamps, so a slow machine changes nothing and a fast one cannot manufacture an intersection
//! that does not exist.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use rigorix_engine::approval::application::dto::{ApproveInput, ApproveOutput};
use rigorix_engine::approval::application::service::{ApprovalService, IntentVerification};
use rigorix_engine::approval::domain::{ApprovalError, ApprovalRecord, ScopeViolation};
use rigorix_engine::dag_engine::domain::{TaskGraph, TaskNode};
use rigorix_engine::event_system::application::dto::DrainPersistedInput;
use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
use rigorix_engine::event_system::application::service::EventBusService;
use rigorix_engine::event_system::domain::ExecutionEvent;
use rigorix_engine::execution_engine::application::dto::{
    ApproveNodeInput, ExecuteGraphInput, ResumeExecutionInput,
};
use rigorix_engine::execution_engine::application::service::ParallelExecutionService;
use rigorix_engine::execution_engine::application::service_impl::{
    ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
};
use rigorix_engine::execution_engine::domain::ParallelExecutorConfig;
use rigorix_engine::permission::application::enforcer_impl::PermissionEnforcerImpl;
use rigorix_engine::permission::domain::PermissionMode;
use rigorix_engine::permission::domain::policy::PermissionPolicy;
use rigorix_engine::tools::application::shell_policy::SandboxDeclaration;
use uuid::Uuid;

/// Long enough that two steps dispatched into the same wave overlap on any scheduler, short enough
/// that the pair of fixtures costs under a second. The tests assert intersection, not a duration.
const BRANCH_COMMAND: &str = "sleep 0.4";
const TRIVIAL_COMMAND: &str = "true";

/// A node whose work takes measurable wall-clock time.
fn step(id: Uuid, name: &str, deps: Vec<Uuid>, command: &str) -> TaskNode {
    TaskNode::new(
        id,
        name,
        "run_command",
        deps,
        format!(r#"{{"command": "{command}"}}"#),
    )
}

struct Shape {
    a: Uuid,
    b: Uuid,
    c: Uuid,
    d: Uuid,
}

/// `a` → `b`, `c` → `d`: two independent branches between a fork and a join.
fn diamond(approve_join: bool) -> (TaskGraph, Shape) {
    let (a, b, c, d) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let mut graph = TaskGraph::new();
    graph
        .add_unchecked(step(a, "a", vec![], TRIVIAL_COMMAND))
        .unwrap();
    graph
        .add_unchecked(step(b, "b", vec![a], BRANCH_COMMAND))
        .unwrap();
    graph
        .add_unchecked(step(c, "c", vec![a], BRANCH_COMMAND))
        .unwrap();
    let join = step(d, "d", vec![b, c], TRIVIAL_COMMAND);
    let join = if approve_join {
        join.with_requires_approval(true)
    } else {
        join
    };
    graph.add_unchecked(join).unwrap();
    graph.seal().unwrap();
    (graph, Shape { a, b, c, d })
}

/// The same four steps with no dependencies between them — the chain a plan without `depends_on`
/// produces, which is the case the diamond must be distinguishable from.
fn chain() -> (TaskGraph, Shape) {
    let (a, b, c, d) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let mut graph = TaskGraph::new();
    graph
        .add_unchecked(step(a, "a", vec![], TRIVIAL_COMMAND))
        .unwrap();
    graph
        .add_unchecked(step(b, "b", vec![a], BRANCH_COMMAND))
        .unwrap();
    graph
        .add_unchecked(step(c, "c", vec![b], BRANCH_COMMAND))
        .unwrap();
    graph
        .add_unchecked(step(d, "d", vec![c], TRIVIAL_COMMAND))
        .unwrap();
    graph.seal().unwrap();
    (graph, Shape { a, b, c, d })
}

/// An approval service that grants whatever it is asked to grant.
///
/// It is a test double for a trait the tree already has, and it is unconditional on purpose: this
/// test is about **when** a join is dispatched relative to its approval, not about how an approval
/// is decided. The production binding's job — bind an intent hash, capture an identity, refuse an
/// unbound approval — is exercised where it lives; here it would only mask the scheduling question,
/// which is the same reason the fixture declares a permissive permission mode and a sandbox.
struct GrantsWhateverIsAsked;

#[async_trait::async_trait]
impl ApprovalService for GrantsWhateverIsAsked {
    async fn approve(&self, input: ApproveInput) -> Result<ApproveOutput, ApprovalError> {
        Ok(ApproveOutput {
            dag_id: input.dag_id,
            approved: input.step_names.clone(),
            not_found: Vec::new(),
            still_pending: Vec::new(),
            approval_records: Vec::new(),
        })
    }

    async fn verify_intent(&self, _node_id: Uuid) -> Result<IntentVerification, ApprovalError> {
        Ok(IntentVerification::Matched)
    }

    async fn consume(&self, _node_id: Uuid) -> Result<(), ApprovalError> {
        Ok(())
    }

    async fn record_scope_violation(
        &self,
        _violation: ScopeViolation,
    ) -> Result<(), ApprovalError> {
        Ok(())
    }

    async fn get_approval(&self, _node_id: Uuid) -> Result<Option<ApprovalRecord>, ApprovalError> {
        Ok(None)
    }
}

/// The cap under test. Set explicitly: at the default of 4 the diamond would run its branches in
/// one wave whether or not the graph said they were independent, which is the confound #1526's
/// resolver exists to remove.
fn executor() -> (ParallelExecutionServiceImpl, Arc<EventBusServiceImpl>) {
    let bus = Arc::new(EventBusServiceImpl::default());
    let config = ParallelExecutorConfig {
        max_concurrent_executions: 2,
        ..Default::default()
    };
    let executor = ParallelExecutionServiceImpl::new(
        config,
        Box::new(RetryEvaluationServiceImpl::new()),
        bus.clone(),
    )
    .with_permission_enforcer(Arc::new(PermissionEnforcerImpl::new(
        PermissionPolicy::default_with_mode(PermissionMode::DangerousFullAccess),
        "/",
    )))
    // #1487: every executing step kind needs a declared regime, or it is refused with
    // `sandbox_undeclared` before it can be scheduled — and then this test would measure the gate,
    // not the graph.
    .with_sandbox_declaration(Arc::new(SandboxDeclaration::container()))
    // The binding is what makes an approval possible at all: without it `approve_node` refuses with
    // "approval binding is not armed", which is the right refusal and the wrong subject here.
    .with_approval_service(Arc::new(GrantsWhateverIsAsked));
    (executor, bus)
}

#[derive(Default)]
struct Interval {
    started: Option<DateTime<Utc>>,
    finished: Option<DateTime<Utc>>,
}

impl Interval {
    fn start_us(&self) -> i64 {
        self.started
            .expect("the node started; an interval without a start is not an interval")
            .timestamp_micros()
    }

    fn finish_us(&self) -> i64 {
        self.finished.expect("the node finished").timestamp_micros()
    }
}

/// Per-node intervals from the events the executor publishes. `epoch` names the execution, so a
/// drain that still holds an earlier run's events cannot be read as this one's.
async fn intervals(
    bus: &EventBusServiceImpl,
    dag_id: Uuid,
) -> (HashMap<String, Interval>, Vec<String>) {
    let drained = bus
        .drain_persisted(DrainPersistedInput { clear: true })
        .await
        .expect("drain persisted events");
    let mut out: HashMap<String, Interval> = HashMap::new();
    let mut sequence: Vec<String> = Vec::new();
    for persisted in &drained.events {
        match &persisted.event {
            ExecutionEvent::NodeStarted {
                execution_id,
                node_id,
                node_name: _,
                timestamp,
            } if execution_id == &dag_id => {
                out.entry(node_id.clone()).or_default().started = Some(*timestamp);
                sequence.push(format!("start:{node_id}"));
            }
            ExecutionEvent::NodeCompleted {
                execution_id,
                node_id,
                timestamp,
                ..
            } if execution_id == &dag_id => {
                out.entry(node_id.clone()).or_default().finished = Some(*timestamp);
                sequence.push(format!("finish:{node_id}"));
            }
            _ => {}
        }
    }
    (out, sequence)
}

/// Microseconds of overlap, negative when the two intervals are disjoint by that much.
fn overlap(a: &Interval, b: &Interval) -> i64 {
    a.finish_us().min(b.finish_us()) - a.start_us().max(b.start_us())
}

async fn run(
    graph: TaskGraph,
    dag_id: Uuid,
) -> (ParallelExecutionServiceImpl, Arc<EventBusServiceImpl>) {
    let (executor, bus) = executor();
    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .expect("the graph executed");
    assert_eq!(
        output.result.failed_count, 0,
        "fixture must not fail on its own: {:?}",
        output.result
    );
    (executor, bus)
}

// ─────────────────────────────────────────────────────────────────────────────
// The diamond intersects; the chain of the same steps does not.
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn graph_concurrency_diamond_branches_intersect() {
    let dag_id = Uuid::new_v4();
    let (graph, shape) = diamond(false);
    let (_executor, bus) = run(graph, dag_id).await;
    let (intervals, sequence) = intervals(&bus, dag_id).await;

    let (b, c) = (
        intervals
            .get(&shape.b.to_string())
            .expect("b has an interval"),
        intervals
            .get(&shape.c.to_string())
            .expect("c has an interval"),
    );

    assert!(
        overlap(b, c) > 0,
        "the two branches of a diamond must run concurrently, but their intervals are disjoint \
         (b {}..{}, c {}..{}, overlap {}µs). Sequence: {sequence:?}",
        b.start_us(),
        b.finish_us(),
        c.start_us(),
        c.finish_us(),
        overlap(b, c)
    );

    // The fork is not decoration: neither branch may start before `a` is done, or "concurrent
    // branches" would describe nodes that were free to run all along.
    let a = intervals
        .get(&shape.a.to_string())
        .expect("a has an interval");
    assert!(
        b.start_us() >= a.finish_us() && c.start_us() >= a.finish_us(),
        "a branch started before its dependency finished (a ends {}µs, b starts {}µs, c starts {}µs)",
        a.finish_us(),
        b.start_us(),
        c.start_us()
    );

    // The join waits for both, which is the other half of what makes this a diamond rather than
    // two independent runs that happened to overlap.
    let d = intervals
        .get(&shape.d.to_string())
        .expect("d has an interval");
    assert!(
        d.start_us() >= b.finish_us().max(c.finish_us()),
        "the join started at {}µs, before both branches finished ({}µs, {}µs)",
        d.start_us(),
        b.finish_us(),
        c.finish_us()
    );
}

#[tokio::test]
async fn graph_concurrency_chain_of_the_same_steps_does_not_intersect() {
    let dag_id = Uuid::new_v4();
    let (graph, shape) = chain();
    let (_executor, bus) = run(graph, dag_id).await;
    let (intervals, sequence) = intervals(&bus, dag_id).await;

    let (b, c) = (
        intervals
            .get(&shape.b.to_string())
            .expect("b has an interval"),
        intervals
            .get(&shape.c.to_string())
            .expect("c has an interval"),
    );

    assert!(
        overlap(b, c) <= 0,
        "this fixture is the control: the same four steps as a chain must NOT overlap, or the \
         diamond assertion above would pass on a chain too (b {}..{}, c {}..{}, overlap {}µs). \
         Sequence: {sequence:?}",
        b.start_us(),
        b.finish_us(),
        c.start_us(),
        c.finish_us(),
        overlap(b, c)
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// The join pauses before it is dispatched, not after.
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn graph_concurrency_join_is_not_dispatched_before_approval() {
    let dag_id = Uuid::new_v4();
    let (graph, shape) = diamond(true);

    let (executor, bus) = executor();
    let paused = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .expect("the graph ran to its pause");

    assert!(
        paused.approval_pending,
        "a join with `requires_approval` must pause the run, got {paused:?}"
    );
    assert!(
        paused.pending_approval_steps.iter().any(|s| s == "d"),
        "the pause must name the join, got {:?}",
        paused.pending_approval_steps
    );

    // The assertion is the **absence** of a dispatch, not a state name: a session can read
    // `Paused` after the node was already spawned, and that is the bug this catches.
    let (before, sequence) = intervals(&bus, dag_id).await;
    assert!(
        !before.contains_key(&shape.d.to_string()),
        "the join was dispatched before approval — it has an interval while the run is paused. \
         Sequence: {sequence:?}"
    );

    executor
        .approve_node(ApproveNodeInput {
            dag_id,
            step_names: vec!["d".to_string()],
            approver_id: Some("tester".to_string()),
            authority: None,
            decision_context: None,
            token_claims_ref: None,
        })
        .await
        .expect("the join is approved");
    executor
        .resume_execution(ResumeExecutionInput { dag_id })
        .await
        .expect("the run resumes");

    let (after, sequence) = intervals(&bus, dag_id).await;
    let d = after
        .get(&shape.d.to_string())
        .expect("the join has an interval after approval");
    assert!(
        d.finished.is_some(),
        "the join started after approval but did not finish. Sequence: {sequence:?}"
    );
}

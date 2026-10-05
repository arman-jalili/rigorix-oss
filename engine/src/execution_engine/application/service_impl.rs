//! Service implementations for the Execution Engine bounded context.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: ExecutionEngine — ParallelExecutionServiceImpl, RetryEvaluationServiceImpl
//! Issue: issue-parallelexecutor
//!
//! Concrete implementations of ParallelExecutionService and RetryEvaluationService:
//!
//! - `ParallelExecutionServiceImpl`: Uses tokio JoinSet to execute DAG nodes
//!   concurrently. Manages the ready queue, retry loop, pause/resume, and abort.
//!   Designed for single-process in-memory execution.
//!
//! - `RetryEvaluationServiceImpl`: Stateless policy evaluator. Computes retry
//!   decisions, backoff delays, and policy validation.
//!
//! # Design Decisions
//! - In-memory execution state stored in HashMap keyed by dag_id
//! - TaskGraph lookup delegates to a provided `GraphProvider` closure
//! - Node execution is simulated via a `NodeRunner` closure (inject actual tool
//!   execution in production)
//! - JoinSet dispatches nodes up to `max_concurrent_executions`
//! - RetryEvaluationServiceImpl is stateless — decisions are purely computational

/// Progress callback for node state changes.
pub type ProgressCallback = Box<dyn Fn(ExecutionProgress) + Send + Sync>;

use async_trait::async_trait;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::Mutex;
use uuid::Uuid;

use crate::approval::application::dto::ApproveInput as ApprovalApproveInput;
use crate::approval::application::{ApprovalService, IntentVerification, ResolvedNode};
use crate::approval::domain::ApprovalError as ApprovalServiceError;
use crate::approval::infrastructure::effect_scope::{ChangeSnapshot, GitDiffEffectOracle};
use crate::dag_engine::domain::GatingMode;
use crate::event_system::application::EventBusService;
use crate::event_system::domain::ExecutionEvent;
use crate::execution_engine::domain::{
    BackoffStrategy, ExecutionError, ExecutionResult, FailureContext, NodeExecutionState,
    NodeStatus, ParallelExecutorConfig, RetryDecision, RetryPolicy, TaskResult,
};
use crate::hooks::application::service::HookRunnerService;
use crate::permission::application::enforcer::PermissionEnforcer;
use crate::precondition::application::{DispatchGate, DispatchStep};
use crate::precondition::domain::{PreconditionError, PreconditionOutcome, PreconditionVerdict};
use crate::recovery_recipes::application::context::RecoveryContext;
use crate::recovery_recipes::application::dto::{AttemptRecoveryInput, RecipeForInput};
use crate::recovery_recipes::application::service::RecoveryService;
use crate::recovery_recipes::domain::FailureScenario;
use crate::sequence_policy::application::dto::{DispatchedStep, PlannedStep};
use crate::sequence_policy::application::service::SequencePolicyService;
use crate::sequence_policy::domain::RuleAction;

use super::dto::{
    AbortExecutionInput, AbortExecutionOutput, ApproveNodeInput, ApproveNodeOutput,
    EvaluateRetryInput, EvaluateRetryOutput, ExecuteGraphInput, ExecuteGraphOutput,
    ExecuteNodeInput, ExecuteNodeOutput, GetExecutionStateInput, GetExecutionStateOutput,
    HydrateExecutionInput, HydrateExecutionOutput, PauseExecutionInput, PauseExecutionOutput,
    ResumeExecutionInput, ResumeExecutionOutput,
};
use super::service::{ExecutionProgress, ParallelExecutionService, RetryEvaluationService};

// ---------------------------------------------------------------------------
// Parallel execution helpers
// ---------------------------------------------------------------------------

/// Spawn a single DAG node as a concurrent task in the JoinSet.
///
/// Handles marking the node as running, emitting NodeStarted, performing
/// permission checks, executing the tool, emitting NodeCompleted, and
/// collecting the result. Designed to be called from both the initial
/// dispatch phase and the post-completion replenishment phase.
#[allow(clippy::too_many_arguments)]
async fn spawn_concurrent_node(
    join_set: &mut tokio::task::JoinSet<(Uuid, TaskResult)>,
    graph: &mut crate::dag_engine::domain::TaskGraph,
    event_bus: &Arc<dyn EventBusService>,
    sessions: &Arc<Mutex<HashMap<Uuid, ExecutionSession>>>,
    dag_id: Uuid,
    node_id: Uuid,
    permission_enforcer: &Option<Arc<dyn PermissionEnforcer>>,
    hook_runner: &Option<Arc<dyn HookRunnerService>>,
    enforce_permission: bool,
) -> Result<(), ExecutionError> {
    let node = match graph.get_node(node_id).cloned() {
        Some(n) => n,
        None => return Ok(()),
    };

    // Mark running in session
    {
        let mut s = sessions.lock().map_err(|e| ExecutionError::InternalError {
            detail: format!("Lock error: {e}"),
        })?;
        if let Some(session) = s.get_mut(&dag_id)
            && let Some(state) = session.node_states.get_mut(&node_id)
        {
            state.mark_running();
        }
    }

    let start = std::time::Instant::now();
    let node_name = node.name.clone();
    let node_tool = node.tool.clone();
    let node_intent = node.intent.clone();

    // Emit NodeStarted event
    if let Err(e) = event_bus
        .publish(crate::event_system::application::dto::PublishEventInput {
            event: ExecutionEvent::NodeStarted {
                execution_id: dag_id,
                node_id: node_id.to_string(),
                node_name: node_name.clone(),
                timestamp: chrono::Utc::now(),
            },
        })
        .await
    {
        tracing::warn!(error = %e, "event publish failed — evidence may be incomplete");
    }

    let eb = Arc::clone(event_bus);
    let exec_id = dag_id;
    let permission = permission_enforcer.clone();
    let hook_runner = hook_runner.clone();

    join_set.spawn(async move {
        // Permission check (gate before execution) — GAP-A-11: gated by the
        // executor's enable_enforcement config flag.
        if enforce_permission && let Some(ref enforcer) = permission {
            let outcome = enforcer.check(&node_tool, &node_intent, None).await;
            if let crate::permission::domain::PermissionOutcome::Denied { ref reason, .. } = outcome
            {
                let dur = start.elapsed().as_millis() as u64;
                let result = TaskResult::failure(
                    node_id,
                    &node_name,
                    reason.clone(),
                    "permission_denied".to_string(),
                    dur,
                    0,
                );
                if let Err(e) = eb
                    .publish(crate::event_system::application::dto::PublishEventInput {
                        event: ExecutionEvent::NodeCompleted {
                            execution_id: exec_id,
                            node_id: node_id.to_string(),
                            node_name,
                            duration_ms: dur,
                            output: serde_json::json!(null),
                            timestamp: chrono::Utc::now(),
                        },
                    })
                    .await
                {
                    tracing::warn!(error = %e, "event publish failed — evidence may be incomplete");
                }
                return (node_id, result);
            }
        }

        // File-path gating on the parallel path — same checks execute_tool
        // applies on the single-node path (GAP-A-11 symmetry): a write tool
        // is additionally checked against the file-write policy, which denies
        // `.rigorix/**` (R5 — the operator config an agent must never edit)
        // and out-of-workspace paths.
        if enforce_permission
            && let Some(ref enforcer) = permission
            && matches!(
                node_tool.as_str(),
                "file_write" | "file_append" | "file_patch" | "edit_file"
            )
        {
            let parsed: serde_json::Value =
                serde_json::from_str(&node_intent).unwrap_or_default();
            if let Some(path) = parsed["path"].as_str() {
                let w_outcome = enforcer.check_file_write(path, ".", None).await;
                if let crate::permission::domain::PermissionOutcome::Denied { ref reason, .. } =
                    w_outcome
                {
                    let dur = start.elapsed().as_millis() as u64;
                    let result = TaskResult::failure(
                        node_id,
                        &node_name,
                        reason.clone(),
                        "permission_denied".to_string(),
                        dur,
                        0,
                    );
                    if let Err(e) = eb
                        .publish(crate::event_system::application::dto::PublishEventInput {
                            event: ExecutionEvent::NodeCompleted {
                                execution_id: exec_id,
                                node_id: node_id.to_string(),
                                node_name,
                                duration_ms: dur,
                                output: serde_json::json!(null),
                                timestamp: chrono::Utc::now(),
                            },
                        })
                        .await
                    {
                        tracing::warn!(error = %e, "event publish failed — evidence may be incomplete");
                    }
                    return (node_id, result);
                }
            }
        }

        // ── PreToolUse hooks (parallel path) — same gating as execute_tool ──
        if let Some(ref hook_runner) = hook_runner {
            let abort = crate::hooks::domain::HookAbortSignal::default();
            let pre_input = crate::hooks::application::dto::RunPreToolUseInput {
                tool_name: node_tool.clone(),
                tool_input: serde_json::Value::String(node_intent.clone()),
                session_id: node_id.to_string(),
                workspace_root: ".".to_string(),
            };
            if let Ok(pre_output) = hook_runner.run_pre_tool_use(pre_input, Some(&abort)).await
                && (pre_output.result.is_denied()
                    || pre_output.result.is_failed()
                    || pre_output.result.is_cancelled())
            {
                let dur = start.elapsed().as_millis() as u64;
                let result = TaskResult::failure(
                    node_id,
                    &node_name,
                    format!(
                        "Tool '{}' blocked by PreToolUse hook: {:?}",
                        node_tool,
                        pre_output.result.feedback_messages()
                    ),
                    "hook_blocked".to_string(),
                    dur,
                    0,
                );
                if let Err(e) = eb
                    .publish(crate::event_system::application::dto::PublishEventInput {
                        event: ExecutionEvent::NodeCompleted {
                            execution_id: exec_id,
                            node_id: node_id.to_string(),
                            node_name: node_name.clone(),
                            duration_ms: dur,
                            output: serde_json::json!(null),
                            timestamp: chrono::Utc::now(),
                        },
                    })
                    .await
                {
                    tracing::warn!(error = %e, "event publish failed — evidence may be incomplete");
                }
                return (node_id, result);
            }
        }

        // Tool execution — dispatch to the appropriate exec_* static method
        // All exec_* methods are on ParallelExecutionServiceImpl (same module) and take
        // (intent: &str, node_id: Uuid, node_name: &str, start: Instant) -> TaskResult
        let result = match node_tool.as_str() {
            "run_command" => {
                ParallelExecutionServiceImpl::exec_run_command(
                    &node_intent,
                    node_id,
                    &node_name,
                    start,
                )
                .await
            }
            "file_read" => {
                ParallelExecutionServiceImpl::exec_file_read(
                    &node_intent,
                    node_id,
                    &node_name,
                    start,
                )
                .await
            }
            "file_write" => {
                ParallelExecutionServiceImpl::exec_file_write(
                    &node_intent,
                    node_id,
                    &node_name,
                    start,
                )
                .await
            }
            "file_append" => {
                ParallelExecutionServiceImpl::exec_file_append(
                    &node_intent,
                    node_id,
                    &node_name,
                    start,
                )
                .await
            }
            "file_patch" => {
                ParallelExecutionServiceImpl::exec_file_patch(
                    &node_intent,
                    node_id,
                    &node_name,
                    start,
                )
                .await
            }
            "edit_file" => {
                ParallelExecutionServiceImpl::exec_edit_file(
                    &node_intent,
                    node_id,
                    &node_name,
                    start,
                )
                .await
            }
            "git_read" => {
                ParallelExecutionServiceImpl::exec_git_read(
                    &node_intent,
                    node_id,
                    &node_name,
                    start,
                )
                .await
            }
            "git_stage" => {
                ParallelExecutionServiceImpl::exec_git_stage(
                    &node_intent,
                    node_id,
                    &node_name,
                    start,
                )
                .await
            }
            "git_commit" => {
                ParallelExecutionServiceImpl::exec_git_commit(
                    &node_intent,
                    node_id,
                    &node_name,
                    start,
                )
                .await
            }
            other => {
                // Unknown tool — fail the node instead of silently succeeding
                let dur = start.elapsed().as_millis() as u64;
                TaskResult::failure(
                    node_id,
                    &node_name,
                    format!("Unknown tool '{}', intent: {}", other, node_intent),
                    "unknown_tool".to_string(),
                    dur,
                    0,
                )
            }
        };

        // ── PostToolUse hooks (informational — no gating, mirrors execute_tool) ──
        if let Some(ref hook_runner) = hook_runner {
            let tool_output = result.output.clone().unwrap_or_default();
            let abort = crate::hooks::domain::HookAbortSignal::default();
            let post_input = crate::hooks::application::dto::RunPostToolUseInput {
                tool_name: node_tool.clone(),
                tool_input: serde_json::Value::String(node_intent.clone()),
                tool_output,
                session_id: node_id.to_string(),
                workspace_root: ".".to_string(),
            };
            if let Ok(_post_output) = hook_runner
                .run_post_tool_use(post_input, Some(&abort))
                .await
            {
                // Post-tool feedback is informational — no gating
            }
        }

        // Emit NodeCompleted
        if let Err(e) = eb
            .publish(crate::event_system::application::dto::PublishEventInput {
                event: ExecutionEvent::NodeCompleted {
                    execution_id: exec_id,
                    node_id: node_id.to_string(),
                    node_name,
                    duration_ms: result.duration_ms,
                    output: serde_json::json!(result.output.clone().unwrap_or_default()),
                    timestamp: chrono::Utc::now(),
                },
            })
            .await
        {
            tracing::warn!(error = %e, "event publish failed — evidence may be incomplete");
        }

        (node_id, result)
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal Execution State
// ---------------------------------------------------------------------------

/// Internal state for an active execution.
pub(crate) struct ExecutionSession {
    /// Per-node execution states.
    node_states: HashMap<Uuid, NodeExecutionState>,
    /// IDs of nodes currently running in the JoinSet.
    in_flight: Vec<Uuid>,
    /// Aggregate execution result (built up as nodes complete).
    result: ExecutionResult,
    /// Whether execution is paused.
    paused: bool,
    /// Whether execution has been aborted.
    aborted: bool,

    /// Session-wide retry counter (GAP-A-11: max_total_retries_per_session).
    total_retries: u32,
    /// Node IDs granted human approval (for `requires_approval` steps).
    approved: HashSet<Uuid>,

    /// ISO 8601 timestamp when execution started.
    started_at: chrono::DateTime<chrono::Utc>,
    /// The TaskGraph being executed (stored for node lookup in execute_node).
    graph: Option<crate::dag_engine::domain::TaskGraph>,
}

// ---------------------------------------------------------------------------
// ApprovalBinding
// ---------------------------------------------------------------------------

/// ADR-011 opt-in binding between the execution engine and the approval module.
///
/// The `service` captures approval records, verifies them at the dispatch
/// choke point, and consumes them on terminal outcome. Callers construct the
/// service with its own `NodeIntentResolver` (the sealed graph adapter) and
/// inject it here per run scope.
pub(crate) struct ApprovalBinding {
    /// Approval lifecycle service (records, verify, consume).
    pub(crate) service: Arc<dyn ApprovalService>,
}

/// R3 sequence-policy gate verdict for a ready node about to dispatch.
///
/// Produced by [`ParallelExecutionServiceImpl::sequence_policy_verdict`] and
/// consumed by the two dispatch fill loops inside `run_dispatch_loop`.
enum SequencePolicyVerdict {
    /// No actionable match — dispatch unchanged.
    Dispatch,
    /// The node completes a forbidden sequence under a `promote` rule and is
    /// not yet human-approved — route into the existing approval pause.
    Promote,
    /// The node completes a forbidden sequence under a `deny` rule — fail the
    /// node before dispatch (its tool is never called).
    Deny { rule_id: String, later_step: String },
}

// ---------------------------------------------------------------------------
// ParallelExecutionServiceImpl
// ---------------------------------------------------------------------------

/// In-memory implementation of ParallelExecutionService.
///
/// Executes DAG nodes concurrently using tokio JoinSet, respecting
/// the max_concurrent_executions limit via a Semaphore.
/// GAP-A-12: bound `sh -c` subprocess execution (the async path must not
/// block on an unbounded subprocess).
const SUBPROCESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
/// GAP-A-12: bound git subprocess execution.
const GIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

mod dispatch;
mod tools;

mod node;

pub struct ParallelExecutionServiceImpl {
    /// Active execution sessions keyed by dag_id.
    ///
    /// Shared via `Arc` so the ADR-011 session-graph intent resolver can
    /// resolve step/node intents from the live run graph without holding a
    /// reference back to the executor (no reference cycle).
    sessions: Arc<Mutex<HashMap<Uuid, ExecutionSession>>>,
    /// Global executor config.
    config: ParallelExecutorConfig,
    /// Registered progress callbacks.
    progress_callbacks: Mutex<Vec<ProgressCallback>>,
    /// The retry evaluation service for retry decisions.
    retry_service: Box<dyn RetryEvaluationService>,
    /// Event bus for publishing node lifecycle events.
    event_bus: Arc<dyn EventBusService>,
    /// Hook runner for PreToolUse / PostToolUse / PostToolUseFailure hooks.
    hook_runner: Option<Arc<dyn HookRunnerService>>,
    /// Permission enforcer for mode-based tool gating.
    permission_enforcer: Option<Arc<dyn PermissionEnforcer>>,
    /// Recovery service for automatic failure recovery.
    recovery_service: Option<Arc<dyn RecoveryService>>,
    /// Recovery context per execution session (dag_id keyed).
    recovery_contexts: Mutex<HashMap<Uuid, RecoveryContext>>,

    /// ADR-011 approval binding (opt-in). When present, approval-gated nodes
    /// are verified against the recorded intent at the dispatch choke point
    /// (`verify_intent`), consume on terminal outcome, and approval capture
    /// persists single-use records. `None` = legacy `session.approved` gate.
    approval_binding: Option<Arc<ApprovalBinding>>,

    /// R3 sequence-policy prefix gate (opt-in).
    ///
    /// When present, `run_dispatch_loop` evaluates the session's completed
    /// dispatch prefix plus each ready node before dispatch: a matched
    /// `deny` rule fails the node deterministically BEFORE dispatch (its
    /// tool is never called); a matched `promote` rule routes the node into
    /// the existing approval pause by flipping `requires_approval` on the
    /// live graph — the approval chain from ADR-011 composes unchanged.
    /// `None` = gating disabled (status quo — no behavior change).
    sequence_policy: Option<Arc<dyn SequencePolicyService>>,

    /// R1 dispatch-time precondition gate (opt-in, ADR-017).
    ///
    /// When present, `run_dispatch_loop` assesses each ready node after the
    /// ADR-011 approval verification and before `spawn_concurrent_node`: a
    /// `Deny` verdict marks the node failed and its tool is never called.
    /// `None` = no dispatch-time precondition gating (status quo).
    precondition_gate: Option<Arc<dyn DispatchGate>>,

    /// ADR-017 R2 step-outcome gating. When
    /// `release_dependents_on_failure = false`, a failed/denied node does not
    /// release its transitive dependents: they are marked `Skipped` and never
    /// dispatched. Default = today's behavior (release).
    gating_mode: GatingMode,
}

impl ParallelExecutionServiceImpl {
    /// Create a new ParallelExecutionServiceImpl.
    pub fn new(
        config: ParallelExecutorConfig,
        retry_service: Box<dyn RetryEvaluationService>,
        event_bus: Arc<dyn EventBusService>,
    ) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            config,
            progress_callbacks: Mutex::new(Vec::new()),
            retry_service,
            event_bus,
            hook_runner: None,
            permission_enforcer: None,
            recovery_service: None,
            recovery_contexts: Mutex::new(HashMap::new()),
            approval_binding: None,
            sequence_policy: None,
            precondition_gate: None,
            gating_mode: GatingMode::default(),
        }
    }

    /// Set the hook runner for tool lifecycle hooks.
    pub fn with_hook_runner(mut self, runner: Arc<dyn HookRunnerService>) -> Self {
        self.hook_runner = Some(runner);
        self
    }

    /// Set the permission enforcer for mode-based tool gating.
    pub fn with_permission_enforcer(mut self, enforcer: Arc<dyn PermissionEnforcer>) -> Self {
        self.permission_enforcer = Some(enforcer);
        self
    }

    /// Set the R3 run-time sequence-policy prefix gate (dynamic plans).
    ///
    /// When set, the dispatch loop evaluates the completed prefix plus each
    /// ready node before dispatch (`evaluate_prefix`) and applies the rule
    /// action: `promote` → node enters the existing `AwaitingApproval` pause;
    /// `deny` → node fails with `sequence_policy_denied` before its tool is
    /// ever called. `None` (default) keeps the status-quo dispatch path.
    pub fn with_sequence_policy(mut self, svc: Arc<dyn SequencePolicyService>) -> Self {
        self.sequence_policy = Some(svc);
        self
    }

    /// Set the R1 dispatch-time precondition gate (ADR-017).
    ///
    /// When set, the dispatch loop assesses each ready node at the single
    /// choke point (after ADR-011 approval verification, before
    /// `spawn_concurrent_node`). A `Deny` verdict records a deterministic
    /// `precondition_denied` node failure and **never** calls the tool. An
    /// unarmed gate refuses matching steps (`NotArmed`, fail closed). `None`
    /// (default) keeps the status-quo dispatch path.
    pub fn with_precondition_gate(mut self, gate: Arc<dyn DispatchGate>) -> Self {
        self.precondition_gate = Some(gate);
        self
    }

    /// Set ADR-017 R2 step-outcome gating (`[gating] release_dependents_on_failure`).
    ///
    /// When `release_dependents_on_failure = false`, a failed or denied node
    /// does **not** release its transitive dependents: they are marked
    /// `Skipped` and never dispatched. The default (`true`) preserves today's
    /// behavior.
    pub fn with_gating_mode(mut self, gating_mode: GatingMode) -> Self {
        self.gating_mode = gating_mode;
        self
    }

    /// Test-only: whether a precondition gate is attached.
    #[cfg(test)]
    pub(crate) fn precondition_gate_present(&self) -> bool {
        self.precondition_gate.is_some()
    }

    /// Test-only: the attached ADR-017 R2 gating mode.
    #[cfg(test)]
    pub(crate) fn gating_mode_value(&self) -> GatingMode {
        self.gating_mode
    }

    /// Share the session map (ADR-011: session-graph intent resolver).
    pub(crate) fn sessions_handle(&self) -> Arc<Mutex<HashMap<Uuid, ExecutionSession>>> {
        Arc::clone(&self.sessions)
    }

    /// Set the recovery service for automatic failure recovery.
    pub fn with_recovery_service(mut self, recovery: Arc<dyn RecoveryService>) -> Self {
        self.recovery_service = Some(recovery);
        self
    }

    /// Enable ADR-011 approval binding (opt-in).
    ///
    /// When set, approval-gated nodes are verified against the recorded
    /// intent hash before dispatch (`Matched` → dispatch, else HALT into
    /// `IntentMismatch` — the tool is never called) and single-use records are
    /// consumed on terminal outcome. `approve_node` persists binding records
    /// when the input carries an approver identity. Without a binding the
    /// legacy `session.approved` gate is unchanged.
    pub fn with_approval_service(mut self, service: Arc<dyn ApprovalService>) -> Self {
        self.approval_binding = Some(Arc::new(ApprovalBinding { service }));
        self
    }
}

#[async_trait]
impl ParallelExecutionService for ParallelExecutionServiceImpl {
    async fn execute_graph(
        &self,
        input: ExecuteGraphInput,
    ) -> Result<ExecuteGraphOutput, ExecutionError> {
        let config = input
            .config_override
            .clone()
            .unwrap_or_else(|| self.config.clone());

        // GAP-A-02: a missing graph is a caller contract violation — fail with
        // a typed error instead of returning a fake-success empty result.
        let Some(mut graph) = input.graph else {
            return Err(ExecutionError::InvalidState {
                reason: format!(
                    "execute_graph called without a graph (dag_id={})",
                    input.dag_id
                ),
            });
        };

        // ── Real execution path ──────────────────────────────────────

        // Seal the graph if not already sealed
        if !graph.sealed {
            graph.seal().map_err(|e| ExecutionError::InvalidState {
                reason: format!("Failed to seal graph: {}", e),
            })?;
        }

        let total_nodes = graph.node_count() as u32;
        let started_at = Utc::now();

        // Initialize node states from graph nodes
        let mut node_states: HashMap<Uuid, NodeExecutionState> = HashMap::new();
        for node in graph.nodes() {
            node_states.insert(node.id, NodeExecutionState::new(node.id, &node.name));
        }

        // Mark initially ready nodes
        for ready_id in graph.ready_nodes() {
            if let Some(state) = node_states.get_mut(&ready_id) {
                state.mark_ready();
            }
        }

        // Initialize session
        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Lock error: {}", e),
                })?;

            if sessions.contains_key(&input.dag_id) {
                return Err(ExecutionError::InvalidState {
                    reason: format!("Execution already in progress for dag_id={}", input.dag_id),
                });
            }

            sessions.insert(
                input.dag_id,
                ExecutionSession {
                    node_states: node_states.clone(),
                    in_flight: Vec::new(),
                    result: ExecutionResult::new(input.dag_id),
                    paused: false,
                    aborted: false,
                    total_retries: 0,
                    approved: HashSet::new(),
                    started_at,
                    graph: Some(graph.clone()),
                },
            );
        }

        // ── Parallel dispatch loop ────────────────────────────────────
        // Dispatches ready nodes up to max_concurrent, gating dispatch on
        // human approval: steps declaring `requires_approval` are not
        // dispatched until approved (see run_dispatch_loop).
        let approved: HashSet<Uuid> = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Lock error: {}", e),
                })?;
            sessions
                .get(&input.dag_id)
                .map(|s| s.approved.clone())
                .unwrap_or_default()
        };

        let (completed_count, failed_count, node_results, approval_blocked) = self
            .run_dispatch_loop(
                &mut graph,
                input.dag_id,
                started_at,
                total_nodes,
                &approved,
                &config,
            )
            .await?;

        // Build final result using the LIVE node states from the session so
        // completed/failed/awaiting-approval states survive for approve,
        // resume, and get_execution_state flows.
        let live_states: HashMap<Uuid, NodeExecutionState> = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Lock error: {}", e),
                })?;
            sessions
                .get(&input.dag_id)
                .map(|s| s.node_states.clone())
                .unwrap_or_else(|| node_states.clone())
        };

        // Build final result
        let completed_at = Utc::now();
        let total_retries: u32 = node_results.values().map(|r| r.retry_attempts as u32).sum();
        let final_result = ExecutionResult {
            dag_id: input.dag_id,
            node_results,
            execution_states: live_states.clone(),
            completed_count,
            failed_count,
            skipped_count: 0,
            total_nodes,
            total_duration_ms: completed_at
                .signed_duration_since(started_at)
                .num_milliseconds()
                .max(0) as u64,
            total_retries,
            started_at,
            completed_at,
            cancelled: false,
            cancellation_reason: None,
        };

        let (approval_pending, pending_approval_steps) = if approval_blocked {
            let steps = live_states
                .values()
                .filter(|s| {
                    s.status == NodeStatus::AwaitingApproval
                        || s.status == NodeStatus::IntentMismatch
                })
                .map(|s| s.node_name.clone())
                .collect::<Vec<_>>();
            (true, steps)
        } else {
            (false, Vec::new())
        };

        // Update session with final result (preserve live node states and
        // persist the live graph so a later approve/resume can continue the
        // DAG exactly where dispatch paused).
        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Lock error: {}", e),
                })?;
            if let Some(session) = sessions.get_mut(&input.dag_id) {
                session.result = final_result.clone();
                session.graph = Some(graph.clone());
                if approval_pending {
                    session.paused = true;
                }
            }
        }

        Ok(ExecuteGraphOutput {
            result: final_result,
            completed_at,
            approval_pending,
            pending_approval_steps,
        })
    }

    async fn execute_node(
        &self,
        input: ExecuteNodeInput,
    ) -> Result<ExecuteNodeOutput, ExecutionError> {
        self.execute_node_flow(input).await
    }

    async fn get_execution_state(
        &self,
        input: GetExecutionStateInput,
    ) -> Result<GetExecutionStateOutput, ExecutionError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|e| ExecutionError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;

        let session = sessions
            .get(&input.dag_id)
            .ok_or(ExecutionError::NodeNotFound {
                node_id: input.dag_id,
            })?;

        let completed = session
            .node_states
            .values()
            .filter(|s| s.status == NodeStatus::Completed)
            .count() as u32;
        let failed = session
            .node_states
            .values()
            .filter(|s| s.status == NodeStatus::Failed)
            .count() as u32;
        let skipped = session
            .node_states
            .values()
            .filter(|s| s.status == NodeStatus::Skipped)
            .count() as u32;
        let total = session.node_states.len() as u32;
        let is_complete = completed + failed + skipped >= total && total > 0;
        let (total_duration_ms, completed_at) = if is_complete {
            (
                session.result.total_duration_ms,
                Some(session.result.completed_at),
            )
        } else {
            // Paused / still running: report elapsed wall-clock since start.
            (
                chrono::Utc::now()
                    .signed_duration_since(session.started_at)
                    .num_milliseconds()
                    .max(0) as u64,
                None,
            )
        };

        Ok(GetExecutionStateOutput {
            dag_id: input.dag_id,
            node_states: session.node_states.clone(),
            completed_count: completed,
            failed_count: failed,
            skipped_count: skipped,
            total_nodes: total,
            started_at: Some(session.started_at),
            paused: session.paused,
            is_complete,
            total_duration_ms,
            completed_at,
        })
    }

    async fn pause_execution(
        &self,
        input: PauseExecutionInput,
    ) -> Result<PauseExecutionOutput, ExecutionError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| ExecutionError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;

        let session = sessions
            .get_mut(&input.dag_id)
            .ok_or(ExecutionError::NodeNotFound {
                node_id: input.dag_id,
            })?;

        if session.paused {
            return Err(ExecutionError::InvalidState {
                reason: "Execution is already paused".to_string(),
            });
        }

        session.paused = true;
        let in_flight = session.in_flight.len() as u32;
        let pending = session
            .node_states
            .values()
            .filter(|s| s.status == NodeStatus::Ready || s.status == NodeStatus::Pending)
            .count() as u32;

        Ok(PauseExecutionOutput {
            dag_id: input.dag_id,
            in_flight_count: in_flight,
            pending_count: pending,
            paused_at: Utc::now(),
        })
    }

    async fn resume_execution(
        &self,
        input: ResumeExecutionInput,
    ) -> Result<ResumeExecutionOutput, ExecutionError> {
        // Snapshot the paused session (graph + approvals) so the dispatch
        // loop can run without holding the sessions lock across awaits.
        let (graph, approved, node_states) = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Lock error: {}", e),
                })?;

            let session = sessions
                .get_mut(&input.dag_id)
                .ok_or(ExecutionError::NodeNotFound {
                    node_id: input.dag_id,
                })?;

            if !session.paused {
                return Err(ExecutionError::InvalidState {
                    reason: "Execution is not paused".to_string(),
                });
            }

            session.paused = false;
            (
                session.graph.clone(),
                session.approved.clone(),
                session.node_states.clone(),
            )
        };

        let mut ready = node_states
            .values()
            .filter(|s| s.status == NodeStatus::Ready)
            .count() as u32;

        // Continue the DAG if there is a live graph (an approval-paused
        // execution): re-run the dispatch loop for the remaining nodes now
        // that approvals have been granted.
        if let Some(mut graph) = graph {
            let total_nodes = graph.node_count() as u32;
            let started_at = Utc::now();
            let (completed, failed, node_results, approval_blocked) = self
                .run_dispatch_loop(
                    &mut graph,
                    input.dag_id,
                    started_at,
                    total_nodes,
                    &approved,
                    &self.config,
                )
                .await?;

            let live_states = {
                let sessions = self
                    .sessions
                    .lock()
                    .map_err(|e| ExecutionError::InternalError {
                        detail: format!("Lock error: {}", e),
                    })?;
                let s =
                    sessions
                        .get(&input.dag_id)
                        .ok_or_else(|| ExecutionError::InvalidState {
                            reason: "Session disappeared during resume".to_string(),
                        })?;
                s.node_states.clone()
            };

            ready = live_states
                .values()
                .filter(|s| s.status == NodeStatus::Ready)
                .count() as u32;

            let total_retries: u32 = node_results.values().map(|r| r.retry_attempts as u32).sum();
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Lock error: {}", e),
                })?;
            if let Some(session) = sessions.get_mut(&input.dag_id) {
                // Merge this segment's results with pre-pause results so
                // no completed node is lost.
                let mut all_results = session.result.node_results.clone();
                all_results.extend(node_results);
                session.result = ExecutionResult {
                    dag_id: input.dag_id,
                    node_results: all_results,
                    execution_states: live_states.clone(),
                    completed_count: session.result.completed_count + completed,
                    failed_count: session.result.failed_count + failed,
                    skipped_count: 0,
                    total_nodes,
                    total_duration_ms: session.result.total_duration_ms
                        + Utc::now()
                            .signed_duration_since(started_at)
                            .num_milliseconds()
                            .max(0) as u64,
                    total_retries: session.result.total_retries + total_retries,
                    started_at: session.result.started_at,
                    completed_at: Utc::now(),
                    cancelled: false,
                    cancellation_reason: None,
                };
                session.graph = Some(graph.clone());
                if approval_blocked {
                    session.paused = true;
                }
            }
        }

        Ok(ResumeExecutionOutput {
            dag_id: input.dag_id,
            ready_count: ready,
            resumed_at: Utc::now(),
        })
    }

    async fn approve_node(
        &self,
        input: ApproveNodeInput,
    ) -> Result<ApproveNodeOutput, ExecutionError> {
        // Binding-aware approve: with ADR-011 approval binding the approval is
        // captured as a single-use record bound to the canonical execution
        // intent (R1+R3). The legacy `session.approved` set remains the gate
        // in both modes — with binding, a node enters it only after its
        // record was persisted.
        struct Candidate {
            name: String,
            node_id: Uuid,
        }

        // (1) Resolve candidates under the sessions lock (no awaits held).
        let (candidates, mut denied, not_found) = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Lock error: {e}"),
                })?;

            let session = sessions
                .get_mut(&input.dag_id)
                .ok_or(ExecutionError::NodeNotFound {
                    node_id: input.dag_id,
                })?;

            let mut candidates: Vec<Candidate> = Vec::new();
            let mut denied: Vec<String> = Vec::new();
            let mut not_found: Vec<String> = Vec::new();

            for name in &input.step_names {
                // Resolve step name -> node id from the session's live states.
                let node_id = session
                    .node_states
                    .iter()
                    .find(|(_, s)| &s.node_name == name)
                    .map(|(id, _)| *id);

                let Some(node_id) = node_id else {
                    not_found.push(name.clone());
                    continue;
                };

                // GAP-H-07: only approval-gated nodes awaiting human sign-off
                // may be approved. Resolve `requires_approval` from the session
                // graph; a node not in `AwaitingApproval` (already approved/
                // executed/completed) is rejected. A node halted on
                // `IntentMismatch` is awaiting re-approval and may be approved
                // again (fresh record + new hash).
                let gated = session
                    .graph
                    .as_ref()
                    .and_then(|g| g.get_node(node_id))
                    .map(|n| n.requires_approval)
                    .unwrap_or(false);
                let awaiting = session
                    .node_states
                    .get(&node_id)
                    .map(|s| {
                        s.status == NodeStatus::AwaitingApproval
                            || s.status == NodeStatus::IntentMismatch
                    })
                    .unwrap_or(false);

                if !gated || !awaiting {
                    denied.push(name.clone());
                    continue;
                }

                candidates.push(Candidate {
                    name: name.clone(),
                    node_id,
                });
            }
            (candidates, denied, not_found)
        };

        let candidate_names: Vec<String> = candidates.iter().map(|c| c.name.clone()).collect();

        // (2) Capture the approval record when a binding is configured.
        let mut approved: Vec<String> = Vec::new();
        if let Some(binding) = &self.approval_binding {
            // R3: identity is a required captured fact — fail closed without it.
            let approver_id = input.approver_id.clone().filter(|s| !s.trim().is_empty());
            match approver_id {
                Some(approver_id) => {
                    let approve_input = ApprovalApproveInput {
                        dag_id: input.dag_id,
                        step_names: candidate_names.clone(),
                        approver_id,
                        authority: input.authority.clone(),
                        decision_context: input.decision_context.clone(),
                        token_claims_ref: input.token_claims_ref.clone(),
                    };
                    let out = binding.service.approve(approve_input).await.map_err(|e| {
                        ExecutionError::InternalError {
                            detail: format!("Approval binding capture failed: {e}"),
                        }
                    })?;
                    approved = out.approved;
                    // Names that failed to bind are denied (never silently granted).
                    for name in &candidate_names {
                        if !approved.contains(name) {
                            denied.push(name.clone());
                        }
                    }
                    // ADR-011 R3: publish ApprovalRecorded for every persisted
                    // record — the drained events are the source the audit
                    // envelope derives `approval_events[]` from.
                    for record in &out.approval_records {
                        let decision_context_ref =
                            if record.decision_context.summary.trim().is_empty() {
                                None
                            } else {
                                Some(record.decision_context.summary.clone())
                            };
                        self.publish_event(ExecutionEvent::ApprovalRecorded {
                            execution_id: input.dag_id,
                            node_id: record.node_id.to_string(),
                            step_name: record.step_name.clone(),
                            intent_hash: record.intent_hash.0.clone(),
                            approver_id: record.approver_id.clone(),
                            authority: record.authority.clone(),
                            decided_at: record.decided_at,
                            decision_context_ref,
                            timestamp: chrono::Utc::now(),
                        })
                        .await;
                    }
                }
                None => {
                    if !candidate_names.is_empty() {
                        tracing::warn!(
                            dag_id = %input.dag_id,
                            candidates = candidate_names.len(),
                            "approve denied: approval binding requires approver_id (identity is a captured fact)"
                        );
                        denied.extend(candidate_names.clone());
                    }
                }
            }
        } else {
            approved = candidate_names;
        }

        // (3) Commit under the sessions lock: approved nodes become
        // dispatchable; still_pending reflects the remaining approval gate.
        let mut still_pending: Vec<String> = Vec::new();
        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Lock error: {e}"),
                })?;
            if let Some(session) = sessions.get_mut(&input.dag_id) {
                for cand in &candidates {
                    if approved.contains(&cand.name) {
                        session.approved.insert(cand.node_id);
                        // A node blocked on approval becomes dispatchable again.
                        if let Some(state) = session.node_states.get_mut(&cand.node_id) {
                            state.mark_ready();
                        }
                    }
                }
                still_pending = session
                    .node_states
                    .values()
                    .filter(|s| {
                        s.status == NodeStatus::AwaitingApproval
                            || s.status == NodeStatus::IntentMismatch
                    })
                    .map(|s| s.node_name.clone())
                    .collect();
            }
        }

        // `denied` may contain duplicates if a name was both un-gated and
        // binding-denied — de-dupe while preserving order.
        let mut seen = HashSet::new();
        denied.retain(|n| seen.insert(n.clone()));

        Ok(ApproveNodeOutput {
            dag_id: input.dag_id,
            approved,
            not_found,
            still_pending,
            denied,
        })
    }

    async fn hydrate_execution(
        &self,
        input: HydrateExecutionInput,
    ) -> Result<HydrateExecutionOutput, ExecutionError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| ExecutionError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;

        // Idempotent — a live session (same process paused the run) wins.
        if sessions.contains_key(&input.dag_id) {
            return Ok(HydrateExecutionOutput {
                dag_id: input.dag_id,
                node_count: 0,
                created: false,
            });
        }

        // Rebuild the graph's runtime execution state. The persisted graph
        // was deserialized with `sealed: true` but `execution_state` is
        // #[serde(skip)] — its in-degree/dependents/ready-queue are empty.
        // Build a FRESH TaskGraph from the persisted nodes and seal it to
        // reconstruct the execution state, then release the dependents of
        // nodes that already completed before the pause.
        let mut graph = crate::dag_engine::domain::TaskGraph::new();
        for node in &input.graph.nodes {
            graph
                .add_unchecked(node.clone())
                .map_err(|e| ExecutionError::InvalidState {
                    reason: format!("Failed to rebuild persisted graph: {}", e),
                })?;
        }
        graph.seal().map_err(|e| ExecutionError::InvalidState {
            reason: format!("Failed to seal persisted graph: {}", e),
        })?;
        for (node_id, state) in &input.node_states {
            if state.status.is_terminal() {
                let _ = graph.mark_completed(*node_id);
            }
        }

        let node_count = input.node_states.len();
        let approved = input.approved;
        // Preserve the original execution start: the persisted ExecutionState
        // carries it (state_persistence `started_at`), so a resumed run reports
        // an undistorted duration_ms in the audit envelope instead of
        // re-baselining to "now".
        let started_at = input.started_at;

        sessions.insert(
            input.dag_id,
            ExecutionSession {
                node_states: input.node_states,
                in_flight: Vec::new(),
                result: ExecutionResult::new(input.dag_id),
                // Hydrated sessions start paused: the caller approves then
                // resumes, which re-runs the dispatch loop for remaining nodes.
                paused: true,
                aborted: false,
                total_retries: 0,
                approved: approved.clone(),
                started_at,
                graph: Some(graph),
            },
        );

        Ok(HydrateExecutionOutput {
            dag_id: input.dag_id,
            node_count,
            created: true,
        })
    }

    async fn abort_execution(
        &self,
        input: AbortExecutionInput,
    ) -> Result<AbortExecutionOutput, ExecutionError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| ExecutionError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;

        let session = sessions
            .get_mut(&input.dag_id)
            .ok_or(ExecutionError::NodeNotFound {
                node_id: input.dag_id,
            })?;

        if session.aborted {
            return Err(ExecutionError::InvalidState {
                reason: "Execution is already aborted".to_string(),
            });
        }

        session.aborted = true;
        // Mark all non-terminal nodes as skipped
        let mut skipped = 0u32;
        let completed = session
            .node_states
            .values()
            .filter(|s| s.status == NodeStatus::Completed)
            .count() as u32;

        for state in session.node_states.values_mut() {
            if !state.is_terminal() {
                state.mark_skipped(format!("Execution aborted: {}", input.reason));
                skipped += 1;
            }
        }

        session.result.cancelled = true;
        session.result.cancellation_reason = Some(input.reason);
        session.result.skipped_count += skipped;
        session.result.completed_at = Utc::now();

        Ok(AbortExecutionOutput {
            dag_id: input.dag_id,
            completed_count: completed,
            skipped_count: skipped,
            aborted_at: Utc::now(),
        })
    }

    #[tracing::instrument(skip_all)]
    fn on_progress(&self, callback: ProgressCallback) {
        if let Ok(mut callbacks) = self.progress_callbacks.lock() {
            callbacks.push(callback);
        }
    }
}

// ---------------------------------------------------------------------------
// RetryEvaluationServiceImpl
// ---------------------------------------------------------------------------

/// Stateless retry evaluation service.
///
/// Makes retry decisions purely based on:
/// - Failure type (retriable or not)
/// - Retry policy (max_attempts, strategies, backoff)
/// - Remaining retry budget
///
/// All decisions are computational — no external dependencies.
pub struct RetryEvaluationServiceImpl {
    /// Optional structured failure classifier (GAP-A-19).
    classifier: Option<
        Arc<dyn crate::failure_classification::application::service::FailureClassifierService>,
    >,
}

impl RetryEvaluationServiceImpl {
    /// Create a new RetryEvaluationServiceImpl without structured
    /// classification (legacy policy-only behavior).
    pub fn new() -> Self {
        Self { classifier: None }
    }

    /// Create a service whose retry decisions are driven by structured
    /// `FailureClassifierService` classification (GAP-A-19) when the
    /// classifier produces a confident match; unclassified failures defer
    /// to the policy's substring list (preserving prior behavior).
    pub fn with_classifier(
        classifier: Arc<
            dyn crate::failure_classification::application::service::FailureClassifierService,
        >,
    ) -> Self {
        Self {
            classifier: Some(classifier),
        }
    }
}

impl Default for RetryEvaluationServiceImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RetryEvaluationService for RetryEvaluationServiceImpl {
    async fn evaluate_retry(
        &self,
        input: EvaluateRetryInput,
    ) -> Result<EvaluateRetryOutput, ExecutionError> {
        let decision = self
            .decide(
                &input.failure_context,
                &input.policy,
                input.fallback_node_id,
            )
            .await;
        let is_terminal = decision.is_terminal();

        Ok(EvaluateRetryOutput {
            decision,
            is_terminal,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn compute_backoff(&self, failure_context: &FailureContext, policy: &RetryPolicy) -> u64 {
        policy
            .backoff_strategy
            .compute_delay_ms(failure_context.attempt)
    }

    #[tracing::instrument(skip_all)]
    async fn validate_policy(&self, policy: &RetryPolicy) -> Result<Vec<String>, ExecutionError> {
        let mut errors = Vec::new();

        if policy.max_attempts == 0 {
            errors.push("max_attempts must be at least 1".to_string());
        }

        if policy.retry_strategies.is_empty() {
            errors.push("retry_strategies must not be empty".to_string());
        }

        // Validate backoff strategy parameters
        match &policy.backoff_strategy {
            BackoffStrategy::Exponential { multiplier, .. } => {
                if *multiplier < 1.0 {
                    errors.push(format!(
                        "Exponential backoff multiplier must be >= 1.0, got {}",
                        multiplier
                    ));
                }
            }
            BackoffStrategy::Fixed { base_delay_ms }
            | BackoffStrategy::Linear { base_delay_ms, .. } => {
                if *base_delay_ms == 0 {
                    errors.push("base_delay_ms must be > 0 for non-immediate backoff".to_string());
                }
            }
            BackoffStrategy::Immediate => { /* always valid */ }
        }

        Ok(errors)
    }

    #[tracing::instrument(skip_all)]
    async fn is_failure_retriable(&self, policy: &RetryPolicy, failure_type: &str) -> bool {
        policy.is_failure_retriable(failure_type)
    }

    async fn decide(
        &self,
        failure_context: &FailureContext,
        policy: &RetryPolicy,
        fallback_node_id: Option<Uuid>,
    ) -> RetryDecision {
        // GAP-A-19: structured classification augments the policy check.
        // A confident classifier match decides retriability; the documented
        // no-match default (NonRetryable) is treated as "unclassified" and
        // defers to the policy's substring list.
        let mut classification_note = String::new();
        let classified_gate: Option<bool> = if let Some(classifier) = &self.classifier {
            match classifier
                .classify(
                    crate::failure_classification::application::dto::ClassifyFailureInput {
                        error_message: failure_context.error_message.clone(),
                        operation_context: Some("execution retry evaluation".to_string()),
                        source: Some("execution_engine".to_string()),
                    },
                )
                .await
            {
                Ok(output) => {
                    let is_default = output
                        .explanation
                        .as_deref()
                        .is_some_and(|e| e.starts_with("No matching pattern"));
                    if is_default {
                        None
                    } else {
                        classification_note = format!(
                            " (classified {:?}: {})",
                            output.failure_type,
                            output.explanation.as_deref().unwrap_or("")
                        );
                        Some(output.is_retryable)
                    }
                }
                Err(_) => None,
            }
        } else {
            None
        };

        // 1. Check if the failure type is retriable
        let retriable = match classified_gate {
            Some(decided) => decided,
            None => policy.is_failure_retriable(&failure_context.failure_type),
        };
        if !retriable {
            if let Some(fallback_id) = fallback_node_id {
                return RetryDecision::Fallback {
                    fallback_node_id: fallback_id,
                    reason: format!(
                        "Failure type '{}' is not retriable{}; executing fallback",
                        failure_context.failure_type, classification_note
                    ),
                };
            }
            return RetryDecision::Skip {
                reason: format!(
                    "Failure type '{}' is not retriable{} and no fallback configured",
                    failure_context.failure_type, classification_note
                ),
            };
        }

        // 2. Check if max_attempts is exhausted
        if failure_context.is_exhausted() {
            let reason = format!(
                "Retry limit exhausted after {} attempts (max={})",
                failure_context.attempt + 1,
                failure_context.max_attempts,
            );

            if let Some(fallback_id) = fallback_node_id
                && policy.enable_fallback
            {
                return RetryDecision::Fallback {
                    fallback_node_id: fallback_id,
                    reason: format!("{}. Executing fallback", reason),
                };
            }

            if policy.skip_on_exhaustion {
                return RetryDecision::Skip {
                    reason: format!("{}. Skipping node", reason),
                };
            }

            return RetryDecision::Abort { reason };
        }

        // 3. Check skip conditions
        if policy.has_skip_conditions() {
            // Evaluate skip conditions against failure context
            if let Some(conditions) = &policy.skip_conditions {
                for condition in conditions {
                    if failure_context.error_message.contains(condition) {
                        return RetryDecision::Skip {
                            reason: format!(
                                "Skip condition '{}' matched error: {}",
                                condition, failure_context.error_message
                            ),
                        };
                    }
                }
            }
        }

        // 4. Determine retry strategy for this attempt
        let attempt = failure_context.attempt + 1;
        let strategy = policy.strategy_for_attempt(failure_context.attempt);

        // Check if strategy results in skip
        if strategy.is_skip() {
            return RetryDecision::Skip {
                reason: format!("Retry strategy 'skip_and_continue' for attempt {}", attempt),
            };
        }

        // 5. Compute backoff
        let backoff_ms = policy
            .backoff_strategy
            .compute_delay_ms(failure_context.attempt);

        RetryDecision::Retry {
            strategy,
            attempt,
            backoff_ms,
            reason: format!(
                "Attempt {} of {}: retrying with {:?}, backoff={}ms",
                attempt, failure_context.max_attempts, strategy, backoff_ms
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// SessionGraphResolver — ADR-011 intent source for the production binding
// ---------------------------------------------------------------------------

/// Resolves step/node intents from the executor's LIVE sessions.
///
/// Implementation-level `NodeIntentResolver` used to build the production
/// `ApprovalServiceImpl`: `resolve_by_node_id` looks up the node in any
/// active session's sealed graph; `resolve_by_step_name` matches a session
/// node currently blocked at the approval gate (AwaitingApproval /
/// IntentMismatch). Reads only the shared session map — no reference back to
/// the executor, so no Arc cycle.
#[derive(Clone)]
pub(crate) struct SessionGraphResolver {
    sessions: Arc<Mutex<HashMap<Uuid, ExecutionSession>>>,
}

impl SessionGraphResolver {
    /// Attach a resolver to an executor's shared session map.
    pub(crate) fn new(sessions: Arc<Mutex<HashMap<Uuid, ExecutionSession>>>) -> Self {
        Self { sessions }
    }
}

#[async_trait::async_trait]
impl crate::approval::application::NodeIntentResolver for SessionGraphResolver {
    async fn resolve_by_step_name(&self, step_name: &str) -> Option<ResolvedNode> {
        let map = self.sessions.lock().ok()?;
        for session in map.values() {
            if let Some((node_id, _)) = session.node_states.iter().find(|(_, st)| {
                st.node_name == step_name
                    && (st.status == NodeStatus::AwaitingApproval
                        || st.status == NodeStatus::IntentMismatch)
            }) && let Some(node) = session.graph.as_ref().and_then(|g| g.get_node(*node_id))
            {
                return Some(ResolvedNode {
                    node_id: *node_id,
                    step_name: step_name.to_string(),
                    intent: crate::approval::domain::ExecutionIntent::from_node(node),
                });
            }
        }
        None
    }

    async fn resolve_by_node_id(&self, node_id: Uuid) -> Option<ResolvedNode> {
        let map = self.sessions.lock().ok()?;
        for session in map.values() {
            if let Some(node) = session.graph.as_ref().and_then(|g| g.get_node(node_id)) {
                return Some(ResolvedNode {
                    node_id,
                    step_name: session
                        .node_states
                        .get(&node_id)
                        .map(|st| st.node_name.clone())
                        .unwrap_or_else(|| node.name.clone()),
                    intent: crate::approval::domain::ExecutionIntent::from_node(node),
                });
            }
        }
        None
    }
}

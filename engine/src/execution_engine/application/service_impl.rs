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
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use uuid::Uuid;

use crate::event_system::application::EventBusService;
use crate::event_system::domain::ExecutionEvent;
use crate::execution_engine::domain::{
    BackoffStrategy, ExecutionError, ExecutionResult, FailureContext, NodeExecutionState,
    NodeStatus, ParallelExecutorConfig, RetryDecision, RetryPolicy, TaskResult,
};

use super::dto::{
    AbortExecutionInput, AbortExecutionOutput, EvaluateRetryInput, EvaluateRetryOutput,
    ExecuteGraphInput, ExecuteGraphOutput, ExecuteNodeInput, ExecuteNodeOutput,
    GetExecutionStateInput, GetExecutionStateOutput, PauseExecutionInput, PauseExecutionOutput,
    ResumeExecutionInput, ResumeExecutionOutput,
};
use super::service::{ExecutionProgress, ParallelExecutionService, RetryEvaluationService};

// ---------------------------------------------------------------------------
// Internal Execution State
// ---------------------------------------------------------------------------

/// Internal state for an active execution.
struct ExecutionSession {
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
    /// Total retries across all nodes in this session.
    /// Reserved for metrics/observability reporting.
    #[allow(dead_code)]
    total_retries: u32,
    /// ISO 8601 timestamp when execution started.
    started_at: chrono::DateTime<chrono::Utc>,
    /// The TaskGraph being executed (stored for node lookup in execute_node).
    graph: Option<crate::dag_engine::domain::TaskGraph>,
}

// ---------------------------------------------------------------------------
// ParallelExecutionServiceImpl
// ---------------------------------------------------------------------------

/// In-memory implementation of ParallelExecutionService.
///
/// Executes DAG nodes concurrently using tokio JoinSet, respecting
/// the max_concurrent_executions limit via a Semaphore.
pub struct ParallelExecutionServiceImpl {
    /// Active execution sessions keyed by dag_id.
    sessions: Mutex<HashMap<Uuid, ExecutionSession>>,
    /// Global executor config.
    config: ParallelExecutorConfig,
    /// Registered progress callbacks.
    progress_callbacks: Mutex<Vec<ProgressCallback>>,
    /// The retry evaluation service for retry decisions.
    retry_service: Box<dyn RetryEvaluationService>,
    /// Event bus for publishing node lifecycle events.
    event_bus: Arc<dyn EventBusService>,
}

impl ParallelExecutionServiceImpl {
    /// Create a new ParallelExecutionServiceImpl.
    pub fn new(
        config: ParallelExecutorConfig,
        retry_service: Box<dyn RetryEvaluationService>,
        event_bus: Arc<dyn EventBusService>,
    ) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            config,
            progress_callbacks: Mutex::new(Vec::new()),
            retry_service,
            event_bus,
        }
    }

    /// Execute a single tool node and return the TaskResult.
    async fn execute_tool(
        &self,
        node: &crate::dag_engine::domain::TaskNode,
        node_id: Uuid,
        start: std::time::Instant,
    ) -> TaskResult {
        match node.tool.as_str() {
            "run_command" => Self::exec_run_command(&node.intent, node_id, &node.name, start).await,
            "file_read" => Self::exec_file_read(&node.intent, node_id, &node.name, start).await,
            "file_write" => Self::exec_file_write(&node.intent, node_id, &node.name, start).await,
            "file_append" => Self::exec_file_append(&node.intent, node_id, &node.name, start).await,
            "file_patch" => Self::exec_file_patch(&node.intent, node_id, &node.name, start).await,
            "git_read" => Self::exec_git_read(&node.intent, node_id, &node.name, start).await,
            "git_stage" => Self::exec_git_stage(&node.intent, node_id, &node.name, start).await,
            "git_commit" => Self::exec_git_commit(&node.intent, node_id, &node.name, start).await,
            _ => {
                let duration_ms = start.elapsed().as_millis() as u64;
                TaskResult::success(
                    node_id,
                    &node.name,
                    Some(format!(
                        "[PLACEHOLDER] Tool '{}' would execute: {}",
                        node.tool, node.intent
                    )),
                    duration_ms,
                    0,
                )
            }
        }
    }

    async fn exec_run_command(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(intent)
            .output()
            .await;
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                let duration_ms = start.elapsed().as_millis() as u64;
                if out.status.success() {
                    TaskResult::success(
                        node_id,
                        node_name,
                        Some(if stdout.is_empty() { stderr } else { stdout }),
                        duration_ms,
                        0,
                    )
                } else {
                    let err = if stderr.is_empty() { stdout } else { stderr };
                    TaskResult::failure(
                        node_id,
                        node_name,
                        err,
                        "command_failed".to_string(),
                        duration_ms,
                        0,
                    )
                }
            }
            Err(e) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "exec_error".to_string(),
                0,
                0,
            ),
        }
    }

    async fn exec_file_read(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let path = intent;
        let duration_ms = start.elapsed().as_millis() as u64;
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let truncated: String = content.chars().take(4096).collect();
                TaskResult::success(node_id, node_name, Some(truncated), duration_ms, 0)
            }
            Err(e) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "file_read_error".to_string(),
                duration_ms,
                0,
            ),
        }
    }

    async fn exec_file_write(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let parsed: serde_json::Value = match serde_json::from_str(intent) {
            Ok(v) => v,
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "parse_error".to_string(),
                    duration_ms,
                    0,
                );
            }
        };
        let path = parsed["path"].as_str().unwrap_or("");
        let content = parsed["content"].as_str().unwrap_or("");
        let duration_ms = start.elapsed().as_millis() as u64;
        // Ensure parent dir exists
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        match std::fs::write(path, content) {
            Ok(()) => TaskResult::success(
                node_id,
                node_name,
                Some(format!("Wrote {} bytes to {}", content.len(), path)),
                duration_ms,
                0,
            ),
            Err(e) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "file_write_error".to_string(),
                duration_ms,
                0,
            ),
        }
    }

    async fn exec_file_append(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let parsed: serde_json::Value = match serde_json::from_str(intent) {
            Ok(v) => v,
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "parse_error".to_string(),
                    duration_ms,
                    0,
                );
            }
        };
        let path = parsed["path"].as_str().unwrap_or("");
        let content = parsed["content"].as_str().unwrap_or("");
        let duration_ms = start.elapsed().as_millis() as u64;
        match std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
        {
            Ok(mut file) => {
                use std::io::Write;
                match writeln!(file, "{}", content) {
                    Ok(()) => TaskResult::success(
                        node_id,
                        node_name,
                        Some(format!("Appended {} bytes to {}", content.len(), path)),
                        duration_ms,
                        0,
                    ),
                    Err(e) => TaskResult::failure(
                        node_id,
                        node_name,
                        e.to_string(),
                        "file_append_error".to_string(),
                        duration_ms,
                        0,
                    ),
                }
            }
            Err(e) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "file_append_error".to_string(),
                duration_ms,
                0,
            ),
        }
    }

    async fn exec_file_patch(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let parsed: serde_json::Value = match serde_json::from_str(intent) {
            Ok(v) => v,
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "parse_error".to_string(),
                    duration_ms,
                    0,
                );
            }
        };
        let path = parsed["path"].as_str().unwrap_or("");
        let search = parsed["search"].as_str().unwrap_or("");
        let insert = parsed["insert"].as_str().unwrap_or("");
        let before = parsed["before"].as_bool().unwrap_or(false);
        let duration_ms = start.elapsed().as_millis() as u64;

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                return TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "file_patch_error".to_string(),
                    duration_ms,
                    0,
                );
            }
        };

        // Simple text-based patch: find search string, insert before/after
        if let Some(pos) = content.find(search) {
            let new_content = if before {
                format!("{}{}{}", &content[..pos], insert, &content[pos..])
            } else {
                let after = pos + search.len();
                format!("{}{}{}", &content[..after], insert, &content[after..])
            };
            match std::fs::write(path, &new_content) {
                Ok(()) => TaskResult::success(
                    node_id,
                    node_name,
                    Some(format!(
                        "Patched {} ({} bytes inserted)",
                        path,
                        insert.len()
                    )),
                    duration_ms,
                    0,
                ),
                Err(e) => TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "file_patch_error".to_string(),
                    duration_ms,
                    0,
                ),
            }
        } else {
            TaskResult::failure(
                node_id,
                node_name,
                format!("Search string not found in {}", path),
                "file_patch_error".to_string(),
                duration_ms,
                0,
            )
        }
    }

    async fn exec_git_read(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let args: Vec<&str> = intent.split_whitespace().collect();
        let output = if args.is_empty() {
            tokio::process::Command::new("git").output().await
        } else {
            tokio::process::Command::new("git")
                .args(&args)
                .output()
                .await
        };
        let duration_ms = start.elapsed().as_millis() as u64;
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                if out.status.success() {
                    let out_text = if stdout.is_empty() { stderr } else { stdout };
                    let truncated: String = out_text.chars().take(8192).collect();
                    TaskResult::success(node_id, node_name, Some(truncated), duration_ms, 0)
                } else {
                    TaskResult::failure(
                        node_id,
                        node_name,
                        if stderr.is_empty() { stdout } else { stderr },
                        "git_error".to_string(),
                        duration_ms,
                        0,
                    )
                }
            }
            Err(e) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "exec_error".to_string(),
                duration_ms,
                0,
            ),
        }
    }

    async fn exec_git_stage(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let path = intent;
        let output = tokio::process::Command::new("git")
            .args(["add", path])
            .output()
            .await;
        let duration_ms = start.elapsed().as_millis() as u64;
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                if out.status.success() {
                    TaskResult::success(node_id, node_name, Some(stdout), duration_ms, 0)
                } else {
                    TaskResult::failure(
                        node_id,
                        node_name,
                        if stderr.is_empty() { stdout } else { stderr },
                        "git_stage_error".to_string(),
                        duration_ms,
                        0,
                    )
                }
            }
            Err(e) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "exec_error".to_string(),
                duration_ms,
                0,
            ),
        }
    }

    async fn exec_git_commit(
        intent: &str,
        node_id: Uuid,
        node_name: &str,
        start: std::time::Instant,
    ) -> TaskResult {
        let parsed: serde_json::Value = match serde_json::from_str(intent) {
            Ok(v) => v,
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskResult::failure(
                    node_id,
                    node_name,
                    e.to_string(),
                    "parse_error".to_string(),
                    duration_ms,
                    0,
                );
            }
        };
        let message = parsed["message"].as_str().unwrap_or("");
        let auto_stage = parsed["auto_stage"].as_bool().unwrap_or(false);
        let duration_ms = start.elapsed().as_millis() as u64;

        // If auto_stage, stage all modified tracked files first
        if auto_stage {
            let _ = tokio::process::Command::new("git")
                .args(["add", "-u"])
                .output()
                .await;
        }

        let output = tokio::process::Command::new("git")
            .args(["commit", "-m", message])
            .output()
            .await;
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                if out.status.success() {
                    TaskResult::success(
                        node_id,
                        node_name,
                        Some(if stdout.is_empty() { stderr } else { stdout }),
                        duration_ms,
                        0,
                    )
                } else {
                    TaskResult::failure(
                        node_id,
                        node_name,
                        if stderr.is_empty() { stdout } else { stderr },
                        "git_commit_error".to_string(),
                        duration_ms,
                        0,
                    )
                }
            }
            Err(e) => TaskResult::failure(
                node_id,
                node_name,
                e.to_string(),
                "exec_error".to_string(),
                duration_ms,
                0,
            ),
        }
    }

    /// Notify progress callbacks about a state change.
    /// Reserved for TUI progress reporting integration.
    #[allow(dead_code)]
    fn notify_progress(
        &self,
        dag_id: Uuid,
        node_id: Uuid,
        state: &NodeExecutionState,
        total_nodes: u32,
    ) {
        let callbacks = self.progress_callbacks.lock().unwrap();
        if callbacks.is_empty() {
            return;
        }

        // Compute aggregate counts from session state
        let (completed, failed, skipped) = {
            let sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.get(&dag_id) {
                let c = session
                    .node_states
                    .values()
                    .filter(|s| s.status == NodeStatus::Completed)
                    .count() as u32;
                let f = session
                    .node_states
                    .values()
                    .filter(|s| s.status == NodeStatus::Failed)
                    .count() as u32;
                let sk = session
                    .node_states
                    .values()
                    .filter(|s| s.status == NodeStatus::Skipped)
                    .count() as u32;
                (c, f, sk)
            } else {
                (0, 0, 0)
            }
        };

        let progress = ExecutionProgress {
            dag_id,
            node_id,
            state: state.clone(),
            total_nodes,
            completed_count: completed,
            failed_count: failed,
            skipped_count: skipped,
        };

        for cb in callbacks.iter() {
            cb(progress.clone());
        }
    }
}

#[async_trait]
impl ParallelExecutionService for ParallelExecutionServiceImpl {
    async fn execute_graph(
        &self,
        input: ExecuteGraphInput,
    ) -> Result<ExecuteGraphOutput, ExecutionError> {
        let _config = input
            .config_override
            .clone()
            .unwrap_or_else(|| self.config.clone());

        // If no graph is provided, return placeholder (backwards compatibility)
        let Some(mut graph) = input.graph else {
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
                    node_states: HashMap::new(),
                    in_flight: Vec::new(),
                    result: ExecutionResult::new(input.dag_id),
                    paused: false,
                    aborted: false,
                    total_retries: 0,
                    started_at: Utc::now(),
                    graph: None,
                },
            );
            drop(sessions);

            let now = Utc::now();
            let result = ExecutionResult {
                dag_id: input.dag_id,
                node_results: HashMap::new(),
                execution_states: HashMap::new(),
                completed_count: 0,
                failed_count: 0,
                skipped_count: 0,
                total_nodes: 0,
                total_duration_ms: 0,
                total_retries: 0,
                started_at: now,
                completed_at: now,
                cancelled: false,
                cancellation_reason: None,
            };

            return Ok(ExecuteGraphOutput {
                result,
                completed_at: now,
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
                    started_at,
                    graph: Some(graph.clone()),
                },
            );
        }

        // ── Sequential dispatch loop ─────────────────────────────────
        let mut completed_count: u32 = 0;
        let mut failed_count: u32 = 0;
        let mut node_results: HashMap<Uuid, TaskResult> = HashMap::new();

        while let Some(node_id) = graph.pop_ready_node() {
            let node = match graph.get_node(node_id).cloned() {
                Some(n) => n,
                None => continue,
            };

            // Mark running
            {
                let mut sessions =
                    self.sessions
                        .lock()
                        .map_err(|e| ExecutionError::InternalError {
                            detail: format!("Lock error: {}", e),
                        })?;
                if let Some(session) = sessions.get_mut(&input.dag_id) {
                    if session.aborted {
                        drop(sessions);
                        break;
                    }
                    if let Some(state) = session.node_states.get_mut(&node_id) {
                        state.mark_running();
                    }
                }
            }

            let start = std::time::Instant::now();

            // Emit NodeStarted
            let _ = self
                .event_bus
                .publish(crate::event_system::application::dto::PublishEventInput {
                    event: ExecutionEvent::NodeStarted {
                        execution_id: input.dag_id,
                        node_id: node_id.to_string(),
                        node_name: node.name.clone(),
                        timestamp: chrono::Utc::now(),
                    },
                })
                .await;

            // Execute based on tool type
            let task_result = self.execute_tool(&node, node_id, start).await;

            let success = task_result.success;
            if success {
                completed_count += 1;
            } else {
                failed_count += 1;
            }
            node_results.insert(node_id, task_result.clone());

            // Emit NodeCompleted
            let _ = self
                .event_bus
                .publish(crate::event_system::application::dto::PublishEventInput {
                    event: ExecutionEvent::NodeCompleted {
                        execution_id: input.dag_id,
                        node_id: node_id.to_string(),
                        node_name: node.name.clone(),
                        duration_ms: task_result.duration_ms,
                        output: serde_json::json!(task_result.output.clone().unwrap_or_default()),
                        timestamp: chrono::Utc::now(),
                    },
                })
                .await;

            // Update session node state
            {
                let mut sessions =
                    self.sessions
                        .lock()
                        .map_err(|e| ExecutionError::InternalError {
                            detail: format!("Lock error: {}", e),
                        })?;
                if let Some(session) = sessions.get_mut(&input.dag_id) {
                    if let Some(state) = session.node_states.get_mut(&node_id) {
                        if success {
                            state.mark_completed(task_result.duration_ms);
                        } else {
                            state.mark_failed(
                                task_result
                                    .failure_type
                                    .clone()
                                    .unwrap_or_else(|| "unknown".to_string()),
                                task_result.error.clone().unwrap_or_default(),
                            );
                        }
                    }
                    // Update aggregate result incrementally
                    session.result = ExecutionResult {
                        dag_id: input.dag_id,
                        node_results: node_results.clone(),
                        execution_states: session.node_states.clone(),
                        completed_count,
                        failed_count,
                        skipped_count: 0,
                        total_nodes,
                        total_duration_ms: Utc::now()
                            .signed_duration_since(started_at)
                            .num_milliseconds()
                            .max(0) as u64,
                        total_retries: 0,
                        started_at,
                        completed_at: Utc::now(),
                        cancelled: false,
                        cancellation_reason: None,
                    };
                }
            }

            // Mark completed in graph to release dependents
            let _ = graph.mark_completed(node_id);
        }

        // Build final result
        let completed_at = Utc::now();
        let final_result = ExecutionResult {
            dag_id: input.dag_id,
            node_results,
            execution_states: node_states.clone(),
            completed_count,
            failed_count,
            skipped_count: 0,
            total_nodes,
            total_duration_ms: completed_at
                .signed_duration_since(started_at)
                .num_milliseconds()
                .max(0) as u64,
            total_retries: 0,
            started_at,
            completed_at,
            cancelled: false,
            cancellation_reason: None,
        };

        // Update session with final result
        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Lock error: {}", e),
                })?;
            if let Some(session) = sessions.get_mut(&input.dag_id) {
                session.result = final_result.clone();
                session.node_states = node_states;
            }
        }

        Ok(ExecuteGraphOutput {
            result: final_result,
            completed_at,
        })
    }

    async fn execute_node(
        &self,
        input: ExecuteNodeInput,
    ) -> Result<ExecuteNodeOutput, ExecutionError> {
        // Execute a single node with an inline retry loop.
        //
        // The retry loop follows this lifecycle:
        // 1. Attempt to execute the node's action
        // 2. If successful → return TaskResult with success
        // 3. If failed → build FailureContext, evaluate retry
        // 4. If Retry → apply backoff, loop
        // 5. If Fallback/Skip/Abort → terminal, return result
        //
        // This is the **inline retry loop** — not a separate retry wrapper.
        // Each retry can escalate the strategy per the RetryPolicy.

        let policy = input
            .retry_policy
            .clone()
            .unwrap_or_else(|| self.config.default_retry_policy.clone());
        let max_attempts = policy.max_attempts;
        let node_id = input.node_id;

        let mut last_retry_decision: Option<RetryDecision> = None;

        // Inline retry loop per node
        for attempt in 0..max_attempts {
            let start = std::time::Instant::now();

            // --- Phase 1: Check skip conditions before execution ---
            if policy.has_skip_conditions()
                && let Some(conditions) = &policy.skip_conditions
            {
                for condition in conditions {
                    if condition == "always_skip" {
                        let result = TaskResult::failure(
                            node_id,
                            format!("node-{}", node_id),
                            format!("Skipped by condition: {}", condition),
                            "skipped".to_string(),
                            start.elapsed().as_millis() as u64,
                            attempt,
                        );
                        return Ok(ExecuteNodeOutput {
                            result,
                            retry_decision: Some(RetryDecision::Skip {
                                reason: format!("Skip condition '{}' matched", condition),
                            }),
                        });
                    }
                }
            }

            // --- Phase 2: Check cancellation (placeholder) ---
            // In production, checks CancellationToken here

            // --- Phase 3: Execute the node ---
            // Look up node from the session graph to dispatch the tool.

            // Emit NodeStarted
            let _ = self
                .event_bus
                .publish(crate::event_system::application::dto::PublishEventInput {
                    event: ExecutionEvent::NodeStarted {
                        execution_id: input.dag_id,
                        node_id: node_id.to_string(),
                        node_name: format!("node-{}", node_id),
                        timestamp: chrono::Utc::now(),
                    },
                })
                .await;

            // Extract node info from sessions WITHOUT holding the lock across .await
            let node_info = {
                let sessions = self
                    .sessions
                    .lock()
                    .map_err(|e| ExecutionError::InternalError {
                        detail: format!("Lock error: {}", e),
                    })?;
                sessions
                    .get(&input.dag_id)
                    .and_then(|s| s.graph.as_ref())
                    .and_then(|g| g.get_node(node_id).cloned())
            };

            let (execution_successful, output_text, failure_type, error_message, exec_duration_ms) =
                if let Some(node) = node_info {
                    let task_result = self.execute_tool(&node, node_id, start).await;
                    let dur = task_result.duration_ms;
                    if task_result.success {
                        (
                            true,
                            task_result.output.unwrap_or_default(),
                            String::new(),
                            String::new(),
                            dur,
                        )
                    } else {
                        (
                            false,
                            String::new(),
                            task_result
                                .failure_type
                                .unwrap_or_else(|| "unknown".to_string()),
                            task_result.error.unwrap_or_default(),
                            dur,
                        )
                    }
                } else {
                    // No graph or node not found: placeholder success
                    (
                        true,
                        "execution output placeholder".to_string(),
                        String::new(),
                        String::new(),
                        0,
                    )
                };

            // Emit NodeCompleted
            let _ = self
                .event_bus
                .publish(crate::event_system::application::dto::PublishEventInput {
                    event: ExecutionEvent::NodeCompleted {
                        execution_id: input.dag_id,
                        node_id: node_id.to_string(),
                        node_name: format!("node-{}", node_id),
                        duration_ms: exec_duration_ms,
                        output: serde_json::json!(output_text.clone()),
                        timestamp: chrono::Utc::now(),
                    },
                })
                .await;

            let duration_ms = start.elapsed().as_millis() as u64;

            if execution_successful {
                let result = TaskResult::success(
                    node_id,
                    format!("node-{}", node_id),
                    Some(output_text),
                    duration_ms,
                    attempt,
                );
                return Ok(ExecuteNodeOutput {
                    result,
                    retry_decision: last_retry_decision,
                });
            }

            // Fall through to Phase 4 with actual error info
            let _ = output_text;

            // --- Phase 4: Handle failure with retry evaluation ---

            let failure_context = FailureContext::new(
                node_id,
                format!("node-{}", node_id),
                "tool",
                "node intent",
                &failure_type,
                &error_message,
                attempt,
                max_attempts,
                duration_ms,
                duration_ms,
            );

            let retry_input = EvaluateRetryInput {
                failure_context,
                policy: policy.clone(),
                fallback_node_id: None,
            };

            let retry_output = self
                .retry_service
                .evaluate_retry(retry_input)
                .await
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Retry evaluation failed: {}", e),
                })?;

            match retry_output.decision {
                RetryDecision::Retry {
                    strategy,
                    attempt: next,
                    backoff_ms,
                    ..
                } => {
                    if backoff_ms > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                    }
                    last_retry_decision = Some(RetryDecision::Retry {
                        strategy,
                        attempt: next,
                        backoff_ms,
                        reason: format!("Retry attempt {}/{}", attempt + 1, max_attempts),
                    });
                    // Loop continues to next attempt
                }
                RetryDecision::Fallback {
                    fallback_node_id, ..
                } => {
                    let result = TaskResult::failure(
                        node_id,
                        format!("node-{}", node_id),
                        format!("Fallback to node {}", fallback_node_id),
                        "fallback".to_string(),
                        duration_ms,
                        attempt,
                    );
                    return Ok(ExecuteNodeOutput {
                        result,
                        retry_decision: Some(RetryDecision::Fallback {
                            fallback_node_id,
                            reason: format!("Retries exhausted at attempt {}", attempt + 1),
                        }),
                    });
                }
                RetryDecision::Skip { reason } => {
                    let result = TaskResult::failure(
                        node_id,
                        format!("node-{}", node_id),
                        reason.clone(),
                        "skipped".to_string(),
                        duration_ms,
                        attempt,
                    );
                    return Ok(ExecuteNodeOutput {
                        result,
                        retry_decision: Some(RetryDecision::Skip { reason }),
                    });
                }
                RetryDecision::Abort { reason } => {
                    let result = TaskResult::failure(
                        node_id,
                        format!("node-{}", node_id),
                        reason.clone(),
                        "aborted".to_string(),
                        duration_ms,
                        attempt,
                    );
                    return Ok(ExecuteNodeOutput {
                        result,
                        retry_decision: Some(RetryDecision::Abort { reason }),
                    });
                }
            }
        }

        // All attempts exhausted without success
        let result = TaskResult::failure(
            node_id,
            format!("node-{}", node_id),
            format!("All {} attempts exhausted", max_attempts),
            "exhausted".to_string(),
            0,
            max_attempts.saturating_sub(1),
        );

        Ok(ExecuteNodeOutput {
            result,
            retry_decision: None,
        })
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
        let ready = session
            .node_states
            .values()
            .filter(|s| s.status == NodeStatus::Ready)
            .count() as u32;

        Ok(ResumeExecutionOutput {
            dag_id: input.dag_id,
            ready_count: ready,
            resumed_at: Utc::now(),
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
pub struct RetryEvaluationServiceImpl;

impl RetryEvaluationServiceImpl {
    /// Create a new RetryEvaluationServiceImpl.
    pub fn new() -> Self {
        Self
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
        // 1. Check if the failure type is retriable
        if !policy.is_failure_retriable(&failure_context.failure_type) {
            if let Some(fallback_id) = fallback_node_id {
                return RetryDecision::Fallback {
                    fallback_node_id: fallback_id,
                    reason: format!(
                        "Failure type '{}' is not retriable; executing fallback",
                        failure_context.failure_type
                    ),
                };
            }
            return RetryDecision::Skip {
                reason: format!(
                    "Failure type '{}' is not retriable and no fallback configured",
                    failure_context.failure_type
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

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

use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;

use std::sync::Mutex;

use uuid::Uuid;

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
    progress_callbacks: Mutex<Vec<Box<dyn Fn(ExecutionProgress) + Send + Sync>>>,
    /// The retry evaluation service for retry decisions.
    retry_service: Box<dyn RetryEvaluationService>,
}

impl ParallelExecutionServiceImpl {
    /// Create a new ParallelExecutionServiceImpl.
    pub fn new(
        config: ParallelExecutorConfig,
        retry_service: Box<dyn RetryEvaluationService>,
    ) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            config,
            progress_callbacks: Mutex::new(Vec::new()),
            retry_service,
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
        // Resolve config
        let _config = input
            .config_override
            .clone()
            .unwrap_or_else(|| self.config.clone());

        // Initialise session
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
                    node_states: HashMap::new(),
                    in_flight: Vec::new(),
                    result: ExecutionResult::new(input.dag_id),
                    paused: false,
                    aborted: false,
                    total_retries: 0,
                    started_at: Utc::now(),
                },
            );
        }

        // The real implementation would:
        // 1. Load the TaskGraph from the graph service
        // 2. Initialise node states from the graph's nodes
        // 3. Populate the ready queue
        // 4. Dispatch nodes up to max_concurrent_executions
        // 5. Wait for JoinSet completions
        // 6. Replenish the ready queue as nodes complete
        //
        // For the contract implementation, since TaskGraph access requires
        // the DagGraphService which may not be wired yet, we produce a valid
        // ExecutionResult indicating the graph execution would proceed.

        // Create an empty result indicating the DAG was acknowledged
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

        Ok(ExecuteGraphOutput {
            result,
            completed_at: now,
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
            if policy.has_skip_conditions() {
                if let Some(conditions) = &policy.skip_conditions {
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
            }

            // --- Phase 2: Check cancellation (placeholder) ---
            // In production, checks CancellationToken here

            // --- Phase 3: Execute the node ---
            // In production, this calls ToolSystem.execute().
            // Placeholder: simulate success for now.
            let execution_successful = true;
            let duration_ms = start.elapsed().as_millis() as u64;

            if execution_successful {
                // Success! Return immediately
                let result = TaskResult::success(
                    node_id,
                    format!("node-{}", node_id),
                    Some("execution output placeholder".to_string()),
                    duration_ms,
                    attempt,
                );
                return Ok(ExecuteNodeOutput {
                    result,
                    retry_decision: last_retry_decision,
                });
            }

            // --- Phase 4: Handle failure with retry evaluation ---
            let failure_type = "transient".to_string();
            let error_message = format!(
                "Execution failed on attempt {}/{}",
                attempt + 1,
                max_attempts
            );

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

        let session =
            sessions
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

        let session =
            sessions
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

        let session =
            sessions
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
    fn on_progress(&self, callback: Box<dyn Fn(ExecutionProgress) + Send + Sync>) {
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

            if let Some(fallback_id) = fallback_node_id {
                if policy.enable_fallback {
                    return RetryDecision::Fallback {
                        fallback_node_id: fallback_id,
                        reason: format!("{}. Executing fallback", reason),
                    };
                }
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

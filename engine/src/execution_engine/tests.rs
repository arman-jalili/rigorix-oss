//! Execution Engine tests — contract compliance verification.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: Contract Freeze — test skeletons for implementation validation
//! Issue: issue-contract-freeze
//!
//! These tests verify that the sealed contracts are internally consistent:
//! - Domain types construct and default correctly
//! - DTOs serialise/deserialise as expected
//! - Enums have valid string representations
//!
//! Implementation-specific tests will be added during the implementation phase.

use uuid::Uuid;

use crate::execution_engine::domain::{
    BackoffStrategy, ExecutionResult, FailureContext, NodeExecutionState, NodeStatus,
    ParallelExecutorConfig, RetryDecision, RetryPolicy, RetryStrategy, TaskResult,
};

// ---------------------------------------------------------------------------
// Domain Type Validation Tests
// ---------------------------------------------------------------------------

#[test]
fn test_parallel_executor_config_defaults() {
    let config = ParallelExecutorConfig::default();
    assert_eq!(config.max_concurrent_executions, 4);
    assert!(config.enable_cancellation);
    assert!(config.enable_enforcement);
    assert!(config.enable_fallback);
    assert!(config.enable_validation);
    assert_eq!(config.max_total_retries_per_session, 100);
    assert_eq!(config.max_failures_before_abort, 0);
}

#[test]
fn test_retry_policy_defaults() {
    let policy = RetryPolicy::default();
    assert_eq!(policy.max_attempts, 4);
    assert_eq!(policy.retry_strategies.len(), 3);
    assert!(policy.enable_fallback);
    assert!(!policy.skip_on_exhaustion);
    assert!(policy.retryable_failures.is_empty());
}

#[test]
fn test_retry_policy_strategy_for_attempt() {
    let policy = RetryPolicy::default();
    // attempt 0 → index 0: SameOperation
    assert_eq!(
        policy.strategy_for_attempt(0),
        RetryStrategy::SameOperation
    );
    // attempt 1 → index 1: SameOperation
    assert_eq!(
        policy.strategy_for_attempt(1),
        RetryStrategy::SameOperation
    );
    // attempt 2 → index 2: ExpandContext
    assert_eq!(
        policy.strategy_for_attempt(2),
        RetryStrategy::ExpandContext
    );
    // out of bounds → last strategy
    assert_eq!(
        policy.strategy_for_attempt(5),
        RetryStrategy::ExpandContext
    );
}

#[test]
fn test_backoff_strategy_fixed() {
    let strategy = BackoffStrategy::Fixed { base_delay_ms: 500 };
    assert_eq!(strategy.compute_delay_ms(0), 500);
    assert_eq!(strategy.compute_delay_ms(3), 500); // always 500
}

#[test]
fn test_backoff_strategy_exponential() {
    let strategy = BackoffStrategy::Exponential {
        base_delay_ms: 100,
        multiplier: 2.0,
        max_delay_ms: 10_000,
    };
    assert_eq!(strategy.compute_delay_ms(0), 100); // 100 * 2^0
    assert_eq!(strategy.compute_delay_ms(1), 200); // 100 * 2^1
    assert_eq!(strategy.compute_delay_ms(2), 400); // 100 * 2^2
    assert_eq!(strategy.compute_delay_ms(3), 800); // 100 * 2^3
}

#[test]
fn test_backoff_strategy_exponential_capped() {
    let strategy = BackoffStrategy::Exponential {
        base_delay_ms: 100,
        multiplier: 2.0,
        max_delay_ms: 500,
    };
    assert_eq!(strategy.compute_delay_ms(0), 100);
    assert_eq!(strategy.compute_delay_ms(1), 200);
    assert_eq!(strategy.compute_delay_ms(2), 400);
    assert_eq!(strategy.compute_delay_ms(3), 500); // capped at 500
    assert_eq!(strategy.compute_delay_ms(10), 500); // still capped
}

#[test]
fn test_backoff_strategy_linear() {
    let strategy = BackoffStrategy::Linear {
        base_delay_ms: 100,
        step_ms: 50,
        max_delay_ms: 10_000,
    };
    assert_eq!(strategy.compute_delay_ms(0), 100); // 100 + (50 * 0)
    assert_eq!(strategy.compute_delay_ms(1), 150); // 100 + (50 * 1)
    assert_eq!(strategy.compute_delay_ms(2), 200); // 100 + (50 * 2)
}

#[test]
fn test_backoff_strategy_immediate() {
    let strategy = BackoffStrategy::Immediate;
    assert_eq!(strategy.compute_delay_ms(0), 0);
    assert_eq!(strategy.compute_delay_ms(10), 0);
}

#[test]
fn test_backoff_strategy_as_str() {
    assert_eq!(
        BackoffStrategy::Fixed { base_delay_ms: 100 }.as_str(),
        "fixed"
    );
    assert_eq!(
        BackoffStrategy::Exponential {
            base_delay_ms: 100,
            multiplier: 2.0,
            max_delay_ms: 30_000
        }
        .as_str(),
        "exponential"
    );
    assert_eq!(
        BackoffStrategy::Linear {
            base_delay_ms: 100,
            step_ms: 50,
            max_delay_ms: 10_000
        }
        .as_str(),
        "linear"
    );
    assert_eq!(BackoffStrategy::Immediate.as_str(), "immediate");
}

#[test]
fn test_retry_strategy_as_str() {
    assert_eq!(RetryStrategy::SameOperation.as_str(), "same_operation");
    assert_eq!(RetryStrategy::ExpandContext.as_str(), "expand_context");
    assert_eq!(RetryStrategy::SimplifyOperation.as_str(), "simplify_operation");
    assert_eq!(RetryStrategy::AlternateApproach.as_str(), "alternate_approach");
    assert_eq!(RetryStrategy::SkipAndContinue.as_str(), "skip_and_continue");
}

#[test]
fn test_retry_strategy_is_skip() {
    assert!(!RetryStrategy::SameOperation.is_skip());
    assert!(RetryStrategy::SkipAndContinue.is_skip());
}

#[test]
fn test_node_status_transitions() {
    assert!(!NodeStatus::Pending.is_terminal());
    assert!(!NodeStatus::Ready.is_terminal());
    assert!(!NodeStatus::Running.is_terminal());
    assert!(NodeStatus::Completed.is_terminal());
    assert!(NodeStatus::Failed.is_terminal());
    assert!(NodeStatus::Skipped.is_terminal());
}

#[test]
fn test_node_status_can_execute() {
    assert!(!NodeStatus::Pending.can_execute());
    assert!(NodeStatus::Ready.can_execute());
    assert!(!NodeStatus::Running.can_execute());
    assert!(!NodeStatus::Completed.can_execute());
    assert!(!NodeStatus::Failed.can_execute());
    assert!(!NodeStatus::Skipped.can_execute());
}

#[test]
fn test_node_status_as_str() {
    assert_eq!(NodeStatus::Pending.as_str(), "pending");
    assert_eq!(NodeStatus::Ready.as_str(), "ready");
    assert_eq!(NodeStatus::Running.as_str(), "running");
    assert_eq!(NodeStatus::Completed.as_str(), "completed");
    assert_eq!(NodeStatus::Failed.as_str(), "failed");
    assert_eq!(NodeStatus::Skipped.as_str(), "skipped");
}

#[test]
fn test_node_execution_state_lifecycle() {
    let node_id = Uuid::new_v4();
    let mut state = NodeExecutionState::new(node_id, "test-node");

    assert_eq!(state.status, NodeStatus::Pending);
    assert_eq!(state.retry_attempts, 0);

    state.mark_ready();
    assert_eq!(state.status, NodeStatus::Ready);
    assert!(state.ready_at.is_some());

    state.mark_running();
    assert_eq!(state.status, NodeStatus::Running);
    assert!(state.started_at.is_some());

    state.mark_completed(100);
    assert_eq!(state.status, NodeStatus::Completed);
    assert_eq!(state.last_duration_ms, Some(100));
    assert!(state.is_terminal());
}

#[test]
fn test_node_execution_state_retry() {
    let node_id = Uuid::new_v4();
    let mut state = NodeExecutionState::new(node_id, "retry-node");

    state.mark_ready();
    state.mark_running();
    state.mark_failed("transient".to_string(), "timeout".to_string());
    assert_eq!(state.status, NodeStatus::Failed);
    assert!(state.is_terminal());

    // Mark for retry resets to Ready
    state.mark_for_retry();
    assert_eq!(state.status, NodeStatus::Ready);
    assert_eq!(state.retry_attempts, 1);
    assert!(!state.is_terminal());

    // Complete on second attempt
    state.mark_running();
    state.mark_completed(200);
    assert_eq!(state.status, NodeStatus::Completed);
    assert_eq!(state.total_duration_ms, 200); // only the last attempt counted
}

#[test]
fn test_task_result_success() {
    let node_id = Uuid::new_v4();
    let result = TaskResult::success(node_id, "build", Some("ok".to_string()), 500, 0);
    assert!(result.success);
    assert_eq!(result.node_id, node_id);
    assert_eq!(result.output, Some("ok".to_string()));
    assert_eq!(result.duration_ms, 500);
    assert!(result.error.is_none());
}

#[test]
fn test_task_result_failure() {
    let node_id = Uuid::new_v4();
    let result = TaskResult::failure(
        node_id,
        "build",
        "build error".to_string(),
        "compile_error".to_string(),
        300,
        2,
    );
    assert!(!result.success);
    assert_eq!(result.error, Some("build error".to_string()));
    assert_eq!(result.failure_type, Some("compile_error".to_string()));
    assert_eq!(result.retry_attempts, 2);
}

#[test]
fn test_execution_result_aggregation() {
    let dag_id = Uuid::new_v4();
    let mut exec_result = ExecutionResult::new(dag_id);
    exec_result.total_nodes = 3;

    let node_a = Uuid::new_v4();
    let node_b = Uuid::new_v4();
    let node_c = Uuid::new_v4();

    exec_result.record_result(TaskResult::success(node_a, "a", None, 100, 0));
    exec_result.record_result(TaskResult::success(node_b, "b", Some("output".into()), 200, 1));
    exec_result.record_result(TaskResult::failure(
        node_c,
        "c",
        "crashed".into(),
        "panic".into(),
        50,
        3,
    ));

    assert_eq!(exec_result.completed_count, 2);
    assert_eq!(exec_result.failed_count, 1);
    assert_eq!(exec_result.total_retries, 4); // 0 + 1 + 3
    assert_eq!(exec_result.total_duration_ms, 350); // 100 + 200 + 50
    assert!(!exec_result.all_succeeded());
    assert!(exec_result.has_failures());
}

#[test]
fn test_execution_result_all_succeeded() {
    let dag_id = Uuid::new_v4();
    let mut exec_result = ExecutionResult::new(dag_id);
    exec_result.total_nodes = 2;

    let node_a = Uuid::new_v4();
    let node_b = Uuid::new_v4();

    exec_result.record_result(TaskResult::success(node_a, "a", None, 100, 0));
    exec_result.record_result(TaskResult::success(node_b, "b", None, 200, 0));

    assert!(exec_result.all_succeeded());
    assert!(!exec_result.has_failures());
    assert!(!exec_result.has_issues());
}

#[test]
fn test_execution_result_display() {
    let dag_id = Uuid::new_v4();
    let mut exec_result = ExecutionResult::new(dag_id);
    exec_result.total_nodes = 1;

    let display = format!("{}", exec_result);
    assert!(display.contains(&dag_id.to_string()));
    assert!(display.contains("completed=0"));
}

#[test]
fn test_retry_policy_is_failure_retriable() {
    // Empty retryable_failures means all failures retriable
    let policy = RetryPolicy::default();
    assert!(policy.is_failure_retriable("transient"));
    assert!(policy.is_failure_retriable("compile_error"));

    // Filtered retryable_failures
    let mut policy = RetryPolicy::default();
    policy.retryable_failures = vec!["transient".to_string(), "lsp_conflict".to_string()];
    assert!(policy.is_failure_retriable("transient"));
    assert!(policy.is_failure_retriable("lsp_conflict"));
    assert!(!policy.is_failure_retriable("compile_error"));
    assert!(!policy.is_failure_retriable("permanent"));
}

#[test]
fn test_failure_context_basic() {
    let node_id = Uuid::new_v4();
    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile the project",
        "transient",
        "network timeout",
        0,
        4,
        1000,
        1000,
    );

    assert!(ctx.is_first_failure());
    assert!(!ctx.is_exhausted());
    assert_eq!(ctx.retries_remaining(), 3);
    assert_eq!(ctx.node_name, "test-node");
    assert_eq!(ctx.tool, "cargo build");
}

#[test]
fn test_failure_context_exhausted() {
    let node_id = Uuid::new_v4();
    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile the project",
        "compile_error",
        "syntax error",
        3, // attempt 3 (4th attempt = last)
        4, // max 4 attempts
        500,
        2500,
    );

    assert!(!ctx.is_first_failure());
    assert!(ctx.is_exhausted());
    assert_eq!(ctx.retries_remaining(), 0);
}

#[test]
fn test_retry_decision_is_retry() {
    let node_id = Uuid::new_v4();
    let decision = RetryDecision::Retry {
        strategy: RetryStrategy::SameOperation,
        attempt: 2,
        backoff_ms: 200,
        reason: "transient failure, retrying".to_string(),
    };
    assert!(decision.is_retry());
    assert!(!decision.is_terminal());
}

#[test]
fn test_retry_decision_is_terminal() {
    let node_id = Uuid::new_v4();
    let decision = RetryDecision::Skip {
        reason: "skip".to_string(),
    };
    assert!(!decision.is_retry());
    assert!(decision.is_terminal());

    let fallback = RetryDecision::Fallback {
        fallback_node_id: Uuid::new_v4(),
        reason: "fallback".to_string(),
    };
    assert!(fallback.is_terminal());
}

#[test]
fn test_parallel_executor_config_custom() {
    let config = ParallelExecutorConfig {
        max_concurrent_executions: 8,
        max_failures_before_abort: 3,
        ..Default::default()
    };
    assert_eq!(config.max_concurrent_executions, 8);
    assert_eq!(config.max_failures_before_abort, 3);
    assert!(config.enable_cancellation);
}

#[test]
fn test_node_execution_state_skipped() {
    let node_id = Uuid::new_v4();
    let mut state = NodeExecutionState::new(node_id, "skip-node");
    state.mark_skipped("not needed".to_string());
    assert_eq!(state.status, NodeStatus::Skipped);
    assert!(state.is_terminal());
}

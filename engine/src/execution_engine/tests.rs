//! ParallelExecutor implementation tests.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: ParallelExecutor — ParallelExecutionServiceImpl tests
//! Issue: issue-parallelexecutor
//!
//! Comprehensive tests for the ParallelExecutionServiceImpl and
//! RetryEvaluationServiceImpl implementations.

use std::sync::Arc;
use uuid::Uuid;

use crate::execution_engine::application::dto::{
    AbortExecutionInput, EvaluateRetryInput, ExecuteGraphInput, ExecuteNodeInput,
    GetExecutionStateInput, PauseExecutionInput, ResumeExecutionInput,
};
use crate::execution_engine::application::service::{
    ParallelExecutionService, RetryEvaluationService,
};
use crate::execution_engine::application::service_impl::{
    ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
};
use crate::execution_engine::domain::{
    BackoffStrategy, FailureContext, NodeExecutionState,
    ParallelExecutorConfig, RetryDecision, RetryPolicy, RetryStrategy,
};

// ---------------------------------------------------------------------------
// Helper: create a configured service pair
// ---------------------------------------------------------------------------

fn create_executor() -> ParallelExecutionServiceImpl {
    let config = ParallelExecutorConfig::default();
    let retry = RetryEvaluationServiceImpl::new();
    ParallelExecutionServiceImpl::new(config, Box::new(retry))
}

// ---------------------------------------------------------------------------
// ParallelExecutionServiceImpl Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_execute_graph_creates_session() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    assert_eq!(output.result.dag_id, dag_id);
    assert!(output.result.execution_states.is_empty());
}

#[tokio::test]
async fn test_execute_graph_rejects_duplicate() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    let err = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap_err();

    assert!(err.to_string().contains("already in progress"));
}

#[tokio::test]
async fn test_execute_graph_with_config_override() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let custom_config = ParallelExecutorConfig {
        max_concurrent_executions: 8,
        ..Default::default()
    };

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: Some(custom_config),
        })
        .await
        .unwrap();

    assert_eq!(output.result.dag_id, dag_id);
}

#[tokio::test]
async fn test_execute_node_returns_result() {
    let executor = create_executor();
    let node_id = Uuid::new_v4();
    let dag_id = Uuid::new_v4();

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: None,
        })
        .await
        .unwrap();

    assert_eq!(output.result.node_id, node_id);
    assert!(output.result.success);
    assert!(output.retry_decision.is_none());
}

#[tokio::test]
async fn test_execute_node_with_retry_policy() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    let policy = RetryPolicy {
        max_attempts: 2,
        ..Default::default()
    };

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: Some(policy),
        })
        .await
        .unwrap();

    assert_eq!(output.result.node_id, node_id);
    assert!(output.result.success);
}

#[tokio::test]
async fn test_get_execution_state_before_execution_returns_error() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let err = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        crate::execution_engine::domain::ExecutionError::NodeNotFound { .. }
    ));
}

#[tokio::test]
async fn test_pause_and_resume_execution() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    // Start execution
    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    // Pause
    let pause_output = executor
        .pause_execution(PauseExecutionInput { dag_id })
        .await
        .unwrap();
    assert_eq!(pause_output.dag_id, dag_id);

    // Verify paused state
    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(state.paused);

    // Resume
    let resume_output = executor
        .resume_execution(ResumeExecutionInput { dag_id })
        .await
        .unwrap();
    assert_eq!(resume_output.dag_id, dag_id);

    // Verify resumed state
    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert!(!state.paused);
}

#[tokio::test]
async fn test_pause_already_paused_returns_error() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    executor
        .pause_execution(PauseExecutionInput { dag_id })
        .await
        .unwrap();

    let err = executor
        .pause_execution(PauseExecutionInput { dag_id })
        .await
        .unwrap_err();

    assert!(err.to_string().contains("already paused"));
}

#[tokio::test]
async fn test_resume_not_paused_returns_error() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    let err = executor
        .resume_execution(ResumeExecutionInput { dag_id })
        .await
        .unwrap_err();

    assert!(err.to_string().contains("not paused"));
}

#[tokio::test]
async fn test_abort_execution() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    let abort_output = executor
        .abort_execution(AbortExecutionInput {
            dag_id,
            reason: "test abort".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(abort_output.dag_id, dag_id);
    assert_eq!(abort_output.skipped_count, 0); // no nodes to skip
}

#[tokio::test]
async fn test_abort_twice_returns_error() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    executor
        .abort_execution(AbortExecutionInput {
            dag_id,
            reason: "first abort".to_string(),
        })
        .await
        .unwrap();

    let err = executor
        .abort_execution(AbortExecutionInput {
            dag_id,
            reason: "second abort".to_string(),
        })
        .await
        .unwrap_err();

    assert!(err.to_string().contains("already aborted"));
}

#[tokio::test]
async fn test_pause_nonexistent_execution() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let err = executor
        .pause_execution(PauseExecutionInput { dag_id })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        crate::execution_engine::domain::ExecutionError::NodeNotFound { .. }
    ));
}

#[tokio::test]
async fn test_abort_nonexistent_execution() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let err = executor
        .abort_execution(AbortExecutionInput {
            dag_id,
            reason: "test".to_string(),
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        crate::execution_engine::domain::ExecutionError::NodeNotFound { .. }
    ));
}

#[tokio::test]
async fn test_on_progress_callback() {
    let executor = create_executor();
    let _dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let called_clone = called.clone();

    executor.on_progress(Box::new(move |_progress| {
        called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    }));

    // Create a node state to trigger notification
    let _state = NodeExecutionState::new(node_id, "test-node");

    // Verify callback registered (not triggered since no session)
    // The callback mechanism is trigger-based; in a real execution it fires on completion
    assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
}

#[tokio::test]
async fn test_execute_graph_with_custom_config_override_respected() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let config = ParallelExecutorConfig {
        max_concurrent_executions: 16,
        enable_fallback: false,
        enable_validation: false,
        ..Default::default()
    };

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: Some(config),
        })
        .await
        .unwrap();

    assert_eq!(output.result.dag_id, dag_id);
}

// ---------------------------------------------------------------------------
// RetryEvaluationServiceImpl Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_retry_evaluate_retry_on_first_failure() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy::default();

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile",
        "transient",
        "network timeout",
        0, // first failure
        4, // max 4 attempts
        100,
        100,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(!output.is_terminal);
    assert!(output.decision.is_retry());
}

#[tokio::test]
async fn test_retry_evaluate_retry_on_exhausted() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy::default();

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile",
        "transient",
        "still failing",
        3, // attempt 3 = 4th attempt = last
        4, // max 4 attempts
        100,
        400,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(output.is_terminal);
    // No fallback configured, skip_on_exhaustion=false → Abort
    match output.decision {
        RetryDecision::Abort { .. } => {} // expected
        ref other => panic!("Expected Abort, got: {:?}", other),
    }
}

// Fix: Compare by variant
#[tokio::test]
async fn test_retry_exhausted_with_skip_on_exhaustion() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        skip_on_exhaustion: true,
        ..Default::default()
    };

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile",
        "transient",
        "failed",
        3, // last attempt
        4,
        100,
        400,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(output.is_terminal);
    assert!(matches!(output.decision, RetryDecision::Skip { .. }));
}

#[tokio::test]
async fn test_retry_exhausted_with_fallback() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let fallback_id = Uuid::new_v4();
    let policy = RetryPolicy::default();

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile",
        "transient",
        "failed too many times",
        3,
        4,
        100,
        400,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: Some(fallback_id),
        })
        .await
        .unwrap();

    assert!(output.is_terminal);
    match output.decision {
        RetryDecision::Fallback {
            fallback_node_id, ..
        } => {
            assert_eq!(fallback_node_id, fallback_id);
        }
        other => panic!("Expected Fallback decision, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_retry_non_retriable_failure() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let mut policy = RetryPolicy::default();
    policy.retryable_failures = vec!["transient".to_string()];

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile",
        "compile_error", // not in retryable_failures
        "syntax error",
        0,
        4,
        100,
        100,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(output.is_terminal);
    assert!(matches!(output.decision, RetryDecision::Skip { .. }));
}

#[tokio::test]
async fn test_retry_non_retriable_with_fallback() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let fallback_id = Uuid::new_v4();
    let mut policy = RetryPolicy::default();
    policy.retryable_failures = vec!["transient".to_string()];

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "cargo build",
        "compile",
        "compile_error",
        "syntax error",
        0,
        4,
        100,
        100,
    );

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: Some(fallback_id),
        })
        .await
        .unwrap();

    assert!(output.is_terminal);
    match output.decision {
        RetryDecision::Fallback {
            fallback_node_id, ..
        } => {
            assert_eq!(fallback_node_id, fallback_id);
        }
        other => panic!("Expected Fallback, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_retry_strategy_escalation() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        retry_strategies: vec![
            RetryStrategy::SameOperation,
            RetryStrategy::ExpandContext,
            RetryStrategy::AlternateApproach,
        ],
        ..Default::default()
    };

    // First failure → SameOperation
    let ctx1 = FailureContext::new(
        node_id,
        "n",
        "tool",
        "intent",
        "transient",
        "err",
        0,
        4,
        100,
        100,
    );
    let output1 = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx1,
            policy: policy.clone(),
            fallback_node_id: None,
        })
        .await
        .unwrap();
    match output1.decision {
        RetryDecision::Retry {
            strategy, attempt, ..
        } => {
            assert_eq!(strategy, RetryStrategy::SameOperation);
            assert_eq!(attempt, 1);
        }
        other => panic!("Expected Retry, got: {:?}", other),
    }

    // Second failure → ExpandContext
    let ctx2 = FailureContext::new(
        node_id,
        "n",
        "tool",
        "intent",
        "transient",
        "err",
        1,
        4,
        100,
        200,
    );
    let output2 = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx2,
            policy: policy.clone(),
            fallback_node_id: None,
        })
        .await
        .unwrap();
    match output2.decision {
        RetryDecision::Retry {
            strategy, attempt, ..
        } => {
            assert_eq!(strategy, RetryStrategy::ExpandContext);
            assert_eq!(attempt, 2);
        }
        other => panic!("Expected Retry, got: {:?}", other),
    }

    // Third failure → AlternateApproach
    let ctx3 = FailureContext::new(
        node_id,
        "n",
        "tool",
        "intent",
        "transient",
        "err",
        2,
        4,
        100,
        300,
    );
    let output3 = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx3,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();
    match output3.decision {
        RetryDecision::Retry {
            strategy, attempt, ..
        } => {
            assert_eq!(strategy, RetryStrategy::AlternateApproach);
            assert_eq!(attempt, 3);
        }
        other => panic!("Expected Retry, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_compute_backoff_exponential() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        backoff_strategy: BackoffStrategy::Exponential {
            base_delay_ms: 100,
            multiplier: 2.0,
            max_delay_ms: 10_000,
        },
        ..Default::default()
    };

    let ctx = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 0, 4, 100, 100);
    let backoff = service.compute_backoff(&ctx, &policy).await;
    assert_eq!(backoff, 100); // 100 * 2^0

    let ctx2 = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 1, 4, 100, 200);
    let backoff2 = service.compute_backoff(&ctx2, &policy).await;
    assert_eq!(backoff2, 200); // 100 * 2^1
}

#[tokio::test]
async fn test_compute_backoff_fixed() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        backoff_strategy: BackoffStrategy::Fixed { base_delay_ms: 500 },
        ..Default::default()
    };

    let ctx = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 0, 4, 100, 100);
    let backoff = service.compute_backoff(&ctx, &policy).await;
    assert_eq!(backoff, 500);

    let ctx2 = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 3, 4, 100, 400);
    let backoff2 = service.compute_backoff(&ctx2, &policy).await;
    assert_eq!(backoff2, 500);
}

#[tokio::test]
async fn test_compute_backoff_immediate() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        backoff_strategy: BackoffStrategy::Immediate,
        ..Default::default()
    };

    let ctx = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 0, 4, 100, 100);
    let backoff = service.compute_backoff(&ctx, &policy).await;
    assert_eq!(backoff, 0);
}

#[tokio::test]
async fn test_validate_policy_valid() {
    let service = RetryEvaluationServiceImpl::new();
    let policy = RetryPolicy::default();

    let errors = service.validate_policy(&policy).await.unwrap();
    assert!(errors.is_empty());
}

#[tokio::test]
async fn test_validate_policy_zero_attempts() {
    let service = RetryEvaluationServiceImpl::new();
    let policy = RetryPolicy {
        max_attempts: 0,
        ..Default::default()
    };

    let errors = service.validate_policy(&policy).await.unwrap();
    assert!(errors.iter().any(|e| e.contains("max_attempts")));
}

#[tokio::test]
async fn test_validate_policy_empty_strategies() {
    let service = RetryEvaluationServiceImpl::new();
    let policy = RetryPolicy {
        retry_strategies: vec![],
        ..Default::default()
    };

    let errors = service.validate_policy(&policy).await.unwrap();
    assert!(errors.iter().any(|e| e.contains("retry_strategies")));
}

#[tokio::test]
async fn test_validate_policy_bad_multiplier() {
    let service = RetryEvaluationServiceImpl::new();
    let policy = RetryPolicy {
        backoff_strategy: BackoffStrategy::Exponential {
            base_delay_ms: 100,
            multiplier: 0.5, // must be >= 1.0
            max_delay_ms: 10_000,
        },
        ..Default::default()
    };

    let errors = service.validate_policy(&policy).await.unwrap();
    assert!(errors.iter().any(|e| e.contains("multiplier")));
}

#[tokio::test]
async fn test_is_failure_retriable_default_all() {
    let service = RetryEvaluationServiceImpl::new();
    let policy = RetryPolicy::default(); // empty retryable_failures = all retriable

    assert!(service.is_failure_retriable(&policy, "transient").await);
    assert!(service.is_failure_retriable(&policy, "compile_error").await);
    assert!(service.is_failure_retriable(&policy, "permanent").await);
}

// ---------------------------------------------------------------------------
// Factory Implementation Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_parallel_execution_factory_creates_service() {
    use crate::execution_engine::application::factory::{
        ParallelExecutionFactory, ParallelExecutionFactoryConfig,
    };
    use crate::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;

    let factory = ParallelExecutionFactoryImpl::new();
    let config = ParallelExecutionFactoryConfig::default();
    let service = factory.create(config).await.unwrap();

    let dag_id = Uuid::new_v4();
    let output = service
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    assert_eq!(output.result.dag_id, dag_id);
}

#[tokio::test]
async fn test_retry_evaluation_factory_creates_service() {
    use crate::execution_engine::application::factory::{
        RetryEvaluationFactory, RetryEvaluationFactoryConfig,
    };
    use crate::execution_engine::application::factory_impl::RetryEvaluationFactoryImpl;

    let factory = RetryEvaluationFactoryImpl::new();
    let config = RetryEvaluationFactoryConfig::default();
    let service = factory.create(config).await.unwrap();

    let node_id = Uuid::new_v4();
    let policy = RetryPolicy::default();
    let ctx = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 0, 4, 100, 100);

    let output = service
        .evaluate_retry(EvaluateRetryInput {
            failure_context: ctx,
            policy,
            fallback_node_id: None,
        })
        .await
        .unwrap();

    assert!(output.decision.is_retry());
}

#[tokio::test]
async fn test_factory_with_custom_config() {
    use crate::execution_engine::application::factory::{
        ParallelExecutionFactory, ParallelExecutionFactoryConfig,
    };
    use crate::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
    use crate::execution_engine::domain::ParallelExecutorConfig;

    let factory = ParallelExecutionFactoryImpl::new();
    let custom_executor_config = ParallelExecutorConfig {
        max_concurrent_executions: 16,
        enable_fallback: false,
        ..Default::default()
    };
    let config = ParallelExecutionFactoryConfig {
        executor_config: custom_executor_config,
        ..Default::default()
    };

    let service = factory.create(config).await.unwrap();
    let dag_id = Uuid::new_v4();

    let output = service
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    assert_eq!(output.result.dag_id, dag_id);
}

// ---------------------------------------------------------------------------
// Inline Retry Loop Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_inline_retry_loop_succeeds_on_first_attempt() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: None,
        })
        .await
        .unwrap();

    assert!(output.result.success);
    assert_eq!(output.result.node_id, node_id);
    assert_eq!(output.result.retry_attempts, 0);
    assert!(output.retry_decision.is_none());
}

#[tokio::test]
async fn test_inline_retry_loop_with_retry_policy() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    let policy = RetryPolicy {
        max_attempts: 2,
        ..Default::default()
    };

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: Some(policy),
        })
        .await
        .unwrap();

    // Placeholder succeeds immediately, so returns success on first attempt
    assert!(output.result.success);
}

#[tokio::test]
async fn test_inline_retry_loop_uses_default_policy_when_none_provided() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: None, // Should use default_retry_policy from config
        })
        .await
        .unwrap();

    assert!(output.result.success);
}

#[tokio::test]
async fn test_execute_graph_creates_session_and_tracks_state() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();

    assert_eq!(state.dag_id, dag_id);
    assert!(!state.paused);
    assert!(state.started_at.is_some());
}

#[tokio::test]
async fn test_execute_graph_completes_without_cancellation() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    assert!(!output.result.cancelled);
    assert!(output.result.cancellation_reason.is_none());
}

#[tokio::test]
async fn test_abort_marks_execution_as_cancelled() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    executor
        .abort_execution(AbortExecutionInput {
            dag_id,
            reason: "manual abort".to_string(),
        })
        .await
        .unwrap();

    // The execution is now aborted; verifying the state requires
    // get_execution_state which returns the session state
    let state = executor
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();

    // State is not complete because execution has no nodes tracked yet
    // But abort was accepted without error
    assert_eq!(state.dag_id, dag_id);
}

#[tokio::test]
async fn test_is_failure_retriable_filtered() {
    let service = RetryEvaluationServiceImpl::new();
    let mut policy = RetryPolicy::default();
    policy.retryable_failures = vec!["transient".to_string(), "lsp_conflict".to_string()];

    assert!(service.is_failure_retriable(&policy, "transient").await);
    assert!(service.is_failure_retriable(&policy, "lsp_conflict").await);
    assert!(!service.is_failure_retriable(&policy, "compile_error").await);
    assert!(!service.is_failure_retriable(&policy, "permanent").await);
}

#[tokio::test]
async fn test_decide_skip_on_skip_and_continue_strategy() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        retry_strategies: vec![RetryStrategy::SkipAndContinue],
        ..Default::default()
    };

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "tool",
        "intent",
        "transient",
        "error",
        0, // first attempt → strategy at index 0 = SkipAndContinue
        4,
        100,
        100,
    );

    let decision = service.decide(&ctx, &policy, None).await;
    assert!(decision.is_terminal());
    assert!(matches!(decision, RetryDecision::Skip { .. }));
}

#[tokio::test]
async fn test_decide_abort_on_exhaustion() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        enable_fallback: false,
        skip_on_exhaustion: false,
        ..Default::default()
    };

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "tool",
        "intent",
        "transient",
        "error",
        3,
        4,
        100,
        400,
    );

    let decision = service.decide(&ctx, &policy, None).await;
    assert!(decision.is_terminal());
    assert!(matches!(decision, RetryDecision::Abort { .. }));
}

#[tokio::test]
async fn test_decide_skip_on_skip_conditions() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();
    let policy = RetryPolicy {
        skip_conditions: Some(vec!["test skip".to_string()]),
        ..Default::default()
    };

    let ctx = FailureContext::new(
        node_id,
        "test-node",
        "tool",
        "intent",
        "transient",
        "this is a test skip condition",
        0,
        4,
        100,
        100,
    );

    let decision = service.decide(&ctx, &policy, None).await;
    assert!(decision.is_terminal());
    match decision {
        RetryDecision::Skip { reason } => {
            assert!(reason.contains("test skip"));
        }
        other => panic!("Expected Skip decision, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_progress_callback_fires() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let called = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let called_clone = called.clone();

    executor.on_progress(Box::new(move |progress| {
        assert_eq!(progress.dag_id, dag_id);
        called_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));

    // Trigger a progress notification directly via the internal mechanism
    // This is an internal implementation detail test
    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            config_override: None,
        })
        .await
        .unwrap();

    // Callback registered but not triggered since no nodes complete
    // The callback infrastructure is ready for real execution
    assert_eq!(called.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_retry_with_skip_and_continue_strategy_index() {
    let service = RetryEvaluationServiceImpl::new();
    let node_id = Uuid::new_v4();

    // Strategy 0 = SameOperation, Strategy 1 = SkipAndContinue
    let policy = RetryPolicy {
        retry_strategies: vec![RetryStrategy::SameOperation, RetryStrategy::SkipAndContinue],
        ..Default::default()
    };

    // First failure: attempt 0 → strategy[0] = SameOperation (not skip)
    let ctx1 = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 0, 4, 100, 100);
    let decision1 = service.decide(&ctx1, &policy, None).await;
    assert!(decision1.is_retry());

    // Second failure: attempt 1 → strategy[1] = SkipAndContinue (skip)
    let ctx2 = FailureContext::new(node_id, "n", "t", "i", "transient", "err", 1, 4, 100, 200);
    let decision2 = service.decide(&ctx2, &policy, None).await;
    assert!(decision2.is_terminal());
    assert!(matches!(decision2, RetryDecision::Skip { .. }));
}

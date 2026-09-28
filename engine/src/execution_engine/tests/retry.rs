//! execution_engine tests (split by concern, #917).

use super::*;

#[tokio::test]
async fn test_execute_node_returns_result() {
    let executor = create_executor();
    let node_id = Uuid::new_v4();
    let dag_id = Uuid::new_v4();

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: Some(fast_non_retriable_policy()),
        })
        .await
        .unwrap();

    // A-02: a node that does not exist in any session graph must FAIL, not
    // return the old placeholder success.
    assert_eq!(output.result.node_id, node_id);
    assert!(!output.result.success);
    let err = output.result.error.as_deref().unwrap_or("");
    assert!(
        err.contains("node_not_found") || err.contains("not found"),
        "error should mention node_not_found, got: {err}"
    );
}

#[tokio::test]
async fn test_execute_node_with_retry_policy() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: Some(fast_non_retriable_policy()),
        })
        .await
        .unwrap();

    // A-02: missing node fails regardless of the configured retry policy.
    assert_eq!(output.result.node_id, node_id);
    assert!(!output.result.success);
    assert!(
        output
            .result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("node_not_found"),
        "missing node must fail, not succeed"
    );
}

/// GAP-A-11: max_total_retries_per_session stops the retry loop once the
/// session-wide budget is consumed.
#[tokio::test]
async fn test_max_total_retries_per_session_stops_retrying() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    let retry =
        crate::execution_engine::application::service_impl::RetryEvaluationServiceImpl::new();
    let event_bus = Arc::new(EventBusServiceImpl::default());
    let executor = ParallelExecutionServiceImpl::new(
        ParallelExecutorConfig {
            max_total_retries_per_session: 1,
            ..Default::default()
        },
        Box::new(retry),
        event_bus,
    );
    let dag_id = Uuid::new_v4();
    let node = TaskNode::new(Uuid::new_v4(), "fragile", "no_such_tool", vec![], "boom")
        .with_requires_approval(true);
    let mut graph = TaskGraph::new();
    graph.add_unchecked(node.clone()).unwrap();
    graph.seal().unwrap();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();
    assert!(output.approval_pending);

    let node_out = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id: node.id,
            retry_policy: None, // default: all failures retriable, 4 attempts
        })
        .await
        .unwrap();

    assert!(!node_out.result.success);
    assert_eq!(
        node_out.result.failure_type.as_deref(),
        Some("retry_budget_exhausted"),
        "retry loop must stop at the session budget, got {:?}",
        node_out.result.failure_type
    );
}

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
    let policy = RetryPolicy {
        retryable_failures: vec!["transient".to_string()],
        ..Default::default()
    };

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
    let policy = RetryPolicy {
        retryable_failures: vec!["transient".to_string()],
        ..Default::default()
    };

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
async fn test_retry_decision_driven_by_classification_transient() {
    // GAP-A-19: a confident Transient classification (network/timeout)
    // drives a Retry decision.
    let service = RetryEvaluationServiceImpl::with_classifier(std::sync::Arc::new(
        FailureClassifierServiceImpl,
    ));
    let policy = RetryPolicy {
        // Policy says this failure_code is NOT retriable; the structured
        // classification (Transient) overrides and grants the retry.
        retryable_failures: vec!["compile_error".to_string()],
        max_attempts: 3,
        ..Default::default()
    };
    let ctx = FailureContext::new(
        uuid::Uuid::new_v4(),
        "test-node",
        "run-command",
        "run",
        "command_failed",
        "connection to host timed out after 30s",
        0,
        3,
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

    assert!(
        output.decision.is_retry(),
        "Transient must retry: {:?}",
        output.decision
    );
}

#[tokio::test]
async fn test_retry_decision_driven_by_classification_build_failure_skips() {
    // GAP-A-19: a confident BuildFailure classification is NOT retryable
    // even though the policy would allow the code.
    let service = RetryEvaluationServiceImpl::with_classifier(std::sync::Arc::new(
        FailureClassifierServiceImpl,
    ));
    let policy = RetryPolicy::default(); // all codes retriable
    let ctx = FailureContext::new(
        uuid::Uuid::new_v4(),
        "test-node",
        "cargo build",
        "build",
        "command_failed",
        "error: build failed: cannot compile",
        0,
        3,
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

    assert!(
        matches!(output.decision, RetryDecision::Skip { .. }),
        "BuildFailure must skip: {:?}",
        output.decision
    );
}

#[tokio::test]
async fn test_retry_decision_unclassified_defers_to_policy() {
    // GAP-A-19: an unmatched message (documented NonRetryable default) is
    // treated as unclassified and defers to the policy (all retriable here).
    let service = RetryEvaluationServiceImpl::with_classifier(std::sync::Arc::new(
        FailureClassifierServiceImpl,
    ));
    let policy = RetryPolicy::default();
    let ctx = FailureContext::new(
        uuid::Uuid::new_v4(),
        "test-node",
        "tool",
        "op",
        "custom_failure",
        "some completely unusual error text",
        0,
        3,
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

    assert!(
        output.decision.is_retry(),
        "unclassified must defer to policy: {:?}",
        output.decision
    );
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
async fn test_inline_retry_loop_succeeds_on_first_attempt() {
    let (executor, dag_id, node_id) = paused_session_with_node().await;

    let output = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id,
            retry_policy: None,
        })
        .await
        .unwrap();

    assert!(
        output.result.success,
        "real tool must succeed on first attempt"
    );
    assert_eq!(output.result.node_id, node_id);
    assert_eq!(output.result.retry_attempts, 0);
    assert!(output.retry_decision.is_none());
}

#[tokio::test]
async fn test_inline_retry_loop_with_retry_policy() {
    let (executor, dag_id, node_id) = paused_session_with_node().await;

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

    // A real tool succeeds on the first attempt — no retries consumed.
    assert!(output.result.success);
    assert_eq!(output.result.retry_attempts, 0);
}

#[tokio::test]
async fn test_inline_retry_loop_uses_default_policy_when_none_provided() {
    let (executor, dag_id, node_id) = paused_session_with_node().await;

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
async fn test_is_failure_retriable_filtered() {
    let service = RetryEvaluationServiceImpl::new();
    let policy = RetryPolicy {
        retryable_failures: vec!["transient".to_string(), "lsp_conflict".to_string()],
        ..Default::default()
    };

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

// ---------------------------------------------------------------------------
// R3 sequence-policy run-time prefix gate (AC#9)
// ---------------------------------------------------------------------------
//
// Dynamic-plan semantics: step A completes, then step B is proposed (would
// complete the forbidden pair). The dispatch loop's prefix gate evaluates the
// session's completed prefix + B BEFORE B dispatches — promote routes B into
// the existing approval pause; deny fails B pre-dispatch (tool never called).
// Real in-process tools (file_append / file_write against the temp dir) prove
// the "executes / never called" legs with observable side effects.

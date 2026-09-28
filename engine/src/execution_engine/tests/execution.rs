//! execution_engine tests (split by concern, #917).

use super::*;

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
async fn test_factory_threads_permission_enforcer_read_only_gates_bash_write() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    use crate::execution_engine::application::factory::{
        ParallelExecutionFactory, ParallelExecutionFactoryConfig,
    };
    use crate::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
    use crate::permission::application::enforcer_factory_impl::PermissionEnforcerFactoryImpl;
    use crate::permission::application::factory::PermissionEnforcerFactory;
    use crate::permission::domain::mode::PermissionMode;

    // Build a ReadOnly enforcer and thread it through the factory config —
    // this is exactly what the CLI/action/MCP entry points now do.
    let enforcer = PermissionEnforcerFactoryImpl
        .create_with_mode(PermissionMode::ReadOnly)
        .await
        .expect("read-only enforcer construction");

    let service = ParallelExecutionFactoryImpl::new()
        .create(ParallelExecutionFactoryConfig {
            permission_enforcer: Some(Arc::from(enforcer)),
            ..Default::default()
        })
        .await
        .expect("factory create");

    let dag_id = Uuid::new_v4();
    // `run_command` requires WorkspaceWrite → denied in ReadOnly before exec.
    let write_node = TaskNode::new(
        Uuid::new_v4(),
        "write",
        "run_command",
        vec![],
        "touch /tmp/rigorix-perm",
    );
    // `grep_search` is allow-listed at ReadOnly → real file_read completes.
    let read_node = TaskNode::new(
        Uuid::new_v4(),
        "read",
        "file_read",
        vec![],
        r#"{"path": "Cargo.toml"}"#,
    );
    let mut graph = TaskGraph::new();
    graph.add_unchecked(write_node).unwrap();
    graph.add_unchecked(read_node).unwrap();
    graph.seal().unwrap();

    let output = service
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();
    assert_eq!(output.result.dag_id, dag_id);

    let state = service
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert_eq!(
        state.failed_count, 1,
        "bash write command must be denied in read_only mode"
    );
    assert_eq!(
        state.completed_count, 1,
        "allow-listed read tool must still complete"
    );
}

/// AC#13 (permission R5): a `workspace_write` agent file-write to
/// `.rigorix/**` is denied by the DEFAULT permission config — the operator's
/// sequence rules are never writable by the agent they judge.
#[tokio::test]
async fn test_default_permission_denies_rigorix_config_write() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    use crate::execution_engine::application::factory::{
        ParallelExecutionFactory, ParallelExecutionFactoryConfig,
    };
    use crate::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
    use crate::permission::application::enforcer_factory_impl::PermissionEnforcerFactoryImpl;
    use crate::permission::application::factory::PermissionEnforcerFactory;

    // Default posture: PermissionConfig::default() (workspace_write mode).
    let enforcer = PermissionEnforcerFactoryImpl
        .create_default()
        .await
        .expect("default enforcer construction");

    let service = ParallelExecutionFactoryImpl::new()
        .create(ParallelExecutionFactoryConfig {
            permission_enforcer: Some(Arc::from(enforcer)),
            ..Default::default()
        })
        .await
        .expect("factory create");

    let dag_id = Uuid::new_v4();
    // Agent tries to overwrite the operator's sequence-policy rules.
    let rule_write = TaskNode::new(
        Uuid::new_v4(),
        "write_rule",
        "file_write",
        vec![],
        r#"{"path": ".rigorix/sequence-policy.toml", "content": "evil"}"#,
    );
    // Control: an ordinary workspace write under the same mode stays allowed.
    let scratch = std::env::current_dir()
        .expect("cwd")
        .join(format!("rigorix-r5-control-{}.txt", Uuid::new_v4()))
        .display()
        .to_string();
    let scratch_write = TaskNode::new(
        Uuid::new_v4(),
        "write_scratch",
        "file_write",
        vec![],
        format!(r#"{{"path": "{}", "content": "ok"}}"#, scratch),
    );
    let mut graph = TaskGraph::new();
    graph.add_unchecked(rule_write).unwrap();
    graph.add_unchecked(scratch_write).unwrap();
    graph.seal().unwrap();

    let output = service
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .unwrap();

    let rule_node_id = output
        .result
        .execution_states
        .iter()
        .find(|(_, s)| s.node_name == "write_rule")
        .map(|(id, _)| *id)
        .expect("write_rule node");
    let rule_result = output
        .result
        .node_results
        .get(&rule_node_id)
        .expect("rule write result");
    assert!(!rule_result.success, "rule file write must be denied");
    assert_eq!(
        rule_result.failure_type.as_deref(),
        Some("permission_denied"),
        "denial surfaces as the structured permission_denied failure"
    );
    assert!(
        !std::path::Path::new(".rigorix/sequence-policy.toml").exists()
            || std::fs::read_to_string(".rigorix/sequence-policy.toml")
                .map(|c| !c.contains("evil"))
                .unwrap_or(true),
        "the operator rule file must be untouched"
    );

    // Control leg: same-mode ordinary write completed.
    let state = service
        .get_execution_state(GetExecutionStateInput { dag_id })
        .await
        .unwrap();
    assert_eq!(state.completed_count, 1, "scratch write completes");
    let _ = std::fs::remove_file(&scratch);
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
            graph: Some(sample_graph()),
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
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();

    // GAP-A-02 change: the graph now really executes (sample_graph has one
    // shell node), so the progress callback fires for the completed node.
    // This also guards the notify_progress re-lock deadlock (regression):
    // a registered callback used to self-deadlock on the sessions Mutex.
    let fired = called.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        fired >= 1,
        "progress callback should fire for completed node, got {fired}"
    );
}

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

    // Held for the test's duration: the handle is what keeps the callback registered (#1573).
    let _subscription = executor.on_progress(Box::new(move |_progress| {
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

    let _subscription = executor.on_progress(Box::new(move |progress| {
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

/// #1573: dropping the subscription removes the callback.
///
/// The registry used to be a `Vec` that nothing removed from, so every caller that registered one —
/// the TUI once, and since #1570 every receipt-driven run — left it behind forever. The assertion is
/// about what the callback **stops** receiving, because "the list is smaller" is not observable from
/// outside and would pass for a registry that clears itself at the wrong moment.
#[tokio::test]
async fn test_progress_subscription_removes_the_callback_when_dropped() {
    let executor = create_executor();
    let fired = Arc::new(std::sync::atomic::AtomicU32::new(0));

    let subscription = {
        let fired = fired.clone();
        executor.on_progress(Box::new(move |_progress| {
            fired.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }))
    };

    // Alive: a graph runs and the callback sees it.
    executor
        .execute_graph(ExecuteGraphInput {
            dag_id: Uuid::new_v4(),
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();
    let after_first = fired.load(std::sync::atomic::Ordering::SeqCst);
    assert!(after_first >= 1, "a registered callback fires");

    drop(subscription);

    // Dropped: the same graph runs and the callback does not.
    executor
        .execute_graph(ExecuteGraphInput {
            dag_id: Uuid::new_v4(),
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();
    assert_eq!(
        fired.load(std::sync::atomic::Ordering::SeqCst),
        after_first,
        "a dropped subscription must stop receiving progress"
    );
}

/// #1573: a callback that removes itself mid-notification does not deadlock and does not stop the rest.
///
/// This is the case that decides the registry's shape: a `Vec` with removal under the iteration lock
/// either deadlocks or shifts every later index. The registry clones the arcs out and drops the lock
/// before calling, so a callback that unsubscribes touches the map while nobody is iterating it.
#[tokio::test]
async fn test_a_callback_may_unsubscribe_while_progress_is_being_notified() {
    let executor = Arc::new(create_executor());
    let fired_self = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let fired_other = Arc::new(std::sync::atomic::AtomicU32::new(0));

    // The self-removing subscription, held where the callback can reach it.
    let held: Arc<
        std::sync::Mutex<
            Option<crate::execution_engine::application::service::ProgressSubscription>,
        >,
    > = Arc::new(std::sync::Mutex::new(None));

    let self_subscription = {
        let fired_self = fired_self.clone();
        let held_by_callback = held.clone();
        executor.on_progress(Box::new(move |_progress| {
            fired_self.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if let Some(subscription) = held_by_callback.lock().expect("not poisoned").take() {
                drop(subscription);
            }
        }))
    };
    *held.lock().expect("not poisoned") = Some(self_subscription);

    // Held for the life of the test: dropping this handle would remove the callback before the
    // notification this case is about.
    let _other_subscription = {
        let fired_other = fired_other.clone();
        executor.on_progress(Box::new(move |_progress| {
            fired_other.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }))
    };

    executor
        .execute_graph(ExecuteGraphInput {
            dag_id: Uuid::new_v4(),
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .expect("a self-removing callback must not deadlock the notification");

    assert!(
        fired_other.load(std::sync::atomic::Ordering::SeqCst) >= 1,
        "the other callback still fires"
    );
}

/// #1573: the registry is what the live subscriptions are, not what was ever registered.
///
/// The acceptance criterion the issue asked for, in its own words: after N runs, the count is what it
/// was before them. Counting notifications cannot answer it — silence is also what a callback that
/// never fired looks like — so the count is asserted directly.
#[tokio::test]
async fn test_progress_registry_holds_only_live_subscriptions() {
    let executor = create_executor();
    assert_eq!(executor.registered_progress_callbacks(), 0);

    let mut held = Vec::new();
    for _ in 0..8 {
        held.push(executor.on_progress(Box::new(|_| {})));
    }
    assert_eq!(executor.registered_progress_callbacks(), 8);

    while let Some(subscription) = held.pop() {
        drop(subscription);
    }
    assert_eq!(
        executor.registered_progress_callbacks(),
        0,
        "eight subscriptions and eight drops leave the registry as it was"
    );
}

//! dag_engine tests (split by concern, #917).

use super::*;

#[test]
fn test_add_unchecked_increases_count() {
    let mut graph = TaskGraph::new();
    let node = make_node(Uuid::new_v4(), "compile", vec![]);
    assert!(graph.add_unchecked(node).is_ok());
    assert_eq!(graph.node_count(), 1);
    assert!(!graph.is_empty());
}

#[test]
fn test_add_unchecked_rejects_duplicate_id() {
    let mut graph = TaskGraph::new();
    let id = Uuid::new_v4();
    let node1 = make_node(id, "first", vec![]);
    let node2 = make_node(id, "second", vec![]);
    assert!(graph.add_unchecked(node1).is_ok());
    let err = graph.add_unchecked(node2).unwrap_err();
    assert!(matches!(err, DagError::DuplicateTaskId { .. }));
}

#[test]
fn test_add_unchecked_rejects_sealed_graph() {
    let mut graph = TaskGraph::new();
    graph
        .add_unchecked(make_node(Uuid::new_v4(), "first", vec![]))
        .unwrap();
    graph.seal().unwrap();
    let node = make_node(Uuid::new_v4(), "late", vec![]);
    let err = graph.add_unchecked(node).unwrap_err();
    assert!(matches!(err, DagError::InvalidGraph { .. }));
}

// ---------------------------------------------------------------------------
// Graph Sealing
// ---------------------------------------------------------------------------

#[test]
fn test_seal_empty_graph_fails() {
    let mut graph = TaskGraph::new();
    let err = graph.seal().unwrap_err();
    assert!(matches!(err, DagError::InvalidGraph { .. }));
    let msg = format!("{}", err);
    assert!(
        msg.contains("empty"),
        "Expected error about empty graph: {}",
        msg
    );
}

#[test]
fn test_seal_rejects_missing_dependency() {
    let mut graph = TaskGraph::new();
    let missing_id = Uuid::new_v4();
    let node = make_node(Uuid::new_v4(), "orphan", vec![missing_id]);
    graph.add_unchecked(node).unwrap();
    let err = graph.seal().unwrap_err();
    assert!(matches!(err, DagError::DependencyNotFound { .. }));
    let msg = format!("{}", err);
    assert!(msg.contains("not found"), "Expected 'not found': {}", msg);
}

// ---------------------------------------------------------------------------
// Cycle Detection
// ---------------------------------------------------------------------------

#[test]
fn test_task_node_new() {
    let id = Uuid::new_v4();
    let node = TaskNode::new(id, "compile", "cargo build", vec![], "Build the project");
    assert_eq!(node.id, id);
    assert_eq!(node.name, "compile");
    assert_eq!(node.tool, "cargo build");
    assert!(node.dependencies.is_empty());
    assert_eq!(node.intent, "Build the project");
    assert_eq!(node.policy, ExecutionPolicy::default());
    assert!(node.validation_rule.is_none());
}

#[test]
fn test_task_node_with_policy() {
    let id = Uuid::new_v4();
    let policy = ExecutionPolicy {
        max_retries: 5,
        backoff_ms: 500,
        ..ExecutionPolicy::default()
    };
    let node = TaskNode::with_policy(
        id,
        "deploy",
        "kubectl apply",
        vec![],
        "Deploy to prod",
        policy.clone(),
    );
    assert_eq!(node.policy.max_retries, 5);
    assert_eq!(node.policy.backoff_ms, 500);
}

#[test]
fn test_task_node_with_validation() {
    let id = Uuid::new_v4();
    let node = TaskNode::with_policy_and_validation(
        id,
        "test",
        "cargo test",
        vec![],
        "Run tests",
        ExecutionPolicy::default(),
        ValidationRule::TestPass,
    );
    assert_eq!(node.validation_rule, Some(ValidationRule::TestPass));
}

// ---------------------------------------------------------------------------
// ExecutionPolicy
// ---------------------------------------------------------------------------

#[test]
fn test_execution_policy_defaults() {
    let policy = ExecutionPolicy::default();
    assert_eq!(policy.max_retries, 3);
    assert_eq!(policy.retry_strategy, RetryStrategy::SameOperation);
    assert_eq!(
        policy.retry_on,
        vec![FailureType::Transient, FailureType::LspConflict]
    );
    assert!(policy.fallback_node.is_none());
    assert!(policy.validation_rule.is_none());
    assert_eq!(policy.backoff_ms, 100);
    assert_eq!(policy.backoff_multiplier, 2.0);
    assert_eq!(policy.max_backoff_ms, 30_000);
}

// ---------------------------------------------------------------------------
// FailureType
// ---------------------------------------------------------------------------

#[test]
fn test_failure_type_as_str() {
    assert_eq!(FailureType::Transient.as_str(), "transient");
    assert_eq!(FailureType::LspConflict.as_str(), "lsp_conflict");
    assert_eq!(FailureType::BuildFailure.as_str(), "build_failure");
    assert_eq!(FailureType::TestFailure.as_str(), "test_failure");
    assert_eq!(
        FailureType::MissingDependency.as_str(),
        "missing_dependency"
    );
    assert_eq!(FailureType::PlanConflict.as_str(), "plan_conflict");
    assert_eq!(FailureType::NonRetryable.as_str(), "non_retryable");
    assert_eq!(FailureType::Unknown.as_str(), "unknown");
}

// ---------------------------------------------------------------------------
// ValidationRule
// ---------------------------------------------------------------------------

#[test]
fn test_validation_rule_as_str() {
    assert_eq!(ValidationRule::TypeCheck.as_str(), "type_check");
    assert_eq!(ValidationRule::TestPass.as_str(), "test_pass");
    assert_eq!(ValidationRule::LintPass.as_str(), "lint_pass");
    assert_eq!(ValidationRule::Custom("my-check".into()).as_str(), "custom");
}

// ---------------------------------------------------------------------------
// RetryStrategy
// ---------------------------------------------------------------------------

#[test]
fn test_retry_strategy_as_str() {
    assert_eq!(RetryStrategy::SameOperation.as_str(), "same_operation");
    assert_eq!(
        RetryStrategy::ExpandContext { level: 1 }.as_str(),
        "expand_context"
    );
    assert_eq!(RetryStrategy::SkipAndContinue.as_str(), "skip_and_continue");
}

// ---------------------------------------------------------------------------
// ImpactLevel
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_service_construct_graph() {
    let service = DagGraphServiceImpl::new();
    let node = make_node(Uuid::new_v4(), "compile", vec![]);
    let output = service
        .construct_graph(ConstructGraphInput { nodes: vec![node] })
        .await
        .unwrap();

    assert_eq!(output.node_count, 1);
    assert!(!output.graph.sealed);
}

#[tokio::test]
async fn test_service_construct_and_seal() {
    let service = DagGraphServiceImpl::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();

    let construct = service
        .construct_graph(ConstructGraphInput {
            nodes: vec![
                make_node(a, "compile", vec![]),
                make_node(b, "test", vec![a]),
            ],
        })
        .await
        .unwrap();

    let dag_id = construct.dag_id;
    assert_eq!(construct.node_count, 2);

    // Seal the graph
    let sealed = service.seal_graph(SealGraphInput { dag_id }).await.unwrap();
    assert_eq!(sealed.total_nodes, 2);
    assert_eq!(sealed.processed_count, 2);
    assert_eq!(sealed.topological_order.len(), 2);

    // Verify topological order: a before b
    let order = sealed.topological_order;
    let pos_a = order.iter().position(|id| *id == a).unwrap();
    let pos_b = order.iter().position(|id| *id == b).unwrap();
    assert!(pos_a < pos_b, "a must come before b");

    // Verify is_sealed
    let is_sealed = service.is_sealed(dag_id).await.unwrap();
    assert!(is_sealed);

    // Verify ready nodes
    let ready = service.get_ready_nodes(dag_id).await.unwrap();
    assert!(ready.contains(&a), "a should be ready (no deps)");
    assert!(!ready.contains(&b), "b should not be ready yet");

    // Mark a as completed
    service.mark_node_completed(dag_id, a).await.unwrap();

    // Now b should be ready
    let ready = service.get_ready_nodes(dag_id).await.unwrap();
    assert!(ready.contains(&b), "b should be ready after a completes");
}

// ---------------------------------------------------------------------------
// DagPlanningService Integration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_planning_service_compare_identical() {
    let service = DagPlanningServiceImpl::new();
    let id = Uuid::new_v4();
    let node = make_node(id, "build", vec![]);
    let output = service
        .compare_plans(ComparePlansInput {
            old_nodes: vec![node.clone()],
            new_nodes: vec![node],
        })
        .await
        .unwrap();

    assert_eq!(output.diff.added.len(), 0);
    assert_eq!(output.diff.removed.len(), 0);
    assert_eq!(output.diff.impact_level, ImpactLevel::None);
}

#[tokio::test]
async fn test_service_add_node_after_construction() {
    let service = DagGraphServiceImpl::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();

    let construct = service
        .construct_graph(ConstructGraphInput {
            nodes: vec![make_node(a, "first", vec![])],
        })
        .await
        .unwrap();
    let dag_id = construct.dag_id;
    assert_eq!(construct.node_count, 1);

    // Add another node
    let add = service
        .add_node(AddNodeInput {
            dag_id,
            node: make_node(b, "second", vec![a]),
        })
        .await
        .unwrap();
    assert_eq!(add.node_count, 2);
    assert_eq!(add.node_id, b);
}

#[tokio::test]
async fn test_service_get_graph_and_node() {
    let service = DagGraphServiceImpl::new();
    let a = Uuid::new_v4();

    let construct = service
        .construct_graph(ConstructGraphInput {
            nodes: vec![make_node(a, "hello", vec![])],
        })
        .await
        .unwrap();
    let dag_id = construct.dag_id;

    // Get graph
    let get = service.get_graph(GetGraphInput { dag_id }).await.unwrap();
    assert_eq!(get.dag_id, dag_id);
    assert_eq!(get.graph.node_count(), 1);

    // Get node
    let node = service
        .get_node(GetNodeInput { dag_id, node_id: a })
        .await
        .unwrap();
    assert_eq!(node.node.name, "hello");
}

#[tokio::test]
async fn test_service_list_nodes() {
    let service = DagGraphServiceImpl::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();

    let construct = service
        .construct_graph(ConstructGraphInput {
            nodes: vec![
                make_node(a, "first", vec![]),
                make_node(b, "second", vec![]),
            ],
        })
        .await
        .unwrap();
    let dag_id = construct.dag_id;

    let list = service.list_nodes(ListNodesInput { dag_id }).await.unwrap();
    assert_eq!(list.total_count, 2);
    assert_eq!(list.nodes.len(), 2);
}

#[tokio::test]
async fn test_service_operations_on_nonexistent_graph() {
    let service = DagGraphServiceImpl::new();
    let phantom = Uuid::new_v4();

    // All operations should fail with InvalidGraph
    let err = service
        .seal_graph(SealGraphInput { dag_id: phantom })
        .await
        .unwrap_err();
    assert!(matches!(err, DagError::InvalidGraph { .. }));

    let err = service
        .get_graph(GetGraphInput { dag_id: phantom })
        .await
        .unwrap_err();
    assert!(matches!(err, DagError::InvalidGraph { .. }));

    let err = service.get_ready_nodes(phantom).await.unwrap_err();
    assert!(matches!(err, DagError::InvalidGraph { .. }));

    let err = service
        .mark_node_completed(phantom, Uuid::new_v4())
        .await
        .unwrap_err();
    assert!(matches!(err, DagError::InvalidGraph { .. }));
}

#[test]
fn test_taskgraph_serde_roundtrip() {
    let mut graph = TaskGraph::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    graph.add_unchecked(make_node(a, "a", vec![])).unwrap();
    graph.add_unchecked(make_node(b, "b", vec![a])).unwrap();
    graph.seal().unwrap();

    let json = serde_json::to_string(&graph).unwrap();
    let deserialized: TaskGraph = serde_json::from_str(&json).unwrap();

    assert!(deserialized.sealed);
    assert_eq!(deserialized.node_count(), 2);
    // ExecutionState is skipped during serialization, so it will be default
    assert!(deserialized.execution_state.in_degree.is_empty());
}

// ---------------------------------------------------------------------------
// ExecutionPolicyService — Should Retry
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_should_retry_retriable_failure() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy::default();

    let decision = service
        .should_retry(ShouldRetryInput {
            policy,
            failure_type: FailureType::Transient,
            retries_attempted: 0,
        })
        .await
        .unwrap();

    match decision {
        RetryDecision::Retry {
            strategy, attempt, ..
        } => {
            assert_eq!(strategy, RetryStrategy::SameOperation);
            assert_eq!(attempt, 1);
        }
        _ => panic!("Expected Retry, got NoRetry"),
    }
}

#[tokio::test]
async fn test_should_retry_non_retriable_failure() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy::default();

    let decision = service
        .should_retry(ShouldRetryInput {
            policy,
            failure_type: FailureType::NonRetryable,
            retries_attempted: 0,
        })
        .await
        .unwrap();

    match decision {
        RetryDecision::NoRetry { use_fallback, .. } => {
            assert!(!use_fallback, "No fallback configured");
        }
        _ => panic!("Expected NoRetry, got Retry"),
    }
}

#[tokio::test]
async fn test_should_retry_exhausted_retries() {
    let service = ExecutionPolicyServiceImpl::new();
    // max_retries = 3 by default
    let policy = ExecutionPolicy::default();

    // After 3 attempts, no more retries
    let decision = service
        .should_retry(ShouldRetryInput {
            policy: policy.clone(),
            failure_type: FailureType::Transient,
            retries_attempted: 3,
        })
        .await
        .unwrap();

    match decision {
        RetryDecision::NoRetry { reason, .. } => {
            assert!(reason.contains("exhausted"));
        }
        _ => panic!("Expected NoRetry after exhausting retries"),
    }
}

#[tokio::test]
async fn test_should_retry_with_custom_policy() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy {
        max_retries: 5,
        retry_on: vec![FailureType::Transient, FailureType::MissingDependency],
        ..ExecutionPolicy::default()
    };

    // MissingDependency is retriable
    let decision = service
        .should_retry(ShouldRetryInput {
            policy,
            failure_type: FailureType::MissingDependency,
            retries_attempted: 0,
        })
        .await
        .unwrap();

    match decision {
        RetryDecision::Retry { attempt, .. } => {
            assert_eq!(attempt, 1);
        }
        _ => panic!("Expected Retry"),
    }
}

#[tokio::test]
async fn test_should_retry_max_retries_zero() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy {
        max_retries: 0,
        ..ExecutionPolicy::default()
    };

    let decision = service
        .should_retry(ShouldRetryInput {
            policy,
            failure_type: FailureType::Transient,
            retries_attempted: 0,
        })
        .await
        .unwrap();

    match decision {
        RetryDecision::NoRetry { reason, .. } => {
            assert!(reason.contains("exhausted") || reason.contains("0"));
        }
        _ => panic!("Expected NoRetry with max_retries=0"),
    }
}

// ---------------------------------------------------------------------------
// ExecutionPolicyService — Backoff Computation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_compute_backoff_first_attempt() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy::default();

    let output = service
        .compute_backoff(ComputeBackoffInput { policy, attempt: 1 })
        .await
        .unwrap();

    // First attempt: 100ms * 2.0^0 = 100ms
    assert_eq!(output.delay_ms, 100);
    assert!(output.explanation.contains("100ms"));
}

#[tokio::test]
async fn test_compute_backoff_second_attempt() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy::default();

    let output = service
        .compute_backoff(ComputeBackoffInput { policy, attempt: 2 })
        .await
        .unwrap();

    // Second attempt: 100ms * 2.0^1 = 200ms
    assert_eq!(output.delay_ms, 200);
}

#[tokio::test]
async fn test_compute_backoff_third_attempt() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy::default();

    let output = service
        .compute_backoff(ComputeBackoffInput { policy, attempt: 3 })
        .await
        .unwrap();

    // Third attempt: 100ms * 2.0^2 = 400ms
    assert_eq!(output.delay_ms, 400);
}

#[tokio::test]
async fn test_compute_backoff_capped_at_max() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy {
        backoff_ms: 100,
        backoff_multiplier: 10.0,
        max_backoff_ms: 5000,
        ..ExecutionPolicy::default()
    };

    let output = service
        .compute_backoff(ComputeBackoffInput {
            policy,
            attempt: 5, // 100 * 10^4 = 1,000,000 → capped at 5,000
        })
        .await
        .unwrap();

    assert_eq!(output.delay_ms, 5000);
}

#[tokio::test]
async fn test_compute_backoff_custom_policy() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy {
        backoff_ms: 1000,
        backoff_multiplier: 3.0,
        max_backoff_ms: 60_000,
        ..ExecutionPolicy::default()
    };

    let output = service
        .compute_backoff(ComputeBackoffInput {
            policy,
            attempt: 3, // 1000 * 3.0^2 = 9000ms
        })
        .await
        .unwrap();

    assert_eq!(output.delay_ms, 9000);
}

// ---------------------------------------------------------------------------
// ExecutionPolicyService — Policy Validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_validate_policy_default_valid() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy::default();

    let output = service
        .validate_policy(ValidatePolicyInput { policy })
        .await
        .unwrap();

    assert!(output.is_valid);
    assert!(output.errors.is_empty());
}

#[tokio::test]
async fn test_validate_policy_zero_backoff() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy {
        backoff_ms: 0,
        ..ExecutionPolicy::default()
    };

    let output = service
        .validate_policy(ValidatePolicyInput { policy })
        .await
        .unwrap();

    assert!(!output.is_valid);
    assert!(output.errors.iter().any(|e| e.contains("backoff_ms")));
}

#[tokio::test]
async fn test_validate_policy_multiplier_less_than_one() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy {
        backoff_multiplier: 0.5,
        ..ExecutionPolicy::default()
    };

    let output = service
        .validate_policy(ValidatePolicyInput { policy })
        .await
        .unwrap();

    assert!(!output.is_valid);
    assert!(output.errors.iter().any(|e| e.contains("multiplier")));
}

#[tokio::test]
async fn test_validate_policy_max_less_than_base() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy {
        backoff_ms: 1000,
        max_backoff_ms: 500,
        ..ExecutionPolicy::default()
    };

    let output = service
        .validate_policy(ValidatePolicyInput { policy })
        .await
        .unwrap();

    assert!(!output.is_valid);
    assert!(output.errors.iter().any(|e| e.contains("max_backoff")));
}

#[tokio::test]
async fn test_validate_policy_multiple_errors() {
    let service = ExecutionPolicyServiceImpl::new();
    let policy = ExecutionPolicy {
        backoff_ms: 0,
        backoff_multiplier: 0.5,
        max_backoff_ms: 0,
        ..ExecutionPolicy::default()
    };

    let output = service
        .validate_policy(ValidatePolicyInput { policy })
        .await
        .unwrap();

    assert!(!output.is_valid);
    assert!(output.errors.len() >= 2);
}

#[tokio::test]
async fn test_validate_policy_warnings() {
    let service = ExecutionPolicyServiceImpl::new();

    // max_retries = 0 should warn
    let policy = ExecutionPolicy {
        max_retries: 0,
        ..ExecutionPolicy::default()
    };
    let output = service
        .validate_policy(ValidatePolicyInput { policy })
        .await
        .unwrap();
    assert!(output.warnings.iter().any(|w| w.contains("max_retries")));

    // empty retry_on should warn
    let policy = ExecutionPolicy {
        retry_on: vec![],
        ..ExecutionPolicy::default()
    };
    let output = service
        .validate_policy(ValidatePolicyInput { policy })
        .await
        .unwrap();
    assert!(output.warnings.iter().any(|w| w.contains("retry_on")));
}

// ---------------------------------------------------------------------------
// Edge Cases
// ---------------------------------------------------------------------------

#[test]
fn test_get_node_found() {
    let mut graph = TaskGraph::new();
    let id = Uuid::new_v4();
    graph.add_unchecked(make_node(id, "found", vec![])).unwrap();
    let node = graph.get_node(id);
    assert!(node.is_some());
    assert_eq!(node.unwrap().name, "found");
}

#[test]
fn test_get_node_not_found() {
    let graph = TaskGraph::new();
    let node = graph.get_node(Uuid::new_v4());
    assert!(node.is_none());
}

#[test]
fn test_dag_error_display() {
    let err = DagError::CycleDetected { found: 2, total: 5 };
    let msg = format!("{}", err);
    assert!(msg.contains("Cycle detected"));
    assert!(msg.contains("2"));
    assert!(msg.contains("5"));
}

#[test]
fn test_error_impl() {
    let err = DagError::TaskNotFound { id: Uuid::nil() };
    let err_ref: &dyn std::error::Error = &err;
    assert!(err_ref.source().is_none());
}

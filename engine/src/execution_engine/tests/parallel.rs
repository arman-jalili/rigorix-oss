//! execution_engine tests (split by concern, #917).

use super::*;

#[tokio::test]
async fn test_execute_graph_with_config_override() {
    let executor = create_executor();
    let dag_id = Uuid::new_v4();

    let custom_config = ParallelExecutorConfig {
        max_concurrent_executions: 8,
        ..Default::default()
    };

    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    let mut graph = TaskGraph::new();
    let root = TaskNode::new(Uuid::new_v4(), "root", "shell", vec![], "echo hi");
    graph.add_unchecked(root.clone()).unwrap();
    graph.seal().unwrap();

    let output = executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: Some(custom_config),
        })
        .await
        .unwrap();

    assert_eq!(output.result.dag_id, dag_id);
}

/// A-01 (parallel path): an unknown tool name in a graph must produce a
/// node failure, never a silent success.
#[tokio::test]
async fn test_unknown_tool_fails_graph_execution() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    let node = TaskNode::new(
        Uuid::new_v4(),
        "step",
        "no_such_tool",
        vec![],
        "no such tool",
    );
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

    let nr = output
        .result
        .node_results
        .get(&node.id)
        .expect("node result present");
    assert!(
        !nr.success,
        "unknown tool must fail the node, not report success"
    );
    assert_eq!(
        nr.failure_type.as_deref(),
        Some("unknown_tool"),
        "failure type must be unknown_tool"
    );
    assert!(
        nr.error.as_deref().unwrap_or("").contains("no_such_tool"),
        "error must name the tool"
    );
}

/// A-01 (single-node path): the same unknown-tool failure must surface via
/// `execute_node` (the `execute_tool` dispatch), not just the parallel loop.
#[tokio::test]
async fn test_unknown_tool_fails_single_node_path() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    let executor = create_executor();
    let dag_id = Uuid::new_v4();
    // Approval-gated so execute_graph leaves the node in the session graph
    // without dispatching it; then execute_node drives the single-node path.
    let node = TaskNode::new(
        Uuid::new_v4(),
        "step",
        "no_such_tool",
        vec![],
        "no such tool",
    )
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
    assert!(output.approval_pending, "gate must pause before dispatch");

    let node_out = executor
        .execute_node(ExecuteNodeInput {
            dag_id,
            node_id: node.id,
            retry_policy: Some(fast_non_retriable_policy()),
        })
        .await
        .unwrap();
    assert!(
        !node_out.result.success,
        "single-node path must fail on unknown tool"
    );
    let err = node_out.result.error.as_deref().unwrap_or("");
    assert!(
        err.contains("no_such_tool") || err.contains("unknown_tool"),
        "error must reference the unknown tool, got: {err}"
    );
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
            graph: Some(sample_graph()),
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
            graph: Some(sample_graph()),
            config_override: None,
        })
        .await
        .unwrap();

    assert_eq!(output.result.dag_id, dag_id);
}

//! execution_engine tests (split by concern, #917).

use super::*;

/// A-05: PreToolUse hooks must gate the PARALLEL dispatch loop, not just the
/// single-node `execute_tool` path. A hook returning deny blocks the tool.
#[tokio::test]
async fn test_pre_tool_use_hook_blocks_parallel_dispatch() {
    use crate::dag_engine::domain::{TaskGraph, TaskNode};
    use crate::hooks::application::hook_runner_impl::HookRunnerImpl;
    use crate::hooks::domain::config::HookConfig;

    let retry =
        crate::execution_engine::application::service_impl::RetryEvaluationServiceImpl::new();
    let event_bus = Arc::new(EventBusServiceImpl::default());
    let runner = Arc::new(HookRunnerImpl::new(HookConfig {
        pre_tool_use: vec![r#"echo '{"decision":"Deny","reason":"blocked by test"}'"#.into()],
        ..Default::default()
    }));
    let executor = ParallelExecutionServiceImpl::new(
        ParallelExecutorConfig::default(),
        Box::new(retry),
        event_bus,
    )
    .with_hook_runner(runner);

    let dag_id = Uuid::new_v4();
    let node = TaskNode::new(
        Uuid::new_v4(),
        "step",
        "run_command",
        vec![],
        r#"{"command": "echo hi"}"#,
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
        "PreToolUse deny must block the tool in the parallel path"
    );
    assert_eq!(
        nr.failure_type.as_deref(),
        Some("hook_blocked"),
        "failure type must be hook_blocked"
    );
    assert!(
        nr.error
            .as_deref()
            .unwrap_or("")
            .contains("blocked by test"),
        "error must carry the hook reason"
    );
}

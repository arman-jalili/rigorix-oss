//! Concurrent-safety tests for the Execution Engine.

#[cfg(test)]
mod tests {
    use crate::execution_engine::domain::parallel_executor::NodeExecutionState;
    use uuid::Uuid;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_node_state_lifecycle() {
        let node_id = Uuid::new_v4();
        let mut state = NodeExecutionState::new(node_id, "test-node");
        assert_eq!(state.status, crate::execution_engine::domain::parallel_executor::NodeStatus::Pending);
        assert!(!state.is_terminal());

        state.mark_ready();
        state.mark_running();
        state.mark_completed(100);
        assert!(state.is_terminal());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_node_state_fail_and_retry() {
        let node_id = Uuid::new_v4();
        let mut state = NodeExecutionState::new(node_id, "retry-node");
        assert!(!state.is_terminal());

        state.mark_ready();
        state.mark_running();
        state.mark_failed("Timeout".to_string(), "connection reset".to_string());
        assert!(state.is_terminal());

        state.mark_for_retry();
        // After retry, should be ready again
        assert!(!state.is_terminal());
    }
}

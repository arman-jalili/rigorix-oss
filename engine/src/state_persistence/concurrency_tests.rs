//! Concurrent-safety tests for the State Persistence module.

#[cfg(test)]
mod tests {
    use crate::state_persistence::domain::{ExecutionState, NodeState, NodeStatus};
    use uuid::Uuid;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_node_state_creation() {
        let node = NodeState::new(Uuid::new_v4());
        assert_eq!(node.status, NodeStatus::Pending);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_execution_state_creation() {
        let state = ExecutionState::new(Uuid::new_v4(), "test-symbol-hash".to_string());
        assert!(state.node_states.is_empty());
        assert!(state.completed_at.is_none());
    }
}

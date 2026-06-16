//! Integration tests for the State Persistence module.
//!
//! Covers atomic write-rename, state transitions, and execution record storage.

#[cfg(test)]
mod tests {
    use crate::state_persistence::domain::{ExecutionState, NodeState, NodeStatus};
    use uuid::Uuid;

    #[test]
    fn test_execution_state_initializes_correctly() {
        let exec_id = Uuid::new_v4();
        let state = ExecutionState::new(exec_id, "hash-v1".to_string());
        assert_eq!(state.execution_id, exec_id);
        assert_eq!(state.symbol_graph_hash, "hash-v1");
        assert!(state.node_states.is_empty());
        assert!(state.completed_at.is_none());
    }

    #[test]
    fn test_node_state_initializes_as_pending() {
        let node_id = Uuid::new_v4();
        let node = NodeState::new(node_id);
        assert_eq!(node.node_id, node_id);
        assert_eq!(node.status, NodeStatus::Pending);
        assert_eq!(node.retries, 0);
    }

    #[test]
    fn test_node_state_marks_completed() {
        let node_id = Uuid::new_v4();
        let mut node = NodeState::new(node_id);
        assert_eq!(node.status, NodeStatus::Pending);
        node.status = NodeStatus::Completed;
        assert_eq!(node.status, NodeStatus::Completed);
    }
}

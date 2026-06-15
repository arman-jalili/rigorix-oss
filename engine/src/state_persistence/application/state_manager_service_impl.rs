//! Implementation of the StateManagerService.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#application
//! Implements: ISSUE-STATE-PERSISTENCE-1 — FileSystemStateManager
//! Issue: #79
//!
//! Provides the concrete `FileSystemStateManager` that persists execution
//! state using atomic write-rename, manages node state transitions, and
//! lists available executions.
//!
//! # Thread Safety
//! - Repository backed by atomic file operations (safe from multiple processes)
//! - All async methods are safe to call from multiple tasks
//! - File operations use tokio::fs for non-blocking I/O

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use crate::state_persistence::application::dto::{
    ExecutionSummary, ListExecutionsInput, ListExecutionsOutput, LoadStateInput, LoadStateOutput,
    NodeStateChangedInput, NodeStateChangedOutput, SaveStateInput, SaveStateOutput,
};
use crate::state_persistence::application::service::StateManagerService;
use crate::state_persistence::domain::StateError;
use crate::state_persistence::infrastructure::repository::StateRepository;

/// Concrete implementation of `StateManagerService` backed by a `StateRepository`.
///
/// Manages the full lifecycle of execution state:
/// - Save state at each phase (Pending → Running → Completed/Failed/Cancelled)
/// - Load state for recovery or inspection
/// - Update individual node states with atomic load-modify-save
/// - List available executions
pub struct FileSystemStateManager {
    /// The repository used for state persistence.
    repository: Box<dyn StateRepository>,
}

impl FileSystemStateManager {
    /// Create a new `FileSystemStateManager` with the given repository.
    pub fn new(repository: Box<dyn StateRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl StateManagerService for FileSystemStateManager {
    #[tracing::instrument(skip_all)]
    async fn save_state(&self, input: SaveStateInput) -> Result<SaveStateOutput, StateError> {
        let state = input.state;
        let execution_id = state.execution_id;
        let status = state.status;
        let node_count = state.node_states.len() as u32;

        self.repository.save(&state).await?;

        Ok(SaveStateOutput {
            execution_id,
            status,
            node_count,
            saved_at: Utc::now(),
        })
    }

    #[tracing::instrument(skip_all)]
    async fn load_state(&self, input: LoadStateInput) -> Result<LoadStateOutput, StateError> {
        let state = self.repository.load(input.execution_id).await?;

        Ok(LoadStateOutput {
            state,
            loaded_at: Utc::now(),
        })
    }

    async fn update_node_state(
        &self,
        input: NodeStateChangedInput,
    ) -> Result<NodeStateChangedOutput, StateError> {
        // Load current state
        let mut state = self.repository.load(input.execution_id).await?;

        // Apply the state transition based on new_status
        let node_id = input.node_id;

        match input.new_status {
            crate::state_persistence::domain::NodeStatus::InProgress => {
                state.node_started(node_id)?;
            }
            crate::state_persistence::domain::NodeStatus::Completed => {
                state.node_completed(node_id, input.output, input.duration_ms.unwrap_or(0))?;
            }
            crate::state_persistence::domain::NodeStatus::Failed => {
                state.node_failed(node_id, input.error.unwrap_or_default())?;
            }
            crate::state_persistence::domain::NodeStatus::Skipped => {
                state.node_skipped(node_id, input.error)?;
            }
            crate::state_persistence::domain::NodeStatus::Pending => {
                // Only allow resetting to Pending via increment_retry
                state.increment_retry(node_id)?;
            }
        }

        // Get the updated node state before saving
        let node_state = state
            .node_states
            .get(&node_id)
            .ok_or_else(|| StateError::NodeNotFound {
                node_id: node_id.to_string(),
                execution_id: input.execution_id.to_string(),
            })?
            .clone();

        // Save the updated state
        self.repository.save(&state).await?;

        Ok(NodeStateChangedOutput {
            execution_id: input.execution_id,
            node_id,
            node_state,
            updated_at: Utc::now(),
        })
    }

    async fn list_executions(
        &self,
        input: ListExecutionsInput,
    ) -> Result<ListExecutionsOutput, StateError> {
        let all_ids = self.repository.list_ids().await?;
        let total_count = all_ids.len() as u32;

        let limit = input.limit.unwrap_or(50) as usize;
        let offset = input.offset.unwrap_or(0) as usize;

        let mut executions = Vec::new();

        for id in all_ids.iter().skip(offset).take(limit) {
            match self.repository.load(*id).await {
                Ok(state) => {
                    let summary = ExecutionSummary::from(&state);

                    // Apply status filter if specified
                    if let Some(ref filter) = input.status_filter {
                        if state.status != *filter {
                            continue;
                        }
                    }

                    executions.push(summary);
                }
                Err(_) => {
                    // Skip corrupted states in listings
                    continue;
                }
            }
        }

        Ok(ListExecutionsOutput {
            executions,
            total_count,
            limit: limit as u32,
            offset: offset as u32,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn delete_state(&self, execution_id: Uuid) -> Result<(), StateError> {
        self.repository.delete(execution_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    use crate::state_persistence::domain::{ExecutionState, ExecutionStatus, NodeStatus};
    use crate::state_persistence::infrastructure::FileSystemStateRepository;

    #[tracing::instrument(skip_all)]
    async fn create_manager() -> (FileSystemStateManager, TempDir) {
        let dir = TempDir::new().unwrap();
        let repo = FileSystemStateRepository::new(dir.path().to_path_buf())
            .await
            .unwrap();
        let manager = FileSystemStateManager::new(Box::new(repo));
        (manager, dir)
    }

    #[tracing::instrument(skip_all)]
    fn create_save_input(execution_id: Uuid, _status: ExecutionStatus) -> SaveStateInput {
        let state = ExecutionState::new(execution_id, "hash_123".to_string());
        SaveStateInput { state }
    }

    #[tokio::test]
    async fn test_save_and_load_state() {
        let (manager, _dir) = create_manager().await;
        let execution_id = Uuid::new_v4();
        let save_input = create_save_input(execution_id, ExecutionStatus::Pending);

        let save_output = manager.save_state(save_input).await.unwrap();
        assert_eq!(save_output.execution_id, execution_id);
        assert_eq!(save_output.status, ExecutionStatus::Pending);

        let load_input = LoadStateInput { execution_id };
        let load_output = manager.load_state(load_input).await.unwrap();
        assert_eq!(load_output.state.execution_id, execution_id);
        assert_eq!(load_output.state.status, ExecutionStatus::Pending);
    }

    #[tokio::test]
    async fn test_load_nonexistent_returns_error() {
        let (manager, _dir) = create_manager().await;
        let execution_id = Uuid::new_v4();

        let result = manager.load_state(LoadStateInput { execution_id }).await;
        assert!(matches!(result, Err(StateError::StateNotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_node_state_to_in_progress() {
        let (manager, _dir) = create_manager().await;
        let execution_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();

        // Save initial state with node
        let mut state = ExecutionState::new(execution_id, "hash".to_string());
        state.status = ExecutionStatus::Running;
        state.init_node_states(&[node_id]);
        manager.save_state(SaveStateInput { state }).await.unwrap();

        // Update node to in progress
        let output = manager
            .update_node_state(NodeStateChangedInput {
                execution_id,
                node_id,
                new_status: NodeStatus::InProgress,
                output: None,
                error: None,
                duration_ms: None,
            })
            .await
            .unwrap();

        assert_eq!(output.node_state.status, NodeStatus::InProgress);
    }

    #[tokio::test]
    async fn test_update_node_state_to_completed() {
        let (manager, _dir) = create_manager().await;
        let execution_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();

        let mut state = ExecutionState::new(execution_id, "hash".to_string());
        state.status = ExecutionStatus::Running;
        state.init_node_states(&[node_id]);
        // Transition to in progress first
        state.node_started(node_id).unwrap();
        manager.save_state(SaveStateInput { state }).await.unwrap();

        let output = manager
            .update_node_state(NodeStateChangedInput {
                execution_id,
                node_id,
                new_status: NodeStatus::Completed,
                output: Some("done".to_string()),
                error: None,
                duration_ms: Some(500),
            })
            .await
            .unwrap();

        assert_eq!(output.node_state.status, NodeStatus::Completed);
        assert_eq!(output.node_state.output, Some("done".to_string()));
        assert_eq!(output.node_state.duration_ms, Some(500));
    }

    #[tokio::test]
    async fn test_update_node_state_to_failed() {
        let (manager, _dir) = create_manager().await;
        let execution_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();

        let mut state = ExecutionState::new(execution_id, "hash".to_string());
        state.status = ExecutionStatus::Running;
        state.init_node_states(&[node_id]);
        state.node_started(node_id).unwrap();
        manager.save_state(SaveStateInput { state }).await.unwrap();

        let output = manager
            .update_node_state(NodeStateChangedInput {
                execution_id,
                node_id,
                new_status: NodeStatus::Failed,
                output: None,
                error: Some("error occurred".to_string()),
                duration_ms: Some(300),
            })
            .await
            .unwrap();

        assert_eq!(output.node_state.status, NodeStatus::Failed);
        assert_eq!(output.node_state.error, Some("error occurred".to_string()));
    }

    #[tokio::test]
    async fn test_delete_removes_state() {
        let (manager, _dir) = create_manager().await;
        let execution_id = Uuid::new_v4();

        manager
            .save_state(create_save_input(execution_id, ExecutionStatus::Pending))
            .await
            .unwrap();

        assert!(manager
            .load_state(LoadStateInput { execution_id })
            .await
            .is_ok());

        manager.delete_state(execution_id).await.unwrap();

        let result = manager.load_state(LoadStateInput { execution_id }).await;
        assert!(matches!(result, Err(StateError::StateNotFound { .. })));
    }

    #[tokio::test]
    async fn test_list_executions() {
        let (manager, _dir) = create_manager().await;
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // List should be empty initially
        let list = manager
            .list_executions(ListExecutionsInput::default())
            .await
            .unwrap();
        assert!(list.executions.is_empty());

        // Save two states
        manager
            .save_state(create_save_input(id1, ExecutionStatus::Running))
            .await
            .unwrap();
        manager
            .save_state(create_save_input(id2, ExecutionStatus::Completed))
            .await
            .unwrap();

        let list = manager
            .list_executions(ListExecutionsInput::default())
            .await
            .unwrap();
        assert_eq!(list.total_count, 2);
    }

    #[tokio::test]
    async fn test_list_executions_status_filter() {
        let (manager, _dir) = create_manager().await;
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let mut state1 = ExecutionState::new(id1, "hash".to_string());
        state1.status = ExecutionStatus::Running;

        let mut state2 = ExecutionState::new(id2, "hash".to_string());
        state2.status = ExecutionStatus::Completed;

        manager
            .save_state(SaveStateInput { state: state1 })
            .await
            .unwrap();
        manager
            .save_state(SaveStateInput { state: state2 })
            .await
            .unwrap();

        let list = manager
            .list_executions(ListExecutionsInput {
                limit: Some(50),
                status_filter: Some(ExecutionStatus::Running),
                offset: Some(0),
            })
            .await
            .unwrap();
        assert_eq!(list.executions.len(), 1);
        assert_eq!(list.executions[0].status, ExecutionStatus::Running);
    }

    #[tokio::test]
    async fn test_list_executions_pagination() {
        let (manager, _dir) = create_manager().await;
        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

        for id in &ids {
            manager
                .save_state(create_save_input(*id, ExecutionStatus::Pending))
                .await
                .unwrap();
        }

        // Get first 2 with offset 0
        let list = manager
            .list_executions(ListExecutionsInput {
                limit: Some(2),
                status_filter: None,
                offset: Some(0),
            })
            .await
            .unwrap();
        assert_eq!(list.executions.len(), 2);
        assert_eq!(list.total_count, 5);
    }
}

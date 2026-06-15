//! Implementation of the GraphManagerService.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#application
//! Implements: ISSUE-STATE-PERSISTENCE-2 — FileSystemGraphManager
//! Issue: #80
//!
//! Provides the concrete `FileSystemGraphManager` that persists execution
//! graphs for TUI history view, with CRUD operations and listing.

use async_trait::async_trait;
use uuid::Uuid;

use crate::state_persistence::application::dto::{ExecutionSummary, ListExecutionsOutput};
use crate::state_persistence::application::service::GraphManagerService;
use crate::state_persistence::domain::{ExecutionGraph, StateError};
use crate::state_persistence::infrastructure::repository::GraphRepository;

/// Concrete implementation of `GraphManagerService` backed by a `GraphRepository`.
///
/// Manages execution graph persistence for TUI history view.
pub struct FileSystemGraphManager {
    repository: Box<dyn GraphRepository>,
}

impl FileSystemGraphManager {
    pub fn new(repository: Box<dyn GraphRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl GraphManagerService for FileSystemGraphManager {
    #[tracing::instrument(skip_all)]
    async fn save_graph(&self, graph: &ExecutionSummary) -> Result<(), StateError> {
        // Build an ExecutionGraph from the summary data.
        // Use execution_id as graph_id for direct lookup by execution ID.
        let mut exec_graph = ExecutionGraph::new(
            graph.execution_id,
            format!("Execution {}", graph.execution_id),
            graph.status,
            graph.started_at,
            graph.completed_at,
            vec![],
            graph.duration_ms.unwrap_or(0),
        );
        exec_graph.graph_id = graph.execution_id;

        self.repository.save_graph(&exec_graph).await
    }

    #[tracing::instrument(skip_all)]
    async fn load_graph(&self, graph_id: Uuid) -> Result<ExecutionSummary, StateError> {
        let graph = self.repository.load_graph(graph_id).await?;
        Ok(ExecutionSummary {
            execution_id: graph.execution_id,
            status: graph.status,
            started_at: graph.started_at,
            completed_at: graph.completed_at,
            duration_ms: Some(graph.total_duration_ms),
            node_count: graph.total_node_count,
            completed_node_count: graph.completed_node_count,
            failed_node_count: graph.failed_node_count,
            skipped_node_count: graph.skipped_node_count,
            has_active_nodes: false,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn list_graphs(&self, limit: u32) -> Result<ListExecutionsOutput, StateError> {
        let ids = self.repository.list_graphs(limit, 0).await?;
        let total_count = self.repository.count().await? as u32;

        let mut executions = Vec::new();
        for id in ids {
            if let Ok(graph) = self.repository.load_graph(id).await {
                executions.push(ExecutionSummary {
                    execution_id: graph.execution_id,
                    status: graph.status,
                    started_at: graph.started_at,
                    completed_at: graph.completed_at,
                    duration_ms: Some(graph.total_duration_ms),
                    node_count: graph.total_node_count,
                    completed_node_count: graph.completed_node_count,
                    failed_node_count: graph.failed_node_count,
                    skipped_node_count: graph.skipped_node_count,
                    has_active_nodes: false,
                });
            }
        }

        Ok(ListExecutionsOutput {
            executions,
            total_count,
            limit,
            offset: 0,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn delete_graph(&self, graph_id: Uuid) -> Result<(), StateError> {
        self.repository.delete_graph(graph_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    use crate::state_persistence::domain::ExecutionStatus;
    use crate::state_persistence::infrastructure::FileSystemGraphRepository;

    #[tracing::instrument(skip_all)]
    async fn create_manager() -> (FileSystemGraphManager, TempDir) {
        let dir = TempDir::new().unwrap();
        let repo = FileSystemGraphRepository::new(dir.path().to_path_buf())
            .await
            .unwrap();
        let manager = FileSystemGraphManager::new(Box::new(repo));
        (manager, dir)
    }

    #[tracing::instrument(skip_all)]
    fn create_summary(execution_id: Uuid) -> ExecutionSummary {
        ExecutionSummary {
            execution_id,
            status: ExecutionStatus::Completed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            duration_ms: Some(1500),
            node_count: 2,
            completed_node_count: 2,
            failed_node_count: 0,
            skipped_node_count: 0,
            has_active_nodes: false,
        }
    }

    #[tokio::test]
    async fn test_save_and_load_graph() {
        let (manager, _dir) = create_manager().await;
        let execution_id = Uuid::new_v4();
        let summary = create_summary(execution_id);

        manager.save_graph(&summary).await.unwrap();

        // Load back via list
        let list = manager.list_graphs(10).await.unwrap();
        assert_eq!(list.executions.len(), 1);
        assert_eq!(list.executions[0].execution_id, execution_id);
    }

    #[tokio::test]
    async fn test_list_graphs_empty() {
        let (manager, _dir) = create_manager().await;
        let list = manager.list_graphs(10).await.unwrap();
        assert!(list.executions.is_empty());
    }

    #[tokio::test]
    async fn test_list_graphs_multiple() {
        let (manager, _dir) = create_manager().await;
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        manager.save_graph(&create_summary(id1)).await.unwrap();
        manager.save_graph(&create_summary(id2)).await.unwrap();

        let list = manager.list_graphs(10).await.unwrap();
        assert_eq!(list.executions.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_graph() {
        let (manager, _dir) = create_manager().await;
        let execution_id = Uuid::new_v4();
        let summary = create_summary(execution_id);

        manager.save_graph(&summary).await.unwrap();
        assert_eq!(manager.list_graphs(10).await.unwrap().executions.len(), 1);

        // Delete by execution_id (which is the lookup key used in save_graph)
        manager.delete_graph(execution_id).await.unwrap();

        let list = manager.list_graphs(10).await.unwrap();
        assert_eq!(list.executions.len(), 0);
    }
}

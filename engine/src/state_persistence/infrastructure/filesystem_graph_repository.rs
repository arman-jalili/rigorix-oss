//! Filesystem implementation of `GraphRepository`.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#infrastructure
//! Implements: ISSUE-STATE-PERSISTENCE-2 — FileSystemGraphRepository
//! Issue: #80
//!
//! Provides a filesystem-backed `GraphRepository` that stores execution graphs
//! as JSON files. Graph files are stored as `{graph_dir}/{graph_id}.json`.
//!
//! Graphs are persisted separately from state files since they are larger
//! (include the full DAG structure and node metadata) and are accessed less
//! frequently (primarily by the TUI).

use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

use crate::state_persistence::domain::{ExecutionGraph, StateError};

use super::repository::GraphRepository;

/// Filesystem-backed implementation of `GraphRepository`.
///
/// Stores execution graphs as JSON files in a configurable directory.
/// Each graph is stored as `{graph_id}.json`.
pub struct FileSystemGraphRepository {
    /// Directory where graph files are stored.
    graph_dir: PathBuf,
}

impl FileSystemGraphRepository {
    /// Create a new `FileSystemGraphRepository`.
    ///
    /// The graph directory is created if it doesn't exist.
    pub async fn new(graph_dir: impl Into<PathBuf>) -> Result<Self, StateError> {
        let graph_dir: PathBuf = graph_dir.into();

        if !graph_dir.exists() {
            fs::create_dir_all(&graph_dir)
                .await
                .map_err(|e| StateError::DirectoryError {
                    detail: format!("Failed to create graph directory {:?}: {}", graph_dir, e),
                })?;
        }

        if !graph_dir.is_dir() {
            return Err(StateError::DirectoryError {
                detail: format!("Graph path {:?} exists but is not a directory", graph_dir),
            });
        }

        Ok(Self { graph_dir })
    }

    fn graph_path(&self, graph_id: Uuid) -> PathBuf {
        self.graph_dir.join(format!("{}.graph.json", graph_id))
    }

    fn execution_index_path(&self, execution_id: Uuid) -> PathBuf {
        self.graph_dir
            .join(format!("idx_{}.graph.json", execution_id))
    }
}

#[async_trait]
impl GraphRepository for FileSystemGraphRepository {
    async fn save_graph(&self, graph: &ExecutionGraph) -> Result<(), StateError> {
        let path = self.graph_path(graph.graph_id);
        let temp = self
            .graph_dir
            .join(format!("{}.graph.json.tmp", graph.graph_id));

        // Serialise
        let json =
            serde_json::to_string_pretty(graph).map_err(|e| StateError::SerialisationError {
                detail: format!("Failed to serialise execution graph: {}", e),
            })?;

        // Write to temp
        fs::write(&temp, &json)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to write temp graph file {:?}: {}", temp, e),
            })?;

        // Atomic rename
        fs::rename(&temp, &path)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!(
                    "Failed to rename graph file {:?} to {:?}: {}",
                    temp, path, e
                ),
            })?;

        // Also save an execution-id-indexed copy for lookup by execution ID
        let idx_path = self.execution_index_path(graph.execution_id);
        // Create a symlink or copy for the index
        // Using copy for simplicity (symlinks may not work on all platforms)
        if !idx_path.exists() {
            fs::copy(&path, &idx_path)
                .await
                .map_err(|e| StateError::IoError {
                    detail: format!("Failed to create execution index for graph: {}", e),
                })?;
        }

        Ok(())
    }

    async fn load_graph(&self, graph_id: Uuid) -> Result<ExecutionGraph, StateError> {
        let path = self.graph_path(graph_id);

        if !path.exists() {
            return Err(StateError::GraphNotFound {
                graph_id: graph_id.to_string(),
            });
        }

        let data = fs::read_to_string(&path)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to read graph file {:?}: {}", path, e),
            })?;

        serde_json::from_str(&data).map_err(|e| StateError::CorruptedState {
            path: path.to_string_lossy().to_string(),
            detail: format!("Failed to deserialise execution graph: {}", e),
        })
    }

    async fn load_by_execution_id(&self, execution_id: Uuid) -> Result<ExecutionGraph, StateError> {
        let idx_path = self.execution_index_path(execution_id);

        if !idx_path.exists() {
            return Err(StateError::GraphNotFound {
                graph_id: format!("execution:{}", execution_id),
            });
        }

        let data = fs::read_to_string(&idx_path)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to read graph index file {:?}: {}", idx_path, e),
            })?;

        serde_json::from_str(&data).map_err(|e| StateError::CorruptedState {
            path: idx_path.to_string_lossy().to_string(),
            detail: format!("Failed to deserialise execution graph from index: {}", e),
        })
    }

    async fn delete_graph(&self, graph_id: Uuid) -> Result<(), StateError> {
        let path = self.graph_path(graph_id);
        let temp = self.graph_dir.join(format!("{}.graph.json.tmp", graph_id));

        if path.exists() {
            // Before deleting, try to load the graph to get the execution_id for index cleanup
            if let Ok(graph) = self.load_graph(graph_id).await {
                let idx_path = self.execution_index_path(graph.execution_id);
                if idx_path.exists() {
                    let _ = fs::remove_file(&idx_path).await;
                }
            }

            fs::remove_file(&path)
                .await
                .map_err(|e| StateError::IoError {
                    detail: format!("Failed to delete graph file {:?}: {}", path, e),
                })?;
        }

        // Clean up temp file
        if temp.exists() {
            let _ = fs::remove_file(&temp).await;
        }

        Ok(())
    }

    async fn list_graphs(&self, limit: u32, offset: u32) -> Result<Vec<Uuid>, StateError> {
        let mut entries = fs::read_dir(&self.graph_dir)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to read graph directory {:?}: {}", self.graph_dir, e),
            })?;

        let mut ids = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to read directory entry: {}", e),
            })?
        {
            let path = entry.path();

            // Only match graph files: {uuid}.graph.json
            let file_name = path
                .file_name()
                .map_or(String::new(), |n| n.to_string_lossy().to_string());
            if !file_name.ends_with(".graph.json") || file_name.starts_with("idx_") {
                continue;
            }

            // Extract UUID from filename (remove .graph.json suffix)
            if let Some(stem) = file_name.strip_suffix(".graph.json")
                && let Ok(uuid) = Uuid::parse_str(stem) {
                    ids.push(uuid);
                }
        }

        // Sort by UUID for deterministic ordering (most recent UUIDs sort later)
        ids.sort();

        let offset = offset as usize;
        let limit = limit as usize;
        if offset >= ids.len() {
            return Ok(Vec::new());
        }

        Ok(ids[offset..std::cmp::min(offset + limit, ids.len())].to_vec())
    }

    async fn count(&self) -> Result<u64, StateError> {
        Ok(self.list_graphs(u32::MAX, 0).await?.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    use crate::state_persistence::domain::{ExecutionGraphNode, ExecutionStatus};

    fn create_test_graph(execution_id: Uuid) -> ExecutionGraph {
        let nodes = vec![
            ExecutionGraphNode::new(
                Uuid::new_v4(),
                "compile".to_string(),
                "build".to_string(),
                vec![],
            ),
            ExecutionGraphNode::new(
                Uuid::new_v4(),
                "test".to_string(),
                "test".to_string(),
                vec![],
            ),
        ];

        ExecutionGraph::new(
            execution_id,
            "test-execution".to_string(),
            ExecutionStatus::Completed,
            Utc::now(),
            Some(Utc::now()),
            nodes,
            1500,
        )
    }

    async fn create_repo() -> (FileSystemGraphRepository, TempDir) {
        let dir = TempDir::new().unwrap();
        let repo = FileSystemGraphRepository::new(dir.path().to_path_buf())
            .await
            .unwrap();
        (repo, dir)
    }

    #[tokio::test]
    async fn test_save_and_load_graph() {
        let (repo, _dir) = create_repo().await;
        let execution_id = Uuid::new_v4();
        let graph = create_test_graph(execution_id);

        repo.save_graph(&graph).await.unwrap();

        let loaded = repo.load_graph(graph.graph_id).await.unwrap();
        assert_eq!(loaded.graph_id, graph.graph_id);
        assert_eq!(loaded.execution_id, execution_id);
        assert_eq!(loaded.name, "test-execution");
        assert_eq!(loaded.nodes.len(), 2);
    }

    #[tokio::test]
    async fn test_load_nonexistent_graph() {
        let (repo, _dir) = create_repo().await;
        let result = repo.load_graph(Uuid::new_v4()).await;
        assert!(matches!(result, Err(StateError::GraphNotFound { .. })));
    }

    #[tokio::test]
    async fn test_load_by_execution_id() {
        let (repo, _dir) = create_repo().await;
        let execution_id = Uuid::new_v4();
        let graph = create_test_graph(execution_id);

        repo.save_graph(&graph).await.unwrap();

        let loaded = repo.load_by_execution_id(execution_id).await.unwrap();
        assert_eq!(loaded.execution_id, execution_id);
    }

    #[tokio::test]
    async fn test_delete_graph() {
        let (repo, _dir) = create_repo().await;
        let execution_id = Uuid::new_v4();
        let graph = create_test_graph(execution_id);

        repo.save_graph(&graph).await.unwrap();
        assert!(repo.load_graph(graph.graph_id).await.is_ok());

        repo.delete_graph(graph.graph_id).await.unwrap();
        assert!(matches!(
            repo.load_graph(graph.graph_id).await,
            Err(StateError::GraphNotFound { .. })
        ));
    }

    #[tokio::test]
    async fn test_delete_graph_removes_index() {
        let (repo, _dir) = create_repo().await;
        let execution_id = Uuid::new_v4();
        let graph = create_test_graph(execution_id);

        repo.save_graph(&graph).await.unwrap();
        assert!(repo.load_by_execution_id(execution_id).await.is_ok());

        repo.delete_graph(graph.graph_id).await.unwrap();
        assert!(matches!(
            repo.load_by_execution_id(execution_id).await,
            Err(StateError::GraphNotFound { .. })
        ));
    }

    #[tokio::test]
    async fn test_list_graphs() {
        let (repo, _dir) = create_repo().await;
        let eid1 = Uuid::new_v4();
        let eid2 = Uuid::new_v4();

        assert!(repo.list_graphs(10, 0).await.unwrap().is_empty());

        repo.save_graph(&create_test_graph(eid1)).await.unwrap();
        repo.save_graph(&create_test_graph(eid2)).await.unwrap();

        let ids = repo.list_graphs(10, 0).await.unwrap();
        assert_eq!(ids.len(), 2);
    }

    #[tokio::test]
    async fn test_list_graphs_pagination() {
        let (repo, _dir) = create_repo().await;

        // Add 5 graphs
        for _ in 0..5 {
            repo.save_graph(&create_test_graph(Uuid::new_v4()))
                .await
                .unwrap();
        }

        let page1 = repo.list_graphs(2, 0).await.unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = repo.list_graphs(2, 2).await.unwrap();
        assert_eq!(page2.len(), 2);

        let page3 = repo.list_graphs(2, 4).await.unwrap();
        assert_eq!(page3.len(), 1);
    }

    #[tokio::test]
    async fn test_count() {
        let (repo, _dir) = create_repo().await;
        assert_eq!(repo.count().await.unwrap(), 0);

        repo.save_graph(&create_test_graph(Uuid::new_v4()))
            .await
            .unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);
    }
}

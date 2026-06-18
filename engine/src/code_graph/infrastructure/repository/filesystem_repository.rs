//! Filesystem-backed repository for CodeGraph persistence.
//!
//! @canonical .pi/architecture/modules/code-graph.md#infrastructure
//! Implements: PersistedCodeGraph — FilesystemCodeGraphRepository
//! Issue: issue-persistedcodegraph
//!
//! Stores CodeGraph instances as individual JSON files on disk at a
//! configurable directory path. Each graph is saved as:
//!   `{storage_dir}/{graph_id}.json`
//!
//! An index file `{storage_dir}/.index.json` tracks all graph IDs for
//! efficient listing without scanning the directory.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::code_graph::domain::{CodeGraph, CodeGraphError};

use super::CodeGraphRepository;

/// Filesystem-backed implementation of CodeGraphRepository.
///
/// Stores each CodeGraph as a JSON file in the specified storage directory.
/// Thread-safe with Mutex-guarded operations.
///
/// # Contract
/// - Graphs are atomically persisted (write to temp file, then rename)
/// - An index file tracks all stored graph IDs
/// - Storage directory is created on first use if it doesn't exist
pub struct FilesystemCodeGraphRepository {
    /// Path to the storage directory.
    storage_dir: PathBuf,

    /// In-memory index for graph listing (persisted as .index.json).
    index: Mutex<HashMap<Uuid, PathBuf>>,
}

impl FilesystemCodeGraphRepository {
    /// Create a new FilesystemCodeGraphRepository.
    ///
    /// The storage directory will be created on the first write operation.
    /// The index is loaded synchronously from disk on construction.
    pub fn new(storage_dir: impl Into<PathBuf>) -> Self {
        let storage_dir: PathBuf = storage_dir.into();
        let index = if storage_dir.join(".index.json").exists() {
            std::fs::read_to_string(storage_dir.join(".index.json"))
                .ok()
                .and_then(|content| serde_json::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        Self {
            storage_dir,
            index: Mutex::new(index),
        }
    }

    /// Return the path to a graph's JSON file.
    fn graph_path(&self, graph_id: Uuid) -> PathBuf {
        self.storage_dir.join(format!("{}.json", graph_id))
    }

    /// Return the path to the index file.
    fn index_path(&self) -> PathBuf {
        self.storage_dir.join(".index.json")
    }

    /// Ensure the storage directory exists.
    async fn ensure_directory(&self) -> Result<(), CodeGraphError> {
        fs::create_dir_all(&self.storage_dir)
            .await
            .map_err(|e| CodeGraphError::IoError {
                detail: format!("Failed to create storage directory: {}", e),
            })
    }

    /// Save the index to disk.
    async fn save_index(&self, index: &HashMap<Uuid, PathBuf>) -> Result<(), CodeGraphError> {
        let content = serde_json::to_string_pretty(index).map_err(|e| {
            CodeGraphError::SerializationError {
                detail: format!("Failed to serialize index: {}", e),
            }
        })?;

        let mut file = fs::File::create(self.index_path())
            .await
            .map_err(|e| CodeGraphError::IoError {
                detail: format!("Failed to create index file: {}", e),
            })?;

        file.write_all(content.as_bytes())
            .await
            .map_err(|e| CodeGraphError::IoError {
                detail: format!("Failed to write index file: {}", e),
            })?;

        Ok(())
    }
}

#[async_trait]
impl CodeGraphRepository for FilesystemCodeGraphRepository {
    async fn save(&self, graph: &CodeGraph) -> Result<(), CodeGraphError> {
        self.ensure_directory().await?;

        let graph_id = Uuid::new_v4();
        let path = self.graph_path(graph_id);

        // Serialize graph to JSON
        let serialized = serde_json::to_string_pretty(graph).map_err(|e| {
            CodeGraphError::SerializationError {
                detail: format!("Failed to serialize graph: {}", e),
            }
        })?;

        // Write to a temp file first, then rename for atomicity
        let temp_path = self.storage_dir.join(format!("{}.tmp", graph_id));
        let mut file = fs::File::create(&temp_path)
            .await
            .map_err(|e| CodeGraphError::IoError {
                detail: format!("Failed to create temp file: {}", e),
            })?;
        file.write_all(serialized.as_bytes())
            .await
            .map_err(|e| CodeGraphError::IoError {
                detail: format!("Failed to write graph data: {}", e),
            })?;

        // Rename temp file to final path (atomic on most filesystems)
        fs::rename(&temp_path, &path)
            .await
            .map_err(|e| CodeGraphError::IoError {
                detail: format!("Failed to rename temp file: {}", e),
            })?;

        // Update index
        let index_clone = {
            let mut index = self.index.lock().map_err(|e| CodeGraphError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;
            index.insert(graph_id, path);
            index.clone()
        };
        self.save_index(&index_clone).await?;

        Ok(())
    }

    async fn load(&self, graph_id: Uuid) -> Result<CodeGraph, CodeGraphError> {
        let path = self.graph_path(graph_id);

        let content = fs::read_to_string(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CodeGraphError::InvalidOperation {
                    reason: format!("Graph not found: {}", graph_id),
                }
            } else {
                CodeGraphError::IoError {
                    detail: format!("Failed to read graph file: {}", e),
                }
            }
        })?;

        serde_json::from_str(&content).map_err(|e| CodeGraphError::DeserializationError {
            detail: format!("Failed to deserialize graph: {}", e),
        })
    }

    async fn exists(&self, graph_id: Uuid) -> Result<bool, CodeGraphError> {
        let path = self.graph_path(graph_id);
        let index = self.index.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;
        Ok(index.contains_key(&graph_id) || path.exists())
    }

    async fn delete(&self, graph_id: Uuid) -> Result<(), CodeGraphError> {
        let path = self.graph_path(graph_id);

        // Remove file (ignore if not found)
        let _ = fs::remove_file(&path).await;

        // Update index
        let index_clone = {
            let mut index = self.index.lock().map_err(|e| CodeGraphError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;
            index.remove(&graph_id);
            index.clone()
        };
        self.save_index(&index_clone).await?;

        Ok(())
    }

    async fn list_ids(&self) -> Result<Vec<Uuid>, CodeGraphError> {
        let index = self.index.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;
        let mut ids: Vec<Uuid> = index.keys().copied().collect();
        ids.sort();
        Ok(ids)
    }

    async fn list_ids_paginated(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Uuid>, CodeGraphError> {
        let all = self.list_ids().await?;
        Ok(all
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
    }

    async fn count(&self) -> Result<u64, CodeGraphError> {
        let index = self.index.lock().map_err(|e| CodeGraphError::InternalError {
            detail: format!("Lock error: {}", e),
        })?;
        Ok(index.len() as u64)
    }

    async fn search(&self, query: &str, limit: u32) -> Result<Vec<CodeGraph>, CodeGraphError> {
        let ids = self.list_ids().await?;
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for id in ids {
            if results.len() >= limit as usize {
                break;
            }
            if let Ok(graph) = self.load(id).await {
                if graph.metadata.name.to_lowercase().contains(&query_lower)
                    || graph.metadata.source.to_lowercase().contains(&query_lower)
                    || graph.metadata.description.to_lowercase().contains(&query_lower)
                {
                    results.push(graph);
                }
            }
        }

        results.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_graph::domain::GraphMetadata;
    use chrono::Utc;
    use tempfile::tempdir;

    fn create_test_graph(name: &str) -> CodeGraph {
        CodeGraph::new(GraphMetadata {
            name: name.to_string(),
            source: "test".to_string(),
            created_at: Utc::now(),
            description: "Test graph".to_string(),
            total_modules_scanned: 0,
            schema_version: "1.0.0".to_string(),
        })
    }

    #[tokio::test]
    async fn test_filesystem_save_and_load() {
        let dir = tempdir().unwrap();
        let repo = FilesystemCodeGraphRepository::new(dir.path().join("graphs"));
        let graph = create_test_graph("fs-test");

        repo.save(&graph).await.unwrap();

        let ids = repo.list_ids().await.unwrap();
        assert_eq!(ids.len(), 1);

        let loaded = repo.load(ids[0]).await.unwrap();
        assert_eq!(loaded.metadata.name, "fs-test");
    }

    #[tokio::test]
    async fn test_filesystem_exists() {
        let dir = tempdir().unwrap();
        let repo = FilesystemCodeGraphRepository::new(dir.path().join("graphs"));
        let graph = create_test_graph("exists-test");
        repo.save(&graph).await.unwrap();

        let ids = repo.list_ids().await.unwrap();
        assert!(repo.exists(ids[0]).await.unwrap());
        assert!(!repo.exists(Uuid::new_v4()).await.unwrap());
    }

    #[tokio::test]
    async fn test_filesystem_delete() {
        let dir = tempdir().unwrap();
        let repo = FilesystemCodeGraphRepository::new(dir.path().join("graphs"));
        let graph = create_test_graph("delete-test");
        repo.save(&graph).await.unwrap();

        let ids = repo.list_ids().await.unwrap();
        assert_eq!(ids.len(), 1);

        repo.delete(ids[0]).await.unwrap();
        assert_eq!(repo.list_ids().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_filesystem_load_nonexistent() {
        let dir = tempdir().unwrap();
        let repo = FilesystemCodeGraphRepository::new(dir.path().join("graphs"));
        let result = repo.load(Uuid::new_v4()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_filesystem_persistence_across_reloads() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("graphs");

        // Save in first repo instance
        {
            let repo = FilesystemCodeGraphRepository::new(&path);
            let graph = create_test_graph("persist-test");
            repo.save(&graph).await.unwrap();
        }

        // Load in second repo instance (reads from disk)
        {
            let repo = FilesystemCodeGraphRepository::new(&path);
            let ids = repo.list_ids().await.unwrap();
            assert_eq!(ids.len(), 1, "Should persist across reloads");

            let loaded = repo.load(ids[0]).await.unwrap();
            assert_eq!(loaded.metadata.name, "persist-test");
        }
    }
}

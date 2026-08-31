//! Filesystem-backed `SymbolRepository` implementation (JSON cache file).
//!
//! @canonical .pi/architecture/modules/repo-engine.md#repositories
//! Implements: GAP-A-15 — SymbolRepository filesystem impl
//!
//! Persists `SymbolDefinition`s as JSON at a configured path. Loads restore
//! a `SymbolGraph` with all stored symbols (adjacency is rebuilt by the
//! graph service on demand).

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::repo_engine::domain::{RepoEngineError, SymbolDefinition, SymbolGraph};
use crate::repo_engine::infrastructure::repository::SymbolRepository;

/// Filesystem-backed symbol repository storing a JSON array of definitions.
pub struct FileSystemSymbolRepository {
    /// Path to the JSON cache file.
    path: PathBuf,
}

impl FileSystemSymbolRepository {
    /// Create a repository persisting to the given file path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[async_trait]
impl SymbolRepository for FileSystemSymbolRepository {
    async fn save_symbol(&self, symbol: &SymbolDefinition) -> Result<(), RepoEngineError> {
        let mut all = self.load_raw().await?;
        if all.contains_key(&symbol.name) {
            return Err(RepoEngineError::DuplicateSymbol {
                name: symbol.name.clone(),
            });
        }
        all.insert(symbol.name.clone(), symbol.clone());
        self.write_raw(&all).await
    }

    async fn save_symbols_batch(
        &self,
        symbols: &[SymbolDefinition],
    ) -> Result<Vec<SymbolDefinition>, RepoEngineError> {
        let mut all = self.load_raw().await?;
        let mut rejected = Vec::new();
        for symbol in symbols {
            if all.contains_key(&symbol.name) {
                rejected.push(symbol.clone());
            } else {
                all.insert(symbol.name.clone(), symbol.clone());
            }
        }
        self.write_raw(&all).await?;
        Ok(rejected)
    }

    async fn load_all(&self) -> Result<SymbolGraph, RepoEngineError> {
        let all = self.load_raw().await?;
        let mut graph = SymbolGraph::new();
        for symbol in all.values() {
            let _ = graph.add_symbol(symbol.clone()); // duplicates can't occur (keyed by name)
        }
        Ok(graph)
    }

    async fn contains(&self, name: &str) -> Result<bool, RepoEngineError> {
        Ok(self.load_raw().await?.contains_key(name))
    }

    async fn count(&self) -> Result<usize, RepoEngineError> {
        Ok(self.load_raw().await?.len())
    }

    async fn delete(&self, name: &str) -> Result<bool, RepoEngineError> {
        let mut all = self.load_raw().await?;
        let removed = all.remove(name).is_some();
        if removed {
            self.write_raw(&all).await?;
        }
        Ok(removed)
    }

    async fn clear(&self) -> Result<(), RepoEngineError> {
        self.write_raw(&HashMap::new()).await
    }
}

impl FileSystemSymbolRepository {
    /// Load the raw name -> symbol map (empty if the file is absent).
    async fn load_raw(&self) -> Result<HashMap<String, SymbolDefinition>, RepoEngineError> {
        if !tokio::fs::try_exists(&self.path).await.unwrap_or(false) {
            return Ok(HashMap::new());
        }
        let content = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(RepoEngineError::from)?;
        if content.trim().is_empty() {
            return Ok(HashMap::new());
        }
        serde_json::from_str(&content).map_err(|e| RepoEngineError::Internal {
            detail: format!("corrupt symbol cache '{}': {}", self.path.display(), e),
        })
    }

    /// Persist the map as pretty JSON, creating parent directories.
    async fn write_raw(
        &self,
        symbols: &HashMap<String, SymbolDefinition>,
    ) -> Result<(), RepoEngineError> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let json =
            serde_json::to_string_pretty(symbols).map_err(|e| RepoEngineError::Internal {
                detail: format!("serialize symbol cache: {e}"),
            })?;
        tokio::fs::write(&self.path, json)
            .await
            .map_err(RepoEngineError::from)
    }
}

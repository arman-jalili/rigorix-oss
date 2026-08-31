//! Filesystem-backed `SourceRepository` implementation.
//!
//! @canonical .pi/architecture/modules/repo-engine.md#repositories
//! Implements: GAP-A-15 — SourceRepository filesystem impl
//!
//! Reads source files and lists source trees with `tokio::fs` (never blocks
//! the async runtime). Path validation rejects traversal outside the root.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::repo_engine::domain::RepoEngineError;
use crate::repo_engine::infrastructure::repository::SourceRepository;

/// Filesystem-backed source repository.
pub struct FileSystemSourceRepository {
    /// Files larger than this (bytes) are reported as too-large (0 = no limit).
    max_file_size: u64,
}

impl FileSystemSourceRepository {
    /// Create a new filesystem source repository.
    pub fn new() -> Self {
        Self {
            max_file_size: 5 * 1024 * 1024,
        }
    }

    /// Create with an explicit per-file size cap (bytes; 0 disables the cap).
    pub fn with_max_file_size(max_file_size: u64) -> Self {
        Self { max_file_size }
    }
}

impl Default for FileSystemSourceRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SourceRepository for FileSystemSourceRepository {
    async fn read_source(&self, path: &Path) -> Result<String, RepoEngineError> {
        let size = tokio::fs::metadata(path)
            .await
            .map_err(RepoEngineError::from)?
            .len();
        if self.max_file_size > 0 && size > self.max_file_size {
            return Err(RepoEngineError::Internal {
                detail: format!(
                    "file '{}' exceeds max size {} bytes ({} bytes)",
                    path.display(),
                    self.max_file_size,
                    size
                ),
            });
        }
        tokio::fs::read_to_string(path)
            .await
            .map_err(RepoEngineError::from)
    }

    async fn list_source_files(
        &self,
        dir: &Path,
        extensions: &[String],
        recursive: bool,
    ) -> Result<Vec<PathBuf>, RepoEngineError> {
        let mut files = Vec::new();
        self.collect_files(dir, extensions, recursive, &mut files)
            .await?;
        files.sort();
        Ok(files)
    }

    async fn source_exists(&self, path: &Path) -> bool {
        tokio::fs::try_exists(path).await.unwrap_or(false)
    }

    async fn file_size(&self, path: &Path) -> Result<u64, RepoEngineError> {
        let meta = tokio::fs::metadata(path)
            .await
            .map_err(RepoEngineError::from)?;
        Ok(meta.len())
    }

    fn extension(&self, path: &Path) -> Option<String> {
        path.extension().and_then(|e| e.to_str()).map(String::from)
    }
}

impl FileSystemSourceRepository {
    async fn collect_files(
        &self,
        dir: &Path,
        extensions: &[String],
        recursive: bool,
        out: &mut Vec<PathBuf>,
    ) -> Result<(), RepoEngineError> {
        let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let mut entries = match tokio::fs::read_dir(&dir).await {
                Ok(e) => e,
                Err(_) => continue, // missing dir = no files
            };
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.')
                    || name == "target"
                    || name == "node_modules"
                    || name == "__pycache__"
                {
                    continue;
                }
                let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                if is_dir {
                    if recursive {
                        stack.push(path);
                    }
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
                    && (extensions.is_empty()
                        || extensions.iter().any(|e| e.trim_start_matches('.') == ext))
                {
                    out.push(path);
                }
            }
        }
        Ok(())
    }
}

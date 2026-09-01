//! Filesystem-backed `TemplateRepository`.
//!
//! @canonical .pi/architecture/modules/template-system.md#repository
//! Implements: GAP-A-20 — filesystem TemplateRepository in the engine
//!
//! Reads templates from `.rigorix/templates/` (or any configured dir) with
//! `tokio::fs`; built-in templates are delegated to an embedded
//! `InMemoryTemplateRepository` (compile-time sources).

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::templates::application::dto::{LoadBuiltinsInput, LoadBuiltinsOutput};
use crate::templates::domain::error::TemplateError;
use crate::templates::infrastructure::repository::{
    InMemoryTemplateRepository, TemplateRepository,
};

/// Filesystem `TemplateRepository` backed by `.rigorix/templates/`.
pub struct FileSystemTemplateRepository {
    /// Root directory scanned for template files (e.g. ".rigorix/templates").
    root: PathBuf,

    /// Delegated built-in template sources.
    builtins: InMemoryTemplateRepository,

    /// Maximum template file size in bytes (0 = no limit).
    max_file_size: u64,
}

impl FileSystemTemplateRepository {
    /// Create a repository rooted at the given templates directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            builtins: InMemoryTemplateRepository::new(),
            max_file_size: 0,
        }
    }

    /// Register a built-in template (compile-time embedded source).
    pub fn add_builtin(&mut self, id: &'static str, toml: &'static str) {
        self.builtins.add_builtin(id, toml);
    }
}

impl Default for FileSystemTemplateRepository {
    fn default() -> Self {
        Self::new(".rigorix/templates")
    }
}

#[async_trait]
impl TemplateRepository for FileSystemTemplateRepository {
    async fn read_template_file(&self, path: &str) -> Result<String, TemplateError> {
        let resolved = self.root.join(path);
        if !tokio::fs::try_exists(&resolved).await.unwrap_or(false) {
            return Err(TemplateError::NotFound {
                id: path.to_string(),
                available: Vec::new(),
            });
        }
        if self.max_file_size > 0
            && let Ok(meta) = tokio::fs::metadata(&resolved).await
            && meta.len() > self.max_file_size
        {
            return Err(TemplateError::Io {
                io_error: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "template '{}' exceeds max size {} bytes",
                        path, self.max_file_size
                    ),
                ),
            });
        }
        tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| TemplateError::Io { io_error: e })
    }

    async fn list_template_files(
        &self,
        dir: &str,
        extension: &str,
    ) -> Result<Vec<String>, TemplateError> {
        let target = self.root.join(dir);
        let mut files = Vec::new();
        let mut entries = match tokio::fs::read_dir(&target).await {
            Ok(e) => e,
            Err(_) => return Ok(Vec::new()), // missing dir = no templates (not an error)
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                continue;
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && ext == extension.trim_start_matches('.')
            {
                files.push(path.to_string_lossy().to_string());
            }
        }
        files.sort();
        Ok(files)
    }

    async fn template_file_exists(&self, path: &str) -> bool {
        tokio::fs::try_exists(self.root.join(path))
            .await
            .unwrap_or(false)
    }

    async fn load_builtin_sources(
        &self,
        input: LoadBuiltinsInput,
    ) -> Result<LoadBuiltinsOutput, TemplateError> {
        self.builtins.load_builtin_sources(input).await
    }

    async fn get_builtin_source(&self, id: &str) -> Option<&'static str> {
        self.builtins.get_builtin_source(id).await
    }

    async fn list_builtin_ids(&self) -> Vec<&'static str> {
        self.builtins.list_builtin_ids().await
    }
}

/// Select a `TemplateRepository` backed by filesystem when a templates
/// directory is configured, else the in-memory repository (GAP-A-20
/// config-gated wiring).
pub fn template_repository_from_config(
    template_dirs: &[String],
) -> Box<dyn TemplateRepository + Send + Sync> {
    if let Some(dir) = template_dirs.first().filter(|d| !d.is_empty()) {
        Box::new(FileSystemTemplateRepository::new(Path::new(dir)))
    } else {
        Box::new(InMemoryTemplateRepository::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_template_from_disk() {
        let dir = TempDir::new().unwrap();
        let repo = FileSystemTemplateRepository::new(dir.path());
        let template_path = "simple.toml";
        std::fs::write(dir.path().join(template_path), "version = \"1.0.0\"\n").unwrap();

        let content = repo.read_template_file(template_path).await.unwrap();
        assert!(content.contains("1.0.0"));
        assert!(repo.template_file_exists(template_path).await);
        assert!(!repo.template_file_exists("missing.toml").await);
    }

    #[tokio::test]
    async fn test_read_template_not_found_is_typed_error() {
        let dir = TempDir::new().unwrap();
        let repo = FileSystemTemplateRepository::new(dir.path());
        let err = repo
            .read_template_file("does-not-exist.toml")
            .await
            .unwrap_err();
        assert!(matches!(err, TemplateError::NotFound { .. }));
    }

    #[tokio::test]
    async fn test_list_template_files_by_extension() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("a.toml"), "a").unwrap();
        std::fs::write(dir.path().join("b.txt"), "b").unwrap();
        std::fs::write(dir.path().join("sub/c.toml"), "c").unwrap();

        let repo = FileSystemTemplateRepository::new(dir.path());
        let files = repo.list_template_files(".", "toml").await.unwrap();
        assert_eq!(files.len(), 1, "non-recursive listing: {files:?}");
        assert!(files[0].ends_with("a.toml"));
    }

    #[tokio::test]
    async fn test_builtin_delegation() {
        let dir = TempDir::new().unwrap();
        let mut repo = FileSystemTemplateRepository::new(dir.path());
        repo.add_builtin("builtin-1", "version = \"1.0.0\"\n");
        assert_eq!(repo.list_builtin_ids().await, vec!["builtin-1"]);
        assert!(repo.get_builtin_source("builtin-1").await.is_some());
    }

    #[tokio::test]
    async fn test_config_gated_selection() {
        // Configured templates dir -> filesystem repo; empty -> in-memory.
        let dir = TempDir::new().unwrap();
        let fs_repo = template_repository_from_config(&[dir.path().to_string_lossy().to_string()]);
        std::fs::write(dir.path().join("x.toml"), "x").unwrap();
        assert!(fs_repo.template_file_exists("x.toml").await);

        let mem_repo = template_repository_from_config(&[]);
        assert!(!mem_repo.template_file_exists("x.toml").await);
    }
}

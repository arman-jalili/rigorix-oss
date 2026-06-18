//! Filesystem-based implementation of `ConfigRepository`.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: ConfigRepository trait — reads TOML files and resolves paths
//! Issue: #3
//!
//! Reads configuration from the filesystem. Supports explicit paths,
//! CWD (`rigorix.toml`), and home directory (`~/.rigorix/config.toml`)
//! fallback resolution.

use async_trait::async_trait;
use std::path::PathBuf;

use crate::configuration::domain::ConfigurationError;
use crate::configuration::domain::error::ConfigSource;

use super::repository::ConfigRepository;

/// Implementation of `ConfigRepository` that reads from the filesystem.
///
/// Uses `tokio::fs` for async file I/O. Path resolution follows:
/// 1. Explicit path
/// 2. `$CWD/rigorix.toml`
/// 3. `$HOME/.rigorix/config.toml`
pub struct FilesystemConfigRepository {
    /// Current working directory for resolution.
    cwd: PathBuf,
    /// Home directory for fallback resolution.
    home_dir: Option<PathBuf>,
}

impl FilesystemConfigRepository {
    /// Create a new filesystem repository with the given CWD.
    ///
    /// Home directory is detected automatically from `$HOME` env var.
    pub fn new(cwd: PathBuf) -> Self {
        let home_dir = std::env::var_os("HOME").map(PathBuf::from);
        Self { cwd, home_dir }
    }

    /// Create a new filesystem repository with explicit CWD and home.
    pub fn with_home(cwd: PathBuf, home_dir: PathBuf) -> Self {
        Self {
            cwd,
            home_dir: Some(home_dir),
        }
    }
}

#[async_trait]
impl ConfigRepository for FilesystemConfigRepository {
    async fn read_toml_file(&self, path: &str) -> Result<String, ConfigurationError> {
        let content =
            tokio::fs::read_to_string(path)
                .await
                .map_err(|_| ConfigurationError::NotFound {
                    path: path.to_string(),
                    config_source: ConfigSource::CwdFile,
                })?;
        Ok(content)
    }

    async fn resolve_config_path(&self, explicit_path: Option<&str>) -> Option<String> {
        // 1. Check explicit path
        if let Some(path) = explicit_path
            && tokio::fs::try_exists(&path).await.unwrap_or(false)
        {
            return Some(path.to_string());
        }

        // 2. Check CWD/rigorix.toml
        let cwd_path = self.cwd.join("rigorix.toml");
        if tokio::fs::try_exists(&cwd_path).await.unwrap_or(false) {
            return Some(cwd_path.to_string_lossy().to_string());
        }

        // 3. Check ~/.rigorix/config.toml
        if let Some(home) = &self.home_dir {
            let home_path = home.join(".rigorix").join("config.toml");
            if tokio::fs::try_exists(&home_path).await.unwrap_or(false) {
                return Some(home_path.to_string_lossy().to_string());
            }
        }

        None
    }

    async fn read_env_vars(&self, prefix: &str) -> std::collections::HashMap<String, String> {
        let mut vars = std::collections::HashMap::new();
        for (key, value) in std::env::vars() {
            if let Some(stripped) = key.strip_prefix(prefix) {
                // Convert RIGORIX__LOGGING__LEVEL → logging.level
                let mapped = stripped.to_lowercase().replace("__", ".");
                vars.insert(mapped, value);
            }
        }
        vars
    }

    async fn read_env_var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_toml_file_not_found() {
        let repo = FilesystemConfigRepository::new(PathBuf::from("/nonexistent"));
        let result = repo.read_toml_file("/nonexistent/config.toml").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigurationError::NotFound { path, .. } => {
                assert_eq!(path, "/nonexistent/config.toml");
            }
            other => panic!("Expected NotFound, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_read_toml_file_success() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("rigorix.toml");
        tokio::fs::write(&config_path, r#"title = "test""#)
            .await
            .unwrap();

        let repo = FilesystemConfigRepository::new(PathBuf::from("/nonexistent"));
        let content = repo
            .read_toml_file(config_path.to_str().unwrap())
            .await
            .unwrap();
        assert_eq!(content, r#"title = "test""#);
    }

    #[tokio::test]
    async fn test_resolve_config_path_explicit() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("myconfig.toml");
        tokio::fs::write(&config_path, "").await.unwrap();

        let repo = FilesystemConfigRepository::new(PathBuf::from("/nonexistent"));
        let resolved = repo
            .resolve_config_path(Some(config_path.to_str().unwrap()))
            .await;
        assert_eq!(resolved, Some(config_path.to_str().unwrap().to_string()));
    }

    #[tokio::test]
    async fn test_resolve_config_path_cwd() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("rigorix.toml");
        tokio::fs::write(&config_path, "").await.unwrap();

        let repo = FilesystemConfigRepository::new(dir.path().to_path_buf());
        let resolved = repo.resolve_config_path(None).await;
        assert_eq!(resolved, Some(config_path.to_str().unwrap().to_string()));
    }

    #[tokio::test]
    async fn test_resolve_config_path_none_found() {
        let repo = FilesystemConfigRepository::with_home(
            PathBuf::from("/nonexistent"),
            PathBuf::from("/nonexistent_home"),
        );
        let resolved = repo.resolve_config_path(None).await;
        assert_eq!(resolved, None);
    }

    #[tokio::test]
    async fn test_read_env_var() {
        let repo = FilesystemConfigRepository::new(PathBuf::from("/"));
        // Use a known env var that always exists
        let result = repo.read_env_var("PATH").await;
        assert!(result.is_some());
        assert!(!result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_read_env_var_not_found() {
        let repo = FilesystemConfigRepository::new(PathBuf::from("/"));
        let result = repo.read_env_var("RIGORIX_NONEXISTENT_VAR_12345").await;
        assert_eq!(result, None);
    }
}

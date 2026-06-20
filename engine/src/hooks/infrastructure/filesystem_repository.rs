//! Filesystem implementation of `HookCommandRepository`.
//!
//! @canonical .pi/architecture/modules/hooks.md
//! Implements: HookCommandRepository — filesystem-backed hook config storage
//! Issue: #415
//!
//! Reads and writes hook configuration from TOML files on the filesystem.
//! Uses the standard `.rigorix/hooks.toml` path by default.

use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;

use crate::hooks::domain::config::HookConfig;
use crate::hooks::domain::error::HookError;

use super::repository::HookCommandRepository;

/// Default hook configuration file path relative to workspace root.
const DEFAULT_HOOKS_CONFIG_PATH: &str = ".rigorix/hooks.toml";

/// Filesystem-backed implementation of `HookCommandRepository`.
///
/// Reads hook configuration from TOML files. Supports loading from the
/// default path (`.rigorix/hooks.toml`) or any custom path.
///
/// # Format
///
/// ```toml
/// [hooks]
/// pre_tool_use = [
///     "rigorix-hook-validate-path",
///     "rigorix-hook-ci-guard",
/// ]
/// post_tool_use = [
///     "rigorix-hook-fmt-check",
/// ]
/// post_tool_use_failure = [
///     "rigorix-hook-notify",
/// ]
/// timeout_secs = 30
/// sequential_pre_tool_use = true
/// ```
pub struct FilesystemHookConfigRepository {
    /// Path to the hook configuration file.
    config_path: PathBuf,
}

impl FilesystemHookConfigRepository {
    /// Create a new repository with the default config path.
    pub fn new(workspace_root: &str) -> Self {
        Self {
            config_path: PathBuf::from(workspace_root).join(DEFAULT_HOOKS_CONFIG_PATH),
        }
    }

    /// Create a new repository with a custom config path.
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            config_path: path.into(),
        }
    }
}

#[async_trait]
impl HookCommandRepository for FilesystemHookConfigRepository {
    async fn load(&self) -> Result<HookConfig, HookError> {
        self.load_from(self.config_path.to_str().unwrap_or(""))
            .await
    }

    async fn load_from(&self, path: &str) -> Result<HookConfig, HookError> {
        let content = fs::read_to_string(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                // File doesn't exist — return empty config
                return HookError::CommandNotFound {
                    command: path.to_string(),
                };
            }
            HookError::Internal {
                detail: format!("Failed to read hook config from '{}': {}", path, e),
            }
        })?;

        // Try parsing as TOML, mapping to HookConfig's section
        #[derive(serde::Deserialize)]
        struct HooksToml {
            hooks: Option<HookConfig>,
        }

        match toml::from_str::<HooksToml>(&content) {
            Ok(toml_config) => Ok(toml_config.hooks.unwrap_or_default()),
            Err(e) => Err(HookError::InvalidJson {
                command: path.to_string(),
                detail: format!("TOML parse error: {}", e),
                raw_output: content.chars().take(500).collect(),
            }),
        }
    }

    async fn save(&self, config: &HookConfig) -> Result<(), HookError> {
        let toml_string = toml::to_string_pretty(config).map_err(|e| HookError::Internal {
            detail: format!("Failed to serialize hook config: {}", e),
        })?;

        let toml_content = format!("[hooks]\n{}", toml_string);

        // Ensure parent directory exists
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| HookError::Internal {
                detail: format!(
                    "Failed to create config directory '{}': {}",
                    parent.display(),
                    e
                ),
            })?;
        }

        fs::write(&self.config_path, &toml_content).map_err(|e| HookError::Internal {
            detail: format!(
                "Failed to write hook config to '{}': {}",
                self.config_path.display(),
                e
            ),
        })?;

        Ok(())
    }

    async fn exists(&self) -> Result<bool, HookError> {
        Ok(self.config_path.exists())
    }

    async fn reset(&self) -> Result<(), HookError> {
        if self.config_path.exists() {
            fs::remove_file(&self.config_path).map_err(|e| HookError::Internal {
                detail: format!(
                    "Failed to remove hook config '{}': {}",
                    self.config_path.display(),
                    e
                ),
            })?;
        }
        Ok(())
    }

    fn source_path(&self) -> &str {
        self.config_path.to_str().unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config() -> HookConfig {
        HookConfig {
            pre_tool_use: vec!["hook-a".into(), "hook-b".into()],
            post_tool_use: vec!["hook-c".into()],
            post_tool_use_failure: vec!["hook-d".into()],
            timeout_secs: 30,
            sequential_pre_tool_use: false,
        }
    }

    #[tokio::test]
    async fn test_load_non_existent_path() {
        let repo = FilesystemHookConfigRepository::with_path("/tmp/nonexistent/hooks.toml");
        // Should return CommandNotFound (file not found)
        let result = repo.load().await;
        assert!(result.is_err());
        match result {
            Err(HookError::CommandNotFound { .. }) => {} // Expected
            Err(HookError::Internal { detail }) => {
                // On some systems file read fails with different error
                assert!(detail.contains("nonexistent"));
            }
            _ => panic!("Expected CommandNotFound or Internal error"),
        }
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join(".rigorix").join("hooks.toml");
        let repo = FilesystemHookConfigRepository::with_path(&config_path);

        let config = test_config();
        repo.save(&config).await.unwrap();
        assert!(config_path.exists());

        let loaded = repo.load().await.unwrap();
        assert_eq!(loaded.pre_tool_use, config.pre_tool_use);
        assert_eq!(loaded.post_tool_use, config.post_tool_use);
        assert_eq!(loaded.post_tool_use_failure, config.post_tool_use_failure);
        assert_eq!(loaded.timeout_secs, config.timeout_secs);
    }

    #[tokio::test]
    async fn test_exists() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("hooks.toml");
        let repo = FilesystemHookConfigRepository::with_path(&config_path);

        assert!(!repo.exists().await.unwrap());
        repo.save(&test_config()).await.unwrap();
        assert!(repo.exists().await.unwrap());
    }

    #[tokio::test]
    async fn test_reset() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("hooks.toml");
        let repo = FilesystemHookConfigRepository::with_path(&config_path);

        repo.save(&test_config()).await.unwrap();
        assert!(config_path.exists());

        repo.reset().await.unwrap();
        assert!(!config_path.exists());
    }

    #[tokio::test]
    async fn test_source_path() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join(".rigorix").join("hooks.toml");
        let repo = FilesystemHookConfigRepository::with_path(&config_path);

        assert!(
            repo.source_path().ends_with(".rigorix/hooks.toml")
                || repo.source_path().contains("hooks.toml")
        );
    }

    #[tokio::test]
    async fn test_load_invalid_toml() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("bad.toml");
        fs::write(&config_path, "invalid toml {{{").unwrap();

        let repo = FilesystemHookConfigRepository::with_path(&config_path);
        let result = repo.load().await;
        assert!(result.is_err());
        match result {
            Err(HookError::InvalidJson { .. }) => {} // Expected
            _ => panic!("Expected InvalidJson error"),
        }
    }
}

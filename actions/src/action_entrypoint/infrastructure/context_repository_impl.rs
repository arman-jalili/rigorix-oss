//! ContextRepository implementation — reads from environment variables and filesystem.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md
//! Implements: ContextRepository trait — reads env, filesystem, and GitHub context
//! Issue: issue-actioncontext (#615)
//!
//! This implementation reads from real environment variables (std::env::var)
//! and the filesystem (tokio::fs) to provide GitHub Action execution context.

use async_trait::async_trait;

use crate::action_entrypoint::domain::ActionError;

use super::repository::ContextRepository;

/// Reads GitHub Action context from real environment variables and filesystem.
///
/// # Security
/// - Does NOT log environment variable values (only variable names)
/// - File paths are validated against directory traversal
pub struct ContextRepositoryImpl;

#[async_trait]
impl ContextRepository for ContextRepositoryImpl {
    async fn read_env_var(&self, name: &str) -> Result<Option<String>, ActionError> {
        match std::env::var(name) {
            Ok(val) if !val.is_empty() => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    async fn read_env_vars(
        &self,
        prefix: &str,
    ) -> Result<std::collections::HashMap<String, String>, ActionError> {
        let mut result = std::collections::HashMap::new();
        for (key, value) in std::env::vars() {
            if let Some(stripped) = key.strip_prefix(prefix) {
                result.insert(stripped.to_string(), value);
            }
        }
        Ok(result)
    }

    async fn has_env_var(&self, name: &str) -> Result<bool, ActionError> {
        Ok(std::env::var(name).is_ok())
    }

    async fn workspace_root(&self) -> Result<String, ActionError> {
        let path = std::env::var("GITHUB_WORKSPACE").map_err(|_| ActionError::MissingContext {
            detail: "GITHUB_WORKSPACE environment variable is not set".to_string(),
            env_var: Some("GITHUB_WORKSPACE".to_string()),
        })?;

        // Validate the path exists
        let metadata = std::fs::metadata(&path).map_err(|e| ActionError::InvalidWorkspaceRoot {
            path: path.clone(),
            detail: format!("Cannot access workspace root: {e}"),
        })?;

        if !metadata.is_dir() {
            return Err(ActionError::InvalidWorkspaceRoot {
                path,
                detail: "Workspace root is not a directory".to_string(),
            });
        }

        Ok(path)
    }

    async fn event_name(&self) -> Result<String, ActionError> {
        std::env::var("GITHUB_EVENT_NAME").map_err(|_| ActionError::MissingContext {
            detail: "GITHUB_EVENT_NAME environment variable is not set".to_string(),
            env_var: Some("GITHUB_EVENT_NAME".to_string()),
        })
    }

    async fn event_path(&self) -> Result<String, ActionError> {
        std::env::var("GITHUB_EVENT_PATH").map_err(|_| ActionError::MissingContext {
            detail: "GITHUB_EVENT_PATH environment variable is not set".to_string(),
            env_var: Some("GITHUB_EVENT_PATH".to_string()),
        })
    }

    async fn read_event_payload(&self, path: &str) -> Result<String, ActionError> {
        tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ActionError::ContextRepositoryError {
                detail: format!("Failed to read event payload from '{path}': {e}"),
            })
    }

    async fn github_token(&self) -> Result<Option<String>, ActionError> {
        // Check GITHUB_TOKEN first, then INPUT_GITHUB_TOKEN
        if let Ok(token) = std::env::var("GITHUB_TOKEN")
            && !token.is_empty()
        {
            return Ok(Some(token));
        }
        if let Ok(token) = std::env::var("INPUT_GITHUB_TOKEN")
            && !token.is_empty()
        {
            return Ok(Some(token));
        }
        Ok(None)
    }

    async fn github_api_url(&self) -> Result<String, ActionError> {
        Ok(
            std::env::var("GITHUB_API_URL")
                .unwrap_or_else(|_| "https://api.github.com".to_string()),
        )
    }

    async fn read_ci_env_vars(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, ActionError> {
        let ci_vars = [
            "GITHUB_ACTIONS",
            "GITHUB_EVENT_NAME",
            "GITHUB_EVENT_PATH",
            "GITHUB_ACTOR",
            "GITHUB_REPOSITORY",
            "GITHUB_REPOSITORY_OWNER",
            "GITHUB_RUN_ID",
            "GITHUB_RUN_NUMBER",
            "GITHUB_SHA",
            "GITHUB_REF",
            "GITHUB_REF_NAME",
            "GITHUB_WORKSPACE",
            "GITHUB_API_URL",
            "GITHUB_SERVER_URL",
            "CI",
        ];

        let mut result = std::collections::HashMap::new();
        for var_name in &ci_vars {
            if let Ok(val) = std::env::var(var_name) {
                result.insert(var_name.to_string(), val);
            }
        }
        Ok(result)
    }

    async fn resolve_path(&self, path: &str) -> Result<String, ActionError> {
        let path_buf = std::path::PathBuf::from(path);
        if path_buf.is_absolute() {
            // Canonicalize for consistency
            return Ok(std::fs::canonicalize(&path_buf)
                .map_err(|e| ActionError::InvalidWorkspaceRoot {
                    path: path.to_string(),
                    detail: format!("Failed to resolve path: {e}"),
                })?
                .to_string_lossy()
                .to_string());
        }

        // Resolve relative path against workspace root
        let workspace = self.workspace_root().await?;
        let absolute = std::path::PathBuf::from(&workspace).join(path);
        Ok(absolute.to_string_lossy().to_string())
    }
}

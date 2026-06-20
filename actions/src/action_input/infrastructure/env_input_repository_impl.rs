//! Environment-based implementation of `InputRepository`.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md#parser
//! Implements: InputRepository trait — reads from environment variables
//! Issue: #522
//!
//! Reads environment variables using `std::env::var`. Supports prefix-based
//! filtering for `INPUT_*` style variables and CI-specific variables.

use async_trait::async_trait;
use std::collections::HashMap;

use crate::action_input::domain::ActionInputError;
use crate::action_input::infrastructure::repository::InputRepository;

/// Implementation of `InputRepository` that reads from real environment variables.
///
/// All reads are synchronous (`std::env::var`) but wrapped in async for
/// trait compatibility. This is acceptable because env var access is
/// instantaneous and non-blocking.
///
/// # Security
/// - Env var values are never logged or exposed in error messages
/// - Variable names (keys) are safe for logging
pub struct EnvInputRepository;

impl EnvInputRepository {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EnvInputRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputRepository for EnvInputRepository {
    async fn read_env_var(&self, name: &str) -> Result<Option<String>, ActionInputError> {
        Ok(std::env::var(name).ok().filter(|v| !v.is_empty()))
    }

    async fn read_env_vars(
        &self,
        prefix: &str,
    ) -> Result<HashMap<String, String>, ActionInputError> {
        let prefix_upper = prefix.to_uppercase();
        Ok(std::env::vars()
            .filter(|(key, value)| key.starts_with(&prefix_upper) && !value.is_empty())
            .map(|(key, value)| {
                let stripped = key[prefix_upper.len()..].to_string();
                (stripped, value)
            })
            .collect())
    }

    async fn has_env_var(&self, name: &str) -> Result<bool, ActionInputError> {
        Ok(std::env::var(name).is_ok())
    }

    async fn workspace_root(&self) -> Result<String, ActionInputError> {
        std::env::var("GITHUB_WORKSPACE").or_else(|_| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .map_err(|e| ActionInputError::EnvironmentError {
                    detail: format!("Cannot determine workspace root: {}", e),
                })
        })
    }

    async fn read_ci_env_vars(&self) -> Result<HashMap<String, String>, ActionInputError> {
        let ci_vars = [
            "GITHUB_ACTIONS",
            "GITHUB_EVENT_NAME",
            "GITHUB_EVENT_PATH",
            "GITHUB_ACTOR",
            "GITHUB_WORKSPACE",
            "GITHUB_SHA",
            "GITHUB_REF",
            "GITHUB_REPOSITORY",
            "GITHUB_RUN_ID",
            "CI",
        ];
        let mut map = HashMap::new();
        for var in ci_vars {
            if let Ok(value) = std::env::var(var) {
                map.insert(var.to_string(), value);
            }
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_env(key: &str, value: &str) {
        // SAFETY: test-only environment manipulation — no concurrent access
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn remove_env(key: &str) {
        // SAFETY: test-only environment manipulation — no concurrent access
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[tokio::test]
    async fn test_read_existing_env_var() {
        let var = "RIGORIX_TEST_READ_EXISTING";
        set_env(var, "test_value");
        let repo = EnvInputRepository::new();
        let result = repo.read_env_var(var).await.unwrap();
        assert_eq!(result, Some("test_value".to_string()));
        remove_env(var);
    }

    #[tokio::test]
    async fn test_read_missing_env_var() {
        let var = "RIGORIX_TEST_READ_MISSING";
        remove_env(var);
        let repo = EnvInputRepository::new();
        let result = repo.read_env_var(var).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_read_empty_env_var() {
        let var = "RIGORIX_TEST_READ_EMPTY";
        set_env(var, "");
        let repo = EnvInputRepository::new();
        let result = repo.read_env_var(var).await.unwrap();
        assert_eq!(result, None);
        remove_env(var);
    }

    #[tokio::test]
    async fn test_read_env_vars_with_prefix() {
        set_env("RIGORIX_TEST_PREFIX_INTENT", "fix bugs");
        set_env("RIGORIX_TEST_PREFIX_MODE", "run");
        set_env("RIGORIX_TEST_PREFIX_MAX_LLM_CALLS", "10");
        set_env("OTHER_UNIQUE_VAR", "should_not_appear");

        let repo = EnvInputRepository::new();
        let result = repo.read_env_vars("RIGORIX_TEST_PREFIX_").await.unwrap();

        assert_eq!(result.get("INTENT"), Some(&"fix bugs".to_string()));
        assert_eq!(result.get("MODE"), Some(&"run".to_string()));
        assert_eq!(result.get("MAX_LLM_CALLS"), Some(&"10".to_string()));
        assert_eq!(result.get("OTHER_UNIQUE_VAR"), None);

        remove_env("RIGORIX_TEST_PREFIX_INTENT");
        remove_env("RIGORIX_TEST_PREFIX_MODE");
        remove_env("RIGORIX_TEST_PREFIX_MAX_LLM_CALLS");
        remove_env("OTHER_UNIQUE_VAR");
    }

    #[tokio::test]
    async fn test_has_env_var() {
        let var = "RIGORIX_TEST_HAS_VAR";
        set_env(var, "true");
        let repo = EnvInputRepository::new();
        assert!(repo.has_env_var(var).await.unwrap());
        remove_env(var);
        assert!(
            !repo
                .has_env_var("RIGORIX_TEST_NONEXISTENT_HAS")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_workspace_root_fallback_to_cwd() {
        remove_env("GITHUB_WORKSPACE");
        let repo = EnvInputRepository::new();
        let result = repo.workspace_root().await.unwrap();
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn test_read_ci_env_vars() {
        set_env("RIGORIX_TEST_CI_VAR", "true");
        set_env("RIGORIX_TEST_GHA_VAR", "true");

        let repo = EnvInputRepository::new();
        // CI vars only read specific names, so our custom vars won't appear
        let result = repo.read_ci_env_vars().await.unwrap();

        // These won't match because CI vars list is hardcoded, but we verify it doesn't error
        assert!(result.is_empty() || result.contains_key("CI"));

        remove_env("RIGORIX_TEST_CI_VAR");
        remove_env("RIGORIX_TEST_GHA_VAR");
    }

    #[tokio::test]
    async fn test_read_env_vars_empty_prefix() {
        let var = "RIGORIX_TEST_EMPTY_PREFIX_VAR";
        set_env(var, "value");
        let repo = EnvInputRepository::new();
        let result = repo
            .read_env_vars("RIGORIX_NONEXISTENT_PREFIX_")
            .await
            .unwrap();
        assert!(result.is_empty());
        remove_env(var);
    }
}

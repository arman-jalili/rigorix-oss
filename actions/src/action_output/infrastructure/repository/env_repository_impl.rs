//! Implementation of `EnvRepository`.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md#envrepository
//! Implements: EnvRepository — reads environment variables relevant to output formatting
//! Issue: issue-outputformatter
//!
//! # Contract
//! - Implements `EnvRepository` trait from the frozen contract
//! - Reads real `std::env` variables
//! - Variable names (keys) are safe for logging, values are never logged

use async_trait::async_trait;
use std::collections::HashMap;

use crate::action_output::domain::ActionOutputError;

/// Environment-variable-based implementation of `EnvRepository`.
///
/// Reads environment variables like `GITHUB_STEP_SUMMARY`, `GITHUB_OUTPUT`,
/// `GITHUB_TOKEN`, `GITHUB_REPOSITORY`, etc.
///
/// # Security
/// - Only variable names are exposed — values are returned but not logged
/// - Callers are responsible for not logging values
pub struct EnvRepositoryImpl;

impl Default for EnvRepositoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvRepositoryImpl {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl crate::action_output::infrastructure::repository::EnvRepository for EnvRepositoryImpl {
    async fn read_env_var(&self, name: &str) -> Result<Option<String>, ActionOutputError> {
        Ok(std::env::var(name).ok())
    }

    async fn read_step_summary_path(&self) -> Result<Option<String>, ActionOutputError> {
        Ok(std::env::var("GITHUB_STEP_SUMMARY").ok())
    }

    async fn read_output_path(&self) -> Result<Option<String>, ActionOutputError> {
        Ok(std::env::var("GITHUB_OUTPUT").ok())
    }

    async fn read_github_token(&self) -> Result<Option<String>, ActionOutputError> {
        // Check INPUT_GITHUB_TOKEN first (passed via action inputs), then GITHUB_TOKEN
        Ok(std::env::var("INPUT_GITHUB_TOKEN")
            .ok()
            .or_else(|| std::env::var("GITHUB_TOKEN").ok()))
    }

    async fn read_repository(&self) -> Result<Option<String>, ActionOutputError> {
        Ok(std::env::var("GITHUB_REPOSITORY").ok())
    }

    async fn read_ci_context(&self) -> Result<HashMap<String, String>, ActionOutputError> {
        let mut ctx = HashMap::new();
        for var in &[
            "GITHUB_ACTIONS",
            "GITHUB_EVENT_NAME",
            "GITHUB_REPOSITORY",
            "GITHUB_ACTOR",
            "GITHUB_WORKSPACE",
            "GITHUB_SHA",
            "GITHUB_REF_NAME",
            "GITHUB_RUN_ID",
            "GITHUB_RUN_NUMBER",
            "GITHUB_STEP_SUMMARY",
            "GITHUB_OUTPUT",
            "CI",
        ] {
            if let Ok(val) = std::env::var(var) {
                ctx.insert(var.to_string(), val);
            }
        }
        Ok(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_output::infrastructure::repository::EnvRepository;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[tokio::test]
    async fn test_read_env_var_present() {
        let _lock = ENV_LOCK.lock().unwrap();
        // SAFETY: test-only env manipulation
        unsafe { std::env::set_var("RIGORIX_TEST_VAR", "test-value") };
        let repo = EnvRepositoryImpl::new();
        let result = repo.read_env_var("RIGORIX_TEST_VAR").await.unwrap();
        assert_eq!(result, Some("test-value".to_string()));
        unsafe { std::env::remove_var("RIGORIX_TEST_VAR") };
    }

    #[tokio::test]
    async fn test_read_env_var_missing() {
        // SAFETY: test-only env manipulation
        unsafe { std::env::remove_var("RIGORIX_TEST_MISSING") };
        let repo = EnvRepositoryImpl::new();
        let result = repo.read_env_var("RIGORIX_TEST_MISSING").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_read_step_summary_path() {
        let _lock = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("GITHUB_STEP_SUMMARY", "/tmp/summary.md") };
        let repo = EnvRepositoryImpl::new();
        let result = repo.read_step_summary_path().await.unwrap();
        assert_eq!(result, Some("/tmp/summary.md".to_string()));
        unsafe { std::env::remove_var("GITHUB_STEP_SUMMARY") };
    }

    #[tokio::test]
    async fn test_read_github_token_prefers_input() {
        let _lock = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("INPUT_GITHUB_TOKEN", "input-token");
            std::env::set_var("GITHUB_TOKEN", "env-token");
        }
        let repo = EnvRepositoryImpl::new();
        let result = repo.read_github_token().await.unwrap();
        assert_eq!(result, Some("input-token".to_string()));
        unsafe {
            std::env::remove_var("INPUT_GITHUB_TOKEN");
            std::env::remove_var("GITHUB_TOKEN");
        }
    }

    #[tokio::test]
    async fn test_read_ci_context() {
        let _lock = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("GITHUB_ACTIONS", "true") };
        let repo = EnvRepositoryImpl::new();
        let ctx = repo.read_ci_context().await.unwrap();
        assert_eq!(ctx.get("GITHUB_ACTIONS"), Some(&"true".to_string()));
        unsafe { std::env::remove_var("GITHUB_ACTIONS") };
    }
}

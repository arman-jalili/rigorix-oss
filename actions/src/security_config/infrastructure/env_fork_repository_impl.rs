//! Environment-based implementation of `ForkRepository`.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md#fork
//! Implements: ForkRepository trait — reads GitHub env vars for fork detection
//! Issue: #539
//!
//! Reads fork detection information from GitHub-provided environment variables:
//! - `GITHUB_REPOSITORY` — base repo (e.g., "org/repo")
//! - `GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME` — head repo (PR only)
//! - `GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_OWNER` — fork owner (forks only)
//! - `GITHUB_EVENT_NAME` — event type
//! - `GITHUB_EVENT_PULL_REQUEST_NUMBER` — PR number

use async_trait::async_trait;

use crate::security_config::domain::SecurityError;
use crate::security_config::infrastructure::repository::ForkRepository;

/// Implementation of `ForkRepository` that reads from real environment variables.
///
/// All reads are synchronous (`std::env::var`) but wrapped in async for
/// trait compatibility. This is acceptable because env var access is
/// instantaneous and non-blocking.
pub struct EnvForkRepository;

impl EnvForkRepository {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EnvForkRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ForkRepository for EnvForkRepository {
    async fn base_repo(&self) -> Result<String, SecurityError> {
        std::env::var("GITHUB_REPOSITORY").map_err(|_| SecurityError::Internal {
            detail: "GITHUB_REPOSITORY not set".to_string(),
        })
    }

    async fn head_repo(&self) -> Result<Option<String>, SecurityError> {
        Ok(std::env::var("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME")
            .ok()
            .filter(|v| !v.is_empty()))
    }

    async fn head_repo_owner(&self) -> Result<Option<String>, SecurityError> {
        Ok(std::env::var("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_OWNER")
            .ok()
            .filter(|v| !v.is_empty()))
    }

    async fn event_name(&self) -> Result<Option<String>, SecurityError> {
        Ok(std::env::var("GITHUB_EVENT_NAME").ok().filter(|v| !v.is_empty()))
    }

    async fn pr_number(&self) -> Result<Option<u64>, SecurityError> {
        Ok(std::env::var("GITHUB_EVENT_PULL_REQUEST_NUMBER")
            .ok()
            .and_then(|v| v.parse().ok()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn set_env(key: &str, value: &str) {
        std::env::set_var(key, value);
    }

    unsafe fn remove_env(key: &str) {
        std::env::remove_var(key);
    }

    #[tokio::test]
    async fn test_base_repo() {
        unsafe { set_env("GITHUB_REPOSITORY", "org/test-repo"); }
        let repo = EnvForkRepository::new();
        let result = repo.base_repo().await.unwrap();
        assert_eq!(result, "org/test-repo");
        unsafe { remove_env("GITHUB_REPOSITORY"); }
    }

    #[tokio::test]
    async fn test_base_repo_missing() {
        unsafe { remove_env("GITHUB_REPOSITORY"); }
        let repo = EnvForkRepository::new();
        assert!(repo.base_repo().await.is_err());
    }

    #[tokio::test]
    async fn test_head_repo_present() {
        unsafe {
            set_env("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME", "fork-user/test-repo");
        }
        let repo = EnvForkRepository::new();
        let result = repo.head_repo().await.unwrap();
        assert_eq!(result, Some("fork-user/test-repo".to_string()));
        unsafe { remove_env("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME"); }
    }

    #[tokio::test]
    async fn test_head_repo_absent() {
        unsafe { remove_env("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME"); }
        let repo = EnvForkRepository::new();
        assert_eq!(repo.head_repo().await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_head_repo_owner() {
        unsafe { set_env("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_OWNER", "fork-user"); }
        let repo = EnvForkRepository::new();
        let result = repo.head_repo_owner().await.unwrap();
        assert_eq!(result, Some("fork-user".to_string()));
        unsafe { remove_env("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_OWNER"); }
    }

    #[tokio::test]
    async fn test_event_name() {
        unsafe { set_env("GITHUB_EVENT_NAME", "pull_request"); }
        let repo = EnvForkRepository::new();
        let result = repo.event_name().await.unwrap();
        assert_eq!(result, Some("pull_request".to_string()));
        unsafe { remove_env("GITHUB_EVENT_NAME"); }
    }

    #[tokio::test]
    async fn test_pr_number() {
        unsafe { set_env("GITHUB_EVENT_PULL_REQUEST_NUMBER", "42"); }
        let repo = EnvForkRepository::new();
        let result = repo.pr_number().await.unwrap();
        assert_eq!(result, Some(42));
        unsafe { remove_env("GITHUB_EVENT_PULL_REQUEST_NUMBER"); }
    }

    #[tokio::test]
    async fn test_pr_number_absent() {
        unsafe { remove_env("GITHUB_EVENT_PULL_REQUEST_NUMBER"); }
        let repo = EnvForkRepository::new();
        assert_eq!(repo.pr_number().await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_head_repo_empty() {
        unsafe { set_env("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME", ""); }
        let repo = EnvForkRepository::new();
        assert_eq!(repo.head_repo().await.unwrap(), None);
        unsafe { remove_env("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME"); }
    }
}

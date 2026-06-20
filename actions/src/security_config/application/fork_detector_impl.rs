//! Implementation of `ForkDetectionService`.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md#fork
//! Implements: ForkDetectionService trait — detects fork PRs via GitHub event context
//! Issue: #539
//!
//! The ForkDetector detects whether a PR originates from a forked repository
//! by comparing the head repository against the base repository.
//!
//! # Detection Logic
//!
//! CORRECT: Compare `GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME` against
//! `GITHUB_REPOSITORY`.
//!
//! WRONG: Comparing against `GITHUB_REPOSITORY_OWNER` — that only gives
//! the org name, which would false-positive all internal PRs.
//!
//! # Security Implications
//!
//! Fork PRs cannot access repository secrets. The GITHUB_TOKEN is
//! read-only for fork PRs. We detect this explicitly to fail gracefully
//! rather than with cryptic auth errors.

use async_trait::async_trait;
use std::collections::HashMap;

use crate::security_config::application::dto::{DetectForkInput, DetectForkOutput};
use crate::security_config::application::service::ForkDetectionService;
use crate::security_config::domain::SecurityError;
use crate::security_config::infrastructure::repository::ForkRepository;

/// Implementation of `ForkDetectionService`.
///
/// Detects fork PRs by comparing head repo against base repo.
/// Uses `ForkRepository` for testability — production uses `EnvForkRepository`.
pub struct ForkDetectorImpl {
    repository: Box<dyn ForkRepository>,
}

impl ForkDetectorImpl {
    pub fn new(repository: Box<dyn ForkRepository>) -> Self {
        Self { repository }
    }
}

impl Default for ForkDetectorImpl {
    fn default() -> Self {
        Self::new(Box::new(
            crate::security_config::infrastructure::env_fork_repository_impl::EnvForkRepository::new(),
        ))
    }
}

#[async_trait]
impl ForkDetectionService for ForkDetectorImpl {
    async fn detect(&self, input: DetectForkInput) -> Result<DetectForkOutput, SecurityError> {
        // If env_override is provided, use it; otherwise read from real env
        let (base_repo, head_repo, head_owner) = if let Some(ref override_map) = input.env_override
        {
            Self::detect_from_map(override_map)
        } else {
            self.detect_from_env().await?
        };

        let is_fork = match (&head_repo, &base_repo) {
            (Some(head), base) => head != base,
            (None, _) => false, // Not a PR event — not a fork
        };

        Ok(DetectForkOutput {
            is_fork,
            head_repo,
            base_repo: Some(base_repo),
            fork_owner: if is_fork { head_owner } else { None },
        })
    }

    async fn fork_owner(&self) -> Result<Option<String>, SecurityError> {
        self.repository.head_repo_owner().await
    }

    async fn head_repo(&self) -> Result<Option<String>, SecurityError> {
        self.repository.head_repo().await
    }

    async fn base_repo(&self) -> Result<String, SecurityError> {
        self.repository.base_repo().await
    }
}

// ── Private helpers ──

impl ForkDetectorImpl {
    /// Detect fork from an env override map (for testing).
    fn detect_from_map(env: &HashMap<String, String>) -> (String, Option<String>, Option<String>) {
        let base_repo = env.get("GITHUB_REPOSITORY").cloned().unwrap_or_default();
        let head_repo = env
            .get("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME")
            .filter(|v| !v.is_empty())
            .cloned();
        let head_owner = env
            .get("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_OWNER")
            .filter(|v| !v.is_empty())
            .cloned();
        (base_repo, head_repo, head_owner)
    }

    /// Detect fork from real environment variables.
    async fn detect_from_env(
        &self,
    ) -> Result<(String, Option<String>, Option<String>), SecurityError> {
        let base_repo = self.repository.base_repo().await?;
        let head_repo = self.repository.head_repo().await?;
        let head_owner = self.repository.head_repo_owner().await?;
        Ok((base_repo, head_repo, head_owner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security_config::infrastructure::repository::ForkRepository;
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct MockForkRepository {
        env: Mutex<HashMap<String, String>>,
    }

    impl MockForkRepository {
        fn new(vars: HashMap<String, String>) -> Self {
            Self {
                env: Mutex::new(vars),
            }
        }
    }

    #[async_trait]
    impl ForkRepository for MockForkRepository {
        async fn base_repo(&self) -> Result<String, SecurityError> {
            let env = self.env.lock().unwrap();
            Ok(env.get("GITHUB_REPOSITORY").cloned().unwrap_or_default())
        }

        async fn head_repo(&self) -> Result<Option<String>, SecurityError> {
            let env = self.env.lock().unwrap();
            Ok(env
                .get("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME")
                .filter(|v| !v.is_empty())
                .cloned())
        }

        async fn head_repo_owner(&self) -> Result<Option<String>, SecurityError> {
            let env = self.env.lock().unwrap();
            Ok(env
                .get("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_OWNER")
                .filter(|v| !v.is_empty())
                .cloned())
        }

        async fn event_name(&self) -> Result<Option<String>, SecurityError> {
            let env = self.env.lock().unwrap();
            Ok(env.get("GITHUB_EVENT_NAME").cloned())
        }

        async fn pr_number(&self) -> Result<Option<u64>, SecurityError> {
            let env = self.env.lock().unwrap();
            Ok(env
                .get("GITHUB_EVENT_PULL_REQUEST_NUMBER")
                .and_then(|v| v.parse().ok()))
        }
    }

    fn make_detector(env: HashMap<String, String>) -> ForkDetectorImpl {
        ForkDetectorImpl::new(Box::new(MockForkRepository::new(env)))
    }

    fn make_env(base: &str, head: Option<&str>, owner: Option<&str>) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("GITHUB_REPOSITORY".to_string(), base.to_string());
        if let Some(h) = head {
            map.insert(
                "GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME".to_string(),
                h.to_string(),
            );
        }
        if let Some(o) = owner {
            map.insert(
                "GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_OWNER".to_string(),
                o.to_string(),
            );
        }
        map
    }

    // ── Tests using env_override ──

    #[tokio::test]
    async fn test_detect_fork_pr() {
        let env = make_env(
            "org/main-repo",
            Some("fork-user/main-repo"),
            Some("fork-user"),
        );
        let detector = make_detector(env);
        let input = DetectForkInput { env_override: None };
        let result = detector.detect(input).await.unwrap();

        assert!(result.is_fork);
        assert_eq!(result.head_repo, Some("fork-user/main-repo".to_string()));
        assert_eq!(result.base_repo, Some("org/main-repo".to_string()));
        assert_eq!(result.fork_owner, Some("fork-user".to_string()));
    }

    #[tokio::test]
    async fn test_detect_non_fork_pr() {
        let env = make_env("org/main-repo", Some("org/main-repo"), None);
        let detector = make_detector(env);
        let input = DetectForkInput { env_override: None };
        let result = detector.detect(input).await.unwrap();

        assert!(!result.is_fork);
        assert_eq!(result.head_repo, Some("org/main-repo".to_string()));
        assert_eq!(result.fork_owner, None);
    }

    #[tokio::test]
    async fn test_detect_non_pr_event() {
        let env = make_env("org/main-repo", None, None);
        let detector = make_detector(env);
        let input = DetectForkInput { env_override: None };
        let result = detector.detect(input).await.unwrap();

        assert!(!result.is_fork);
        assert_eq!(result.head_repo, None);
        assert_eq!(result.fork_owner, None);
    }

    #[tokio::test]
    async fn test_detect_with_env_override() {
        let mut env = HashMap::new();
        env.insert("GITHUB_REPOSITORY".to_string(), "org/main".to_string());
        env.insert(
            "GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME".to_string(),
            "fork/main".to_string(),
        );
        env.insert(
            "GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_OWNER".to_string(),
            "fork".to_string(),
        );

        let mut detector_env = HashMap::new();
        detector_env.insert("GITHUB_REPOSITORY".to_string(), "org/main".to_string());
        let detector = make_detector(detector_env); // Repository returns same repo
        let input = DetectForkInput {
            env_override: Some(env),
        };
        let result = detector.detect(input).await.unwrap();

        assert!(result.is_fork);
        assert_eq!(result.head_repo, Some("fork/main".to_string()));
        assert_eq!(result.fork_owner, Some("fork".to_string()));
    }

    #[tokio::test]
    async fn test_fork_owner() {
        let env = make_env("org/main", None, Some("fork-user"));
        let detector = make_detector(env);
        let result = detector.fork_owner().await.unwrap();
        assert_eq!(result, Some("fork-user".to_string()));
    }

    #[tokio::test]
    async fn test_fork_owner_none() {
        let env = HashMap::new();
        let detector = make_detector(env);
        let result = detector.fork_owner().await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_head_repo() {
        let env = make_env("org/main", Some("fork/main"), None);
        let detector = make_detector(env);
        let result = detector.head_repo().await.unwrap();
        assert_eq!(result, Some("fork/main".to_string()));
    }

    #[tokio::test]
    async fn test_base_repo() {
        let env = make_env("org/main", None, None);
        let detector = make_detector(env);
        let result = detector.base_repo().await.unwrap();
        assert_eq!(result, "org/main".to_string());
    }

    #[tokio::test]
    async fn test_detect_case_sensitive_comparison() {
        // GitHub repo names are case-insensitive, but env var values should match
        let env = make_env("Org/Main-Repo", Some("org/main-repo"), None);
        let detector = make_detector(env);
        let input = DetectForkInput { env_override: None };
        let result = detector.detect(input).await.unwrap();
        // Case differs — treated as fork (GitHub normalizes to lowercase in practice)
        assert!(result.is_fork);
    }

    #[tokio::test]
    async fn test_detect_empty_head_repo() {
        let mut env = HashMap::new();
        env.insert("GITHUB_REPOSITORY".to_string(), "org/main".to_string());
        env.insert(
            "GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME".to_string(),
            "".to_string(),
        );
        let detector = make_detector(env);
        let input = DetectForkInput { env_override: None };
        let result = detector.detect(input).await.unwrap();
        assert!(!result.is_fork);
        assert_eq!(result.head_repo, None);
    }
}

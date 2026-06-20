//! Implementation of `CiDetectionService`.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md#ci-detector
//! Implements: CiDetectionService trait — detects CI environment and sets permissions
//! Issue: #525
//!
//! Detects the runtime environment by checking for CI-specific environment
//! variables. In GitHub Actions, defaults to `workspace_write` permission mode.
//! Locally, defaults to `prompt` for interactive confirmation.
//!
//! # Detection Logic
//!
//! | Check | CI | Local |
//! |-------|----|-------|
//! | `GITHUB_ACTIONS` set | ✅ GitHubActions | ❌ |
//! | Permission mode | workspace_write | prompt |
//! | Workspace | GITHUB_WORKSPACE | CWD |

use async_trait::async_trait;
use std::collections::HashMap;

use crate::action_input::application::dto::{DetectCiInput, DetectCiOutput};
use crate::action_input::application::service::CiDetectionService;
use crate::action_input::domain::{ActionInputError, CiEnvironment};

/// Implementation of `CiDetectionService`.
///
/// Checks for `GITHUB_ACTIONS` env var to detect GitHub Actions CI.
/// Falls back to `CiEnvironment::Local` when not in CI.
pub struct CiDetectorImpl;

impl CiDetectorImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CiDetectorImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CiDetectionService for CiDetectorImpl {
    async fn detect(&self, input: DetectCiInput) -> Result<DetectCiOutput, ActionInputError> {
        let is_ci = if let Some(ref env_override) = input.env_override {
            env_override.contains_key("GITHUB_ACTIONS")
        } else {
            std::env::var("GITHUB_ACTIONS").is_ok()
        };

        let (environment, default_mode) = if is_ci {
            let workspace = if let Some(ref env_override) = input.env_override {
                env_override
                    .get("GITHUB_WORKSPACE")
                    .cloned()
                    .unwrap_or_default()
            } else {
                std::env::var("GITHUB_WORKSPACE").unwrap_or_default()
            };

            let event_name = if let Some(ref env_override) = input.env_override {
                env_override
                    .get("GITHUB_EVENT_NAME")
                    .cloned()
                    .unwrap_or_default()
            } else {
                std::env::var("GITHUB_EVENT_NAME").unwrap_or_default()
            };

            let actor = if let Some(ref env_override) = input.env_override {
                env_override
                    .get("GITHUB_ACTOR")
                    .cloned()
                    .unwrap_or_default()
            } else {
                std::env::var("GITHUB_ACTOR").unwrap_or_default()
            };

            (
                CiEnvironment::GitHubActions {
                    workspace,
                    event_name,
                    actor,
                },
                "workspace_write".to_string(),
            )
        } else {
            (CiEnvironment::Local, "prompt".to_string())
        };

        let permission_mode = input.permission_mode_override.unwrap_or(default_mode);

        Ok(DetectCiOutput {
            environment,
            permission_mode,
            is_ci,
        })
    }

    async fn default_permission_mode(&self) -> String {
        if std::env::var("GITHUB_ACTIONS").is_ok() {
            "workspace_write".to_string()
        } else {
            "prompt".to_string()
        }
    }

    async fn is_ci(&self) -> bool {
        std::env::var("GITHUB_ACTIONS").is_ok()
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_ci_with_override() {
        let detector = CiDetectorImpl::new();
        let mut env = HashMap::new();
        env.insert("GITHUB_ACTIONS".to_string(), "true".to_string());
        env.insert(
            "GITHUB_WORKSPACE".to_string(),
            "/home/runner/work/test".to_string(),
        );
        env.insert("GITHUB_EVENT_NAME".to_string(), "pull_request".to_string());
        env.insert("GITHUB_ACTOR".to_string(), "test-bot".to_string());

        let input = DetectCiInput {
            env_override: Some(env),
            permission_mode_override: None,
        };

        let result = detector.detect(input).await.unwrap();

        assert!(result.is_ci);
        assert_eq!(result.permission_mode, "workspace_write");
        match result.environment {
            CiEnvironment::GitHubActions {
                workspace,
                event_name,
                actor,
            } => {
                assert_eq!(workspace, "/home/runner/work/test");
                assert_eq!(event_name, "pull_request");
                assert_eq!(actor, "test-bot");
            }
            CiEnvironment::Local => panic!("Expected GitHubActions"),
        }
    }

    #[tokio::test]
    async fn test_detect_local_with_override() {
        let detector = CiDetectorImpl::new();
        let env = HashMap::new(); // No GITHUB_ACTIONS

        let input = DetectCiInput {
            env_override: Some(env),
            permission_mode_override: None,
        };

        let result = detector.detect(input).await.unwrap();

        assert!(!result.is_ci);
        assert_eq!(result.permission_mode, "prompt");
        assert_eq!(result.environment, CiEnvironment::Local);
    }

    #[tokio::test]
    async fn test_detect_permission_mode_override() {
        let detector = CiDetectorImpl::new();
        let mut env = HashMap::new();
        env.insert("GITHUB_ACTIONS".to_string(), "true".to_string());

        let input = DetectCiInput {
            env_override: Some(env),
            permission_mode_override: Some("read_only".to_string()),
        };

        let result = detector.detect(input).await.unwrap();

        assert!(result.is_ci);
        assert_eq!(result.permission_mode, "read_only");
    }

    #[tokio::test]
    async fn test_default_permission_mode_no_ci() {
        let detector = CiDetectorImpl::new();
        let result = detector
            .detect(DetectCiInput {
                env_override: Some(HashMap::new()),
                permission_mode_override: None,
            })
            .await
            .unwrap();
        assert_eq!(result.permission_mode, "prompt");
    }

    #[tokio::test]
    async fn test_is_ci_detected_via_override() {
        let detector = CiDetectorImpl::new();
        let mut env = HashMap::new();
        env.insert("GITHUB_ACTIONS".to_string(), "true".to_string());
        let result = detector
            .detect(DetectCiInput {
                env_override: Some(env),
                permission_mode_override: None,
            })
            .await
            .unwrap();
        assert!(result.is_ci);
    }

    #[tokio::test]
    async fn test_is_ci_not_detected_via_override() {
        let detector = CiDetectorImpl::new();
        let result = detector
            .detect(DetectCiInput {
                env_override: Some(HashMap::new()),
                permission_mode_override: None,
            })
            .await
            .unwrap();
        assert!(!result.is_ci);
    }

    #[tokio::test]
    async fn test_ci_env_with_partial_info() {
        let detector = CiDetectorImpl::new();
        let mut env = HashMap::new();
        env.insert("GITHUB_ACTIONS".to_string(), "true".to_string());
        // No workspace, event_name, or actor

        let input = DetectCiInput {
            env_override: Some(env),
            permission_mode_override: None,
        };

        let result = detector.detect(input).await.unwrap();

        assert!(result.is_ci);
        match result.environment {
            CiEnvironment::GitHubActions {
                workspace,
                event_name,
                actor,
            } => {
                assert_eq!(workspace, "");
                assert_eq!(event_name, "");
                assert_eq!(actor, "");
            }
            CiEnvironment::Local => panic!("Expected GitHubActions"),
        }
    }

    #[tokio::test]
    async fn test_workspace_root_fallback() {
        // SAFETY: test-only env manipulation
        unsafe {
            std::env::remove_var("GITHUB_WORKSPACE");
        }
        let detector = CiDetectorImpl::new();
        let result = detector.workspace_root().await.unwrap();
        assert!(!result.is_empty());
    }
}

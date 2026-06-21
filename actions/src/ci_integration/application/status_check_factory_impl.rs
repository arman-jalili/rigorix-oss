//! Implementation of `StatusCheckFactory`.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md#status-check
//! Implements: StatusCheckFactory trait — builds GitHubStatus payloads for status checks
//! Issue: issue-statuscheckmanager
//!
//! Handles context naming conventions (rigorix/execution, rigorix/validation),
//! target URL generation linking to execution runs, and state-to-description
//! mapping logic.

use async_trait::async_trait;

use crate::ci_integration::application::factory::StatusCheckFactory;
use crate::ci_integration::domain::{
    CiIntegrationError, GitHubStatus, StatusCheckState, status_context,
};

/// Implementation of `StatusCheckFactory`.
///
/// Builds `GitHubStatus` payloads with proper context naming, target URLs,
/// and human-readable descriptions based on status check state.
pub struct StatusCheckFactoryImpl {
    /// Base URL for execution detail links (e.g., GitHub Actions run URL).
    /// In production, this is derived from GitHub Actions environment.
    base_execution_url: String,
    /// Prefix for status check contexts (e.g., "rigorix").
    context_prefix: String,
}

impl StatusCheckFactoryImpl {
    /// Create a new factory with the given base execution URL and context prefix.
    pub fn new(base_execution_url: impl Into<String>, context_prefix: impl Into<String>) -> Self {
        Self {
            base_execution_url: base_execution_url.into(),
            context_prefix: context_prefix.into(),
        }
    }
}

impl Default for StatusCheckFactoryImpl {
    fn default() -> Self {
        Self {
            base_execution_url: String::new(),
            context_prefix: "rigorix".to_string(),
        }
    }
}

#[async_trait]
impl StatusCheckFactory for StatusCheckFactoryImpl {
    async fn build_pending_status(
        &self,
        execution_id: &str,
        description: &str,
    ) -> Result<GitHubStatus, CiIntegrationError> {
        let context = self.build_context("execution");
        let target_url = self.build_target_url(execution_id).await;

        Ok(GitHubStatus::new("pending", &context, description).with_target_url(target_url))
    }

    async fn build_outcome_status(
        &self,
        execution_id: &str,
        state: StatusCheckState,
        iterations: u32,
    ) -> Result<GitHubStatus, CiIntegrationError> {
        let description = match &state {
            StatusCheckState::Pending => "Execution in progress".to_string(),
            StatusCheckState::Success => "All validations passed".to_string(),
            StatusCheckState::Failure => {
                format!("Validation failed after {} iteration(s)", iterations)
            }
            StatusCheckState::Error => {
                "Partial recovery — some nodes recovered, others failed".to_string()
            }
        };

        let context = self.build_context("execution");
        let target_url = self.build_target_url(execution_id).await;

        Ok(
            GitHubStatus::new(state.as_github_state(), &context, &description)
                .with_target_url(target_url),
        )
    }

    async fn build_target_url(&self, execution_id: &str) -> String {
        if self.base_execution_url.is_empty() {
            format!("https://github.com/rigorix/executions/{}", execution_id)
        } else {
            format!("{}{}", self.base_execution_url, execution_id)
        }
    }

    fn build_context(&self, suffix: &str) -> String {
        if suffix == "execution" {
            status_context::EXECUTION.to_string()
        } else if suffix == "validation" {
            status_context::VALIDATION.to_string()
        } else {
            format!("{}/{}", self.context_prefix, suffix)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_pending_status() {
        let factory = StatusCheckFactoryImpl::default();
        let status = factory
            .build_pending_status("exec-123", "Rigorix execution in progress")
            .await
            .unwrap();

        assert_eq!(status.state, "pending");
        assert_eq!(status.context, "rigorix/execution");
        assert_eq!(status.description, "Rigorix execution in progress");
        assert!(status.target_url.is_some());
    }

    #[tokio::test]
    async fn test_build_success_status() {
        let factory = StatusCheckFactoryImpl::default();
        let status = factory
            .build_outcome_status("exec-123", StatusCheckState::Success, 1)
            .await
            .unwrap();

        assert_eq!(status.state, "success");
        assert_eq!(status.description, "All validations passed");
        assert_eq!(status.context, "rigorix/execution");
    }

    #[tokio::test]
    async fn test_build_failure_status() {
        let factory = StatusCheckFactoryImpl::default();
        let status = factory
            .build_outcome_status("exec-123", StatusCheckState::Failure, 3)
            .await
            .unwrap();

        assert_eq!(status.state, "failure");
        assert!(status.description.contains("3"));
        assert!(status.description.contains("iteration"));
    }

    #[tokio::test]
    async fn test_build_error_status() {
        let factory = StatusCheckFactoryImpl::default();
        let status = factory
            .build_outcome_status("exec-123", StatusCheckState::Error, 2)
            .await
            .unwrap();

        assert_eq!(status.state, "error");
        assert!(
            status
                .description
                .to_lowercase()
                .contains("partial recovery")
        );
    }

    #[tokio::test]
    async fn test_build_target_url_empty_base() {
        let factory = StatusCheckFactoryImpl::default();
        let url = factory.build_target_url("exec-456").await;
        assert!(url.contains("exec-456"));
    }

    #[tokio::test]
    async fn test_build_target_url_with_base() {
        let factory =
            StatusCheckFactoryImpl::new("https://github.com/owner/repo/actions/runs/", "rigorix");
        let url = factory.build_target_url("12345").await;
        assert_eq!(url, "https://github.com/owner/repo/actions/runs/12345");
    }

    #[test]
    fn test_build_context_execution() {
        let factory = StatusCheckFactoryImpl::default();
        assert_eq!(factory.build_context("execution"), "rigorix/execution");
    }

    #[test]
    fn test_build_context_validation() {
        let factory = StatusCheckFactoryImpl::default();
        assert_eq!(factory.build_context("validation"), "rigorix/validation");
    }

    #[test]
    fn test_build_context_custom() {
        let factory = StatusCheckFactoryImpl::default();
        assert_eq!(factory.build_context("custom"), "rigorix/custom");
    }
}

//! Implementation of `StatusCheckService`.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md#status-check
//! Implements: StatusCheckManager — creates/updates GitHub commit status checks
//! Issue: issue-statuscheckmanager
//!
//! Maps engine execution states to GitHub commit status check states:
//! - Pending/Running → "pending"
//! - Completed/Validated → "success"
//! - Failed/Exhausted → "failure"
//! - PartialRecovery → "error"

use async_trait::async_trait;
use std::sync::Arc;
use tracing::info;

use crate::ci_integration::application::dto::{
    CreatePendingStatusInput, CreatePendingStatusOutput, ExecutionOutcomeDto, UpdateStatusInput,
    UpdateStatusOutput,
};
use crate::ci_integration::application::factory::StatusCheckFactory;
use crate::ci_integration::application::service::StatusCheckService;
use crate::ci_integration::domain::{CiIntegrationError, StatusCheckState};
use crate::ci_integration::infrastructure::repository::StatusCheckRepository;

/// Implementation of the `StatusCheckService` (StatusCheckManager).
///
/// Orchestrates creation and update of GitHub commit status checks by
/// delegating to a `StatusCheckFactory` for payload construction and
/// a `StatusCheckRepository` for API communication.
///
/// # Architecture
///
/// ```text
/// StatusCheckServiceImpl
///   ├── StatusCheckFactory    → builds GitHubStatus payloads
///   └── StatusCheckRepository → sends to GitHub API
/// ```
pub struct StatusCheckServiceImpl {
    factory: Arc<dyn StatusCheckFactory>,
    repository: Arc<dyn StatusCheckRepository>,
    /// Repository owner (e.g., "rigorix").
    owner: String,
    /// Repository name (e.g., "rigorix-oss").
    repo: String,
}

impl StatusCheckServiceImpl {
    /// Create a new StatusCheckManager implementation.
    pub fn new(
        factory: Arc<dyn StatusCheckFactory>,
        repository: Arc<dyn StatusCheckRepository>,
        owner: impl Into<String>,
        repo: impl Into<String>,
    ) -> Self {
        Self {
            factory,
            repository,
            owner: owner.into(),
            repo: repo.into(),
        }
    }
}

#[async_trait]
impl StatusCheckService for StatusCheckServiceImpl {
    async fn create_pending(
        &self,
        input: CreatePendingStatusInput,
    ) -> Result<CreatePendingStatusOutput, CiIntegrationError> {
        // Validate input
        if input.commit_sha.is_empty() {
            return Err(CiIntegrationError::InvalidArgument {
                detail: "commit_sha must not be empty".to_string(),
            });
        }
        if input.description.is_empty() {
            return Err(CiIntegrationError::InvalidArgument {
                detail: "description must not be empty".to_string(),
            });
        }

        // Build the pending status payload
        let status = self
            .factory
            .build_pending_status(&input.execution_id.to_string(), &input.description)
            .await?;

        // Send to GitHub API via repository
        self.repository
            .create_status(&self.owner, &self.repo, &input.commit_sha, status)
            .await?;

        info!(
            commit_sha = %input.commit_sha,
            execution_id = %input.execution_id,
            "status_check: created pending"
        );

        Ok(CreatePendingStatusOutput {
            context: "rigorix/execution".to_string(),
            state: StatusCheckState::Pending,
        })
    }

    async fn update_status(
        &self,
        input: UpdateStatusInput,
    ) -> Result<UpdateStatusOutput, CiIntegrationError> {
        // Validate input
        if input.commit_sha.is_empty() {
            return Err(CiIntegrationError::InvalidArgument {
                detail: "commit_sha must not be empty".to_string(),
            });
        }

        // Determine the state from the outcome
        let state = determine_state(&input.outcome);
        let description = self.outcome_description(&input.outcome);

        // Build the terminal status payload
        let status = self
            .factory
            .build_outcome_status(
                &input.execution_id.to_string(),
                state.clone(),
                input.outcome.iterations,
            )
            .await?;

        // Send to GitHub API via repository
        self.repository
            .create_status(&self.owner, &self.repo, &input.commit_sha, status)
            .await?;

        info!(
            commit_sha = %input.commit_sha,
            execution_id = %input.execution_id,
            new_state = %state.as_github_state(),
            iterations = input.outcome.iterations,
            "status_check: updated"
        );

        Ok(UpdateStatusOutput {
            github_state: state.as_github_state().to_string(),
            state,
            description,
            context: "rigorix/execution".to_string(),
        })
    }

    async fn execution_url(&self, execution_id: &str) -> String {
        self.factory.build_target_url(execution_id).await
    }
}

impl StatusCheckServiceImpl {
    /// Generate a human-readable description for the outcome.
    fn outcome_description(&self, outcome: &ExecutionOutcomeDto) -> String {
        if outcome.is_validated {
            "All validations passed".to_string()
        } else if outcome.is_partial_recovery {
            "Partial recovery — some nodes recovered, others failed".to_string()
        } else {
            format!(
                "Validation failed after {} iteration(s)",
                outcome.iterations
            )
        }
    }
}

/// Determine the `StatusCheckState` from an execution outcome DTO.
fn determine_state(outcome: &ExecutionOutcomeDto) -> StatusCheckState {
    if outcome.is_validated {
        StatusCheckState::Success
    } else if outcome.is_partial_recovery {
        StatusCheckState::Error
    } else {
        StatusCheckState::Failure
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use uuid::Uuid;

    use crate::ci_integration::domain::GitHubStatus;
    use crate::ci_integration::infrastructure::repository::StatusCheckRepository;

    // ── Mock Dependencies ──

    struct MockFactory;

    #[async_trait]
    impl StatusCheckFactory for MockFactory {
        async fn build_pending_status(
            &self,
            _execution_id: &str,
            _description: &str,
        ) -> Result<GitHubStatus, CiIntegrationError> {
            Ok(GitHubStatus::new("pending", "rigorix/execution", "test"))
        }

        async fn build_outcome_status(
            &self,
            _execution_id: &str,
            state: StatusCheckState,
            _iterations: u32,
        ) -> Result<GitHubStatus, CiIntegrationError> {
            let s = state.as_github_state();
            Ok(GitHubStatus::new(s, "rigorix/execution", "test outcome"))
        }

        async fn build_target_url(&self, _execution_id: &str) -> String {
            "https://github.com/rigorix/executions/test".to_string()
        }

        fn build_context(&self, suffix: &str) -> String {
            format!("rigorix/{}", suffix)
        }
    }

    struct MockRepository;

    #[async_trait]
    impl StatusCheckRepository for MockRepository {
        async fn create_status(
            &self,
            _owner: &str,
            _repo: &str,
            _sha: &str,
            _status: GitHubStatus,
        ) -> Result<(), CiIntegrationError> {
            Ok(())
        }

        async fn get_status(
            &self,
            _owner: &str,
            _repo: &str,
            _sha: &str,
            _context: &str,
        ) -> Result<Option<GitHubStatus>, CiIntegrationError> {
            Ok(None)
        }

        async fn list_statuses(
            &self,
            _owner: &str,
            _repo: &str,
            _sha: &str,
        ) -> Result<Vec<GitHubStatus>, CiIntegrationError> {
            Ok(vec![])
        }
    }

    fn make_service() -> StatusCheckServiceImpl {
        StatusCheckServiceImpl::new(
            Arc::new(MockFactory),
            Arc::new(MockRepository),
            "rigorix",
            "rigorix-oss",
        )
    }

    // ── Tests ──

    #[tokio::test]
    async fn test_create_pending_success() {
        let svc = make_service();
        let result = svc
            .create_pending(CreatePendingStatusInput {
                commit_sha: "abc123def456".to_string(),
                execution_id: Uuid::new_v4(),
                description: "Rigorix execution in progress".to_string(),
                context_override: None,
            })
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.state, StatusCheckState::Pending);
        assert_eq!(output.context, "rigorix/execution");
    }

    #[tokio::test]
    async fn test_create_pending_empty_sha() {
        let svc = make_service();
        let result = svc
            .create_pending(CreatePendingStatusInput {
                commit_sha: String::new(),
                execution_id: Uuid::new_v4(),
                description: "test".to_string(),
                context_override: None,
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CiIntegrationError::InvalidArgument { .. }
        ));
    }

    #[tokio::test]
    async fn test_create_pending_empty_description() {
        let svc = make_service();
        let result = svc
            .create_pending(CreatePendingStatusInput {
                commit_sha: "abc123".to_string(),
                execution_id: Uuid::new_v4(),
                description: String::new(),
                context_override: None,
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CiIntegrationError::InvalidArgument { .. }
        ));
    }

    #[tokio::test]
    async fn test_update_status_success() {
        let svc = make_service();
        let result = svc
            .update_status(UpdateStatusInput {
                commit_sha: "abc123def456".to_string(),
                execution_id: Uuid::new_v4(),
                outcome: ExecutionOutcomeDto {
                    is_validated: true,
                    is_failed: false,
                    is_partial_recovery: false,
                    iterations: 1,
                    description: "All validations passed".to_string(),
                },
                context_override: None,
            })
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.github_state, "success");
        assert_eq!(output.state, StatusCheckState::Success);
    }

    #[tokio::test]
    async fn test_update_status_failure() {
        let svc = make_service();
        let result = svc
            .update_status(UpdateStatusInput {
                commit_sha: "abc123".to_string(),
                execution_id: Uuid::new_v4(),
                outcome: ExecutionOutcomeDto {
                    is_validated: false,
                    is_failed: true,
                    is_partial_recovery: false,
                    iterations: 3,
                    description: "Validation failed".to_string(),
                },
                context_override: None,
            })
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.github_state, "failure");
    }

    #[tokio::test]
    async fn test_update_status_partial_recovery() {
        let svc = make_service();
        let result = svc
            .update_status(UpdateStatusInput {
                commit_sha: "abc123".to_string(),
                execution_id: Uuid::new_v4(),
                outcome: ExecutionOutcomeDto {
                    is_validated: false,
                    is_failed: false,
                    is_partial_recovery: true,
                    iterations: 2,
                    description: "Partial recovery".to_string(),
                },
                context_override: None,
            })
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.github_state, "error");
    }

    #[tokio::test]
    async fn test_update_status_empty_sha() {
        let svc = make_service();
        let result = svc
            .update_status(UpdateStatusInput {
                commit_sha: String::new(),
                execution_id: Uuid::new_v4(),
                outcome: ExecutionOutcomeDto {
                    is_validated: true,
                    is_failed: false,
                    is_partial_recovery: false,
                    iterations: 1,
                    description: "ok".to_string(),
                },
                context_override: None,
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execution_url() {
        let svc = make_service();
        let url = svc.execution_url("exec-789").await;
        // Mock factory returns fixed URL for all execution IDs
        assert!(url.contains("rigorix/executions/test"));
    }

    // ── Unit: determine_state ──

    #[test]
    fn test_determine_state_validated() {
        let outcome = ExecutionOutcomeDto {
            is_validated: true,
            is_failed: false,
            is_partial_recovery: false,
            iterations: 1,
            description: String::new(),
        };
        assert_eq!(determine_state(&outcome), StatusCheckState::Success);
    }

    #[test]
    fn test_determine_state_failed() {
        let outcome = ExecutionOutcomeDto {
            is_validated: false,
            is_failed: true,
            is_partial_recovery: false,
            iterations: 3,
            description: String::new(),
        };
        assert_eq!(determine_state(&outcome), StatusCheckState::Failure);
    }

    #[test]
    fn test_determine_state_partial_recovery() {
        let outcome = ExecutionOutcomeDto {
            is_validated: false,
            is_failed: false,
            is_partial_recovery: true,
            iterations: 2,
            description: String::new(),
        };
        assert_eq!(determine_state(&outcome), StatusCheckState::Error);
    }
}

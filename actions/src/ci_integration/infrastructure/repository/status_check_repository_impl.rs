//! Implementation of `StatusCheckRepository`.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md#status-check
//! Implements: StatusCheckRepository trait — GitHub REST API via shared GitHubClient
//! Issue: issue-statuscheckmanager
//!
//! Delegates to the shared `GitHubClient` for all GitHub API operations.
//! Combines `owner` and `repo` into the `owner/repo` format expected by the client.

use async_trait::async_trait;
use std::sync::Arc;

use crate::ci_integration::domain::{CiIntegrationError, GitHubStatus};
use crate::ci_integration::infrastructure::repository::StatusCheckRepository;
use crate::shared::github_client::GitHubClientError;

/// Implementation of `StatusCheckRepository` backed by the shared `GitHubClient`.
pub struct StatusCheckRepositoryImpl {
    client: Arc<crate::shared::github_client::GitHubClient>,
}

impl StatusCheckRepositoryImpl {
    /// Create a new repository implementation wrapping the shared client.
    pub fn new(client: Arc<crate::shared::github_client::GitHubClient>) -> Self {
        Self { client }
    }

    /// Build a combined `owner/repo` string for the shared client API.
    fn format_repo(&self, owner: &str, repo: &str) -> String {
        format!("{}/{}", owner, repo)
    }

    /// Convert a string state to `StatusState`.
    fn parse_state(state: &str) -> crate::shared::github_client::StatusState {
        match state {
            "pending" => crate::shared::github_client::StatusState::Pending,
            "success" => crate::shared::github_client::StatusState::Success,
            "failure" => crate::shared::github_client::StatusState::Failure,
            "error" => crate::shared::github_client::StatusState::Error,
            _ => crate::shared::github_client::StatusState::Error,
        }
    }
}

#[async_trait]
impl StatusCheckRepository for StatusCheckRepositoryImpl {
    async fn create_status(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        status: GitHubStatus,
    ) -> Result<(), CiIntegrationError> {
        let full_repo = self.format_repo(owner, repo);

        // Convert domain GitHubStatus to shared GitHubStatus
        let shared_status = crate::shared::github_client::GitHubStatus {
            state: Self::parse_state(&status.state),
            description: status.description,
            context: status.context,
            target_url: status.target_url,
        };

        self.client
            .create_status(&full_repo, sha, &shared_status)
            .await
            .map_err(map_github_error)
    }

    async fn get_status(
        &self,
        _owner: &str,
        _repo: &str,
        _sha: &str,
        _context: &str,
    ) -> Result<Option<GitHubStatus>, CiIntegrationError> {
        // GitHub's combined status API doesn't filter by context server-side.
        // This is a stub — full implementation requires a separate API call
        // to list statuses and filter by context.
        Err(CiIntegrationError::Internal {
            detail: "get_status not yet implemented — use list_statuses + client-side filter"
                .to_string(),
        })
    }

    async fn list_statuses(
        &self,
        _owner: &str,
        _repo: &str,
        _sha: &str,
    ) -> Result<Vec<GitHubStatus>, CiIntegrationError> {
        // Requires a separate API endpoint: GET /repos/{owner}/{repo}/commits/{sha}/statuses
        Err(CiIntegrationError::Internal {
            detail: "list_statuses not yet implemented".to_string(),
        })
    }
}

/// Map shared `GitHubClientError` to `CiIntegrationError`.
fn map_github_error(err: GitHubClientError) -> CiIntegrationError {
    match err {
        GitHubClientError::AuthFailed(msg) => CiIntegrationError::GitHubApi(
            crate::shared::github_client::GitHubClientError::AuthFailed(msg),
        ),
        GitHubClientError::RateLimited { retry_after_secs } => {
            CiIntegrationError::RateLimitExceeded { retry_after_secs }
        }
        GitHubClientError::NotFound(path) => CiIntegrationError::Internal {
            detail: format!("GitHub resource not found: {}", path),
        },
        GitHubClientError::PermissionDenied(msg) => CiIntegrationError::GitHubApi(
            crate::shared::github_client::GitHubClientError::PermissionDenied(msg),
        ),
        GitHubClientError::ApiError { status, message } => CiIntegrationError::Internal {
            detail: format!("GitHub API error ({}): {}", status, message),
        },
        GitHubClientError::NetworkError(e) => {
            CiIntegrationError::Io(std::io::Error::other(e.to_string()))
        }
        GitHubClientError::Serialization(e) => CiIntegrationError::Json(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_repo() {
        let client = Arc::new(crate::shared::github_client::GitHubClient::new(
            "test-token",
        ));
        let repo = StatusCheckRepositoryImpl::new(client);
        assert_eq!(
            repo.format_repo("rigorix", "rigorix-oss"),
            "rigorix/rigorix-oss"
        );
    }

    #[test]
    fn test_parse_state() {
        assert!(matches!(
            StatusCheckRepositoryImpl::parse_state("pending"),
            crate::shared::github_client::StatusState::Pending
        ));
        assert!(matches!(
            StatusCheckRepositoryImpl::parse_state("success"),
            crate::shared::github_client::StatusState::Success
        ));
        assert!(matches!(
            StatusCheckRepositoryImpl::parse_state("failure"),
            crate::shared::github_client::StatusState::Failure
        ));
        assert!(matches!(
            StatusCheckRepositoryImpl::parse_state("error"),
            crate::shared::github_client::StatusState::Error
        ));
        assert!(matches!(
            StatusCheckRepositoryImpl::parse_state("unknown"),
            crate::shared::github_client::StatusState::Error
        ));
    }

    #[test]
    fn test_map_auth_error() {
        let err = GitHubClientError::AuthFailed("bad token".to_string());
        let mapped = map_github_error(err);
        assert!(mapped.is_retriable());
    }

    #[test]
    fn test_map_rate_limit_error() {
        let err = GitHubClientError::RateLimited {
            retry_after_secs: 60,
        };
        let mapped = map_github_error(err);
        assert!(mapped.is_retriable());
    }

    #[test]
    fn test_map_network_error_via_string() {
        let err = GitHubClientError::AuthFailed("network error".to_string());
        let mapped = map_github_error(err);
        // AuthFailed wraps into GitHubApi which wraps GitHubClientError::AuthFailed
        assert!(mapped.is_retriable());
    }
}

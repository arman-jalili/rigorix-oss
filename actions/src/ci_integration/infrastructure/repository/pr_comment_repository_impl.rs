//! Implementation of `PrCommentRepository`.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md#pr-comment
//! Implements: PrCommentRepository trait — GitHub REST API via shared GitHubClient
//! Issue: issue-prcommentmanager
//!
//! Delegates to the shared `GitHubClient` for all GitHub API operations on
//! issue/PR comments. Combines `owner` and `repo` into the `owner/repo` format
//! expected by the client.

use async_trait::async_trait;
use std::sync::Arc;

use crate::ci_integration::domain::{CiIntegrationError, PrComment, PrCommentType};
use crate::ci_integration::infrastructure::repository::PrCommentRepository;
use crate::shared::github_client::GitHubClientError;

/// Implementation of `PrCommentRepository` backed by the shared `GitHubClient`.
pub struct PrCommentRepositoryImpl {
    client: Arc<crate::shared::github_client::GitHubClient>,
}

impl PrCommentRepositoryImpl {
    /// Create a new repository implementation wrapping the shared client.
    pub fn new(client: Arc<crate::shared::github_client::GitHubClient>) -> Self {
        Self { client }
    }

    /// Build a combined `owner/repo` string for the shared client API.
    fn format_repo(&self, owner: &str, repo: &str) -> String {
        format!("{}/{}", owner, repo)
    }

    /// Convert a shared `Comment` to our domain `PrComment`.
    fn to_domain_comment(
        comment: &crate::shared::github_client::Comment,
        issue_number: u64,
    ) -> PrComment {
        let is_bot = comment
            .body
            .contains(crate::ci_integration::domain::BOT_IDENTIFIER);
        let mut pc = PrComment::new(
            comment.id,
            issue_number,
            comment.body.clone(),
            String::new(),
            PrCommentType::ExecutionSummary,
        );
        if is_bot {
            pc = pc.mark_as_bot();
        }
        pc
    }
}

#[async_trait]
impl PrCommentRepository for PrCommentRepositoryImpl {
    async fn create_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        body: &str,
    ) -> Result<PrComment, CiIntegrationError> {
        let full_repo = self.format_repo(owner, repo);
        let comment = self
            .client
            .create_issue_comment(&full_repo, issue_number, body)
            .await
            .map_err(|e| map_comment_error(e, issue_number))?;

        Ok(Self::to_domain_comment(&comment, issue_number))
    }

    async fn update_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
        body: &str,
    ) -> Result<PrComment, CiIntegrationError> {
        let _full_repo = self.format_repo(owner, repo);
        self.client
            .update_issue_comment(comment_id, body)
            .await
            .map_err(|e| map_comment_error(e, comment_id))?;

        Ok(PrComment::new(
            comment_id,
            0, // We don't have the issue_number in update context
            body.to_string(),
            String::new(), // User info not returned by update
            PrCommentType::ExecutionSummary,
        ))
    }

    async fn list_comments(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> Result<Vec<PrComment>, CiIntegrationError> {
        let full_repo = self.format_repo(owner, repo);
        let comments = self
            .client
            .list_issue_comments(&full_repo, issue_number)
            .await
            .map_err(|e| map_comment_error(e, issue_number))?;

        Ok(comments
            .iter()
            .map(|c| Self::to_domain_comment(c, issue_number))
            .collect())
    }

    async fn get_comment(
        &self,
        _owner: &str,
        _repo: &str,
        comment_id: u64,
    ) -> Result<PrComment, CiIntegrationError> {
        Err(CiIntegrationError::Internal {
            detail: format!(
                "get_comment not yet implemented — comment_id={}",
                comment_id
            ),
        })
    }

    async fn delete_comment(
        &self,
        _owner: &str,
        _repo: &str,
        comment_id: u64,
    ) -> Result<(), CiIntegrationError> {
        Err(CiIntegrationError::Internal {
            detail: format!(
                "delete_comment not yet implemented — comment_id={}",
                comment_id
            ),
        })
    }
}

/// Map shared `GitHubClientError` to `CiIntegrationError` for comment operations.
fn map_comment_error(err: GitHubClientError, issue_number: u64) -> CiIntegrationError {
    match err {
        GitHubClientError::AuthFailed(_) | GitHubClientError::PermissionDenied(_) => {
            CiIntegrationError::PrCommentFailed {
                issue_number,
                detail: format!("GitHub API authentication/permission error: {}", err),
            }
        }
        GitHubClientError::RateLimited { retry_after_secs } => {
            CiIntegrationError::RateLimitExceeded { retry_after_secs }
        }
        GitHubClientError::NotFound(msg) => CiIntegrationError::Internal {
            detail: format!("GitHub resource not found: {}", msg),
        },
        GitHubClientError::ApiError { status, message } => CiIntegrationError::PrCommentFailed {
            issue_number,
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
        let repo = PrCommentRepositoryImpl::new(client);
        assert_eq!(
            repo.format_repo("rigorix", "rigorix-oss"),
            "rigorix/rigorix-oss"
        );
    }

    #[test]
    fn test_to_domain_comment() {
        let comment = crate::shared::github_client::Comment {
            id: 12345,
            body: "<!-- rigorix-bot -->\n## Summary".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let domain = PrCommentRepositoryImpl::to_domain_comment(&comment, 42);
        assert_eq!(domain.id, 12345);
        assert_eq!(domain.issue_number, 42);
        assert!(domain.is_bot_comment);
    }

    #[test]
    fn test_to_domain_comment_non_bot() {
        let comment = crate::shared::github_client::Comment {
            id: 67890,
            body: "Just a regular comment".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let domain = PrCommentRepositoryImpl::to_domain_comment(&comment, 42);
        assert_eq!(domain.id, 67890);
        assert!(!domain.is_bot_comment);
    }

    #[test]
    fn test_map_comment_error_rate_limit() {
        let err = GitHubClientError::RateLimited {
            retry_after_secs: 30,
        };
        let mapped = map_comment_error(err, 42);
        assert!(mapped.is_retriable());
    }

    #[test]
    fn test_map_comment_error_not_found() {
        let err = GitHubClientError::NotFound("comment not found".to_string());
        let mapped = map_comment_error(err, 42);
        assert!(!mapped.is_retriable());
    }

    #[test]
    fn test_map_comment_error_auth() {
        let err = GitHubClientError::AuthFailed("bad token".to_string());
        let mapped = map_comment_error(err, 42);
        // PrCommentFailed is not retriable (requires user intervention)
        assert!(!mapped.is_retriable());
    }
}

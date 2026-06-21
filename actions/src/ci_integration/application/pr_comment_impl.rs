//! Implementation of `PrCommentService`.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md#pr-comment
//! Implements: PrCommentManager — posts/updates structured PR review comments
//! Issue: issue-prcommentmanager
//!
//! Uses a "sticky comment" pattern: identifies the existing rigorix bot
//! comment and updates it in-place rather than posting multiple comments.
//! Bot comments are identified by the `BOT_IDENTIFIER` marker.

use async_trait::async_trait;
use std::sync::Arc;
use tracing::info;

use crate::ci_integration::application::dto::{
    FindBotCommentInput, FindBotCommentOutput, UpsertCommentInput, UpsertCommentOutput,
};
use crate::ci_integration::application::factory::PrCommentFactory;
use crate::ci_integration::application::service::PrCommentService;
use crate::ci_integration::domain::{BOT_IDENTIFIER, CiIntegrationError};
use crate::ci_integration::infrastructure::repository::PrCommentRepository;

/// Implementation of the `PrCommentService` (PrCommentManager).
///
/// Posts structured PR comments with execution summaries using a sticky
/// comment pattern. Delegates to `PrCommentFactory` for markdown rendering
/// and `PrCommentRepository` for GitHub API communication.
///
/// # Architecture
///
/// ```text
/// PrCommentServiceImpl
///   ├── PrCommentFactory      → builds markdown summaries
///   └── PrCommentRepository   → sends to GitHub API
/// ```
pub struct PrCommentServiceImpl {
    factory: Arc<dyn PrCommentFactory>,
    repository: Arc<dyn PrCommentRepository>,
    /// Repository owner (e.g., "rigorix").
    owner: String,
    /// Repository name (e.g., "rigorix-oss").
    repo: String,
}

impl PrCommentServiceImpl {
    /// Create a new PrCommentManager implementation.
    pub fn new(
        factory: Arc<dyn PrCommentFactory>,
        repository: Arc<dyn PrCommentRepository>,
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
impl PrCommentService for PrCommentServiceImpl {
    async fn upsert(
        &self,
        input: UpsertCommentInput,
    ) -> Result<UpsertCommentOutput, CiIntegrationError> {
        // Render the summary as markdown
        let markdown = self.factory.format_as_markdown(&input.summary).await?;

        let execution_id = input.summary.execution_id;

        // Check if we have an existing comment ID to update
        if let Some(comment_id) = input.existing_comment_id {
            let comment = self
                .repository
                .update_comment(&self.owner, &self.repo, comment_id, &markdown)
                .await?;

            info!(
                issue_number = input.issue_number,
                comment_id = comment_id,
                %execution_id,
                "pr_comment: updated by explicit comment_id"
            );

            return Ok(UpsertCommentOutput {
                comment_id: comment.id,
                created: false,
            });
        }

        // Otherwise, use the sticky comment pattern: find existing bot comment
        let existing = self
            .find_bot_comment(FindBotCommentInput {
                issue_number: input.issue_number,
            })
            .await?;

        if let Some(bot_comment) = existing.comment {
            // Update existing bot comment
            let comment = self
                .repository
                .update_comment(&self.owner, &self.repo, bot_comment.id, &markdown)
                .await?;

            info!(
                issue_number = input.issue_number,
                comment_id = bot_comment.id,
                %execution_id,
                "pr_comment: updated existing bot comment"
            );

            Ok(UpsertCommentOutput {
                comment_id: comment.id,
                created: false,
            })
        } else {
            // Create new comment
            let comment = self
                .repository
                .create_comment(&self.owner, &self.repo, input.issue_number, &markdown)
                .await?;

            info!(
                issue_number = input.issue_number,
                comment_id = comment.id,
                %execution_id,
                "pr_comment: created new bot comment"
            );

            Ok(UpsertCommentOutput {
                comment_id: comment.id,
                created: true,
            })
        }
    }

    async fn find_bot_comment(
        &self,
        input: FindBotCommentInput,
    ) -> Result<FindBotCommentOutput, CiIntegrationError> {
        let comments = self
            .repository
            .list_comments(&self.owner, &self.repo, input.issue_number)
            .await?;

        let bot_comment = comments
            .into_iter()
            .find(|c| c.body.contains(BOT_IDENTIFIER));

        Ok(FindBotCommentOutput {
            found: bot_comment.is_some(),
            comment: bot_comment,
        })
    }

    async fn post_annotation(
        &self,
        issue_number: u64,
        body: &str,
        _commit_sha: &str,
        _path: &str,
        _line: u32,
    ) -> Result<UpsertCommentOutput, CiIntegrationError> {
        // Annotation comments are posted as regular issue comments
        // Full PR review annotation support requires the GitHub Checks API
        let comment = self
            .repository
            .create_comment(&self.owner, &self.repo, issue_number, body)
            .await?;

        Ok(UpsertCommentOutput {
            comment_id: comment.id,
            created: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use uuid::Uuid;

    use crate::ci_integration::application::dto::ExecutionOutcomeDto;
    use crate::ci_integration::domain::{
        ExecutionStatus, ExecutionStep, ExecutionSummary, PrComment, PrCommentType, StepStatus,
        ValidationInfo,
    };
    use crate::ci_integration::infrastructure::repository::PrCommentRepository;

    // ── Mock Factory ──

    struct MockCommentFactory;

    #[async_trait]
    impl PrCommentFactory for MockCommentFactory {
        async fn build_summary(
            &self,
            _execution_id: &str,
            _status: &str,
            _quality: Option<&str>,
            _steps: Vec<ExecutionOutcomeDto>,
        ) -> Result<ExecutionSummary, CiIntegrationError> {
            Ok(ExecutionSummary {
                execution_id: Uuid::new_v4(),
                status: ExecutionStatus::Passed,
                quality: None,
                steps: vec![],
                validation: None,
                follow_up: None,
            })
        }

        async fn format_as_markdown(
            &self,
            _summary: &ExecutionSummary,
        ) -> Result<String, CiIntegrationError> {
            Ok("<!-- rigorix-bot -->\n## Summary\n\nExecution passed.".to_string())
        }
    }

    // ── Mock Repository ──

    struct MockCommentRepository {
        comments: std::sync::Mutex<Vec<PrComment>>,
    }

    impl MockCommentRepository {
        fn new() -> Self {
            Self {
                comments: std::sync::Mutex::new(vec![]),
            }
        }

        fn with_bot_comment() -> Self {
            let bot = PrComment::new(
                1,
                42,
                "<!-- rigorix-bot -->\n## Old Summary".to_string(),
                "rigorix-bot".to_string(),
                PrCommentType::ExecutionSummary,
            )
            .mark_as_bot();
            Self {
                comments: std::sync::Mutex::new(vec![bot]),
            }
        }
    }

    #[async_trait]
    impl PrCommentRepository for MockCommentRepository {
        async fn create_comment(
            &self,
            _owner: &str,
            _repo: &str,
            issue_number: u64,
            body: &str,
        ) -> Result<PrComment, CiIntegrationError> {
            let comment = PrComment::new(
                99,
                issue_number,
                body.to_string(),
                "rigorix-bot".to_string(),
                PrCommentType::ExecutionSummary,
            )
            .mark_as_bot();
            self.comments.lock().unwrap().push(comment.clone());
            Ok(comment)
        }

        async fn update_comment(
            &self,
            _owner: &str,
            _repo: &str,
            comment_id: u64,
            body: &str,
        ) -> Result<PrComment, CiIntegrationError> {
            // Update the comment in our mock store
            let mut comments = self.comments.lock().unwrap();
            if let Some(comment) = comments.iter_mut().find(|c| c.id == comment_id) {
                comment.body = body.to_string();
            }
            Ok(PrComment::new(
                comment_id,
                0,
                body.to_string(),
                "rigorix-bot".to_string(),
                PrCommentType::ExecutionSummary,
            ))
        }

        async fn list_comments(
            &self,
            _owner: &str,
            _repo: &str,
            _issue_number: u64,
        ) -> Result<Vec<PrComment>, CiIntegrationError> {
            Ok(self.comments.lock().unwrap().clone())
        }

        async fn get_comment(
            &self,
            _owner: &str,
            _repo: &str,
            _comment_id: u64,
        ) -> Result<PrComment, CiIntegrationError> {
            Err(CiIntegrationError::Internal {
                detail: "not implemented".to_string(),
            })
        }

        async fn delete_comment(
            &self,
            _owner: &str,
            _repo: &str,
            _comment_id: u64,
        ) -> Result<(), CiIntegrationError> {
            Ok(())
        }
    }

    fn make_service(repo: MockCommentRepository) -> PrCommentServiceImpl {
        PrCommentServiceImpl::new(
            Arc::new(MockCommentFactory),
            Arc::new(repo),
            "rigorix",
            "rigorix-oss",
        )
    }

    // ── Tests ──

    #[tokio::test]
    async fn test_upsert_creates_new_comment() {
        let repo = MockCommentRepository::new();
        let svc = make_service(repo);

        let summary = ExecutionSummary {
            execution_id: Uuid::new_v4(),
            status: ExecutionStatus::Passed,
            quality: None,
            steps: vec![],
            validation: None,
            follow_up: None,
        };

        let result = svc
            .upsert(UpsertCommentInput {
                issue_number: 42,
                summary,
                existing_comment_id: None,
            })
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.created);
        assert_eq!(output.comment_id, 99);
    }

    #[tokio::test]
    async fn test_upsert_updates_existing_comment() {
        let repo = MockCommentRepository::with_bot_comment();
        let svc = make_service(repo);

        let summary = ExecutionSummary {
            execution_id: Uuid::new_v4(),
            status: ExecutionStatus::Passed,
            quality: None,
            steps: vec![],
            validation: None,
            follow_up: None,
        };

        let result = svc
            .upsert(UpsertCommentInput {
                issue_number: 42,
                summary,
                existing_comment_id: None,
            })
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.created);
        assert_eq!(output.comment_id, 1);
    }

    #[tokio::test]
    async fn test_upsert_with_explicit_comment_id() {
        let repo = MockCommentRepository::new();
        let svc = make_service(repo);

        let summary = ExecutionSummary {
            execution_id: Uuid::new_v4(),
            status: ExecutionStatus::Passed,
            quality: None,
            steps: vec![],
            validation: None,
            follow_up: None,
        };

        let result = svc
            .upsert(UpsertCommentInput {
                issue_number: 42,
                summary,
                existing_comment_id: Some(5),
            })
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.created);
        assert_eq!(output.comment_id, 5);
    }

    #[tokio::test]
    async fn test_find_bot_comment_found() {
        let repo = MockCommentRepository::with_bot_comment();
        let svc = make_service(repo);

        let result = svc
            .find_bot_comment(FindBotCommentInput { issue_number: 42 })
            .await
            .unwrap();

        assert!(result.found);
        assert!(result.comment.is_some());
    }

    #[tokio::test]
    async fn test_find_bot_comment_not_found() {
        let repo = MockCommentRepository::new();
        let svc = make_service(repo);

        let result = svc
            .find_bot_comment(FindBotCommentInput { issue_number: 42 })
            .await
            .unwrap();

        assert!(!result.found);
        assert!(result.comment.is_none());
    }

    #[tokio::test]
    async fn test_post_annotation() {
        let repo = MockCommentRepository::new();
        let svc = make_service(repo);

        let result = svc
            .post_annotation(
                42,
                "## Annotation\n\nIssue found at line 10.",
                "abc123",
                "src/main.rs",
                10,
            )
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.created);
    }
}

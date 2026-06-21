//! Implementation of `PrCommentService`.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md#pr-comment
//! Implements: PrCommentService — posts PR comments via GitHub API
//! Issue: issue-outputformatter
//!
//! # Contract
//! - Implements `PrCommentService` trait from the frozen contract
//! - Wraps `shared::github_client::GitHubClient` for API calls
//! - Token is never logged

use async_trait::async_trait;
use tracing::info;

use crate::action_output::domain::{ActionOutputError, ExecutionContext, ExecutionStatus};

use super::dto::PostPrCommentInput;
use super::dto::PostPrCommentOutput;
use super::service::PrCommentService;

/// Default implementation of `PrCommentService`.
///
/// Posts markdown-formatted PR comments using the GitHub REST API.
/// Delegates to `shared::github_client::GitHubClient` for HTTP.
///
/// # Dependencies
/// - `shared::github_client::GitHubClient` — for GitHub API calls
///
/// # Construction
/// Use `PrCommentServiceImpl::new(github_client)`.
pub struct PrCommentServiceImpl {
    client: crate::shared::github_client::GitHubClient,
}

impl PrCommentServiceImpl {
    pub fn new(client: crate::shared::github_client::GitHubClient) -> Self {
        Self { client }
    }

    /// Render an execution summary as markdown for PR comments.
    fn render_execution_summary(&self, context: &ExecutionContext) -> String {
        let status_icon = match context.status {
            ExecutionStatus::Completed => "✅",
            ExecutionStatus::Failed => "❌",
            ExecutionStatus::PartialFailure => "⚠️",
        };
        let duration_secs = context.duration_ms as f64 / 1000.0;
        let quality = context.quality_level.as_deref().unwrap_or("unknown");

        let mut body = String::new();
        body.push_str(&format!(
            "## Rigorix Execution #{}\n\n",
            context.execution_id
        ));
        body.push_str(&format!(
            "**Status:** {} {} | **Duration:** {:.1}s | **Quality:** {}\n\n",
            status_icon,
            context.status.as_str(),
            duration_secs,
            quality,
        ));

        // Execution steps
        if !context.execution_steps.is_empty() {
            body.push_str("### Execution Plan\n\n");
            for step in &context.execution_steps {
                let icon = if step.success { "✅" } else { "❌" };
                let duration = step.duration_ms as f64 / 1000.0;
                body.push_str(&format!(
                    "{} **{}** — {} ({:.1}s)\n",
                    icon, step.id, step.description, duration,
                ));
            }
            body.push('\n');
        }

        // Validation info
        if context.iterations > 0 {
            body.push_str(&format!(
                "- Iterations: {}/{}\n",
                context.iterations, context.max_iterations,
            ));
            body.push_str(&format!(
                "- Cumulative tokens: {}\n",
                context.cumulative_tokens,
            ));
            body.push_str(&format!("- File changes: {}\n", context.file_changes.len(),));
            body.push('\n');
        }

        body
    }

    /// Render a failure summary as markdown for PR comments.
    fn render_failure_summary(
        &self,
        context: &ExecutionContext,
        execution_id: &uuid::Uuid,
    ) -> String {
        let duration_secs = context.duration_ms as f64 / 1000.0;

        let mut body = String::new();
        body.push_str("## Rigorix Validation Failed\n\n");
        body.push_str(&format!(
            "**Execution:** `{}` | **Duration:** {:.1}s | **Failures:** {}\n\n",
            execution_id, duration_secs, context.failure_count,
        ));

        if context.iterations > 0 {
            body.push_str(&format!(
                "- Iterations: {}/{}\n",
                context.iterations, context.max_iterations,
            ));
            body.push_str(&format!(
                "- Cumulative tokens: {}\n",
                context.cumulative_tokens,
            ));
            body.push('\n');
        }

        body
    }
}

#[async_trait]
impl PrCommentService for PrCommentServiceImpl {
    async fn post_comment(
        &self,
        input: PostPrCommentInput,
    ) -> Result<PostPrCommentOutput, ActionOutputError> {
        use crate::shared::github_client::GitHubClientError;

        let comment = self
            .client
            .create_issue_comment(&input.repo, input.pr_number, &input.body)
            .await
            .map_err(|e| match e {
                GitHubClientError::AuthFailed(msg) => ActionOutputError::GitHubApiError {
                    endpoint: format!("issues/{}/comments", input.pr_number),
                    status_code: 401,
                    response: msg,
                },
                GitHubClientError::PermissionDenied(msg) => ActionOutputError::GitHubApiError {
                    endpoint: format!("issues/{}/comments", input.pr_number),
                    status_code: 403,
                    response: msg,
                },
                GitHubClientError::RateLimited { retry_after_secs } => {
                    ActionOutputError::GitHubApiError {
                        endpoint: format!("issues/{}/comments", input.pr_number),
                        status_code: 429,
                        response: format!("rate limited, retry after {}s", retry_after_secs),
                    }
                }
                GitHubClientError::NotFound(msg) => ActionOutputError::GitHubApiError {
                    endpoint: format!("issues/{}/comments", input.pr_number),
                    status_code: 404,
                    response: msg,
                },
                GitHubClientError::ApiError { status, message } => {
                    ActionOutputError::GitHubApiError {
                        endpoint: format!("issues/{}/comments", input.pr_number),
                        status_code: status,
                        response: message,
                    }
                }
                GitHubClientError::NetworkError(e) => {
                    ActionOutputError::Io(std::io::Error::other(e.to_string()))
                }
                GitHubClientError::Serialization(e) => ActionOutputError::Json(e),
            })?;

        info!(
            pr_number = input.pr_number,
            comment_id = comment.id,
            "PR comment posted"
        );

        let html_url = format!(
            "https://github.com/{}/issues/{}",
            input.repo, input.pr_number
        );
        Ok(PostPrCommentOutput {
            comment_id: comment.id,
            html_url,
        })
    }

    async fn format_execution_summary(
        &self,
        context: &ExecutionContext,
    ) -> Result<String, ActionOutputError> {
        Ok(self.render_execution_summary(context))
    }

    async fn format_failure_summary(
        &self,
        context: &ExecutionContext,
        execution_id: &uuid::Uuid,
    ) -> Result<String, ActionOutputError> {
        Ok(self.render_failure_summary(context, execution_id))
    }

    async fn is_api_accessible(&self, token: &str) -> bool {
        // Create a temporary client with the provided token
        let temp_client = crate::shared::github_client::GitHubClient::new(token);
        temp_client.validate_token().await.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_output::domain::ExecutionContext;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_context() -> ExecutionContext {
        ExecutionContext {
            execution_id: Uuid::parse_str("e1852176-e586-4377-a8e8-d1cb4be89144").unwrap(),
            status: ExecutionStatus::Completed,
            iterations: 2,
            max_iterations: 3,
            cumulative_tokens: 3240,
            duration_ms: 12400,
            quality_level: Some("workspace".to_string()),
            template_id: Some("tpl".to_string()),
            failure_count: 0,
            file_changes: vec![],
            execution_steps: vec![],
            metadata: HashMap::new(),
        }
    }

    // We test formatting logic without network by constructing the service
    // This avoids needing a mock for the shared GitHubClient

    #[tokio::test]
    async fn test_format_execution_summary() {
        let client = crate::shared::github_client::GitHubClient::new("test-token");
        let svc = PrCommentServiceImpl::new(client);
        let context = make_context();

        let result = svc.format_execution_summary(&context).await;
        assert!(result.is_ok());

        let body = result.unwrap();
        assert!(body.contains("Rigorix Execution #"));
        assert!(body.contains("completed"));
        assert!(body.contains("12.4s"));
    }

    #[tokio::test]
    async fn test_format_failure_summary() {
        let client = crate::shared::github_client::GitHubClient::new("test-token");
        let svc = PrCommentServiceImpl::new(client);
        let context = ExecutionContext {
            status: ExecutionStatus::Failed,
            failure_count: 3,
            ..make_context()
        };

        let result = svc.format_failure_summary(&context, &Uuid::new_v4()).await;
        assert!(result.is_ok());

        let body = result.unwrap();
        assert!(body.len() > 50, "Body too short: {:?}", body);
        assert!(
            body.contains("Rigorix Validation Failed"),
            "Missing heading in: {:?}",
            body
        );
    }

    #[tokio::test]
    async fn test_format_execution_summary_with_steps() {
        use crate::action_output::domain::ExecutionStep;

        let client = crate::shared::github_client::GitHubClient::new("test-token");
        let svc = PrCommentServiceImpl::new(client);
        let context = ExecutionContext {
            execution_steps: vec![
                ExecutionStep {
                    id: "step-1".to_string(),
                    description: "Read file".to_string(),
                    success: true,
                    duration_ms: 300,
                    error: None,
                },
                ExecutionStep {
                    id: "step-2".to_string(),
                    description: "Write file".to_string(),
                    success: false,
                    duration_ms: 1000,
                    error: Some("Error".to_string()),
                },
            ],
            ..make_context()
        };

        let result = svc.format_execution_summary(&context).await;
        assert!(result.is_ok());
        let body = result.unwrap();
        assert!(body.contains("Execution Plan"));
        assert!(body.contains("step-1"));
        assert!(body.contains("step-2"));
    }
}

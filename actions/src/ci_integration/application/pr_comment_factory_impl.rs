//! Implementation of `PrCommentFactory`.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md#pr-comment
//! Implements: PrCommentFactory trait — builds ExecutionSummary and markdown rendering
//! Issue: issue-prcommentmanager
//!
//! Handles construction of structured execution summaries and their rendering
//! as GitHub-flavored markdown following the bot comment format.

use async_trait::async_trait;

use crate::ci_integration::application::dto::ExecutionOutcomeDto;
use crate::ci_integration::application::factory::PrCommentFactory;
use crate::ci_integration::domain::{
    BOT_IDENTIFIER, CiIntegrationError, ExecutionStatus, ExecutionStep, ExecutionSummary,
    StepStatus, ValidationInfo,
};

/// Implementation of `PrCommentFactory`.
///
/// Builds `ExecutionSummary` payloads and renders them as GitHub-flavored
/// markdown with the bot identifier marker.
pub struct PrCommentFactoryImpl;

impl PrCommentFactoryImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PrCommentFactoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PrCommentFactory for PrCommentFactoryImpl {
    async fn build_summary(
        &self,
        execution_id: &str,
        status: &str,
        quality: Option<&str>,
        steps: Vec<ExecutionOutcomeDto>,
    ) -> Result<ExecutionSummary, CiIntegrationError> {
        let exec_status = parse_status(status);
        let execution_steps: Vec<ExecutionStep> = steps
            .into_iter()
            .map(|s| ExecutionStep {
                name: s.description,
                status: if s.is_validated {
                    StepStatus::Completed
                } else if s.is_failed {
                    StepStatus::Failed
                } else {
                    StepStatus::Running
                },
                duration_secs: None,
            })
            .collect();

        let validation_info = ValidationInfo {
            iterations: 0,
            tokens: 0,
            template: None,
        };

        Ok(ExecutionSummary {
            execution_id: uuid::Uuid::parse_str(execution_id).map_err(|e| {
                CiIntegrationError::InvalidArgument {
                    detail: format!("invalid execution_id UUID: {}", e),
                }
            })?,
            status: exec_status,
            quality: quality.map(|q| q.to_string()),
            steps: execution_steps,
            validation: Some(validation_info),
            follow_up: Some(format!("/rigorix retry {}", execution_id)),
        })
    }

    async fn format_as_markdown(
        &self,
        summary: &ExecutionSummary,
    ) -> Result<String, CiIntegrationError> {
        let status_emoji = match &summary.status {
            ExecutionStatus::Running => "⏳",
            ExecutionStatus::Passed => "✅",
            ExecutionStatus::Failed => "❌",
            ExecutionStatus::PartialRecovery => "⚠️",
        };

        let mut body = String::new();
        body.push_str(BOT_IDENTIFIER);
        body.push('\n');
        body.push_str("## 🤖 Rigorix Execution Summary\n\n");
        body.push_str(&format!(
            "**Execution:** `{}` | **Status:** {} {}",
            summary.execution_id,
            status_emoji,
            status_label(&summary.status)
        ));

        if let Some(ref quality) = summary.quality {
            body.push_str(&format!(" | **Quality:** {}", quality));
        }
        body.push('\n');
        body.push('\n');

        // Plan steps
        if !summary.steps.is_empty() {
            body.push_str("### Plan\n");
            body.push_str("| Step | Status | Duration |\n");
            body.push_str("|------|--------|----------|\n");
            for step in &summary.steps {
                let step_emoji = match &step.status {
                    StepStatus::Completed => "✅",
                    StepStatus::Failed => "❌",
                    StepStatus::Running => "⏳",
                    StepStatus::Skipped => "⏭️",
                };
                let duration = step
                    .duration_secs
                    .map(|d| format!("{:.1}s", d))
                    .unwrap_or_else(|| "-".to_string());
                body.push_str(&format!(
                    "| {} {} | {} | {} |\n",
                    step_emoji, step.name, step_emoji, duration
                ));
            }
            body.push('\n');
        }

        // Validation info
        if let Some(ref validation) = summary.validation {
            body.push_str(&format!(
                "**Validation:** {} iteration(s)",
                validation.iterations
            ));
            if validation.tokens > 0 {
                body.push_str(&format!(" | **Tokens:** {}", validation.tokens));
            }
            if let Some(ref tmpl) = validation.template {
                body.push_str(&format!(" | **Template:** `{}`", tmpl));
            }
            body.push('\n');
            body.push('\n');
        }

        // Follow-up
        if let Some(ref follow_up) = summary.follow_up {
            body.push_str(&format!(
                "> Reply `{}` to re-run this execution.\n",
                follow_up
            ));
        }

        Ok(body)
    }
}

/// Parse a status string into `ExecutionStatus`.
fn parse_status(status: &str) -> ExecutionStatus {
    match status.to_lowercase().as_str() {
        "passed" | "success" | "validated" => ExecutionStatus::Passed,
        "failed" | "failure" => ExecutionStatus::Failed,
        "partial" | "partial_recovery" => ExecutionStatus::PartialRecovery,
        _ => ExecutionStatus::Running,
    }
}

/// Get a human-readable label for the execution status.
fn status_label(status: &ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Running => "Running",
        ExecutionStatus::Passed => "Passed",
        ExecutionStatus::Failed => "Failed",
        ExecutionStatus::PartialRecovery => "Partial Recovery",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_summary_passed() {
        let factory = PrCommentFactoryImpl::new();
        let summary = factory
            .build_summary(
                "e1852176-e586-4377-a8e8-d1cb4be89144",
                "passed",
                Some("workspace"),
                vec![],
            )
            .await
            .unwrap();

        assert_eq!(summary.status, ExecutionStatus::Passed);
        assert_eq!(summary.quality, Some("workspace".to_string()));
        assert!(summary.follow_up.is_some());
    }

    #[tokio::test]
    async fn test_build_summary_failed() {
        let factory = PrCommentFactoryImpl::new();
        let summary = factory
            .build_summary(
                "e1852176-e586-4377-a8e8-d1cb4be89144",
                "failed",
                None,
                vec![],
            )
            .await
            .unwrap();

        assert_eq!(summary.status, ExecutionStatus::Failed);
        assert_eq!(summary.quality, None);
    }

    #[tokio::test]
    async fn test_build_summary_invalid_uuid() {
        let factory = PrCommentFactoryImpl::new();
        let result = factory
            .build_summary("not-a-uuid", "passed", None, vec![])
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_format_markdown_passed() {
        let factory = PrCommentFactoryImpl::new();
        let summary = factory
            .build_summary(
                "e1852176-e586-4377-a8e8-d1cb4be89144",
                "passed",
                Some("workspace"),
                vec![],
            )
            .await
            .unwrap();

        let md = factory.format_as_markdown(&summary).await.unwrap();
        assert!(md.contains("Rigorix Execution Summary"));
        assert!(md.contains("✅"));
        assert!(md.contains("Passed"));
        assert!(md.contains(BOT_IDENTIFIER));
        assert!(md.contains("workspace"));
        assert!(md.contains("retry"));
    }

    #[tokio::test]
    async fn test_format_markdown_failed() {
        let factory = PrCommentFactoryImpl::new();
        let summary = factory
            .build_summary(
                "e1852176-e586-4377-a8e8-d1cb4be89144",
                "failed",
                None,
                vec![],
            )
            .await
            .unwrap();

        let md = factory.format_as_markdown(&summary).await.unwrap();
        assert!(md.contains("❌"));
        assert!(md.contains("Failed"));
    }

    #[tokio::test]
    async fn test_format_markdown_with_steps() {
        let factory = PrCommentFactoryImpl::new();
        let steps = vec![
            ExecutionOutcomeDto {
                is_validated: true,
                is_failed: false,
                is_partial_recovery: false,
                iterations: 1,
                description: "read-task-file".to_string(),
            },
            ExecutionOutcomeDto {
                is_validated: true,
                is_failed: false,
                is_partial_recovery: false,
                iterations: 1,
                description: "add-get-active-tasks-method".to_string(),
            },
        ];

        let summary = factory
            .build_summary(
                "e1852176-e586-4377-a8e8-d1cb4be89144",
                "passed",
                None,
                steps,
            )
            .await
            .unwrap();

        let md = factory.format_as_markdown(&summary).await.unwrap();
        assert!(md.contains("read-task-file"));
        assert!(md.contains("add-get-active-tasks-method"));
        assert!(md.contains("| Step |"));
    }

    #[test]
    fn test_parse_status_variants() {
        assert_eq!(parse_status("passed"), ExecutionStatus::Passed);
        assert_eq!(parse_status("success"), ExecutionStatus::Passed);
        assert_eq!(parse_status("validated"), ExecutionStatus::Passed);
        assert_eq!(parse_status("failed"), ExecutionStatus::Failed);
        assert_eq!(parse_status("failure"), ExecutionStatus::Failed);
        assert_eq!(parse_status("partial"), ExecutionStatus::PartialRecovery);
        assert_eq!(parse_status("running"), ExecutionStatus::Running);
    }

    #[test]
    fn test_status_label() {
        assert_eq!(status_label(&ExecutionStatus::Passed), "Passed");
        assert_eq!(status_label(&ExecutionStatus::Failed), "Failed");
        assert_eq!(status_label(&ExecutionStatus::Running), "Running");
        assert_eq!(
            status_label(&ExecutionStatus::PartialRecovery),
            "Partial Recovery"
        );
    }
}

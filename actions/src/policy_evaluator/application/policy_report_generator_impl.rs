//! Implementation of `PolicyReportGenerationService`.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md
//! Issue: issue-policyevaluator

use async_trait::async_trait;

use crate::policy_evaluator::domain::{PolicyError, PolicyResult, PolicyViolation};

use super::dto::{
    GenerateReportInput, GenerateReportOutput, ViolationReportEntry,
};
use super::service::PolicyReportGenerationService;

/// Default implementation of `PolicyReportGenerationService`.
///
/// Formats policy evaluation results into:
/// - GitHub workflow annotations (`::error`, `::warning`, `::notice`)
/// - Structured violation entries for machine consumption
/// - Markdown summaries suitable for PR comments
pub struct PolicyReportGenerationServiceImpl;

#[async_trait]
impl PolicyReportGenerationService for PolicyReportGenerationServiceImpl {
    async fn generate_report(
        &self,
        input: GenerateReportInput,
    ) -> Result<GenerateReportOutput, PolicyError> {
        let mut annotations = Vec::new();
        let mut entries = Vec::new();

        for violation in &input.result.violations {
            let annotation = self.format_annotation(violation).await;
            annotations.push(annotation);

            let (annotation_type, _) = violation.to_annotation();
            entries.push(ViolationReportEntry {
                violation_type: violation.violation_type().to_string(),
                rule: violation.rule_name().to_string(),
                file: violation.file().to_string(),
                message: match violation {
                    PolicyViolation::Deny { message, .. } => message.clone(),
                    PolicyViolation::Flag { message, .. } => message.clone(),
                    PolicyViolation::RequireReview { description, file, .. } => {
                        format!("File '{}' requires review: {}", file, description)
                    }
                },
                annotation_type: annotation_type.to_string(),
            });
        }

        let markdown_summary = self.format_markdown_summary(&input.result).await;
        let status_line = self.format_status_line(&input.result).await;

        let should_fail = input.result.has_blocking_violations;

        Ok(GenerateReportOutput {
            annotations,
            entries,
            markdown_summary: format!("{}\n\n{}", status_line, markdown_summary),
            should_fail,
        })
    }

    async fn format_annotation(
        &self,
        violation: &PolicyViolation,
    ) -> String {
        let (annotation_type, message) = violation.to_annotation();
        let file = violation.file();
        let escaped = self.escape_annotation(&message).await;
        format!(
            "::{} file={},title={} violation::{}",
            annotation_type,
            file,
            violation.violation_type(),
            escaped
        )
    }

    async fn format_markdown_summary(&self, result: &PolicyResult) -> String {
        let mut md = String::new();

        if result.violations.is_empty() {
            md.push_str("## ✅ Policy Evaluation: Passed\n\n");
            md.push_str("No policy violations found.\n");
            return md;
        }

        md.push_str("## 🔍 Policy Evaluation Results\n\n");

        if result.has_blocking_violations {
            md.push_str("### ❌ Blocking Violations\n\n");
            for v in &result.violations {
                if v.is_blocking() {
                    md.push_str(&format!(
                        "- **{}**: `{}` — {}\n",
                        v.rule_name(),
                        v.file(),
                        match v {
                            PolicyViolation::Deny { message, .. } => message.as_str(),
                            _ => "Blocking violation",
                        }
                    ));
                }
            }
            md.push('\n');
        }

        if result.has_warnings {
            md.push_str("### ⚠️ Warnings\n\n");
            for v in &result.violations {
                if !v.is_blocking() {
                    md.push_str(&format!(
                        "- **{}**: `{}` — {}\n",
                        v.rule_name(),
                        v.file(),
                        match v {
                            PolicyViolation::RequireReview { description, .. } => description.as_str(),
                            PolicyViolation::Flag { message, .. } => message.as_str(),
                            _ => "Warning",
                        }
                    ));
                }
            }
            md.push('\n');
        }

        md.push_str(&format!(
            "### Summary\n- Deny violations: {}\n- Review required: {}\n- Flags: {}\n",
            result.counts.deny, result.counts.review, result.counts.flag
        ));

        md
    }

    async fn format_status_line(&self, result: &PolicyResult) -> String {
        if result.violations.is_empty() {
            "✅ No violations found".to_string()
        } else if result.has_blocking_violations {
            format!(
                "❌ {} blocking violation(s), {} warning(s)",
                result.counts.deny,
                result.counts.review + result.counts.flag
            )
        } else {
            format!(
                "⚠️ {} warning(s) found (no blocking violations)",
                result.counts.review + result.counts.flag
            )
        }
    }

    async fn escape_annotation(&self, text: &str) -> String {
        text.replace('%', "%25")
            .replace('\n', "%0A")
            .replace('\r', "%0D")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_evaluator::domain::{PolicyMetadata, Severity, ViolationCounts};

    #[tokio::test]
    async fn test_format_no_violations() {
        let reporter = PolicyReportGenerationServiceImpl;
        let result = PolicyResult::new(
            vec![],
            false,
            PolicyMetadata {
                policy_version: "1.0.0".to_string(),
                org_policy_merged: false,
                deny_rule_count: 0,
                review_rule_count: 0,
                flag_rule_count: 0,
            },
        );
        let input = GenerateReportInput {
            result,
            diff: crate::diff_analyzer::domain::PrDiff {
                files: vec![],
                total_size_bytes: 0,
                excluded_files: vec![],
                limits_exceeded: false,
                policy_modified: false,
                ai_signals: None,
                metadata: None,
            },
            github_format: Some(true),
            include_violations: Some(true),
        };
        let output = reporter.generate_report(input).await.unwrap();
        assert!(output.annotations.is_empty());
        assert!(output.markdown_summary.contains("Passed"));
    }

    #[tokio::test]
    async fn test_format_deny_annotation() {
        let reporter = PolicyReportGenerationServiceImpl;
        let violation = PolicyViolation::Deny {
            rule: "no-sql".to_string(),
            description: "No raw SQL".to_string(),
            severity: Severity::Critical,
            file: "db/migrate.sql".to_string(),
            message: "Blocked by no-sql".to_string(),
        };
        let annotation = reporter.format_annotation(&violation).await;
        assert!(annotation.starts_with("::error"));
        assert!(annotation.contains("db/migrate.sql"));
        assert!(annotation.contains("deny"));
    }

    #[tokio::test]
    async fn test_escape_annotation() {
        let reporter = PolicyReportGenerationServiceImpl;
        let escaped = reporter
            .escape_annotation("100% done\nnew line\r")
            .await;
        assert_eq!(escaped, "100%25 done%0Anew line%0D");
    }

    #[tokio::test]
    async fn test_markdown_with_violations() {
        let reporter = PolicyReportGenerationServiceImpl;
        let violations = vec![
            PolicyViolation::Deny {
                rule: "no-sql".to_string(),
                description: "Desc".to_string(),
                severity: Severity::Critical,
                file: "db/migrate.sql".to_string(),
                message: "Blocked".to_string(),
            },
            PolicyViolation::Flag {
                rule: "big-file".to_string(),
                description: "Desc".to_string(),
                file: "src/main.rs".to_string(),
                message: "Flagged".to_string(),
            },
        ];
        let result = PolicyResult::new(
            violations,
            false,
            PolicyMetadata {
                policy_version: "1.0.0".to_string(),
                org_policy_merged: false,
                deny_rule_count: 1,
                review_rule_count: 0,
                flag_rule_count: 1,
            },
        );
        let md = reporter.format_markdown_summary(&result).await;
        assert!(md.contains("Blocking"));
        assert!(md.contains("no-sql"));
        assert!(md.contains("Flagged"));
    }

    #[tokio::test]
    async fn test_status_line() {
        let reporter = PolicyReportGenerationServiceImpl;
        let empty = PolicyResult::new(
            vec![],
            false,
            PolicyMetadata {
                policy_version: "1.0.0".to_string(),
                org_policy_merged: false,
                deny_rule_count: 0,
                review_rule_count: 0,
                flag_rule_count: 0,
            },
        );
        assert!(reporter.format_status_line(&empty).await.contains("No violations"));
    }
}

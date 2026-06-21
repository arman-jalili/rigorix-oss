//! Implementation of `PolicyTamperDetectionService`.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#loader
//! Issue: issue-policyloader

use async_trait::async_trait;

use crate::diff_analyzer::domain::PrDiff;
use crate::policy_evaluator::domain::PolicyError;

use super::dto::{DetectTamperInput, DetectTamperOutput};
use super::service::PolicyTamperDetectionService;

/// Default implementation of `PolicyTamperDetectionService`.
pub struct PolicyTamperDetectionServiceImpl;

#[async_trait]
impl PolicyTamperDetectionService for PolicyTamperDetectionServiceImpl {
    async fn detect(&self, input: DetectTamperInput) -> Result<DetectTamperOutput, PolicyError> {
        let status = self
            .get_change_status(&input.diff, &input.policy_path)
            .await;
        let tamper_detected = status.is_some();

        Ok(DetectTamperOutput {
            tamper_detected,
            tampered_path: if tamper_detected {
                Some(input.policy_path.clone())
            } else {
                None
            },
            change_status: status,
            proceed: true, // Warn-only by default for fail-open
        })
    }

    async fn is_policy_file(&self, file_path: &str, policy_path: &str) -> bool {
        let normalized_file = file_path.trim_start_matches("./");
        let normalized_policy = policy_path.trim_start_matches("./");
        normalized_file == normalized_policy
    }

    async fn get_change_status(&self, diff: &PrDiff, file_path: &str) -> Option<String> {
        for file in &diff.files {
            if self.is_policy_file(&file.path, file_path).await {
                return Some(match file.status {
                    crate::diff_analyzer::domain::FileStatus::Added => "added".to_string(),
                    crate::diff_analyzer::domain::FileStatus::Modified => "modified".to_string(),
                    crate::diff_analyzer::domain::FileStatus::Deleted => "deleted".to_string(),
                    crate::diff_analyzer::domain::FileStatus::Renamed { .. } => {
                        "renamed".to_string()
                    }
                });
            }
        }
        None
    }

    async fn tamper_warning(&self, policy_path: &str) -> String {
        format!(
            "⚠️ Policy file '{}' has been modified in this PR — requires admin review.",
            policy_path
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_analyzer::domain::{ChangedFile, FileStatus, PrDiff};

    fn make_diff_with_file(file_path: &str, status: FileStatus) -> PrDiff {
        PrDiff {
            files: vec![ChangedFile {
                path: file_path.to_string(),
                status,
                additions: 1,
                deletions: 0,
                is_binary: false,
                hunks: vec![],
                risk: crate::diff_analyzer::domain::FileRisk::Low,
                raw_diff: None,
            }],
            total_size_bytes: 100,
            excluded_files: vec![],
            limits_exceeded: false,
            policy_modified: false,
            ai_signals: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_tamper_detected() {
        let detector = PolicyTamperDetectionServiceImpl;
        let diff = make_diff_with_file(".rigorix/policy.toml", FileStatus::Modified);
        let input = DetectTamperInput {
            diff,
            policy_path: ".rigorix/policy.toml".to_string(),
        };
        let result = detector.detect(input).await.unwrap();
        assert!(result.tamper_detected);
        assert_eq!(result.change_status.as_deref(), Some("modified"));
    }

    #[tokio::test]
    async fn test_no_tamper() {
        let detector = PolicyTamperDetectionServiceImpl;
        let diff = make_diff_with_file("src/main.rs", FileStatus::Modified);
        let input = DetectTamperInput {
            diff,
            policy_path: ".rigorix/policy.toml".to_string(),
        };
        let result = detector.detect(input).await.unwrap();
        assert!(!result.tamper_detected);
        assert!(result.tampered_path.is_none());
    }

    #[tokio::test]
    async fn test_tamper_with_path_normalization() {
        let detector = PolicyTamperDetectionServiceImpl;
        assert!(
            detector
                .is_policy_file(".rigorix/policy.toml", ".rigorix/policy.toml")
                .await
        );
        assert!(
            detector
                .is_policy_file("./.rigorix/policy.toml", ".rigorix/policy.toml")
                .await
        );
        assert!(
            !detector
                .is_policy_file(".rigorix/other.toml", ".rigorix/policy.toml")
                .await
        );
    }

    #[tokio::test]
    async fn test_tamper_warning_message() {
        let detector = PolicyTamperDetectionServiceImpl;
        let msg = detector.tamper_warning(".rigorix/policy.toml").await;
        assert!(msg.contains("policy.toml"));
        assert!(msg.contains("admin review"));
    }
}

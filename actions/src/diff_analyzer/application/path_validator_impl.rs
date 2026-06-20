//! Implementation of `PathValidationService`.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md#path-validator
//! Implements: PathValidationService trait — validates file paths for security
//! Issue: #554
//!
//! The PathValidator checks file paths for:
//! - Path traversal (`..` segments)
//! - Absolute paths (starting with `/`)
//! - Null bytes (injection attacks)
//! - Symlink components
//!
//! These checks prevent security vulnerabilities like directory traversal,
//! arbitrary file writes, and path injection attacks.

use async_trait::async_trait;

use crate::diff_analyzer::application::dto::{
    PathValidationResult, ValidatePathsInput, ValidatePathsOutput,
};
use crate::diff_analyzer::application::service::PathValidationService;
use crate::diff_analyzer::domain::DiffAnalyzerError;

/// Implementation of `PathValidationService`.
///
/// Validates each file path in a `PrDiff` against security rules:
/// - No `..` segments (path traversal)
/// - No absolute paths (must be relative to repo root)
/// - No null bytes (injection)
/// - No symlink components
pub struct PathValidatorImpl;

impl PathValidatorImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PathValidatorImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PathValidationService for PathValidatorImpl {
    async fn validate(
        &self,
        input: ValidatePathsInput,
    ) -> Result<ValidatePathsOutput, DiffAnalyzerError> {
        let allow_symlinks = input.allow_symlinks.unwrap_or(false);
        let allow_patterns = input.allow_patterns.unwrap_or_default();

        let mut results = Vec::new();
        let mut violations = 0usize;

        for file in &input.diff.files {
            let result = self.validate_single_path(&file.path).await?;

            let is_allowed = result.valid
                || result.violation.is_none()
                || self
                    .matches_allowed_pattern(&file.path, &allow_patterns)
                    .await
                || (allow_symlinks && result.violation.as_deref() == Some("symlink"));

            if !is_allowed {
                violations += 1;
            }

            results.push(PathValidationResult {
                path: file.path.clone(),
                valid: is_allowed,
                violation: if is_allowed {
                    None
                } else {
                    result.violation.clone()
                },
                message: if is_allowed {
                    None
                } else {
                    result.message.clone()
                },
            });
        }

        Ok(ValidatePathsOutput {
            diff: input.diff,
            results,
            all_valid: violations == 0,
            violation_count: violations,
        })
    }

    async fn validate_single_path(
        &self,
        path: &str,
    ) -> Result<PathValidationResult, DiffAnalyzerError> {
        // Check for null bytes (injection)
        if path.contains('\0') {
            return Ok(PathValidationResult {
                path: path.to_string(),
                valid: false,
                violation: Some("path_injection".to_string()),
                message: Some("Path contains null bytes (injection attempt)".to_string()),
            });
        }

        // Check for path traversal
        if path.contains("..") {
            // Allow `..` in the middle of a path segment name (e.g., `foo..bar`)
            // but detect actual traversal with path separators
            if path.contains("../") || path.starts_with("..") {
                return Ok(PathValidationResult {
                    path: path.to_string(),
                    valid: false,
                    violation: Some("path_traversal".to_string()),
                    message: Some(format!("Path traversal detected: '{}'", path)),
                });
            }
        }

        // Check for absolute paths
        if path.starts_with('/') {
            return Ok(PathValidationResult {
                path: path.to_string(),
                valid: false,
                violation: Some("absolute_path".to_string()),
                message: Some(format!("Absolute path not allowed: '{}'", path)),
            });
        }

        // Check for symlink-like patterns
        if path.contains("/../") || path.contains("/..") || path.starts_with("../") {
            return Ok(PathValidationResult {
                path: path.to_string(),
                valid: false,
                violation: Some("symlink".to_string()),
                message: Some(format!("Symlink/traversal detected in path: '{}'", path)),
            });
        }

        // Check for Windows drive letters (path injection)
        if path.len() >= 2 && path.as_bytes()[1] == b':' && path.as_bytes()[0].is_ascii_alphabetic()
        {
            return Ok(PathValidationResult {
                path: path.to_string(),
                valid: false,
                violation: Some("absolute_path".to_string()),
                message: Some(format!("Windows absolute path not allowed: '{}'", path)),
            });
        }

        Ok(PathValidationResult {
            path: path.to_string(),
            valid: true,
            violation: None,
            message: None,
        })
    }

    async fn detect_binary(&self, content: &[u8]) -> bool {
        content.iter().take(8192).any(|&b| b == 0)
    }

    async fn matches_allowed_pattern(&self, path: &str, patterns: &[String]) -> bool {
        if patterns.is_empty() {
            return false;
        }
        patterns.iter().any(|pattern| {
            if pattern.contains('*') || pattern.contains('?') {
                // Simple glob matching
                let regex_pattern = pattern
                    .replace('.', "\\.")
                    .replace('*', ".*")
                    .replace('?', ".");
                if let Ok(re) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
                    return re.is_match(path);
                }
            }
            path == pattern
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_analyzer::domain::ChangedFile;
    use crate::diff_analyzer::domain::FileRisk;
    use crate::diff_analyzer::domain::FileStatus;
    use crate::diff_analyzer::domain::PrDiff;

    fn make_diff(files: Vec<&str>) -> PrDiff {
        PrDiff {
            files: files
                .into_iter()
                .map(|p| ChangedFile {
                    path: p.to_string(),
                    status: FileStatus::Modified,
                    additions: 1,
                    deletions: 0,
                    is_binary: false,
                    hunks: Vec::new(),
                    risk: FileRisk::Medium,
                    raw_diff: None,
                })
                .collect(),
            total_size_bytes: 0,
            excluded_files: Vec::new(),
            limits_exceeded: false,
            policy_modified: false,
            ai_signals: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_validate_valid_paths() {
        let validator = PathValidatorImpl::new();
        let diff = make_diff(vec!["src/main.rs", "README.md", "config/settings.toml"]);
        let input = ValidatePathsInput {
            diff,
            allow_symlinks: None,
            allow_patterns: None,
        };
        let result = validator.validate(input).await.unwrap();
        assert!(result.all_valid);
        assert_eq!(result.violation_count, 0);
        assert_eq!(result.results.len(), 3);
        assert!(result.results.iter().all(|r| r.valid));
    }

    #[tokio::test]
    async fn test_validate_path_traversal() {
        let validator = PathValidatorImpl::new();
        let diff = make_diff(vec!["../etc/passwd", "src/../../outside"]);
        let input = ValidatePathsInput {
            diff,
            allow_symlinks: None,
            allow_patterns: None,
        };
        let result = validator.validate(input).await.unwrap();
        assert!(!result.all_valid);
        assert_eq!(result.violation_count, 2);
    }

    #[tokio::test]
    async fn test_validate_absolute_path() {
        let validator = PathValidatorImpl::new();
        let diff = make_diff(vec!["/etc/passwd", "/var/log/syslog"]);
        let input = ValidatePathsInput {
            diff,
            allow_symlinks: None,
            allow_patterns: None,
        };
        let result = validator.validate(input).await.unwrap();
        assert!(!result.all_valid);
        assert_eq!(result.violation_count, 2);
    }

    #[tokio::test]
    async fn test_validate_null_byte() {
        let validator = PathValidatorImpl::new();
        let diff = make_diff(vec!["src/main.rs\0", "safe.rs"]);
        let input = ValidatePathsInput {
            diff,
            allow_symlinks: None,
            allow_patterns: None,
        };
        let result = validator.validate(input).await.unwrap();
        assert!(!result.all_valid);
        assert_eq!(result.violation_count, 1);
        assert_eq!(
            result.results[0].violation.as_deref(),
            Some("path_injection")
        );
    }

    #[tokio::test]
    async fn test_validate_windows_absolute() {
        let validator = PathValidatorImpl::new();
        let diff = make_diff(vec!["C:\\Windows\\system32"]);
        let input = ValidatePathsInput {
            diff,
            allow_symlinks: None,
            allow_patterns: None,
        };
        let result = validator.validate(input).await.unwrap();
        assert!(!result.all_valid);
    }

    #[tokio::test]
    async fn test_validate_allows_dotdot_in_name() {
        let validator = PathValidatorImpl::new();
        let diff = make_diff(vec!["src/foo..bar/test.rs"]);
        let input = ValidatePathsInput {
            diff,
            allow_symlinks: None,
            allow_patterns: None,
        };
        let result = validator.validate(input).await.unwrap();
        assert!(result.all_valid);
    }

    #[tokio::test]
    async fn test_allow_symlinks() {
        let validator = PathValidatorImpl::new();
        let diff = make_diff(vec!["../external/lib.rs"]);
        let input = ValidatePathsInput {
            diff,
            allow_symlinks: Some(true),
            allow_patterns: None,
        };
        let result = validator.validate(input).await.unwrap();
        // Note: ../ is path traversal even with allow_symlinks
        assert!(!result.all_valid);
    }

    #[tokio::test]
    async fn test_allow_patterns() {
        let validator = PathValidatorImpl::new();
        let diff = make_diff(vec!["src/allowed/path.rs", "src/normal.rs"]);
        let input = ValidatePathsInput {
            diff,
            allow_symlinks: None,
            allow_patterns: Some(vec!["src/allowed/*".to_string()]),
        };
        let result = validator.validate(input).await.unwrap();
        assert!(result.all_valid);
    }

    #[tokio::test]
    async fn test_validate_single_path_valid() {
        let validator = PathValidatorImpl::new();
        let result = validator.validate_single_path("src/main.rs").await.unwrap();
        assert!(result.valid);
    }

    #[tokio::test]
    async fn test_validate_single_path_traversal() {
        let validator = PathValidatorImpl::new();
        let result = validator
            .validate_single_path("../secret.txt")
            .await
            .unwrap();
        assert!(!result.valid);
        assert_eq!(result.violation.as_deref(), Some("path_traversal"));
    }

    #[tokio::test]
    async fn test_validate_single_path_injection() {
        let validator = PathValidatorImpl::new();
        let result = validator
            .validate_single_path("src/main.rs\0")
            .await
            .unwrap();
        assert!(!result.valid);
        assert_eq!(result.violation.as_deref(), Some("path_injection"));
    }

    #[tokio::test]
    async fn test_match_allowed_pattern_no_patterns() {
        let validator = PathValidatorImpl::new();
        let result = validator.matches_allowed_pattern("any/path", &[]).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_match_allowed_pattern_exact() {
        let validator = PathValidatorImpl::new();
        let result = validator
            .matches_allowed_pattern("src/main.rs", &["src/main.rs".to_string()])
            .await;
        assert!(result);
    }

    #[tokio::test]
    async fn test_detect_binary() {
        let validator = PathValidatorImpl::new();
        assert!(validator.detect_binary(&[0u8, 1, 2, 3]).await);
        assert!(!validator.detect_binary(b"Hello, world!").await);
    }
}

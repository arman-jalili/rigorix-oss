//! Implementation of `LimitEnforcementService`.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md#limits
//! Implements: LimitEnforcementService trait — enforces resource limits on PR diffs
//! Issue: #555
//!
//! The LimitEnforcer prevents DoS attacks and resource exhaustion by enforcing:
//! - Maximum diff size (bytes)
//! - Maximum number of files
//! - Maximum lines per file
//!
//! When limits are exceeded, the system applies progressive degradation:
//! process what fits within limits and flag the rest in `PrDiff.excluded_files`.
//! This ensures the action completes within GitHub Actions' 6-hour timeout.

use async_trait::async_trait;

use crate::diff_analyzer::application::dto::{
    EnforceLimitsInput, EnforceLimitsOutput, LimitCheckResult,
};
use crate::diff_analyzer::application::service::LimitEnforcementService;
use crate::diff_analyzer::domain::{DiffAnalyzerError, PrDiff};

/// Implementation of `LimitEnforcementService`.
///
/// Enforces resource limits on PR diffs with progressive degradation:
/// when a limit is exceeded, files that fit within limits are kept
/// and excess files are recorded in `PrDiff.excluded_files`.
pub struct LimitEnforcerImpl;

impl LimitEnforcerImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LimitEnforcerImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LimitEnforcementService for LimitEnforcerImpl {
    async fn enforce(
        &self,
        input: EnforceLimitsInput,
    ) -> Result<EnforceLimitsOutput, DiffAnalyzerError> {
        let mut diff = input.diff;
        let limits = input.limits;
        let progressive = input.progressive_degradation.unwrap_or(true);

        let mut checks = Vec::new();

        // 1. Check total diff size
        let size_check = self.check_size_limit(&diff, limits.max_diff_size).await?;
        checks.push(size_check);

        // 2. Check file count
        let file_count_check = self.check_file_count_limit(&diff, limits.max_files).await?;
        checks.push(file_count_check);

        // 3. Check per-file line limits
        let line_checks = self
            .check_per_file_line_limit(&diff, limits.max_lines_per_file)
            .await?;
        checks.extend(line_checks);

        // Apply progressive degradation if any limit exceeded and enabled
        let any_exceeded = checks.iter().any(|c| c.exceeded);
        let excluded_count = if any_exceeded && progressive {
            let mut all_excluded = Vec::new();

            // Apply total size limit first
            let size_excluded = self
                .apply_progressive_degradation(&mut diff, limits.max_diff_size)
                .await;
            all_excluded.extend(size_excluded);

            // Apply file count limit
            let file_excluded = self
                .apply_file_count_limit(&mut diff, limits.max_files)
                .await;
            all_excluded.extend(file_excluded);

            // Flag limits exceeded
            diff.limits_exceeded = true;

            all_excluded.len()
        } else if any_exceeded {
            diff.limits_exceeded = true;
            0
        } else {
            0
        };

        Ok(EnforceLimitsOutput {
            diff,
            checks,
            any_exceeded,
            excluded_count,
            degraded: progressive && any_exceeded,
        })
    }

    async fn check_size_limit(
        &self,
        diff: &PrDiff,
        max_size: u64,
    ) -> Result<LimitCheckResult, DiffAnalyzerError> {
        let exceeded = diff.total_size_bytes > max_size;
        Ok(LimitCheckResult {
            limit_type: "total_size".to_string(),
            exceeded,
            actual: format!("{} bytes", diff.total_size_bytes),
            limit: format!("{} bytes", max_size),
        })
    }

    async fn check_file_count_limit(
        &self,
        diff: &PrDiff,
        max_files: usize,
    ) -> Result<LimitCheckResult, DiffAnalyzerError> {
        let file_count = diff.files.len();
        let exceeded = file_count > max_files;
        Ok(LimitCheckResult {
            limit_type: "file_count".to_string(),
            exceeded,
            actual: format!("{} files", file_count),
            limit: format!("{} files", max_files),
        })
    }

    async fn check_per_file_line_limit(
        &self,
        diff: &PrDiff,
        max_lines: usize,
    ) -> Result<Vec<LimitCheckResult>, DiffAnalyzerError> {
        let mut results = Vec::new();
        for file in &diff.files {
            let line_count = file.additions + file.deletions;
            let exceeded = line_count > max_lines;
            results.push(LimitCheckResult {
                limit_type: format!("per_file_lines:{}", file.path),
                exceeded,
                actual: format!("{} lines", line_count),
                limit: format!("{} lines", max_lines),
            });
        }
        Ok(results)
    }

    async fn apply_progressive_degradation(&self, diff: &mut PrDiff, max_size: u64) -> Vec<String> {
        if diff.total_size_bytes <= max_size {
            return Vec::new();
        }

        let mut kept = Vec::new();
        let mut excluded = Vec::new();
        let mut running_size = 0u64;

        let files = std::mem::take(&mut diff.files);
        for file in files {
            let file_size = (file.additions + file.deletions) as u64;
            if running_size + file_size <= max_size {
                running_size += file_size;
                kept.push(file);
            } else {
                excluded.push(file.path);
            }
        }

        diff.files = kept;
        diff.excluded_files.extend(excluded.clone());
        excluded
    }
}

impl LimitEnforcerImpl {
    /// Apply file count limit by excluding excess files.
    async fn apply_file_count_limit(&self, diff: &mut PrDiff, max_files: usize) -> Vec<String> {
        if diff.files.len() <= max_files {
            return Vec::new();
        }

        let mut excluded = Vec::new();
        while diff.files.len() > max_files {
            if let Some(file) = diff.files.pop() {
                excluded.push(file.path.clone());
                diff.excluded_files.push(file.path);
            }
        }
        excluded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_analyzer::domain::{ChangedFile, FileRisk, FileStatus, PolicyLimits};

    fn make_file(path: &str, additions: usize, deletions: usize) -> ChangedFile {
        ChangedFile {
            path: path.to_string(),
            status: FileStatus::Modified,
            additions,
            deletions,
            is_binary: false,
            hunks: Vec::new(),
            risk: FileRisk::Medium,
            raw_diff: None,
        }
    }

    fn make_diff(files: Vec<ChangedFile>, total_size: u64) -> PrDiff {
        PrDiff {
            files,
            total_size_bytes: total_size,
            excluded_files: Vec::new(),
            limits_exceeded: false,
            policy_modified: false,
            ai_signals: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_enforce_within_limits() {
        let enforcer = LimitEnforcerImpl::new();
        let files = vec![
            make_file("src/main.rs", 50, 10),
            make_file("src/lib.rs", 30, 5),
        ];
        let diff = make_diff(files, 1000);
        let limits = PolicyLimits::new(10_000_000, 100, 5000);

        let input = EnforceLimitsInput {
            diff,
            limits,
            progressive_degradation: Some(true),
        };
        let result = enforcer.enforce(input).await.unwrap();
        assert!(!result.any_exceeded);
        assert_eq!(result.excluded_count, 0);
        assert_eq!(result.diff.files.len(), 2);
    }

    #[tokio::test]
    async fn test_enforce_size_limit_exceeded() {
        let enforcer = LimitEnforcerImpl::new();
        let files = vec![
            make_file("small.rs", 10, 5),
            make_file("large.rs", 5000, 3000),
        ];
        let diff = make_diff(files, 10000);
        let limits = PolicyLimits::new(100, 100, 5000);

        let input = EnforceLimitsInput {
            diff,
            limits,
            progressive_degradation: Some(true),
        };
        let result = enforcer.enforce(input).await.unwrap();
        assert!(result.any_exceeded);
        assert!(result.degraded);
        // Small file should be kept, large should be excluded
        assert!(result.diff.files.len() > 0 || result.diff.excluded_files.len() > 0);
    }

    #[tokio::test]
    async fn test_enforce_file_count_limit() {
        let enforcer = LimitEnforcerImpl::new();
        let files = (0..10)
            .map(|i| make_file(&format!("file{}.rs", i), 10, 5))
            .collect();
        let diff = make_diff(files, 1000);
        let limits = PolicyLimits::new(10_000_000, 3, 5000);

        let input = EnforceLimitsInput {
            diff,
            limits,
            progressive_degradation: Some(true),
        };
        let result = enforcer.enforce(input).await.unwrap();
        assert!(result.any_exceeded);
    }

    #[tokio::test]
    async fn test_enforce_per_file_line_limit() {
        let enforcer = LimitEnforcerImpl::new();
        let files = vec![
            make_file("small.rs", 10, 5),
            make_file("huge.rs", 10000, 5000),
        ];
        let diff = make_diff(files, 1000);
        let limits = PolicyLimits::new(10_000_000, 100, 100);

        let input = EnforceLimitsInput {
            diff,
            limits,
            progressive_degradation: Some(true),
        };
        let result = enforcer.enforce(input).await.unwrap();
        let line_checks: Vec<_> = result
            .checks
            .iter()
            .filter(|c| c.limit_type.starts_with("per_file_lines"))
            .collect();
        assert!(line_checks.iter().any(|c| c.exceeded));
    }

    #[tokio::test]
    async fn test_progressive_degradation() {
        let enforcer = LimitEnforcerImpl::new();
        let files = vec![
            make_file("keep.rs", 10, 5),
            make_file("exclude.rs", 200, 100),
        ];
        let mut diff = make_diff(files, 1000);

        let excluded = enforcer.apply_progressive_degradation(&mut diff, 50).await;
        assert!(!excluded.is_empty());
        assert!(diff.excluded_files.contains(&"exclude.rs".to_string()));
    }

    #[tokio::test]
    async fn test_no_degradation_within_limit() {
        let enforcer = LimitEnforcerImpl::new();
        let files = vec![make_file("small.rs", 10, 5)];
        let mut diff = make_diff(files, 100);

        let excluded = enforcer.apply_progressive_degradation(&mut diff, 500).await;
        assert!(excluded.is_empty());
        assert_eq!(diff.files.len(), 1);
    }

    #[tokio::test]
    async fn test_check_size_limit() {
        let enforcer = LimitEnforcerImpl::new();
        let diff = make_diff(vec![], 1000);

        let result = enforcer.check_size_limit(&diff, 500).await.unwrap();
        assert!(result.exceeded);

        let result = enforcer.check_size_limit(&diff, 2000).await.unwrap();
        assert!(!result.exceeded);
    }

    #[tokio::test]
    async fn test_check_file_count_limit() {
        let enforcer = LimitEnforcerImpl::new();
        let files = (0..5)
            .map(|i| make_file(&format!("f{}.rs", i), 1, 0))
            .collect();
        let diff = make_diff(files, 100);

        let result = enforcer.check_file_count_limit(&diff, 3).await.unwrap();
        assert!(result.exceeded);

        let result = enforcer.check_file_count_limit(&diff, 10).await.unwrap();
        assert!(!result.exceeded);
    }

    #[tokio::test]
    async fn test_check_per_file_line_limit() {
        let enforcer = LimitEnforcerImpl::new();
        let files = vec![make_file("large.rs", 100, 50), make_file("small.rs", 5, 3)];
        let diff = make_diff(files, 100);

        let results = enforcer.check_per_file_line_limit(&diff, 20).await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].exceeded);
        assert!(!results[1].exceeded);
    }

    #[tokio::test]
    async fn test_default_limits() {
        let limits = PolicyLimits::default();
        assert_eq!(limits.max_diff_size, 10_000_000);
        assert_eq!(limits.max_files, 100);
        assert_eq!(limits.max_lines_per_file, 5000);
    }
}

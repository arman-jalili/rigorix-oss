//! Service interfaces (use cases) for the Diff Analyzer bounded context.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md
//! Implements: Contract Freeze — DiffParsingService, PathValidationService,
//! LimitEnforcementService, RiskClassificationService,
//! AiSignalDetectionService, DiffAnalysisPipelineService traits
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for parsing PR diffs,
//! validating file paths, enforcing resource limits, classifying risk,
//! and detecting AI-generated code signals. All methods are async and
//! return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::diff_analyzer::domain::{DiffAnalyzerError, DiffHunk, PrDiff};

use super::dto::{
    AnalyzeDiffInput, AnalyzeDiffOutput, ClassifyRiskInput, ClassifyRiskOutput,
    DetectAiSignalsInput, DetectAiSignalsOutput, EnforceLimitsInput, EnforceLimitsOutput,
    ParseDiffInput, ParseDiffOutput, ValidatePathsInput, ValidatePathsOutput,
};

/// Application service for parsing raw git diff output into structured `PrDiff`.
///
/// Implements the contract defined in `DiffParser` from the architecture doc.
/// Splits raw diff output by `diff --git` headers, extracts hunks with
/// line numbers, and detects binary files.
///
/// # Contract (Frozen)
/// - `parse()` is the primary entry point
/// - Accepts raw unified diff output (as returned by `git diff`)
/// - Detects binary files via NUL byte in first 8KB
/// - Returns structured `DiffParseResult` even on partial parse failures
#[async_trait]
pub trait DiffParsingService: Send + Sync {
    /// Parse raw git diff output into a structured `PrDiff`.
    ///
    /// Splits the raw diff by `diff --git a/... b/...` headers.
    /// Each file section is parsed into a `ChangedFile` with its hunks.
    /// Binary files are detected and flagged.
    ///
    /// # Returns
    ///
    /// `ParseDiffOutput` containing:
    /// - The parsed diff result (may be partial on non-fatal errors)
    /// - Whether binary files were detected
    /// - The detected encoding
    async fn parse(&self, input: ParseDiffInput) -> Result<ParseDiffOutput, DiffAnalyzerError>;

    /// Parse a single file's diff section from the raw output.
    ///
    /// Extracts the file header, hunks, and metadata from a single
    /// `diff --git` block.
    async fn parse_file_section(
        &self,
        section: &str,
    ) -> Result<crate::diff_analyzer::domain::ChangedFile, DiffAnalyzerError>;

    /// Parse a single diff hunk header and its content lines.
    ///
    /// Handles `@@ -old_start,old_lines +new_start,new_lines @@ header` format.
    async fn parse_hunk(&self, hunk_text: &str) -> Result<DiffHunk, DiffAnalyzerError>;

    /// Detect whether a file is binary from its content.
    ///
    /// Checks for NUL bytes in the first 8192 bytes of content.
    async fn detect_binary(&self, content: &[u8]) -> bool;

    /// Extract the file paths from a `diff --git` header line.
    ///
    /// Returns `(old_path, new_path)`.
    async fn extract_paths(&self, header: &str) -> Result<(String, String), DiffAnalyzerError>;
}

/// Application service for validating file paths in a PR diff.
///
/// Implements the `PathValidator` component from the architecture doc.
/// Validates all file paths for security violations:
/// - Path traversal (`../`)
/// - Absolute paths (starting with `/`)
/// - Null bytes (path injection)
/// - Symlink components
///
/// # Contract (Frozen)
/// - `validate()` validates all paths in a diff
/// - Returns individual validation results for each path
/// - Security violations are blocking errors (not warnings)
#[async_trait]
pub trait PathValidationService: Send + Sync {
    /// Validate all file paths in a PR diff.
    ///
    /// Checks each file path for:
    /// - Path traversal (`..` segments)
    /// - Absolute paths (starting with `/`)
    /// - Null bytes (injection attacks)
    /// - Symlink components
    ///
    /// Returns individual results per path with violation details.
    async fn validate(&self, input: ValidatePathsInput) -> Result<ValidatePathsOutput, DiffAnalyzerError>;

    /// Validate a single file path.
    ///
    /// Returns a `PathValidationResult` indicating pass/fail with violation details.
    async fn validate_single_path(
        &self,
        path: &str,
    ) -> Result<crate::diff_analyzer::application::dto::PathValidationResult, DiffAnalyzerError>;

    /// Detect whether a file is binary from its first bytes.
    ///
    /// A file is binary if a NUL byte (0x00) is found within the first 8192 bytes.
    async fn detect_binary(&self, content: &[u8]) -> bool;

    /// Check if a path matches any allowed pattern.
    async fn matches_allowed_pattern(&self, path: &str, patterns: &[String]) -> bool;
}

/// Application service for enforcing resource limits on PR diffs.
///
/// Implements the `LimitEnforcer` component from the architecture doc.
/// Prevents DoS attacks via massive diffs by enforcing:
/// - Maximum diff size (bytes)
/// - Maximum number of files
/// - Maximum lines per file
///
/// When limits are exceeded, the system applies progressive degradation:
/// process what fits within limits and flag the rest in `PrDiff.excluded_files`.
///
/// # Contract (Frozen)
/// - `enforce()` applies all configured limits
/// - Progressive degradation is the default behavior
/// - Returns detailed check results for transparency
#[async_trait]
pub trait LimitEnforcementService: Send + Sync {
    /// Enforce resource limits on a PR diff.
    ///
    /// Applies all configured limits in order:
    /// 1. Total diff size limit
    /// 2. Maximum files limit
    /// 3. Per-file line limits
    ///
    /// When a limit is exceeded with progressive degradation enabled,
    /// files that fit within the limit are kept and excess files are
    /// excluded (recorded in `PrDiff.excluded_files`).
    async fn enforce(&self, input: EnforceLimitsInput) -> Result<EnforceLimitsOutput, DiffAnalyzerError>;

    /// Check total diff size against the configured limit.
    ///
    /// Returns a limit check result with actual size and limit.
    async fn check_size_limit(
        &self,
        diff: &PrDiff,
        max_size: u64,
    ) -> Result<crate::diff_analyzer::application::dto::LimitCheckResult, DiffAnalyzerError>;

    /// Check the number of files against the configured limit.
    async fn check_file_count_limit(
        &self,
        diff: &PrDiff,
        max_files: usize,
    ) -> Result<crate::diff_analyzer::application::dto::LimitCheckResult, DiffAnalyzerError>;

    /// Check per-file line count against the configured limit.
    async fn check_per_file_line_limit(
        &self,
        diff: &PrDiff,
        max_lines: usize,
    ) -> Result<Vec<crate::diff_analyzer::application::dto::LimitCheckResult>, DiffAnalyzerError>;

    /// Apply progressive degradation to a diff that exceeds limits.
    ///
    /// Keeps files in order until the limit is reached, then excludes the rest.
    async fn apply_progressive_degradation(
        &self,
        diff: &mut PrDiff,
        max_size: u64,
    ) -> Vec<String>;
}

/// Application service for classifying file changes by risk level.
///
/// Implements the `RiskClassifier` component from the architecture doc.
/// Classifies files based on path patterns:
/// - **Low**: Documentation, config files, text assets
/// - **Medium**: Source code (default for most files)
/// - **High**: Migrations, SQL files, infrastructure changes
/// - **Critical**: Auth, security, secrets, access control
///
/// # Contract (Frozen)
/// - `classify()` classifies all files in a diff
/// - Supports custom override patterns
/// - Classification is based on path patterns only (not content)
#[async_trait]
pub trait RiskClassificationService: Send + Sync {
    /// Classify all files in a PR diff by risk level.
    ///
    /// Uses path-based heuristics to assign risk levels:
    /// - Paths containing `migrations/` or ending in `.sql` → High
    /// - Paths containing `auth/` or `security/` → Critical
    /// - Paths ending in `.rs`, `.ts`, `.py`, `.js`, `.go` → Medium
    /// - Paths ending in `.md`, `.txt`, `.json`, `.yaml`, `.toml` → Low
    /// - Custom patterns override defaults
    async fn classify(&self, input: ClassifyRiskInput) -> Result<ClassifyRiskOutput, DiffAnalyzerError>;

    /// Classify a single file path by risk level.
    ///
    /// Uses path pattern matching to determine risk.
    /// Returns the risk level and the pattern that matched.
    async fn classify_path(
        &self,
        path: &str,
        custom_patterns: &std::collections::HashMap<String, crate::diff_analyzer::domain::FileRisk>,
    ) -> Result<crate::diff_analyzer::application::dto::FileClassificationResult, DiffAnalyzerError>;

    /// Get the default risk level for an unknown/unclassifiable path.
    async fn default_risk(&self) -> crate::diff_analyzer::domain::FileRisk;

    /// Check if a path matches a glob pattern.
    async fn matches_pattern(&self, path: &str, pattern: &str) -> bool;
}

/// Application service for detecting AI-generated code signals in PR diffs.
///
/// Implements the `AiSignalDetector` component from the architecture doc.
/// Uses heuristic pattern matching to detect common AI-generation artifacts:
/// - Comment patterns ("Here's the implementation...", "This function...")
/// - Unusually uniform indentation
/// - Overly verbose variable names
/// - Hallucination indicators (referencing non-existent APIs)
///
/// Note: This is a heuristic, not a forensic tool. False positives are possible.
/// Results are advisory — they flag code for extra review, not block it.
///
/// # Contract (Frozen)
/// - `detect()` analyzes all non-binary hunks in a diff
/// - Returns confidence score (0.0–1.0) and individual signal list
/// - Custom patterns can be injected for domain-specific detection
/// - Results are advisory only (never blocking)
#[async_trait]
pub trait AiSignalDetectionService: Send + Sync {
    /// Detect AI-generated code signals in a PR diff.
    ///
    /// Analyzes all non-binary file hunks for common AI-generation patterns:
    /// - AI-style comment patterns (explanatory, overly verbose)
    /// - Uniform indentation (characteristic of LLM output)
    /// - Custom patterns (if provided)
    ///
    /// Returns an overall confidence score and individual signal locations.
    async fn detect(
        &self,
        input: DetectAiSignalsInput,
    ) -> Result<DetectAiSignalsOutput, DiffAnalyzerError>;

    /// Check a hunk's content for AI-style comment patterns.
    ///
    /// Looks for patterns like "Here's the", "This function", "This implementation",
    /// "As requested", "I've added", "I've created".
    async fn has_ai_comment_pattern(&self, content: &str) -> bool;

    /// Check a hunk's content for unusually uniform indentation.
    ///
    /// Indentation is considered uniform if >60% of non-empty lines
    /// share the same indentation depth (ignoring leading whitespace).
    async fn has_uniform_indentation(&self, content: &str) -> bool;

    /// Check for custom AI patterns in content.
    ///
    /// Returns the matched pattern name and confidence if any custom
    /// patterns are found.
    async fn check_custom_patterns(
        &self,
        content: &str,
        patterns: &std::collections::HashMap<String, Vec<String>>,
    ) -> Vec<(String, f64)>;

    /// Compute overall confidence score from individual signal results.
    async fn compute_confidence(&self, flagged_hunks: usize, total_hunks: usize) -> f64;
}

/// Application service for running the full diff analysis pipeline.
///
/// Orchestrates the complete analysis flow:
/// 1. Parse raw diff → `PrDiff`
/// 2. Validate file paths
/// 3. Enforce resource limits
/// 4. Classify by risk
/// 5. Detect AI signals
///
/// Returns a comprehensive analysis result with all intermediate outputs.
///
/// # Contract (Frozen)
/// - `analyze()` runs the complete pipeline
/// - Steps are executed in order; failure in earlier steps may still produce partial results
/// - Returns detailed per-step outputs for transparency
#[async_trait]
pub trait DiffAnalysisPipelineService: Send + Sync {
    /// Run the full diff analysis pipeline.
    ///
    /// Executes all analysis steps in sequence:
    /// 1. Parse the raw diff
    /// 2. Validate file paths
    /// 3. Enforce limits (with progressive degradation if needed)
    /// 4. Classify files by risk
    /// 5. Detect AI signals
    ///
    /// Each step's output is included in the result for full transparency.
    async fn analyze(&self, input: AnalyzeDiffInput) -> Result<AnalyzeDiffOutput, DiffAnalyzerError>;

    /// Run analysis with default limits and configuration.
    ///
    /// Convenience method that uses `PolicyLimits::default()` and sensible
    /// defaults for AI detection thresholds.
    async fn analyze_default(&self, raw_diff: String) -> Result<AnalyzeDiffOutput, DiffAnalyzerError>;
}

//! Data Transfer Objects for the Diff Analyzer module.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md
//! Implements: Contract Freeze — DTO schemas for diff parsing, path validation,
//! limit enforcement, risk classification, and AI signal detection
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for event processing)
//! - Validation constraints are documented in field docs

use serde::{Deserialize, Serialize};

use crate::diff_analyzer::domain::{
    AiSignalResult, DiffParseResult, FileRisk, PolicyLimits, PrDiff,
};

// ---------------------------------------------------------------------------
// Diff Parsing DTOs
// ---------------------------------------------------------------------------

/// Input for parsing a raw git diff into a structured `PrDiff`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseDiffInput {
    /// The raw git diff output (unified diff format).
    pub raw_diff: String,

    /// Override the PR number for context.
    pub pr_number: Option<u64>,

    /// The base branch name (for metadata).
    pub base_branch: Option<String>,

    /// The head branch name (for metadata).
    pub head_branch: Option<String>,

    /// The head commit SHA (for metadata).
    pub head_sha: Option<String>,

    /// Whether to detect binary files during parsing.
    pub detect_binary: Option<bool>,
}

/// Output from parsing a raw git diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseDiffOutput {
    /// The parsed diff result.
    pub result: DiffParseResult,

    /// Whether the diff contains any binary files.
    pub has_binary_files: bool,

    /// The detected encoding of the diff content.
    pub encoding: String,
}

// ---------------------------------------------------------------------------
// Path Validation DTOs
// ---------------------------------------------------------------------------

/// Input for validating file paths in a PR diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatePathsInput {
    /// The PR diff whose file paths should be validated.
    pub diff: PrDiff,

    /// Whether to allow symlink paths (default: false).
    pub allow_symlinks: Option<bool>,

    /// Additional path patterns to allow (glob patterns).
    pub allow_patterns: Option<Vec<String>>,
}

/// A single path validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathValidationResult {
    /// The file path that was validated.
    pub path: String,
    /// Whether the path passed validation.
    pub valid: bool,
    /// The violation type, if validation failed.
    pub violation: Option<String>,
    /// Human-readable error message, if validation failed.
    pub message: Option<String>,
}

/// Output from validating file paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatePathsOutput {
    /// The PR diff after path validation (unchanged, but enriched with validation state).
    pub diff: PrDiff,
    /// Individual validation results for each file path.
    pub results: Vec<PathValidationResult>,
    /// Whether all paths passed validation.
    pub all_valid: bool,
    /// Number of security violations detected.
    pub violation_count: usize,
}

// ---------------------------------------------------------------------------
// Limit Enforcement DTOs
// ---------------------------------------------------------------------------

/// Input for enforcing resource limits on a PR diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforceLimitsInput {
    /// The PR diff to enforce limits on.
    pub diff: PrDiff,

    /// The limits to enforce.
    pub limits: PolicyLimits,

    /// Whether to apply progressive degradation (default: true).
    /// When true, files that fit within limits are kept; excess files are excluded.
    pub progressive_degradation: Option<bool>,
}

/// A single limit check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitCheckResult {
    /// The limit type that was checked.
    pub limit_type: String,
    /// Whether the limit was exceeded.
    pub exceeded: bool,
    /// The actual value.
    pub actual: String,
    /// The configured limit.
    pub limit: String,
}

/// Output from enforcing resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforceLimitsOutput {
    /// The PR diff after limit enforcement.
    pub diff: PrDiff,
    /// Results of each limit check.
    pub checks: Vec<LimitCheckResult>,
    /// Whether any limits were exceeded.
    pub any_exceeded: bool,
    /// Number of files excluded due to limits.
    pub excluded_count: usize,
    /// Whether progressive degradation was applied.
    pub degraded: bool,
}

// ---------------------------------------------------------------------------
// Risk Classification DTOs
// ---------------------------------------------------------------------------

/// Input for classifying file risk levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyRiskInput {
    /// The PR diff whose files should be classified.
    pub diff: PrDiff,

    /// Custom risk patterns for classification overrides.
    /// Maps glob pattern to FileRisk level.
    pub custom_patterns: Option<std::collections::HashMap<String, FileRisk>>,
}

/// A single file classification result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileClassificationResult {
    /// The file path.
    pub path: String,
    /// The assigned risk level.
    pub risk: FileRisk,
    /// The pattern that matched (e.g., "*.rs", "migrations/").
    pub matched_pattern: Option<String>,
}

/// Output from risk classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyRiskOutput {
    /// The PR diff after risk classification (files now have risk assigned).
    pub diff: PrDiff,
    /// Classification results for each file.
    pub classifications: Vec<FileClassificationResult>,
    /// Files classified as critical risk.
    pub critical_files: Vec<String>,
    /// Files classified as high risk.
    pub high_risk_files: Vec<String>,
}

// ---------------------------------------------------------------------------
// AI Signal Detection DTOs
// ---------------------------------------------------------------------------

/// Input for detecting AI-generated code signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectAiSignalsInput {
    /// The PR diff to analyze for AI signals.
    pub diff: PrDiff,

    /// Override confidence threshold (default: 0.7).
    pub threshold: Option<f64>,

    /// Whether to include uniform indentation analysis (default: true).
    pub check_indentation: Option<bool>,

    /// Whether to include comment pattern analysis (default: true).
    pub check_comments: Option<bool>,

    /// Custom AI pattern definitions (maps pattern name to list of trigger strings).
    pub custom_patterns: Option<std::collections::HashMap<String, Vec<String>>>,
}

/// Output from AI signal detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectAiSignalsOutput {
    /// The AI signal detection results.
    pub result: AiSignalResult,
    /// Whether the confidence exceeds the configured threshold.
    pub exceeds_threshold: bool,
    /// The threshold that was applied.
    pub threshold: f64,
    /// Number of hunks analyzed.
    pub hunks_analyzed: usize,
    /// Number of hunks flagged.
    pub hunks_flagged: usize,
}

// ---------------------------------------------------------------------------
// Full Pipeline DTOs
// ---------------------------------------------------------------------------

/// Input for running the full diff analysis pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeDiffInput {
    /// The raw git diff output.
    pub raw_diff: String,

    /// Policy limits to enforce.
    pub limits: PolicyLimits,

    /// PR metadata.
    pub pr_number: Option<u64>,
    pub base_branch: Option<String>,
    pub head_branch: Option<String>,
    pub head_sha: Option<String>,

    /// AI detection configuration.
    pub ai_threshold: Option<f64>,
    pub check_indentation: Option<bool>,
    pub check_comments: Option<bool>,

    /// Custom risk patterns (maps glob to risk level).
    pub custom_risk_patterns: Option<std::collections::HashMap<String, FileRisk>>,

    /// Whether to allow symlinks.
    pub allow_symlinks: Option<bool>,

    /// Whether to apply progressive degradation on limit exceed.
    pub progressive_degradation: Option<bool>,
}

/// Output from the full diff analysis pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeDiffOutput {
    /// The fully analyzed and enriched PR diff.
    pub diff: PrDiff,

    /// Path validation summary.
    pub path_validation: ValidatePathsOutput,

    /// Limit enforcement summary.
    pub limit_enforcement: EnforceLimitsOutput,

    /// Risk classification summary.
    pub risk_classification: ClassifyRiskOutput,

    /// AI signal detection results.
    pub ai_detection: DetectAiSignalsOutput,

    /// Total processing time in milliseconds.
    pub processing_time_ms: u64,
}

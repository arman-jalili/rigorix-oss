//! Factory interfaces for constructing Diff Analyzer domain objects.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md
//! Implements: Contract Freeze — DiffFactory, LimitConfigFactory,
//! AiSignalDetectorFactory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::diff_analyzer::domain::{DiffAnalyzerError, PolicyLimits, PrDiff};

use super::dto::ParseDiffInput;

/// Factory for constructing `PrDiff` from raw diff data.
///
/// Handles parsing of raw git diff output into structured `PrDiff` objects
/// with proper metadata, binary detection, and hunk extraction.
#[async_trait]
pub trait DiffFactory: Send + Sync {
    /// Build a `PrDiff` from a raw git diff string.
    ///
    /// Parses the raw unified diff output and constructs a fully
    /// structured `PrDiff` with all files, hunks, and metadata.
    async fn build_from_raw_diff(
        &self,
        input: ParseDiffInput,
    ) -> Result<PrDiff, DiffAnalyzerError>;

    /// Create an empty `PrDiff` with no files.
    fn empty_diff(&self) -> PrDiff;

    /// Create a `PrDiff` with metadata only (no files yet).
    fn with_metadata(
        &self,
        pr_number: u64,
        base_branch: String,
        head_branch: String,
        head_sha: String,
    ) -> PrDiff;
}

/// Factory for constructing `PolicyLimits` configuration.
///
/// Handles creation of policy limits from various sources:
/// - Default values
/// - Policy file configuration
/// - Environment-specific overrides
#[async_trait]
pub trait LimitConfigFactory: Send + Sync {
    /// Build `PolicyLimits` from individual parameters.
    ///
    /// Validates that all values are positive and sensible.
    async fn build(
        &self,
        max_diff_size: u64,
        max_files: usize,
        max_lines_per_file: usize,
    ) -> Result<PolicyLimits, DiffAnalyzerError>;

    /// Create `PolicyLimits` with default values.
    ///
    /// Defaults:
    /// - max_diff_size: 10 MB
    /// - max_files: 100
    /// - max_lines_per_file: 5000
    fn defaults(&self) -> PolicyLimits;

    /// Build `PolicyLimits` from a configuration map.
    ///
    /// Expected keys: `max_diff_size`, `max_files`, `max_lines_per_file`.
    /// Values are parsed as their respective types.
    async fn build_from_config(
        &self,
        config: std::collections::HashMap<String, String>,
    ) -> Result<PolicyLimits, DiffAnalyzerError>;

    /// Merge two `PolicyLimits`, with the second taking precedence.
    async fn merge(
        &self,
        base: PolicyLimits,
        overrides: PolicyLimits,
    ) -> Result<PolicyLimits, DiffAnalyzerError>;
}

/// Factory for constructing AI signal detection configuration.
///
/// Provides default AI signal patterns and thresholds.
#[async_trait]
pub trait AiSignalDetectorFactory: Send + Sync {
    /// Get the default AI comment patterns.
    ///
    /// Returns patterns like "Here's the", "This function", "I've added", etc.
    fn default_comment_patterns(&self) -> Vec<String>;

    /// Get the default indentation uniformity threshold.
    ///
    /// Default: 0.6 (60% of lines with same indentation).
    fn default_uniformity_threshold(&self) -> f64;

    /// Get the default confidence threshold for flagging.
    ///
    /// Default: 0.7 (70% confidence).
    fn default_confidence_threshold(&self) -> f64;

    /// Build a compressed pattern map from a list of pattern strings.
    ///
    /// Groups patterns into named categories for easier reporting.
    fn build_pattern_map(
        &self,
        patterns: Vec<String>,
    ) -> std::collections::HashMap<String, Vec<String>>;
}

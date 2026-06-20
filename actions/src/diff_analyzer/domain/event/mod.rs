//! Event payload schemas for the Diff Analyzer bounded context.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md
//! Implements: Contract Freeze — DiffAnalyzerEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted on the EventBus whenever a PR diff is parsed,
//! validated, classified, or analyzed for AI signals. Consumers
//! (audit, policy evaluator, CI integration) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - Events are serializable for audit logging

use serde::{Deserialize, Serialize};

use super::types::{AiSignalResult, PolicyLimits, PrDiff};

/// Events emitted by the Diff Analyzer module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffAnalyzerEvent {
    /// A raw PR diff was successfully parsed into a `PrDiff`.
    DiffParsed {
        /// The parsed PR diff.
        diff: PrDiff,
        /// Number of files in the diff.
        file_count: usize,
        /// Total diff size in bytes.
        total_size_bytes: u64,
        /// Whether the policy file was modified in this diff.
        policy_modified: bool,
    },

    /// File path validation completed.
    PathsValidated {
        /// The PR diff after path validation.
        diff: PrDiff,
        /// Whether any path violations were detected.
        violations_detected: bool,
        /// Number of security violations found.
        violation_count: usize,
    },

    /// A path validation violation was detected.
    PathViolationDetected {
        /// The file path that caused the violation.
        path: String,
        /// The violation type (traversal, absolute, injection, symlink).
        violation_type: String,
        /// Whether this is a blocking violation.
        is_blocking: bool,
    },

    /// Resource limit enforcement applied.
    LimitsEnforced {
        /// The PR diff after limit enforcement.
        diff: PrDiff,
        /// Whether any limits were exceeded.
        limits_exceeded: bool,
        /// Number of files excluded due to limits.
        excluded_count: usize,
        /// The limits that were applied.
        limits: PolicyLimits,
    },

    /// A resource limit was exceeded.
    LimitExceeded {
        /// The limit type that was exceeded.
        limit_type: String,
        /// The actual value.
        actual: String,
        /// The configured limit value.
        limit: String,
        /// Whether progressive degradation was applied.
        degraded: bool,
    },

    /// Risk classification completed for all files.
    RiskClassified {
        /// The PR diff after risk classification.
        diff: PrDiff,
        /// Count of low-risk files.
        low_count: usize,
        /// Count of medium-risk files.
        medium_count: usize,
        /// Count of high-risk files.
        high_count: usize,
        /// Count of critical-risk files.
        critical_count: usize,
    },

    /// A file was classified as critical risk.
    CriticalChangeDetected {
        /// The critical file path.
        path: String,
        /// The file change status.
        status: String,
    },

    /// AI signal detection completed.
    AiSignalsDetected {
        /// The AI signal detection results.
        result: AiSignalResult,
        /// Whether the confidence exceeds the advisory threshold.
        exceeds_threshold: bool,
        /// The threshold that was applied.
        threshold: f64,
    },

    /// A specific AI signal pattern was detected.
    AiSignalFound {
        /// The file where the signal was found.
        file: String,
        /// The pattern name.
        pattern: String,
        /// Confidence score for this signal.
        confidence: f64,
    },

    /// An error occurred during diff analysis (non-fatal).
    AnalysisWarning {
        /// The component that produced the warning.
        component: String,
        /// Warning message.
        message: String,
    },

    /// Full diff analysis pipeline completed.
    AnalysisCompleted {
        /// The final enriched diff.
        diff: PrDiff,
        /// Total processing time in milliseconds.
        processing_time_ms: u64,
        /// Summary of all analysis results.
        summary: AnalysisSummary,
    },
}

/// Summary of a full diff analysis pipeline execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSummary {
    /// Number of files parsed.
    pub files_parsed: usize,
    /// Number of files excluded.
    pub files_excluded: usize,
    /// Number of path violations detected.
    pub path_violations: usize,
    /// Whether any limits were exceeded.
    pub limits_exceeded: bool,
    /// Whether critical risk files were found.
    pub has_critical_changes: bool,
    /// Number of AI signals detected.
    pub ai_signals_count: usize,
    /// Whether the policy file was modified.
    pub policy_modified: bool,
}

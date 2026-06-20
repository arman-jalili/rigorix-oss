//! Domain types for PR diff analysis.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md#types
//! Implements: Contract Freeze — PrDiff, ChangedFile, DiffHunk, FileStatus,
//! FileRisk, PolicyLimits, AiSignal, AiSignalResult
//! Issue: issue-contract-freeze
//!
//! These are the core domain types that represent a parsed GitHub PR diff,
//! individual changed files, diff hunks, risk classifications, and
//! AI-generated code signals. They serve as the frozen contract that all
//! implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All types are serializable (Serialize + Deserialize) where applicable

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// PrDiff
// ---------------------------------------------------------------------------

/// Structured representation of a Pull Request diff.
///
/// Contains all changed files with their hunks, plus metadata about
/// the diff size and any exceeded limits.
///
/// ## Data Flow
///
/// 1. `DiffParsingService::parse()` generates this from raw git diff output
/// 2. `PathValidationService::validate()` validates all file paths
/// 3. `LimitEnforcementService::enforce()` applies resource limits (progressive degradation)
/// 4. `RiskClassificationService::classify()` classifies each file by risk
/// 5. `AiSignalDetectionService::detect()` performs AI signal analysis
///
/// The enriched `PrDiff` is then passed to the Policy Evaluator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrDiff {
    /// All changed files in the PR.
    pub files: Vec<ChangedFile>,

    /// Total diff size in bytes.
    pub total_size_bytes: u64,

    /// Files that were excluded due to limit enforcement.
    pub excluded_files: Vec<String>,

    /// Whether any limit was exceeded.
    pub limits_exceeded: bool,

    /// Whether the policy file itself was modified.
    pub policy_modified: bool,

    /// AI signal detection results, if analysis was performed.
    pub ai_signals: Option<AiSignalResult>,

    /// Metadata about the PR that generated this diff.
    pub metadata: Option<DiffMetadata>,
}

/// Metadata about the PR associated with a diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffMetadata {
    /// The PR number.
    pub pr_number: u64,
    /// The base branch name.
    pub base_branch: String,
    /// The head branch name.
    pub head_branch: String,
    /// The head commit SHA.
    pub head_sha: String,
}

impl PrDiff {
    /// Iterate over changed files (excluding binary and excluded).
    pub fn changed_files(&self) -> impl Iterator<Item = &ChangedFile> {
        self.files.iter().filter(|f| !f.is_binary)
    }

    /// Get the count of non-binary changed files.
    pub fn changed_file_count(&self) -> usize {
        self.changed_files().count()
    }

    /// Check if any files in the diff match a given risk level.
    pub fn has_risk_level(&self, level: FileRisk) -> bool {
        self.files.iter().any(|f| f.risk == level)
    }
}

// ---------------------------------------------------------------------------
// ChangedFile
// ---------------------------------------------------------------------------

/// A single file changed in a PR.
///
/// Contains the file path, change status, line counts, binary flag,
/// diff hunks, and risk classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedFile {
    /// File path relative to repository root.
    pub path: String,

    /// Change status: added, modified, deleted, renamed.
    pub status: FileStatus,

    /// Number of lines added.
    pub additions: usize,

    /// Number of lines deleted.
    pub deletions: usize,

    /// Whether the file is binary (detected via NUL byte in first 8KB).
    pub is_binary: bool,

    /// The diff hunks for this file.
    pub hunks: Vec<DiffHunk>,

    /// Risk classification.
    pub risk: FileRisk,

    /// Raw diff content for this file (if available).
    pub raw_diff: Option<String>,
}

impl ChangedFile {
    /// Total lines changed (additions + deletions).
    pub fn total_lines(&self) -> usize {
        self.additions + self.deletions
    }

    /// Whether this file is a documentation file (Low risk by convention).
    pub fn is_documentation(&self) -> bool {
        self.risk == FileRisk::Low
    }
}

// ---------------------------------------------------------------------------
// FileStatus
// ---------------------------------------------------------------------------

/// Change status of a file in a PR diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatus {
    /// File was added.
    Added,
    /// File was modified.
    Modified,
    /// File was deleted.
    Deleted,
    /// File was renamed (previous path preserved).
    Renamed {
        /// The previous file path before rename.
        previous_path: String,
    },
}

impl FileStatus {
    /// Whether this status represents an addition or modification.
    pub fn is_addition_or_modification(&self) -> bool {
        matches!(self, FileStatus::Added | FileStatus::Modified)
    }

    /// Whether this status represents a deletion.
    pub fn is_deletion(&self) -> bool {
        matches!(self, FileStatus::Deleted)
    }
}

// ---------------------------------------------------------------------------
// DiffHunk
// ---------------------------------------------------------------------------

/// A single diff hunk (unified diff section) from a PR.
///
/// Corresponds to a `@@ ... @@` section in a unified diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    /// Start line number in the original file (1-indexed).
    pub old_start: usize,
    /// Number of lines in the original file for this hunk.
    pub old_lines: usize,
    /// Start line number in the new file (1-indexed).
    pub new_start: usize,
    /// Number of lines in the new file for this hunk.
    pub new_lines: usize,
    /// The hunk header (e.g., "@@ -1,3 +1,4 @@").
    pub header: String,
    /// The raw diff lines for this hunk.
    pub lines: Vec<DiffLine>,
}

/// A single line within a diff hunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    /// The line content (without leading +/-/space marker).
    pub content: String,
    /// The line type.
    pub line_type: DiffLineType,
    /// The line number in the original file (None for added lines).
    pub old_lineno: Option<usize>,
    /// The line number in the new file (None for deleted lines).
    pub new_lineno: Option<usize>,
}

/// Type of a line within a diff hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffLineType {
    /// Context line (unchanged, present in both versions).
    Context,
    /// Added line (present in new file, absent in old).
    Added,
    /// Deleted line (present in old file, absent in new).
    Deleted,
    /// No newline at end of file marker.
    NoNewline,
}

impl DiffHunk {
    /// Get the content of this hunk (all lines joined).
    pub fn content(&self) -> String {
        self.lines
            .iter()
            .map(|l| match l.line_type {
                DiffLineType::Added => format!("+{}", l.content),
                DiffLineType::Deleted => format!("-{}", l.content),
                DiffLineType::Context => format!(" {}", l.content),
                DiffLineType::NoNewline => "\\ No newline at end of file".to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Whether this hunk is empty (no lines).
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

// ---------------------------------------------------------------------------
// FileRisk
// ---------------------------------------------------------------------------

/// Risk level classification for a changed file.
///
/// Ordered by severity: Low < Medium < High < Critical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FileRisk {
    /// Documentation, config files, text assets.
    Low,
    /// Source code (default for most files).
    Medium,
    /// Migrations, SQL files, infrastructure changes.
    High,
    /// Auth, security, secrets, access control.
    Critical,
}

impl FileRisk {
    /// Whether this risk level requires mandatory review.
    pub fn requires_review(&self) -> bool {
        *self >= FileRisk::High
    }

    /// Whether this risk level triggers a blocking policy check.
    pub fn is_blocking(&self) -> bool {
        *self >= FileRisk::Critical
    }

    /// Get the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            FileRisk::Low => "low",
            FileRisk::Medium => "medium",
            FileRisk::High => "high",
            FileRisk::Critical => "critical",
        }
    }
}

// ---------------------------------------------------------------------------
// PolicyLimits
// ---------------------------------------------------------------------------

/// Resource limits for PR diff processing.
///
/// These limits prevent DoS attacks via massive diffs and ensure
/// the action completes within GitHub Actions' 6-hour timeout.
///
/// When limits are exceeded, the system applies progressive degradation:
/// process what fits within limits and flag the rest in `PrDiff.excluded_files`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyLimits {
    /// Maximum diff size in bytes (default: 10 MB).
    pub max_diff_size: u64,
    /// Maximum number of files to process (default: 100).
    pub max_files: usize,
    /// Maximum lines per file (default: 5000).
    pub max_lines_per_file: usize,
}

impl Default for PolicyLimits {
    fn default() -> Self {
        Self {
            max_diff_size: 10_000_000, // 10 MB
            max_files: 100,
            max_lines_per_file: 5000,
        }
    }
}

impl PolicyLimits {
    /// Create a new `PolicyLimits` with the given values.
    pub fn new(max_diff_size: u64, max_files: usize, max_lines_per_file: usize) -> Self {
        Self {
            max_diff_size,
            max_files,
            max_lines_per_file,
        }
    }
}

// ---------------------------------------------------------------------------
// AiSignal
// ---------------------------------------------------------------------------

/// A single detected AI-generated code signal.
///
/// Each signal identifies a specific location in the diff where an
/// AI-like pattern was detected, along with a confidence score.
///
/// Note: These are heuristic detections, not forensic evidence.
/// False positives are possible. Results are advisory — they flag
/// code for extra review, not block it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSignal {
    /// The file where the signal was detected.
    pub file: String,
    /// The hunk start line number.
    pub hunk_start: usize,
    /// The pattern name (e.g., "ai_comment", "uniform_indentation").
    pub pattern: String,
    /// Confidence score for this signal (0.0 to 1.0).
    pub confidence: f64,
    /// Optional description of the detected pattern.
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// AiSignalResult
// ---------------------------------------------------------------------------

/// Results of AI-generated code signal detection on a PR diff.
///
/// Contains all detected signals plus an overall confidence score
/// indicating how likely the entire diff is AI-generated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSignalResult {
    /// Individual detected signals.
    pub signals: Vec<AiSignal>,
    /// Overall AI-like confidence score (0.0 to 1.0).
    pub confidence: f64,
    /// Total number of hunks analyzed.
    pub total_hunks: usize,
    /// Number of hunks that were flagged.
    pub flagged_hunks: usize,
}

impl AiSignalResult {
    /// Whether any signals were detected.
    pub fn has_signals(&self) -> bool {
        !self.signals.is_empty()
    }

    /// Whether the overall confidence exceeds a threshold.
    ///
    /// Threshold of 0.7 is the default for flagging AI-generated code.
    pub fn exceeds_threshold(&self, threshold: f64) -> bool {
        self.confidence >= threshold
    }

    /// Get signals matching a specific pattern.
    pub fn signals_for_pattern(&self, pattern: &str) -> Vec<&AiSignal> {
        self.signals
            .iter()
            .filter(|s| s.pattern == pattern)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// DiffParseResult
// ---------------------------------------------------------------------------

/// Result of a raw diff parse operation.
///
/// Contains either a fully formed `PrDiff` or a partial result with errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffParseResult {
    /// The parsed diff, may be partial if errors occurred.
    pub diff: PrDiff,
    /// Parsing errors that were encountered (non-fatal).
    pub errors: Vec<String>,
    /// Number of files successfully parsed.
    pub files_parsed: usize,
    /// Number of files that failed to parse.
    pub files_failed: usize,
}

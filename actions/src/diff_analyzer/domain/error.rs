//! Error types for the Diff Analyzer bounded context.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md#error
//! Implements: Contract Freeze — DiffAnalyzerError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `DiffAnalyzerError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility

use thiserror::Error;

use crate::shared::github_client::GitHubClientError;

/// Errors that can occur during PR diff analysis.
#[derive(Debug, Error)]
pub enum DiffAnalyzerError {
    /// The raw diff content could not be parsed into a `PrDiff`.
    #[error("Failed to parse diff: {detail}")]
    DiffParseError {
        /// Description of the parse failure.
        detail: String,
        /// Line number in the raw diff where parsing failed (if known).
        line: Option<usize>,
    },

    /// A file path contains traversal (`../`) which is a security violation.
    #[error("Path traversal detected: '{path}'")]
    PathTraversal {
        /// The offending file path.
        path: String,
    },

    /// A file path is absolute (starts with `/`), which is not allowed.
    #[error("Absolute path not allowed: '{path}'")]
    AbsolutePath {
        /// The offending file path.
        path: String,
    },

    /// A file path contains null bytes, indicating a path injection attempt.
    #[error("Path injection detected (null byte): '{path}'")]
    PathInjection {
        /// The offending file path.
        path: String,
    },

    /// A file path contains a symlink component, which is not allowed.
    #[error("Symlink detected in path: '{path}'")]
    SymlinkDetected {
        /// The offending file path.
        path: String,
    },

    /// The total diff size exceeds the configured maximum.
    #[error("Diff too large: {size_bytes} bytes exceeds limit of {limit_bytes} bytes")]
    DiffTooLarge {
        /// Actual diff size in bytes.
        size_bytes: u64,
        /// Maximum allowed diff size in bytes.
        limit_bytes: u64,
    },

    /// The number of changed files exceeds the configured maximum.
    #[error("Too many files: {file_count} files exceeds limit of {file_limit}")]
    TooManyFiles {
        /// Actual number of files.
        file_count: usize,
        /// Maximum allowed files.
        file_limit: usize,
    },

    /// A single file exceeds the maximum lines limit.
    #[error("File too large: '{path}' has {line_count} lines, exceeds limit of {line_limit}")]
    FileTooLarge {
        /// The file path.
        path: String,
        /// Actual line count.
        line_count: usize,
        /// Maximum allowed lines.
        line_limit: usize,
    },

    /// A file could not be classified (unknown path pattern).
    #[error("Unclassifiable file path: '{path}'")]
    UnclassifiablePath {
        /// The unclassifiable file path.
        path: String,
    },

    /// AI signal detection encountered an error.
    #[error("AI signal detection error: {detail}")]
    AiSignalDetectionError {
        /// Description of the detection error.
        detail: String,
    },

    /// A binary file was detected but not expected.
    #[error("Unexpected binary file: '{path}'")]
    UnexpectedBinaryFile {
        /// The binary file path.
        path: String,
    },

    /// The policy limits configuration is invalid.
    #[error("Invalid policy limits: {detail}")]
    InvalidPolicyLimits {
        /// Description of the configuration issue.
        detail: String,
    },

    /// Failed to fetch the PR diff from the GitHub API.
    #[error("Failed to fetch PR diff: {detail}")]
    DiffFetchError {
        /// Description of the fetch failure.
        detail: String,
    },

    /// The PR number is invalid or missing.
    #[error("Invalid or missing PR number: {detail}")]
    InvalidPrNumber {
        /// Error details.
        detail: String,
    },

    /// IO error (file system, network).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// GitHub API client error.
    #[error("GitHub API error: {0}")]
    GitHubApi(#[from] GitHubClientError),

    /// Internal invariant violation (should not happen).
    #[error("Internal error: {detail}")]
    Internal {
        /// Error description.
        detail: String,
    },
}

impl DiffAnalyzerError {
    /// Whether the error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            DiffAnalyzerError::Io(_)
                | DiffAnalyzerError::GitHubApi(_)
                | DiffAnalyzerError::DiffFetchError { .. }
        )
    }

    /// Whether the error is a security violation (blocking).
    pub fn is_security_violation(&self) -> bool {
        matches!(
            self,
            DiffAnalyzerError::PathTraversal { .. }
                | DiffAnalyzerError::AbsolutePath { .. }
                | DiffAnalyzerError::PathInjection { .. }
                | DiffAnalyzerError::SymlinkDetected { .. }
        )
    }

    /// Whether the error is a limit violation.
    pub fn is_limit_violation(&self) -> bool {
        matches!(
            self,
            DiffAnalyzerError::DiffTooLarge { .. }
                | DiffAnalyzerError::TooManyFiles { .. }
                | DiffAnalyzerError::FileTooLarge { .. }
        )
    }
}

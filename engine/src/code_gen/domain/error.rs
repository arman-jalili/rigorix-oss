//! CodeGenError — typed error enum for code generation failures.
//!
//! @canonical .pi/architecture/modules/code-generation.md#error-handling
//! Implements: Contract Freeze — CodeGenError enum
//! Issue: #424
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `CodeGenError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility

use thiserror::Error;

/// Errors that can occur during code generation operations.
///
/// # Error Recovery
///
/// | Variant | Severity | Recovery |
/// |---------|----------|----------|
/// | `OldStringNotFound` | Error | LLM must issue corrected edit_file |
/// | `IdentityEdit` | Warning | LLM must provide differing old/new strings |
/// | `BinaryFile` | Error | Edit_file rejected for binary files |
/// | `FileTooLarge` | Error | Rerun with smaller scope |
/// | `WorkspaceEscape` | Critical | Path traversal blocked |
/// | `SyntaxError` | Warning | Edit applied but syntax errors detected |
/// | `PathValidationFailed` | Error | Path invalid or symlink detected |
#[derive(Debug, Error)]
pub enum CodeGenError {
    /// The old_string was not found in the file.
    ///
    /// This is the correctness anchor for edit_file. The LLM must verify
    /// the exact text it wants to replace and issue a corrected edit.
    #[error("old_string not found in {path}")]
    OldStringNotFound {
        /// The file path that was searched.
        path: String,
    },

    /// old_string and new_string are identical.
    ///
    /// The edit would be a no-op. The LLM must provide different values.
    #[error("old_string and new_string must differ")]
    IdentityEdit,

    /// The file appears to be binary (contains NUL bytes).
    ///
    /// Binary files cannot be edited with old_string matching.
    #[error("File appears to be binary: {path}")]
    BinaryFile {
        /// The file path that was detected as binary.
        path: String,
    },

    /// The file exceeds the maximum allowed size.
    #[error("File too large: {size} bytes (max {max})")]
    FileTooLarge {
        /// Actual file size in bytes.
        size: u64,
        /// Maximum allowed size in bytes.
        max: u64,
    },

    /// The specified path escapes the workspace boundary.
    #[error("Path escapes workspace: {path}")]
    WorkspaceEscape {
        /// The offending path.
        path: String,
    },

    /// Syntax error detected during post-edit verification.
    ///
    /// The edit was applied but tree-sitter detected syntax errors.
    #[error("Syntax error at {path}:{line}:{col}: {message}")]
    SyntaxError {
        /// The file with the syntax error.
        path: String,
        /// Line number (1-indexed).
        line: usize,
        /// Column number (1-indexed).
        col: usize,
        /// Error description.
        message: String,
    },

    /// Path validation failed (symlink detected, invalid path, etc.).
    #[error("Path validation failed: {path}: {reason}")]
    PathValidationFailed {
        /// The path that failed validation.
        path: String,
        /// Reason for failure.
        reason: String,
    },

    /// Internal error occurred.
    #[error("Internal code generation error: {detail}")]
    Internal {
        /// Error detail for diagnostics.
        detail: String,
    },
}

impl CodeGenError {
    /// Returns true if this error is recoverable (edit was applied but has issues).
    pub fn is_recoverable(&self) -> bool {
        matches!(self, CodeGenError::SyntaxError { .. })
    }

    /// Returns true if this error is retriable (LLM can reissue the edit).
    pub fn is_retriable(&self) -> bool {
        matches!(self, CodeGenError::OldStringNotFound { .. } | CodeGenError::IdentityEdit)
    }

    /// Returns the file path associated with this error, if available.
    pub fn path(&self) -> Option<&str> {
        match self {
            CodeGenError::OldStringNotFound { path }
            | CodeGenError::BinaryFile { path }
            | CodeGenError::WorkspaceEscape { path }
            | CodeGenError::SyntaxError { path, .. }
            | CodeGenError::PathValidationFailed { path, .. } => Some(path),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_old_string_not_found() {
        let err = CodeGenError::OldStringNotFound {
            path: "src/main.rs".into(),
        };
        assert!(err.is_retriable());
        assert!(!err.is_recoverable());
        assert_eq!(err.path(), Some("src/main.rs"));
        assert_eq!(err.to_string(), "old_string not found in src/main.rs");
    }

    #[test]
    fn test_identity_edit() {
        let err = CodeGenError::IdentityEdit;
        assert!(err.is_retriable());
        assert!(!err.is_recoverable());
        assert!(err.path().is_none());
        assert_eq!(err.to_string(), "old_string and new_string must differ");
    }

    #[test]
    fn test_binary_file() {
        let err = CodeGenError::BinaryFile {
            path: "image.png".into(),
        };
        assert!(!err.is_retriable());
        assert_eq!(err.to_string(), "File appears to be binary: image.png");
    }

    #[test]
    fn test_file_too_large() {
        let err = CodeGenError::FileTooLarge {
            size: 20_000_000,
            max: 10_000_000,
        };
        assert!(err.path().is_none());
        assert!(err.to_string().contains("20,000,000") || err.to_string().contains("20000000"));
    }

    #[test]
    fn test_workspace_escape() {
        let err = CodeGenError::WorkspaceEscape {
            path: "../../etc/passwd".into(),
        };
        assert!(err.to_string().contains("../../etc/passwd"));
    }

    #[test]
    fn test_syntax_error() {
        let err = CodeGenError::SyntaxError {
            path: "lib.rs".into(),
            line: 42,
            col: 5,
            message: "expected `;`".into(),
        };
        assert!(err.is_recoverable());
        assert!(err.to_string().contains("lib.rs:42:5"));
    }

    #[test]
    fn test_path_validation_failed() {
        let err = CodeGenError::PathValidationFailed {
            path: "/etc/passwd".into(),
            reason: "path traverses symlink outside workspace".into(),
        };
        assert!(!err.is_retriable());
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_internal() {
        let err = CodeGenError::Internal {
            detail: "parse error".into(),
        };
        assert!(err.path().is_none());
        assert!(!err.is_retriable());
    }
}

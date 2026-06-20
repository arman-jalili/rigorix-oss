//! Event payload schemas for the Code Generation Pipeline.
//!
//! @canonical .pi/architecture/modules/code-generation.md#events
//! Implements: Contract Freeze — CodeGenEvent payload schemas
//! Issue: #424
//!
//! These events are emitted when code editing operations are performed.
//! Consumers (audit trail, console printer, TUI) subscribe to these events.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - All types are serializable for event bus integration

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Events emitted by the Code Generation Pipeline.
///
/// Wrapped in `ExecutionEvent::CodeGen(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CodeGenEvent {
    /// An edit_file operation was started.
    EditFileStarted {
        /// The session/execution ID for correlation.
        session_id: String,
        /// Path of the file being edited.
        file_path: String,
        /// Length of old_string (for logging/diagnostics).
        old_string_length: usize,
        /// Length of new_string (for logging/diagnostics).
        new_string_length: usize,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// An edit_file operation completed successfully.
    EditFileCompleted {
        /// The session/execution ID for correlation.
        session_id: String,
        /// Path of the edited file.
        file_path: String,
        /// Whether replace_all was used.
        replace_all: bool,
        /// Length of the unified diff produced.
        diff_length: usize,
        /// Whether the syntax gate was applied.
        syntax_gate_applied: bool,
        /// Whether the syntax gate passed.
        syntax_gate_passed: bool,
        /// Duration of the operation in milliseconds.
        duration_ms: u64,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// An edit_file operation failed.
    EditFileFailed {
        /// The session/execution ID for correlation.
        session_id: String,
        /// Path of the file that was targeted.
        file_path: String,
        /// Error message describing the failure.
        error: String,
        /// Error code for machine-readable handling.
        error_code: String,
        /// Duration of the operation in milliseconds.
        duration_ms: u64,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// A read_file operation was performed.
    ReadFileCompleted {
        /// The session/execution ID for correlation.
        session_id: String,
        /// Path of the file that was read.
        file_path: String,
        /// Total lines in the file.
        total_lines: usize,
        /// Number of bytes read.
        bytes_read: u64,
        /// Whether the file was detected as binary.
        is_binary: bool,
        /// Duration of the operation in milliseconds.
        duration_ms: u64,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// The syntax gate was applied to an edited file.
    SyntaxGateApplied {
        /// The session/execution ID for correlation.
        session_id: String,
        /// Path of the file verified.
        file_path: String,
        /// The syntax gate outcome (passed, failed, skipped).
        outcome: String,
        /// Number of syntax errors found (0 if passed/skipped).
        error_count: usize,
        /// Duration of the syntax check in milliseconds.
        duration_ms: u64,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session() -> String {
        "session-1".into()
    }

    #[test]
    fn test_edit_file_started() {
        let event = CodeGenEvent::EditFileStarted {
            session_id: sample_session(),
            file_path: "src/main.rs".into(),
            old_string_length: 10,
            new_string_length: 20,
            timestamp: Utc::now(),
        };
        match &event {
            CodeGenEvent::EditFileStarted { file_path, .. } => {
                assert_eq!(file_path, "src/main.rs");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_edit_file_completed() {
        let event = CodeGenEvent::EditFileCompleted {
            session_id: sample_session(),
            file_path: "src/lib.rs".into(),
            replace_all: false,
            diff_length: 200,
            syntax_gate_applied: true,
            syntax_gate_passed: true,
            duration_ms: 50,
            timestamp: Utc::now(),
        };
        match &event {
            CodeGenEvent::EditFileCompleted {
                file_path,
                syntax_gate_passed,
                ..
            } => {
                assert_eq!(file_path, "src/lib.rs");
                assert!(*syntax_gate_passed);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_edit_file_failed() {
        let event = CodeGenEvent::EditFileFailed {
            session_id: sample_session(),
            file_path: "src/main.rs".into(),
            error: "old_string not found".into(),
            error_code: "OLD_STRING_NOT_FOUND".into(),
            duration_ms: 10,
            timestamp: Utc::now(),
        };
        match &event {
            CodeGenEvent::EditFileFailed { error_code, .. } => {
                assert_eq!(error_code, "OLD_STRING_NOT_FOUND");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_read_file_completed() {
        let event = CodeGenEvent::ReadFileCompleted {
            session_id: sample_session(),
            file_path: "Cargo.toml".into(),
            total_lines: 50,
            bytes_read: 2048,
            is_binary: false,
            duration_ms: 5,
            timestamp: Utc::now(),
        };
        match &event {
            CodeGenEvent::ReadFileCompleted {
                file_path,
                total_lines,
                ..
            } => {
                assert_eq!(file_path, "Cargo.toml");
                assert_eq!(*total_lines, 50);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_syntax_gate_applied() {
        let event = CodeGenEvent::SyntaxGateApplied {
            session_id: sample_session(),
            file_path: "src/main.rs".into(),
            outcome: "passed".into(),
            error_count: 0,
            duration_ms: 100,
            timestamp: Utc::now(),
        };
        match &event {
            CodeGenEvent::SyntaxGateApplied {
                outcome,
                error_count,
                ..
            } => {
                assert_eq!(outcome, "passed");
                assert_eq!(*error_count, 0);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        let event = CodeGenEvent::EditFileCompleted {
            session_id: "s1".into(),
            file_path: "f.rs".into(),
            replace_all: false,
            diff_length: 100,
            syntax_gate_applied: true,
            syntax_gate_passed: true,
            duration_ms: 30,
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: CodeGenEvent = serde_json::from_str(&json).unwrap();
        match deserialized {
            CodeGenEvent::EditFileCompleted { file_path, .. } => {
                assert_eq!(file_path, "f.rs");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_serde_tagged_union() {
        let event = CodeGenEvent::ReadFileCompleted {
            session_id: "s1".into(),
            file_path: "f.rs".into(),
            total_lines: 100,
            bytes_read: 5000,
            is_binary: false,
            duration_ms: 10,
            timestamp: Utc::now(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(
            json.get("type").and_then(|v| v.as_str()),
            Some("read_file_completed")
        );
    }
}

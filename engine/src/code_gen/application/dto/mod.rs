//! Data Transfer Objects for the Code Generation Pipeline module.
//!
//! @canonical .pi/architecture/modules/code-generation.md
//! Implements: Contract Freeze — DTO schemas for edit_file, read_file, syntax gate
//! Issue: #424
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// EditFile DTOs
// ---------------------------------------------------------------------------

/// Input for the edit_file operation.
///
/// The LLM provides the exact text to find and replace. The `old_string`
/// serves as both the position anchor and the correctness check.
///
/// # Example
///
/// ```json
/// {
///     "path": "src/main.rs",
///     "old_string": "let x = 5;",
///     "new_string": "let x = 10;",
///     "replace_all": false
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditFileInput {
    /// Absolute or workspace-relative file path.
    pub path: String,

    /// Exact text to find in the file. This is the position anchor and
    /// correctness check — the edit is rejected if this text does not
    /// exist in the file.
    pub old_string: String,

    /// Replacement text. Can be larger, smaller, or the same length
    /// as old_string. Must differ from old_string (identity check).
    pub new_string: String,

    /// Replace all occurrences of old_string in the file.
    /// Default: only the first occurrence is replaced.
    #[serde(default)]
    pub replace_all: Option<bool>,
}

/// Output from the edit_file operation.
///
/// Returns complete before/after content plus a unified diff so the
/// LLM can self-verify its edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditFileOutput {
    /// Path of the file that was edited.
    pub file_path: String,

    /// The old_string that was matched (echoed back for verification).
    pub old_string: String,

    /// The new_string that was inserted (echoed back for verification).
    pub new_string: String,

    /// Complete original file content before the edit.
    pub original_file: String,

    /// Complete updated file content after the edit.
    pub updated_content: String,

    /// Unified diff between original and updated (human and LLM readable).
    pub unified_diff: String,

    /// Whether all occurrences were replaced.
    pub replace_all: bool,

    /// Number of occurrences that were replaced.
    pub occurrences_replaced: usize,

    /// Syntax gate result (if configured).
    #[serde(default)]
    pub syntax_gate_result: Option<crate::code_gen::domain::result::SyntaxGateResult>,

    /// Structured patch hunks for programmatic processing.
    #[serde(default)]
    pub patch_hunks: Vec<StructuredPatchHunk>,
}

// ---------------------------------------------------------------------------
// StructuredPatchHunk
// ---------------------------------------------------------------------------

/// A single diff hunk with old/new line ranges.
///
/// Provides structured diff output for programmatic processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredPatchHunk {
    /// Starting line of the hunk in the original file (1-indexed).
    pub old_start: usize,

    /// Number of lines in the original file covered by this hunk.
    pub old_lines: usize,

    /// Starting line of the hunk in the new file (1-indexed).
    pub new_start: usize,

    /// Number of lines in the new file covered by this hunk.
    pub new_lines: usize,

    /// The diff lines, prefixed with '-' (removed), '+' (added), or ' ' (context).
    pub lines: Vec<String>,
}

// ---------------------------------------------------------------------------
// ReadFile DTOs
// ---------------------------------------------------------------------------

/// Input for the read_file operation (extended beyond basic file read).
///
/// Supports offset/limit paging for large files and binary detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileInput {
    /// Absolute or workspace-relative file path.
    pub path: String,

    /// Starting line offset (1-indexed). If omitted, read from line 1.
    #[serde(default)]
    pub offset: Option<usize>,

    /// Maximum number of lines to return. If omitted, return all lines.
    #[serde(default)]
    pub limit: Option<usize>,

    /// Maximum file size in bytes (default: 10MB).
    /// Files larger than this are rejected with FileTooLarge.
    #[serde(default = "default_max_file_size")]
    pub max_file_size: Option<u64>,
}

fn default_max_file_size() -> Option<u64> {
    Some(10_485_760) // 10 MB
}

/// Output from the read_file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileOutput {
    /// Path of the file that was read.
    pub file_path: String,

    /// The file contents (or the requested line window).
    pub content: String,

    /// Starting line number of the returned content (1-indexed).
    pub start_line: usize,

    /// Total number of lines in the file.
    pub total_lines: usize,

    /// Total file size in bytes.
    pub total_bytes: u64,

    /// Whether the file was detected as binary.
    pub is_binary: bool,

    /// The requested offset (if any).
    pub requested_offset: Option<usize>,

    /// The requested limit (if any).
    pub requested_limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// SyntaxGate DTOs
// ---------------------------------------------------------------------------

/// Input for running the syntax gate on a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxGateInput {
    /// Path of the file to verify (used for language detection).
    pub file_path: String,

    /// The file content to verify.
    pub content: String,
}

/// Output from running the syntax gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxGateOutput {
    /// The syntax gate result.
    pub result: crate::code_gen::domain::result::SyntaxGateResult,

    /// The language that was detected for verification.
    pub detected_language: Option<String>,

    /// Duration of the syntax check in milliseconds.
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// SyntaxGate Configuration DTO
// ---------------------------------------------------------------------------

/// Configuration for the SyntaxGate service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxGateConfig {
    /// Whether the syntax gate is enabled.
    /// When disabled, edits are applied without syntax verification.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Whether to block edits that produce syntax errors.
    /// When false, edits are applied and syntax errors are reported as warnings.
    #[serde(default)]
    pub block_on_error: bool,

    /// Whether to skip syntax verification for files without a parser.
    /// When true, files in unsupported languages pass without verification.
    #[serde(default = "default_skip_unsupported")]
    pub skip_unsupported: bool,

    /// Maximum file size in bytes for syntax verification.
    /// Files larger than this are skipped.
    #[serde(default = "default_max_verify_size")]
    pub max_verify_size: u64,

    /// List of supported language identifiers (e.g., "rust", "typescript", "python").
    #[serde(default)]
    pub supported_languages: Vec<String>,
}

fn default_enabled() -> bool { true }
fn default_skip_unsupported() -> bool { true }
fn default_max_verify_size() -> u64 { 1_048_576 } // 1 MB

impl Default for SyntaxGateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            block_on_error: false,
            skip_unsupported: true,
            max_verify_size: 1_048_576,
            supported_languages: vec![
                "rust".into(),
                "typescript".into(),
                "python".into(),
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// EditFileConfig
// ---------------------------------------------------------------------------

/// Configuration for the edit_file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditFileConfig {
    /// Maximum file size in bytes for editing (default: 10MB).
    #[serde(default = "default_max_edit_size")]
    pub max_file_size: u64,

    /// Whether to enable the identity check (old_string == new_string rejection).
    #[serde(default = "default_identity_check")]
    pub enable_identity_check: bool,

    /// Whether to require syntax gate verification after each edit.
    #[serde(default)]
    pub require_syntax_gate: bool,

    /// Maximum number of occurrences to replace when replace_all is true.
    /// Prevents excessive replacements.
    #[serde(default = "default_max_replacements")]
    pub max_replacements: usize,
}

fn default_max_edit_size() -> u64 { 10_485_760 }
fn default_identity_check() -> bool { true }
fn default_max_replacements() -> usize { 1000 }

impl Default for EditFileConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10_485_760,
            enable_identity_check: true,
            require_syntax_gate: false,
            max_replacements: 1000,
        }
    }
}

// ---------------------------------------------------------------------------
// CodeGen Service DTOs
// ---------------------------------------------------------------------------

/// Input for configuring the code generation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGenConfig {
    /// Configuration for the edit_file operation.
    #[serde(default)]
    pub edit_file: EditFileConfig,

    /// Configuration for the syntax gate.
    #[serde(default)]
    pub syntax_gate: SyntaxGateConfig,

    /// Workspace root for path validation.
    pub workspace_root: Option<String>,
}

impl Default for CodeGenConfig {
    fn default() -> Self {
        Self {
            edit_file: EditFileConfig::default(),
            syntax_gate: SyntaxGateConfig::default(),
            workspace_root: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: Diff computation constants
// ---------------------------------------------------------------------------

/// Maximum number of context lines in a unified diff.
pub const DIFF_CONTEXT_LINES: usize = 3;

/// Maximum diff size in bytes before truncation.
pub const MAX_DIFF_SIZE: u64 = 100_000;

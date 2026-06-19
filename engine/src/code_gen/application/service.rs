//! Service interfaces (use cases) for the Code Generation Pipeline.
//!
//! @canonical .pi/architecture/modules/code-generation.md#syntax-service
//! Implements: Contract Freeze — SyntaxGateService trait, EditFileService trait
//! Issue: #424
//!
//! These traits define the application-level operations for code generation:
//! syntax verification, file editing orchestration, and code generation
//! configuration.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - No implementation — only contract signatures

use crate::code_gen::domain::error::CodeGenError;

use super::dto::{
    CodeGenConfig, EditFileInput, EditFileOutput, ReadFileInput, ReadFileOutput,
    SyntaxGateInput, SyntaxGateOutput,
};

/// Service for post-edit syntax verification using tree-sitter.
///
/// Validates that a file's content parses without syntax errors after
/// an edit. Uses Rigorix's existing tree-sitter integration for AST
/// parsing. Language is auto-detected from file extension.
///
/// # Behaviour
/// - If the language has no parser, returns `Skipped`
/// - If the file is syntactically valid, returns `Passed`
/// - If syntax errors are found, returns `Failed` with error locations
/// - Maximum file size is configurable (files larger than limit are Skipped)
pub trait SyntaxGateService: Send + Sync {
    /// Verify that `content` produces a valid AST for the file at `path`.
    ///
    /// Language is auto-detected from file extension. If no parser is
    /// available, the check is skipped (not an error).
    fn verify(&self, input: SyntaxGateInput) -> Result<SyntaxGateOutput, CodeGenError>;

    /// Verify multiple files in batch.
    ///
    /// Useful when multiple files were edited in a single turn.
    fn verify_batch(&self, inputs: Vec<SyntaxGateInput>) -> Result<Vec<SyntaxGateOutput>, CodeGenError>;

    /// Get the current configuration of the syntax gate.
    fn get_config(&self) -> super::dto::SyntaxGateConfig;

    /// Update the syntax gate configuration at runtime.
    fn reconfigure(&self, config: super::dto::SyntaxGateConfig) -> Result<(), CodeGenError>;

    /// Get the list of languages supported by this syntax gate instance.
    fn supported_languages(&self) -> Vec<String>;
}

/// Service for orchestrating the edit_file operation.
///
/// Handles the full edit pipeline:
/// 1. Path validation (workspace boundary, symlink detection, binary detection)
/// 2. File reading
/// 3. Identity check (old_string != new_string)
/// 4. Existence check (old_string exists in file)
/// 5. String replacement (first or all occurrences)
/// 6. Atomic write
/// 7. Unified diff computation
/// 8. Optional syntax gate verification
pub trait EditFileService: Send + Sync {
    /// Execute an edit_file operation.
    ///
    /// Performs all validation gates, the replacement, and returns
    /// structured before/after/diff output.
    fn edit(&self, input: EditFileInput) -> Result<EditFileOutput, CodeGenError>;

    /// Preview an edit without applying it.
    ///
    /// Returns the diff and updated content but does NOT write to disk.
    /// Useful for the LLM to verify an edit before committing.
    fn preview_edit(&self, input: EditFileInput) -> Result<EditFileOutput, CodeGenError>;
}

/// Service for reading files with extended options.
///
/// Extends the basic file read with:
/// - Offset/limit paging (line-based)
/// - Binary file detection (NUL byte scan)
/// - File size limits
/// - Total line count reporting
pub trait ReadFileService: Send + Sync {
    /// Read a file with optional offset/limit.
    fn read(&self, input: ReadFileInput) -> Result<ReadFileOutput, CodeGenError>;

    /// Read only the first N bytes of a file for binary detection.
    /// Returns Ok if text, Err if binary.
    fn detect_binary(&self, path: &str) -> Result<bool, CodeGenError>;
}

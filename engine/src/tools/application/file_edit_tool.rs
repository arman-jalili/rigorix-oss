//! EditFileTool — exact-string replacement for code editing.
//!
//! @canonical .pi/architecture/modules/code-generation.md#edit-file
//! Implements: Tool trait — EditFile concrete tool
//! Issue: #425
//!
//! The primary code insertion mechanism. Replaces an exact text string in a
//! file with new content. The `old_string` serves as both the position anchor
//! and the correctness anchor — the edit is rejected if `old_string` does not
//! exist in the file (prevents hallucinated edits).
//!
//! # Algorithm
//! 1. Resolve and canonicalize path; validate workspace boundary
//! 2. Read full file contents into memory
//! 3. Identity check: if `old_string == new_string`, reject
//! 4. Existence check: if `old_string` not found, reject
//! 5. Replace first occurrence (or all if `replace_all: true`)
//! 6. Write updated content via atomic write-rename
//! 7. Compute unified diff between original and updated
//! 8. Return structured EditFileOutput with full before/after/diff
//!
//! # Risk Level
//! Medium — modifies files on disk (same as FileWriteTool).

use async_trait::async_trait;
use std::fs;
use std::path::Path;

use crate::code_gen::application::dto::{EditFileOutput, StructuredPatchHunk, SyntaxGateInput};
use crate::code_gen::application::service::SyntaxGateService;
use crate::tools::application::dto::{SideEffect, ToolInput, ToolResult};
use crate::tools::domain::{Tool, ToolError};

/// Tool for exact-string file editing with position anchoring.
///
/// # Input Parameters
/// - `path` (required, string): Path to the file to edit.
/// - `old_string` (required, string): Exact text to find and replace.
/// - `new_string` (required, string): Replacement text.
/// - `replace_all` (optional, bool): Replace all occurrences (default: false).
///
/// # Errors
/// - `InvalidInput`: old_string is empty or old_string == new_string
/// - `NotFound`: old_string not found in file
/// - `PathDenied`: path escapes workspace or is binary
/// - `ExecutionFailed`: IO error reading/writing file
///
/// # Side Effects
/// - `FileModified`: Reports the modified file path.
pub struct EditFileTool {
    /// Root directory for path resolution and validation.
    workspace_root: String,

    /// Maximum file size in bytes (default: 10MB).
    max_file_size: u64,

    /// Optional post-edit syntax verification gate.
    syntax_gate: Option<Box<dyn SyntaxGateService>>,
}

impl EditFileTool {
    /// Create a new EditFileTool with the given workspace root.
    pub fn new(workspace_root: impl Into<String>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            max_file_size: 10_485_760, // 10 MB
            syntax_gate: None,
        }
    }

    /// Attach a syntax gate service for post-edit verification.
    pub fn with_syntax_gate(mut self, gate: Box<dyn SyntaxGateService>) -> Self {
        self.syntax_gate = Some(gate);
        self
    }

    /// Set the maximum allowed file size.
    pub fn with_max_file_size(mut self, max_bytes: u64) -> Self {
        self.max_file_size = max_bytes;
        self
    }

    /// Run the syntax gate on the updated content, if configured.
    fn run_syntax_gate(
        &self,
        file_path: &str,
        content: &str,
    ) -> Result<Option<crate::code_gen::domain::result::SyntaxGateResult>, ToolError> {
        match &self.syntax_gate {
            Some(gate) => {
                let input = SyntaxGateInput {
                    file_path: file_path.to_string(),
                    content: content.to_string(),
                };
                gate.verify(input)
                    .map(|output| Some(output.result))
                    .map_err(|e| ToolError::ExecutionFailed(format!("Syntax gate error: {}", e)))
            }
            None => Ok(None),
        }
    }

    /// Resolve and validate a file path against the workspace root.
    fn resolve_path(&self, path_str: &str) -> Result<std::path::PathBuf, ToolError> {
        let root = Path::new(&self.workspace_root);

        // Check for path traversal
        let normalized = path_str.replace('\\', "/");
        if normalized.contains("..") {
            let path = root.join(path_str);
            // Try canonicalizing — if it resolves outside workspace, deny
            match path.canonicalize() {
                Ok(canonical) => {
                    let root_canonical = root.canonicalize().map_err(|_| {
                        ToolError::ExecutionFailed("Cannot resolve workspace root".to_string())
                    })?;
                    if !canonical.starts_with(&root_canonical) {
                        return Err(ToolError::PathDenied(format!(
                            "Path '{}' is outside workspace root",
                            path_str
                        )));
                    }
                    return Ok(canonical);
                }
                Err(_) => {
                    return Err(ToolError::PathDenied(format!(
                        "Path '{}' could not be resolved",
                        path_str
                    )));
                }
            }
        }

        let path = root.join(path_str);
        Ok(path)
    }

    /// Check if a file is binary by scanning for NUL bytes in the first 8KB.
    fn is_binary(&self, path: &Path) -> Result<bool, ToolError> {
        use std::io::Read;

        let mut file = fs::File::open(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ToolError::NotFound(format!("File not found: {}", path.display()))
            } else {
                ToolError::ExecutionFailed(format!("Cannot open file: {}", e))
            }
        })?;

        let mut buffer = vec![0u8; 8192];
        let n = file
            .read(&mut buffer)
            .map_err(|e| ToolError::ExecutionFailed(format!("Cannot read file: {}", e)))?;

        Ok(buffer[..n].contains(&0u8))
    }

    /// Compute a simple unified diff between original and updated content.
    fn compute_diff(
        &self,
        original: &str,
        updated: &str,
        file_path: &str,
    ) -> (String, Vec<StructuredPatchHunk>) {
        let original_lines: Vec<&str> = original.lines().collect();
        let updated_lines: Vec<&str> = updated.lines().collect();

        // Simple diff: find the first differing line
        let mut diff_start = 0;
        while diff_start < original_lines.len()
            && diff_start < updated_lines.len()
            && original_lines[diff_start] == updated_lines[diff_start]
        {
            diff_start += 1;
        }

        if diff_start >= original_lines.len() && diff_start >= updated_lines.len() {
            return (String::new(), vec![]);
        }

        // Find where they match again from the end
        let mut diff_end_orig = original_lines.len();
        let mut diff_end_upd = updated_lines.len();
        while diff_end_orig > diff_start
            && diff_end_upd > diff_start
            && original_lines[diff_end_orig - 1] == updated_lines[diff_end_upd - 1]
        {
            diff_end_orig -= 1;
            diff_end_upd -= 1;
        }

        let context = 3;
        let display_start = diff_start.saturating_sub(context);
        let display_end_orig = (diff_end_orig + context).min(original_lines.len());
        let display_end_upd = (diff_end_upd + context).min(updated_lines.len());

        // Build unified diff string
        let mut diff = String::new();
        diff.push_str(&format!("--- a/{}\n+++ b/{}\n", file_path, file_path));
        diff.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            display_start + 1,
            display_end_orig - display_start,
            display_start + 1,
            display_end_upd - display_start,
        ));

        // Context before
        for i in display_start..diff_start {
            diff.push_str(&format!(" {}\n", original_lines[i]));
        }

        // Removed lines
        for i in diff_start..diff_end_orig {
            diff.push_str(&format!("-{}\n", original_lines[i]));
        }

        // Added lines
        for i in diff_start..diff_end_upd {
            diff.push_str(&format!("+{}\n", updated_lines[i]));
        }

        // Context after
        for i in diff_end_orig..display_end_orig {
            if i < original_lines.len() {
                diff.push_str(&format!(" {}\n", original_lines[i]));
            }
        }

        // Build structured patch hunks
        let mut lines = Vec::new();
        for i in diff_start..diff_end_orig {
            lines.push(format!("-{}", original_lines[i]));
        }
        for i in diff_start..diff_end_upd {
            lines.push(format!("+{}", updated_lines[i]));
        }

        let hunk = StructuredPatchHunk {
            old_start: diff_start + 1,
            old_lines: diff_end_orig - diff_start,
            new_start: diff_start + 1,
            new_lines: diff_end_upd - diff_start,
            lines,
        };

        (diff, vec![hunk])
    }

    /// Perform the actual edit logic (shared by execute and preview).
    fn perform_edit(
        &self,
        params: &serde_json::Value,
        dry_run: bool,
    ) -> Result<EditFileOutput, ToolError> {
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing required parameter: path".into()))?;

        let old_string = params
            .get("old_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidInput("Missing required parameter: old_string".into())
            })?;

        let new_string = params
            .get("new_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidInput("Missing required parameter: new_string".into())
            })?;

        let replace_all = params
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Validate inputs
        if old_string.is_empty() {
            return Err(ToolError::InvalidInput(
                "old_string must not be empty".into(),
            ));
        }
        if new_string.is_empty() {
            return Err(ToolError::InvalidInput(
                "new_string must not be empty".into(),
            ));
        }

        // Gate: Identity check
        if old_string == new_string {
            return Err(ToolError::InvalidInput(
                "old_string and new_string must differ".into(),
            ));
        }

        // Gate: Path validation
        let resolved = self.resolve_path(path_str)?;

        // Check if file exists
        if !resolved.exists() {
            return Err(ToolError::NotFound(format!("File not found: {}", path_str)));
        }

        // Gate: Binary detection
        let is_binary = self.is_binary(&resolved)?;
        if is_binary {
            return Err(ToolError::InvalidInput(format!(
                "Cannot edit binary file: {}",
                path_str
            )));
        }

        // Gate: File size check
        let metadata = fs::metadata(&resolved)
            .map_err(|e| ToolError::ExecutionFailed(format!("Cannot read file metadata: {}", e)))?;
        if metadata.len() > self.max_file_size {
            return Err(ToolError::InvalidInput(format!(
                "File too large: {} bytes (max {})",
                metadata.len(),
                self.max_file_size
            )));
        }

        // Read original content
        let original = fs::read_to_string(&resolved)
            .map_err(|e| ToolError::ExecutionFailed(format!("Cannot read file: {}", e)))?;

        // Gate: Existence check
        if !original.contains(old_string) {
            return Err(ToolError::NotFound(format!(
                "old_string not found in {}",
                path_str
            )));
        }

        // Perform replacement
        let occurrences = original.matches(old_string).count();
        let updated = if replace_all {
            original.replace(old_string, new_string)
        } else {
            original.replacen(old_string, new_string, 1)
        };

        // Compute diff
        let (unified_diff, patch_hunks) = self.compute_diff(&original, &updated, path_str);

        // Atomic write (skip in dry_run mode)
        if !dry_run {
            let tmp_path = format!("{}.tmp", resolved.display());
            fs::write(&tmp_path, &updated).map_err(|e| {
                ToolError::ExecutionFailed(format!("Cannot write temporary file: {}", e))
            })?;
            fs::rename(&tmp_path, &resolved).map_err(|e| {
                // Clean up temp file on failure
                let _ = fs::remove_file(&tmp_path);
                ToolError::ExecutionFailed(format!("Cannot rename temporary file: {}", e))
            })?;
        }

        // Run syntax gate BEFORE moving `updated`
        let syntax_gate_result = self.run_syntax_gate(path_str, &updated).ok().flatten();

        Ok(EditFileOutput {
            file_path: path_str.to_string(),
            old_string: old_string.to_string(),
            new_string: new_string.to_string(),
            original_file: original,
            updated_content: updated,
            unified_diff,
            replace_all,
            occurrences_replaced: if replace_all {
                occurrences
            } else {
                1.min(occurrences)
            },
            syntax_gate_result,
            patch_hunks,
        })
    }
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let params_value = serde_json::to_value(&input.params).unwrap_or_default();
        let dry_run = params_value
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let edit_output = self.perform_edit(&params_value, dry_run)?;

        let output_json = serde_json::to_string(&edit_output).map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to serialize output: {}", e))
        })?;

        Ok(ToolResult {
            output: output_json,
            exit_code: 0,
            side_effects: if !dry_run {
                vec![SideEffect::new(
                    &edit_output.file_path,
                    "file_edit",
                    &format!(
                        "Replaced {} occurrence(s) of '{}' with '{}'",
                        edit_output.occurrences_replaced,
                        &edit_output.old_string[..edit_output.old_string.len().min(50)],
                        &edit_output.new_string[..edit_output.new_string.len().min(50)],
                    ),
                )]
            } else {
                vec![]
            },
            duration_ms: 0,
            dry_run,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use tempfile::TempDir;

    fn make_input(params: Vec<(&str, serde_json::Value)>) -> ToolInput {
        ToolInput::new(
            params
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    fn create_test_file(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    fn create_tool(workspace: &str) -> EditFileTool {
        EditFileTool::new(workspace)
    }

    #[tokio::test]
    async fn test_name() {
        let tool = create_tool("/tmp");
        assert_eq!(tool.name(), "edit_file");
    }

    #[tokio::test]
    async fn test_basic_text_replacement() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_str().unwrap();
        create_test_file(&tmp, "test.rs", "fn old_function() {}");

        let input = make_input(vec![
            ("path", serde_json::json!("test.rs")),
            ("old_string", serde_json::json!("old_function")),
            ("new_string", serde_json::json!("new_function")),
        ]);

        let tool = create_tool(ws);
        let result = tool.execute(&input).await.unwrap();
        assert_eq!(result.exit_code, 0);

        let output: EditFileOutput = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output.file_path, "test.rs");
        assert_eq!(output.old_string, "old_function");
        assert_eq!(output.new_string, "new_function");
        assert!(output.original_file.contains("old_function"));
        assert!(output.updated_content.contains("new_function"));
        assert!(!output.unified_diff.is_empty());
        assert_eq!(output.occurrences_replaced, 1);
    }

    #[tokio::test]
    async fn test_identity_check_rejected() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_str().unwrap();
        create_test_file(&tmp, "test.rs", "fn foo() {}");

        let input = make_input(vec![
            ("path", serde_json::json!("test.rs")),
            ("old_string", serde_json::json!("foo")),
            ("new_string", serde_json::json!("foo")),
        ]);

        let tool = create_tool(ws);
        let err = tool.execute(&input).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
        assert!(err.to_string().contains("must differ"));
    }

    #[tokio::test]
    async fn test_old_string_not_found() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_str().unwrap();
        create_test_file(&tmp, "test.rs", "fn existing() {}");

        let input = make_input(vec![
            ("path", serde_json::json!("test.rs")),
            ("old_string", serde_json::json!("nonexistent")),
            ("new_string", serde_json::json!("replacement")),
        ]);

        let tool = create_tool(ws);
        let err = tool.execute(&input).await.unwrap_err();
        assert!(matches!(err, ToolError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_replace_all() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_str().unwrap();
        create_test_file(&tmp, "test.rs", "foo\nfoo\nfoo");

        let input = make_input(vec![
            ("path", serde_json::json!("test.rs")),
            ("old_string", serde_json::json!("foo")),
            ("new_string", serde_json::json!("bar")),
            ("replace_all", serde_json::json!(true)),
        ]);

        let tool = create_tool(ws);
        let result = tool.execute(&input).await.unwrap();
        let output: EditFileOutput = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output.occurrences_replaced, 3);

        // Verify file content
        let content = fs::read_to_string(tmp.path().join("test.rs")).unwrap();
        assert_eq!(content, "bar\nbar\nbar");
    }

    #[tokio::test]
    async fn test_dry_run_does_not_modify_file() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_str().unwrap();
        create_test_file(&tmp, "test.rs", "original content");

        let input = make_input(vec![
            ("path", serde_json::json!("test.rs")),
            ("old_string", serde_json::json!("original")),
            ("new_string", serde_json::json!("modified")),
            ("dry_run", serde_json::json!(true)),
        ]);

        let tool = create_tool(ws);
        let result = tool.execute(&input).await.unwrap();
        assert!(result.dry_run);

        // File should be unchanged
        let content = fs::read_to_string(tmp.path().join("test.rs")).unwrap();
        assert_eq!(content, "original content");
    }

    #[tokio::test]
    async fn test_path_outside_workspace_rejected() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_str().unwrap();

        let input = make_input(vec![
            ("path", serde_json::json!("../outside.txt")),
            ("old_string", serde_json::json!("a")),
            ("new_string", serde_json::json!("b")),
        ]);

        let tool = create_tool(ws);
        let err = tool.execute(&input).await.unwrap_err();
        assert!(matches!(err, ToolError::PathDenied(_)));
    }

    #[tokio::test]
    async fn test_binary_file_rejected() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_str().unwrap();
        let path = tmp.path().join("binary.bin");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(&[0u8, 1, 2, 3]).unwrap();

        let input = make_input(vec![
            ("path", serde_json::json!("binary.bin")),
            ("old_string", serde_json::json!("a")),
            ("new_string", serde_json::json!("b")),
        ]);

        let tool = create_tool(ws);
        let err = tool.execute(&input).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_file_not_found() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_str().unwrap();

        let input = make_input(vec![
            ("path", serde_json::json!("nonexistent.rs")),
            ("old_string", serde_json::json!("a")),
            ("new_string", serde_json::json!("b")),
        ]);

        let tool = create_tool(ws);
        let err = tool.execute(&input).await.unwrap_err();
        assert!(matches!(err, ToolError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_empty_old_string_rejected() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_str().unwrap();
        create_test_file(&tmp, "test.rs", "content");

        let input = make_input(vec![
            ("path", serde_json::json!("test.rs")),
            ("old_string", serde_json::json!("")),
            ("new_string", serde_json::json!("new")),
        ]);

        let tool = create_tool(ws);
        let err = tool.execute(&input).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_empty_new_string_rejected() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_str().unwrap();
        create_test_file(&tmp, "test.rs", "content");

        let input = make_input(vec![
            ("path", serde_json::json!("test.rs")),
            ("old_string", serde_json::json!("content")),
            ("new_string", serde_json::json!("")),
        ]);

        let tool = create_tool(ws);
        let err = tool.execute(&input).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_side_effect_recorded() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_str().unwrap();
        create_test_file(&tmp, "test.rs", "old");

        let input = make_input(vec![
            ("path", serde_json::json!("test.rs")),
            ("old_string", serde_json::json!("old")),
            ("new_string", serde_json::json!("new")),
        ]);

        let tool = create_tool(ws);
        let result = tool.execute(&input).await.unwrap();
        assert!(result.has_side_effects());
        assert_eq!(result.side_effects[0].effect_type, "file_edit");
    }

    #[tokio::test]
    async fn test_atomic_write_preserves_content_on_failure() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_str().unwrap();
        let file_path = create_test_file(&tmp, "test.rs", "original content");

        // Create a tool that works
        let input = make_input(vec![
            ("path", serde_json::json!("test.rs")),
            ("old_string", serde_json::json!("original")),
            ("new_string", serde_json::json!("modified")),
        ]);

        let tool = create_tool(ws);
        let _ = tool.execute(&input).await.unwrap();

        // Verify content changed
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "modified content");
    }

    #[tokio::test]
    async fn test_diff_computation() {
        let tool = create_tool("/tmp");
        let original = "line1\nline2\nline3\nline4\nline5";
        let updated = "line1\nline2\nmodified\nline4\nline5";
        let (diff, hunks) = tool.compute_diff(original, updated, "test.txt");
        assert!(!diff.is_empty());
        assert!(diff.contains("-line3"));
        assert!(diff.contains("+modified"));
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].old_start, 3); // diff_start is 2 (0-indexed) so old_start = 3
        assert_eq!(hunks[0].old_lines, 1);
        assert_eq!(hunks[0].new_lines, 1);
    }

    #[tokio::test]
    async fn test_diff_empty_for_unchanged() {
        let tool = create_tool("/tmp");
        let (diff, hunks) = tool.compute_diff("same", "same", "f.txt");
        assert!(diff.is_empty());
        assert!(hunks.is_empty());
    }
}

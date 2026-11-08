//! FileWriteTool and FileAppendTool — file writing operations.
//!
//! @canonical .pi/architecture/modules/tool-system.md#file-write
//! Implements: Tool trait — FileWrite and FileAppend concrete tools
//! Issue: #125, #1479
//!
//! FileWriteTool writes/overwrites files using an atomic write-rename pattern.
//! FileAppendTool appends content to existing files.
//!
//! # Validation and mutation share a handle (#1479, SINK-2)
//!
//! This module used to canonicalize the deepest existing ancestor, return a
//! `PathBuf`, and then re-resolve that path through `create_dir_all` / `fs::write` /
//! `fs::rename`. The check and the mutation were separate events, so a component
//! swapped for a symlink in between redirected the write. The mutation now goes
//! through [`crate::tools::infrastructure::fs_at`], which returns an opened parent
//! directory handle and mutates relative to it; this file no longer touches the
//! filesystem itself, and cannot: it never holds a path it can write to.
//!
//! # What this does not do
//!
//! It refuses a symlinked component rather than following it, and it cannot
//! survive a full disk. See `.pi/architecture/sink-register.tsv`
//! (`tools:file-write-path`) for the `does_not` column.

use async_trait::async_trait;

use crate::tools::application::dto::{SideEffect, ToolInput, ToolResult};
use crate::tools::domain::{Tool, ToolError};
use crate::tools::infrastructure::fs_at;

/// Run a blocking, single-directory mutation off the async runtime.
///
/// `openat`/`write`/`renameat` are bounded syscalls, but a large `content` is not
/// something to write inline on a worker thread — the previous implementation used
/// `tokio::fs` for the same reason.
async fn off_runtime<F>(operation: F) -> Result<(), ToolError>
where
    F: FnOnce() -> Result<(), ToolError> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("file task failed: {e}")))?
}

// ---------------------------------------------------------------------------
// FileWriteTool
// ---------------------------------------------------------------------------

/// Tool for writing content to files using atomic write-rename pattern.
///
/// # Input Parameters
/// - `path` (required, string): Path to the file to write.
/// - `content` (required, string): Content to write to the file.
///
/// # Risk Level
/// Medium — modifies files on disk.
///
/// # Atomicity
/// Writes to a temporary file first, then renames to the target path
/// to prevent partial writes from being visible.
pub struct FileWriteTool {
    /// Root directory for path resolution and validation.
    workspace_root: String,
}

impl FileWriteTool {
    /// Create a new FileWriteTool with the given workspace root.
    pub fn new(workspace_root: impl Into<String>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    #[tracing::instrument(skip_all)]
    fn name(&self) -> &str {
        "file-write"
    }

    #[tracing::instrument(skip_all)]
    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let path_str = input.require_string("path")?;
        let content = input.require_string("content")?;

        // #1479: one resolution, one handle. The target is opened here and mutated
        // through the same handle by `fs_at`; nothing in this file re-resolves it.
        let target = fs_at::resolve_for_write(&self.workspace_root, &path_str)?;
        let bytes = content.len();
        off_runtime(move || fs_at::write_atomic(&target, content.as_bytes())).await?;

        let result = ToolResult {
            output: format!("Written {} bytes to '{}'", bytes, path_str),
            exit_code: 0,
            side_effects: vec![SideEffect::new(
                &path_str,
                "file_write",
                format!("Written {bytes} bytes"),
            )],
            duration_ms: 0,
            dry_run: false,
        };

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// FileAppendTool
// ---------------------------------------------------------------------------

/// Tool for appending content to existing files.
///
/// # Input Parameters
/// - `path` (required, string): Path to the file to append to.
/// - `content` (required, string): Content to append.
///
/// # Risk Level
/// Medium — modifies files on disk.
pub struct FileAppendTool {
    /// Root directory for path resolution and validation.
    workspace_root: String,
}

impl FileAppendTool {
    /// Create a new FileAppendTool with the given workspace root.
    pub fn new(workspace_root: impl Into<String>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

#[async_trait]
impl Tool for FileAppendTool {
    #[tracing::instrument(skip_all)]
    fn name(&self) -> &str {
        "file-append"
    }

    #[tracing::instrument(skip_all)]
    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let path_str = input.require_string("path")?;
        let content = input.require_string("content")?;

        // #1479: appended through the pinned parent handle, `O_APPEND|O_NOFOLLOW`.
        // A missing file is an error (use file-write), and an existing symlink at
        // the final component is refused rather than written through.
        let target = fs_at::resolve_for_write(&self.workspace_root, &path_str)?;
        let bytes = content.len();
        off_runtime(move || fs_at::append_to_file(&target, content.as_bytes())).await?;

        let result = ToolResult {
            output: format!("Appended {} bytes to '{}'", bytes, path_str),
            exit_code: 0,
            side_effects: vec![SideEffect::new(
                &path_str,
                "file_append",
                format!("Appended {bytes} bytes"),
            )],
            duration_ms: 0,
            dry_run: false,
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[tracing::instrument(skip_all)]
    fn make_input(path: &str, content: &str) -> ToolInput {
        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::Value::String(path.to_string()),
        );
        params.insert(
            "content".to_string(),
            serde_json::Value::String(content.to_string()),
        );
        ToolInput::new(params)
    }

    // -----------------------------------------------------------------------
    // FileWriteTool tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_write_new_file() {
        let dir = TempDir::new().unwrap();
        let tool = FileWriteTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input("test.txt", "hello world"))
            .await
            .unwrap();

        assert!(result.is_success());
        assert!(result.has_side_effects());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("test.txt")).unwrap(),
            "hello world"
        );
    }

    #[tokio::test]
    async fn test_write_overwrites_existing() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "original").unwrap();

        let tool = FileWriteTool::new(dir.path().to_str().unwrap());
        tool.execute(&make_input("test.txt", "overwritten"))
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "overwritten");
    }

    #[tokio::test]
    async fn test_write_creates_subdirectories() {
        let dir = TempDir::new().unwrap();
        let tool = FileWriteTool::new(dir.path().to_str().unwrap());
        tool.execute(&make_input("sub/dir/file.txt", "nested"))
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join("sub/dir/file.txt")).unwrap(),
            "nested"
        );
    }

    #[tokio::test]
    async fn test_write_path_traversal_denied() {
        let dir = TempDir::new().unwrap();
        let tool = FileWriteTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("../outside.txt", "content")).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::PathDenied(_)));
    }

    #[tokio::test]
    async fn test_write_missing_parameters() {
        let dir = TempDir::new().unwrap();
        let tool = FileWriteTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&ToolInput::new(HashMap::new())).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_write_tool_name() {
        let dir = TempDir::new().unwrap();
        let tool = FileWriteTool::new(dir.path().to_str().unwrap());
        assert_eq!(tool.name(), "file-write");
    }

    // -----------------------------------------------------------------------
    // #1479 (SINK-2): the check and the mutation share a handle
    // -----------------------------------------------------------------------

    /// #1479 acceptance 1: nothing in this file re-resolves a path after
    /// validation — the mutation lives in `fs_at`, behind an opaque handle.
    #[test]
    fn the_tool_does_not_touch_the_filesystem_itself() {
        // Comments name the calls this file used to make; the code may not.
        let source = include_str!("file_write_tool.rs");
        // Only the tool's own code: this test names the forbidden calls, and the
        // module docs name the calls the file used to make.
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        let code: String = production
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in ["fs::write", "fs::rename", "create_dir_all", "tokio::fs::"] {
            assert!(
                !code.contains(forbidden),
                "file-write must mutate only through fs_at's pinned handle, found `{forbidden}`"
            );
        }
    }

    /// #1479 acceptance 3, at the tool boundary: a symlinked component is a
    /// `PathDenied`, not a followed link.
    #[tokio::test]
    async fn test_write_through_a_symlinked_component_is_denied() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();

        let tool = FileWriteTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input("link/escape.txt", "content"))
            .await;

        assert!(
            matches!(result, Err(ToolError::PathDenied(_))),
            "a symlinked component must be refused: {result:?}"
        );
        assert!(!outside.path().join("escape.txt").exists());
    }

    // -----------------------------------------------------------------------
    // FileAppendTool tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_append_to_existing_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello ").unwrap();

        let tool = FileAppendTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input("test.txt", "world"))
            .await
            .unwrap();

        assert!(result.is_success());
        assert!(result.has_side_effects());
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "hello world");
    }

    #[tokio::test]
    async fn test_append_to_nonexistent_file() {
        let dir = TempDir::new().unwrap();
        let tool = FileAppendTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input("nonexistent.txt", "content"))
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn test_append_path_traversal_denied() {
        let dir = TempDir::new().unwrap();
        let tool = FileAppendTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("../outside.txt", "content")).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::PathDenied(_)));
    }

    #[tokio::test]
    async fn test_append_tool_name() {
        let dir = TempDir::new().unwrap();
        let tool = FileAppendTool::new(dir.path().to_str().unwrap());
        assert_eq!(tool.name(), "file-append");
    }
}

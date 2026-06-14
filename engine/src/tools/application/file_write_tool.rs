//! FileWriteTool and FileAppendTool — file writing operations.
//!
//! @canonical .pi/architecture/modules/tool-system.md#file-write
//! Implements: Tool trait — FileWrite and FileAppend concrete tools
//! Issue: #125
//!
//! FileWriteTool writes/overwrites files using an atomic write-rename pattern.
//! FileAppendTool appends content to existing files.
//!
//! Both validate that paths are within the workspace root to prevent
//! directory traversal attacks.

use async_trait::async_trait;
use std::path::Path;

use crate::tools::application::dto::{SideEffect, ToolInput, ToolResult};
use crate::tools::domain::{Tool, ToolError};

// ---------------------------------------------------------------------------
// Helper: path validation
// ---------------------------------------------------------------------------

fn resolve_and_validate_path(
    workspace_root: &str,
    path_str: &str,
) -> Result<std::path::PathBuf, ToolError> {
    let root = Path::new(workspace_root);

    // Check for path traversal before requiring canonicalization
    let normalized = path_str.replace('\\', "/");
    if normalized.contains("..") {
        return Err(ToolError::PathDenied(format!(
            "Path '{}' contains '..' and could escape workspace root",
            path_str
        )));
    }

    let path = root.join(path_str);

    // For write operations, the file may not exist yet.
    // Validate by checking the parent directory exists or can be created.
    let parent = path.parent().ok_or_else(|| {
        ToolError::InvalidInput(format!("Invalid path: {}", path_str))
    })?;

    // Find the deepest existing ancestor to canonicalize and validate
    let mut check = Some(parent);
    while let Some(p) = check {
        if p.exists() {
            let p_canonical = p.canonicalize().map_err(|e| {
                ToolError::ExecutionFailed(format!("Cannot resolve path: {}", e))
            })?;
            let root_canonical = root.canonicalize().map_err(|_| {
                ToolError::ExecutionFailed("Cannot resolve workspace root".to_string())
            })?;
            if !p_canonical.starts_with(&root_canonical) {
                return Err(ToolError::PathDenied(format!(
                    "Path '{}' is outside workspace root",
                    path_str
                )));
            }
            return Ok(path);
        }
        check = p.parent();
    }

    // No existing ancestor found — validate root itself
    let root_canonical = root.canonicalize().map_err(|_| {
        ToolError::ExecutionFailed("Cannot resolve workspace root".to_string())
    })?;

    // Path must be within workspace root
    let path_str_abs = root.join(path_str);
    if !path_str_abs.starts_with(&root_canonical) {
        return Err(ToolError::PathDenied(format!(
            "Path '{}' is outside workspace root",
            path_str
        )));
    }

    Ok(path)
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
    fn name(&self) -> &str {
        "file-write"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let path_str = input.require_string("path")?;
        let content = input.require_string("content")?;
        let resolved = resolve_and_validate_path(&self.workspace_root, &path_str)?;

        // Ensure parent directory exists
        if let Some(parent) = resolved.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| {
                    ToolError::ExecutionFailed(format!(
                        "Failed to create directory '{}': {}",
                        parent.display(),
                        e
                    ))
                })?;
        }

        // Atomic write: write to temp file, then rename
        let temp_path = resolved.with_extension(format!(
            ".tmp.{}",
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
        ));

        tokio::fs::write(&temp_path, &content).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to write file '{}': {}", path_str, e))
        })?;

        tokio::fs::rename(&temp_path, &resolved)
            .await
            .map_err(|e| {
                // Clean up temp file on rename failure
                let _ = std::fs::remove_file(&temp_path);
                ToolError::ExecutionFailed(format!(
                    "Failed to atomically write file '{}': {}",
                    path_str, e
                ))
            })?;

        let result = ToolResult {
            output: format!("Written {} bytes to '{}'", content.len(), path_str),
            exit_code: 0,
            side_effects: vec![SideEffect::new(
                &path_str,
                "file_write",
                format!("Written {} bytes", content.len()),
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
    fn name(&self) -> &str {
        "file-append"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let path_str = input.require_string("path")?;
        let content = input.require_string("content")?;
        let resolved = resolve_and_validate_path(&self.workspace_root, &path_str)?;

        // Check file exists before appending
        if !resolved.exists() {
            return Err(ToolError::ExecutionFailed(format!(
                "File '{}' does not exist. Use file-write to create new files.",
                path_str
            )));
        }

        let mut existing = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to read file '{}': {}", path_str, e))
            })?;

        existing.push_str(&content);

        tokio::fs::write(&resolved, &existing).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to append to file '{}': {}", path_str, e))
        })?;

        let result = ToolResult {
            output: format!("Appended {} bytes to '{}'", content.len(), path_str),
            exit_code: 0,
            side_effects: vec![SideEffect::new(
                &path_str,
                "file_append",
                format!("Appended {} bytes", content.len()),
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
        let result = tool.execute(&make_input("test.txt", "hello world")).await.unwrap();

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

        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "overwritten"
        );
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
    // FileAppendTool tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_append_to_existing_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello ").unwrap();

        let tool = FileAppendTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("test.txt", "world")).await.unwrap();

        assert!(result.is_success());
        assert!(result.has_side_effects());
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "hello world"
        );
    }

    #[tokio::test]
    async fn test_append_to_nonexistent_file() {
        let dir = TempDir::new().unwrap();
        let tool = FileAppendTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("nonexistent.txt", "content")).await;

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

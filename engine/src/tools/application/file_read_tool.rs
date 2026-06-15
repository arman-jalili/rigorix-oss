//! FileReadTool — reads a file from the workspace.
//!
//! @canonical .pi/architecture/modules/tool-system.md#file-read
//! Implements: Tool trait — FileRead concrete tool
//! Issue: #125
//!
//! Reads file contents from a path within the workspace.
//! Validates that the path is within the allowed workspace root
//! to prevent directory traversal attacks.
//!
//! # Contract
//! - Path must be within workspace root
//! - Read-only operation (Low risk)
//! - Returns file contents as output text

use async_trait::async_trait;
use std::path::Path;

use crate::tools::application::dto::{ToolInput, ToolResult};
use crate::tools::domain::{Tool, ToolError};

/// Tool for reading file contents from the workspace.
///
/// # Input Parameters
/// - `path` (required, string): Path to the file to read, relative to workspace root.
///
/// # Risk Level
/// Low — read-only, no side effects.
pub struct FileReadTool {
    /// Root directory for path resolution and validation.
    workspace_root: String,
}

impl FileReadTool {
    /// Create a new FileReadTool with the given workspace root.
    ///
    /// All file paths are validated against this root to prevent
    /// directory traversal attacks.
    pub fn new(workspace_root: impl Into<String>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    /// Resolve and validate the file path against the workspace root.
    ///
    /// For read operations, the file must exist (canonicalize succeeds).
    /// For path traversal detection, we check the path before canonicalization
    /// using the normalized form.
    fn resolve_path(&self, path_str: &str) -> Result<std::path::PathBuf, ToolError> {
        let root = Path::new(&self.workspace_root);

        // First, check for obvious path traversal (../) without requiring canonicalization
        let normalized = path_str.replace('\\', "/");
        if normalized.contains("..") {
            // Only deny if the traversal actually escapes the workspace root
            let path = root.join(path_str);
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
                    // Path doesn't exist but contains ".." — treat as potential traversal
                    return Err(ToolError::PathDenied(format!(
                        "Path '{}' contains '..' and could escape workspace root",
                        path_str
                    )));
                }
            }
        }

        let path = root.join(path_str);
        let canonical = path.canonicalize().map_err(|e| {
            ToolError::ExecutionFailed(format!("Cannot access path '{}': {}", path_str, e))
        })?;

        let root_canonical = root
            .canonicalize()
            .map_err(|_| ToolError::ExecutionFailed("Cannot resolve workspace root".to_string()))?;

        if !canonical.starts_with(&root_canonical) {
            return Err(ToolError::PathDenied(format!(
                "Path '{}' is outside workspace root",
                path_str
            )));
        }

        Ok(canonical)
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file-read"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let path_str = input.require_string("path")?;
        let resolved = self.resolve_path(&path_str)?;

        let contents = tokio::fs::read_to_string(&resolved).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read file '{}': {}", path_str, e))
        })?;

        Ok(ToolResult::success(contents))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_input(path: &str) -> ToolInput {
        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::Value::String(path.to_string()),
        );
        ToolInput::new(params)
    }

    #[tokio::test]
    async fn test_read_existing_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world").unwrap();

        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("test.txt")).await.unwrap();

        assert!(result.is_success());
        assert_eq!(result.output, "hello world");
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let dir = TempDir::new().unwrap();
        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("nonexistent.txt")).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn test_read_with_missing_parameter() {
        let dir = TempDir::new().unwrap();
        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&ToolInput::new(HashMap::new())).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_path_traversal_outside_workspace() {
        let dir = TempDir::new().unwrap();
        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("../etc/passwd")).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::PathDenied(_)));
    }

    #[tokio::test]
    async fn test_name_returns_correct_name() {
        let dir = TempDir::new().unwrap();
        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        assert_eq!(tool.name(), "file-read");
    }

    #[tokio::test]
    async fn test_read_empty_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("empty.txt");
        std::fs::write(&file_path, "").unwrap();

        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("empty.txt")).await.unwrap();

        assert!(result.is_success());
        assert_eq!(result.output, "");
    }

    #[tokio::test]
    async fn test_read_file_in_subdirectory() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("nested.txt"), "nested content").unwrap();

        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input("subdir/nested.txt"))
            .await
            .unwrap();

        assert!(result.is_success());
        assert_eq!(result.output, "nested content");
    }
}

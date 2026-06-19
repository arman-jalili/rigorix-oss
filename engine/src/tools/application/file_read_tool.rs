//! FileReadTool — reads a file from the workspace with offset/limit and binary detection.
//!
//! @canonical .pi/architecture/modules/tool-system.md#file-read
//! Implements: Tool trait — FileRead concrete tool
//! Issue: #429
//!
//! Reads file contents from a path within the workspace.
//! Supports:
//! - Offset/limit paging (line-based, 1-indexed)
//! - Binary file detection via NUL byte scan (first 8KB)
//! - File size limits (10MB cap)
//! - Returns total_lines for the LLM to understand file scope
//!
//! # Contract
//! - Path must be within workspace root
//! - Read-only operation (Low risk)
//! - Returns file contents as output text (JSON with content, start_line, total_lines)
//!
//! # Input Parameters
//! - `path` (required, string): Path to the file to read.
//! - `offset` (optional, number): Starting line offset (1-indexed, default: 1).
//! - `limit` (optional, number): Maximum lines to return (default: all).

use async_trait::async_trait;
use std::io::Read;
use std::path::Path;

use crate::code_gen::application::dto::ReadFileOutput;
use crate::tools::application::dto::{ToolInput, ToolResult};
use crate::tools::domain::{Tool, ToolError};

/// Maximum file size in bytes (10 MB).
const MAX_FILE_SIZE: u64 = 10_485_760;

/// Number of bytes to read for binary detection.
const BINARY_SCAN_SIZE: usize = 8192;

/// Tool for reading file contents from the workspace.
///
/// # Input Parameters
/// - `path` (required, string): Path to the file to read, relative to workspace root.
/// - `offset` (optional, number): Starting line offset (1-indexed). Default: 1.
/// - `limit` (optional, number): Maximum number of lines to return. Default: all.
///
/// # Output
/// Returns a JSON-serialized `ReadFileOutput` with content, start_line,
/// total_lines, total_bytes, and is_binary fields.
///
/// # Risk Level
/// Low — read-only, no side effects.
pub struct FileReadTool {
    /// Root directory for path resolution and validation.
    workspace_root: String,
}

impl FileReadTool {
    /// Create a new FileReadTool with the given workspace root.
    pub fn new(workspace_root: impl Into<String>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    /// Detect whether a file is binary by scanning for NUL bytes.
    fn detect_binary(path: &Path) -> Result<bool, ToolError> {
        let mut file = std::fs::File::open(path).map_err(|e| {
            ToolError::ExecutionFailed(format!("Cannot open file for binary detection: {}", e))
        })?;

        let mut buffer = vec![0u8; BINARY_SCAN_SIZE];
        let n = file.read(&mut buffer).map_err(|e| {
            ToolError::ExecutionFailed(format!("Cannot read file for binary detection: {}", e))
        })?;

        Ok(buffer[..n].contains(&0u8))
    }

    /// Resolve and validate the file path against the workspace root.
    fn resolve_path(&self, path_str: &str) -> Result<std::path::PathBuf, ToolError> {
        let root = Path::new(&self.workspace_root);

        let normalized = path_str.replace('\\', "/");
        if normalized.contains("..") {
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

        // Check file exists and get metadata
        let metadata = tokio::fs::metadata(&resolved).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Cannot access file '{}': {}", path_str, e))
        })?;

        let file_size = metadata.len();

        // Gate: File size check
        if file_size > MAX_FILE_SIZE {
            return Err(ToolError::InvalidInput(format!(
                "File too large: {} bytes (max {})",
                file_size, MAX_FILE_SIZE
            )));
        }

        // Gate: Binary detection (scan on open)
        let is_binary = Self::detect_binary(&resolved)?;
        if is_binary {
            return Err(ToolError::InvalidInput(format!(
                "Cannot read binary file: {}",
                path_str
            )));
        }

        // Read file contents
        let contents = tokio::fs::read_to_string(&resolved).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read file '{}': {}", path_str, e))
        })?;

        let total_lines = contents.lines().count();

        // Parse optional offset/limit
        let offset = input.get_u64("offset").map(|v| v as usize).unwrap_or(1).max(1);
        let limit = input.get_u64("limit").map(|v| v as usize);

        // Apply offset/limit
        let (extracted_content, start_line) = if offset > 1 || limit.is_some() {
            let lines: Vec<&str> = contents.lines().collect();
            let start_idx = (offset - 1).min(lines.len());
            let end_idx = match limit {
                Some(l) => (start_idx + l).min(lines.len()),
                None => lines.len(),
            };
            (lines[start_idx..end_idx].join("\n"), offset)
        } else {
            (contents, 1usize)
        };

        let output = ReadFileOutput {
            file_path: path_str,
            content: extracted_content,
            start_line,
            total_lines,
            total_bytes: file_size,
            is_binary,
            requested_offset: input.get_u64("offset").map(|v| v as usize),
            requested_limit: input.get_u64("limit").map(|v| v as usize),
        };

        let output_json = serde_json::to_string(&output).map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to serialize output: {}", e))
        })?;

        Ok(ToolResult::success(output_json))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_input(path: &str) -> ToolInput {
        let mut params = HashMap::new();
        params.insert("path".to_string(), serde_json::Value::String(path.to_string()));
        ToolInput::new(params)
    }

    fn make_input_with_opts(path: &str, offset: Option<u64>, limit: Option<u64>) -> ToolInput {
        let mut params = HashMap::new();
        params.insert("path".to_string(), serde_json::Value::String(path.to_string()));
        if let Some(o) = offset {
            params.insert("offset".to_string(), serde_json::json!(o));
        }
        if let Some(l) = limit {
            params.insert("limit".to_string(), serde_json::json!(l));
        }
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
        let output: ReadFileOutput = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output.content, "hello world");
        assert_eq!(output.start_line, 1);
        assert_eq!(output.total_lines, 1);
        assert!(!output.is_binary);
    }

    #[tokio::test]
    async fn test_read_with_offset() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3\nline4\nline5").unwrap();

        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input_with_opts("test.txt", Some(3), None)).await.unwrap();

        let output: ReadFileOutput = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output.content, "line3\nline4\nline5");
        assert_eq!(output.start_line, 3);
        assert_eq!(output.total_lines, 5);
    }

    #[tokio::test]
    async fn test_read_with_limit() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3\nline4\nline5").unwrap();

        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input_with_opts("test.txt", Some(2), Some(2))).await.unwrap();

        let output: ReadFileOutput = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output.content, "line2\nline3");
        assert_eq!(output.start_line, 2);
    }

    #[tokio::test]
    async fn test_read_with_offset_beyond_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2").unwrap();

        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input_with_opts("test.txt", Some(10), None)).await.unwrap();

        let output: ReadFileOutput = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output.content, "");
        assert_eq!(output.start_line, 10);
    }

    #[tokio::test]
    async fn test_binary_file_rejected() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("binary.bin");
        std::fs::write(&file_path, &[0u8, 1, 2, 3]).unwrap();

        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("binary.bin")).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let dir = TempDir::new().unwrap();
        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("nonexistent.txt")).await;
        assert!(result.is_err());
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
    }

    #[tokio::test]
    async fn test_total_lines_reported() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "a\nb\nc\nd\ne\nf").unwrap();

        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("test.txt")).await.unwrap();
        let output: ReadFileOutput = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output.total_lines, 6);
    }

    #[tokio::test]
    async fn test_total_bytes_reported() {
        let dir = TempDir::new().unwrap();
        let content = "hello";
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, content).unwrap();

        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("test.txt")).await.unwrap();
        let output: ReadFileOutput = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output.total_bytes, content.len() as u64);
    }
}

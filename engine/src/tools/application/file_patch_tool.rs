//! FilePatchTool — AST-aware file patching with search/replace.
//!
//! @canonical .pi/architecture/modules/tool-system.md#file-patch
//! Implements: Tool trait — FilePatch concrete tool
//! Issue: #125
//!
//! Finds a unique search string in a file and inserts content before or after it.
//! Validates that the search string appears exactly once to prevent ambiguity.
//! Validates that paths are within the workspace root.

use async_trait::async_trait;
use std::path::Path;

use crate::tools::application::dto::{SideEffect, ToolInput, ToolResult};
use crate::tools::domain::{Tool, ToolError};

/// Tool for AST-aware file patching with search/replace.
///
/// # Input Parameters
/// - `path` (required, string): Path to the file to patch.
/// - `search` (required, string): Search string to locate the insertion point.
/// - `insert` (required, string): Content to insert.
/// - `before` (optional, bool): Insert before the search match (default: false, insert after).
///
/// # Risk Level
/// Medium — modifies files on disk.
///
/// # Ambiguity Protection
/// The search string must appear exactly once in the file. If it appears
/// zero or multiple times, an error is returned to prevent unintended edits.
pub struct FilePatchTool {
    /// Root directory for path resolution and validation.
    workspace_root: String,
}

impl FilePatchTool {
    /// Create a new FilePatchTool with the given workspace root.
    pub fn new(workspace_root: impl Into<String>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    fn resolve_path(&self, path_str: &str) -> Result<std::path::PathBuf, ToolError> {
        let root = Path::new(&self.workspace_root);

        // Check for path traversal before canonicalization
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

        let root_canonical = root.canonicalize().map_err(|_| {
            ToolError::ExecutionFailed("Cannot resolve workspace root".to_string())
        })?;

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
impl Tool for FilePatchTool {
    fn name(&self) -> &str {
        "file-patch"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let path_str = input.require_string("path")?;
        let search = input.require_string("search")?;
        let insert = input.require_string("insert")?;
        let before = input.get_string("before").map(|s| s == "true").unwrap_or(false);

        let resolved = self.resolve_path(&path_str)?;

        let contents = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to read file '{}': {}", path_str, e))
            })?;

        // Find all occurrences of the search string
        let occurrences: Vec<_> = contents.match_indices(&search).collect();

        if occurrences.is_empty() {
            return Err(ToolError::ExecutionFailed(format!(
                "Search string '{}' not found in file '{}'",
                search, path_str
            )));
        }

        if occurrences.len() > 1 {
            return Err(ToolError::ExecutionFailed(format!(
                "Search string '{}' found {} times in file '{}'. Expected exactly one match.",
                search,
                occurrences.len(),
                path_str
            )));
        }

        let (match_pos, _) = occurrences[0];
        let insert_pos = if before {
            match_pos
        } else {
            match_pos + search.len()
        };

        let new_contents = format!("{}{}{}", &contents[..insert_pos], insert, &contents[insert_pos..]);

        tokio::fs::write(&resolved, &new_contents).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to write patched file '{}': {}", path_str, e))
        })?;

        let patched_len = insert.len();
        let result = ToolResult {
            output: format!(
                "Inserted {} bytes {} '{}' in '{}'",
                patched_len,
                if before { "before" } else { "after" },
                search,
                path_str
            ),
            exit_code: 0,
            side_effects: vec![SideEffect::new(
                &path_str,
                "file_patch",
                format!("Inserted {} bytes {} '{}'", patched_len, if before { "before" } else { "after" }, search),
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

    fn make_input(path: &str, search: &str, insert: &str) -> ToolInput {
        let mut params = HashMap::new();
        params.insert("path".to_string(), serde_json::Value::String(path.to_string()));
        params.insert("search".to_string(), serde_json::Value::String(search.to_string()));
        params.insert("insert".to_string(), serde_json::Value::String(insert.to_string()));
        ToolInput::new(params)
    }

    fn make_input_with_before(path: &str, search: &str, insert: &str, before: bool) -> ToolInput {
        let mut params = HashMap::new();
        params.insert("path".to_string(), serde_json::Value::String(path.to_string()));
        params.insert("search".to_string(), serde_json::Value::String(search.to_string()));
        params.insert("insert".to_string(), serde_json::Value::String(insert.to_string()));
        params.insert("before".to_string(), serde_json::Value::String(before.to_string()));
        ToolInput::new(params)
    }

    #[tokio::test]
    async fn test_patch_insert_after() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {\n}").unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("test.rs", "{\n", "    println!(\"hi\");\n")).await.unwrap();

        assert!(result.is_success());
        assert!(result.has_side_effects());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("println!(\"hi\")"));
    }

    #[tokio::test]
    async fn test_patch_insert_before() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "println!(\"world\");").unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input_with_before("test.rs", "println!(\"world\");", "println!(\"hello \");", true)).await.unwrap();

        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.starts_with("println!(\"hello \")"));
    }

    #[tokio::test]
    async fn test_patch_search_not_found() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("test.rs", "nonexistent", "content")).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn test_patch_ambiguous_search() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "abc abc abc").unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("test.rs", "abc", "xyz")).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn test_patch_missing_parameters() {
        let dir = TempDir::new().unwrap();
        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&ToolInput::new(HashMap::new())).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_patch_tool_name() {
        let dir = TempDir::new().unwrap();
        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        assert_eq!(tool.name(), "file-patch");
    }

    #[tokio::test]
    async fn test_patch_path_traversal_denied() {
        let dir = TempDir::new().unwrap();
        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("../outside.rs", "search", "insert")).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::PathDenied(_)));
    }
}

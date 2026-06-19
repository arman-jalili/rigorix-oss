//! FilePatchTool — AST-aware file patching with tree-sitter anchors or search/replace.
//!
//! @canonical .pi/architecture/modules/tool-system.md#file-patch
//! Implements: Tool trait — FilePatch concrete tool with tree-sitter anchor support
//! Issue: #199
//! Last Architecture Sync: 2026-06-18
//!
//! Supports two modes:
//!
//! ## Mode 1: Tree-sitter Anchor (deterministic, preferred)
//! Uses `anchor_type`, `anchor_name`, optional `container`, and `position` to
//! find the exact AST node via tree-sitter parsing. No whitespace dependence.
//!
//! ## Mode 2: Text Search (backward compatible, fallback)
//! Uses the traditional `search` string approach. The search string must appear
//! exactly once in the file to prevent ambiguity.
//!
//! # Risk Level
//! Medium — modifies files on disk.

use async_trait::async_trait;
use std::path::Path;

use crate::tools::application::dto::{SideEffect, ToolInput, ToolResult};
use crate::tools::domain::{Tool, ToolError};
use crate::tools::infrastructure::tree_sitter_anchor::{AnchorParams, TreeSitterAnchorFinder};

/// Tool for AST-aware file patching with tree-sitter anchors or search/replace.
///
/// # Input Parameters (Mode 1 — Tree-sitter Anchor, preferred)
/// - `path` (required, string): Path to the file to patch.
/// - `anchor_type` (required, string): Type of AST node to find.
///   One of: "class", "struct", "impl", "method", "function", "interface",
///   "end_of_file".
/// - `anchor_name` (required, string): Name of the symbol to find
///   (ignored for "end_of_file").
/// - `container` (optional, string): Parent scope to restrict the search
///   (e.g., class name, impl type).
/// - `position` (optional, string): Where to insert relative to the anchor.
///   "before" or "after" (default: "after").
/// - `insert` (required, string): Content to insert.
///
/// # Input Parameters (Mode 2 — Text Search, fallback)
/// - `path` (required, string): Path to the file to patch.
/// - `search` (required, string): Search string to locate the insertion point.
/// - `insert` (required, string): Content to insert.
/// - `before` (optional, bool): Insert before the search match
///   (default: false, insert after).
///
/// # Ambiguity Protection (Mode 2)
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

    /// Execute a patch using tree-sitter anchor mode.
    async fn execute_anchor(
        &self,
        path_str: &str,
        resolved: &std::path::PathBuf,
        insert: &str,
        input: &ToolInput,
    ) -> Result<ToolResult, ToolError> {
        let anchor_type = input.require_string("anchor_type")?;
        let anchor_name = input.get_string("anchor_name").unwrap_or_default();
        let container = input.get_string("container");
        let position = input
            .get_string("position")
            .unwrap_or_else(|| "after".to_string());

        let contents = tokio::fs::read_to_string(resolved).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read file '{}': {}", path_str, e))
        })?;

        let params = AnchorParams {
            anchor_type: anchor_type.clone(),
            anchor_name: anchor_name.clone(),
            container: container.clone(),
            position: position.clone(),
        };

        // Try the primary anchor lookup. If it fails because the anchor type
        // is a member (method/function) and a container is specified, fall back
        // to inserting at the end of the container body. This handles LLM
        // hallucination of method names (e.g., generated a method name that
        // doesn't exist in the source).
        let anchor = match TreeSitterAnchorFinder::find_anchor(&contents, path_str, &params) {
            Ok(a) => a,
            Err(e) => {
                // Check if we can fall back to container-level insertion
                let can_fallback = matches!(
                    params.anchor_type.as_str(),
                    "method" | "function" | "constructor"
                ) && params.container.is_some();

                if can_fallback {
                    // Retry with the container name as anchor_name and class as anchor_type
                    let fallback_params = AnchorParams {
                        anchor_type: "class".to_string(),
                        anchor_name: params.container.clone().unwrap(),
                        container: None,
                        position: "after".to_string(),
                    };
                    match TreeSitterAnchorFinder::find_anchor(&contents, path_str, &fallback_params) {
                        Ok(fallback_anchor) => fallback_anchor,
                        Err(_) => return Err(e), // Return original error if fallback also fails
                    }
                } else {
                    return Err(e);
                }
            }
        };

        let new_contents = format!(
            "{}{}{}",
            &contents[..anchor.insert_offset],
            insert,
            &contents[anchor.insert_offset..]
        );

        tokio::fs::write(resolved, &new_contents)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to write patched file '{}': {}",
                    path_str, e
                ))
            })?;

        let patched_len = insert.len();
        let result = ToolResult {
            output: format!(
                "Inserted {} bytes {} '{}' in '{}'",
                patched_len, anchor.description, params.anchor_name, path_str
            ),
            exit_code: 0,
            side_effects: vec![SideEffect::new(
                path_str,
                "file_patch",
                format!(
                    "Inserted {} bytes {} '{}' via anchor {}",
                    patched_len, anchor.description, params.anchor_name, params.anchor_type
                ),
            )],
            duration_ms: 0,
            dry_run: false,
        };

        Ok(result)
    }

    /// Execute a patch using text search fallback mode (original behavior).
    async fn execute_search(
        &self,
        path_str: &str,
        resolved: &std::path::PathBuf,
        search: &str,
        insert: &str,
        before: bool,
    ) -> Result<ToolResult, ToolError> {
        let contents = tokio::fs::read_to_string(resolved).await.map_err(|e| {
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
        // Try to smart-resolve: if the search string matched a container declaration
        // (class/struct/impl/interface), tree-sitter can correct the position to
        // inside the body before the closing brace.
        let position_str = if before { "before" } else { "after" };
        let resolved_pos = crate::tools::infrastructure::tree_sitter_anchor::TreeSitterAnchorFinder::resolve_search_to_container(
            &contents,
            path_str,
            match_pos,
            position_str,
        );
        let insert_pos = resolved_pos.unwrap_or_else(|| {
            if before {
                match_pos
            } else {
                match_pos + search.len()
            }
        });
        let used_smart = resolved_pos.is_some();

        let new_contents = format!(
            "{}{}{}",
            &contents[..insert_pos],
            insert,
            &contents[insert_pos..]
        );

        tokio::fs::write(resolved, &new_contents)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to write patched file '{}': {}",
                    path_str, e
                ))
            })?;

        let patched_len = insert.len();
        let position_label = if used_smart {
            "inside container body"
        } else if before {
            "before"
        } else {
            "after"
        };
        let result = ToolResult {
            output: format!(
                "Inserted {} bytes {} '{}' in '{}'",
                patched_len, position_label, search, path_str
            ),
            exit_code: 0,
            side_effects: vec![SideEffect::new(
                path_str,
                "file_patch",
                format!(
                    "Inserted {} bytes {} '{}'",
                    patched_len, position_label, search
                ),
            )],
            duration_ms: 0,
            dry_run: false,
        };

        Ok(result)
    }
}

#[async_trait]
impl Tool for FilePatchTool {
    fn name(&self) -> &str {
        "file-patch"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let path_str = input.require_string("path")?;
        let insert = input.require_string("insert")?;
        let resolved = self.resolve_path(&path_str)?;

        // Determine mode: anchor_mode if anchor_type is provided, otherwise search mode
        let has_anchor = input.get_string("anchor_type").is_some();

        if has_anchor {
            self.execute_anchor(&path_str, &resolved, &insert, input)
                .await
        } else {
            let search = input.require_string("search")?;
            let before = input
                .get_string("before")
                .map(|s| s == "true")
                .unwrap_or(false);
            self.execute_search(&path_str, &resolved, &search, &insert, before)
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    // ------------------------------------------------------------------
    // Helper: build ToolInput from key-value pairs
    // ------------------------------------------------------------------

    fn make_input(pairs: Vec<(&str, &str)>) -> ToolInput {
        let mut params = HashMap::new();
        for (key, value) in pairs {
            params.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
        ToolInput::new(params)
    }

    // ------------------------------------------------------------------
    // Search mode tests (backward compatible)
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_search_insert_after() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {\n}").unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.rs"),
                ("search", "{\n"),
                ("insert", "    println!(\"hi\");\n"),
            ]))
            .await
            .unwrap();

        assert!(result.is_success());
        assert!(result.has_side_effects());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("println!(\"hi\")"));
    }

    #[tokio::test]
    async fn test_search_insert_before() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "println!(\"world\");").unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.rs"),
                ("search", "println!(\"world\");"),
                ("insert", "println!(\"hello \");"),
                ("before", "true"),
            ]))
            .await
            .unwrap();

        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.starts_with("println!(\"hello \")"));
    }

    #[tokio::test]
    async fn test_search_not_found() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.rs"),
                ("search", "nonexistent"),
                ("insert", "content"),
            ]))
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn test_search_ambiguous() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "abc abc abc").unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.rs"),
                ("search", "abc"),
                ("insert", "xyz"),
            ]))
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::ExecutionFailed(_)));
    }

    // ------------------------------------------------------------------
    // Anchor mode tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_anchor_rust_function_after() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(
            &file_path,
            "fn existing() {\n    let x = 1;\n}\n\nfn other() {\n    let y = 2;\n}\n",
        )
        .unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.rs"),
                ("anchor_type", "function"),
                ("anchor_name", "existing"),
                ("insert", "\nfn new_func() {\n    let z = 3;\n}\n"),
                ("position", "after"),
            ]))
            .await
            .unwrap();

        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("new_func"));
        // new_func should appear after existing, before other
        let existing_pos = content.find("fn existing").unwrap();
        let new_func_pos = content.find("fn new_func").unwrap();
        let other_pos = content.find("fn other").unwrap();
        assert!(
            existing_pos < new_func_pos,
            "new_func should be after existing"
        );
        assert!(new_func_pos < other_pos, "new_func should be before other");
    }

    #[tokio::test]
    async fn test_anchor_rust_function_before() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn alpha() {}\n\nfn beta() {}\n").unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.rs"),
                ("anchor_type", "function"),
                ("anchor_name", "beta"),
                ("insert", "fn between() {}\n\n"),
                ("position", "before"),
            ]))
            .await
            .unwrap();

        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();
        let alpha_pos = content.find("fn alpha").unwrap();
        let between_pos = content.find("fn between").unwrap();
        let beta_pos = content.find("fn beta").unwrap();
        assert!(alpha_pos < between_pos, "between should be after alpha");
        assert!(between_pos < beta_pos, "between should be before beta");
    }

    #[tokio::test]
    async fn test_anchor_rust_method_in_container() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(
            &file_path,
            "impl TaskList {\n    fn activeCount(&self) -> usize { 5 }\n\n    fn totalCount(&self) -> usize { 10 }\n}\n",
        )
        .unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.rs"),
                ("anchor_type", "method"),
                ("anchor_name", "activeCount"),
                ("container", "TaskList"),
                (
                    "insert",
                    "\n    fn getActiveTasks(&self) -> Vec<Task> { vec![] }\n",
                ),
                ("position", "after"),
            ]))
            .await
            .unwrap();

        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("getActiveTasks"));
        let active_pos = content.find("activeCount").unwrap();
        let get_pos = content.find("getActiveTasks").unwrap();
        let total_pos = content.find("totalCount").unwrap();
        assert!(
            active_pos < get_pos,
            "getActiveTasks should be after activeCount"
        );
        assert!(
            get_pos < total_pos,
            "getActiveTasks should be before totalCount"
        );
    }

    #[tokio::test]
    async fn test_anchor_rust_struct() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(
            &file_path,
            "struct MyStruct {\n    field1: i32,\n}\n\nfn main() {}\n",
        )
        .unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.rs"),
                ("anchor_type", "struct"),
                ("anchor_name", "MyStruct"),
                ("insert", "    pub new_field: String,\n"),
                ("position", "after"),
            ]))
            .await
            .unwrap();

        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();
        // Content should be inserted INSIDE the struct body, before closing brace
        assert!(
            content.contains("struct MyStruct {"),
            "struct should still be present"
        );
        assert!(
            content.contains("new_field"),
            "new_field should be inserted"
        );
        // Verify new_field is INSIDE the struct body (before the closing brace)
        let struct_close = content.rfind("}\n\nfn main").unwrap();
        let new_field_pos = content.find("new_field").unwrap();
        assert!(
            new_field_pos < struct_close,
            "new_field should be inside struct body, before closing brace"
        );
    }

    #[tokio::test]
    async fn test_anchor_rust_impl() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(
            &file_path,
            "impl MyStruct {\n    fn method1(&self) {}\n}\n\nfn main() {}\n",
        )
        .unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.rs"),
                ("anchor_type", "impl"),
                ("anchor_name", "MyStruct"),
                ("insert", "\n    fn method2(&self) {}\n"),
                ("position", "after"),
            ]))
            .await
            .unwrap();

        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();
        // Content should be inserted INSIDE the impl body, before closing }}
        assert!(
            content.contains("method2"),
            "method2 should be inserted"
        );
        // Verify method2 is INSIDE the impl block
        let impl_close = content.rfind("}\n\nfn main").unwrap();
        let method2_pos = content.find("method2").unwrap();
        assert!(
            method2_pos < impl_close,
            "method2 should be inside impl, before closing brace"
        );
    }

    #[tokio::test]
    async fn test_anchor_typescript_method_in_class() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        std::fs::write(
            &file_path,
            "class TaskList {\n    activeCount(): number {\n        return 0;\n    }\n}\n",
        )
        .unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.ts"),
                ("anchor_type", "method"),
                ("anchor_name", "activeCount"),
                ("container", "TaskList"),
                (
                    "insert",
                    "\n    getActiveTasks(): Task[] {\n        return [];\n    }\n",
                ),
                ("position", "after"),
            ]))
            .await
            .unwrap();

        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("getActiveTasks"));
        // Verify getActiveTasks is INSIDE the class body (before class closing brace)
        // The class source is: "class TaskList {\n    activeCount...\n}\n"
        // The closing brace is at the end; find the last }}
        let class_close = content.rfind('}').unwrap();
        let get_pos = content.find("getActiveTasks").unwrap();
        assert!(
            get_pos < class_close,
            "getActiveTasks should be inside class body"
        );
    }

    /// Reproduces the exact bug scenario: inserting a method into a TypeScript
    /// class using `anchor_type = "class"` + `position = "after"`.
    /// Before the fix, this would insert AFTER the closing brace of the class.
    /// After the fix, it inserts INSIDE the class body before the closing brace.
    #[tokio::test]
    async fn test_anchor_typescript_add_method_via_class() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        let source = r##"export class TaskList {
  private tasks: Task[] = [];

  add(title: string): Task {
    const task = createTask(id, title);
    this.tasks.push(task);
    return task;
  }

  count(): number {
    return this.tasks.length;
  }

  activeCount(): number {
    return this.tasks.filter(isTaskActive).length;
  }
}
"##;
        std::fs::write(&file_path, source).unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let insert_content = r##"  getActiveTasks(): Task[] {
    return this.tasks.filter(isTaskActive);
  }
"##;
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.ts"),
                ("anchor_type", "class"),
                ("anchor_name", "TaskList"),
                ("insert", insert_content),
                ("position", "after"),
            ]))
            .await
            .unwrap();

        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();

        // The inserted method should be INSIDE the class body, before closing }}
        let class_decl = content.find("export class TaskList {").unwrap();
        let class_close = content.rfind("}").unwrap();
        let method_pos = content.find("getActiveTasks").unwrap();

        assert!(
            method_pos > class_decl,
            "getActiveTasks should be inside class body (after class declaration)"
        );
        assert!(
            method_pos < class_close,
            "getActiveTasks should be inside class body (before closing brace)"
        );

        // Also verify the method is before the export scope's closing brace
        // (the file ends with the class, so the last }} is the class close)
        assert!(
            content.ends_with("}\n"),
            "file should still end with class closing brace"
        );
        let _ = class_decl;
    }

    /// Test inserting a method after an explicit constructor using
    /// anchor_type = "constructor".
    #[tokio::test]
    async fn test_anchor_typescript_after_constructor() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        let source = r##"class TaskList {
  private tasks: Task[] = [];

  constructor() {
    // init
  }

  add(title: string): Task {
    return createTask(0, title);
  }
}
"##;
        std::fs::write(&file_path, source).unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let insert_content = r##"  getActiveTasks(): Task[] {
    return [];
  }
"##;
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.ts"),
                ("anchor_type", "constructor"),
                ("anchor_name", "constructor"),
                ("container", "TaskList"),
                ("insert", insert_content),
                ("position", "after"),
            ]))
            .await
            .unwrap();

        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();
        // getActiveTasks should be after constructor, before add()
        let ctor_pos = content.find("constructor()").unwrap();
        let get_pos = content.find("getActiveTasks").unwrap();
        let add_pos = content.find("add(title").unwrap();
        assert!(
            ctor_pos < get_pos,
            "getActiveTasks should be after constructor"
        );
        assert!(
            get_pos < add_pos,
            "getActiveTasks should be before add()"
        );
    }

    /// Test that when anchor_type = "constructor" is not found (no explicit
    /// constructor in the class), the tool falls back to inserting at the end
    /// of the container body instead of failing.
    #[tokio::test]
    async fn test_anchor_typescript_constructor_not_found_falls_back() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        let source = r##"class TaskList {
  private tasks: Task[] = [];

  add(title: string): Task {
    return createTask(0, title);
  }

  count(): number {
    return this.tasks.length;
  }
}
"##;
        std::fs::write(&file_path, source).unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let insert_content = r##"  newMethod(): void {}
"##;
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.ts"),
                ("anchor_type", "constructor"),
                ("anchor_name", "constructor"),
                ("container", "TaskList"),
                ("insert", insert_content),
                ("position", "after"),
            ]))
            .await
            .unwrap();

        // Fallback should succeed — inserts at end of class body
        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("newMethod"));
        // newMethod should be at the END of the class body (before closing }}
        let class_close = content.rfind('}').unwrap();
        let new_method_pos = content.find("newMethod").unwrap();
        assert!(
            new_method_pos < class_close,
            "newMethod should be inside class body (before closing brace)"
        );
        // newMethod should be after count() (last existing method)
        let count_pos = content.find("count()").unwrap();
        assert!(
            count_pos < new_method_pos,
            "newMethod should be after count() (at end of body)"
        );
    }

    /// Test that search mode smart-resolves class declarations: when search =
    /// "class TaskList {", the tool should detect this is a class via tree-sitter
    /// and insert inside the body before the closing brace, not after the opening `{{`.
    /// This makes the behavior deterministic regardless of LLM prompt adherence.
    #[tokio::test]
    async fn test_search_smart_resolves_class_body() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ts");
        let source = r##"export class TaskList {
  private tasks: Task[] = [];
  private nextId: number = 0;

  add(title: string): Task {
    return createTask(0, title);
  }

  count(): number {
    return this.tasks.length;
  }

  activeCount(): number {
    return this.tasks.filter(isTaskActive).length;
  }
}
"##;
        std::fs::write(&file_path, source).unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let insert_content = r##"  getActiveTasks(): Task[] {
    return this.tasks.filter(task => isTaskActive(task));
  }
"##;
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.ts"),
                ("search", "class TaskList {"),
                ("insert", insert_content),
            ]))
            .await
            .unwrap();

        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();

        // The method should be at the END of the class body, before the closing brace
        let class_open = content.find("class TaskList {").unwrap();
        let class_close = content.rfind('}').unwrap();
        let get_pos = content.find("getActiveTasks").unwrap();

        assert!(
            get_pos > class_open,
            "getActiveTasks should be inside class body"
        );
        assert!(
            get_pos < class_close,
            "getActiveTasks should be BEFORE the closing brace (at end of body)"
        );

        // The last member (activeCount) should be before getActiveTasks
        let active_count_pos = content.find("activeCount").unwrap();
        assert!(
            active_count_pos < get_pos,
            "getActiveTasks should be AFTER activeCount (at end of body)"
        );
    }

    #[tokio::test]
    async fn test_anchor_end_of_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}\n").unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.rs"),
                ("anchor_type", "end_of_file"),
                ("insert", "\nfn new_fn() {}\n"),
            ]))
            .await
            .unwrap();

        assert!(result.is_success());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.ends_with("fn main() {}\n\nfn new_fn() {}\n"));
    }

    #[tokio::test]
    async fn test_anchor_not_found() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn existing() {}").unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.rs"),
                ("anchor_type", "function"),
                ("anchor_name", "nonexistent"),
                ("insert", "content"),
            ]))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_anchor_unsupported_language() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.json");
        std::fs::write(&file_path, "{}").unwrap();

        let tool = FilePatchTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input(vec![
                ("path", "test.json"),
                ("anchor_type", "function"),
                ("anchor_name", "foo"),
                ("insert", "{}"),
            ]))
            .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("does not support AST-based anchoring")
        );
    }

    // ------------------------------------------------------------------
    // Common tests
    // ------------------------------------------------------------------

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
        let result = tool
            .execute(&make_input(vec![
                ("path", "../outside.rs"),
                ("search", "search"),
                ("insert", "insert"),
            ]))
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::PathDenied(_)));
    }
}

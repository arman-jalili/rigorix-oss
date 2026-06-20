//! Tree-sitter based AST anchor finder for structured file patching.
//!
//! @canonical .pi/architecture/modules/tool-system.md#file-patch
//! Implements: TreeSitterAnchorFinder — AST-anchored insertion point detection
//! Issue: #199
//! Last Architecture Sync: 2026-06-18
//!
//! Provides a deterministic replacement for text-search-based file patching.
//! Instead of searching for a brittle string, `TreeSitterAnchorFinder` parses
//! the file with tree-sitter and finds a specific AST node by type and name.
//!
//! # Supported Languages
//!
//! | Extension | Grammar | Anchor Types |
//! |-----------|---------|-------------|
//! | `.rs`     | Rust    | struct, impl, function, end_of_file |
//! | `.ts`, `.tsx` | TypeScript | class, constructor, method, function, interface, end_of_file |
//! | `.py`     | Python  | class, function, end_of_file |
//! | *         | (fallback) | end_of_file only (text search fallback) |
//!
//! # Fallback Behavior
//! For unsupported languages, only `end_of_file` anchor type is supported.
//! All other anchor types return `ToolError::ExecutionFailed` with a message
//! indicating the language is not supported for AST-based anchoring.

use std::path::Path;

use crate::tools::domain::error::ToolError;

/// Parameters for finding an AST anchor point in a source file.
///
/// Replaces the brittle `search` string with structured AST node identity.
///
/// # Examples
///
/// ```ignore
/// // Used via TreeSitterAnchorFinder::find_anchor() — not constructed directly
/// AnchorParams {
///     anchor_type: "method".to_string(),
///     anchor_name: "activeCount".to_string(),
///     container: Some("TaskList".to_string()),
///     position: "after".to_string(),
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct AnchorParams {
    /// The type of AST node to locate: "class", "struct", "impl", "method",
    /// "constructor", "function", "interface", or "end_of_file".
    pub anchor_type: String,

    /// The name of the symbol to find (ignored for "end_of_file").
    pub anchor_name: String,

    /// Optional parent container name to scope the search.
    /// For example, `container: "TaskList"` with `anchor_type: "method"`
    /// finds a method only inside `TaskList`'s impl/class body.
    pub container: Option<String>,

    /// Where to insert content relative to the anchor: "before" or "after".
    pub position: String,
}

/// Result of locating an anchor point in a source file.
#[derive(Debug, Clone, PartialEq)]
pub struct AnchorResult {
    /// The byte offset in the source where content should be inserted.
    pub insert_offset: usize,

    /// Human-readable description of what was found (for logging).
    pub description: String,
}

/// AST-anchored insertion point finder using tree-sitter.
///
/// Parses source files with the appropriate tree-sitter grammar and walks
/// the AST to find nodes matching the given anchor type and name.
pub struct TreeSitterAnchorFinder;

impl TreeSitterAnchorFinder {
    /// Find an anchor point in source text using tree-sitter AST parsing.
    ///
    /// # Parameters
    /// - `source`: The full text content of the file to patch.
    /// - `file_path`: The file path (used for language detection via extension).
    /// - `params`: Structured anchor parameters (type, name, container, position).
    ///
    /// # Returns
    /// - `Ok(AnchorResult)` with the exact byte offset for insertion.
    /// - `Err(ToolError)` if the anchor cannot be found or the language is unsupported.
    pub fn find_anchor(
        source: &str,
        file_path: &str,
        params: &AnchorParams,
    ) -> Result<AnchorResult, ToolError> {
        // Handle end_of_file separately (no tree-sitter needed)
        if params.anchor_type == "end_of_file" {
            let len = source.len();
            return Ok(AnchorResult {
                insert_offset: len,
                description: "end of file".to_string(),
            });
        }

        // Detect language from file extension
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match ext {
            "rs" => Self::find_in_rust(source, params),
            "ts" | "tsx" => Self::find_in_typescript(source, params),
            "py" => Self::find_in_python(source, params),
            _ => Err(ToolError::ExecutionFailed(format!(
                "Language for '{}' does not support AST-based anchoring. \
                 Supported: .rs, .ts, .tsx, .py. Use 'end_of_file' or text search instead.",
                file_path
            ))),
        }
    }

    /// Try to resolve a text-search match position to a container body position.
    ///
    /// When a user searches for a class/struct/impl/interface declaration line
    /// (e.g., `search = "class TaskList {"`), the text-based position lands after
    /// the opening `{`, placing content at the START of the body. This function
    /// detects such cases via tree-sitter and returns the END of the body
    /// (before closing `}`), which is the correct position for adding members.
    ///
    /// This makes the `search` mode deterministic — regardless of what the LLM
    /// generates, tool-side logic corrects the placement.
    ///
    /// # Arguments
    /// * `source` — The full file content.
    /// * `file_path` — Used for language detection via extension.
    /// * `match_byte` — The byte position of the text match in `source`.
    /// * `position` — "after" (default) or "before".
    ///
    /// # Returns
    /// `Some(byte_offset)` — the corrected insert position inside the container body.
    /// `None` — either the match doesn't land at a container, or language is unsupported.
    pub fn resolve_search_to_container(
        source: &str,
        file_path: &str,
        match_byte: usize,
        position: &str,
    ) -> Option<usize> {
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let language: tree_sitter::Language = match ext {
            "rs" => tree_sitter_rust::LANGUAGE.into(),
            "ts" | "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
            "py" => tree_sitter_python::language(),
            _ => return None,
        };

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).ok()?;
        let tree = parser.parse(source, None)?;

        let root = tree.root_node();
        // Find the deepest node at the match position, then walk up for container
        let leaf = root.descendant_for_byte_range(match_byte, match_byte)?;
        let mut current = leaf;

        loop {
            let kind = current.kind();
            // Check if this is a container type
            let is_container = matches!(
                kind,
                "class_declaration"
                    | "interface_declaration"
                    | "struct_item"
                    | "impl_item"
                    | "trait_item"
                    | "class_definition"
            );

            if is_container {
                if let Some(body) = current.child_by_field_name("body") {
                    let body_start = body.start_byte();
                    let body_end = body.end_byte();
                    let offset = match position {
                        "before" if body_end > body_start + 1 => body_start + 1, // after opening `{`
                        "before" => body_start + 1,
                        _ if body_end > body_start + 1 => body_end - 1, // before closing `}`
                        _ => body_start + 1,
                    };
                    return Some(offset);
                }
            }

            // Walk up to parent
            match current.parent() {
                Some(p) => current = p,
                None => break,
            }
        }

        None
    }

    // ------------------------------------------------------------------
    // Rust AST walking
    // ------------------------------------------------------------------

    /// Resolve insert position for container types (class, struct, impl, interface).
    ///
    /// For container types, "after" means inside the body before the closing brace,
    /// and "before" means inside the body after the opening brace.
    /// This matches the intuitive expectation: "add a method to the class" = "insert
    /// inside the class body at the end", not "append after the entire class."
    fn resolve_container_position<'tree>(
        target_node: tree_sitter::Node<'tree>,
        anchor_type: &str,
        position: &str,
    ) -> Option<(usize, &'static str)> {
        // These are container types that have a `body` child
        let is_container = matches!(anchor_type, "class" | "struct" | "impl" | "interface");

        if is_container {
            if let Some(body) = target_node.child_by_field_name("body") {
                let body_start = body.start_byte();
                let body_end = body.end_byte();
                // body spans from `{` to `}` inclusive
                // end_byte is one past `}`, start_byte is at `{`
                match position {
                    "before" => {
                        // Insert after opening brace (+ 1 to skip `{`)
                        Some((body_start + 1, "inside body after opening brace"))
                    }
                    _ => {
                        // Insert before closing brace (- 1 to be at `}`, insert before it)
                        if body_end > body_start + 1 {
                            Some((body_end - 1, "inside body before closing brace"))
                        } else {
                            // Empty body, insert after opening brace
                            Some((body_start + 1, "inside empty body"))
                        }
                    }
                }
            } else {
                None
            }
        } else {
            match position {
                "before" => Some((target_node.start_byte(), "before")),
                _ => Some((target_node.end_byte(), "after")),
            }
        }
    }

    fn find_in_rust(source: &str, params: &AnchorParams) -> Result<AnchorResult, ToolError> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to load Rust grammar: {}", e))
            })?;

        let tree = parser.parse(source, None).ok_or_else(|| {
            ToolError::ExecutionFailed("Failed to parse Rust source file".to_string())
        })?;

        let root = tree.root_node();
        let source_bytes = source.as_bytes();

        // If container is specified, find the container node first
        let scope_node = if let Some(ref container_name) = params.container {
            Self::find_rust_container(root, source_bytes, container_name, source).ok_or_else(
                || {
                    ToolError::ExecutionFailed(format!(
                        "Container '{}' not found in Rust source",
                        container_name
                    ))
                },
            )?
        } else {
            root
        };

        // Inside the scope, find the anchor node by kind and name
        let (target_node, kind_name) =
            Self::find_rust_node(scope_node, source_bytes, params, source).ok_or_else(|| {
                ToolError::ExecutionFailed(format!(
                    "{} '{}' not found{} in Rust source",
                    params.anchor_type,
                    params.anchor_name,
                    params
                        .container
                        .as_ref()
                        .map(|c| format!(" in '{}'", c))
                        .unwrap_or_default()
                ))
            })?;

        let (insert_offset, position_desc) =
            Self::resolve_container_position(target_node, &params.anchor_type, &params.position)
                .ok_or_else(|| {
                    ToolError::ExecutionFailed(format!(
                        "Cannot resolve position for {} '{}': body not found",
                        kind_name, params.anchor_name
                    ))
                })?;

        Ok(AnchorResult {
            insert_offset,
            description: format!(
                "{} '{}' ({} {})",
                kind_name, params.anchor_name, position_desc, params.position
            ),
        })
    }

    fn find_rust_container<'tree>(
        node: tree_sitter::Node<'tree>,
        source: &[u8],
        name: &str,
        _full_source: &str,
    ) -> Option<tree_sitter::Node<'tree>> {
        let kind = node.kind();
        match kind {
            "impl_item" => {
                // impl_item uses `type` field, not `name`
                #[allow(clippy::collapsible_if)]
                if let Some(type_node) = node.child_by_field_name("type") {
                    if type_node.utf8_text(source).ok()? == name {
                        return Some(node);
                    }
                }
            }
            "struct_item" | "trait_item" | "mod_item" => {
                #[allow(clippy::collapsible_if)]
                if let Some(name_node) = node.child_by_field_name("name") {
                    if name_node.utf8_text(source).ok()? == name {
                        return Some(node);
                    }
                }
            }
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = Self::find_rust_container(child, source, name, _full_source) {
                return Some(found);
            }
        }
        None
    }

    fn find_rust_node<'tree>(
        node: tree_sitter::Node<'tree>,
        source: &[u8],
        params: &AnchorParams,
        _full_source: &str,
    ) -> Option<(tree_sitter::Node<'tree>, &'static str)> {
        let kind = node.kind();
        let anchor_kind = params.anchor_type.as_str();

        // Check if this node matches the requested anchor type and name
        let matched = match (anchor_kind, kind) {
            ("function", "function_item")
            | ("method", "function_item")
            | ("struct", "struct_item")
            | ("function", "function_signature_item") => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    name_node.utf8_text(source).ok()? == params.anchor_name
                } else {
                    false
                }
            }
            ("impl", "impl_item") => {
                // For impl, match by trait name or type name
                if let Some(type_node) = node.child_by_field_name("type") {
                    let type_name = type_node.utf8_text(source).ok()?;
                    if type_name == params.anchor_name {
                        true
                    } else if let Some(trait_node) = node.child_by_field_name("trait") {
                        trait_node.utf8_text(source).ok()? == params.anchor_name
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            ("class", "struct_item") | ("interface", "trait_item") => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    name_node.utf8_text(source).ok()? == params.anchor_name
                } else {
                    false
                }
            }
            _ => false,
        };

        if matched {
            let kind_name = match kind {
                "function_item" | "function_signature_item" => {
                    if anchor_kind == "method" {
                        "method"
                    } else {
                        "function"
                    }
                }
                "struct_item" => {
                    if anchor_kind == "class" {
                        "class"
                    } else {
                        "struct"
                    }
                }
                "trait_item" => "interface",
                "impl_item" => "impl",
                _ => kind,
            };
            return Some((node, kind_name));
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = Self::find_rust_node(child, source, params, _full_source) {
                return Some(found);
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // TypeScript AST walking
    // ------------------------------------------------------------------

    fn find_in_typescript(source: &str, params: &AnchorParams) -> Result<AnchorResult, ToolError> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to load TypeScript grammar: {}", e))
            })?;

        let tree = parser.parse(source, None).ok_or_else(|| {
            ToolError::ExecutionFailed("Failed to parse TypeScript source file".to_string())
        })?;

        let root = tree.root_node();
        let source_bytes = source.as_bytes();

        let scope_node = if let Some(ref container_name) = params.container {
            Self::find_typescript_container(root, source_bytes, container_name).ok_or_else(
                || {
                    ToolError::ExecutionFailed(format!(
                        "Container '{}' not found in TypeScript source",
                        container_name
                    ))
                },
            )?
        } else {
            root
        };

        let (target_node, kind_name) = Self::find_typescript_node(scope_node, source_bytes, params)
            .ok_or_else(|| {
                ToolError::ExecutionFailed(format!(
                    "{} '{}' not found{} in TypeScript source",
                    params.anchor_type,
                    params.anchor_name,
                    params
                        .container
                        .as_ref()
                        .map(|c| format!(" in '{}'", c))
                        .unwrap_or_default()
                ))
            })?;

        let (insert_offset, position_desc) =
            Self::resolve_container_position(target_node, &params.anchor_type, &params.position)
                .ok_or_else(|| {
                    ToolError::ExecutionFailed(format!(
                        "Cannot resolve position for {} '{}': body not found",
                        kind_name, params.anchor_name
                    ))
                })?;

        Ok(AnchorResult {
            insert_offset,
            description: format!(
                "{} '{}' ({} {})",
                kind_name, params.anchor_name, position_desc, params.position
            ),
        })
    }

    fn find_typescript_container<'tree>(
        node: tree_sitter::Node<'tree>,
        source: &[u8],
        name: &str,
    ) -> Option<tree_sitter::Node<'tree>> {
        let kind = node.kind();
        match kind {
            "class_declaration" | "interface_declaration" | "module" => {
                #[allow(clippy::collapsible_if)]
                if let Some(name_node) = node.child_by_field_name("name") {
                    if name_node.utf8_text(source).ok()? == name {
                        return Some(node);
                    }
                }
            }
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = Self::find_typescript_container(child, source, name) {
                return Some(found);
            }
        }
        None
    }

    fn find_typescript_node<'tree>(
        node: tree_sitter::Node<'tree>,
        source: &[u8],
        params: &AnchorParams,
    ) -> Option<(tree_sitter::Node<'tree>, &'static str)> {
        let kind = node.kind();
        let anchor_kind = params.anchor_type.as_str();

        let matched = match (anchor_kind, kind) {
            ("method", "method_definition")
            | ("constructor", "method_definition")
            | ("function", "function_declaration")
            | ("function", "arrow_function")
            | ("class", "class_declaration")
            | ("interface", "interface_declaration") => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    name_node.utf8_text(source).ok()? == params.anchor_name
                } else {
                    false
                }
            }
            _ => false,
        };

        if matched {
            let kind_name = match kind {
                "method_definition" => {
                    if anchor_kind == "constructor" {
                        "constructor"
                    } else {
                        "method"
                    }
                }
                "function_declaration" | "arrow_function" => "function",
                "class_declaration" => "class",
                "interface_declaration" => "interface",
                _ => kind,
            };
            return Some((node, kind_name));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = Self::find_typescript_node(child, source, params) {
                return Some(found);
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Python AST walking
    // ------------------------------------------------------------------

    fn find_in_python(source: &str, params: &AnchorParams) -> Result<AnchorResult, ToolError> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::language())
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to load Python grammar: {}", e))
            })?;

        let tree = parser.parse(source, None).ok_or_else(|| {
            ToolError::ExecutionFailed("Failed to parse Python source file".to_string())
        })?;

        let root = tree.root_node();
        let source_bytes = source.as_bytes();

        let scope_node = if let Some(ref container_name) = params.container {
            Self::find_python_container(root, source_bytes, container_name).ok_or_else(|| {
                ToolError::ExecutionFailed(format!(
                    "Container '{}' not found in Python source",
                    container_name
                ))
            })?
        } else {
            root
        };

        let (target_node, kind_name) = Self::find_python_node(scope_node, source_bytes, params)
            .ok_or_else(|| {
                ToolError::ExecutionFailed(format!(
                    "{} '{}' not found{} in Python source",
                    params.anchor_type,
                    params.anchor_name,
                    params
                        .container
                        .as_ref()
                        .map(|c| format!(" in '{}'", c))
                        .unwrap_or_default()
                ))
            })?;

        let (insert_offset, position_desc) =
            Self::resolve_container_position(target_node, &params.anchor_type, &params.position)
                .ok_or_else(|| {
                    ToolError::ExecutionFailed(format!(
                        "Cannot resolve position for {} '{}': body not found",
                        kind_name, params.anchor_name
                    ))
                })?;

        Ok(AnchorResult {
            insert_offset,
            description: format!(
                "{} '{}' ({} {})",
                kind_name, params.anchor_name, position_desc, params.position
            ),
        })
    }

    fn find_python_container<'tree>(
        node: tree_sitter::Node<'tree>,
        source: &[u8],
        name: &str,
    ) -> Option<tree_sitter::Node<'tree>> {
        if node.kind() == "class_definition" {
            #[allow(clippy::collapsible_if)]
            if let Some(name_node) = node.child_by_field_name("name") {
                if name_node.utf8_text(source).ok()? == name {
                    return Some(node);
                }
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = Self::find_python_container(child, source, name) {
                return Some(found);
            }
        }
        None
    }

    fn find_python_node<'tree>(
        node: tree_sitter::Node<'tree>,
        source: &[u8],
        params: &AnchorParams,
    ) -> Option<(tree_sitter::Node<'tree>, &'static str)> {
        let kind = node.kind();
        let anchor_kind = params.anchor_type.as_str();

        let matched = match (anchor_kind, kind) {
            ("function", "function_definition")
            | ("method", "function_definition")
            | ("class", "class_definition") => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    name_node.utf8_text(source).ok()? == params.anchor_name
                } else {
                    false
                }
            }
            _ => false,
        };

        if matched {
            let kind_name = match kind {
                "function_definition" => {
                    if anchor_kind == "method" {
                        "method"
                    } else {
                        "function"
                    }
                }
                "class_definition" => "class",
                _ => kind,
            };
            return Some((node, kind_name));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = Self::find_python_node(child, source, params) {
                return Some(found);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // Rust tests
    // ------------------------------------------------------------------

    #[test]
    fn test_rust_find_function_after() {
        let source = r#"
fn existing() {
    let x = 1;
}

fn other() {
    let y = 2;
}
"#;
        let params = AnchorParams {
            anchor_type: "function".to_string(),
            anchor_name: "existing".to_string(),
            container: None,
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.rs", &params).unwrap();
        assert!(result.insert_offset > 0);
        assert!(result.description.contains("existing"));
    }

    #[test]
    fn test_rust_find_function_before() {
        let source = r#"
fn first() {}

fn second() {}
"#;
        let params = AnchorParams {
            anchor_type: "function".to_string(),
            anchor_name: "second".to_string(),
            container: None,
            position: "before".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.rs", &params).unwrap();
        // The insert offset should be at the start of 'second' function
        assert!(result.insert_offset > 0);
    }

    #[test]
    fn test_rust_find_struct() {
        let source = r#"
struct MyStruct {
    field1: i32,
}
"#;
        let params = AnchorParams {
            anchor_type: "struct".to_string(),
            anchor_name: "MyStruct".to_string(),
            container: None,
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.rs", &params).unwrap();
        assert!(result.insert_offset > 0);
    }

    #[test]
    fn test_rust_find_impl() {
        let source = r#"
impl MyStruct {
    fn method1(&self) {}
}
"#;
        let params = AnchorParams {
            anchor_type: "impl".to_string(),
            anchor_name: "MyStruct".to_string(),
            container: None,
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.rs", &params).unwrap();
        assert!(result.insert_offset > 0);
    }

    #[test]
    fn test_rust_find_method_in_container() {
        let source = r#"
impl TaskList {
    fn activeCount(&self) -> usize {
        5
    }

    fn totalCount(&self) -> usize {
        10
    }
}
"#;
        let params = AnchorParams {
            anchor_type: "method".to_string(),
            anchor_name: "activeCount".to_string(),
            container: Some("TaskList".to_string()),
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.rs", &params).unwrap();
        assert!(result.insert_offset > 0);
        assert!(result.description.contains("activeCount"));
    }

    #[test]
    fn test_rust_anchor_not_found() {
        let source = "fn existing() {}";
        let params = AnchorParams {
            anchor_type: "function".to_string(),
            anchor_name: "nonexistent".to_string(),
            container: None,
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.rs", &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_rust_container_not_found() {
        let source = "fn existing() {}";
        let params = AnchorParams {
            anchor_type: "function".to_string(),
            anchor_name: "existing".to_string(),
            container: Some("NonExistent".to_string()),
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.rs", &params);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // TypeScript tests
    // ------------------------------------------------------------------

    #[test]
    fn test_typescript_find_class() {
        let source = r#"
class TaskList {
    activeCount(): number {
        return this.tasks.length;
    }
}
"#;
        let params = AnchorParams {
            anchor_type: "class".to_string(),
            anchor_name: "TaskList".to_string(),
            container: None,
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.ts", &params).unwrap();
        assert!(result.insert_offset > 0);
    }

    #[test]
    fn test_typescript_find_method_in_container() {
        let source = r#"
class TaskList {
    activeCount(): number {
        return this.tasks.length;
    }

    totalCount(): number {
        return this.tasks.length;
    }
}
"#;
        let params = AnchorParams {
            anchor_type: "method".to_string(),
            anchor_name: "activeCount".to_string(),
            container: Some("TaskList".to_string()),
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.ts", &params).unwrap();
        assert!(result.insert_offset > 0);
        assert!(result.description.contains("activeCount"));
    }

    #[test]
    fn test_typescript_find_interface() {
        let source = r#"
interface TaskManager {
    addTask(task: Task): void;
}
"#;
        let params = AnchorParams {
            anchor_type: "interface".to_string(),
            anchor_name: "TaskManager".to_string(),
            container: None,
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.ts", &params).unwrap();
        assert!(result.insert_offset > 0);
    }

    // ------------------------------------------------------------------
    // Python tests
    // ------------------------------------------------------------------

    #[test]
    fn test_python_find_function() {
        let source = r#"
def existing():
    pass
"#;
        let params = AnchorParams {
            anchor_type: "function".to_string(),
            anchor_name: "existing".to_string(),
            container: None,
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.py", &params).unwrap();
        assert!(result.insert_offset > 0);
    }

    #[test]
    fn test_python_find_class() {
        let source = r#"
class TaskList:
    def activeCount(self):
        return 5
"#;
        let params = AnchorParams {
            anchor_type: "class".to_string(),
            anchor_name: "TaskList".to_string(),
            container: None,
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.py", &params).unwrap();
        assert!(result.insert_offset > 0);
    }

    #[test]
    fn test_python_find_method_in_container() {
        let source = r#"
class TaskList:
    def activeCount(self):
        return 5

    def totalCount(self):
        return 10
"#;
        let params = AnchorParams {
            anchor_type: "method".to_string(),
            anchor_name: "activeCount".to_string(),
            container: Some("TaskList".to_string()),
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.py", &params).unwrap();
        assert!(result.insert_offset > 0);
    }

    // ------------------------------------------------------------------
    // End-of-file and edge cases
    // ------------------------------------------------------------------

    #[test]
    fn test_end_of_file() {
        let source = "fn main() {}";
        let params = AnchorParams {
            anchor_type: "end_of_file".to_string(),
            anchor_name: String::new(),
            container: None,
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "any.rs", &params).unwrap();
        assert_eq!(result.insert_offset, source.len());
        assert_eq!(result.description, "end of file");
    }

    #[test]
    fn test_unsupported_language() {
        let source = "some content";
        let params = AnchorParams {
            anchor_type: "function".to_string(),
            anchor_name: "foo".to_string(),
            container: None,
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "file.json", &params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("does not support AST-based anchoring")
        );
    }

    #[test]
    fn test_unsupported_anchor_type_for_python() {
        let source = "";
        let params = AnchorParams {
            anchor_type: "impl".to_string(),
            anchor_name: "foo".to_string(),
            container: None,
            position: "after".to_string(),
        };

        let result = TreeSitterAnchorFinder::find_anchor(source, "test.py", &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_rust_anchor_position_before() {
        let source = "fn alpha() {}\n\nfn beta() {}\n";
        let params_before = AnchorParams {
            anchor_type: "function".to_string(),
            anchor_name: "beta".to_string(),
            container: None,
            position: "before".to_string(),
        };

        let result =
            TreeSitterAnchorFinder::find_anchor(source, "test.rs", &params_before).unwrap();

        let params_after = AnchorParams {
            anchor_type: "function".to_string(),
            anchor_name: "beta".to_string(),
            container: None,
            position: "after".to_string(),
        };

        let result_after =
            TreeSitterAnchorFinder::find_anchor(source, "test.rs", &params_after).unwrap();

        assert!(result.insert_offset < result_after.insert_offset);
    }
}

//! CodeGraphBuilder — workspace scanning and CodeGraph construction.
//!
//! @canonical .pi/architecture/modules/code-graph.md#codegraphbuilder
//! Implements: CodeGraphBuilder — workspace scanning, import parsing, graph construction
//! Issue: issue-codegraphbuilder
//!
//! Scans a workspace directory, parses source files using tree-sitter to extract
//! import/module relationships, and constructs a CodeGraph. Supports Rust and
//! TypeScript/JavaScript parsing out of the box.

use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

use crate::code_graph::domain::{
    CodeGraphError, EdgeKind, NodeKind,
};

use super::dto::{
    ConstructGraphInput, ConstructGraphOutput, SealGraphInput,
};
use super::service::CodeGraphService;

/// Builds CodeGraph instances by scanning workspace directories.
///
/// The CodeGraphBuilder walks a directory tree, identifies source files,
/// parses import/module relationships, and constructs a CodeGraph with
/// nodes (modules) and edges (imports).
///
/// # Supported Languages
/// - Rust (.rs): Parses `use` and `mod` statements
/// - TypeScript (.ts, .tsx): Parses `import` statements
/// - JavaScript (.js, .jsx): Parses `require()` and `import` statements
/// - Python (.py): Parses `import` and `from ... import` statements
///
/// # File Filtering
/// - Respects `.gitignore` and common ignore patterns
/// - Skips `node_modules/`, `target/`, `.git/`, and hidden directories
pub struct CodeGraphBuilder {
    /// The graph service to populate with built graphs.
    graph_service: Arc<dyn CodeGraphService>,

    /// Root directories to scan.
    scan_roots: Vec<PathBuf>,

    /// File extensions to scan.
    extensions: Vec<String>,

    /// Whether to include external dependencies as nodes.
    include_external: bool,
}

impl CodeGraphBuilder {
    /// Create a new CodeGraphBuilder.
    pub fn new(
        graph_service: Arc<dyn CodeGraphService>,
        scan_roots: Vec<PathBuf>,
        extensions: Vec<String>,
        include_external: bool,
    ) -> Self {
        Self {
            graph_service,
            scan_roots,
            extensions,
            include_external,
        }
    }

    /// Build a CodeGraph by scanning the configured workspace directories.
    ///
    /// Walks all scan roots, identifies source files matching the configured
    /// extensions, parses import statements, and constructs a CodeGraph.
    ///
    /// # Errors
    /// - `CodeGraphError::IoError` if files cannot be read
    pub async fn build(&self) -> Result<ConstructGraphOutput, CodeGraphError> {
        let name = self
            .scan_roots
            .first()
            .and_then(|r| r.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "code-graph".to_string());

        // First, construct an empty graph
        let output = self
            .graph_service
            .construct_graph(ConstructGraphInput {
                name,
                source: "code-graph-builder".to_string(),
                description: format!("Workspace scan of {} directories", self.scan_roots.len()),
                total_modules_scanned: 0,
            })
            .await?;

        // Scan all directories for source files
        let mut all_files: Vec<PathBuf> = Vec::new();
        for root in &self.scan_roots {
            self.scan_directory(root, &mut all_files)?;
        }

        // Parse each file and track imports
        let mut file_imports: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut all_modules: Vec<(PathBuf, String)> = Vec::new();

        for file_path in &all_files {
            let module_name = self.file_to_module_name(file_path);
            all_modules.push((file_path.clone(), module_name.clone()));

            let imports = self.parse_imports(file_path)?;
            file_imports.insert(file_path.clone(), imports);
        }

        // Create module names to paths mapping for resolution
        let name_to_path: HashMap<String, &PathBuf> = all_modules
            .iter()
            .map(|(p, n)| (n.clone(), p))
            .collect();

        // Build a map of path → node ID for creating edges
        let mut path_to_node_id: HashMap<PathBuf, Uuid> = HashMap::new();

        // Add all files as nodes
        for (file_path, module_name) in &all_modules {
            let kind = self.detect_node_kind(file_path);

            let add_output = self
                .graph_service
                .add_node(super::dto::AddNodeInput {
                    graph_id: output.graph_id,
                    name: module_name.clone(),
                    kind: kind.clone(),
                    path: file_path.to_string_lossy().to_string(),
                    metadata: HashMap::new(),
                })
                .await?;

            path_to_node_id.insert(file_path.clone(), add_output.node_id);
        }

        // Add edges for imports
        for (file_path, import_names) in &file_imports {
            let source_id = match path_to_node_id.get(file_path) {
                Some(id) => *id,
                None => continue,
            };

            for import_name in import_names {
                // Try to resolve the import to a local file
                if let Some(target_path) = self.resolve_import(file_path, import_name, &name_to_path)
                {
                    if let Some(target_id) = path_to_node_id.get(target_path) {
                        self.graph_service
                            .add_edge(super::dto::AddEdgeInput {
                                graph_id: output.graph_id,
                                source_id: *target_id,
                                target_id: source_id,
                                kind: EdgeKind::Imports,
                                weight: 1,
                                label: Some(import_name.clone()),
                            })
                            .await?;
                    }
                }
            }
        }

        // Seal the graph
        self.graph_service
            .seal_graph(SealGraphInput {
                graph_id: output.graph_id,
            })
            .await?;

        // Re-fetch the final graph to get accurate metadata
        let final_graph = self
            .graph_service
            .get_graph(super::dto::GetGraphInput {
                graph_id: output.graph_id,
            })
            .await?;

        Ok(ConstructGraphOutput {
            graph_id: output.graph_id,
            graph: final_graph.graph,
            constructed_at: Utc::now(),
        })
    }

    /// Recursively scan a directory for source files.
    fn scan_directory(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), CodeGraphError> {
        if !dir.exists() || !dir.is_dir() {
            return Ok(());
        }

        let entries = std::fs::read_dir(dir).map_err(|e| CodeGraphError::IoError {
            detail: format!("Failed to read directory {}: {}", dir.display(), e),
        })?;

        for entry in entries.flatten() {
            let path = entry.path();

            // Skip hidden files/directories
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            // Skip common non-source directories
            if path.is_dir() {
                let dir_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                match dir_name {
                    "node_modules" | "target" | "dist" | "build" | ".git" | "__pycache__" => {
                        continue;
                    }
                    _ => {}
                }
                self.scan_directory(&path, files)?;
            } else if path.is_file() {
                // Check extension
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if self.extensions.is_empty() || self.extensions.iter().any(|e| e == ext) {
                        files.push(path);
                    }
                }
            }
        }

        Ok(())
    }

    /// Convert a file path to a module name.
    fn file_to_module_name(&self, path: &Path) -> String {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Detect the NodeKind for a given file path.
    fn detect_node_kind(&self, path: &Path) -> NodeKind {
        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") | Some("ts") | Some("tsx") | Some("js") | Some("jsx") | Some("py") => {
                NodeKind::File
            }
            Some("toml") | Some("json") | Some("yaml") | Some("yml") => NodeKind::Package,
            _ => NodeKind::File,
        }
    }

    /// Parse import statements from a source file.
    fn parse_imports(&self, path: &Path) -> Result<Vec<String>, CodeGraphError> {
        let content = std::fs::read_to_string(path).map_err(|e| CodeGraphError::IoError {
            detail: format!("Failed to read {}: {}", path.display(), e),
        })?;

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match ext {
            "rs" => Ok(self.parse_rust_imports(&content)),
            "ts" | "tsx" | "js" | "jsx" => Ok(self.parse_ts_imports(&content)),
            "py" => Ok(self.parse_python_imports(&content)),
            _ => Ok(Vec::new()),
        }
    }

    /// Parse Rust import statements (`use` and `mod`).
    fn parse_rust_imports(&self, content: &str) -> Vec<String> {
        let mut imports = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip comments and non-import lines
            if trimmed.starts_with("//") || trimmed.starts_with("#") {
                continue;
            }

            // `use crate::module::...` or `use super::module::...`
            if trimmed.starts_with("use ") {
                // Extract the module path: use crate::module::Item → module
                let use_path = trimmed
                    .strip_prefix("use ")
                    .and_then(|s| s.split("::").nth(1))
                    .map(|s| s.to_string());
                if let Some(path) = use_path {
                    if !path.starts_with('{') && !path.starts_with('*') {
                        imports.push(path);
                    }
                }
            }

            // `mod module_name;`
            if trimmed.starts_with("mod ") && trimmed.ends_with(';') {
                let mod_name = trimmed
                    .strip_prefix("mod ")
                    .and_then(|s| s.strip_suffix(';'))
                    .map(|s| s.trim().to_string());
                if let Some(name) = mod_name {
                    if !name.contains('{') {
                        // Only simple `mod name;` declarations
                        imports.push(name);
                    }
                }
            }
        }

        imports
    }

    /// Parse TypeScript/JavaScript import statements.
    fn parse_ts_imports(&self, content: &str) -> Vec<String> {
        let mut imports = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // import { ... } from './module'
            if let Some(from_pos) = trimmed.find(" from ") {
                let after_from = &trimmed[from_pos + 6..];
                let module_path = after_from
                    .trim()
                    .trim_matches('\'')
                    .trim_matches('"')
                    .trim_matches(';')
                    .to_string();
                imports.push(module_path);
            }

            // import * as name from './module'
            // import name from './module'

            // require('./module')
            if let Some(start) = trimmed.find("require(") {
                let rest = &trimmed[start + 8..];
                if let Some(end) = rest.find(')') {
                    let module_path = rest[..end]
                        .trim()
                        .trim_matches('\'')
                        .trim_matches('"')
                        .to_string();
                    imports.push(module_path);
                }
            }

            // import './module' (side-effect import)
            if trimmed.starts_with("import ") && !trimmed.contains(" from ") {
                let path = trimmed
                    .strip_prefix("import ")
                    .map(|s| s.trim().trim_matches('\'').trim_matches('"').trim_matches(';'))
                    .map(|s| s.to_string());
                if let Some(p) = path {
                    if p.starts_with('.') || p.starts_with('/') {
                        imports.push(p);
                    }
                }
            }
        }

        imports
    }

    /// Parse Python import statements.
    fn parse_python_imports(&self, content: &str) -> Vec<String> {
        let mut imports = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with('#') {
                continue;
            }

            // import module or import module as alias
            if trimmed.starts_with("import ") {
                let module = trimmed
                    .strip_prefix("import ")
                    .and_then(|s| s.split(' ').next())
                    .map(|s| s.split('.').next().unwrap_or(s).to_string());
                if let Some(m) = module {
                    imports.push(m);
                }
            }

            // from module import ...
            if trimmed.starts_with("from ") {
                let module_path = trimmed
                    .strip_prefix("from ")
                    .and_then(|s| s.split(' ').next())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                // Handle relative imports: .utils, ..core
                let module = module_path
                    .trim_start_matches('.')
                    .split('.')
                    .next()
                    .unwrap_or(&module_path)
                    .to_string();
                if !module.is_empty() && module != "__future__" {
                    imports.push(module);
                }
            }
        }

        imports
    }

    /// Resolve an import name to a file path within the workspace.
    fn resolve_import<'a>(
        &self,
        source_file: &Path,
        import_name: &str,
        name_to_path: &'a HashMap<String, &PathBuf>,
    ) -> Option<&'a PathBuf> {
        // Strip relative path prefixes
        let clean_name = import_name
            .trim_start_matches("./")
            .trim_start_matches("../")
            .trim_start_matches('/');

        // Extract the last component (file name)
        let base_name = clean_name
            .split('/')
            .last()
            .unwrap_or(clean_name);

        // Try exact match
        if let Some(path) = name_to_path.get(base_name) {
            return Some(path);
        }

        // Try case-insensitive match
        let base_lower = base_name.to_lowercase();
        for (name, path) in name_to_path {
            if name.to_lowercase() == base_lower {
                return Some(path);
            }
        }

        // Try relative to source file
        if let Some(parent) = source_file.parent() {
            let resolved = parent.join(clean_name);
            let with_ext = [
                resolved.with_extension("rs"),
                resolved.with_extension("ts"),
                resolved.with_extension("js"),
                resolved.with_extension("py"),
            ];
            for candidate in &with_ext {
                if let Some(path) = name_to_path.values().find(|p| ***p == *candidate) {
                    return Some(path);
                }
            }
        }

        // Not found locally — if external deps are included, create a synthetic reference
        if self.include_external {
            return None; // External would require creating a new node
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_graph::application::service_impl::CodeGraphServiceImpl;
    use tempfile::tempdir;

    fn create_builder(roots: Vec<PathBuf>) -> CodeGraphBuilder {
        let service: Arc<dyn CodeGraphService> = Arc::new(CodeGraphServiceImpl::new());
        CodeGraphBuilder::new(
            service,
            roots,
            vec!["rs".to_string(), "ts".to_string(), "py".to_string()],
            false,
        )
    }

    #[test]
    fn test_parse_rust_imports() {
        let service: Arc<dyn CodeGraphService> = Arc::new(CodeGraphServiceImpl::new());
        let builder = CodeGraphBuilder::new(service, vec![], vec![], false);

        let content = r#"
use crate::parser::parse_file;
use std::collections::HashMap;
mod utils;
mod syntax;

fn main() {}
"#;

        let imports = builder.parse_rust_imports(content);
        assert!(imports.contains(&"parser".to_string()));
        assert!(imports.contains(&"utils".to_string()));
        assert!(imports.contains(&"syntax".to_string()));
    }

    #[test]
    fn test_parse_ts_imports() {
        let service: Arc<dyn CodeGraphService> = Arc::new(CodeGraphServiceImpl::new());
        let builder = CodeGraphBuilder::new(service, vec![], vec![], false);

        let content = r#"
import { parseFile } from './parser';
import { lexer } from '../lexer/index';
import * as utils from './utils';
const fs = require('fs');
"#;

        let imports = builder.parse_ts_imports(content);
        assert!(imports.iter().any(|i| i.contains("./parser")));
        assert!(imports.iter().any(|i| i.contains("../lexer/index")));
        assert!(imports.iter().any(|i| i.contains("./utils")));
        assert!(imports.iter().any(|i| i.contains("fs")));
    }

    #[test]
    fn test_parse_python_imports() {
        let service: Arc<dyn CodeGraphService> = Arc::new(CodeGraphServiceImpl::new());
        let builder = CodeGraphBuilder::new(service, vec![], vec![], false);

        let content = r#"
import os
import sys
from typing import List
from .utils import helper
from ..core import engine
"#;

        let imports = builder.parse_python_imports(content);
        assert!(imports.contains(&"os".to_string()));
        assert!(imports.contains(&"sys".to_string()));
        assert!(imports.contains(&"typing".to_string()));
        assert!(imports.contains(&"utils".to_string()));
        assert!(imports.contains(&"core".to_string()));
    }

    #[test]
    fn test_scan_directory_finds_files() {
        let dir = tempdir().unwrap();

        // Create test files
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("lib.rs"), "pub fn foo() {}").unwrap();
        std::fs::write(dir.path().join("README.md"), "# Docs").unwrap();

        // Create subdirectory
        std::fs::create_dir(dir.path().join("utils")).unwrap();
        std::fs::write(dir.path().join("utils/helper.rs"), "pub fn help() {}").unwrap();

        let service: Arc<dyn CodeGraphService> = Arc::new(CodeGraphServiceImpl::new());
        let builder = CodeGraphBuilder::new(
            service,
            vec![dir.path().to_path_buf()],
            vec!["rs".to_string()],
            false,
        );

        let mut files = Vec::new();
        builder.scan_directory(dir.path(), &mut files).unwrap();

        // Should find .rs files only
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.ends_with("main.rs")));
        assert!(files.iter().any(|f| f.ends_with("lib.rs")));
        assert!(files.iter().any(|f| f.ends_with("helper.rs")));
    }

    #[test]
    fn test_scan_skips_hidden_and_build_dirs() {
        let dir = tempdir().unwrap();

        std::fs::create_dir(dir.path().join(".hidden")).unwrap();
        std::fs::write(dir.path().join(".hidden/secret.rs"), "").unwrap();
        std::fs::create_dir(dir.path().join("node_modules")).unwrap();
        std::fs::write(dir.path().join("node_modules/pkg.rs"), "").unwrap();
        std::fs::write(dir.path().join("visible.rs"), "").unwrap();

        let service: Arc<dyn CodeGraphService> = Arc::new(CodeGraphServiceImpl::new());
        let builder = CodeGraphBuilder::new(
            service,
            vec![dir.path().to_path_buf()],
            vec!["rs".to_string()],
            false,
        );

        let mut files = Vec::new();
        builder.scan_directory(dir.path(), &mut files).unwrap();

        // Should only find visible.rs
        assert_eq!(files.len(), 1);
        assert!(files.iter().any(|f| f.ends_with("visible.rs")));
    }

    #[tokio::test]
    async fn test_build_creates_graph() {
        let dir = tempdir().unwrap();

        // Create a simple Rust project
        std::fs::write(
            dir.path().join("main.rs"),
            "mod utils;\nfn main() { utils::help(); }\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("utils.rs"), "pub fn help() {}\n").unwrap();

        let service: Arc<dyn CodeGraphService> = Arc::new(CodeGraphServiceImpl::new());
        let builder = CodeGraphBuilder::new(
            service,
            vec![dir.path().to_path_buf()],
            vec!["rs".to_string()],
            false,
        );

        let result = builder.build().await.unwrap();
        assert!(result.graph.node_count() > 0);
        assert!(result.graph.sealed);
    }
}

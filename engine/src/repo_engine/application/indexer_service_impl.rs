//! `IndexerService` implementation with tree-sitter symbol extraction.
//!
//! @canonical .pi/architecture/modules/repo-engine.md#indexer
//! Implements: GAP-A-15 — IndexerService real implementation
//!
//! Parses source files with the bundled tree-sitter grammars (via
//! `GrammarRepository`) and extracts `SymbolDefinition`s for function,
//! struct/enum/trait/class/interface/type/module declarations.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::repo_engine::application::dto::{
    AddSymbolInput, DetectProjectInput, DetectProjectOutput, IndexDirectoryInput,
    IndexDirectoryOutput, IndexFileInput, IndexFileOutput,
};
use crate::repo_engine::application::service::{IndexerService, SymbolGraphService};
use crate::repo_engine::domain::{
    Location, RepoEngineError, SourceLanguage, SymbolDefinition, SymbolKind,
};
use crate::repo_engine::infrastructure::repository::{GrammarRepository, SourceRepository};

/// Default extensions per language.
const RUST_EXT: &str = "rs";
const PY_EXT: &str = "py";
const TS_EXTS: [&str; 2] = ["ts", "tsx"];

/// Default per-file size cap for indexing (5 MiB).
/// Tree-sitter implementation of `IndexerService`.
pub struct IndexerServiceImpl {
    /// Graph that `index_directory` adds symbols to.
    graph: Arc<dyn SymbolGraphService>,
    /// Source access (filesystem-backed in production).
    source_repo: Arc<dyn SourceRepository>,
    /// Grammar loading (bundled tree-sitter grammars).
    grammar_repo: Arc<dyn GrammarRepository>,
    /// Session count of indexed files.
    indexed_count: AtomicUsize,
}

impl IndexerServiceImpl {
    /// Create the indexer with its collaborators.
    pub fn new(
        graph: Arc<dyn SymbolGraphService>,
        source_repo: Arc<dyn SourceRepository>,
        grammar_repo: Arc<dyn GrammarRepository>,
    ) -> Self {
        Self {
            graph,
            source_repo,
            grammar_repo,
            indexed_count: AtomicUsize::new(0),
        }
    }

    /// Detect the language from a file extension.
    fn language_for_extension(extension: &str) -> Option<SourceLanguage> {
        match extension.trim_start_matches('.') {
            RUST_EXT => Some(SourceLanguage::Rust),
            PY_EXT => Some(SourceLanguage::Python),
            "ts" | "tsx" => Some(SourceLanguage::TypeScript),
            _ => None,
        }
    }

    /// Build a `SymbolDefinition` from a tree-sitter declaration node.
    fn build_symbol(
        source: &str,
        source_bytes: &[u8],
        node: tree_sitter::Node<'_>,
        path: &Path,
        language: &SourceLanguage,
    ) -> Option<SymbolDefinition> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source_bytes).ok()?.to_string();
        let kind = Self::kind_for_node(node.kind(), language)?;
        let position = node.start_position();
        let signature = node
            .utf8_text(source_bytes)
            .ok()?
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        let definition_text = node.utf8_text(source_bytes).ok()?.to_string();
        let _ = source;

        Some(SymbolDefinition::new(
            name,
            kind,
            Location {
                file: path.to_path_buf(),
                line: position.row as u32,
                column: position.column as u32,
            },
            signature,
            definition_text,
            language.clone(),
        ))
    }

    /// Map a tree-sitter node kind to a `SymbolKind` for the given language.
    fn kind_for_node(kind: &str, language: &SourceLanguage) -> Option<SymbolKind> {
        match language {
            SourceLanguage::Rust => match kind {
                "function_item" => Some(SymbolKind::Function),
                "struct_item" => Some(SymbolKind::Struct),
                "enum_item" => Some(SymbolKind::Enum),
                "trait_item" => Some(SymbolKind::Trait),
                "impl_item" => Some(SymbolKind::Trait),
                "const_item" | "static_item" => Some(SymbolKind::Constant),
                "type_item" => Some(SymbolKind::Type),
                "mod_item" => Some(SymbolKind::Module),
                _ => None,
            },
            SourceLanguage::Python => match kind {
                "function_definition" => Some(SymbolKind::Function),
                "class_definition" => Some(SymbolKind::Type),
                _ => None,
            },
            SourceLanguage::TypeScript => match kind {
                "function_declaration" => Some(SymbolKind::Function),
                "class_declaration" => Some(SymbolKind::Struct),
                "interface_declaration" => Some(SymbolKind::Type),
                "type_alias_declaration" => Some(SymbolKind::Type),
                "enum_declaration" => Some(SymbolKind::Enum),
                "module_declaration" => Some(SymbolKind::Module),
                _ => None,
            },
        }
    }

    /// Walk the tree and collect all declaration nodes.
    fn collect_symbols(
        source: &str,
        source_bytes: &[u8],
        tree: &tree_sitter::Tree,
        path: &Path,
        language: SourceLanguage,
    ) -> Vec<SymbolDefinition> {
        let mut symbols = Vec::new();
        let mut cursor = tree.walk();
        loop {
            let node = cursor.node();
            if let Some(symbol) = Self::build_symbol(source, source_bytes, node, path, &language) {
                symbols.push(symbol);
            }
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return symbols;
                }
            }
        }
    }
}

#[async_trait]
impl IndexerService for IndexerServiceImpl {
    async fn index_file(&self, input: IndexFileInput) -> Result<IndexFileOutput, RepoEngineError> {
        let language = match input.language {
            Some(lang) => lang,
            None => {
                let ext = self.source_repo.extension(&input.path).ok_or_else(|| {
                    RepoEngineError::Internal {
                        detail: format!("cannot detect language for '{}'", input.path.display()),
                    }
                })?;
                Self::language_for_extension(&ext).ok_or_else(|| RepoEngineError::Internal {
                    detail: format!("unsupported extension '{}'", ext),
                })?
            }
        };

        // Size cap (0 = no limit): oversized files are skipped (empty symbols).
        if input.max_file_size > 0 {
            let size = self.source_repo.file_size(&input.path).await?;
            if size > input.max_file_size {
                return Ok(IndexFileOutput {
                    path: input.path,
                    language,
                    symbols: Vec::new(),
                    symbols_added: 0,
                    symbols_rejected: 0,
                    duration_ms: 0,
                    success: true,
                });
            }
        }

        let source = self.source_repo.read_source(&input.path).await?;

        // Parse with the grammar.
        let grammar = self.grammar_repo.load_grammar(&language).await?;
        let language_clone = language.clone();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&grammar)
            .map_err(|e| RepoEngineError::Internal {
                detail: format!("failed to set grammar for {language:?}: {e}"),
            })?;
        let tree =
            parser
                .parse(source.as_bytes(), None)
                .ok_or_else(|| RepoEngineError::Internal {
                    detail: format!("parse failed for '{}'", input.path.display()),
                })?;

        let start = std::time::Instant::now();
        let symbols = Self::collect_symbols(
            &source,
            source.as_bytes(),
            &tree,
            &input.path,
            language_clone,
        );
        self.indexed_count.fetch_add(1, Ordering::Relaxed);

        Ok(IndexFileOutput {
            path: input.path,
            language,
            symbols: symbols.clone(),
            symbols_added: symbols.len(),
            symbols_rejected: 0,
            duration_ms: start.elapsed().as_millis() as u64,
            success: true,
        })
    }

    async fn index_directory(
        &self,
        input: IndexDirectoryInput,
    ) -> Result<IndexDirectoryOutput, RepoEngineError> {
        // Resolve extensions: explicit > detect_project_type > defaults.
        let start = std::time::Instant::now();
        let mut project_type = "unknown".to_string();
        let extensions = match input.extensions {
            Some(exts) => exts,
            None => {
                let detected = self
                    .detect_project_type(DetectProjectInput {
                        root_dir: input.root_dir.clone(),
                        scan_subdirs: false,
                    })
                    .await?;
                project_type.clone_from(&detected.project_type);
                detected.extensions
            }
        };

        let mut files = self
            .source_repo
            .list_source_files(&input.root_dir, &extensions, true)
            .await?;
        if input.max_files > 0 {
            files.truncate(input.max_files);
        }

        let mut files_indexed = 0usize;
        let mut files_failed = Vec::new();
        let mut files_skipped = Vec::new();
        let mut symbols_added = 0usize;

        for file in files {
            let indexed = self
                .index_file(IndexFileInput {
                    path: file.clone(),
                    language: None,
                    max_file_size: input.max_file_size,
                })
                .await;

            match indexed {
                Ok(output) => {
                    if output.symbols.is_empty() {
                        files_skipped.push(crate::repo_engine::application::dto::SkippedFile {
                            path: file,
                            reason: "no symbols extracted (unsupported or oversized)".to_string(),
                        });
                        continue;
                    }
                    for symbol in output.symbols {
                        if self
                            .graph
                            .add_symbol(AddSymbolInput {
                                name: symbol.name.clone(),
                                kind: symbol.kind,
                                location: symbol.location.clone(),
                                signature: symbol.signature.clone(),
                                definition_text: symbol.definition_text.clone(),
                                language: symbol.language,
                                documentation: symbol.documentation.clone(),
                                visibility: crate::repo_engine::domain::SymbolVisibility::Public,
                                tags: symbol.tags.clone(),
                            })
                            .await
                            .is_ok()
                        {
                            symbols_added += 1;
                        }
                    }
                    files_indexed += 1;
                }
                Err(e) => files_failed.push(crate::repo_engine::application::dto::IndexFailure {
                    path: file,
                    error: e.to_string(),
                    line: None,
                }),
            }
        }

        let mut symbols_by_language = std::collections::HashMap::new();
        symbols_by_language.insert(project_type.clone(), symbols_added);

        Ok(IndexDirectoryOutput {
            root_dir: input.root_dir,
            total_files: files_indexed + files_failed.len() + files_skipped.len(),
            files_indexed,
            files_failed,
            files_skipped,
            symbols_added,
            symbols_by_language,
            duration_ms: start.elapsed().as_millis() as u64,
            languages: vec![project_type.clone()],
            success: true,
        })
    }

    async fn detect_project_type(
        &self,
        input: DetectProjectInput,
    ) -> Result<DetectProjectOutput, RepoEngineError> {
        let root = &input.root_dir;
        let mut languages: Vec<SourceLanguage> = Vec::new();
        let mut extensions: Vec<String> = Vec::new();
        let mut manifest: Option<PathBuf> = None;

        for (name, lang, exts) in [
            (
                "Cargo.toml",
                SourceLanguage::Rust,
                vec![RUST_EXT.to_string()],
            ),
            (
                "pyproject.toml",
                SourceLanguage::Python,
                vec![PY_EXT.to_string()],
            ),
            ("setup.py", SourceLanguage::Python, vec![PY_EXT.to_string()]),
            (
                "tsconfig.json",
                SourceLanguage::TypeScript,
                TS_EXTS.iter().map(|e| e.to_string()).collect(),
            ),
            (
                "package.json",
                SourceLanguage::TypeScript,
                TS_EXTS.iter().map(|e| e.to_string()).collect(),
            ),
        ] {
            let candidate = root.join(name);
            if self.source_repo.source_exists(&candidate).await {
                if manifest.is_none() {
                    manifest = Some(candidate);
                }
                if !languages.contains(&lang) {
                    languages.push(lang);
                    extensions.extend(exts);
                }
            }
        }

        let project_type = match languages.first() {
            Some(SourceLanguage::Rust) => "rust".to_string(),
            Some(SourceLanguage::Python) => "python".to_string(),
            Some(SourceLanguage::TypeScript) => "typescript".to_string(),
            _ => "unknown".to_string(),
        };

        Ok(DetectProjectOutput {
            project_type,
            languages: languages.clone(),
            extensions,
            manifest_file: manifest,
            detected: !languages.is_empty(),
        })
    }

    fn is_extension_supported(&self, extension: &str) -> bool {
        Self::language_for_extension(extension).is_some()
    }

    fn supported_extensions(&self) -> Vec<String> {
        vec![
            RUST_EXT.to_string(),
            PY_EXT.to_string(),
            TS_EXTS[0].to_string(),
            TS_EXTS[1].to_string(),
        ]
    }

    async fn indexed_file_count(&self) -> usize {
        self.indexed_count.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo_engine::application::service::SymbolGraphService;
    use crate::repo_engine::application::symbol_graph_service_impl::SymbolGraphServiceImpl;
    use crate::repo_engine::infrastructure::fs_source_repository::FileSystemSourceRepository;
    use crate::repo_engine::infrastructure::in_memory_grammar_repository::InMemoryGrammarRepository;
    use tempfile::TempDir;

    fn make_indexer() -> (Arc<SymbolGraphServiceImpl>, IndexerServiceImpl) {
        let graph = Arc::new(SymbolGraphServiceImpl::new());
        let source_repo = Arc::new(FileSystemSourceRepository::new()) as Arc<dyn SourceRepository>;
        let grammar_repo = Arc::new(InMemoryGrammarRepository::new()) as Arc<dyn GrammarRepository>;
        let indexer = IndexerServiceImpl::new(
            graph.clone() as Arc<dyn SymbolGraphService>,
            source_repo,
            grammar_repo,
        );
        (graph, indexer)
    }

    #[tokio::test]
    async fn test_index_file_extracts_rust_symbols() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("lib.rs");
        std::fs::write(
            &file,
            "pub fn greet(name: &str) -> String { format!(\"hi {name}\") }\npub struct Point { x: i32 }\npub trait Shape {}\n",
        )
        .unwrap();

        let (_, indexer) = make_indexer();
        let output = indexer
            .index_file(IndexFileInput {
                path: file,
                language: Some(SourceLanguage::Rust),
                max_file_size: 0,
            })
            .await
            .unwrap();

        let kinds: Vec<SymbolKind> = output.symbols.iter().map(|s| s.kind.clone()).collect();
        assert!(kinds.contains(&SymbolKind::Function), "got {kinds:?}");
        assert!(kinds.contains(&SymbolKind::Struct), "got {kinds:?}");
        assert!(kinds.contains(&SymbolKind::Trait), "got {kinds:?}");
        assert!(output.symbols.iter().any(|s| s.name == "greet"));
    }

    #[tokio::test]
    async fn test_index_file_unsupported_extension_errors() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "hello").unwrap();

        let (_, indexer) = make_indexer();
        let result = indexer
            .index_file(IndexFileInput {
                path: file,
                language: None,
                max_file_size: 0,
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_index_directory_round_trip_graph_lookup() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "pub fn helper() -> i32 { 42 }\npub struct Config { pub name: String }\n",
        )
        .unwrap();

        let (graph, indexer) = make_indexer();
        let output = indexer
            .index_directory(IndexDirectoryInput {
                root_dir: dir.path().to_path_buf(),
                extensions: None,
                exclude_patterns: None,
                max_file_size: 0,
                max_files: 0,
                recursive: true,
                detect_project: true,
            })
            .await
            .unwrap();

        assert_eq!(
            output.files_indexed, 1,
            "files_failed: {:?}",
            output.files_failed
        );
        assert!(
            output.symbols_added >= 2,
            "symbols_added: {}",
            output.symbols_added
        );

        // Symbols retrievable via the graph service (AC3 round-trip).
        let lookup = graph
            .lookup_symbol(crate::repo_engine::application::dto::LookupSymbolInput {
                name: "helper".to_string(),
                include_adjacency: false,
                reference_depth: 0,
            })
            .await
            .unwrap();
        assert!(lookup.symbol.is_some());
        let lookup2 = graph
            .lookup_symbol(crate::repo_engine::application::dto::LookupSymbolInput {
                name: "Config".to_string(),
                include_adjacency: false,
                reference_depth: 0,
            })
            .await
            .unwrap();
        assert!(lookup2.symbol.is_some());
    }

    #[tokio::test]
    async fn test_detect_project_type_rust() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\n").unwrap();
        std::fs::write(dir.path().join("src"), "// x").unwrap();

        let (_, indexer) = make_indexer();
        let output = indexer
            .detect_project_type(DetectProjectInput {
                root_dir: dir.path().to_path_buf(),
                scan_subdirs: false,
            })
            .await
            .unwrap();
        assert_eq!(output.project_type, "rust");
        assert!(output.languages.contains(&SourceLanguage::Rust));
        assert!(output.extensions.contains(&"rs".to_string()));
    }
}

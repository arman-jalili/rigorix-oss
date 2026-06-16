//! Implementation of `SymbolGraphService`.
//!
//! @canonical .pi/architecture/modules/repo-engine.md#graph
//! Implements: SymbolGraphService — add, lookup, search, remove symbols
//! Issue: #139
//!
//! Provides the application-level operations for managing the in-memory symbol graph.
//! Wraps the domain `SymbolGraph` behind the `SymbolGraphService` trait, handling DTO
//! conversion, error mapping, and reference traversal.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

#[cfg_attr(not(test), allow(unused_imports))]
use crate::repo_engine::domain::{
    RepoEngineError, SourceLanguage, SymbolDefinition, SymbolGraph, SymbolKind,
};

use super::dto::{
    AddSymbolInput, AddSymbolOutput, GraphStatsInput, GraphStatsOutput, LookupSymbolInput,
    LookupSymbolOutput, SearchSymbolsInput, SearchSymbolsOutput, SymbolsByFileInput,
    SymbolsByFileOutput,
};
use super::service::SymbolGraphService;

// ---------------------------------------------------------------------------
// SymbolGraphServiceImpl
// ---------------------------------------------------------------------------

/// Implementation of `SymbolGraphService` backed by an in-memory `SymbolGraph`.
///
/// All graph operations are synchronized through an internal `RwLock`, allowing
/// concurrent reads and exclusive writes. The underlying `SymbolGraph` domain
/// entity provides O(1) lookups by name.
///
/// # Thread Safety
/// - `add_symbol`, `remove_symbol`, `clear_graph`, `add_reference` acquire write locks
/// - `lookup_symbol`, `search_symbols`, `symbols_by_file`, `graph_stats` acquire read locks
/// - The `graph()` method returns a reference to the locked graph (not for cross-await use)
pub struct SymbolGraphServiceImpl {
    /// The underlying symbol graph protected by a read-write lock.
    graph: RwLock<SymbolGraph>,
}

impl SymbolGraphServiceImpl {
    /// Create a new `SymbolGraphServiceImpl` with an empty graph.
    pub fn new() -> Self {
        Self {
            graph: RwLock::new(SymbolGraph::new()),
        }
    }

    /// Create a new `SymbolGraphServiceImpl` with a pre-populated graph.
    pub fn from_graph(graph: SymbolGraph) -> Self {
        Self {
            graph: RwLock::new(graph),
        }
    }

    /// Create a new `SymbolGraphServiceImpl` with a capacity limit.
    pub fn with_capacity(max: usize) -> Self {
        Self {
            graph: RwLock::new(SymbolGraph::with_capacity(max)),
        }
    }
}

impl Default for SymbolGraphServiceImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SymbolGraphService for SymbolGraphServiceImpl {
    #[tracing::instrument(skip_all)]
    async fn add_symbol(&self, input: AddSymbolInput) -> Result<AddSymbolOutput, RepoEngineError> {
        let def = SymbolDefinition::new(
            input.name.clone(),
            input.kind,
            input.location,
            input.signature,
            input.definition_text,
            input.language,
        );
        let mut def = def;
        def.documentation = input.documentation;
        def.visibility = input.visibility;
        def.tags = input.tags;

        let name = def.name.clone();
        let mut graph = self.graph.write().map_err(|e| RepoEngineError::Internal {
            detail: format!("RwLock poisoned: {}", e),
        })?;

        let total_before = graph.len();
        graph.add_symbol(def)?;

        Ok(AddSymbolOutput {
            symbol_id: graph.lookup(&name).unwrap().id,
            name,
            total_symbols: total_before + 1,
            accepted: true,
        })
    }

    async fn lookup_symbol(
        &self,
        input: LookupSymbolInput,
    ) -> Result<LookupSymbolOutput, RepoEngineError> {
        let graph = self.graph.read().map_err(|e| RepoEngineError::Internal {
            detail: format!("RwLock poisoned: {}", e),
        })?;

        let symbol = graph.lookup(&input.name).cloned();

        let (references_from, references_to) = if input.include_adjacency {
            let from = graph
                .references_from(&input.name)
                .map(|set| set.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            let to = graph.references_to(&input.name);
            (from, to)
        } else {
            (Vec::new(), Vec::new())
        };

        Ok(LookupSymbolOutput {
            found: symbol.is_some(),
            symbol,
            references_from,
            references_to,
        })
    }

    async fn search_symbols(
        &self,
        input: SearchSymbolsInput,
    ) -> Result<SearchSymbolsOutput, RepoEngineError> {
        let graph = self.graph.read().map_err(|e| RepoEngineError::Internal {
            detail: format!("RwLock poisoned: {}", e),
        })?;

        let mut results: Vec<SymbolDefinition> = graph
            .search(&input.pattern)
            .into_iter().filter(|&s| {
                let kind_ok = input.kind_filter.as_ref().is_none_or(|k| &s.kind == k);
                let lang_ok = input
                    .language_filter
                    .as_ref()
                    .is_none_or(|l| &s.language == l);
                kind_ok && lang_ok
            }).cloned()
            .collect();

        let total_matches = results.len();
        let truncated = input
            .max_results
            .is_some_and(|limit| total_matches > limit);

        if let Some(limit) = input.max_results {
            results.truncate(limit);
        }

        Ok(SearchSymbolsOutput {
            symbols: results,
            total_matches,
            pattern: input.pattern,
            truncated,
        })
    }

    async fn symbols_by_file(
        &self,
        input: SymbolsByFileInput,
    ) -> Result<SymbolsByFileOutput, RepoEngineError> {
        let graph = self.graph.read().map_err(|e| RepoEngineError::Internal {
            detail: format!("RwLock poisoned: {}", e),
        })?;

        let symbols: Vec<SymbolDefinition> = graph
            .lookup_by_file(&input.file)
            .into_iter()
            .filter(|s| input.kind_filter.as_ref().is_none_or(|k| &s.kind == k)).cloned()
            .collect();

        let total = symbols.len();

        Ok(SymbolsByFileOutput {
            file: input.file,
            symbols,
            total,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn remove_symbol(&self, name: &str) -> Result<bool, RepoEngineError> {
        let mut graph = self.graph.write().map_err(|e| RepoEngineError::Internal {
            detail: format!("RwLock poisoned: {}", e),
        })?;

        if !graph.contains_key(name) {
            let suggestions: Vec<String> = graph
                .keys()
                .filter(|k| k.to_lowercase().contains(&name.to_lowercase()))
                .take(5)
                .cloned()
                .collect();
            return Err(RepoEngineError::SymbolNotFound {
                name: name.to_string(),
                suggestions,
            });
        }

        Ok(graph.remove(name))
    }

    #[tracing::instrument(skip_all)]
    async fn clear_graph(&self) -> Result<(), RepoEngineError> {
        let mut graph = self.graph.write().map_err(|e| RepoEngineError::Internal {
            detail: format!("RwLock poisoned: {}", e),
        })?;

        *graph = SymbolGraph::new();
        Ok(())
    }

    async fn graph_stats(
        &self,
        input: GraphStatsInput,
    ) -> Result<GraphStatsOutput, RepoEngineError> {
        let graph = self.graph.read().map_err(|e| RepoEngineError::Internal {
            detail: format!("RwLock poisoned: {}", e),
        })?;

        let by_kind = if input.detailed {
            let mut map = HashMap::new();
            for def in graph.all_definitions().values() {
                *map.entry(format!("{:?}", def.kind)).or_insert(0) += 1;
            }
            map
        } else {
            HashMap::new()
        };

        let by_language = {
            let mut map = HashMap::new();
            for def in graph.all_definitions().values() {
                *map.entry(format!("{:?}", def.language)).or_insert(0) += 1;
            }
            map
        };

        // Count total reference edges
        let reference_count: usize = graph
            .all_definitions()
            .keys()
            .filter_map(|k| graph.references_from(k))
            .map(|set| set.len())
            .sum();

        Ok(GraphStatsOutput {
            total_symbols: graph.len(),
            total_indexed: graph.total_indexed(),
            by_kind,
            by_language,
            max_capacity: graph.max_capacity,
            reference_count,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn add_reference(&self, from: &str, to: &str) -> Result<bool, RepoEngineError> {
        let mut graph = self.graph.write().map_err(|e| RepoEngineError::Internal {
            detail: format!("RwLock poisoned: {}", e),
        })?;

        if !graph.contains_key(from) {
            return Err(RepoEngineError::SymbolNotFound {
                name: from.to_string(),
                suggestions: graph
                    .keys()
                    .filter(|k| k.to_lowercase().contains(&from.to_lowercase()))
                    .take(5)
                    .cloned()
                    .collect(),
            });
        }

        if !graph.contains_key(to) {
            return Err(RepoEngineError::SymbolNotFound {
                name: to.to_string(),
                suggestions: graph
                    .keys()
                    .filter(|k| k.to_lowercase().contains(&to.to_lowercase()))
                    .take(5)
                    .cloned()
                    .collect(),
            });
        }

        Ok(graph.add_reference(from, to))
    }

    #[tracing::instrument(skip_all)]
    fn graph(&self) -> &SymbolGraph {
        // This method is intentionally limited — the RwLock prevents returning
        // a reference to the inner graph. Implementations requiring direct access
        // should use SharedSymbolGraph or the domain SymbolGraph directly.
        panic!("graph() returns a reference that cannot outlive the RwLock guard. Use the service methods instead.");
    }
}

// ---------------------------------------------------------------------------
// Helper to create test symbols
// ---------------------------------------------------------------------------

#[cfg(test)]
fn create_test_symbol(name: &str, kind: SymbolKind, file: &str, line: u32) -> AddSymbolInput {
    use crate::repo_engine::domain::Location;
    use std::path::PathBuf;

    AddSymbolInput {
        name: name.to_string(),
        kind,
        location: Location::new(PathBuf::from(file), line, 0),
        signature: format!("pub fn {}()", name),
        definition_text: format!("pub fn {}() {{\n    // test\n}}", name),
        language: SourceLanguage::Rust,
        documentation: Some(format!("The {} function", name)),
        visibility: Default::default(),
        tags: vec![],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo_engine::application::dto::GraphStatsInput;
    use crate::repo_engine::domain::SymbolKind;

    #[tracing::instrument(skip_all)]
    fn sample_add_input(name: &str) -> AddSymbolInput {
        create_test_symbol(name, SymbolKind::Function, "src/lib.rs", 10)
    }

    #[tokio::test]
    async fn test_add_and_lookup_symbol() {
        let service = SymbolGraphServiceImpl::new();

        let output = service
            .add_symbol(sample_add_input("my_function"))
            .await
            .unwrap();

        assert!(output.accepted);
        assert_eq!(output.name, "my_function");
        assert_eq!(output.total_symbols, 1);

        let lookup = service
            .lookup_symbol(LookupSymbolInput {
                name: "my_function".to_string(),
                include_adjacency: false,
                reference_depth: 0,
            })
            .await
            .unwrap();

        assert!(lookup.found);
        assert_eq!(lookup.symbol.as_ref().unwrap().name, "my_function");
    }

    #[tokio::test]
    async fn test_add_duplicate_returns_error() {
        let service = SymbolGraphServiceImpl::new();

        service.add_symbol(sample_add_input("dup")).await.unwrap();

        let err = service
            .add_symbol(sample_add_input("dup"))
            .await
            .unwrap_err();
        match err {
            RepoEngineError::DuplicateSymbol { name } => {
                assert_eq!(name, "dup");
            }
            _ => panic!("Expected DuplicateSymbol error"),
        }
    }

    #[tokio::test]
    async fn test_lookup_nonexistent_returns_not_found() {
        let service = SymbolGraphServiceImpl::new();

        let lookup = service
            .lookup_symbol(LookupSymbolInput {
                name: "nonexistent".to_string(),
                include_adjacency: false,
                reference_depth: 0,
            })
            .await
            .unwrap();

        assert!(!lookup.found);
        assert!(lookup.symbol.is_none());
    }

    #[tokio::test]
    async fn test_search_by_pattern() {
        let service = SymbolGraphServiceImpl::new();

        service
            .add_symbol(sample_add_input("parse_file"))
            .await
            .unwrap();
        service
            .add_symbol(sample_add_input("parse_str"))
            .await
            .unwrap();
        service
            .add_symbol(sample_add_input("render_output"))
            .await
            .unwrap();

        let result = service
            .search_symbols(SearchSymbolsInput {
                pattern: "parse".to_string(),
                kind_filter: None,
                language_filter: None,
                max_results: None,
            })
            .await
            .unwrap();

        assert_eq!(result.total_matches, 2);
        assert_eq!(result.symbols.len(), 2);
    }

    #[tokio::test]
    async fn test_search_with_kind_filter() {
        let service = SymbolGraphServiceImpl::new();

        let mut input = sample_add_input("MyStruct");
        input.kind = SymbolKind::Struct;
        service.add_symbol(input).await.unwrap();

        let mut input = sample_add_input("my_fn");
        input.kind = SymbolKind::Function;
        service.add_symbol(input).await.unwrap();

        // Search with Struct filter
        let result = service
            .search_symbols(SearchSymbolsInput {
                pattern: "My".to_string(),
                kind_filter: Some(SymbolKind::Struct),
                language_filter: None,
                max_results: None,
            })
            .await
            .unwrap();

        assert_eq!(result.total_matches, 1);
        assert_eq!(result.symbols[0].kind, SymbolKind::Struct);
    }

    #[tokio::test]
    async fn test_search_with_max_results() {
        let service = SymbolGraphServiceImpl::new();

        for i in 0..10 {
            service
                .add_symbol(sample_add_input(&format!("func_{}", i)))
                .await
                .unwrap();
        }

        let result = service
            .search_symbols(SearchSymbolsInput {
                pattern: "func".to_string(),
                kind_filter: None,
                language_filter: None,
                max_results: Some(3),
            })
            .await
            .unwrap();

        assert_eq!(result.symbols.len(), 3);
        assert_eq!(result.total_matches, 10);
        assert!(result.truncated);
    }

    #[tokio::test]
    async fn test_symbols_by_file() {
        let service = SymbolGraphServiceImpl::new();

        // Add symbols from two different files
        let mut input1 = sample_add_input("func_a");
        input1.location.file = std::path::PathBuf::from("src/module_a.rs");
        input1.location.line = 10;
        service.add_symbol(input1).await.unwrap();

        let mut input2 = sample_add_input("func_b");
        input2.location.file = std::path::PathBuf::from("src/module_a.rs");
        input2.location.line = 30;
        service.add_symbol(input2).await.unwrap();

        let mut input3 = sample_add_input("func_c");
        input3.location.file = std::path::PathBuf::from("src/module_b.rs");
        service.add_symbol(input3).await.unwrap();

        let result = service
            .symbols_by_file(SymbolsByFileInput {
                file: std::path::PathBuf::from("src/module_a.rs"),
                kind_filter: None,
            })
            .await
            .unwrap();

        assert_eq!(result.total, 2);
        assert_eq!(result.file.to_str().unwrap(), "src/module_a.rs");

        // Verify they're sorted by line number
        assert_eq!(result.symbols[0].name, "func_a");
        assert_eq!(result.symbols[1].name, "func_b");
    }

    #[tokio::test]
    async fn test_remove_symbol() {
        let service = SymbolGraphServiceImpl::new();

        service
            .add_symbol(sample_add_input("to_remove"))
            .await
            .unwrap();
        assert!(
            service
                .lookup_symbol(LookupSymbolInput {
                    name: "to_remove".to_string(),
                    include_adjacency: false,
                    reference_depth: 0,
                })
                .await
                .unwrap()
                .found
        );

        let removed = service.remove_symbol("to_remove").await.unwrap();
        assert!(removed);

        let lookup = service
            .lookup_symbol(LookupSymbolInput {
                name: "to_remove".to_string(),
                include_adjacency: false,
                reference_depth: 0,
            })
            .await
            .unwrap();
        assert!(!lookup.found);
    }

    #[tokio::test]
    async fn test_remove_nonexistent_returns_error() {
        let service = SymbolGraphServiceImpl::new();

        let err = service.remove_symbol("does_not_exist").await.unwrap_err();
        match err {
            RepoEngineError::SymbolNotFound { name, .. } => {
                assert_eq!(name, "does_not_exist");
            }
            _ => panic!("Expected SymbolNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_clear_graph() {
        let service = SymbolGraphServiceImpl::new();

        service.add_symbol(sample_add_input("sym1")).await.unwrap();
        service.add_symbol(sample_add_input("sym2")).await.unwrap();
        assert_eq!(
            service
                .graph_stats(GraphStatsInput { detailed: false })
                .await
                .unwrap()
                .total_symbols,
            2
        );

        service.clear_graph().await.unwrap();

        assert_eq!(
            service
                .graph_stats(GraphStatsInput { detailed: false })
                .await
                .unwrap()
                .total_symbols,
            0
        );
    }

    #[tokio::test]
    async fn test_add_reference() {
        let service = SymbolGraphServiceImpl::new();

        service
            .add_symbol(sample_add_input("caller_fn"))
            .await
            .unwrap();
        service
            .add_symbol(sample_add_input("callee_fn"))
            .await
            .unwrap();

        let result = service
            .add_reference("caller_fn", "callee_fn")
            .await
            .unwrap();
        assert!(result);

        // Verify via lookup with adjacency
        let lookup = service
            .lookup_symbol(LookupSymbolInput {
                name: "caller_fn".to_string(),
                include_adjacency: true,
                reference_depth: 1,
            })
            .await
            .unwrap();

        assert!(lookup.references_from.contains(&"callee_fn".to_string()));
    }

    #[tokio::test]
    async fn test_add_reference_nonexistent_source() {
        let service = SymbolGraphServiceImpl::new();
        service
            .add_symbol(sample_add_input("target"))
            .await
            .unwrap();

        let err = service
            .add_reference("no_such_fn", "target")
            .await
            .unwrap_err();
        match err {
            RepoEngineError::SymbolNotFound { name, .. } => {
                assert_eq!(name, "no_such_fn");
            }
            _ => panic!("Expected SymbolNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_graph_stats_empty() {
        let service = SymbolGraphServiceImpl::new();

        let stats = service
            .graph_stats(GraphStatsInput { detailed: false })
            .await
            .unwrap();

        assert_eq!(stats.total_symbols, 0);
        assert_eq!(stats.total_indexed, 0);
        assert_eq!(stats.reference_count, 0);
    }

    #[tokio::test]
    async fn test_graph_stats_detailed() {
        let service = SymbolGraphServiceImpl::new();

        let mut input = sample_add_input("my_struct");
        input.kind = SymbolKind::Struct;
        service.add_symbol(input).await.unwrap();

        let mut input = sample_add_input("my_fn");
        input.kind = SymbolKind::Function;
        service.add_symbol(input).await.unwrap();

        let mut input = sample_add_input("my_enum");
        input.kind = SymbolKind::Enum;
        service.add_symbol(input).await.unwrap();

        // Add another function
        let mut input = sample_add_input("another_fn");
        input.kind = SymbolKind::Function;
        service.add_symbol(input).await.unwrap();

        let stats = service
            .graph_stats(GraphStatsInput { detailed: true })
            .await
            .unwrap();

        assert_eq!(stats.total_symbols, 4);
        assert_eq!(stats.by_kind.get("Function").copied().unwrap_or(0), 2);
        assert_eq!(stats.by_kind.get("Struct").copied().unwrap_or(0), 1);
        assert_eq!(stats.by_kind.get("Enum").copied().unwrap_or(0), 1);
        assert_eq!(stats.by_language.get("Rust").copied().unwrap_or(0), 4);
    }

    #[tokio::test]
    async fn test_with_capacity() {
        let service = SymbolGraphServiceImpl::with_capacity(2);

        service.add_symbol(sample_add_input("sym1")).await.unwrap();
        service.add_symbol(sample_add_input("sym2")).await.unwrap();

        let err = service
            .add_symbol(sample_add_input("sym3"))
            .await
            .unwrap_err();
        match err {
            RepoEngineError::CapacityExceeded { capacity } => {
                assert_eq!(capacity, 2);
            }
            _ => panic!("Expected CapacityExceeded error"),
        }
    }

    #[tokio::test]
    async fn test_from_graph() {
        let mut graph = SymbolGraph::new();
        let def = SymbolDefinition::new(
            "preloaded".to_string(),
            SymbolKind::Function,
            crate::repo_engine::domain::Location::new(std::path::PathBuf::from("src/lib.rs"), 1, 0),
            "fn preloaded()".to_string(),
            "fn preloaded() {}".to_string(),
            SourceLanguage::Rust,
        );
        graph.add_symbol(def).unwrap();

        let service = SymbolGraphServiceImpl::from_graph(graph);

        let stats = service
            .graph_stats(GraphStatsInput { detailed: false })
            .await
            .unwrap();
        assert_eq!(stats.total_symbols, 1);
    }

    #[tokio::test]
    async fn test_lookup_with_adjacency() {
        let service = SymbolGraphServiceImpl::new();

        service.add_symbol(sample_add_input("a")).await.unwrap();
        service.add_symbol(sample_add_input("b")).await.unwrap();
        service.add_symbol(sample_add_input("c")).await.unwrap();

        service.add_reference("a", "b").await.unwrap();
        service.add_reference("a", "c").await.unwrap();

        let lookup = service
            .lookup_symbol(LookupSymbolInput {
                name: "a".to_string(),
                include_adjacency: true,
                reference_depth: 1,
            })
            .await
            .unwrap();

        assert_eq!(lookup.references_from.len(), 2);
        assert!(lookup.references_from.contains(&"b".to_string()));
        assert!(lookup.references_from.contains(&"c".to_string()));
    }

    #[tokio::test]
    async fn test_search_in_documentation() {
        let service = SymbolGraphServiceImpl::new();

        let mut input = sample_add_input("process_data");
        input.documentation = Some("Handles user authentication and authorization".to_string());
        service.add_symbol(input).await.unwrap();

        // Search by doc content
        let result = service
            .search_symbols(SearchSymbolsInput {
                pattern: "authentication".to_string(),
                kind_filter: None,
                language_filter: None,
                max_results: None,
            })
            .await
            .unwrap();

        assert_eq!(result.total_matches, 1);
        assert_eq!(result.symbols[0].name, "process_data");
    }

    #[tokio::test]
    async fn test_references_tracking_after_remove() {
        let service = SymbolGraphServiceImpl::new();

        service.add_symbol(sample_add_input("a")).await.unwrap();
        service.add_symbol(sample_add_input("b")).await.unwrap();
        service.add_symbol(sample_add_input("c")).await.unwrap();

        service.add_reference("a", "b").await.unwrap();
        service.add_reference("b", "c").await.unwrap();

        // Remove b — references from b should be cleaned up
        service.remove_symbol("b").await.unwrap();

        // a should still reference something that no longer exists
        let lookup_a = service
            .lookup_symbol(LookupSymbolInput {
                name: "a".to_string(),
                include_adjacency: true,
                reference_depth: 1,
            })
            .await
            .unwrap();

        // After removal of b, a's reference to b is still tracked (adjacency is cleaned on remove)
        // The domain layer removes both the symbol AND its adjacency entry
        assert!(lookup_a.references_from.is_empty());
    }
}

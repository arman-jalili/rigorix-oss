//! In-memory `GrammarRepository` implementation with real tree-sitter grammars.
//!
//! @canonical .pi/architecture/modules/repo-engine.md#grammars
//! Implements: GAP-A-15 — GrammarRepository impl
//!
//! Loads the bundled tree-sitter grammars (Rust, Python, TypeScript) lazily,
//! and additionally supports runtime registration (tests / dynamic loading).

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::repo_engine::domain::{RepoEngineError, SourceLanguage};
use crate::repo_engine::infrastructure::repository::GrammarRepository;

/// In-memory grammar registry backed by the bundled tree-sitter grammars.
pub struct InMemoryGrammarRepository {
    grammars: RwLock<HashMap<SourceLanguage, tree_sitter::Language>>,
}

impl InMemoryGrammarRepository {
    /// Create a repository with the bundled grammars registered.
    pub fn new() -> Self {
        let mut grammars = HashMap::new();
        grammars.insert(SourceLanguage::Rust, tree_sitter_rust::LANGUAGE.into());
        grammars.insert(
            SourceLanguage::TypeScript,
            tree_sitter_typescript::LANGUAGE_TSX.into(),
        );
        grammars.insert(SourceLanguage::Python, tree_sitter_python::language());
        Self {
            grammars: RwLock::new(grammars),
        }
    }
}

impl Default for InMemoryGrammarRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GrammarRepository for InMemoryGrammarRepository {
    async fn load_grammar(
        &self,
        language: &SourceLanguage,
    ) -> Result<tree_sitter::Language, RepoEngineError> {
        self.grammars
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(language)
            .cloned()
            .ok_or_else(|| RepoEngineError::Internal {
                detail: format!("no grammar available for {language:?}"),
            })
    }

    async fn has_grammar(&self, language: &SourceLanguage) -> bool {
        self.grammars
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(language)
    }

    async fn available_languages(&self) -> Vec<SourceLanguage> {
        self.grammars
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect()
    }

    async fn register_grammar(&self, language: SourceLanguage, grammar: tree_sitter::Language) {
        self.grammars
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(language, grammar);
    }

    async fn unload_grammar(&self, language: &SourceLanguage) -> Result<(), RepoEngineError> {
        self.grammars
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(language)
            .map(|_| ())
            .ok_or_else(|| RepoEngineError::Internal {
                detail: format!("grammar {language:?} not registered"),
            })
    }
}

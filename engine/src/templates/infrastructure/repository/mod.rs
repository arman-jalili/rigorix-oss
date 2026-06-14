//! Repository interfaces for the Template System bounded context.
//!
//! @canonical .pi/architecture/modules/template-system.md
//! Implements: TemplateRepository trait
//! Issue: #101
//!
//! Repositories abstract template storage and retrieval behind interfaces,
//! allowing implementations to use filesystem, embedded, or mock storage
//! without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::templates::domain::TemplateError;

use super::super::application::dto::{LoadBuiltinsInput, LoadBuiltinsOutput};

/// Repository for template source data.
///
/// Abstracts the source of template definitions — whether from the filesystem,
/// embedded built-in definitions, or remote storage.
///
/// # Contract (Frozen)
/// - Read operations return raw TOML content for parsing by TemplateParserService
/// - Directory listing returns file paths matching the configured extension
/// - Implementations MUST validate file paths against directory traversal attacks
/// - Built-in templates are loaded from embedded source, not the filesystem
#[async_trait]
pub trait TemplateRepository: Send + Sync {
    /// Read a template file as raw TOML content.
    ///
    /// Returns the raw content for parsing by `TemplateParserService`.
    /// Returns `TemplateError::NotFound` if the file doesn't exist.
    /// Returns `TemplateError::Io` for filesystem errors.
    async fn read_template_file(&self, path: &str) -> Result<String, TemplateError>;

    /// List all template files in a directory.
    ///
    /// Returns file paths matching the configured extension.
    /// Returns an empty vec if the directory doesn't exist (not an error).
    async fn list_template_files(
        &self,
        dir: &str,
        extension: &str,
    ) -> Result<Vec<String>, TemplateError>;

    /// Check if a template file exists at the given path.
    async fn template_file_exists(&self, path: &str) -> bool;

    /// Load built-in template definitions.
    ///
    /// Returns the TOML content for each built-in template by ID.
    /// Built-in templates are embedded at compile time.
    async fn load_builtin_sources(
        &self,
        input: LoadBuiltinsInput,
    ) -> Result<LoadBuiltinsOutput, TemplateError>;

    /// Read a single built-in template source by ID.
    ///
    /// Returns `None` if no built-in template with that ID exists.
    async fn get_builtin_source(&self, id: &str) -> Option<&'static str>;

    /// List available built-in template IDs.
    async fn list_builtin_ids(&self) -> Vec<&'static str>;
}

// ---------------------------------------------------------------------------
// InMemoryTemplateRepository
// ---------------------------------------------------------------------------

/// An in-memory `TemplateRepository` for testing.
///
/// Stores template file content and built-in definitions in memory.
pub struct InMemoryTemplateRepository {
    /// Map of file path → TOML content
    sources: RwLock<HashMap<String, String>>,
    /// Map of built-in ID → TOML content
    builtins: RwLock<HashMap<String, &'static str>>,
    /// List of built-in IDs (for returning &'static str)
    builtin_ids: RwLock<Vec<&'static str>>,
}

impl InMemoryTemplateRepository {
    /// Create an empty in-memory repository.
    pub fn new() -> Self {
        Self {
            sources: RwLock::new(HashMap::new()),
            builtins: RwLock::new(HashMap::new()),
            builtin_ids: RwLock::new(Vec::new()),
        }
    }

    /// Add a template file source to the repository.
    pub fn add_source(&mut self, path: String, content: String) {
        self.sources
            .write()
            .expect("lock poisoned")
            .insert(path, content);
    }

    /// Add a built-in template definition.
    pub fn add_builtin(&mut self, id: &'static str, toml: &'static str) {
        self.builtins
            .write()
            .expect("lock poisoned")
            .insert(id.to_string(), toml);
        self.builtin_ids.write().expect("lock poisoned").push(id);
    }
}

impl Default for InMemoryTemplateRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TemplateRepository for InMemoryTemplateRepository {
    async fn read_template_file(&self, path: &str) -> Result<String, TemplateError> {
        let sources = self.sources.read().expect("lock poisoned");
        sources
            .get(path)
            .cloned()
            .ok_or_else(|| TemplateError::NotFound {
                id: path.to_string(),
                available: sources.keys().cloned().collect(),
            })
    }

    async fn list_template_files(
        &self,
        _dir: &str,
        _extension: &str,
    ) -> Result<Vec<String>, TemplateError> {
        let sources = self.sources.read().expect("lock poisoned");
        Ok(sources.keys().cloned().collect())
    }

    async fn template_file_exists(&self, path: &str) -> bool {
        let sources = self.sources.read().expect("lock poisoned");
        sources.contains_key(path)
    }

    async fn load_builtin_sources(
        &self,
        _input: LoadBuiltinsInput,
    ) -> Result<LoadBuiltinsOutput, TemplateError> {
        let builtins = self.builtins.read().expect("lock poisoned");
        Ok(LoadBuiltinsOutput {
            loaded: builtins.keys().cloned().collect(),
            count: builtins.len(),
        })
    }

    async fn get_builtin_source(&self, id: &str) -> Option<&'static str> {
        let builtins = self.builtins.read().expect("lock poisoned");
        builtins.get(id).copied()
    }

    async fn list_builtin_ids(&self) -> Vec<&'static str> {
        let ids = self.builtin_ids.read().expect("lock poisoned");
        ids.clone()
    }
}

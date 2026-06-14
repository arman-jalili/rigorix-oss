//! Repository interfaces for the Template System bounded context.
//!
//! @canonical .pi/architecture/modules/template-system.md
//! Implements: Contract Freeze — TemplateRepository trait
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

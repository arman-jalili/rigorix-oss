//! Aggregate root and domain service traits for Template Tools.
//!
//! @canonical .pi/architecture/modules/template-tools.md#entities
//! Implements: Contract Freeze — TemplateRepository trait, TemplateConverter trait
//!
//! # TemplateRepository (Aggregate Root)
//!
//! Filesystem-backed repository for plan templates stored as TOML files
//! in `.rigorix/templates/`. All template operations go through this interface.
//!
//! # TemplateConverter (Domain Service)
//!
//! Converts between TOML (filesystem storage) and JSON (MCP transport)
//! template formats with schema validation.
//!
//! # Contract (Frozen)
//!
//! - All methods are async (use async-trait)
//! - All methods return Result with TemplateError
//! - No implementation logic — pure interface
//! - Thread-safe (Send + Sync)

use async_trait::async_trait;
use std::sync::Arc;

use super::error::TemplateError;
use super::value::{PlanTemplate, TemplateFilter, TemplateSummary};

// ---------------------------------------------------------------------------
// TemplateRepository — Aggregate Root
// ---------------------------------------------------------------------------

/// Filesystem-backed repository for plan templates stored as TOML files.
///
/// All template operations (list, get, create, delete, exists) go through
/// this interface. The filesystem implementation handles atomic writes,
/// file locking, and TOML serialization.
///
/// # Invariants (Frozen)
///
/// - All template files are valid TOML conforming to the PlanTemplate schema
/// - Writes are atomic: write to temp file → fsync → rename
/// - Concurrent writes are serialized via file locking
/// - Template names are filesystem-safe (only `[a-zA-Z0-9_-]` characters)
/// - Templates are immutable once created (update is delete + create)
#[async_trait]
pub trait TemplateRepository: Send + Sync {
    /// List all templates matching the given filter criteria.
    ///
    /// Discovers templates from the filesystem and returns summaries
    /// matching the filter. Returns empty vec if no templates match.
    ///
    /// # Errors
    /// - `TemplateError::RepositoryError` if filesystem read fails
    async fn list(&self, filter: &TemplateFilter) -> Result<Vec<TemplateSummary>, TemplateError>;

    /// Get a single template by name.
    ///
    /// Reads and deserializes the TOML file for the named template.
    ///
    /// # Errors
    /// - `TemplateError::NotFound` if no template with that name exists
    /// - `TemplateError::DeserializationFailed` if TOML is malformed
    /// - `TemplateError::RepositoryError` if filesystem read fails
    async fn get(&self, name: &str) -> Result<PlanTemplate, TemplateError>;

    /// Create a new template.
    ///
    /// Serializes the template to TOML and writes it atomically to the
    /// filesystem. If `overwrite` is false and the template already exists,
    /// returns `TemplateError::AlreadyExists`.
    ///
    /// # Errors
    /// - `TemplateError::AlreadyExists` if template exists and overwrite is false
    /// - `TemplateError::SerializationFailed` if TOML serialization fails
    /// - `TemplateError::RepositoryError` if filesystem write fails
    async fn create(&self, template: PlanTemplate, overwrite: bool) -> Result<(), TemplateError>;

    /// Delete an existing template.
    ///
    /// Removes the TOML file from the filesystem.
    ///
    /// # Errors
    /// - `TemplateError::NotFound` if no template with that name exists
    /// - `TemplateError::RepositoryError` if filesystem delete fails
    async fn delete(&self, name: &str) -> Result<(), TemplateError>;

    /// Check if a template with the given name exists.
    ///
    /// Returns `true` if the TOML file exists on disk, `false` otherwise.
    /// This is a fast check — does not deserialize the file contents.
    ///
    /// # Errors
    /// - `TemplateError::RepositoryError` if filesystem access fails
    async fn exists(&self, name: &str) -> Result<bool, TemplateError>;
}

/// Shared ownership of a TemplateRepository implementation.
pub type SharedTemplateRepository = Arc<dyn TemplateRepository>;

// ---------------------------------------------------------------------------
// TemplateConverter — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that converts between TOML (filesystem storage) and
/// JSON (MCP transport) template formats.
///
/// # Contract (Frozen)
///
/// - Input validation happens before conversion
/// - TOML output should be valid TOML conforming to the template schema
/// - JSON output should be valid JSON for MCP transport
/// - Schema validation catches structural issues before storage
#[async_trait]
#[allow(clippy::wrong_self_convention)]
pub trait TemplateConverter: Send + Sync {
    /// Convert a PlanTemplate to TOML format for filesystem storage.
    ///
    /// # Errors
    /// - `TemplateError::SerializationFailed` if TOML serialization fails
    async fn to_toml(&self, template: &PlanTemplate) -> Result<String, TemplateError>;

    /// Convert a PlanTemplate to JSON format for MCP transport.
    ///
    /// # Errors
    /// - `TemplateError::SerializationFailed` if JSON serialization fails
    async fn to_json(&self, template: &PlanTemplate) -> Result<serde_json::Value, TemplateError>;

    /// Parse and validate a TOML string into a PlanTemplate.
    ///
    /// # Errors
    /// - `TemplateError::DeserializationFailed` if TOML is malformed
    /// - `TemplateError::ValidationError` if the template structure is invalid
    async fn from_toml(&self, toml_str: &str) -> Result<PlanTemplate, TemplateError>;

    /// Parse and validate a JSON value into a PlanTemplate.
    ///
    /// # Errors
    /// - `TemplateError::DeserializationFailed` if JSON doesn't match schema
    /// - `TemplateError::ValidationError` if the template structure is invalid
    async fn from_json(&self, json: serde_json::Value) -> Result<PlanTemplate, TemplateError>;

    /// Validate a TOML string's structure without deserializing.
    ///
    /// Fast validation that checks basic TOML syntax and schema compliance
    /// without constructing the full PlanTemplate.
    ///
    /// # Errors
    /// - `TemplateError::DeserializationFailed` if TOML syntax is invalid
    /// - `TemplateError::ValidationError` if the template structure is invalid
    async fn validate_toml(&self, toml_str: &str) -> Result<(), TemplateError>;

    /// Validate and convert a JSON value into a PlanTemplate.
    ///
    /// Combines schema validation and deserialization. This is the primary
    /// entry point for `rigorix_create_template` and `rigorix_validate_template`.
    ///
    /// # Errors
    /// - `TemplateError::DeserializationFailed` if JSON doesn't match schema
    /// - `TemplateError::ValidationError` if the template structure is invalid
    async fn validate_and_convert(
        &self,
        json: serde_json::Value,
    ) -> Result<PlanTemplate, TemplateError>;
}

/// Shared ownership of a TemplateConverter implementation.
pub type SharedTemplateConverter = Arc<dyn TemplateConverter>;

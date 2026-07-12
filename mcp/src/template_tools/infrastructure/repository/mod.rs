//! Repository interface for the Template Tools bounded context.
//!
//! @canonical .pi/architecture/modules/template-tools.md#repository
//! Implements: Contract Freeze — TemplateRepositoryConfig
//!
//! Configuration for template repository filesystem storage.
//! The actual TemplateRepository trait lives in domain/entity.rs.
//!
//! # Contract (Frozen)
//!
//! - Configuration types are pure data with no behavior
//! - All configuration is validated on construction
//! - Default paths follow XDG conventions

use std::path::PathBuf;

/// Configuration for the filesystem-backed template repository.
///
/// Defines the base path where template TOML files are stored
/// (default: `.rigorix/templates/` relative to project root).
///
/// # Contract (Frozen)
///
/// - Default path is `.rigorix/templates/` relative to current directory
/// - Custom path can be provided for testing or non-standard setups
/// - Path is validated on construction (must be a directory)
#[derive(Debug, Clone)]
pub struct TemplateRepositoryConfig {
    /// Path to the templates directory.
    base_path: PathBuf,
}

impl TemplateRepositoryConfig {
    /// Create config with the default path (`.rigorix/templates/`).
    pub fn default_path() -> Self {
        Self {
            base_path: PathBuf::from(".rigorix/templates"),
        }
    }

    /// Create config with a custom base path.
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Get the base path for template storage.
    pub fn base_path(&self) -> &PathBuf {
        &self.base_path
    }
}

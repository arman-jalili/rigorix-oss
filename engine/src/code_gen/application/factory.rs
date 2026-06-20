//! Factory interfaces for constructing Code Generation service instances.
//!
//! @canonical .pi/architecture/modules/code-generation.md
//! Implements: Contract Freeze — SyntaxGateFactory trait, EditFileFactory trait
//! Issue: #424
//!
//! Factories encapsulate the construction of service instances, allowing
//! implementations to inject dependencies (tree-sitter parsers, configuration)
//! and apply default configurations.

use crate::code_gen::domain::error::CodeGenError;

use super::dto::CodeGenConfig;
use super::service::{EditFileService, ReadFileService, SyntaxGateService};

/// Factory for constructing `SyntaxGateService` instances.
pub trait SyntaxGateFactory: Send + Sync {
    /// Create a SyntaxGateService with default configuration.
    fn create_default(&self) -> Result<Box<dyn SyntaxGateService>, CodeGenError>;

    /// Create a SyntaxGateService with explicit configuration.
    fn create(
        &self,
        config: super::dto::SyntaxGateConfig,
    ) -> Result<Box<dyn SyntaxGateService>, CodeGenError>;

    /// Create a SyntaxGateService with the given tree-sitter parsers.
    fn create_with_parsers(
        &self,
        config: super::dto::SyntaxGateConfig,
        parsers: std::collections::HashMap<String, tree_sitter::Parser>,
    ) -> Result<Box<dyn SyntaxGateService>, CodeGenError>;
}

/// Factory for constructing `EditFileService` instances.
pub trait EditFileFactory: Send + Sync {
    /// Create an EditFileService with default configuration.
    fn create_default(&self) -> Result<Box<dyn EditFileService>, CodeGenError>;

    /// Create an EditFileService with explicit configuration.
    fn create(&self, config: CodeGenConfig) -> Result<Box<dyn EditFileService>, CodeGenError>;
}

/// Factory for constructing `ReadFileService` instances.
pub trait ReadFileFactory: Send + Sync {
    /// Create a ReadFileService with the given workspace root.
    fn create(&self, workspace_root: String) -> Result<Box<dyn ReadFileService>, CodeGenError>;
}

//! Factory interfaces for constructing Code Graph service instances.
//!
//! @canonical .pi/architecture/modules/code-graph.md
//! Implements: Contract Freeze — CodeGraphServiceFactory, CodeGraphAnalyzerFactory,
//!   CodeGraphFormatterFactory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of CodeGraphService, CodeGraphAnalyzer,
//! and CodeGraphFormatter instances with appropriate storage paths, configuration,
//! and backend selection.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured service instance
//! - Configuration is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::code_graph::domain::CodeGraphError;

use super::service::{CodeGraphAnalyzer, CodeGraphFormatter, CodeGraphImporter, CodeGraphService};

// ---------------------------------------------------------------------------
// CodeGraphServiceFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `CodeGraphService` instances.
///
/// Handles creation of the code graph service with appropriate storage
/// configuration for persisting CodeGraph records.
#[async_trait]
pub trait CodeGraphServiceFactory: Send + Sync {
    /// Create a `CodeGraphService` instance.
    ///
    /// Initialises the graph storage directory (creating it if it doesn't
    /// exist) and configures the persistence backend.
    async fn create(
        &self,
        config: CodeGraphServiceConfig,
    ) -> Result<Box<dyn CodeGraphService>, CodeGraphError>;
}

/// Configuration for creating a `CodeGraphService` instance.
#[derive(Debug, Clone)]
pub struct CodeGraphServiceConfig {
    /// Directory path for persisting CodeGraph records.
    pub graph_storage_dir: Option<String>,

    /// Maximum number of concurrent graph construction operations.
    pub max_concurrent_operations: usize,

    /// Whether to create the storage directory if it doesn't exist.
    pub create_dir_if_missing: bool,

    /// Storage backend to use (e.g., "filesystem", "database").
    pub storage_backend: String,
}

impl Default for CodeGraphServiceConfig {
    fn default() -> Self {
        Self {
            graph_storage_dir: None,
            max_concurrent_operations: 4,
            create_dir_if_missing: true,
            storage_backend: "filesystem".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// CodeGraphAnalyzerFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `CodeGraphAnalyzer` instances.
///
/// Handles creation of the graph analyzer with analysis configuration
/// for dependency resolution and impact analysis.
#[async_trait]
pub trait CodeGraphAnalyzerFactory: Send + Sync {
    /// Create a `CodeGraphAnalyzer` instance.
    ///
    /// Configures the analyzer with maximum traversal depth and
    /// cycle detection settings.
    async fn create(
        &self,
        config: CodeGraphAnalyzerConfig,
    ) -> Result<Box<dyn CodeGraphAnalyzer>, CodeGraphError>;
}

/// Configuration for creating a `CodeGraphAnalyzer` instance.
#[derive(Debug, Clone)]
pub struct CodeGraphAnalyzerConfig {
    /// Default maximum depth for transitive analysis.
    pub max_traversal_depth: u32,

    /// Whether to abort analysis on first cycle found.
    pub abort_on_first_cycle: bool,

    /// Maximum number of cycles to report (0 = unlimited).
    pub max_cycles_to_report: u32,
}

impl Default for CodeGraphAnalyzerConfig {
    fn default() -> Self {
        Self {
            max_traversal_depth: 10,
            abort_on_first_cycle: false,
            max_cycles_to_report: 100,
        }
    }
}

// ---------------------------------------------------------------------------
// CodeGraphFormatterFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `CodeGraphFormatter` instances.
///
/// Handles creation of the graph formatter with output configuration
/// for rendering graphs in various formats.
#[async_trait]
pub trait CodeGraphFormatterFactory: Send + Sync {
    /// Create a `CodeGraphFormatter` instance.
    async fn create(
        &self,
        config: CodeGraphFormatterConfig,
    ) -> Result<Box<dyn CodeGraphFormatter>, CodeGraphError>;
}

/// Configuration for creating a `CodeGraphFormatter` instance.
#[derive(Debug, Clone)]
pub struct CodeGraphFormatterConfig {
    /// Default output format.
    pub default_format: super::dto::OutputFormat,

    /// Whether to include metadata in output by default.
    pub include_metadata: bool,

    /// Maximum label length before truncation.
    pub max_label_length: usize,
}

impl Default for CodeGraphFormatterConfig {
    fn default() -> Self {
        Self {
            default_format: super::dto::OutputFormat::Tree,
            include_metadata: false,
            max_label_length: 80,
        }
    }
}

// ---------------------------------------------------------------------------
// CodeGraphImporterFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `CodeGraphImporter` instances.
///
/// Handles creation of the graph importer with batch import configuration
/// for populating graphs from external analysis tools.
#[async_trait]
pub trait CodeGraphImporterFactory: Send + Sync {
    /// Create a `CodeGraphImporter` instance.
    async fn create(
        &self,
        config: CodeGraphImporterConfig,
    ) -> Result<Box<dyn CodeGraphImporter>, CodeGraphError>;
}

/// Configuration for creating a `CodeGraphImporter` instance.
#[derive(Debug, Clone)]
pub struct CodeGraphImporterConfig {
    /// Maximum number of nodes per batch import.
    pub max_nodes_per_batch: u32,

    /// Whether to auto-seal after import.
    pub auto_seal_after_import: bool,
}

impl Default for CodeGraphImporterConfig {
    fn default() -> Self {
        Self {
            max_nodes_per_batch: 1000,
            auto_seal_after_import: false,
        }
    }
}

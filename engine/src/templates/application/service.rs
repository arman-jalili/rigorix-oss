//! Service interfaces (use cases) for the Template System bounded context.
//!
//! @canonical .pi/architecture/modules/template-system.md
//! Implements: Contract Freeze — TemplateParserService and TemplateEngineService traits
//! Issue: #101
//!
//! These traits define the application-level operations for template parsing
//! and the template engine. All methods are async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::templates::domain::TemplateError;

use super::dto::{
    GenerateInput, GenerateOutput, GetTemplateInput, ListTemplatesOutput, LoadBuiltinsInput,
    LoadBuiltinsOutput, LoadDirectoryOutput, ParseFileInput, ParseOutput, ParseStrInput,
    RegisterInput, RegisterOutput, ValidateTemplateInput, ValidateTemplateOutput,
};

// ---------------------------------------------------------------------------
// TemplateParserService
// ---------------------------------------------------------------------------

/// Application service for parsing and validating TOML template files.
///
/// Handles TOML deserialization into `Template` structs, schema validation,
/// directory scanning, and file loading.
///
/// # Contract (Frozen)
/// - Parsing returns validated `Template` structs
/// - Validation checks: structural integrity, cycle detection, parameter references
/// - Directory loading aggregates successes and failures
/// - All methods return structured DTOs with error context
#[async_trait]
pub trait TemplateParserService: Send + Sync {
    /// Parse a template from a TOML file at the given path.
    ///
    /// Validates the parsed template if `input.validate` is `true`.
    /// Returns `TemplateError::NotFound` if the file doesn't exist.
    /// Returns `TemplateError::Parse` if the TOML content is invalid.
    async fn parse_file(&self, input: ParseFileInput) -> Result<ParseOutput, TemplateError>;

    /// Parse a template from a TOML string.
    ///
    /// Useful for parsing templates from LLM output, built-in definitions,
    /// or other non-file sources.
    async fn parse_str(&self, input: ParseStrInput) -> Result<ParseOutput, TemplateError>;

    /// Load all template files from a directory.
    ///
    /// Scans the directory for files matching the configured extension,
    /// parses each one, and returns aggregated results. Files that fail
    /// to parse are reported in `failures` rather than failing the whole load.
    async fn load_directory(&self, path: &str) -> Result<LoadDirectoryOutput, TemplateError>;

    /// Validate a template definition.
    ///
    /// Checks:
    /// - Node dependency references are valid
    /// - Parameter references in actions are valid
    /// - Cycle detection (optional)
    /// - Structural integrity (unique node IDs, valid action types)
    async fn validate_template(
        &self,
        input: ValidateTemplateInput,
    ) -> Result<ValidateTemplateOutput, TemplateError>;

    /// Load built-in templates.
    ///
    /// Loads the 13 built-in template definitions that ship with Rigorix.
    /// Returns a list of loaded template IDs.
    async fn load_builtins(
        &self,
        input: LoadBuiltinsInput,
    ) -> Result<LoadBuiltinsOutput, TemplateError>;
}

// ---------------------------------------------------------------------------
// TemplateEngineService
// ---------------------------------------------------------------------------

/// Application service for the template runtime registry and graph generation.
///
/// Manages the template registry (register, lookup, list) and generates
/// executable graphs from registered templates with parameter substitution.
///
/// # Contract (Frozen)
/// - Templates must be registered before they can be used for generation
/// - Registration replaces only if `overwrite` is explicitly set
/// - Generation performs `{{ param }}` substitution with the provided values
/// - Unknown parameters in the template are left as-is (no error)
/// - Missing required parameters return `TemplateError::MissingParameter`
///
/// @todo When the DAG Engine module (crate::dag) is implemented, update
///   `generate()` to return `Result<crate::dag::graph::TaskGraph, TemplateError>`
///   and add a `seal()` step that performs topological sort.
#[async_trait]
pub trait TemplateEngineService: Send + Sync {
    /// Register a template in the engine's runtime registry.
    ///
    /// Returns `TemplateError::DuplicateTemplate` if a template with the
    /// same ID already exists and `overwrite` is not set.
    async fn register(&self, input: RegisterInput) -> Result<RegisterOutput, TemplateError>;

    /// Generate an executable graph from a registered template.
    ///
    /// Performs `{{ param }}` substitution on all node actions using
    /// the provided parameters. Validates that all required parameters
    /// are present. Detects dependency cycles in the generated graph.
    ///
    /// Returns a `GenerateOutput` DTO containing the resolved nodes and edges.
    /// When the DAG Engine module exists, this will return a `TaskGraph` directly.
    async fn generate(&self, input: GenerateInput) -> Result<GenerateOutput, TemplateError>;

    /// Look up a registered template by ID.
    ///
    /// Returns `None` if no template with that ID is registered.
    async fn get_template(
        &self,
        input: GetTemplateInput,
    ) -> Result<Option<super::dto::TemplateSummary>, TemplateError>;

    /// List all registered templates with summary metadata.
    async fn list_templates(&self) -> Result<ListTemplatesOutput, TemplateError>;

    /// Check if a template is registered.
    async fn has_template(&self, template_id: &str) -> bool;

    /// Get the total number of registered templates.
    async fn template_count(&self) -> usize;
}

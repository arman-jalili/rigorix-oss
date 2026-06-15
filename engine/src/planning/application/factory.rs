//! Factory interfaces for constructing Planning Pipeline components.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: Contract Freeze — PlanningPipelineFactory trait
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of the PlanningPipelineService
//! with appropriate classifier, extractor, and template engine dependencies.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured PlanningPipelineService
//! - Dependencies are injected at construction time
//! - No mutable state in factory implementations

use async_trait::async_trait;
use uuid::Uuid;

use crate::dag_engine::domain::TaskGraph;
use crate::planning::domain::classification::Classifier;
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::extractor::ParameterExtractor;
use crate::template_generation::domain::TemplateGenerator;

use super::dto::{ValidationError, ValidationWarning};
use super::service::{PlanningPipelineService, SymbolValidationService, TemplateGenerationService};

/// Factory for constructing `PlanningPipelineService` instances.
///
/// Handles creation of the planning pipeline with appropriate
/// classifier, parameter extractor, and template engine. Supports
/// optional generator fallback injection.
///
/// # Contract (Frozen)
/// - `create_default` — Builds with default classifier + extractor
/// - `create_with_generator` — Builds with generator fallback support
/// - `create_custom` — Builds with fully custom dependencies
/// - All methods validate that required dependencies are present
#[async_trait]
pub trait PlanningPipelineFactory: Send + Sync {
    /// Create a default planning pipeline with classifier and extractor.
    ///
    /// Builds the pipeline with the provided Classifier and ParameterExtractor.
    /// No TemplateGenerator fallback is configured — low-confidence intents
    /// will be returned as clarification requests.
    ///
    /// # Arguments
    ///
    /// * `classifier` — The intent classification engine.
    /// * `extractor` — The parameter extraction engine.
    /// * `template_service` — The template engine service for graph generation.
    ///
    /// # Errors
    ///
    /// Returns `PlanningError::InvalidState` if required dependencies cannot
    /// be initialised from the current configuration.
    async fn create_default(
        &self,
        classifier: Box<dyn Classifier>,
        extractor: Box<dyn ParameterExtractor>,
        template_service: Box<
            dyn crate::templates::application::service::TemplateEngineService,
        >,
    ) -> Result<Box<dyn PlanningPipelineService>, PlanningError>;

    /// Create a planning pipeline with template generator fallback.
    ///
    /// Builds the pipeline with all dependencies including a
    /// TemplateGenerator. When the classifier finds no good match
    /// (confidence < 0.3), the pipeline falls back to the generator
    /// to create a new template on the fly.
    ///
    /// # Arguments
    ///
    /// * `classifier` — The intent classification engine.
    /// * `extractor` — The parameter extraction engine.
    /// * `template_service` — The template engine service for graph generation.
    /// * `template_generator` — Fallback for generating new templates.
    ///
    /// # Errors
    ///
    /// Returns `PlanningError::InvalidState` if required dependencies cannot
    /// be initialised from the current configuration.
    async fn create_with_generator(
        &self,
        classifier: Box<dyn Classifier>,
        extractor: Box<dyn ParameterExtractor>,
        template_service: Box<
            dyn crate::templates::application::service::TemplateEngineService,
        >,
        template_generator: Box<dyn TemplateGenerator>,
    ) -> Result<Box<dyn PlanningPipelineService>, PlanningError>;

    /// Create a planning pipeline with fully custom dependencies.
    ///
    /// Builds the pipeline with explicitly provided components. Useful
    /// for testing, advanced configurations, or when the caller wants
    /// complete control over the pipeline composition.
    ///
    /// # Arguments
    ///
    /// * `classifier` — The intent classification engine.
    /// * `extractor` — The parameter extraction engine.
    /// * `template_service` — The template engine service for graph generation.
    /// * `template_generator` — Optional fallback for generating new templates.
    /// * `validator` — Optional composite validator for plan validation.
    async fn create_custom(
        &self,
        classifier: Box<dyn Classifier>,
        extractor: Box<dyn ParameterExtractor>,
        template_service: Box<
            dyn crate::templates::application::service::TemplateEngineService,
        >,
        template_generator: Option<Box<dyn TemplateGenerator>>,
        validator: Option<Box<dyn CompositeValidator>>,
    ) -> Result<Box<dyn PlanningPipelineService>, PlanningError>;
}

/// Composite validator interface for plan validation.
///
/// Runs a set of validation rules against a generated plan/graph
/// and returns errors and warnings.
///
/// # Contract (Frozen)
/// - Validate returns errors (blocking) and warnings (non-blocking)
/// - Zero errors means the plan is valid for execution
/// - Implementations may run rules in parallel
#[async_trait]
pub trait CompositeValidator: Send + Sync {
    /// Validate a generated TaskGraph.
    ///
    /// # Arguments
    ///
    /// * `execution_id` — The execution ID for correlation.
    /// * `graph` — The TaskGraph to validate.
    /// * `template_id` — The template that generated this graph.
    ///
    /// # Returns
    ///
    /// A tuple of (errors, warnings) where errors are blocking and
    /// warnings are informational.
    async fn validate(
        &self,
        execution_id: Uuid,
        graph: &TaskGraph,
        template_id: &str,
    ) -> Result<(Vec<ValidationError>, Vec<ValidationWarning>), PlanningError>;

    /// Get the number of registered validation rules.
    fn rule_count(&self) -> u32;
}

// ---------------------------------------------------------------------------
// TemplateGenerationFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `TemplateGenerationService` instances.
///
/// Handles creation of the template generation service with appropriate
/// LLM client, repository context builder, symbol validator, and
/// template engine dependencies.
///
/// # Contract (Frozen)
/// - `create_default` — Builds with default LLM generator + symbol validation
/// - `create_with_custom_generator` — Builds with a custom TemplateGenerator
/// - `create_without_symbol_validation` — Skips Phase 3 validation (dev mode)
#[async_trait]
pub trait TemplateGenerationFactory: Send + Sync {
    /// Create a default template generation service.
    ///
    /// Builds with the default Claude-based template generator and
    /// full Phase 3 symbol validation.
    async fn create_default(
        &self,
        symbol_validation: Box<dyn SymbolValidationService>,
        template_engine: Box<
            dyn crate::templates::application::service::TemplateEngineService,
        >,
    ) -> Result<Box<dyn TemplateGenerationService>, PlanningError>;

    /// Create a template generation service with a custom generator.
    ///
    /// Useful for testing with `MockGenerator` or using a different
    /// LLM provider (OpenAI, etc.).
    async fn create_with_generator(
        &self,
        generator: Box<dyn TemplateGenerator>,
        symbol_validation: Box<dyn SymbolValidationService>,
        template_engine: Box<
            dyn crate::templates::application::service::TemplateEngineService,
        >,
    ) -> Result<Box<dyn TemplateGenerationService>, PlanningError>;

    /// Create a template generation service without Phase 3 validation.
    ///
    /// Development/debugging mode only. Generated templates will not
    /// be validated against the symbol graph.
    async fn create_without_validation(
        &self,
        generator: Box<dyn TemplateGenerator>,
    ) -> Result<Box<dyn TemplateGenerationService>, PlanningError>;
}

// ---------------------------------------------------------------------------
// SymbolValidationFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `SymbolValidationService` instances.
///
/// Handles creation of the symbol validation service with the
/// appropriate symbol graph client and validation configuration.
///
/// # Contract (Frozen)
/// - `create_default` — Builds with repo engine symbol graph
/// - `create_disabled` — Returns a pass-through validator (no-op)
#[async_trait]
pub trait SymbolValidationFactory: Send + Sync {
    /// Create a default symbol validation service.
    ///
    /// Validates against the indexed symbol graph from the Repo Engine.
    async fn create_default(
        &self,
        symbol_graph: Box<
            dyn crate::repo_engine::application::service::SymbolGraphService,
        >,
    ) -> Result<Box<dyn SymbolValidationService>, PlanningError>;

    /// Create a disabled symbol validation service (pass-through).
    ///
    /// All templates pass validation. Useful for development or when
    /// the symbol graph is not available.
    async fn create_disabled(&self) -> Result<Box<dyn SymbolValidationService>, PlanningError>;
}

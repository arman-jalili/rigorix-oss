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
use crate::planning::domain::generator::TemplateGenerator;

use super::dto::{ValidationError, ValidationWarning};
use super::service::PlanningPipelineService;

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

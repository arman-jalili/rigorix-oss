//! Factory interfaces and builder for constructing Plan Validation components.
//!
//! @canonical .pi/architecture/modules/plan-validation.md
//! Implements: Contract Freeze — ValidationLoopFactory, ValidationLoopConfigFactory
//! Issue: issue-validationloop-config, issue-validationloopservice
//!
//! Factories encapsulate the construction of validation loop components
//! with appropriate planning pipeline, execution engine, failure parser,
//! and quality gate dependencies.
//!
//! # Contract (Frozen)
//! - Factory methods return configured ValidationLoopService instances
//! - Dependencies are injected at construction time
//! - Builder pattern for optional dependency injection

use async_trait::async_trait;

use crate::failure_parser::application::service::FailureParserService;
use crate::plan_validation::application::service::{
    QualityGateEvaluationService, ValidationLoopService,
};
use crate::plan_validation::domain::error::ValidationLoopError;
use crate::plan_validation::domain::loop_config::ValidationLoopConfig;
use crate::planning::application::service::PlanningPipelineService;
use crate::quality_gates::application::service::QualityGateService;

/// Factory for constructing `ValidationLoopService` instances.
///
/// Handles creation of the validation loop with appropriate
/// planning pipeline, execution engine, failure parser, and
/// quality gate dependencies.
///
/// # Contract (Frozen)
/// - `create_default` — Builds with default dependencies
/// - `create_custom` — Builds with fully custom dependencies
/// - All methods validate that required dependencies are present
#[async_trait]
pub trait ValidationLoopFactory: Send + Sync {
    /// Create a default validation loop service.
    ///
    /// Builds with the provided planning pipeline, failure parser,
    /// and quality gate evaluation service. Uses default ValidationLoopConfig.
    ///
    /// # Arguments
    /// * `planning_pipeline` — The planning pipeline for plan→execute.
    /// * `failure_parser` — The failure parser for parsing errors.
    /// * `quality_gate` — The quality gate evaluation service.
    async fn create_default(
        &self,
        planning_pipeline: Box<dyn PlanningPipelineService>,
        failure_parser: Box<dyn FailureParserService>,
        quality_gate: Box<dyn QualityGateService>,
    ) -> Result<Box<dyn ValidationLoopService>, ValidationLoopError>;

    /// Create a validation loop service with custom config and dependencies.
    ///
    /// # Arguments
    /// * `config` — Custom validation loop configuration.
    /// * `planning_pipeline` — The planning pipeline.
    /// * `failure_parser` — The failure parser.
    /// * `quality_gate` — The quality gate evaluation service.
    async fn create_custom(
        &self,
        config: ValidationLoopConfig,
        planning_pipeline: Box<dyn PlanningPipelineService>,
        failure_parser: Box<dyn FailureParserService>,
        quality_gate: Box<dyn QualityGateService>,
    ) -> Result<Box<dyn ValidationLoopService>, ValidationLoopError>;
}

/// Preset factory for `ValidationLoopConfig` instances.
///
/// Provides static methods for creating configs with common presets
/// (development, production, testing).
///
/// # Contract (Frozen)
/// - `development` — Loose config for active development (5 iterations, Workspace quality)
/// - `production` — Standard config for production use (3 iterations, Package)
/// - `testing` — Test-friendly config (2 iterations, TargetedTests)
pub struct ValidationLoopConfigPresets;

impl ValidationLoopConfigPresets {
    /// Create a development config with relaxed constraints.
    ///
    /// Allows up to 5 iterations with workspace-level quality for
    /// active development and debugging.
    pub fn development() -> ValidationLoopConfig {
        ValidationLoopConfig {
            max_iterations: 5,
            required_quality: crate::quality_gates::domain::QualityLevel::Workspace,
            max_cumulative_tokens: 100_000,
            cache_successful_templates: false,
        }
    }

    /// Create a production config with standard constraints.
    pub fn production() -> ValidationLoopConfig {
        ValidationLoopConfig::default()
    }

    /// Create a test-friendly config with minimal constraints.
    pub fn testing() -> ValidationLoopConfig {
        ValidationLoopConfig {
            max_iterations: 2,
            required_quality: crate::quality_gates::domain::QualityLevel::TargetedTests,
            max_cumulative_tokens: 10_000,
            cache_successful_templates: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for constructing a ValidationLoopConfig with fluent API.
///
/// # Example
///
/// ```rust,ignore
/// let config = ValidationLoopConfigBuilder::new()
///     .max_iterations(5)
///     .required_quality(QualityLevel::Workspace)
///     .max_cumulative_tokens(100_000)
///     .cache_successful_templates(true)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct ValidationLoopConfigBuilder {
    max_iterations: u32,
    required_quality: crate::quality_gates::domain::QualityLevel,
    max_cumulative_tokens: u64,
    cache_successful_templates: bool,
}

impl Default for ValidationLoopConfigBuilder {
    fn default() -> Self {
        Self {
            max_iterations: 3,
            required_quality: crate::quality_gates::domain::QualityLevel::Package,
            max_cumulative_tokens: 50_000,
            cache_successful_templates: true,
        }
    }
}

impl ValidationLoopConfigBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum iterations.
    pub fn max_iterations(mut self, val: u32) -> Self {
        self.max_iterations = val;
        self
    }

    /// Set required quality level.
    pub fn required_quality(mut self, val: crate::quality_gates::domain::QualityLevel) -> Self {
        self.required_quality = val;
        self
    }

    /// Set maximum cumulative tokens.
    pub fn max_cumulative_tokens(mut self, val: u64) -> Self {
        self.max_cumulative_tokens = val;
        self
    }

    /// Set cache successful templates flag.
    pub fn cache_successful_templates(mut self, val: bool) -> Self {
        self.cache_successful_templates = val;
        self
    }

    /// Build the ValidationLoopConfig.
    pub fn build(self) -> ValidationLoopConfig {
        ValidationLoopConfig {
            max_iterations: self.max_iterations,
            required_quality: self.required_quality,
            max_cumulative_tokens: self.max_cumulative_tokens,
            cache_successful_templates: self.cache_successful_templates,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quality_gates::domain::QualityLevel;

    #[test]
    fn test_development_config() {
        let config = ValidationLoopConfigPresets::development();
        assert_eq!(config.max_iterations, 5);
        assert_eq!(config.required_quality, QualityLevel::Workspace);
        assert_eq!(config.max_cumulative_tokens, 100_000);
        assert!(!config.cache_successful_templates);
    }

    #[test]
    fn test_production_config() {
        let config = ValidationLoopConfigPresets::production();
        assert_eq!(config.max_iterations, 3);
        assert_eq!(config.required_quality, QualityLevel::Package);
    }

    #[test]
    fn test_testing_config() {
        let config = ValidationLoopConfigPresets::testing();
        assert_eq!(config.max_iterations, 2);
        assert_eq!(config.required_quality, QualityLevel::TargetedTests);
        assert_eq!(config.max_cumulative_tokens, 10_000);
    }

    #[test]
    fn test_builder_default() {
        let config = ValidationLoopConfigBuilder::new().build();
        assert_eq!(config.max_iterations, 3);
    }

    #[test]
    fn test_builder_custom() {
        let config = ValidationLoopConfigBuilder::new()
            .max_iterations(5)
            .required_quality(QualityLevel::Workspace)
            .max_cumulative_tokens(100_000)
            .cache_successful_templates(false)
            .build();

        assert_eq!(config.max_iterations, 5);
        assert_eq!(config.required_quality, QualityLevel::Workspace);
        assert_eq!(config.max_cumulative_tokens, 100_000);
        assert!(!config.cache_successful_templates);
    }

    #[test]
    fn test_builder_chaining() {
        let config = ValidationLoopConfigBuilder::new()
            .max_iterations(2)
            .cache_successful_templates(true)
            .build();

        assert_eq!(config.max_iterations, 2);
        assert!(config.cache_successful_templates);
    }
}

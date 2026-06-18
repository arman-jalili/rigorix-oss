//! Implementation of the PlanningPipelineFactory.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#pipeline
//! Implements: PlanningPipeline — PlanningPipelineFactory implementation
//! Issue: issue-planningpipeline
//!
//! Provides the concrete `PlanningPipelineFactoryImpl` that constructs
//! `PlanningPipelineService` instances with injected dependencies.

use async_trait::async_trait;
use uuid::Uuid;

use crate::planning::application::factory::{CompositeValidator, PlanningPipelineFactory};
use crate::planning::application::pipeline_impl::PlanningPipelineImpl;
use crate::planning::application::service::PlanningPipelineService;
use crate::planning::domain::classification::Classifier;
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::extractor::ParameterExtractor;
use crate::template_generation::domain::TemplateGenerator;

/// Factory for constructing PlanningPipelineService instances.
///
/// Handles creation of the pipeline with appropriate dependencies
/// and defaults. All constructors generate a new execution ID
/// for each call unless one is explicitly provided.
pub struct PlanningPipelineFactoryImpl;

impl PlanningPipelineFactoryImpl {
    /// Create a new factory instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlanningPipelineFactoryImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlanningPipelineFactory for PlanningPipelineFactoryImpl {
    async fn create_default(
        &self,
        classifier: Box<dyn Classifier>,
        extractor: Box<dyn ParameterExtractor>,
        template_service: std::sync::Arc<
            dyn crate::templates::application::service::TemplateEngineService,
        >,
    ) -> Result<Box<dyn PlanningPipelineService>, PlanningError> {
        let execution_id = Uuid::new_v4();
        let pipeline =
            PlanningPipelineImpl::new(execution_id, classifier, extractor, template_service);
        Ok(Box::new(pipeline))
    }

    async fn create_with_generator(
        &self,
        classifier: Box<dyn Classifier>,
        extractor: Box<dyn ParameterExtractor>,
        template_service: std::sync::Arc<
            dyn crate::templates::application::service::TemplateEngineService,
        >,
        template_generator: Box<dyn TemplateGenerator>,
    ) -> Result<Box<dyn PlanningPipelineService>, PlanningError> {
        let execution_id = Uuid::new_v4();
        let pipeline =
            PlanningPipelineImpl::new(execution_id, classifier, extractor, template_service)
                .with_generator(template_generator);
        Ok(Box::new(pipeline))
    }

    async fn create_custom(
        &self,
        classifier: Box<dyn Classifier>,
        extractor: Box<dyn ParameterExtractor>,
        template_service: std::sync::Arc<
            dyn crate::templates::application::service::TemplateEngineService,
        >,
        template_generator: Option<Box<dyn TemplateGenerator>>,
        validator: Option<Box<dyn CompositeValidator>>,
    ) -> Result<Box<dyn PlanningPipelineService>, PlanningError> {
        let execution_id = Uuid::new_v4();
        let mut pipeline =
            PlanningPipelineImpl::new(execution_id, classifier, extractor, template_service);

        if let Some(generator) = template_generator {
            pipeline = pipeline.with_generator(generator);
        }

        if let Some(validator) = validator {
            pipeline = pipeline.with_validator(validator);
        }

        Ok(Box::new(pipeline))
    }
}

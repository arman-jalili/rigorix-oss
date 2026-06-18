//! Implementation of TemplateGenerationFactory.
//!
//! @canonical .pi/architecture/modules/template-generation.md#factory-impl
//! Implements: TemplateGenerationFactory — creates TemplateGenerationService

use async_trait::async_trait;
use std::sync::Arc;

use crate::template_generation::application::factory::TemplateGenerationFactory;
use crate::template_generation::application::generation_service_impl::TemplateGenerationServiceImpl;
use crate::template_generation::application::service::TemplateGenerationService;
use crate::template_generation::domain::GeneratorError;
use crate::template_generation::domain::{ClaudeGeneratorConfig, ClaudeTemplateGenerator};

/// Factory for creating TemplateGenerationService instances.
pub struct TemplateGenerationFactoryImpl;

#[async_trait]
impl TemplateGenerationFactory for TemplateGenerationFactoryImpl {
    async fn create(&self) -> Result<Box<dyn TemplateGenerationService>, GeneratorError> {
        let api_key = std::env::var("CLAUDE_API_KEY").unwrap_or_else(|_| {
            // In test/dev mode, use a placeholder
            "placeholder-key".to_string()
        });

        let config = ClaudeGeneratorConfig::default();
        let generator = ClaudeTemplateGenerator::new(api_key, Some(config));
        let service = TemplateGenerationServiceImpl::new(Arc::new(generator));
        Ok(Box::new(service))
    }
}

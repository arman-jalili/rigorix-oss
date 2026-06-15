//! Factory interfaces for constructing Template Generation service instances.
//!
//! @canonical .pi/architecture/modules/template-generation.md#factories
//! Issue: issue-contract-freeze

use async_trait::async_trait;

use crate::template_generation::domain::GeneratorError;

use super::service::TemplateGenerationService;

/// Factory for constructing TemplateGenerationService instances.
#[async_trait]
pub trait TemplateGenerationFactory: Send + Sync {
    /// Create a TemplateGenerationService instance.
    async fn create(&self) -> Result<Box<dyn TemplateGenerationService>, GeneratorError>;
}

//! Repository interfaces for the CLI Template Generation module.
//!
//! @canonical .pi/architecture/modules/template-generation.md
//! Implements: Contract Freeze — TemplateGenerationRepository trait
//! Issue: issue-contract-freeze

use async_trait::async_trait;

use crate::template_generation::domain::GenerationCliError;

#[async_trait]
pub trait TemplateGenerationRepository: Send + Sync {
    async fn store_generated_template(
        &self,
        template_id: &str,
        content: &str,
    ) -> Result<(), GenerationCliError>;

    async fn get_generated_template(
        &self,
        template_id: &str,
    ) -> Result<Option<String>, GenerationCliError>;

    async fn list_generated_templates(&self) -> Result<Vec<String>, GenerationCliError>;

    async fn delete_generated_template(
        &self,
        template_id: &str,
    ) -> Result<bool, GenerationCliError>;

    async fn clear(&self) -> Result<(), GenerationCliError>;
}

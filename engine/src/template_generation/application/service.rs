//! Service interfaces for the Template Generation module.
//!
//! @canonical .pi/architecture/modules/template-generation.md#services
//! Issue: issue-contract-freeze

use async_trait::async_trait;

use crate::template_generation::domain::{GeneratedTemplate, GeneratorError, RepoContext};

use super::dto::{
    GenerateCostInput, GenerateCostOutput, GenerateTemplateInput, GenerateTemplateOutput,
    SymbolValidationInput, SymbolValidationOutput,
};

/// Service for generating templates from user intent via LLM.
#[async_trait]
pub trait TemplateGenerationService: Send + Sync {
    /// Generate a new template from user intent.
    async fn generate_template(
        &self,
        input: GenerateTemplateInput,
    ) -> Result<GenerateTemplateOutput, GeneratorError>;
}

/// Service for Phase 3 symbol validation.
#[async_trait]
pub trait SymbolValidationService: Send + Sync {
    /// Validate a template against the symbol graph.
    async fn validate_symbols(
        &self,
        input: SymbolValidationInput,
    ) -> Result<SymbolValidationOutput, GeneratorError>;
}

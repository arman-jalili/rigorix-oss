//! Implementation of TemplateGenerationService.
//!
//! @canonical .pi/architecture/modules/template-generation.md#impl
//! Implements: TemplateGenerationService — generate_template via Claude

use async_trait::async_trait;

use crate::budget_tracking::domain::LlmBudget;
use crate::planning::domain::intent::UserIntent;
use crate::template_generation::domain::TemplateGenerator;
use crate::template_generation::application::dto::{
    GenerateTemplateInput, GenerateTemplateOutput,
};
use crate::template_generation::application::service::TemplateGenerationService;
use crate::template_generation::domain::GeneratorError;
use std::sync::Arc;

/// Implementation of TemplateGenerationService using Claude.
pub struct TemplateGenerationServiceImpl {
    generator: Arc<dyn TemplateGenerator>,
}

impl TemplateGenerationServiceImpl {
    pub fn new(generator: Arc<dyn TemplateGenerator>) -> Self {
        Self { generator }
    }
}

#[async_trait]
impl TemplateGenerationService for TemplateGenerationServiceImpl {
    async fn generate_template(
        &self,
        input: GenerateTemplateInput,
    ) -> Result<GenerateTemplateOutput, GeneratorError> {
        let budget = LlmBudget {
            max_calls: 10,
            max_tokens: 50_000,
            used_calls: 0,
            used_tokens: 0,
            label: "template-generation".to_string(),
        };

        let intent = UserIntent::new(input.intent.clone(), Some(input.session_id));

        let generated = self
            .generator
            .generate(&intent, &input.repo_context, &budget)
            .await?;

        Ok(GenerateTemplateOutput {
            generated,
            symbol_validation_passed: true,
            symbol_validation_errors: vec![],
        })
    }
}

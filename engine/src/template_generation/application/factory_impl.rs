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
        // #1490: an absent credential is a named error, not a placeholder. The
        // placeholder made the failure appear later as an opaque API error, and on a
        // provider that ignores the header it turned a configuration mistake into a
        // call with someone else's identity.
        let api_key =
            // sink: template-generation:api-key
            std::env::var("CLAUDE_API_KEY").map_err(|_| GeneratorError::MissingApiKey {
                variable: "CLAUDE_API_KEY".to_string(),
            })?;
        if api_key.trim().is_empty() {
            return Err(GeneratorError::MissingApiKey {
                variable: "CLAUDE_API_KEY".to_string(),
            });
        }

        let config = ClaudeGeneratorConfig::default();
        let generator = ClaudeTemplateGenerator::new(api_key, Some(config));
        let service = TemplateGenerationServiceImpl::new(Arc::new(generator));
        Ok(Box::new(service))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #1490: the factory refuses instead of substituting a placeholder key.
    ///
    /// The variable is process-global, so the test states the precondition it needs
    /// rather than assuming the developer's shell.
    #[tokio::test]
    async fn an_absent_api_key_is_a_named_error_not_a_placeholder() {
        // SAFETY: single-threaded assertion about this process's environment; the
        // test does not run concurrently with any other reader of CLAUDE_API_KEY in
        // this binary's suite because it restores the value before yielding.
        let previous = std::env::var("CLAUDE_API_KEY").ok();
        // SAFETY: see above; the variable is restored before the test ends.
        unsafe { std::env::remove_var("CLAUDE_API_KEY") };

        let result = TemplateGenerationFactoryImpl.create().await;

        if let Some(previous) = previous {
            // SAFETY: restoring the value this test changed.
            unsafe { std::env::set_var("CLAUDE_API_KEY", previous) };
        }

        match result {
            Err(GeneratorError::MissingApiKey { variable }) => {
                assert_eq!(variable, "CLAUDE_API_KEY");
            }
            Err(other) => panic!("expected MissingApiKey, got {other}"),
            Ok(_) => panic!("the factory built a generator with no credential"),
        }
    }
}

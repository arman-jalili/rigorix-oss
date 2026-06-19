//! Factory implementations for constructing LLM Step service instances.
//!
//! @canonical .pi/architecture/modules/llm-step.md
//! Implements: LlmGenerateNode — LlmStepFactoryImpl, LlmContextBuilderFactoryImpl
//! Issue: issue-llmgeneratenode
//!
//! Concrete factory implementations that wire up service instances with
//! configuration settings.

use async_trait::async_trait;

use crate::llm_step::application::factory::{
    LlmContextBuilderFactory, LlmContextBuilderFactoryConfig, LlmProviderClient,
    LlmProviderClientFactory, LlmProviderConfig, LlmStepFactory, LlmStepFactoryConfig,
};
use crate::llm_step::application::service::{LlmContextBuilderService, LlmStepService};
use crate::llm_step::application::service_impl::{
    LlmContextBuilderServiceImpl, LlmStepServiceImpl,
};
use crate::llm_step::domain::LlmStepError;
use crate::llm_step::infrastructure::llm_provider_client_impl::{
    AnthropicProviderClient, MockLlmProviderClient, OpenAiProviderClient,
};

/// Factory implementation for constructing `LlmStepService` instances.
///
/// Creates LlmStepServiceImpl instances with the given configuration,
/// wiring in a provider client for LLM API calls.
pub struct LlmStepFactoryImpl;

impl LlmStepFactoryImpl {
    /// Create a new LlmStepFactoryImpl.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LlmStepFactoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmStepFactory for LlmStepFactoryImpl {
    async fn create(
        &self,
        config: LlmStepFactoryConfig,
    ) -> Result<Box<dyn LlmStepService>, LlmStepError> {
        // Create a provider client based on config
        let provider_client = create_provider_client(&config.default_provider)?;

        // Create a default context builder
        let context_builder = LlmContextBuilderFactoryImpl::new()
            .create(LlmContextBuilderFactoryConfig::default())
            .await?;

        let service = LlmStepServiceImpl::new(
            provider_client,
            context_builder,
            config.max_retries,
            config.default_timeout_secs,
            config.validate_before_execution,
        );
        Ok(Box::new(service))
    }
}

/// Factory implementation for constructing `LlmContextBuilderService` instances.
///
/// Creates LlmContextBuilderServiceImpl instances for context assembly.
pub struct LlmContextBuilderFactoryImpl;

impl LlmContextBuilderFactoryImpl {
    /// Create a new LlmContextBuilderFactoryImpl.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LlmContextBuilderFactoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmContextBuilderFactory for LlmContextBuilderFactoryImpl {
    async fn create(
        &self,
        _config: LlmContextBuilderFactoryConfig,
    ) -> Result<Box<dyn LlmContextBuilderService>, LlmStepError> {
        let service = LlmContextBuilderServiceImpl::new();
        Ok(Box::new(service))
    }
}

/// Factory implementation for constructing `LlmProviderClient` instances.
///
/// Creates provider-specific clients (Anthropic, OpenAI, Mock) based on
/// the provider name in the configuration.
pub struct LlmProviderClientFactoryImpl;

impl LlmProviderClientFactoryImpl {
    /// Create a new LlmProviderClientFactoryImpl.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LlmProviderClientFactoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProviderClientFactory for LlmProviderClientFactoryImpl {
    async fn create_client(
        &self,
        config: LlmProviderConfig,
    ) -> Result<Box<dyn LlmProviderClient>, LlmStepError> {
        create_provider_client(&config)
    }
}

/// Create a provider client based on the provider name.
fn create_provider_client(
    config: &LlmProviderConfig,
) -> Result<Box<dyn LlmProviderClient>, LlmStepError> {
    match config.provider_name.to_lowercase().as_str() {
        "anthropic" => Ok(Box::new(AnthropicProviderClient::new(
            config.api_url.clone(),
            String::new(), // API key should be injected at runtime
            config.max_tokens as u64,
        ))),
        "openai" => Ok(Box::new(OpenAiProviderClient::new(
            config.api_url.clone(),
            String::new(), // API key should be injected at runtime
            config.max_tokens as u64,
        ))),
        "mock" => Ok(Box::new(MockLlmProviderClient::default())),
        other => Err(LlmStepError::UnsupportedModel {
            model: other.to_string(),
            available_models: vec![
                "anthropic".to_string(),
                "openai".to_string(),
                "mock".to_string(),
            ],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_llm_step_factory_create() {
        let factory = LlmStepFactoryImpl::new();
        let config = LlmStepFactoryConfig::default();

        let result = factory.create(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_context_builder_factory_create() {
        let factory = LlmContextBuilderFactoryImpl::new();
        let config = LlmContextBuilderFactoryConfig::default();

        let result = factory.create(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_provider_client_factory_mock() {
        let factory = LlmProviderClientFactoryImpl::new();
        let config = LlmProviderConfig {
            provider_name: "mock".to_string(),
            ..LlmProviderConfig::default()
        };

        let result = factory.create_client(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_provider_client_factory_unsupported() {
        let factory = LlmProviderClientFactoryImpl::new();
        let config = LlmProviderConfig {
            provider_name: "nonexistent".to_string(),
            ..LlmProviderConfig::default()
        };

        let result = factory.create_client(config).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmStepError::UnsupportedModel { model, .. } => {
                assert_eq!(model, "nonexistent");
            }
            _ => panic!("Expected UnsupportedModel error"),
        }
    }
}

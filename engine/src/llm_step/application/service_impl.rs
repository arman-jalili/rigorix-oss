//! Service implementation for the LLM Step bounded context.
//!
//! @canonical .pi/architecture/modules/llm-step.md
//! Implements: LlmGenerateNode — LlmStepServiceImpl, LlmContextBuilderServiceImpl
//! Issue: issue-llmgeneratenode
//!
//! Concrete implementations of LlmStepService and LlmContextBuilderService
//! that manage node lifecycle, configuration validation, and context
//! assembly for LLM generation.

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use crate::llm_step::application::factory::{
    LlmProviderClient, LlmProviderRequest, LlmProviderResponse,
};
use crate::llm_step::domain::{
    FailureContext, LlmGenerateNode, LlmGenerateNodeState, LlmGenerationOutput, LlmModelConfig,
    LlmOutputFormat, LlmOutputSchema, LlmStepContext, LlmStepError, SourceContext,
};

use super::dto::{
    BuildContextInput, BuildContextOutput, CreateNodeInput, CreateNodeOutput, ExecuteStepInput,
    ExecuteStepOutput, GenerateInput, GenerateOutput, GetFailureContextInput,
    GetFailureContextOutput, GetSourceContextInput, GetSourceContextOutput, RetryGenerationInput,
    RetryGenerationOutput, ValidateNodeConfigInput, ValidateNodeConfigOutput,
};
use super::service::{LlmContextBuilderService, LlmStepService};

/// In-memory implementation of LlmStepService.
///
/// Manages LlmGenerateNode lifecycle, configuration validation, and
/// delegates LLM calls to the configured LlmProviderClient.
pub struct LlmStepServiceImpl {
    /// The LLM provider client used for generation.
    provider_client: Box<dyn LlmProviderClient>,
    /// Maximum retries for transient failures.
    max_retries: u8,
    /// Default timeout for LLM calls in seconds.
    default_timeout_secs: u64,
    /// Whether to validate configurations before execution.
    validate_before_execution: bool,
}

impl std::fmt::Debug for LlmStepServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmStepServiceImpl")
            .field("provider_client", &self.provider_client)
            .field("max_retries", &self.max_retries)
            .field("default_timeout_secs", &self.default_timeout_secs)
            .field("validate_before_execution", &self.validate_before_execution)
            .finish()
    }
}

impl LlmStepServiceImpl {
    /// Create a new LlmStepServiceImpl with the given provider client.
    pub fn new(
        provider_client: Box<dyn LlmProviderClient>,
        max_retries: u8,
        default_timeout_secs: u64,
        validate_before_execution: bool,
    ) -> Self {
        Self {
            provider_client,
            max_retries,
            default_timeout_secs,
            validate_before_execution,
        }
    }

    /// Validate LlmModelConfig before creating a node.
    fn validate_model_config(&self, config: &LlmModelConfig) -> Result<(), LlmStepError> {
        if config.model.is_empty() {
            return Err(LlmStepError::InvalidConfiguration {
                message: "Model identifier must not be empty".to_string(),
                field: "model".to_string(),
            });
        }
        if config.provider.is_empty() {
            return Err(LlmStepError::InvalidConfiguration {
                message: "Provider name must not be empty".to_string(),
                field: "provider".to_string(),
            });
        }
        if config.max_tokens == 0 {
            return Err(LlmStepError::InvalidConfiguration {
                message: "max_tokens must be greater than 0".to_string(),
                field: "max_tokens".to_string(),
            });
        }
        if !(0.0..=2.0).contains(&config.temperature) {
            return Err(LlmStepError::InvalidConfiguration {
                message: "temperature must be between 0.0 and 2.0".to_string(),
                field: "temperature".to_string(),
            });
        }
        if !(0.0..=1.0).contains(&config.top_p) {
            return Err(LlmStepError::InvalidConfiguration {
                message: "top_p must be between 0.0 and 1.0".to_string(),
                field: "top_p".to_string(),
            });
        }
        if config.timeout_secs == 0 {
            return Err(LlmStepError::InvalidConfiguration {
                message: "timeout_secs must be greater than 0".to_string(),
                field: "timeout_secs".to_string(),
            });
        }
        Ok(())
    }

    /// Validate prompt template for required placeholders and structure.
    fn validate_prompt_template(&self, template: &str) -> Result<(), LlmStepError> {
        if template.is_empty() {
            return Err(LlmStepError::InvalidConfiguration {
                message: "Prompt template must not be empty".to_string(),
                field: "prompt_template".to_string(),
            });
        }
        Ok(())
    }

    /// Validate output schema.
    fn validate_output_schema(&self, schema: &LlmOutputSchema) -> Result<(), LlmStepError> {
        if schema.schema.is_empty() {
            return Err(LlmStepError::InvalidConfiguration {
                message: "Output schema description must not be empty".to_string(),
                field: "output_schema.schema".to_string(),
            });
        }
        Ok(())
    }

    /// Estimate token cost based on prompt template and config.
    fn estimate_token_cost(&self, node: &LlmGenerateNode) -> u32 {
        // Rough estimation: ~4 chars per token for the prompt template
        let prompt_chars = node.prompt_template.len();
        let estimated_prompt_tokens = (prompt_chars / 4).max(1) as u32;
        estimated_prompt_tokens + node.model_config.max_tokens
    }

    /// Execute the actual LLM call.
    async fn do_generate(&self, request: LlmProviderRequest) -> Result<LlmProviderResponse, LlmStepError> {
        self.provider_client.generate(request).await
    }
}

#[async_trait]
impl LlmStepService for LlmStepServiceImpl {
    async fn create_node(&self, input: CreateNodeInput) -> Result<CreateNodeOutput, LlmStepError> {
        // Validate configuration
        self.validate_model_config(&input.model_config)?;
        self.validate_prompt_template(&input.prompt_template)?;
        self.validate_output_schema(&input.output_schema)?;

        let now = Utc::now();
        let node = LlmGenerateNode {
            id: Uuid::new_v4(),
            name: input.name,
            model_config: input.model_config,
            prompt_template: input.prompt_template,
            output_schema: input.output_schema,
            state: LlmGenerateNodeState::Created,
            output: None,
            error: None,
            created_at: now,
            started_at: None,
            completed_at: None,
        };

        Ok(CreateNodeOutput {
            created_at: now,
            node,
        })
    }

    async fn build_context(
        &self,
        _input: BuildContextInput,
    ) -> Result<BuildContextOutput, LlmStepError> {
        // Basic context assembly — full implementation with repo engine
        // integration is in LlmContextBuilderService (issue-llmstepcontext)
        Err(LlmStepError::ContextBuildFailed {
            message: "Full context builder not yet wired".to_string(),
            context_source: "LlmStepServiceImpl".to_string(),
        })
    }

    async fn execute_step(
        &self,
        input: ExecuteStepInput,
    ) -> Result<ExecuteStepOutput, LlmStepError> {
        let start = std::time::Instant::now();
        let context_start = start;

        // Build a minimal context from the input
        let context = LlmStepContext {
            node_id: input.node.id,
            execution_id: input.execution_id,
            source_context: SourceContext::default(),
            failure_context: None,
            execution_context: crate::llm_step::domain::ExecutionContext::default(),
            assembled_at: Utc::now(),
            assembled_prompt: input.node.prompt_template.clone(),
        };
        let context_duration = context_start.elapsed().as_millis() as u64;

        // Execute the generation
        let gen_start = std::time::Instant::now();
        let generate_input = GenerateInput {
            node: input.node.clone(),
            context: context.clone(),
            api_key: input.api_key,
        };
        let gen_output = self.generate(generate_input).await?;
        let generation_duration = gen_start.elapsed().as_millis() as u64;

        let total_duration = start.elapsed().as_millis() as u64;

        let total_tokens = gen_output.output.total_tokens;
        let output = gen_output.output;

        Ok(ExecuteStepOutput {
            node_id: input.node.id,
            output,
            context,
            total_duration_ms: total_duration,
            context_duration_ms: context_duration,
            generation_duration_ms: generation_duration,
            total_tokens_used: total_tokens,
            is_retry: false,
            retry_attempt: 0,
            completed_at: Utc::now(),
        })
    }

    async fn generate(&self, input: GenerateInput) -> Result<GenerateOutput, LlmStepError> {
        let start = std::time::Instant::now();

        // Validate before execution if configured
        if self.validate_before_execution {
            let validate_input = ValidateNodeConfigInput {
                node: input.node.clone(),
            };
            let validation = self.validate_node_config(validate_input).await?;
            if !validation.is_valid {
                return Err(LlmStepError::InvalidConfiguration {
                    message: format!("Node config validation failed: {:?}", validation.errors),
                    field: "node".to_string(),
                });
            }
        }

        let timeout = input
            .node
            .model_config
            .timeout_secs
            .max(self.default_timeout_secs);

        let request = LlmProviderRequest {
            model: input.node.model_config.model.clone(),
            system_prompt: "You are generating code for a template step in a DAG execution engine."
                .to_string(),
            user_message: input.context.assembled_prompt.clone(),
            max_tokens: input.node.model_config.max_tokens,
            temperature: input.node.model_config.temperature,
            top_p: input.node.model_config.top_p,
            timeout_secs: timeout,
        };

        let response = self.do_generate(request).await?;
        let duration = start.elapsed().as_millis() as u64;

        // Parse the output
        let parsed_output = match input.node.output_schema.format {
            LlmOutputFormat::Json => serde_json::from_str::<serde_json::Value>(&response.content)
                .unwrap_or(serde_json::Value::String(response.content.clone())),
            _ => serde_json::Value::String(response.content.clone()),
        };

        let output = LlmGenerationOutput {
            raw_output: response.content.clone(),
            parsed_output,
            total_tokens: response.prompt_tokens + response.completion_tokens,
            prompt_tokens: response.prompt_tokens,
            completion_tokens: response.completion_tokens,
            model_used: response.model.clone(),
            generated_at: Utc::now(),
            provider_metadata: response.provider_metadata.clone(),
        };

        Ok(GenerateOutput {
            node_id: input.node.id,
            output,
            duration_ms: duration,
            generated_at: Utc::now(),
        })
    }

    async fn retry_generation(
        &self,
        input: RetryGenerationInput,
    ) -> Result<RetryGenerationOutput, LlmStepError> {
        if input.attempt > self.max_retries {
            return Err(LlmStepError::ProviderError {
                provider: "retry".to_string(),
                status: 0,
                message: format!(
                    "Max retries ({}) exhausted on attempt {}",
                    self.max_retries, input.attempt
                ),
            });
        }

        let start = std::time::Instant::now();

        // Augment the prompt with failure context
        let failure_info = format!(
            "\n\n=== Previous Attempt (Attempt #{}) ===\n\
             Failure Type: {}\n\
             Error: {}\n\
             Error Output:\n{}\n\
             Strategy: {}\n\
             {}",
            input.attempt,
            input.updated_failure_context.failure_type,
            input.updated_failure_context.error_message,
            input.updated_failure_context.error_output,
            input.updated_failure_context.strategy,
            input
                .updated_failure_context
                .scenario_context
                .clone()
                .unwrap_or_default()
        );

        let augmented_prompt = format!(
            "{}\n\n{}",
            input.context.assembled_prompt, failure_info
        );

        let timeout = input
            .node
            .model_config
            .timeout_secs
            .max(self.default_timeout_secs);

        let request = LlmProviderRequest {
            model: input.node.model_config.model.clone(),
            system_prompt: "You are generating code for a template step. \
                           Your previous attempt failed. Review the failure analysis \
                           below and correct the issues in your new output."
                .to_string(),
            user_message: augmented_prompt,
            max_tokens: input.node.model_config.max_tokens,
            temperature: input.node.model_config.temperature,
            top_p: input.node.model_config.top_p,
            timeout_secs: timeout,
        };

        let response = self.do_generate(request).await?;
        let duration = start.elapsed().as_millis() as u64;

        Ok(RetryGenerationOutput {
            output: LlmGenerationOutput {
                raw_output: response.content,
                parsed_output: serde_json::Value::Null,
                total_tokens: response.prompt_tokens + response.completion_tokens,
                prompt_tokens: response.prompt_tokens,
                completion_tokens: response.completion_tokens,
                model_used: response.model,
                generated_at: Utc::now(),
                provider_metadata: response.provider_metadata,
            },
            duration_ms: duration,
            generated_at: Utc::now(),
        })
    }

    async fn validate_node_config(
        &self,
        input: ValidateNodeConfigInput,
    ) -> Result<ValidateNodeConfigOutput, LlmStepError> {
        let mut errors: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        // Validate model config
        if input.node.model_config.model.is_empty() {
            errors.push("Model identifier must not be empty".to_string());
        }
        if input.node.model_config.provider.is_empty() {
            errors.push("Provider name must not be empty".to_string());
        }
        if input.node.model_config.max_tokens == 0 {
            errors.push("max_tokens must be greater than 0".to_string());
        }
        if !(0.0..=2.0).contains(&input.node.model_config.temperature) {
            warnings.push("temperature outside typical range (0.0-2.0)".to_string());
        }
        if input.node.model_config.timeout_secs == 0 {
            errors.push("timeout_secs must be greater than 0".to_string());
        }

        // Validate prompt template
        if input.node.prompt_template.is_empty() {
            errors.push("Prompt template must not be empty".to_string());
        }
        if !input.node.prompt_template.contains("{{")
            && !input.node.prompt_template.contains('{')
        {
            warnings.push(
                "Prompt template has no placeholders — context will not be injected".to_string(),
            );
        }

        // Validate output schema
        if input.node.output_schema.schema.is_empty() {
            errors.push("Output schema must not be empty".to_string());
        }

        let estimated_cost = Some(self.estimate_token_cost(&input.node));

        Ok(ValidateNodeConfigOutput {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            estimated_token_cost: estimated_cost,
        })
    }
}

/// Basic implementation of LlmContextBuilderService.
///
/// Provides minimal context assembly. Full integration with the
/// repo engine and failure classification module is implemented
/// in issue-llmstepcontext.
pub struct LlmContextBuilderServiceImpl;

impl LlmContextBuilderServiceImpl {
    /// Create a new LlmContextBuilderServiceImpl.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LlmContextBuilderServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmContextBuilderService for LlmContextBuilderServiceImpl {
    async fn get_source_context(
        &self,
        _input: GetSourceContextInput,
    ) -> Result<GetSourceContextOutput, LlmStepError> {
        Err(LlmStepError::ContextBuildFailed {
            message: "Repo engine integration not yet wired. Use issue-llmstepcontext.".to_string(),
            context_source: "LlmContextBuilderServiceImpl".to_string(),
        })
    }

    async fn get_failure_context(
        &self,
        _input: GetFailureContextInput,
    ) -> Result<GetFailureContextOutput, LlmStepError> {
        Err(LlmStepError::ContextBuildFailed {
            message: "Failure classification integration not yet wired. Use issue-llmstepcontext."
                .to_string(),
            context_source: "LlmContextBuilderServiceImpl".to_string(),
        })
    }

    async fn assemble_prompt(
        &self,
        template: String,
        _source_context: SourceContext,
        _failure_context: Option<FailureContext>,
    ) -> Result<String, LlmStepError> {
        // Basic prompt assembly — just returns the template
        Ok(template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm_step::infrastructure::llm_provider_client_impl::MockLlmProviderClient;

    fn create_test_service() -> LlmStepServiceImpl {
        let provider = MockLlmProviderClient::default();
        LlmStepServiceImpl::new(Box::new(provider), 3, 120, true)
    }

    fn create_test_input() -> CreateNodeInput {
        CreateNodeInput {
            name: "test-node".to_string(),
            model_config: LlmModelConfig::default(),
            prompt_template: "Generate a {{test}} for the given context.".to_string(),
            output_schema: LlmOutputSchema {
                format: LlmOutputFormat::Text,
                schema: "Generated text output".to_string(),
                strict: false,
            },
        }
    }

    #[tokio::test]
    async fn test_create_node_success() {
        let service = create_test_service();
        let input = create_test_input();

        let result = service.create_node(input).await.unwrap();
        assert_eq!(result.node.name, "test-node");
        assert_eq!(result.node.state, LlmGenerateNodeState::Created);
        assert!(result.node.output.is_none());
    }

    #[tokio::test]
    async fn test_create_node_empty_model() {
        let service = create_test_service();
        let mut input = create_test_input();
        input.model_config.model = String::new();

        let result = service.create_node(input).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmStepError::InvalidConfiguration { field, .. } => {
                assert_eq!(field, "model");
            }
            _ => panic!("Expected InvalidConfiguration error"),
        }
    }

    #[tokio::test]
    async fn test_create_node_empty_prompt() {
        let service = create_test_service();
        let mut input = create_test_input();
        input.prompt_template = String::new();

        let result = service.create_node(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_node_zero_max_tokens() {
        let service = create_test_service();
        let mut input = create_test_input();
        input.model_config.max_tokens = 0;

        let result = service.create_node(input).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmStepError::InvalidConfiguration { field, .. } => {
                assert_eq!(field, "max_tokens");
            }
            _ => panic!("Expected InvalidConfiguration error"),
        }
    }

    #[tokio::test]
    async fn test_generate_with_mock_provider() {
        let service = create_test_service();
        let node = service
            .create_node(create_test_input())
            .await
            .unwrap()
            .node;

        let context = LlmStepContext {
            node_id: node.id,
            execution_id: Uuid::new_v4(),
            source_context: SourceContext::default(),
            failure_context: None,
            execution_context: crate::llm_step::domain::ExecutionContext::default(),
            assembled_at: Utc::now(),
            assembled_prompt: node.prompt_template.clone(),
        };

        let input = GenerateInput {
            node,
            context,
            api_key: "test-key".to_string(),
        };

        let result = service.generate(input).await.unwrap();
        assert_eq!(result.output.raw_output, "Mock generated content");
        assert_eq!(result.output.total_tokens, 30);
    }

    #[tokio::test]
    async fn test_validate_node_config_valid() {
        let service = create_test_service();
        let node = service
            .create_node(create_test_input())
            .await
            .unwrap()
            .node;

        let result = service
            .validate_node_config(ValidateNodeConfigInput { node })
            .await
            .unwrap();
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_validate_node_config_invalid() {
        let service = create_test_service();

        // Create a node directly with invalid config (bypassing create_node validation)
        let node = LlmGenerateNode {
            id: Uuid::new_v4(),
            name: "invalid-node".to_string(),
            model_config: LlmModelConfig {
                model: String::new(),
                ..LlmModelConfig::default()
            },
            prompt_template: String::new(),
            output_schema: LlmOutputSchema {
                format: LlmOutputFormat::Text,
                schema: String::new(),
                strict: false,
            },
            state: LlmGenerateNodeState::Created,
            output: None,
            error: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        };
        let result = service
            .validate_node_config(ValidateNodeConfigInput { node })
            .await
            .unwrap();
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_retry_generation() {
        let service = create_test_service();
        let node = service
            .create_node(create_test_input())
            .await
            .unwrap()
            .node;

        let context = LlmStepContext {
            node_id: node.id,
            execution_id: Uuid::new_v4(),
            source_context: SourceContext::default(),
            failure_context: None,
            execution_context: crate::llm_step::domain::ExecutionContext::default(),
            assembled_at: Utc::now(),
            assembled_prompt: node.prompt_template.clone(),
        };

        let failure_context = FailureContext {
            failure_type: "CompileError".to_string(),
            error_message: "Missing semicolon".to_string(),
            error_output: "error: expected ';' at line 42".to_string(),
            retries_attempted: 1,
            max_retries: 3,
            strategy: "retry_with_augmented_context".to_string(),
            previous_attempts: vec![],
            scenario_context: None,
        };

        let input = RetryGenerationInput {
            node,
            context,
            attempt: 1,
            updated_failure_context: failure_context,
            api_key: "test-key".to_string(),
        };

        let result = service.retry_generation(input).await.unwrap();
        assert!(!result.output.raw_output.is_empty());
    }

    #[tokio::test]
    async fn test_retry_generation_exhausted() {
        let service = create_test_service();
        let node = service
            .create_node(create_test_input())
            .await
            .unwrap()
            .node;

        let context = LlmStepContext {
            node_id: node.id,
            execution_id: Uuid::new_v4(),
            source_context: SourceContext::default(),
            failure_context: None,
            execution_context: crate::llm_step::domain::ExecutionContext::default(),
            assembled_at: Utc::now(),
            assembled_prompt: node.prompt_template.clone(),
        };

        let failure_context = FailureContext {
            failure_type: "CompileError".to_string(),
            error_message: "error".to_string(),
            error_output: "output".to_string(),
            retries_attempted: 3,
            max_retries: 3,
            strategy: "retry".to_string(),
            previous_attempts: vec![],
            scenario_context: None,
        };

        let input = RetryGenerationInput {
            node,
            context,
            attempt: 4, // Exceeds max_retries = 3
            updated_failure_context: failure_context,
            api_key: "test-key".to_string(),
        };

        let result = service.retry_generation(input).await;
        assert!(result.is_err());
    }
}

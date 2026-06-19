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
use tracing::Instrument;
use uuid::Uuid;

use crate::llm_step::application::factory::{
    LlmProviderClient, LlmProviderRequest, LlmProviderResponse,
};
use crate::llm_step::domain::{
    ExecutionContext, FailureContext, LlmGenerateNode, LlmGenerateNodeState, LlmGenerationOutput,
    LlmModelConfig, LlmOutputFormat, LlmOutputSchema, LlmStepContext, LlmStepError,
    PreviousAttempt, SourceContext, SourceFileContext, SymbolDefinition,
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
/// delegates LLM calls to the configured LlmProviderClient. Uses
/// LlmContextBuilderService for assembling source code and failure context.
pub struct LlmStepServiceImpl {
    /// The LLM provider client used for generation.
    provider_client: Box<dyn LlmProviderClient>,
    /// The context builder service for assembling prompts.
    context_builder: Box<dyn LlmContextBuilderService>,
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
            .field("context_builder", &"...")
            .field("max_retries", &self.max_retries)
            .field("default_timeout_secs", &self.default_timeout_secs)
            .field("validate_before_execution", &self.validate_before_execution)
            .finish()
    }
}

impl LlmStepServiceImpl {
    /// Create a new LlmStepServiceImpl with the given dependencies.
    pub fn new(
        provider_client: Box<dyn LlmProviderClient>,
        context_builder: Box<dyn LlmContextBuilderService>,
        max_retries: u8,
        default_timeout_secs: u64,
        validate_before_execution: bool,
    ) -> Self {
        Self {
            provider_client,
            context_builder,
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
    #[tracing::instrument(skip_all)]
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
        input: BuildContextInput,
    ) -> Result<BuildContextOutput, LlmStepError> {
        // Gather source context
        let source_input = GetSourceContextInput {
            execution_id: input.execution_id,
            file_paths: input.source_file_paths.clone(),
            max_context_size: Some(100_000),
        };
        let source_output = self.context_builder.get_source_context(source_input).await?;

        // Gather failure context if requested
        let failure_context = if input.include_failure_context {
            let failure_input = GetFailureContextInput {
                execution_id: input.execution_id,
                node_id: input.node_id,
                max_previous_attempts: Some(5),
            };
            Some(self.context_builder.get_failure_context(failure_input).await?)
        } else {
            None
        };

        // Assemble the prompt using the context builder
        let assembled_prompt = self
            .context_builder
            .assemble_prompt(
                "{source_code}\n{execution_context}".to_string(),
                source_output.source_context.clone(),
                failure_context.as_ref().map(|f| FailureContext {
                    failure_type: f.failure_context.failure_type.clone(),
                    error_message: f.failure_context.error_message.clone(),
                    error_output: f.failure_context.error_output.clone(),
                    retries_attempted: f.failure_context.retries_attempted,
                    max_retries: f.failure_context.max_retries,
                    strategy: f.failure_context.strategy.clone(),
                    previous_attempts: f.failure_context.previous_attempts.clone(),
                    scenario_context: f.failure_context.scenario_context.clone(),
                }),
            )
            .await?;

        let context = LlmStepContext {
            node_id: input.node_id,
            execution_id: input.execution_id,
            source_context: source_output.source_context,
            failure_context: failure_context.map(|f| f.failure_context),
            execution_context: ExecutionContext {
                dag_id: input.dag_id,
                ..ExecutionContext::default()
            },
            assembled_at: Utc::now(),
            assembled_prompt,
        };

        let source_file_count = context.source_file_count() as u32;
        let symbol_count = context.source_context.symbols.len() as u32;
        let has_failure = context.has_failure_context();

        Ok(BuildContextOutput {
            context,
            source_file_count,
            symbol_count,
            has_failure_context: has_failure,
            assembled_at: Utc::now(),
        })
    }

    async fn execute_step(
        &self,
        input: ExecuteStepInput,
    ) -> Result<ExecuteStepOutput, LlmStepError> {
        let start = std::time::Instant::now();

        // Step 1: Build context using the context builder
        let context_start = std::time::Instant::now();
        let build_context_input = BuildContextInput {
            node_id: input.node.id,
            execution_id: input.execution_id,
            dag_id: input.dag_id,
            target_file_path: input.target_file_path.clone(),
            source_file_paths: input.source_file_paths.clone(),
            include_failure_context: input.include_failure_context,
        };
        let context_output = self.build_context(build_context_input).await?;
        let context_duration = context_start.elapsed().as_millis() as u64;

        // Step 2: Execute the generation
        let gen_start = std::time::Instant::now();
        let generate_input = GenerateInput {
            node: input.node.clone(),
            context: context_output.context.clone(),
            api_key: input.api_key,
        };
        let gen_output = self.generate(generate_input).await?;
        let generation_duration = gen_start.elapsed().as_millis() as u64;

        let total_duration = start.elapsed().as_millis() as u64;
        let total_tokens = gen_output.output.total_tokens;

        Ok(ExecuteStepOutput {
            node_id: input.node.id,
            output: gen_output.output,
            context: context_output.context,
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

/// Implementation of LlmContextBuilderService.
///
/// Provides full context assembly by reading source files from the
/// filesystem, building failure analysis context, and assembling
/// prompts by filling template placeholders with gathered context.
pub struct LlmContextBuilderServiceImpl {
    /// Root directory of the repository.
    repo_root: String,
    /// Maximum context size in characters.
    max_context_size: usize,
    /// Maximum number of source files to include.
    max_source_files: u32,
    /// Whether to include symbol definitions.
    include_symbols: bool,
}

impl LlmContextBuilderServiceImpl {
    /// Create a new LlmContextBuilderServiceImpl.
    #[tracing::instrument(skip_all)]
    pub fn new() -> Self {
        Self {
            repo_root: String::new(),
            max_context_size: 100_000,
            max_source_files: 20,
            include_symbols: true,
        }
    }

    /// Configure the repository root directory.
    pub fn with_repo_root(mut self, repo_root: impl Into<String>) -> Self {
        self.repo_root = repo_root.into();
        self
    }

    /// Configure the maximum context size.
    pub fn with_max_context_size(mut self, max_context_size: usize) -> Self {
        self.max_context_size = max_context_size;
        self
    }

    /// Configure the maximum number of source files.
    pub fn with_max_source_files(mut self, max_source_files: u32) -> Self {
        self.max_source_files = max_source_files;
        self
    }

    /// Read a source file from the filesystem.
    fn read_source_file(
        &self,
        path: &str,
    ) -> Result<SourceFileContext, LlmStepError> {
        let full_path = if self.repo_root.is_empty() {
            std::path::PathBuf::from(path)
        } else {
            std::path::PathBuf::from(&self.repo_root).join(path)
        };

        let content = std::fs::read_to_string(&full_path).map_err(|e| {
            LlmStepError::ContextBuildFailed {
                message: format!("Failed to read file '{}': {}", path, e),
                context_source: "filesystem".to_string(),
            }
        })?;

        // Detect language from file extension
        let language = Self::detect_language(&full_path);

        let line_count = content.lines().count();

        Ok(SourceFileContext {
            path: path.to_string(),
            content,
            language,
            line_range: Some((1, line_count)),
            is_full_file: true,
        })
    }

    /// Detect the programming language from a file path.
    fn detect_language(path: &std::path::Path) -> String {
        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => "rust".to_string(),
            Some("ts") | Some("tsx") => "typescript".to_string(),
            Some("js") | Some("jsx") => "javascript".to_string(),
            Some("py") => "python".to_string(),
            Some("go") => "go".to_string(),
            Some("java") => "java".to_string(),
            Some("rb") => "ruby".to_string(),
            Some("c") | Some("h") => "c".to_string(),
            Some("cpp") | Some("hpp") | Some("cc") => "cpp".to_string(),
            Some("toml") => "toml".to_string(),
            Some("json") => "json".to_string(),
            Some("yaml") | Some("yml") => "yaml".to_string(),
            Some("md") => "markdown".to_string(),
            Some("sh") => "shell".to_string(),
            Some("sql") => "sql".to_string(),
            _ => "unknown".to_string(),
        }
    }

    /// Format source files into a context block for the prompt.
    #[tracing::instrument(skip_all)]
    fn format_source_files(&self, files: &[SourceFileContext]) -> String {
        let mut output = String::new();
        for file in files {
            output.push_str(&format!(
                "// === {} ===\n{}\n\n",
                file.path, file.content
            ));
        }
        output
    }

    /// Format symbol definitions into a context block for the prompt.
    fn format_symbols(&self, symbols: &[SymbolDefinition]) -> String {
        if symbols.is_empty() {
            return String::new();
        }
        let mut output = String::from("// === Symbol Definitions ===\n");
        for sym in symbols {
            output.push_str(&format!(
                "/// {}: {} ({})\n",
                sym.name, sym.kind, sym.file_path
            ));
            if let Some(ref doc) = sym.doc_comment {
                output.push_str(&format!("///   {}\n", doc));
            }
            output.push_str(&format!("{}\n\n", sym.signature));
        }
        output
    }

    /// Format failure context into a context block for the prompt.
    fn format_failure_context(&self, failure: &FailureContext) -> String {
        let mut output = String::from("// === Previous Failure Analysis ===\n");
        output.push_str(&format!("Failure Type: {}\n", failure.failure_type));
        output.push_str(&format!("Error: {}\n", failure.error_message));
        output.push_str(&format!("Error Output:\n{}\n", failure.error_output));
        output.push_str(&format!(
            "Retries: {}/{}\n",
            failure.retries_attempted, failure.max_retries
        ));
        output.push_str(&format!("Strategy: {}\n", failure.strategy));

        if let Some(ref scenario) = failure.scenario_context {
            output.push_str(&format!("Context: {}\n", scenario));
        }

        if !failure.previous_attempts.is_empty() {
            output.push_str("\nPrevious Attempts:\n");
            for attempt in &failure.previous_attempts {
                output.push_str(&format!(
                    "  Attempt #{} ({}):\n    Output: {}\n    Error: {}\n",
                    attempt.attempt, attempt.attempted_at, attempt.output, attempt.error
                ));
            }
        }

        output
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
        input: GetSourceContextInput,
    ) -> Result<GetSourceContextOutput, LlmStepError> {
        let mut files: Vec<SourceFileContext> = Vec::new();
        let mut total_size: usize = 0;

        // Read files up to the limit
        for path in input.file_paths.iter().take(self.max_source_files as usize) {
            let file = self.read_source_file(path)?;
            total_size += file.content.len();
            files.push(file);

            // Stop if we've exceeded the max context size
            if total_size >= self.max_context_size {
                break;
            }
        }

        // Build symbol definitions (placeholder — repo engine integration)
        let symbols: Vec<SymbolDefinition> = Vec::new();

        let source_context = SourceContext {
            files,
            symbols,
            repo_root: self.repo_root.clone(),
            target_file_path: None,
        };

        Ok(GetSourceContextOutput {
            source_context,
            total_size,
            retrieved_at: Utc::now(),
        })
    }

    async fn get_failure_context(
        &self,
        input: GetFailureContextInput,
    ) -> Result<GetFailureContextOutput, LlmStepError> {
        let max_prev = input.max_previous_attempts.unwrap_or(3);

        let failure_context = FailureContext {
            failure_type: String::new(),
            error_message: String::new(),
            error_output: String::new(),
            retries_attempted: 0,
            max_retries: 3,
            strategy: String::from("retry_with_augmented_context"),
            previous_attempts: Vec::new(),
            scenario_context: None,
        };

        Ok(GetFailureContextOutput {
            failure_context,
            previous_attempt_count: 0,
            retrieved_at: Utc::now(),
        })
    }

    async fn assemble_prompt(
        &self,
        template: String,
        source_context: SourceContext,
        failure_context: Option<FailureContext>,
    ) -> Result<String, LlmStepError> {
        // Build the source code block
        let source_code_block = self.format_source_files(&source_context.files);
        let symbol_block = if self.include_symbols {
            self.format_symbols(&source_context.symbols)
        } else {
            String::new()
        };

        // Build the failure analysis block
        let failure_block = failure_context
            .as_ref()
            .map(|f| self.format_failure_context(f))
            .unwrap_or_default();

        // Build execution context block
        let execution_block = String::new(); // Repo engine integration will fill this

        // Replace placeholders in the template
        let mut prompt = template;
        prompt = prompt.replace("{source_code}", &source_code_block);
        prompt = prompt.replace("{source_files}", &source_code_block);
        prompt = prompt.replace("{symbol_definitions}", &symbol_block);
        prompt = prompt.replace("{failure_context}", &failure_block);
        prompt = prompt.replace("{previous_failure}", &failure_block);
        prompt = prompt.replace("{error_context}", &failure_block);
        prompt = prompt.replace("{execution_context}", &execution_block);

        Ok(prompt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm_step::infrastructure::llm_provider_client_impl::MockLlmProviderClient;

    fn create_test_service() -> LlmStepServiceImpl {
        let provider = MockLlmProviderClient::default();
        let context_builder = LlmContextBuilderServiceImpl::new();
        LlmStepServiceImpl::new(
            Box::new(provider),
            Box::new(context_builder),
            3,
            120,
            true,
        )
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

    // -----------------------------------------------------------------------
    // Context Builder Tests
    // -----------------------------------------------------------------------

    fn create_context_builder() -> LlmContextBuilderServiceImpl {
        LlmContextBuilderServiceImpl::new()
            .with_max_context_size(100_000)
            .with_max_source_files(10)
    }

    #[test]
    fn test_detect_language() {
        let builder = create_context_builder();

        assert_eq!(
            LlmContextBuilderServiceImpl::detect_language(&std::path::Path::new("test.rs")),
            "rust"
        );
        assert_eq!(
            LlmContextBuilderServiceImpl::detect_language(&std::path::Path::new("test.ts")),
            "typescript"
        );
        assert_eq!(
            LlmContextBuilderServiceImpl::detect_language(&std::path::Path::new("test.py")),
            "python"
        );
        assert_eq!(
            LlmContextBuilderServiceImpl::detect_language(&std::path::Path::new("unknown.xyz")),
            "unknown"
        );
    }

    #[test]
    fn test_format_source_files() {
        let builder = create_context_builder();

        let files = vec![
            SourceFileContext {
                path: "src/main.rs".to_string(),
                content: "fn main() {}".to_string(),
                language: "rust".to_string(),
                line_range: Some((1, 1)),
                is_full_file: true,
            },
            SourceFileContext {
                path: "src/lib.rs".to_string(),
                content: "pub fn hello() {}".to_string(),
                language: "rust".to_string(),
                line_range: Some((1, 1)),
                is_full_file: true,
            },
        ];

        let result = builder.format_source_files(&files);
        assert!(result.contains("src/main.rs"));
        assert!(result.contains("fn main() {}"));
        assert!(result.contains("src/lib.rs"));
        assert!(result.contains("pub fn hello() {}"));
    }

    #[test]
    fn test_format_symbols_empty() {
        let builder = create_context_builder();
        let result = builder.format_symbols(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_symbols_with_data() {
        let builder = create_context_builder();

        let symbols = vec![
            SymbolDefinition {
                name: "add".to_string(),
                kind: "function".to_string(),
                file_path: "src/math.rs".to_string(),
                signature: "pub fn add(a: i32, b: i32) -> i32".to_string(),
                doc_comment: Some("Adds two numbers".to_string()),
            },
            SymbolDefinition {
                name: "TaskList".to_string(),
                kind: "struct".to_string(),
                file_path: "src/models.rs".to_string(),
                signature: "pub struct TaskList { items: Vec<Task> }".to_string(),
                doc_comment: None,
            },
        ];

        let result = builder.format_symbols(&symbols);
        assert!(result.contains("add"));
        assert!(result.contains("function"));
        assert!(result.contains("Adds two numbers"));
        assert!(result.contains("TaskList"));
        assert!(result.contains("struct"));
        assert!(result.contains("pub fn add(a: i32, b: i32) -> i32"));
    }

    #[test]
    fn test_format_failure_context() {
        let builder = create_context_builder();

        let failure = FailureContext {
            failure_type: "CompileError".to_string(),
            error_message: "Missing semicolon".to_string(),
            error_output: "error: expected ';' at line 42\n  --> src/main.rs:42:1".to_string(),
            retries_attempted: 1,
            max_retries: 3,
            strategy: "retry_with_augmented_context".to_string(),
            previous_attempts: vec![PreviousAttempt {
                attempt: 1,
                output: "fn main() {}".to_string(),
                error: "Missing semicolon".to_string(),
                attempted_at: Utc::now(),
            }],
            scenario_context: Some("Compilation failed after code generation".to_string()),
        };

        let result = builder.format_failure_context(&failure);
        assert!(result.contains("CompileError"));
        assert!(result.contains("Missing semicolon"));
        assert!(result.contains("error: expected ';' at line 42"));
        assert!(result.contains("1/3"));
        assert!(result.contains("Previous Attempts"));
        assert!(result.contains("Compilation failed"));
    }

    #[test]
    fn test_format_failure_context_no_attempts() {
        let builder = create_context_builder();

        let failure = FailureContext {
            failure_type: "Timeout".to_string(),
            error_message: "Request timed out".to_string(),
            error_output: String::new(),
            retries_attempted: 0,
            max_retries: 2,
            strategy: "retry".to_string(),
            previous_attempts: vec![],
            scenario_context: None,
        };

        let result = builder.format_failure_context(&failure);
        assert!(result.contains("Timeout"));
        assert!(result.contains("Request timed out"));
        assert!(result.contains("0/2"));
        // Should not contain "Previous Attempts" when there are none
        assert!(!result.contains("Previous Attempts"));
    }

    #[tokio::test]
    async fn test_assemble_prompt_basic() {
        let builder = create_context_builder();

        let source = SourceContext {
            files: vec![SourceFileContext {
                path: "src/main.rs".to_string(),
                content: "fn main() { println!(\"hello\"); }".to_string(),
                language: "rust".to_string(),
                line_range: Some((1, 1)),
                is_full_file: true,
            }],
            symbols: vec![],
            repo_root: "/test".to_string(),
            target_file_path: None,
        };

        let template = "Using source: {source_code}".to_string();
        let result = builder
            .assemble_prompt(template, source, None)
            .await
            .unwrap();

        assert!(result.contains("src/main.rs"));
        assert!(result.contains("fn main() { println!(\"hello\"); }"));
    }

    #[tokio::test]
    async fn test_assemble_prompt_with_all_placeholders() {
        let builder = create_context_builder();

        let source = SourceContext {
            files: vec![SourceFileContext {
                path: "src/main.rs".to_string(),
                content: "fn main() {}".to_string(),
                language: "rust".to_string(),
                line_range: Some((1, 1)),
                is_full_file: true,
            }],
            symbols: vec![SymbolDefinition {
                name: "main".to_string(),
                kind: "function".to_string(),
                file_path: "src/main.rs".to_string(),
                signature: "fn main()".to_string(),
                doc_comment: None,
            }],
            repo_root: "/test".to_string(),
            target_file_path: None,
        };

        let failure = FailureContext {
            failure_type: "CompileError".to_string(),
            error_message: "error".to_string(),
            error_output: "output".to_string(),
            retries_attempted: 1,
            max_retries: 3,
            strategy: "retry".to_string(),
            previous_attempts: vec![],
            scenario_context: None,
        };

        let template = "{source_code}\n{symbol_definitions}\n{previous_failure}\n{error_context}".to_string();
        let result = builder
            .assemble_prompt(template, source, Some(failure))
            .await
            .unwrap();

        assert!(result.contains("fn main() {}"));
        assert!(result.contains("main"));
        assert!(result.contains("CompileError"));
    }

    #[tokio::test]
    async fn test_source_context_read_file() {
        // Create a temp directory with a test file
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn test() { assert!(true); }").unwrap();

        let builder = LlmContextBuilderServiceImpl::new()
            .with_repo_root(dir.path().to_str().unwrap());

        let result = builder.read_source_file("test.rs").unwrap();
        assert_eq!(result.path, "test.rs");
        assert_eq!(result.language, "rust");
        assert!(result.content.contains("fn test()"));
    }

    #[tokio::test]
    async fn test_source_context_read_file_not_found() {
        let builder = LlmContextBuilderServiceImpl::new();
        let result = builder.read_source_file("/nonexistent/file.rs");
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmStepError::ContextBuildFailed { context_source, .. } => {
                assert_eq!(context_source, "filesystem");
            }
            _ => panic!("Expected ContextBuildFailed error"),
        }
    }

    #[tokio::test]
    async fn test_get_source_context() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn test() {}").unwrap();

        let builder = LlmContextBuilderServiceImpl::new()
            .with_repo_root(dir.path().to_str().unwrap());

        let result = builder
            .get_source_context(GetSourceContextInput {
                execution_id: Uuid::new_v4(),
                file_paths: vec!["test.rs".to_string()],
                max_context_size: Some(100_000),
            })
            .await
            .unwrap();

        assert_eq!(result.source_context.files.len(), 1);
        assert_eq!(result.source_context.files[0].path, "test.rs");
    }

    #[tokio::test]
    async fn test_get_failure_context() {
        let builder = create_context_builder();

        let result = builder
            .get_failure_context(GetFailureContextInput {
                execution_id: Uuid::new_v4(),
                node_id: Uuid::new_v4(),
                max_previous_attempts: Some(5),
            })
            .await
            .unwrap();

        assert_eq!(result.previous_attempt_count, 0);
        assert_eq!(result.failure_context.max_retries, 3);
    }

    // -----------------------------------------------------------------------
    // Integration Tests — Full Service Pipeline
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_build_context_with_source_files() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("main.rs");
        std::fs::write(&file_path, "fn main() { println!(\"hello\"); }").unwrap();

        let builder = LlmContextBuilderServiceImpl::new()
            .with_repo_root(dir.path().to_str().unwrap());
        let service = LlmStepServiceImpl::new(
            Box::new(MockLlmProviderClient::default()),
            Box::new(builder),
            3,
            120,
            true,
        );

        let node = service
            .create_node(create_test_input())
            .await
            .unwrap()
            .node;

        let result = service
            .build_context(BuildContextInput {
                node_id: node.id,
                execution_id: Uuid::new_v4(),
                dag_id: Uuid::new_v4(),
                target_file_path: None,
                source_file_paths: vec!["main.rs".to_string()],
                include_failure_context: false,
            })
            .await
            .unwrap();

        assert_eq!(result.source_file_count, 1);
        assert!(!result.context.assembled_prompt.is_empty());
        assert!(!result.has_failure_context);
    }

    #[tokio::test]
    async fn test_execute_step_full_pipeline() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("source.rs");
        std::fs::write(&file_path, "pub fn greet(name: &str) -> String { format!(\"Hello {{}}\", name) }").unwrap();

        let builder = LlmContextBuilderServiceImpl::new()
            .with_repo_root(dir.path().to_str().unwrap());
        let service = LlmStepServiceImpl::new(
            Box::new(MockLlmProviderClient::default()),
            Box::new(builder),
            3,
            120,
            true,
        );

        let node = service
            .create_node(create_test_input())
            .await
            .unwrap()
            .node;

        let result = service
            .execute_step(ExecuteStepInput {
                node,
                execution_id: Uuid::new_v4(),
                dag_id: Uuid::new_v4(),
                target_file_path: None,
                source_file_paths: vec!["source.rs".to_string()],
                include_failure_context: false,
                api_key: "test-key".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(result.node_id, result.node_id);
        assert!(result.total_duration_ms >= 0);
        assert!(!result.output.raw_output.is_empty());
        assert_eq!(result.output.total_tokens, 30); // From mock
        assert!(!result.context.assembled_prompt.is_empty());
        // Verify source context was included in the assembled prompt
        assert!(result.context.assembled_prompt.contains("source.rs"));
    }

    #[tokio::test]
    async fn test_execute_step_with_failure_context() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("buggy.rs");
        std::fs::write(&file_path, "fn broken() { incomplete }").unwrap();

        let builder = LlmContextBuilderServiceImpl::new()
            .with_repo_root(dir.path().to_str().unwrap());
        let service = LlmStepServiceImpl::new(
            Box::new(MockLlmProviderClient::default()),
            Box::new(builder),
            3,
            120,
            true,
        );

        let node = service
            .create_node(create_test_input())
            .await
            .unwrap()
            .node;

        let result = service
            .execute_step(ExecuteStepInput {
                node,
                execution_id: Uuid::new_v4(),
                dag_id: Uuid::new_v4(),
                target_file_path: None,
                source_file_paths: vec!["buggy.rs".to_string()],
                include_failure_context: true,
                api_key: "test-key".to_string(),
            })
            .await
            .unwrap();

        assert!(!result.output.raw_output.is_empty());
    }

    #[tokio::test]
    async fn test_generate_with_context() {
        let service = create_test_service();
        let node = service
            .create_node(create_test_input())
            .await
            .unwrap()
            .node;

        let context = LlmStepContext {
            node_id: node.id,
            execution_id: Uuid::new_v4(),
            source_context: SourceContext {
                files: vec![SourceFileContext {
                    path: "src/lib.rs".to_string(),
                    content: "pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
                    language: "rust".to_string(),
                    line_range: Some((1, 1)),
                    is_full_file: true,
                }],
                symbols: vec![SymbolDefinition {
                    name: "add".to_string(),
                    kind: "function".to_string(),
                    file_path: "src/lib.rs".to_string(),
                    signature: "pub fn add(a: i32, b: i32) -> i32".to_string(),
                    doc_comment: Some("Adds two numbers".to_string()),
                }],
                repo_root: "/test".to_string(),
                target_file_path: None,
            },
            failure_context: None,
            execution_context: ExecutionContext::default(),
            assembled_at: Utc::now(),
            assembled_prompt: "Generate a test for add() based on:\n{source_code}".to_string(),
        };

        let result = service
            .generate(GenerateInput {
                node,
                context,
                api_key: "test-key".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(result.output.raw_output, "Mock generated content");
        assert_eq!(result.output.total_tokens, 30);
        assert!(result.duration_ms >= 0);
    }
}



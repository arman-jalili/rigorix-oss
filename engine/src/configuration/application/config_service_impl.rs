//! Implementation of `ConfigService`.
//!
//! @canonical .pi/architecture/modules/configuration.md#config
//! Implements: ConfigService trait — multi-source config loading
//! Issue: #3
//!
//! Orchestrates loading from multiple sources with correct priority:
//! CLI flags > ENV vars > rigorix.toml (CWD) > ~/.rigorix/config.toml > defaults.

use async_trait::async_trait;

use crate::configuration::application::dto::{
    ConfigDto, LlmConfigDto, LoadConfigInput, LoadConfigOutput, SafetyCaps, ToolsConfigDto,
    ValidateConfigInput, ValidateConfigOutput, ValidationError,
};
use crate::configuration::application::factory::ConfigFactory;
use crate::configuration::application::service::ConfigService;
use crate::configuration::domain::{AuditConfig, ConfigurationError, EnforcementPreset, Secret};
use crate::configuration::infrastructure::config_factory_impl::ConfigFactoryImpl;
use crate::configuration::infrastructure::filesystem_config_repository::FilesystemConfigRepository;
use crate::configuration::infrastructure::repository::ConfigRepository;

/// Implementation of `ConfigService` with full multi-source loading.
///
/// Loads config in priority order:
/// 1. CLI flag overrides (highest)
/// 2. Environment variables (RIGORIX__*)
/// 3. rigorix.toml in CWD
/// 4. ~/.rigorix/config.toml (fallback, lowest file priority)
/// 5. Compiled-in defaults (lowest)
pub struct ConfigServiceImpl {
    repository: Box<dyn ConfigRepository>,
    factory: Box<dyn ConfigFactory>,
    /// Cached last loaded state for reload support.
    last_input: tokio::sync::Mutex<Option<LoadConfigInput>>,
}

impl ConfigServiceImpl {
    pub fn new(repository: Box<dyn ConfigRepository>, factory: Box<dyn ConfigFactory>) -> Self {
        Self {
            repository,
            factory,
            last_input: tokio::sync::Mutex::new(None),
        }
    }
}

impl Default for ConfigServiceImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        let cwd = std::env::current_dir().unwrap_or_default();
        Self::new(
            Box::new(FilesystemConfigRepository::new(cwd)),
            Box::new(ConfigFactoryImpl::new()),
        )
    }
}

#[async_trait]
impl ConfigService for ConfigServiceImpl {
    #[tracing::instrument(skip_all)]
    async fn load(&self, input: LoadConfigInput) -> Result<LoadConfigOutput, ConfigurationError> {
        let mut sources_used: Vec<String> = Vec::new();

        // Start with defaults
        let mut config = self.factory.defaults();
        sources_used.push("defaults".to_string());

        // 1. Try TOML file (CWD rigorix.toml or ~/.rigorix/config.toml)
        let env_prefix = input
            .env_prefix
            .clone()
            .unwrap_or_else(|| "RIGORIX__".to_string());

        if let Some(config_path) = self
            .repository
            .resolve_config_path(input.config_path.as_deref())
            .await
        {
            let content = self.repository.read_toml_file(&config_path).await?;
            let file_config = self.factory.build_from_toml(&content).await?;
            // Merge: file values override defaults
            merge_config(&mut config, file_config);
            sources_used.push(format!("file:{config_path}"));
        }

        // 2. Apply environment variable overrides
        let env_config = self
            .factory
            .apply_env_overrides(config.clone(), &env_prefix)
            .await?;
        if env_config != config {
            config = env_config;
            sources_used.push("env".to_string());
        }

        // 3. Apply CLI flag overrides (highest priority)
        if let Some(cli_overrides) = &input.cli_overrides {
            let cli_config = self
                .factory
                .apply_cli_overrides(config.clone(), cli_overrides.clone())
                .await?;
            if cli_config != config {
                config = cli_config;
                sources_used.push("cli".to_string());
            }
        }

        // 4. Validate
        let validate_input = ValidateConfigInput {
            config: config.clone(),
            safety_caps: Some(SafetyCaps::default()),
        };
        let validation = self.validate(validate_input).await?;

        // Cache the input for reload
        *self.last_input.lock().await = Some(input);

        Ok(LoadConfigOutput {
            config,
            sources_used,
            valid: validation.valid,
        })
    }

    async fn validate(
        &self,
        input: ValidateConfigInput,
    ) -> Result<ValidateConfigOutput, ConfigurationError> {
        let caps = input.safety_caps.unwrap_or_default();
        let mut errors: Vec<ValidationError> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        let config = &input.config;

        // Validate orchestrator
        if config.orchestrator.max_parallel_tasks > caps.max_parallel_tasks_cap {
            errors.push(ValidationError {
                field: "orchestrator.max_parallel_tasks".to_string(),
                message: format!(
                    "Max parallel tasks {} exceeds cap of {}",
                    config.orchestrator.max_parallel_tasks, caps.max_parallel_tasks_cap
                ),
                value: Some(config.orchestrator.max_parallel_tasks.to_string()),
            });
        }

        if config.orchestrator.max_retries > caps.max_retries_cap {
            errors.push(ValidationError {
                field: "orchestrator.max_retries".to_string(),
                message: format!(
                    "Max retries {} exceeds cap of {}",
                    config.orchestrator.max_retries, caps.max_retries_cap
                ),
                value: Some(config.orchestrator.max_retries.to_string()),
            });
        }

        if config.orchestrator.default_timeout_secs > caps.max_timeout_secs_cap {
            warnings.push(format!(
                "Timeout {}s exceeds recommended cap of {}s",
                config.orchestrator.default_timeout_secs, caps.max_timeout_secs_cap
            ));
        }

        // Validate LLM
        if config.llm.max_tokens > caps.max_tokens_cap {
            errors.push(ValidationError {
                field: "llm.max_tokens".to_string(),
                message: format!(
                    "Max tokens {} exceeds cap of {}",
                    config.llm.max_tokens, caps.max_tokens_cap
                ),
                value: Some(config.llm.max_tokens.to_string()),
            });
        }

        if config.llm.temperature < 0.0 || config.llm.temperature > 1.0 {
            errors.push(ValidationError {
                field: "llm.temperature".to_string(),
                message: format!(
                    "Temperature {} out of range [0.0, 1.0]",
                    config.llm.temperature
                ),
                value: Some(config.llm.temperature.to_string()),
            });
        }

        Ok(ValidateConfigOutput {
            valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn reload(&self) -> Result<LoadConfigOutput, ConfigurationError> {
        let input = self.last_input.lock().await.take();
        match input {
            Some(input) => self.load(input).await,
            None => {
                // No previous load — load with defaults
                self.load(LoadConfigInput::default()).await
            }
        }
    }
}

/// Merge `source` fields into `base` (source overrides base).
#[tracing::instrument(skip_all)]
fn merge_config(base: &mut ConfigDto, source: ConfigDto) {
    // Override individual fields that are non-default in source
    // Compare to defaults to detect user-specified values
    let defaults = ConfigDto::default_for_comparison();

    if source.orchestrator != defaults.orchestrator {
        base.orchestrator = source.orchestrator;
    }
    if source.logging != defaults.logging {
        base.logging = source.logging;
    }
    if source.tools != defaults.tools {
        base.tools = source.tools;
    }
    if source.enforcement != defaults.enforcement {
        base.enforcement = source.enforcement;
    }
    if source.audit != defaults.audit {
        base.audit = source.audit;
    }
    if source.llm != defaults.llm {
        base.llm = source.llm;
    }
}

// Need a comparison default — different from the actual domain defaults
// so we can detect user-specified values during merge.
impl ConfigDto {
    #[tracing::instrument(skip_all)]
    fn default_for_comparison() -> Self {
        Self {
            orchestrator: Default::default(),
            logging: Default::default(),
            tools: ToolsConfigDto {
                tool_overrides: std::collections::HashMap::new(),
                auto_confirm_low: true,
                require_review_medium: true,
                dry_run_high: true,
            },
            enforcement: EnforcementPreset::Default,
            audit: AuditConfig::default(),
            llm: LlmConfigDto::default_for_comparison(),
        }
    }
}

impl LlmConfigDto {
    #[tracing::instrument(skip_all)]
    fn default_for_comparison() -> Self {
        Self {
            provider: String::new(),
            model: String::new(),
            base_url: None,
            max_tokens: 0,
            temperature: 0.0,
            api_key: Secret::new(""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration::domain::ConfigSource;
    use crate::configuration::infrastructure::config_factory_impl::ConfigFactoryImpl;

    /// Hermetic test repository that finds no config file and no env vars.
    struct NoopConfigRepository;

    #[async_trait::async_trait]
    impl ConfigRepository for NoopConfigRepository {
        async fn read_toml_file(&self, _path: &str) -> Result<String, ConfigurationError> {
            Err(ConfigurationError::NotFound {
                path: _path.into(),
                config_source: ConfigSource::CwdFile,
            })
        }
        async fn resolve_config_path(&self, _path: Option<&str>) -> Option<String> {
            None
        }
        async fn read_env_vars(&self, _prefix: &str) -> std::collections::HashMap<String, String> {
            std::collections::HashMap::new()
        }
        async fn read_env_var(&self, _name: &str) -> Option<String> {
            None
        }
    }

    #[tracing::instrument(skip_all)]
    fn create_service() -> ConfigServiceImpl {
        // Use NoopConfigRepository so tests are hermetic — no ~/.rigorix/ leaks
        let repo = Box::new(NoopConfigRepository);
        let factory = Box::new(ConfigFactoryImpl::new());
        ConfigServiceImpl::new(repo, factory)
    }

    #[tokio::test]
    async fn test_load_with_defaults() {
        let service = create_service();
        let input = LoadConfigInput::default();
        let output = service.load(input).await.unwrap();

        assert!(output.sources_used.contains(&"defaults".to_string()));
        assert_eq!(output.config.orchestrator.max_parallel_tasks, 4);
        assert_eq!(output.config.llm.model, "claude-sonnet-4-6");
    }

    #[tokio::test]
    async fn test_load_with_cli_overrides() {
        let service = create_service();
        let mut overrides = std::collections::HashMap::new();
        overrides.insert(
            "orchestrator.max_parallel_tasks".to_string(),
            "8".to_string(),
        );

        let input = LoadConfigInput {
            cli_overrides: Some(overrides),
            ..Default::default()
        };
        let output = service.load(input).await.unwrap();

        assert_eq!(output.config.orchestrator.max_parallel_tasks, 8);
        assert!(output.sources_used.contains(&"cli".to_string()));
    }

    #[tokio::test]
    async fn test_validate_passes() {
        let service = create_service();
        let config = service.factory.defaults();
        let input = ValidateConfigInput {
            config,
            safety_caps: Some(SafetyCaps::default()),
        };
        let output = service.validate(input).await.unwrap();
        assert!(output.valid);
        assert!(output.errors.is_empty());
    }

    #[tokio::test]
    async fn test_validate_fails_on_excessive_parallelism() {
        let service = create_service();
        let mut config = service.factory.defaults();
        config.orchestrator.max_parallel_tasks = 100;

        let input = ValidateConfigInput {
            config,
            safety_caps: Some(SafetyCaps::default()),
        };
        let output = service.validate(input).await.unwrap();
        assert!(!output.valid);
        assert!(
            output
                .errors
                .iter()
                .any(|e| e.field == "orchestrator.max_parallel_tasks")
        );
    }

    #[tokio::test]
    async fn test_reload_without_prior_load() {
        let service = create_service();
        let output = service.reload().await.unwrap();
        assert!(output.sources_used.contains(&"defaults".to_string()));
    }
}

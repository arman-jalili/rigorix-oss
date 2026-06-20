//! Implementation of `ConfigLoadingService` and related repository.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md#config-loader
//! Implements: ConfigLoadingService, ConfigRepository — loads action.yml defaults
//! and merges with environment overrides
//! Issue: #526
//!
//! The ConfigLoader merges configuration from multiple sources with proper precedence:
//! 1. `INPUT_*` env vars (runtime overrides, highest priority)
//! 2. CLI arguments (if run outside GitHub Actions)
//! 3. `action.yml` defaults
//! 4. Engine defaults

use async_trait::async_trait;
use std::collections::HashMap;

use crate::action_input::application::dto::{LoadConfigInput, LoadConfigOutput};
use crate::action_input::application::input_parser_impl::InputParserImpl;
use crate::action_input::application::service::{ConfigLoadingService, InputParsingService};
use crate::action_input::domain::{ActionConfig, ActionInputError, ActionInputs};
use crate::action_input::infrastructure::repository::ConfigRepository;

// ---------------------------------------------------------------------------
// Config YAML Repository Implementation
// ---------------------------------------------------------------------------

/// Implementation of `ConfigRepository` that reads from the filesystem.
pub struct YamlConfigRepository;

impl YamlConfigRepository {
    pub fn new() -> Self {
        Self
    }
}

impl Default for YamlConfigRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConfigRepository for YamlConfigRepository {
    async fn read_action_yml(&self, path_override: Option<&str>) -> Result<Option<String>, ActionInputError> {
        let path = if let Some(p) = path_override {
            std::path::PathBuf::from(p)
        } else {
            std::env::current_dir()
                .map_err(|e| ActionInputError::Io(e))?
                .join("action.yml")
        };

        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(&path).await?;
        Ok(Some(content))
    }

    async fn parse_yml_defaults(
        &self,
        yaml_content: &str,
    ) -> Result<HashMap<String, serde_yaml::Value>, ActionInputError> {
        let yaml: serde_yaml::Value = serde_yaml::from_str(yaml_content)?;

        let inputs = yaml
            .get("inputs")
            .and_then(|v| v.as_mapping())
            .ok_or_else(|| ActionInputError::ActionYmlParseError {
                detail: "Missing 'inputs' section in action.yml".to_string(),
            })?;

        let mut defaults = HashMap::new();
        for (name, config) in inputs {
            let name_str = name.as_str().ok_or_else(|| {
                ActionInputError::ActionYmlParseError {
                    detail: "Non-string input name in action.yml".to_string(),
                }
            })?;

            // Store just the "default" sub-value if it exists
            if let Some(default) = config.get("default") {
                defaults.insert(name_str.to_string(), default.clone());
            }
        }

        Ok(defaults)
    }

    async fn read_cli_args(&self) -> Result<HashMap<String, String>, ActionInputError> {
        // CLI args are not available in GitHub Actions context by default.
        // This is a no-op for now; CLI overrides can be provided via
        // the LoadConfigInput.cli_overrides field.
        Ok(HashMap::new())
    }

    async fn resolve_action_yml_path(&self) -> Result<Option<String>, ActionInputError> {
        let cwd = std::env::current_dir().map_err(|e| ActionInputError::Io(e))?;
        let path = cwd.join("action.yml");
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            Ok(Some(path.to_string_lossy().to_string()))
        } else {
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------
// ConfigLoader Implementation
// ---------------------------------------------------------------------------

/// Implementation of `ConfigLoadingService`.
///
/// Loads and merges action configuration from multiple sources:
/// 1. INPUT_* env vars (runtime overrides, highest priority)
/// 2. CLI arguments (if provided)
/// 3. action.yml defaults
/// 4. Engine defaults (lowest)
pub struct ConfigLoaderImpl {
    parser: InputParserImpl,
    repository: Box<dyn ConfigRepository>,
}

impl ConfigLoaderImpl {
    pub fn new(parser: InputParserImpl, repository: Box<dyn ConfigRepository>) -> Self {
        Self { parser, repository }
    }
}

impl Default for ConfigLoaderImpl {
    fn default() -> Self {
        Self::new(
            InputParserImpl::default(),
            Box::new(YamlConfigRepository::default()),
        )
    }
}

#[async_trait]
impl ConfigLoadingService for ConfigLoaderImpl {
    async fn load(&self, input: LoadConfigInput) -> Result<LoadConfigOutput, ActionInputError> {
        let mut yml_defaults_applied = false;
        let mut env_overrides_applied = false;
        let mut cli_overrides_applied = false;
        let mut sources = Vec::new();

        // 1. Start with engine defaults
        let mut merged = ActionInputs::default();
        sources.push("engine defaults".to_string());

        // 2. Load action.yml defaults
        let yml_content = if let Some(ref yml_override) = input.action_yml_override {
            Some(yml_override.clone())
        } else if !input.allow_empty_yml {
            self.repository.read_action_yml(None).await?
        } else {
            None
        };

        if let Some(ref yml) = yml_content {
            let yml_defaults = self.repository.parse_yml_defaults(yml).await?;
            // Convert serde_yaml::Value values to strings
            let string_map: HashMap<String, String> = yml_defaults
                .into_iter()
                .map(|(k, v)| (k, Self::yaml_value_to_string(&v)))
                .filter(|(_, v)| !v.is_empty())
                .collect();
            let yml_inputs = self.build_inputs_from_map(&string_map).await?;
            merged = self.merge_inputs(merged, yml_inputs).await?;
            yml_defaults_applied = true;
            sources.push("action.yml defaults".to_string());
        }

        // 3. Apply CLI overrides
        let cli_args = if let Some(ref overrides) = input.cli_overrides {
            overrides.clone()
        } else {
            self.repository.read_cli_args().await?
        };

        if !cli_args.is_empty() {
            let cli_inputs = self.build_inputs_from_map(&cli_args).await?;
            merged = self.merge_inputs(merged, cli_inputs).await?;
            cli_overrides_applied = true;
            sources.push("CLI arguments".to_string());
        }

        // 4. Apply env overrides (highest priority)
        let env_override = input.env_override.clone();
        let parse_input = crate::action_input::application::dto::ParseInputsInput {
            env_prefix: None,
            env_override,
        };
        let parsed = self.parser.parse(parse_input).await?;

        // Check if any env vars were set
        if parsed.populated_count > 0 {
            merged = self.merge_inputs(merged, parsed.inputs).await?;
            env_overrides_applied = true;
            sources.push("INPUT_* environment variables".to_string());
        }

        // 5. Resolve to ActionConfig (all fields concrete)
        let config = self.resolve(merged).await?;

        Ok(LoadConfigOutput {
            config,
            yml_defaults_applied,
            env_overrides_applied,
            cli_overrides_applied,
            sources,
        })
    }

    async fn load_yml_defaults(
        &self,
        path_override: Option<String>,
    ) -> Result<ActionInputs, ActionInputError> {
        let yml_content = self
            .repository
            .read_action_yml(path_override.as_deref())
            .await?;

        let Some(content) = yml_content else {
            return Ok(ActionInputs::default());
        };

        let yml_defaults = self.repository.parse_yml_defaults(&content).await?;
        let string_map: HashMap<String, String> = yml_defaults
            .into_iter()
            .map(|(k, v)| (k, Self::yaml_value_to_string(&v)))
            .filter(|(_, v)| !v.is_empty())
            .collect();
        self.build_inputs_from_map(&string_map).await
    }

    async fn apply_env_overrides(
        &self,
        base: ActionInputs,
    ) -> Result<ActionInputs, ActionInputError> {
        let parse_input = crate::action_input::application::dto::ParseInputsInput {
            env_prefix: None,
            env_override: None,
        };
        let parsed = self.parser.parse(parse_input).await?;
        self.merge_inputs(base, parsed.inputs).await
    }

    async fn apply_cli_overrides(
        &self,
        base: ActionInputs,
        overrides: HashMap<String, String>,
    ) -> Result<ActionInputs, ActionInputError> {
        let cli_inputs = self.build_inputs_from_map(&overrides).await?;
        self.merge_inputs(base, cli_inputs).await
    }

    async fn resolve(
        &self,
        merged: ActionInputs,
    ) -> Result<ActionConfig, ActionInputError> {
        Ok(ActionConfig {
            intent: merged.intent,
            mode: merged.mode.unwrap_or_else(|| "auto".to_string()),
            permission_mode: merged.permission_mode.unwrap_or_else(|| "workspace_write".to_string()),
            policy_file: merged.policy_file.unwrap_or_else(|| ".rigorix/policy.toml".to_string()),
            fail_on_violation: merged.fail_on_violation.unwrap_or(false),
            fail_on_action_error: merged.fail_on_action_error.unwrap_or(false),
            max_llm_calls: merged.max_llm_calls.unwrap_or(50),
            max_llm_tokens: merged.max_llm_tokens.unwrap_or(50000),
            max_validation_iterations: merged.max_validation_iterations.unwrap_or(3),
            max_retries: merged.max_retries.unwrap_or(3),
            retry_delay_ms: merged.retry_delay_ms.unwrap_or(1000),
            post_pr_comment: merged.post_pr_comment.unwrap_or(true),
            profile: merged.profile,
        })
    }
}

// ── Private helpers ──

impl ConfigLoaderImpl {
    /// Convert a serde_yaml::Value to a string representation.
    fn yaml_value_to_string(value: &serde_yaml::Value) -> String {
        match value {
            serde_yaml::Value::String(s) => s.clone(),
            serde_yaml::Value::Bool(b) => b.to_string(),
            serde_yaml::Value::Number(n) => n.to_string(),
            _ => String::new(),
        }
    }

    /// Build ActionInputs from a flat key-value map (from YAML or CLI args).
    async fn build_inputs_from_map(
        &self,
        map: &HashMap<String, String>,
    ) -> Result<ActionInputs, ActionInputError> {
        let mut inputs = ActionInputs::default();

        // Map input names (with hyphens) to our struct fields
        for (key, value) in map {
            let key_upper = key.to_uppercase().replace('-', "_");
            match key_upper.as_str() {
                "INTENT" => inputs.intent = Some(value.clone()),
                "MODE" => inputs.mode = Some(value.clone()),
                "PERMISSION_MODE" | "PERMISSIONMODE" => {
                    inputs.permission_mode = Some(value.clone())
                }
                "POLICY_FILE" | "POLICYFILE" => inputs.policy_file = Some(value.clone()),
                "FAIL_ON_VIOLATION" | "FAILONVIOLATION" => {
                    inputs.fail_on_violation = Some(value == "true")
                }
                "FAIL_ON_ACTION_ERROR" | "FAILONACTIONERROR" => {
                    inputs.fail_on_action_error = Some(value == "true")
                }
                "MAX_LLM_CALLS" | "MAXLLMCALLS" => {
                    inputs.max_llm_calls = value.parse().ok()
                }
                "MAX_LLM_TOKENS" | "MAXLLMTOKENS" => {
                    inputs.max_llm_tokens = value.parse().ok()
                }
                "MAX_VALIDATION_ITERATIONS" | "MAXVALIDATIONITERATIONS" => {
                    inputs.max_validation_iterations = value.parse().ok()
                }
                "MAX_RETRIES" | "MAXRETRIES" => inputs.max_retries = value.parse().ok(),
                "RETRY_DELAY_MS" | "RETRYDELAYMS" => {
                    inputs.retry_delay_ms = value.parse().ok()
                }
                "POST_PR_COMMENT" | "POSTPRCOMMENT" => {
                    inputs.post_pr_comment = Some(value == "true")
                }
                "PROFILE" => inputs.profile = Some(value.clone()),
                _ => {} // Unknown keys are silently ignored
            }
        }

        Ok(inputs)
    }

    /// Merge two ActionInputs, with `overrides` taking precedence.
    async fn merge_inputs(
        &self,
        base: ActionInputs,
        overrides: ActionInputs,
    ) -> Result<ActionInputs, ActionInputError> {
        Ok(ActionInputs {
            intent: overrides.intent.or(base.intent),
            mode: overrides.mode.or(base.mode),
            permission_mode: overrides.permission_mode.or(base.permission_mode),
            policy_file: overrides.policy_file.or(base.policy_file),
            fail_on_violation: overrides.fail_on_violation.or(base.fail_on_violation),
            fail_on_action_error: overrides.fail_on_action_error.or(base.fail_on_action_error),
            max_llm_calls: overrides.max_llm_calls.or(base.max_llm_calls),
            max_llm_tokens: overrides.max_llm_tokens.or(base.max_llm_tokens),
            max_validation_iterations: overrides.max_validation_iterations.or(base.max_validation_iterations),
            max_retries: overrides.max_retries.or(base.max_retries),
            retry_delay_ms: overrides.retry_delay_ms.or(base.retry_delay_ms),
            post_pr_comment: overrides.post_pr_comment.or(base.post_pr_comment),
            profile: overrides.profile.or(base.profile),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_input::infrastructure::repository::ConfigRepository;

    // ── Mock config repository for testing ──

    struct MockConfigRepository {
        yml_content: Option<String>,
    }

    impl MockConfigRepository {
        fn with_yml(content: &str) -> Self {
            Self {
                yml_content: Some(content.to_string()),
            }
        }

        fn empty() -> Self {
            Self { yml_content: None }
        }
    }

    #[async_trait]
    impl ConfigRepository for MockConfigRepository {
        async fn read_action_yml(&self, _path_override: Option<&str>) -> Result<Option<String>, ActionInputError> {
            Ok(self.yml_content.clone())
        }

        async fn parse_yml_defaults(
            &self,
            yaml_content: &str,
        ) -> Result<HashMap<String, serde_yaml::Value>, ActionInputError> {
            // Delegate to real implementation
            let real = YamlConfigRepository::new();
            real.parse_yml_defaults(yaml_content).await
        }

        async fn read_cli_args(&self) -> Result<HashMap<String, String>, ActionInputError> {
            Ok(HashMap::new())
        }

        async fn resolve_action_yml_path(&self) -> Result<Option<String>, ActionInputError> {
            Ok(None)
        }
    }

    fn make_loader(yml: Option<&str>) -> ConfigLoaderImpl {
        let mock_repo = if let Some(content) = yml {
            Box::new(MockConfigRepository::with_yml(content)) as Box<dyn ConfigRepository>
        } else {
            Box::new(MockConfigRepository::empty()) as Box<dyn ConfigRepository>
        };
        ConfigLoaderImpl::new(
            InputParserImpl::default(),
            mock_repo,
        )
    }

    // ── Tests ──

    #[tokio::test]
    async fn test_load_with_yml_defaults() {
        let yml = r#"
name: 'Rigorix'
inputs:
  mode:
    default: 'run'
  max-llm-calls:
    default: '25'
  post-pr-comment:
    default: 'true'
"#;
        let loader = make_loader(Some(yml));
        let input = LoadConfigInput::default();
        let result = loader.load(input).await.unwrap();

        assert!(result.yml_defaults_applied);
        assert!(!result.env_overrides_applied);
        assert_eq!(result.config.mode, "run");
        assert_eq!(result.config.max_llm_calls, 25);
        assert_eq!(result.config.post_pr_comment, true);
    }

    #[tokio::test]
    async fn test_load_with_env_overrides() {
        let yml = r#"
name: 'Rigorix'
inputs:
  mode:
    default: 'run'
  max-llm-calls:
    default: '50'
"#;
        let loader = make_loader(Some(yml));

        let mut env = HashMap::new();
        env.insert("MODE".to_string(), "validate".to_string());
        env.insert("MAX_LLM_CALLS".to_string(), "10".to_string());

        let input = LoadConfigInput {
            env_override: Some(env),
            ..Default::default()
        };

        let result = loader.load(input).await.unwrap();

        assert!(result.yml_defaults_applied);
        assert!(result.env_overrides_applied);
        assert_eq!(result.config.mode, "validate"); // Overridden
        assert_eq!(result.config.max_llm_calls, 10); // Overridden
    }

    #[tokio::test]
    async fn test_load_with_cli_overrides() {
        let loader = make_loader(None);

        let mut cli = HashMap::new();
        cli.insert("mode".to_string(), "plan".to_string());
        cli.insert("max-llm-calls".to_string(), "5".to_string());

        let input = LoadConfigInput {
            cli_overrides: Some(cli),
            ..Default::default()
        };

        let result = loader.load(input).await.unwrap();

        assert!(!result.yml_defaults_applied);
        assert!(result.cli_overrides_applied);
        assert_eq!(result.config.mode, "plan");
        assert_eq!(result.config.max_llm_calls, 5);
    }

    #[tokio::test]
    async fn test_load_with_all_sources() {
        let yml = r#"
name: 'Rigorix'
inputs:
  mode:
    default: 'run'
  max-llm-calls:
    default: '50'
  post-pr-comment:
    default: 'false'
"#;
        let loader = make_loader(Some(yml));

        let mut cli = HashMap::new();
        cli.insert("max-llm-calls".to_string(), "30".to_string());

        let mut env = HashMap::new();
        env.insert("MODE".to_string(), "validate".to_string());

        let input = LoadConfigInput {
            env_override: Some(env),
            cli_overrides: Some(cli),
            ..Default::default()
        };

        let result = loader.load(input).await.unwrap();

        assert!(result.yml_defaults_applied);
        assert!(result.env_overrides_applied);
        assert!(result.cli_overrides_applied);

        // env override (highest priority)
        assert_eq!(result.config.mode, "validate");
        // CLI override (middle priority) - but env didn't set it
        assert_eq!(result.config.max_llm_calls, 30);
        // YAML default (lowest priority - no override)
        assert_eq!(result.config.post_pr_comment, false);
    }

    #[tokio::test]
    async fn test_load_with_no_sources() {
        let loader = make_loader(None);
        let input = LoadConfigInput::default();
        let result = loader.load(input).await.unwrap();

        assert!(!result.yml_defaults_applied);
        assert!(!result.env_overrides_applied);
        assert!(!result.cli_overrides_applied);
        assert_eq!(result.config.mode, "auto");
        assert_eq!(result.config.max_llm_calls, 50);
        assert_eq!(result.config.permission_mode, "workspace_write");
    }

    #[tokio::test]
    async fn test_load_empty_yml_allowed() {
        let loader = make_loader(None);
        let input = LoadConfigInput {
            allow_empty_yml: true,
            ..Default::default()
        };
        let result = loader.load(input).await.unwrap();
        assert!(!result.yml_defaults_applied);
        assert_eq!(result.config.mode, "auto");
    }

    #[tokio::test]
    async fn test_yml_defaults_parsing() {
        let yml = r#"
name: 'Test Action'
description: 'Test'
inputs:
  mode:
    description: 'Mode'
    required: false
    default: 'run'
  max-llm-calls:
    description: 'LLM calls'
    required: false
    default: '100'
  post-pr-comment:
    description: 'Post PR'
    required: false
    default: 'true'
  fail-on-violation:
    description: 'Fail on violation'
    required: false
    default: 'true'
"#;
        let loader = make_loader(Some(yml));
        let input = LoadConfigInput::default();
        let result = loader.load(input).await.unwrap();

        assert!(result.yml_defaults_applied);
        assert_eq!(result.config.mode, "run");
        assert_eq!(result.config.max_llm_calls, 100);
        assert_eq!(result.config.post_pr_comment, true);
        assert_eq!(result.config.fail_on_violation, true);
    }

    #[tokio::test]
    async fn test_resolve_all_defaults() {
        let loader = make_loader(None);
        let inputs = ActionInputs::default();
        let config = loader.resolve(inputs).await.unwrap();

        assert_eq!(config.mode, "auto");
        assert_eq!(config.permission_mode, "workspace_write");
        assert_eq!(config.policy_file, ".rigorix/policy.toml");
        assert_eq!(config.max_llm_calls, 50);
        assert_eq!(config.max_llm_tokens, 50000);
        assert_eq!(config.max_validation_iterations, 3);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 1000);
        assert_eq!(config.post_pr_comment, true);
        assert_eq!(config.fail_on_violation, false);
        assert_eq!(config.fail_on_action_error, false);
        assert_eq!(config.intent, None);
        assert_eq!(config.profile, None);
    }

    #[tokio::test]
    async fn test_merge_inputs_overrides_priority() {
        let loader = make_loader(None);

        let base = ActionInputs {
            mode: Some("run".to_string()),
            max_llm_calls: Some(50),
            ..Default::default()
        };

        let overrides = ActionInputs {
            mode: Some("validate".to_string()),
            ..Default::default()
        };

        let merged = loader.merge_inputs(base, overrides).await.unwrap();

        assert_eq!(merged.mode, Some("validate".to_string()));
        assert_eq!(merged.max_llm_calls, Some(50)); // Preserved from base
    }

    #[tokio::test]
    async fn test_env_override_takes_highest_priority() {
        let yml = r#"
name: 'Rigorix'
inputs:
  mode:
    default: 'governance'
"#;
        let loader = make_loader(Some(yml));

        let mut cli = HashMap::new();
        cli.insert("mode".to_string(), "run".to_string());

        let mut env = HashMap::new();
        env.insert("MODE".to_string(), "validate".to_string());

        let input = LoadConfigInput {
            env_override: Some(env),
            cli_overrides: Some(cli),
            ..Default::default()
        };

        let result = loader.load(input).await.unwrap();

        // Env overrides CLI overrides YAML defaults
        assert_eq!(result.config.mode, "validate");
    }
}

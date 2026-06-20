//! Implementation of `InputParsingService`.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md#parser
//! Implements: InputParsingService trait — reads INPUT_* env vars into typed ActionInputs
//! Issue: #522
//!
//! The InputParser reads `INPUT_<NAME>` environment variables (GitHub Actions convention)
//! and converts them to typed `ActionInputs` fields. It supports:
//!
//! - String fields: intent, mode, permission_mode, required_quality, profile
//! - Numeric fields: max_llm_calls (u32), max_llm_tokens (u64), max_validation_iterations (u32)
//! - Boolean fields: post_pr_comment, fail_on_violation, fail_on_action_error
//! - Optional fields with None when env var is missing/empty
//!
//! # Env Var Naming
//!
//! GitHub Actions passes `with:` block inputs as environment variables:
//! - `intent` → `INPUT_INTENT`
//! - `permission-mode` → `INPUT_PERMISSION_MODE`
//! - `max-llm-calls` → `INPUT_MAX_LLM_CALLS`

use async_trait::async_trait;

use crate::action_input::application::dto::{ParseInputsInput, ParseInputsOutput};
use crate::action_input::application::service::InputParsingService;
use crate::action_input::domain::{ActionInputError, ActionInputs};
use crate::action_input::infrastructure::repository::InputRepository;

/// Implementation of `InputParsingService` that reads from `INPUT_*` env vars.
///
/// Uses an `InputRepository` for testability — production uses `EnvInputRepository`,
/// tests can use a mock.
///
/// # Input Field Mapping
///
/// | Input name | Env var | Type | Default |
/// |---|---|---|---|
/// | intent | `INPUT_INTENT` | `Option<String>` | None |
/// | mode | `INPUT_MODE` | `Option<String>` | None |
/// | permission-mode | `INPUT_PERMISSION_MODE` | `Option<String>` | None |
/// | policy-file | `INPUT_POLICY_FILE` | `Option<String>` | None |
/// | fail-on-violation | `INPUT_FAIL_ON_VIOLATION` | `Option<bool>` | None |
/// | fail-on-action-error | `INPUT_FAIL_ON_ACTION_ERROR` | `Option<bool>` | None |
/// | max-llm-calls | `INPUT_MAX_LLM_CALLS` | `Option<u32>` | None |
/// | max-llm-tokens | `INPUT_MAX_LLM_TOKENS` | `Option<u64>` | None |
/// | max-validation-iterations | `INPUT_MAX_VALIDATION_ITERATIONS` | `Option<u32>` | None |
/// | max-retries | `INPUT_MAX_RETRIES` | `Option<u32>` | None |
/// | retry-delay-ms | `INPUT_RETRY_DELAY_MS` | `Option<u64>` | None |
/// | post-pr-comment | `INPUT_POST_PR_COMMENT` | `Option<bool>` | None |
/// | profile | `INPUT_PROFILE` | `Option<String>` | None |
pub struct InputParserImpl {
    repository: Box<dyn InputRepository>,
}

impl InputParserImpl {
    pub fn new(repository: Box<dyn InputRepository>) -> Self {
        Self { repository }
    }
}

impl Default for InputParserImpl {
    fn default() -> Self {
        Self::new(Box::new(
            crate::action_input::infrastructure::env_input_repository_impl::EnvInputRepository::new(
            ),
        ))
    }
}

#[async_trait]
impl InputParsingService for InputParserImpl {
    async fn parse(&self, input: ParseInputsInput) -> Result<ParseInputsOutput, ActionInputError> {
        let prefix = input
            .env_prefix
            .clone()
            .unwrap_or_else(|| "INPUT_".to_string());

        // If env_override is provided, use it; otherwise read from env
        let mut env_map = if let Some(override_map) = &input.env_override {
            override_map.clone()
        } else {
            self.repository.read_env_vars(&prefix).await?
        };

        // Trim whitespace from all values
        for value in env_map.values_mut() {
            *value = value.trim().to_string();
        }

        let mut inputs = ActionInputs::default();
        let mut populated_count = 0u32;
        let mut warnings = Vec::new();
        let missing_required = Vec::new();

        // Helper: set a field from the env map
        macro_rules! set_str_field {
            ($field:ident, $key:expr) => {
                if let Some(value) = env_map.get($key).filter(|v| !v.is_empty()) {
                    inputs.$field = Some(value.clone());
                    populated_count += 1;
                }
            };
        }

        macro_rules! set_num_field {
            ($field:ident, $key:expr, $type:ty) => {
                if let Some(value) = env_map.get($key).filter(|v| !v.is_empty()) {
                    match value.parse::<$type>() {
                        Ok(parsed) => {
                            inputs.$field = Some(parsed);
                            populated_count += 1;
                        }
                        Err(_) => {
                            warnings.push(format!(
                                "Invalid value for '{}': expected numeric, got '{}'",
                                $key, value
                            ));
                        }
                    }
                }
            };
        }

        macro_rules! set_bool_field {
            ($field:ident, $key:expr) => {
                if let Some(value) = env_map.get($key).filter(|v| !v.is_empty()) {
                    match value.to_lowercase().as_str() {
                        "true" | "1" | "yes" => {
                            inputs.$field = Some(true);
                            populated_count += 1;
                        }
                        "false" | "0" | "no" => {
                            inputs.$field = Some(false);
                            populated_count += 1;
                        }
                        _ => {
                            warnings.push(format!(
                                "Invalid boolean value for '{}': expected true/false, got '{}'",
                                $key, value
                            ));
                        }
                    }
                }
            };
        }

        // String fields
        set_str_field!(intent, "INTENT");
        set_str_field!(mode, "MODE");
        set_str_field!(permission_mode, "PERMISSION_MODE");
        set_str_field!(policy_file, "POLICY_FILE");

        set_str_field!(profile, "PROFILE");

        // Numeric fields
        set_num_field!(max_llm_calls, "MAX_LLM_CALLS", u32);
        set_num_field!(max_llm_tokens, "MAX_LLM_TOKENS", u64);
        set_num_field!(max_validation_iterations, "MAX_VALIDATION_ITERATIONS", u32);
        set_num_field!(max_retries, "MAX_RETRIES", u32);
        set_num_field!(retry_delay_ms, "RETRY_DELAY_MS", u64);

        // Boolean fields
        set_bool_field!(fail_on_violation, "FAIL_ON_VIOLATION");
        set_bool_field!(fail_on_action_error, "FAIL_ON_ACTION_ERROR");
        set_bool_field!(post_pr_comment, "POST_PR_COMMENT");

        Ok(ParseInputsOutput {
            inputs,
            populated_count,
            missing_required,
            warnings,
        })
    }

    async fn parse_field<T: std::str::FromStr>(
        &self,
        _name: &str,
        value: &str,
    ) -> Result<Option<T>, ActionInputError> {
        if value.is_empty() {
            return Ok(None);
        }
        match value.parse::<T>() {
            Ok(parsed) => Ok(Some(parsed)),
            Err(_) => Ok(None),
        }
    }

    async fn require_input(&self, name: &str) -> Result<String, ActionInputError> {
        let key = format!("INPUT_{}", name);
        std::env::var(&key)
            .ok()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| ActionInputError::MissingRequiredInput(name.to_string()))
    }

    async fn read_env_var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_input::infrastructure::repository::InputRepository;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Mock repository for testing — returns pre-set values.
    struct MockInputRepository {
        env_vars: Mutex<HashMap<String, String>>,
    }

    impl MockInputRepository {
        fn new(vars: HashMap<String, String>) -> Self {
            Self {
                env_vars: Mutex::new(vars),
            }
        }
    }

    #[async_trait]
    impl InputRepository for MockInputRepository {
        async fn read_env_var(&self, name: &str) -> Result<Option<String>, ActionInputError> {
            let map = self.env_vars.lock().unwrap();
            Ok(map.get(name).cloned().filter(|v| !v.is_empty()))
        }

        async fn read_env_vars(
            &self,
            prefix: &str,
        ) -> Result<HashMap<String, String>, ActionInputError> {
            let map = self.env_vars.lock().unwrap();
            Ok(map
                .iter()
                .filter(|(k, _)| k.starts_with(prefix))
                .map(|(k, v)| (k[prefix.len()..].to_string(), v.clone()))
                .collect())
        }

        async fn has_env_var(&self, name: &str) -> Result<bool, ActionInputError> {
            let map = self.env_vars.lock().unwrap();
            Ok(map.contains_key(name))
        }

        async fn workspace_root(&self) -> Result<String, ActionInputError> {
            Ok("/test/workspace".to_string())
        }

        async fn read_ci_env_vars(&self) -> Result<HashMap<String, String>, ActionInputError> {
            Ok(HashMap::new())
        }
    }

    fn make_parser(vars: HashMap<String, String>) -> InputParserImpl {
        InputParserImpl::new(Box::new(MockInputRepository::new(vars)))
    }

    #[tokio::test]
    async fn test_parse_all_fields_provided() {
        let mut env = HashMap::new();
        env.insert("INPUT_INTENT".to_string(), "fix the build".to_string());
        env.insert("INPUT_MODE".to_string(), "run".to_string());
        env.insert(
            "INPUT_PERMISSION_MODE".to_string(),
            "workspace_write".to_string(),
        );
        env.insert(
            "INPUT_POLICY_FILE".to_string(),
            ".rigorix/policy.toml".to_string(),
        );
        env.insert("INPUT_MAX_LLM_CALLS".to_string(), "25".to_string());
        env.insert("INPUT_MAX_LLM_TOKENS".to_string(), "100000".to_string());
        env.insert(
            "INPUT_MAX_VALIDATION_ITERATIONS".to_string(),
            "5".to_string(),
        );
        env.insert("INPUT_MAX_RETRIES".to_string(), "3".to_string());
        env.insert("INPUT_RETRY_DELAY_MS".to_string(), "2000".to_string());
        env.insert("INPUT_POST_PR_COMMENT".to_string(), "true".to_string());
        env.insert("INPUT_FAIL_ON_VIOLATION".to_string(), "true".to_string());
        env.insert("INPUT_FAIL_ON_ACTION_ERROR".to_string(), "true".to_string());
        env.insert(
            "INPUT_REQUIRED_QUALITY".to_string(),
            "merge_ready".to_string(),
        );
        env.insert("INPUT_PROFILE".to_string(), "strict".to_string());

        let parser = make_parser(env);
        let input = ParseInputsInput::default();
        let result = parser.parse(input).await.unwrap();

        assert_eq!(result.inputs.intent, Some("fix the build".to_string()));
        assert_eq!(result.inputs.mode, Some("run".to_string()));
        assert_eq!(
            result.inputs.permission_mode,
            Some("workspace_write".to_string())
        );
        assert_eq!(
            result.inputs.policy_file,
            Some(".rigorix/policy.toml".to_string())
        );
        assert_eq!(result.inputs.max_llm_calls, Some(25));
        assert_eq!(result.inputs.max_llm_tokens, Some(100000));
        assert_eq!(result.inputs.max_validation_iterations, Some(5));
        assert_eq!(result.inputs.max_retries, Some(3));
        assert_eq!(result.inputs.retry_delay_ms, Some(2000));
        assert_eq!(result.inputs.post_pr_comment, Some(true));
        assert_eq!(result.inputs.fail_on_violation, Some(true));
        assert_eq!(result.inputs.fail_on_action_error, Some(true));
        assert_eq!(result.inputs.profile, Some("strict".to_string()));
        assert_eq!(result.populated_count, 13);
        assert!(result.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_parse_no_env_vars() {
        let env = HashMap::new();
        let parser = make_parser(env);
        let input = ParseInputsInput::default();
        let result = parser.parse(input).await.unwrap();

        assert_eq!(result.inputs.intent, None);
        assert_eq!(result.inputs.mode, None);
        assert_eq!(result.inputs.max_llm_calls, None);
        assert_eq!(result.populated_count, 0);
        assert!(result.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_parse_invalid_numeric() {
        let mut env = HashMap::new();
        env.insert(
            "INPUT_MAX_LLM_CALLS".to_string(),
            "not-a-number".to_string(),
        );
        env.insert("INPUT_INTENT".to_string(), "test intent".to_string());

        let parser = make_parser(env);
        let input = ParseInputsInput::default();
        let result = parser.parse(input).await.unwrap();

        assert_eq!(result.inputs.intent, Some("test intent".to_string()));
        assert_eq!(result.inputs.max_llm_calls, None);
        assert_eq!(result.populated_count, 1);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("MAX_LLM_CALLS"));
    }

    #[tokio::test]
    async fn test_parse_invalid_boolean() {
        let mut env = HashMap::new();
        env.insert("INPUT_POST_PR_COMMENT".to_string(), "maybe".to_string());

        let parser = make_parser(env);
        let input = ParseInputsInput::default();
        let result = parser.parse(input).await.unwrap();

        assert_eq!(result.inputs.post_pr_comment, None);
        assert_eq!(result.populated_count, 0);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("POST_PR_COMMENT"));
    }

    #[tokio::test]
    async fn test_parse_bool_variants() {
        let mut env = HashMap::new();
        env.insert("INPUT_POST_PR_COMMENT".to_string(), "true".to_string());
        env.insert("INPUT_FAIL_ON_VIOLATION".to_string(), "1".to_string());
        env.insert("INPUT_FAIL_ON_ACTION_ERROR".to_string(), "yes".to_string());

        let parser = make_parser(env);
        let input = ParseInputsInput::default();
        let result = parser.parse(input).await.unwrap();

        assert_eq!(result.inputs.post_pr_comment, Some(true));
        assert_eq!(result.inputs.fail_on_violation, Some(true));
        assert_eq!(result.inputs.fail_on_action_error, Some(true));
    }

    #[tokio::test]
    async fn test_parse_bool_false_variants() {
        let mut env = HashMap::new();
        env.insert("INPUT_POST_PR_COMMENT".to_string(), "false".to_string());
        env.insert("INPUT_FAIL_ON_VIOLATION".to_string(), "0".to_string());
        env.insert("INPUT_FAIL_ON_ACTION_ERROR".to_string(), "no".to_string());

        let parser = make_parser(env);
        let input = ParseInputsInput::default();
        let result = parser.parse(input).await.unwrap();

        assert_eq!(result.inputs.post_pr_comment, Some(false));
        assert_eq!(result.inputs.fail_on_violation, Some(false));
        assert_eq!(result.inputs.fail_on_action_error, Some(false));
    }

    #[tokio::test]
    async fn test_parse_with_env_override() {
        let parser = make_parser(HashMap::new()); // empty real env
        let mut overrides = HashMap::new();
        overrides.insert("INTENT".to_string(), "override intent".to_string());
        overrides.insert("MODE".to_string(), "validate".to_string());

        let input = ParseInputsInput {
            env_prefix: None,
            env_override: Some(overrides),
        };
        let result = parser.parse(input).await.unwrap();

        assert_eq!(result.inputs.intent, Some("override intent".to_string()));
        assert_eq!(result.inputs.mode, Some("validate".to_string()));
        assert_eq!(result.populated_count, 2);
    }

    #[tokio::test]
    async fn test_parse_custom_prefix() {
        let mut env = HashMap::new();
        env.insert("MY_PREFIX_INTENT".to_string(), "custom prefix".to_string());

        let parser = make_parser(env);
        let input = ParseInputsInput {
            env_prefix: Some("MY_PREFIX_".to_string()),
            env_override: None,
        };
        let result = parser.parse(input).await.unwrap();

        assert_eq!(result.inputs.intent, Some("custom prefix".to_string()));
    }

    #[tokio::test]
    async fn test_parse_ignores_empty_values() {
        let mut env = HashMap::new();
        env.insert("INPUT_INTENT".to_string(), "".to_string());
        env.insert("INPUT_MODE".to_string(), "  ".to_string());

        let parser = make_parser(env);
        let input = ParseInputsInput::default();
        let result = parser.parse(input).await.unwrap();

        assert_eq!(result.inputs.intent, None);
        assert_eq!(result.inputs.mode, None);
        assert_eq!(result.populated_count, 0);
    }

    #[tokio::test]
    async fn test_parse_field_valid() {
        let parser = make_parser(HashMap::new());
        let result: Result<Option<u32>, ActionInputError> = parser.parse_field("count", "42").await;
        assert_eq!(result.unwrap(), Some(42));
    }

    #[tokio::test]
    async fn test_parse_field_empty() {
        let parser = make_parser(HashMap::new());
        let result: Result<Option<u32>, ActionInputError> = parser.parse_field("count", "").await;
        assert_eq!(result.unwrap(), None);
    }

    #[tokio::test]
    async fn test_parse_field_invalid() {
        let parser = make_parser(HashMap::new());
        let result: Result<Option<u32>, ActionInputError> =
            parser.parse_field("count", "not-a-number").await;
        assert_eq!(result.unwrap(), None);
    }

    #[tokio::test]
    async fn test_require_input_present() {
        // require_input reads from real std::env, so we need to set it
        // Using unique name to avoid race with parallel tests
        let key = "INPUT_REQUIRE_PRESENT_TEST";
        unsafe {
            std::env::set_var(key, "test_value");
        }
        let parser = make_parser(HashMap::new());
        let result = parser.require_input("REQUIRE_PRESENT_TEST").await;
        unsafe {
            std::env::remove_var(key);
        }
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_value");
    }

    #[tokio::test]
    async fn test_require_input_missing() {
        // Using a unique name that won't conflict
        let key = "INPUT_SHOULD_BE_MISSING_UNIQUE";
        unsafe {
            std::env::remove_var(key);
        }
        let parser = make_parser(HashMap::new());
        let result = parser.require_input("SHOULD_BE_MISSING_UNIQUE").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ActionInputError::MissingRequiredInput(_)
        ));
    }
}

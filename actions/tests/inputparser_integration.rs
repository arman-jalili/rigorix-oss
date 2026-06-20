//! Integration tests for the InputParser component.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md#parser
//! Issue: #523
//!
//! Tests the full InputParser end-to-end: setting real environment variables,
//! parsing them into typed ActionInputs, and verifying the complete output.

use std::collections::HashMap;

use rigorix_actions::action_input::application::dto::ParseInputsInput;
use rigorix_actions::action_input::application::input_parser_impl::InputParserImpl;
use rigorix_actions::action_input::application::service::InputParsingService;
use rigorix_actions::action_input::infrastructure::env_input_repository_impl::EnvInputRepository;

/// Helper to create an InputParserImpl with a real EnvInputRepository.
fn create_parser() -> InputParserImpl {
    InputParserImpl::new(Box::new(EnvInputRepository::new()))
}

#[tokio::test]
async fn test_inputparser_integration_env_override() {
    // Use env_override to avoid polluting real environment
    let parser = create_parser();

    let mut env = HashMap::new();
    env.insert("INTENT".to_string(), "fix the integration tests".to_string());
    env.insert("MODE".to_string(), "validate".to_string());
    env.insert("PERMISSION_MODE".to_string(), "read_only".to_string());
    env.insert("POLICY_FILE".to_string(), "custom/policy.toml".to_string());
    env.insert("MAX_LLM_CALLS".to_string(), "10".to_string());
    env.insert("MAX_LLM_TOKENS".to_string(), "25000".to_string());
    env.insert(
        "MAX_VALIDATION_ITERATIONS".to_string(),
        "5".to_string(),
    );
    env.insert("MAX_RETRIES".to_string(), "2".to_string());
    env.insert("RETRY_DELAY_MS".to_string(), "500".to_string());
    env.insert("POST_PR_COMMENT".to_string(), "false".to_string());
    env.insert("FAIL_ON_VIOLATION".to_string(), "true".to_string());
    env.insert("FAIL_ON_ACTION_ERROR".to_string(), "false".to_string());
    env.insert("PROFILE".to_string(), "testing".to_string());

    let input = ParseInputsInput {
        env_prefix: None,
        env_override: Some(env),
    };

    let result = parser.parse(input).await.unwrap();

    assert_eq!(
        result.inputs.intent,
        Some("fix the integration tests".to_string())
    );
    assert_eq!(result.inputs.mode, Some("validate".to_string()));
    assert_eq!(result.inputs.permission_mode, Some("read_only".to_string()));
    assert_eq!(
        result.inputs.policy_file,
        Some("custom/policy.toml".to_string())
    );
    assert_eq!(result.inputs.max_llm_calls, Some(10));
    assert_eq!(result.inputs.max_llm_tokens, Some(25000));
    assert_eq!(result.inputs.max_validation_iterations, Some(5));
    assert_eq!(result.inputs.max_retries, Some(2));
    assert_eq!(result.inputs.retry_delay_ms, Some(500));
    assert_eq!(result.inputs.post_pr_comment, Some(false));
    assert_eq!(result.inputs.fail_on_violation, Some(true));
    assert_eq!(result.inputs.fail_on_action_error, Some(false));
    assert_eq!(result.inputs.profile, Some("testing".to_string()));
    assert_eq!(result.populated_count, 13);
    assert!(result.warnings.is_empty());
}

#[tokio::test]
async fn test_inputparser_integration_partial_inputs() {
    let parser = create_parser();

    let mut env = HashMap::new();
    env.insert("MODE".to_string(), "status".to_string());
    env.insert("POST_PR_COMMENT".to_string(), "true".to_string());

    let input = ParseInputsInput {
        env_prefix: None,
        env_override: Some(env),
    };

    let result = parser.parse(input).await.unwrap();

    // Only mode and post_pr_comment should be set
    assert_eq!(result.inputs.intent, None);
    assert_eq!(result.inputs.mode, Some("status".to_string()));
    assert_eq!(result.inputs.permission_mode, None);
    assert_eq!(result.inputs.post_pr_comment, Some(true));
    assert_eq!(result.populated_count, 2);
    assert!(result.warnings.is_empty());
}

#[tokio::test]
async fn test_inputparser_integration_invalid_numeric_values() {
    let parser = create_parser();

    let mut env = HashMap::new();
    env.insert("INTENT".to_string(), "run with bad numbers".to_string());
    env.insert(
        "MAX_LLM_CALLS".to_string(),
        "not-a-number".to_string(),
    );
    env.insert(
        "MAX_LLM_TOKENS".to_string(),
        "also-not-a-number".to_string(),
    );

    let input = ParseInputsInput {
        env_prefix: None,
        env_override: Some(env),
    };

    let result = parser.parse(input).await.unwrap();

    assert_eq!(
        result.inputs.intent,
        Some("run with bad numbers".to_string())
    );
    assert_eq!(result.inputs.max_llm_calls, None);
    assert_eq!(result.inputs.max_llm_tokens, None);
    assert_eq!(result.populated_count, 1); // Only intent
    assert_eq!(result.warnings.len(), 2);
    assert!(result.warnings[0].contains("MAX_LLM_CALLS"));
    assert!(result.warnings[1].contains("MAX_LLM_TOKENS"));
}

#[tokio::test]
async fn test_inputparser_integration_empty_inputs() {
    let parser = create_parser();

    let env = HashMap::new(); // Empty — nothing provided

    let input = ParseInputsInput {
        env_prefix: None,
        env_override: Some(env),
    };

    let result = parser.parse(input).await.unwrap();

    assert_eq!(result.inputs.intent, None);
    assert_eq!(result.inputs.mode, None);
    assert_eq!(result.inputs.max_llm_calls, None);
    assert_eq!(result.populated_count, 0);
    assert!(result.warnings.is_empty());
}

#[tokio::test]
async fn test_inputparser_integration_custom_prefix() {
    let parser = create_parser();

    // Test custom prefix with env_override (keys are already prefix-stripped)
    let mut env = HashMap::new();
    env.insert("INTENT".to_string(), "custom prefix test".to_string());
    env.insert("MODE".to_string(), "plan".to_string());

    let input = ParseInputsInput {
        env_prefix: Some("MY_APP_".to_string()),
        env_override: Some(env),
    };

    let result = parser.parse(input).await.unwrap();

    assert_eq!(
        result.inputs.intent,
        Some("custom prefix test".to_string())
    );
    assert_eq!(result.inputs.mode, Some("plan".to_string()));
    assert_eq!(result.populated_count, 2);
}

#[tokio::test]
async fn test_inputparser_integration_bool_variants() {
    let parser = create_parser();

    let mut env = HashMap::new();
    env.insert("POST_PR_COMMENT".to_string(), "true".to_string());
    env.insert("FAIL_ON_VIOLATION".to_string(), "1".to_string());
    env.insert("FAIL_ON_ACTION_ERROR".to_string(), "yes".to_string());

    let input = ParseInputsInput {
        env_prefix: None,
        env_override: Some(env),
    };

    let result = parser.parse(input).await.unwrap();

    assert_eq!(result.inputs.post_pr_comment, Some(true));
    assert_eq!(result.inputs.fail_on_violation, Some(true));
    assert_eq!(result.inputs.fail_on_action_error, Some(true));

    // Now test false variants
    let mut env = HashMap::new();
    env.insert("POST_PR_COMMENT".to_string(), "false".to_string());
    env.insert("FAIL_ON_VIOLATION".to_string(), "0".to_string());
    env.insert("FAIL_ON_ACTION_ERROR".to_string(), "no".to_string());

    let input = ParseInputsInput {
        env_prefix: None,
        env_override: Some(env),
    };

    let result = parser.parse(input).await.unwrap();

    assert_eq!(result.inputs.post_pr_comment, Some(false));
    assert_eq!(result.inputs.fail_on_violation, Some(false));
    assert_eq!(result.inputs.fail_on_action_error, Some(false));
}

#[tokio::test]
async fn test_inputparser_integration_read_field() {
    let parser = create_parser();

    // Test parse_field with valid numeric
    let result: Result<Option<u32>, _> = parser.parse_field("count", "42").await;
    assert_eq!(result.unwrap(), Some(42));

    // Test parse_field with empty
    let result: Result<Option<u32>, _> = parser.parse_field("count", "").await;
    assert_eq!(result.unwrap(), None);

    // Test parse_field with invalid
    let result: Result<Option<u32>, _> = parser.parse_field("count", "abc").await;
    assert_eq!(result.unwrap(), None);
}

#[tokio::test]
async fn test_inputparser_integration_require_input() {
    let parser = create_parser();
    let var = "INPUT_INTEGRATION_REQUIRE_TEST";

    // SAFETY: test-only env manipulation
    unsafe { std::env::set_var(var, "required_value"); }
    let result = parser.require_input("INTEGRATION_REQUIRE_TEST").await;
    unsafe { std::env::remove_var(var); }

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "required_value");
}

#[tokio::test]
async fn test_inputparser_integration_require_input_missing() {
    let parser = create_parser();
    let var = "INPUT_INTEGRATION_REQUIRE_MISSING";
    unsafe { std::env::remove_var(var); }

    let result = parser.require_input("INTEGRATION_REQUIRE_MISSING").await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        rigorix_actions::action_input::domain::ActionInputError::MissingRequiredInput(_)
    ));
}

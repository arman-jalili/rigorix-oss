//! Integration tests for the Enforcement module.
//!
//! Tests config creation, tool evaluation, and safety cap validation.

use crate::enforcement::application::enforcer_factory_impl::ExecutionEnforcerFactoryImpl;
use crate::enforcement::application::factory::ExecutionEnforcerFactory;
use crate::enforcement::domain::EnforcementConfig;

#[tokio::test]
async fn test_default_config_is_valid() {
    let config = EnforcementConfig::standard();
    // Verify it has budgets set up
    assert!(!config.budgets.is_empty(), "Budget should be configured");
}

#[tokio::test]
async fn test_create_enforcer_from_default_config() {
    let factory = ExecutionEnforcerFactoryImpl;
    let enforcer = factory.create_default("test-exec-1").await;
    assert!(
        enforcer.is_ok(),
        "Should create enforcer from default config"
    );
}

#[tokio::test]
async fn test_evaluate_valid_tool() {
    let factory = ExecutionEnforcerFactoryImpl;
    let enforcer = factory.create_default("test-exec-2").await.unwrap();
    let input = crate::enforcement::application::dto::EvaluateToolCallInput {
        execution_id: "test-exec-2".to_string(),
        node_id: "node-1".to_string(),
        tool: "file-read".to_string(),
        arguments: None,
        is_retry: false,
        attempt: 1,
    };
    let result = enforcer.evaluate_tool_call(input).await;
    assert!(result.is_ok(), "file-read should be allowed");
}

#[tokio::test]
async fn test_enforcement_blocks_disallowed_tool() {
    // GAP-A-23: a tool with `allowed: false` in the policy is blocked by the
    // enforcer — the call never reaches execution.
    let mut config = EnforcementConfig::standard();
    config.tool_policies.insert(
        "run-command".to_string(),
        crate::enforcement::domain::config::ToolPolicy {
            allowed: false,
            max_calls: None,
            budget_key: None,
            risk_level: crate::enforcement::domain::config::ToolRiskLevel::High,
            requires_confirmation: false,
            dry_run: false,
        },
    );

    let factory = ExecutionEnforcerFactoryImpl;
    let enforcer = factory
        .create_from_config("test-exec-3", config)
        .await
        .unwrap();
    let input = crate::enforcement::application::dto::EvaluateToolCallInput {
        execution_id: "test-exec-3".to_string(),
        node_id: "node-1".to_string(),
        tool: "run-command".to_string(),
        arguments: None,
        is_retry: false,
        attempt: 1,
    };
    let output = enforcer.evaluate_tool_call(input).await.unwrap();
    assert!(!output.allowed, "disallowed tool must be blocked");
    let reason = output.reason.unwrap_or_default();
    assert!(reason.contains("not allowed"), "got: {reason}");
}

#[tokio::test]
async fn test_enforcement_allows_permitted_tool() {
    let config = EnforcementConfig::standard();
    let factory = ExecutionEnforcerFactoryImpl;
    let enforcer = factory
        .create_from_config("test-exec-4", config)
        .await
        .unwrap();
    let input = crate::enforcement::application::dto::EvaluateToolCallInput {
        execution_id: "test-exec-4".to_string(),
        node_id: "node-1".to_string(),
        tool: "file-read".to_string(),
        arguments: None,
        is_retry: false,
        attempt: 1,
    };
    let output = enforcer.evaluate_tool_call(input).await.unwrap();
    assert!(output.allowed, "permitted tool must pass enforcement");
}

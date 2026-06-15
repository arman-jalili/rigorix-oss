//! Integration tests for the Enforcement module.
//!
//! Tests config creation, tool evaluation, and safety cap validation.

#[cfg(test)]
mod tests {
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
        assert!(enforcer.is_ok(), "Should create enforcer from default config");
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
}

//! planning tests (split by concern, #917).

use super::*;

#[tokio::test]
async fn test_extractor_auto_generates_mock_values() {
    let extractor = MockParameterExtractor::new();

    let intent = UserIntent::new("test".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50,
        max_tokens: 50000,
        used_calls: 0,
        used_tokens: 0,
        label: "test".to_string(),
    };

    let result = extractor
        .extract(
            &intent,
            &budget,
            "template-test",
            &["path".to_string(), "mode".to_string()],
        )
        .await
        .unwrap();

    assert!(result.complete);
    assert_eq!(result.parameters.get("path").unwrap(), "mock_path");
    assert_eq!(result.parameters.get("mode").unwrap(), "mock_mode");
}

#[tokio::test]
async fn test_mock_generator_returns_template() {
    let generator = MockGenerator;
    let intent = UserIntent::new("test".to_string(), None);
    let ctx = RepoContext::new(std::path::PathBuf::from("."), "rust".to_string());
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 10,
        max_tokens: 10000,
        used_calls: 0,
        used_tokens: 0,
        label: "test".to_string(),
    };

    let result = generator.generate(&intent, &ctx, &budget).await;
    assert!(result.is_ok());
    let template = result.unwrap();
    assert_eq!(template.suggested_id, "generated");
    assert_eq!(template.llm_calls_used, 1);
    assert_eq!(template.llm_tokens_used, 200);
}

#[tokio::test]
async fn test_mock_generator_estimate_cost() {
    let generator = MockGenerator;
    let intent = UserIntent::new("test".to_string(), None);
    let cost = generator.estimate_cost(&intent);
    assert_eq!(cost.estimated_calls, 1);
    assert_eq!(cost.estimated_tokens, 200);
}

#[tokio::test]
async fn test_symbol_validation_passes_known_symbols() {
    let mut symbols = std::collections::HashSet::new();
    symbols.insert("MyStruct".to_string());
    symbols.insert("some_function".to_string());

    let mock_graph = MockSymbolGraph { symbols };
    let validator = SymbolValidationServiceImpl::new(Box::new(mock_graph));

    let template = crate::templates::domain::Template {
        id: "test".to_string(),
        name: "Test".to_string(),
        description: "".to_string(),
        version: "1.0.0".to_string(),
        parameters: vec![],
        nodes: vec![crate::templates::domain::TemplateNode {
            id: "step-1".to_string(),
            name: "Step 1".to_string(),
            depends_on: vec![],
            action: crate::templates::domain::TemplateAction::RunCommand {
                command: "MyStruct".to_string(),
                cwd: None,
                timeout_secs: 30,
                env: std::collections::HashMap::new(),
            },
            description: None,
            retry: crate::templates::domain::RetryConfig::default(),
            validate: vec![],
            requires_approval: false,
            require_identity: false,
            intent: None,
        }],
        tags: vec![],
        category: None,
        author: None,
    };

    let input = crate::planning::application::dto::SymbolValidationInput {
        execution_id: uuid::Uuid::new_v4(),
        template,
        max_invalid_references: 10,
        flag_any_type: true,
    };

    let result = validator.validate_template(input).await.unwrap();
    assert!(result.valid);
    assert!(result.invalid_references.is_empty());
    assert!(result.references_checked > 0);
}

#[tokio::test]
async fn test_symbol_validation_catches_hallucinated_types() {
    let symbols = std::collections::HashSet::new(); // empty - no known symbols
    let mock_graph = MockSymbolGraph { symbols };
    let validator = SymbolValidationServiceImpl::new(Box::new(mock_graph));

    let template = crate::templates::domain::Template {
        id: "test".to_string(),
        name: "Test".to_string(),
        description: "".to_string(),
        version: "1.0.0".to_string(),
        parameters: vec![],
        nodes: vec![crate::templates::domain::TemplateNode {
            id: "step-1".to_string(),
            name: "Step 1".to_string(),
            depends_on: vec![],
            action: crate::templates::domain::TemplateAction::RunCommand {
                command: "NonExistentType".to_string(),
                cwd: None,
                timeout_secs: 30,
                env: std::collections::HashMap::new(),
            },
            description: None,
            retry: crate::templates::domain::RetryConfig::default(),
            validate: vec![],
            requires_approval: false,
            require_identity: false,
            intent: None,
        }],
        tags: vec![],
        category: None,
        author: None,
    };

    let input = crate::planning::application::dto::SymbolValidationInput {
        execution_id: uuid::Uuid::new_v4(),
        template,
        max_invalid_references: 10,
        flag_any_type: true,
    };

    let result = validator.validate_template(input).await.unwrap();
    assert!(!result.valid);
    assert_eq!(result.invalid_references.len(), 1);
    assert_eq!(result.invalid_references[0].symbol, "NonExistentType");
}

#[tokio::test]
async fn test_symbol_validation_detects_any_type() {
    let mut symbols = std::collections::HashSet::new();
    symbols.insert("MyStruct".to_string());
    let mock_graph = MockSymbolGraph { symbols };
    let validator = SymbolValidationServiceImpl::new(Box::new(mock_graph));

    let template = crate::templates::domain::Template {
        id: "test".to_string(),
        name: "Test".to_string(),
        description: "".to_string(),
        version: "1.0.0".to_string(),
        parameters: vec![],
        nodes: vec![crate::templates::domain::TemplateNode {
            id: "step-1".to_string(),
            name: "Step 1".to_string(),
            depends_on: vec![],
            action: crate::templates::domain::TemplateAction::RunCommand {
                command: "any".to_string(),
                cwd: None,
                timeout_secs: 30,
                env: std::collections::HashMap::new(),
            },
            description: None,
            retry: crate::templates::domain::RetryConfig::default(),
            validate: vec![],
            requires_approval: false,
            require_identity: false,
            intent: None,
        }],
        tags: vec![],
        category: None,
        author: None,
    };

    let input = crate::planning::application::dto::SymbolValidationInput {
        execution_id: uuid::Uuid::new_v4(),
        template,
        max_invalid_references: 10,
        flag_any_type: true,
    };

    let result = validator.validate_template(input).await.unwrap();
    assert!(!result.valid);
    assert!(result.any_type_detected);
    assert!(result.invalid_references[0].is_any_type);
}

#[tokio::test]
async fn test_symbol_validation_skips_reserved_names() {
    let symbols = std::collections::HashSet::new(); // empty
    let mock_graph = MockSymbolGraph { symbols };
    let validator = SymbolValidationServiceImpl::new(Box::new(mock_graph));

    let template = crate::templates::domain::Template {
        id: "test".to_string(),
        name: "Test".to_string(),
        description: "".to_string(),
        version: "1.0.0".to_string(),
        parameters: vec![],
        nodes: vec![crate::templates::domain::TemplateNode {
            id: "step-1".to_string(),
            name: "Step 1".to_string(),
            depends_on: vec![],
            action: crate::templates::domain::TemplateAction::RunCommand {
                command: "cargo build".to_string(),
                cwd: None,
                timeout_secs: 30,
                env: std::collections::HashMap::new(),
            },
            description: None,
            retry: crate::templates::domain::RetryConfig::default(),
            validate: vec![],
            requires_approval: false,
            require_identity: false,
            intent: None,
        }],
        tags: vec![],
        category: None,
        author: None,
    };

    let input = crate::planning::application::dto::SymbolValidationInput {
        execution_id: uuid::Uuid::new_v4(),
        template,
        max_invalid_references: 10,
        flag_any_type: true,
    };

    let result = validator.validate_template(input).await.unwrap();
    // "cargo" is in the reserved list, so it should be skipped
    assert!(result.valid);
    assert!(result.invalid_references.is_empty());
}

#[tokio::test]
async fn test_symbol_validation_extracts_references() {
    let symbols = std::collections::HashSet::new(); // empty - catches all
    let mock_graph = MockSymbolGraph { symbols };
    let validator = SymbolValidationServiceImpl::new(Box::new(mock_graph));

    let template = crate::templates::domain::Template {
        id: "test".to_string(),
        name: "Test".to_string(),
        description: "".to_string(),
        version: "1.0.0".to_string(),
        parameters: vec![],
        nodes: vec![crate::templates::domain::TemplateNode {
            id: "step-1".to_string(),
            name: "Step 1".to_string(),
            depends_on: vec![],
            action: crate::templates::domain::TemplateAction::LspQuery {
                query_type: "goto-definition".to_string(),
                file: "src/SomeStruct".to_string(),
                line: 10,
                column: 5,
            },
            description: None,
            retry: crate::templates::domain::RetryConfig::default(),
            validate: vec![],
            requires_approval: false,
            require_identity: false,
            intent: None,
        }],
        tags: vec![],
        category: None,
        author: None,
    };

    let refs = validator
        .extract_symbol_references(&template)
        .await
        .unwrap();
    // Should extract "SomeStruct" from the file path "src/SomeStruct"
    assert!(refs.contains(&"SomeStruct".to_string()));
}

#[tokio::test]
async fn test_symbol_validation_flags_any_without_flagging() {
    let mut symbols = std::collections::HashSet::new();
    symbols.insert("any".to_string());
    let mock_graph = MockSymbolGraph { symbols };
    let validator = SymbolValidationServiceImpl::new(Box::new(mock_graph));

    let template = crate::templates::domain::Template {
        id: "test".to_string(),
        name: "Test".to_string(),
        description: "".to_string(),
        version: "1.0.0".to_string(),
        parameters: vec![],
        nodes: vec![crate::templates::domain::TemplateNode {
            id: "step-1".to_string(),
            name: "Step 1".to_string(),
            depends_on: vec![],
            action: crate::templates::domain::TemplateAction::RunCommand {
                command: "any".to_string(),
                cwd: None,
                timeout_secs: 30,
                env: std::collections::HashMap::new(),
            },
            description: None,
            retry: crate::templates::domain::RetryConfig::default(),
            validate: vec![],
            requires_approval: false,
            require_identity: false,
            intent: None,
        }],
        tags: vec![],
        category: None,
        author: None,
    };

    let input = crate::planning::application::dto::SymbolValidationInput {
        execution_id: uuid::Uuid::new_v4(),
        template,
        max_invalid_references: 10,
        flag_any_type: false, // not flagging any type
    };

    let result = validator.validate_template(input).await.unwrap();
    assert!(result.valid); // Should pass because we're not flagging any type
    assert!(result.any_type_detected); // But still detects it
}

#[tokio::test]
async fn test_symbol_validation_extracts_from_parameters() {
    let symbols = std::collections::HashSet::new(); // empty
    let mock_graph = MockSymbolGraph { symbols };
    let validator = SymbolValidationServiceImpl::new(Box::new(mock_graph));

    let template = crate::templates::domain::Template {
        id: "test".to_string(),
        name: "Test".to_string(),
        description: "".to_string(),
        version: "1.0.0".to_string(),
        parameters: vec![],
        nodes: vec![crate::templates::domain::TemplateNode {
            id: "step-1".to_string(),
            name: "Step 1".to_string(),
            depends_on: vec![],
            action: crate::templates::domain::TemplateAction::RunCommand {
                command: "echo MyStruct".to_string(),
                cwd: None,
                timeout_secs: 30,
                env: std::collections::HashMap::new(),
            },
            description: None,
            retry: crate::templates::domain::RetryConfig::default(),
            validate: vec![],
            requires_approval: false,
            require_identity: false,
            intent: None,
        }],
        tags: vec![],
        category: None,
        author: None,
    };

    let refs = validator
        .extract_symbol_references(&template)
        .await
        .unwrap();
    // "MyStruct" is PascalCase and should be extracted
    assert!(refs.contains(&"MyStruct".to_string()));
    // "echo" is lowercase and should not be extracted
    assert!(!refs.contains(&"echo".to_string()));
}

#[tokio::test]
async fn test_symbol_validation_empty_template_passes() {
    let symbols = std::collections::HashSet::new();
    let mock_graph = MockSymbolGraph { symbols };
    let validator = SymbolValidationServiceImpl::new(Box::new(mock_graph));

    let template = crate::templates::domain::Template::default();

    let input = crate::planning::application::dto::SymbolValidationInput {
        execution_id: uuid::Uuid::new_v4(),
        template,
        max_invalid_references: 10,
        flag_any_type: true,
    };

    let result = validator.validate_template(input).await.unwrap();
    assert!(result.valid);
    assert_eq!(result.references_checked, 0);
}

#[tokio::test]
async fn test_symbol_validation_multiple_hallucinated_types() {
    let symbols = std::collections::HashSet::new(); // empty
    let mock_graph = MockSymbolGraph { symbols };
    let validator = SymbolValidationServiceImpl::new(Box::new(mock_graph));

    let template = crate::templates::domain::Template {
        id: "test".to_string(),
        name: "Test".to_string(),
        description: "".to_string(),
        version: "1.0.0".to_string(),
        parameters: vec![],
        nodes: vec![crate::templates::domain::TemplateNode {
            id: "step-1".to_string(),
            name: "Step 1".to_string(),
            depends_on: vec![],
            action: crate::templates::domain::TemplateAction::RunCommand {
                command: "FakeStruct".to_string(),
                cwd: None,
                timeout_secs: 30,
                env: std::collections::HashMap::new(),
            },
            description: None,
            retry: crate::templates::domain::RetryConfig::default(),
            validate: vec![],
            requires_approval: false,
            require_identity: false,
            intent: None,
        }],
        tags: vec![],
        category: None,
        author: None,
    };

    let refs = validator
        .extract_symbol_references(&template)
        .await
        .unwrap();
    assert!(refs.contains(&"FakeStruct".to_string()));
}

#[tokio::test]
async fn test_symbol_validation_does_not_flag_lowercase_names() {
    let symbols = std::collections::HashSet::new();
    let mock_graph = MockSymbolGraph { symbols };
    let validator = SymbolValidationServiceImpl::new(Box::new(mock_graph));

    let template = crate::templates::domain::Template {
        id: "test".to_string(),
        name: "Test".to_string(),
        description: "".to_string(),
        version: "1.0.0".to_string(),
        parameters: vec![],
        nodes: vec![crate::templates::domain::TemplateNode {
            id: "step-1".to_string(),
            name: "Step 1".to_string(),
            depends_on: vec![],
            action: crate::templates::domain::TemplateAction::RunCommand {
                command: "echo 'hello'".to_string(),
                cwd: None,
                timeout_secs: 30,
                env: std::collections::HashMap::new(),
            },
            description: None,
            retry: crate::templates::domain::RetryConfig::default(),
            validate: vec![],
            requires_approval: false,
            require_identity: false,
            intent: None,
        }],
        tags: vec![],
        category: None,
        author: None,
    };

    let refs = validator
        .extract_symbol_references(&template)
        .await
        .unwrap();
    // "echo" and "hello" are lowercase and should not be extracted as symbol references
    assert!(!refs.contains(&"echo".to_string()));
    assert!(!refs.contains(&"hello".to_string()));
}

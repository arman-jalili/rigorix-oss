//! planning tests (split by concern, #917).

use super::*;

#[tokio::test]
async fn test_extractor_simulates_error() {
    let extractor = MockParameterExtractor::new().with_error();

    let intent = UserIntent::new("test".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50,
        max_tokens: 50000,
        used_calls: 0,
        used_tokens: 0,
        label: "test".to_string(),
    };

    let result = extractor
        .extract(&intent, &budget, "template-test", &[])
        .await;

    assert!(result.is_err());
    match result {
        Err(PlanningError::ExtractionError { .. }) => {} // expected
        _ => panic!("Expected ExtractionError"),
    }
}

#[tokio::test]
async fn test_plan_low_confidence_no_generator_returns_error() {
    let pipeline = create_test_pipeline();
    let intent = UserIntent::new("unknown gibberish input".to_string(), None);

    let input = PlanInput {
        intent,
        execution_id: None,
        enable_generator_fallback: false,
        skip_validation: true,
        repo_root: String::new(),
        module_deps: None,
    };

    let result = pipeline.plan(input).await;

    match result {
        Err(PlanningError::NoMatchingTemplate { .. }) => {} // Expected: no match
        Err(PlanningError::ClassificationError { .. }) => {} // Also valid: low-confidence match
        other => panic!(
            "Expected NoMatchingTemplate or ClassificationError, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_planning_error_display_budget_exhausted() {
    let err = PlanningError::BudgetExhausted {
        used_calls: 5,
        max_calls: 5,
        used_tokens: 10000,
        max_tokens: 10000,
    };
    let msg = format!("{}", err);
    assert!(msg.contains("5") && msg.contains("exhausted"));
}

#[test]
fn test_planning_error_display_missing_parameter() {
    let err = PlanningError::MissingParameter {
        template_id: "test-tpl".to_string(),
        parameter: "target".to_string(),
        description: "The target file path".to_string(),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("target") && msg.contains("test-tpl"));
}

#[test]
fn test_planning_error_display_no_matching_template() {
    let err = PlanningError::NoMatchingTemplate {
        intent_preview: "do something".to_string(),
        templates_evaluated: 3,
    };
    let msg = format!("{}", err);
    assert!(msg.contains("do something"));
}

#[test]
fn test_planning_error_display_validation_failed() {
    let err = PlanningError::ValidationFailed {
        detail: "Cycle detected in graph".to_string(),
        error_count: 1,
    };
    let msg = format!("{}", err);
    assert!(msg.contains("validation"));
}

#[test]
fn test_planning_error_display_cancelled() {
    let err = PlanningError::Cancelled;
    let msg = format!("{}", err);
    assert!(msg.contains("cancelled") || msg.contains("Cancelled"));
}

// ---------------------------------------------------------------------------
// UserIntent Tests
// ---------------------------------------------------------------------------

#[test]
fn test_generator_error_display_invalid_toml() {
    let err = GeneratorError::InvalidToml {
        raw_response: "not toml at all".to_string(),
        parse_error: "expected '=', found 'n'".to_string(),
        attempt: 0,
    };
    let msg = err.to_string();
    assert!(msg.contains("Invalid TOML"));
    assert!(msg.contains("attempt 0"));
    assert!(msg.contains("expected '=', found 'n'"));
}

#[test]
fn test_generator_error_display_validation_failed() {
    let err = GeneratorError::ValidationFailed {
        template_id: "my-template".to_string(),
        errors: vec!["missing required field 'nodes'".to_string()],
        attempt: 2,
    };
    let msg = err.to_string();
    assert!(msg.contains("Validation failed"));
    assert!(msg.contains("my-template"));
    assert!(msg.contains("attempt 2"));
    assert!(msg.contains("missing required field"));
}

#[test]
fn test_generator_error_display_symbol_validation() {
    let err = GeneratorError::SymbolValidation {
        template_id: "my-template".to_string(),
        invalid_references: vec![InvalidSymbolReference {
            symbol: "NonExistentType".to_string(),
            usage: "type".to_string(),
            reason: "Type not found in symbol graph".to_string(),
            is_any_type: false,
        }],
        attempt: 1,
    };
    let msg = err.to_string();
    assert!(msg.contains("Symbol validation failed"));
    assert!(msg.contains("my-template"));
    assert!(msg.contains("1 invalid references"));
}

#[test]
fn test_generator_error_display_budget_exhausted() {
    let err = GeneratorError::BudgetExhausted {
        calls_used: 5,
        max_calls: 10,
    };
    let msg = err.to_string();
    assert!(msg.contains("Budget exhausted"));
    assert!(msg.contains("5/10"));
}

#[test]
fn test_generator_error_display_api_error() {
    let err = GeneratorError::ApiError {
        detail: "Rate limited".to_string(),
        status_code: Some(429),
        retry_after: Some(30),
    };
    let msg = err.to_string();
    assert!(msg.contains("API error"));
    assert!(msg.contains("429"));
    assert!(msg.contains("Rate limited"));
}

#[test]
fn test_generator_error_display_max_retries_exhausted() {
    let err = GeneratorError::MaxRetriesExhausted {
        attempts: 3,
        errors: vec!["Invalid TOML".to_string(), "Validation failed".to_string()],
    };
    let msg = err.to_string();
    assert!(msg.contains("Max retries exhausted"));
    assert!(msg.contains("3 attempts"));
    assert!(msg.contains("Invalid TOML"));
}

#[test]
fn test_generator_error_display_context_build_failed() {
    let err = GeneratorError::ContextBuildFailed {
        detail: "Directory not found".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("Context build failed"));
    assert!(msg.contains("Directory not found"));
}

#[test]
fn test_generator_error_from_budget_exhausted_to_planning_error() {
    let gen_err = GeneratorError::BudgetExhausted {
        calls_used: 3,
        max_calls: 10,
    };
    let plan_err: PlanningError = gen_err.into();
    match plan_err {
        PlanningError::BudgetExhausted {
            used_calls,
            max_calls,
            ..
        } => {
            assert_eq!(used_calls, 3);
            assert_eq!(max_calls, 10);
        }
        other => panic!("Expected BudgetExhausted, got: {:?}", other),
    }
}

#[test]
fn test_generator_error_from_other_to_template_engine_error() {
    let gen_err = GeneratorError::InvalidToml {
        raw_response: "bad".to_string(),
        parse_error: "parse error".to_string(),
        attempt: 0,
    };
    let plan_err: PlanningError = gen_err.into();
    match plan_err {
        PlanningError::TemplateEngineError { .. } => {} // Expected
        other => panic!("Expected TemplateEngineError, got: {:?}", other),
    }
}

#[test]
fn test_generator_error_serialization_roundtrip() {
    let err = GeneratorError::InvalidToml {
        raw_response: "not toml".to_string(),
        parse_error: "expected value".to_string(),
        attempt: 0,
    };
    let json = serde_json::to_string(&err).unwrap();
    let deserialized: GeneratorError = serde_json::from_str(&json).unwrap();
    assert_eq!(err, deserialized);
}

#[test]
fn test_generator_error_serialization_symbol_validation() {
    let err = GeneratorError::SymbolValidation {
        template_id: "t1".to_string(),
        invalid_references: vec![InvalidSymbolReference {
            symbol: "BadType".to_string(),
            usage: "type".to_string(),
            reason: "not found".to_string(),
            is_any_type: false,
        }],
        attempt: 1,
    };
    let json = serde_json::to_string(&err).unwrap();
    let deserialized: GeneratorError = serde_json::from_str(&json).unwrap();
    assert_eq!(err, deserialized);
}

#[test]
fn test_invalid_symbol_reference_creation() {
    let refr = InvalidSymbolReference {
        symbol: "MyStruct".to_string(),
        usage: "field_access".to_string(),
        reason: "field 'missing' not found on MyStruct".to_string(),
        is_any_type: false,
    };
    assert_eq!(refr.symbol, "MyStruct");
    assert_eq!(refr.usage, "field_access");
    assert!(refr.reason.contains("missing"));
    assert!(!refr.is_any_type);
}

#[test]
fn test_invalid_symbol_reference_any_type() {
    let refr = InvalidSymbolReference {
        symbol: "any".to_string(),
        usage: "type".to_string(),
        reason: "LLM used 'any' type as escape hatch".to_string(),
        is_any_type: true,
    };
    assert!(refr.is_any_type);
}

#[test]
fn test_claude_classifier_parse_response_invalid_json() {
    let api_key = "test-key".to_string();
    let classifier =
        crate::planning::infrastructure::claude_classifier::ClaudeClassifier::new(api_key, None);

    let result = classifier.parse_response("not json at all");
    assert!(result.is_err());
}

#[test]
fn test_openai_classifier_parse_response_invalid_json() {
    let api_key = "test-key".to_string();
    let classifier =
        crate::planning::infrastructure::openai_classifier::OpenaiClassifier::new(api_key, None);

    let result = classifier.parse_response("invalid");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_symbol_validation_respects_max_invalid() {
    let symbols = std::collections::HashSet::new(); // empty
    let mock_graph = MockSymbolGraph { symbols };
    let validator = SymbolValidationServiceImpl::new(Box::new(mock_graph));

    // Template with multiple nodes each referencing a different non-existent type
    let node1 = crate::templates::domain::TemplateNode {
        id: "step-1".to_string(),
        name: "Step 1".to_string(),
        depends_on: vec![],
        action: crate::templates::domain::TemplateAction::RunCommand {
            command: "TypeA".to_string(),
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
    };
    let node2 = crate::templates::domain::TemplateNode {
        id: "step-2".to_string(),
        name: "Step 2".to_string(),
        depends_on: vec!["step-1".to_string()],
        action: crate::templates::domain::TemplateAction::RunCommand {
            command: "TypeB".to_string(),
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
    };

    let template = crate::templates::domain::Template {
        id: "test".to_string(),
        name: "Test".to_string(),
        description: "".to_string(),
        version: "1.0.0".to_string(),
        parameters: vec![],
        nodes: vec![node1, node2],
        tags: vec![],
        category: None,
        author: None,
    };

    let input = crate::planning::application::dto::SymbolValidationInput {
        execution_id: uuid::Uuid::new_v4(),
        template,
        max_invalid_references: 1,
        flag_any_type: true,
    };

    let result = validator.validate_template(input).await.unwrap();
    assert!(!result.valid);
    assert_eq!(result.invalid_references.len(), 1); // Only 1 reported due to max
}

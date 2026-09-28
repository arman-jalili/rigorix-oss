//! planning tests (split by concern, #917).

use super::*;

#[tokio::test]
async fn test_classifier_high_confidence_match() {
    let classifier = MockClassifier::new().with_match("read file", "template-read", 0.95);

    let intent = UserIntent::new("please read file xyz".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50,
        max_tokens: 50000,
        used_calls: 0,
        used_tokens: 0,
        label: "test".to_string(),
    };

    let result = classifier
        .classify_with_alternatives(&intent, &budget, &["template-read".to_string()])
        .await
        .unwrap();

    assert!(!result.alternatives.is_empty());
    assert_eq!(result.alternatives[0].template_id, "template-read");
    assert!((result.alternatives[0].confidence - 0.95).abs() < 0.01);
    assert!(
        !result.requires_clarification,
        "High confidence should not require clarification"
    );
    assert!(
        !result.needs_generator,
        "High confidence should not need generator"
    );
}

#[tokio::test]
async fn test_classifier_low_confidence_triggers_generator() {
    let classifier = MockClassifier::new().with_match("unknown thing", "template-generate", 0.15);

    let intent = UserIntent::new("unknown thing here".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50,
        max_tokens: 50000,
        used_calls: 0,
        used_tokens: 0,
        label: "test".to_string(),
    };

    let result = classifier
        .classify_with_alternatives(&intent, &budget, &[])
        .await
        .unwrap();

    assert!(
        result.needs_generator,
        "Confidence < 0.3 should need generator"
    );
    assert!(!result.requires_clarification);
}

#[tokio::test]
async fn test_classifier_no_match_triggers_generator() {
    let classifier = MockClassifier::new();
    let intent = UserIntent::new("something completely different".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50,
        max_tokens: 50000,
        used_calls: 0,
        used_tokens: 0,
        label: "test".to_string(),
    };

    let result = classifier
        .classify_with_alternatives(&intent, &budget, &[])
        .await
        .unwrap();

    assert!(result.alternatives.is_empty());
    assert!(result.needs_generator);
}

#[tokio::test]
async fn test_classifier_multiple_alternatives_ranked() {
    let classifier = MockClassifier::new()
        .with_match("edit file", "template-edit", 0.75)
        .with_match("edit file", "template-write", 0.60)
        .with_match("edit file", "template-patch", 0.40);

    let intent = UserIntent::new("edit file".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50,
        max_tokens: 50000,
        used_calls: 0,
        used_tokens: 0,
        label: "test".to_string(),
    };

    let result = classifier
        .classify_with_alternatives(&intent, &budget, &[])
        .await
        .unwrap();

    assert_eq!(result.alternatives.len(), 3);
    // Check descending order
    assert!(result.alternatives[0].confidence >= result.alternatives[1].confidence);
    assert!(result.alternatives[1].confidence >= result.alternatives[2].confidence);
}

// ---------------------------------------------------------------------------
// MockParameterExtractor Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_extractor_returns_default_values() {
    let extractor = MockParameterExtractor::new()
        .with_default("target", "/tmp/file.txt")
        .with_default("mode", "read");

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
            &["target".to_string(), "mode".to_string()],
        )
        .await
        .unwrap();

    assert!(result.complete);
    assert_eq!(result.parameters.get("target").unwrap(), "/tmp/file.txt");
    assert_eq!(result.parameters.get("mode").unwrap(), "read");
}

#[tokio::test]
async fn test_extractor_missing_parameter() {
    let extractor = MockParameterExtractor::new().with_missing("template-test", "required_param");

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
            &["required_param".to_string()],
        )
        .await
        .unwrap();

    assert!(!result.complete);
    assert!(
        result
            .missing_parameters
            .contains(&"required_param".to_string())
    );
}

#[tokio::test]
async fn test_extractor_template_specific_overrides() {
    let extractor = MockParameterExtractor::new()
        .with_default("target", "/tmp/default.txt")
        .with_override("specific-tpl", "target", "/tmp/override.txt");

    let intent = UserIntent::new("test".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50,
        max_tokens: 50000,
        used_calls: 0,
        used_tokens: 0,
        label: "test".to_string(),
    };

    // Test with overriding template
    let result = extractor
        .extract(&intent, &budget, "specific-tpl", &["target".to_string()])
        .await
        .unwrap();
    assert_eq!(
        result.parameters.get("target").unwrap(),
        "/tmp/override.txt"
    );

    // Test without override (should use default)
    let result2 = extractor
        .extract(&intent, &budget, "other-tpl", &["target".to_string()])
        .await
        .unwrap();
    assert_eq!(
        result2.parameters.get("target").unwrap(),
        "/tmp/default.txt"
    );
}

// ---------------------------------------------------------------------------
// Pipeline Classification Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_pipeline_classify_high_confidence() {
    let pipeline = create_test_pipeline();
    let intent = UserIntent::new("read file".to_string(), None);

    let result = pipeline.classify_intent(intent).await.unwrap();
    assert!(!result.alternatives.is_empty());
    assert_eq!(result.alternatives[0].template_id, "template-read");
    assert!(result.alternatives[0].confidence > 0.9);
    assert!(!result.requires_clarification);
}

#[tokio::test]
async fn test_pipeline_classify_no_match() {
    let pipeline = create_test_pipeline();
    let intent = UserIntent::new("completely unknown request".to_string(), None);

    let result = pipeline.classify_intent(intent).await.unwrap();
    assert!(result.needs_generator || result.alternatives.is_empty());
}

// ---------------------------------------------------------------------------
// Pipeline Extraction Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_pipeline_extract_parameters_success() {
    let pipeline = create_test_pipeline();
    let intent = UserIntent::new("test".to_string(), None);

    let input = ExtractParametersInput {
        execution_id: Uuid::new_v4(),
        intent,
        template_id: "template-test".to_string(),
        parameter_names: vec!["target".to_string(), "content".to_string()],
    };

    let result = pipeline.extract_parameters(input).await.unwrap();
    assert!(result.complete);
    assert_eq!(result.parameters.len(), 2);
}

// ---------------------------------------------------------------------------
// Pipeline Full Flow Tests (through public API)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_plan_high_confidence_returns_planning_result() {
    let pipeline = create_test_pipeline();
    let execution_id = Uuid::new_v4();
    let intent = UserIntent::new("read file".to_string(), Some(execution_id));

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
        Ok(output) => {
            assert_eq!(output.planning_result.template_id, "template-read");
            assert!(!output.from_generator);
            assert!(!output.clarification_used);
            assert_eq!(output.planning_result.planning_hash.as_str().len(), 64);
            assert_eq!(output.planning_result.execution_id, pipeline.execution_id());
        }
        Err(e) => panic!("plan() failed unexpectedly: {:?}", e),
    }
}

#[tokio::test]
async fn test_plan_tracks_llm_usage() {
    let pipeline = create_test_pipeline();
    let intent = UserIntent::new("read file".to_string(), None);

    let input = PlanInput {
        intent,
        execution_id: None,
        enable_generator_fallback: false,
        skip_validation: true,
        repo_root: String::new(),
        module_deps: None,
    };

    let result = pipeline.plan(input).await.unwrap();
    assert!(result.total_llm_calls > 0, "Should track LLM calls");
    assert!(result.total_llm_tokens > 0, "Should track LLM tokens");
}

#[tokio::test]
async fn test_plan_with_generator_fallback_attempts_generation() {
    let test_gen = MockGenerator;
    let pipeline = create_test_pipeline();
    let pipeline = pipeline.with_generator(Box::new(test_gen));

    let intent = UserIntent::new("unknown gibberish".to_string(), None);
    let input = PlanInput {
        intent,
        execution_id: None,
        enable_generator_fallback: true,
        skip_validation: true,
        repo_root: String::new(),
        module_deps: None,
    };

    let result = pipeline.plan(input).await;

    match result {
        Ok(output) => {
            // Generator may or may not succeed depending on template registration
            assert!(output.total_llm_calls > 0);
        }
        Err(PlanningError::NoMatchingTemplate { .. }) => {
            // Acceptable: generator didn't create a matching template
        }
        other => panic!("Unexpected error: {:?}", other),
    }
}

#[tokio::test]
async fn test_plan_with_graph_returns_output() {
    let pipeline = create_test_pipeline();
    let intent = UserIntent::new("read file".to_string(), None);

    let input = PlanWithGraphInput {
        intent,
        execution_id: None,
        enable_generator_fallback: false,
        skip_validation: true,
        repo_root: String::new(),
        module_deps: None,
    };

    let result = pipeline.plan_with_graph(input).await;

    match result {
        Ok(output) => {
            assert_eq!(output.planning_result.template_id, "template-read");
            // graph is default TaskGraph in mock scenario
            assert!(output.total_llm_calls > 0);
        }
        Err(e) => panic!("plan_with_graph() failed: {:?}", e),
    }
}

// ---------------------------------------------------------------------------
// Clarification Flow Tests
// ---------------------------------------------------------------------------

#[test]
fn test_user_intent_generates_session_id() {
    let intent1 = UserIntent::new("test".to_string(), None);
    let intent2 = UserIntent::new("test".to_string(), None);
    assert_ne!(
        intent1.session_id, intent2.session_id,
        "Each UserIntent should get a unique session ID"
    );
}

#[tokio::test]
async fn test_generate_graph_returns_output() {
    let pipeline = create_test_pipeline();
    let mut params = HashMap::new();
    params.insert("target".to_string(), "/tmp/file.txt".to_string());

    let input = GenerateGraphInput {
        execution_id: Uuid::new_v4(),
        template_id: "template-read".to_string(),
        parameters: params,
        seal_graph: true,
    };

    let result = pipeline.generate_graph(input).await.unwrap();
    assert!(!result.from_generator);
}

#[tokio::test]
async fn test_validate_plan_without_validator_returns_passed() {
    let pipeline = create_test_pipeline();
    let graph = crate::dag_engine::domain::TaskGraph::default();

    let input = ValidatePlanInput {
        execution_id: Uuid::new_v4(),
        graph,
        template_id: "test".to_string(),
        full_validation: false,
    };

    let result = pipeline.validate_plan(input).await.unwrap();
    assert!(result.passed, "Without validator, plan should pass");
    assert!(result.errors.is_empty());
    assert!(result.warnings.is_empty());
}

#[tokio::test]
async fn test_available_templates_returns_list() {
    let pipeline = create_test_pipeline();
    let result = pipeline.available_templates().await.unwrap();

    assert!(result.total_count > 0, "Should have at least one template");
    assert_eq!(result.templates.len() as u32, result.total_count);
}

#[test]
fn test_execution_id_is_consistent() {
    let pipeline = create_test_pipeline();
    let id1 = pipeline.execution_id();
    let id2 = pipeline.execution_id();
    assert_eq!(id1, id2, "execution_id must be consistent across calls");
}

#[test]
fn test_different_pipelines_have_different_ids() {
    let p1 = create_test_pipeline();
    let p2 = create_test_pipeline();
    assert_ne!(
        p1.execution_id(),
        p2.execution_id(),
        "Different pipelines must have different IDs"
    );
}

// ---------------------------------------------------------------------------
// MockGenerator for testing
// ---------------------------------------------------------------------------

#[test]
fn test_repo_context_creation() {
    let ctx = RepoContext::new(std::path::PathBuf::from("/project"), "rust".to_string());
    assert_eq!(ctx.root_dir.to_str().unwrap(), "/project");
    assert_eq!(ctx.project_type, "rust");
    assert!(ctx.directory_tree.is_empty());
    assert!(ctx.dependencies.is_empty());
    assert!(ctx.public_api.is_empty());
    assert!(ctx.symbol_graph_snapshot.is_none());
}

#[test]
fn test_repo_context_has_files() {
    let mut ctx = RepoContext::new(std::path::PathBuf::from("."), "python".to_string());
    assert!(!ctx.has_files());
    ctx.directory_tree.push("src/main.py".to_string());
    assert!(ctx.has_files());
}

#[test]
fn test_repo_context_has_public_api() {
    let mut ctx = RepoContext::new(std::path::PathBuf::from("."), "typescript".to_string());
    assert!(!ctx.has_public_api());
    ctx.public_api = "fetchUser".to_string();
    assert!(ctx.has_public_api());
}

#[test]
fn test_repo_context_serialization_roundtrip() {
    let mut ctx = RepoContext::new(std::path::PathBuf::from("/repo"), "rust".to_string());
    ctx.directory_tree.push("src/lib.rs".to_string());
    ctx.dependencies.push("serde".to_string());
    ctx.public_api = "MyStruct".to_string();
    ctx.symbol_graph_snapshot = Some(serde_json::json!({"types": ["MyStruct"]}));

    let json = serde_json::to_string(&ctx).unwrap();
    let deserialized: RepoContext = serde_json::from_str(&json).unwrap();
    assert_eq!(ctx.root_dir, deserialized.root_dir);
    assert_eq!(ctx.project_type, deserialized.project_type);
    assert_eq!(ctx.directory_tree, deserialized.directory_tree);
    assert_eq!(ctx.dependencies, deserialized.dependencies);
    assert_eq!(
        ctx.symbol_graph_snapshot,
        deserialized.symbol_graph_snapshot
    );
}

#[test]
fn test_generated_template_creation() {
    let template = GeneratedTemplate {
        toml_content: "id = \"test\"\nname = \"Test\"\n".to_string(),
        suggested_id: "test".to_string(),
        suggested_name: "Test Template".to_string(),
        description: "A test template".to_string(),
        llm_calls_used: 2,
        llm_tokens_used: 500,
    };
    assert_eq!(template.suggested_id, "test");
    assert!(template.toml_content.contains("id = \"test\""));
    assert_eq!(template.llm_calls_used, 2);
    assert_eq!(template.llm_tokens_used, 500);
}

#[test]
fn test_generated_template_cost_creation() {
    let cost = GeneratedTemplateCost {
        estimated_calls: 1,
        estimated_tokens: 200,
    };
    assert_eq!(cost.estimated_calls, 1);
    assert_eq!(cost.estimated_tokens, 200);
}

#[test]
fn test_template_generator_trait_is_object_safe() {
    // Verify the trait can be used as a trait object
    fn takes_generator(_gen: &dyn TemplateGenerator) {}
    let generator = MockGenerator;
    takes_generator(&generator);
}

// ---------------------------------------------------------------------------
// ClaudeTemplateGenerator Tests
// ---------------------------------------------------------------------------

#[test]
fn test_claude_generator_config_defaults() {
    let config = ClaudeGeneratorConfig::default();
    assert_eq!(config.api_url, "https://api.anthropic.com/v1/messages");
    assert_eq!(config.model, "claude-sonnet-4-20250514");
    assert_eq!(config.max_tokens, 4096);
    assert_eq!(config.timeout_secs, 120);
    assert!((config.temperature - 0.3).abs() < 0.01);
    assert_eq!(config.max_retries, 3);
}

#[test]
fn test_claude_generator_strip_code_fences_no_fences() {
    let input = "id = \"test\"\nname = \"Test\"\n";
    let result = ClaudeTemplateGenerator::strip_code_fences(input);
    assert_eq!(result, "id = \"test\"\nname = \"Test\"");
}

#[test]
fn test_claude_generator_strip_code_fences_with_toml() {
    let input = "```toml\nid = \"test\"\nname = \"Test\"\n```";
    let result = ClaudeTemplateGenerator::strip_code_fences(input);
    assert_eq!(result, "id = \"test\"\nname = \"Test\"");
}

#[test]
fn test_claude_generator_strip_code_fences_with_triple_backtick() {
    let input = "```\nid = \"test\"\n```";
    let result = ClaudeTemplateGenerator::strip_code_fences(input);
    assert_eq!(result, "id = \"test\"");
}

#[test]
fn test_claude_generator_strip_code_fences_no_open_fence() {
    let input = "id = \"test\"\n```";
    let result = ClaudeTemplateGenerator::strip_code_fences(input);
    assert_eq!(result, "id = \"test\"");
}

#[test]
fn test_claude_generator_strip_code_fences_only_closing() {
    let input = "id = \"test\"";
    let result = ClaudeTemplateGenerator::strip_code_fences(input);
    assert_eq!(result, "id = \"test\"");
}

#[test]
fn test_claude_generator_strip_code_fences_whitespace() {
    let input = "  ```toml\nid = \"test\"\n  ```  ";
    let result = ClaudeTemplateGenerator::strip_code_fences(input);
    assert_eq!(result, "id = \"test\"");
}

#[test]
fn test_claude_generator_estimate_cost() {
    let api_key = "test-key".to_string();
    let generator = ClaudeTemplateGenerator::new(api_key, None);
    let intent = crate::planning::domain::intent::UserIntent::new("test".to_string(), None);
    let cost = generator.estimate_cost(&intent);
    assert_eq!(cost.estimated_calls, 3);
    assert_eq!(cost.estimated_tokens, 4096);
}

#[test]
fn test_claude_generator_custom_config() {
    let config = ClaudeGeneratorConfig {
        api_url: "https://custom.api.com/v1/messages".to_string(),
        model: "claude-3-opus".to_string(),
        max_tokens: 2048,
        timeout_secs: 60,
        temperature: 0.5,
        max_retries: 2,
    };
    let api_key = "test-key".to_string();
    let generator = ClaudeTemplateGenerator::new(api_key, Some(config));
    let intent = crate::planning::domain::intent::UserIntent::new("test".to_string(), None);
    let cost = generator.estimate_cost(&intent);
    assert_eq!(cost.estimated_calls, 2);
    assert_eq!(cost.estimated_tokens, 2048);
}

// ---------------------------------------------------------------------------
// ClaudeClassifier Tests
// ---------------------------------------------------------------------------

#[test]
fn test_claude_classifier_parse_response_valid_json() {
    let api_key = "test-key".to_string();
    let classifier =
        crate::planning::infrastructure::claude_classifier::ClaudeClassifier::new(api_key, None);

    let response = r#"{"rankings":[{"template_id":"read-file","confidence":0.95,"reasoning":"Best match for read intent"},{"template_id":"write-file","confidence":0.20,"reasoning":"Poor match"}]}"#;

    let result = classifier.parse_response(response).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].template_id, "read-file");
    assert!((result[0].confidence - 0.95).abs() < 0.01);
    assert_eq!(result[1].template_id, "write-file");
    assert!((result[1].confidence - 0.20).abs() < 0.01);
}

#[test]
fn test_claude_classifier_parse_response_clamps_confidence() {
    let api_key = "test-key".to_string();
    let classifier =
        crate::planning::infrastructure::claude_classifier::ClaudeClassifier::new(api_key, None);

    let response = r#"{"rankings":[{"template_id":"t1","confidence":1.5,"reasoning":"Too high"},{"template_id":"t2","confidence":-0.5,"reasoning":"Too low"}]}"#;

    let result = classifier.parse_response(response).unwrap();
    assert!(
        (result[0].confidence - 1.0).abs() < 0.01,
        "Should clamp to 1.0"
    );
    assert!(
        (result[1].confidence - 0.0).abs() < 0.01,
        "Should clamp to 0.0"
    );
}

#[test]
fn test_claude_classifier_parse_response_empty_rankings() {
    let api_key = "test-key".to_string();
    let classifier =
        crate::planning::infrastructure::claude_classifier::ClaudeClassifier::new(api_key, None);

    let response = r#"{"rankings":[]}"#;
    let result = classifier.parse_response(response);
    assert!(result.is_err());
}

#[test]
fn test_claude_classifier_parse_response_with_markdown_fence() {
    let api_key = "test-key".to_string();
    let classifier =
        crate::planning::infrastructure::claude_classifier::ClaudeClassifier::new(api_key, None);

    // Simulate Claude wrapping JSON in markdown code fences
    let response = "Here's the classification:\n```json\n{\"rankings\":[{\"template_id\":\"read\",\"confidence\":0.9,\"reasoning\":\"Good\"}]}\n```";

    let result = classifier.parse_response(response).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].template_id, "read");
}

#[test]
fn test_claude_classifier_handles_empty_templates() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let api_key = "test-key".to_string();
    let classifier =
        crate::planning::infrastructure::claude_classifier::ClaudeClassifier::new(api_key, None);
    let intent = UserIntent::new("test".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50,
        max_tokens: 50000,
        used_calls: 0,
        used_tokens: 0,
        label: "test".to_string(),
    };

    let result = rt
        .block_on(classifier.classify_with_alternatives(&intent, &budget, &[]))
        .unwrap();
    assert!(result.alternatives.is_empty());
    assert!(result.needs_generator);
}

#[test]
fn test_claude_config_defaults() {
    let config =
        crate::planning::infrastructure::claude_classifier::ClaudeClassifierConfig::default();
    assert_eq!(config.model, "claude-sonnet-4-20250514");
    assert_eq!(config.max_tokens, 1024);
    assert!(config.temperature < 0.5);
}

// ---------------------------------------------------------------------------
// OpenaiClassifier Tests
// ---------------------------------------------------------------------------

#[test]
fn test_openai_classifier_parse_response_valid_json() {
    let api_key = "test-key".to_string();
    let classifier =
        crate::planning::infrastructure::openai_classifier::OpenaiClassifier::new(api_key, None);

    let response =
        r#"{"rankings":[{"template_id":"t1","confidence":0.85,"reasoning":"Good match"}]}"#;

    let result = classifier.parse_response(response).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].template_id, "t1");
    assert!((result[0].confidence - 0.85).abs() < 0.01);
}

#[test]
fn test_openai_classifier_parse_response_empty() {
    let api_key = "test-key".to_string();
    let classifier =
        crate::planning::infrastructure::openai_classifier::OpenaiClassifier::new(api_key, None);

    let result = classifier.parse_response(r#"{"rankings":[]}"#);
    assert!(result.is_err());
}

#[test]
fn test_openai_classifier_parse_response_with_markdown() {
    let api_key = "test-key".to_string();
    let classifier =
        crate::planning::infrastructure::openai_classifier::OpenaiClassifier::new(api_key, None);

    let response = "```\n{\"rankings\":[{\"template_id\":\"t1\",\"confidence\":0.75,\"reasoning\":\"OK\"}]}\n```";
    let result = classifier.parse_response(response).unwrap();
    assert_eq!(result[0].template_id, "t1");
}

#[test]
fn test_openai_classifier_handles_empty_templates() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let api_key = "test-key".to_string();
    let classifier =
        crate::planning::infrastructure::openai_classifier::OpenaiClassifier::new(api_key, None);
    let intent = UserIntent::new("test".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50,
        max_tokens: 50000,
        used_calls: 0,
        used_tokens: 0,
        label: "test".to_string(),
    };

    let result = rt
        .block_on(classifier.classify_with_alternatives(&intent, &budget, &[]))
        .unwrap();
    assert!(result.alternatives.is_empty());
    assert!(result.needs_generator);
}

#[test]
fn test_openai_config_defaults() {
    let config =
        crate::planning::infrastructure::openai_classifier::OpenaiClassifierConfig::default();
    assert_eq!(config.model, "gpt-4o");
    assert_eq!(config.max_tokens, 1024);
}

// ---------------------------------------------------------------------------
// Symbol Validation Tests
// ---------------------------------------------------------------------------

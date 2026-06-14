//! Unit tests for the Planning Pipeline bounded context.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: PlanningPipeline — Unit tests
//! Issue: issue-planningpipeline
//!
//! Tests cover the PlanningPipelineImpl orchestration, MockClassifier,
//! MockParameterExtractor, planning hash computation, error handling,
//! and edge cases.

use std::collections::HashMap;
use uuid::Uuid;

use crate::planning::application::dto::{
    CheckBudgetInput, ExtractParametersInput, GenerateGraphInput, PlanInput, PlanWithGraphInput,
    RequestClarificationInput, ValidatePlanInput,
};
use crate::planning::application::pipeline_impl::PlanningPipelineImpl;
use crate::planning::application::service::PlanningPipelineService;
use crate::planning::domain::classification::Classifier;
use crate::planning::domain::extractor::ParameterExtractor;
use crate::planning::domain::generator::{
    GeneratedTemplate, GeneratedTemplateCost, GeneratorError, InvalidSymbolReference,
    RepoContext, TemplateGenerator,
};
use crate::planning::domain::intent::UserIntent;
use crate::planning::domain::mock_classifier::MockClassifier;
use crate::planning::domain::mock_extractor::MockParameterExtractor;
use crate::planning::domain::result::{PlanOutput, PlanningHash, PlanningResult};

use super::domain::PlanningError;

// ---------------------------------------------------------------------------
// Helper: MockTemplateEngine for controlled testing
// ---------------------------------------------------------------------------

/// A mock template engine service that returns controlled responses.
struct MockTemplateEngine;

impl MockTemplateEngine {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl crate::templates::application::service::TemplateEngineService for MockTemplateEngine {
    async fn register(
        &self,
        _input: crate::templates::application::dto::RegisterInput,
    ) -> Result<crate::templates::application::dto::RegisterOutput, crate::templates::domain::TemplateError>
    {
        Ok(crate::templates::application::dto::RegisterOutput {
            template_id: "mock-registered".to_string(),
            total_templates: 1,
            overwritten: false,
        })
    }

    async fn generate(
        &self,
        input: crate::templates::application::dto::GenerateInput,
    ) -> Result<crate::templates::application::dto::GenerateOutput, crate::templates::domain::TemplateError>
    {
        Ok(crate::templates::application::dto::GenerateOutput {
            template_id: input.template_id,
            nodes: vec![],
            edges: vec![],
            valid: true,
            topological_order: vec![],
            errors: vec![],
            execution_id: input.execution_id,
            node_count: 0,
        })
    }

    async fn get_template(
        &self,
        _input: crate::templates::application::dto::GetTemplateInput,
    ) -> Result<Option<crate::templates::application::dto::TemplateSummary>, crate::templates::domain::TemplateError>
    {
        Ok(None)
    }

    async fn list_templates(
        &self,
    ) -> Result<crate::templates::application::dto::ListTemplatesOutput, crate::templates::domain::TemplateError>
    {
        // Return a "template-read" template so classify_intent finds it
        let template = crate::templates::application::dto::TemplateSummary {
            id: "template-read".to_string(),
            name: "Read File".to_string(),
            description: "Read a file".to_string(),
            version: "1.0.0".to_string(),
            param_count: 2,
            node_count: 1,
            tags: vec![],
            category: None,
            is_builtin: false,
        };
        Ok(crate::templates::application::dto::ListTemplatesOutput {
            templates: vec![template],
            total: 1,
        })
    }

    async fn has_template(&self, _template_id: &str) -> bool {
        true
    }

    async fn template_count(&self) -> usize {
        1
    }
}

// ---------------------------------------------------------------------------
// Helper: create a minimal test pipeline
// ---------------------------------------------------------------------------

fn create_test_pipeline() -> PlanningPipelineImpl {
    let classifier = Box::new(
        MockClassifier::new()
            .with_match("read file", "template-read", 0.95)
            .with_match("write file", "template-write", 0.85)
            .with_match("ambiguous task", "template-a", 0.45)
            .with_match("unknown", "template-generate", 0.15),
    );

    let extractor = Box::new(
        MockParameterExtractor::new()
            .with_default("target", "/tmp/test.txt")
            .with_default("content", "hello world"),
    );

    let execution_id = Uuid::new_v4();
    PlanningPipelineImpl::new(execution_id, classifier, extractor, Box::new(MockTemplateEngine::new()))
}

// ---------------------------------------------------------------------------
// Planning Hash Tests (via public helper)
// ---------------------------------------------------------------------------

#[test]
fn test_planning_hash_deterministic_across_parameter_order() {
    let mut params1 = HashMap::new();
    params1.insert("target".to_string(), "/tmp/file.txt".to_string());
    params1.insert("mode".to_string(), "read".to_string());

    let mut params2 = HashMap::new();
    params2.insert("mode".to_string(), "read".to_string());
    params2.insert("target".to_string(), "/tmp/file.txt".to_string());

    let intent = UserIntent::new("read the file".to_string(), None);

    let hash1 = crate::planning::application::pipeline_impl::compute_planning_hash(
        "template-id", &params1, &intent.input,
    );
    let hash2 = crate::planning::application::pipeline_impl::compute_planning_hash(
        "template-id", &params2, &intent.input,
    );

    // Different parameter order should produce the same hash
    assert_eq!(hash1, hash2, "Hash must be deterministic regardless of parameter order");
}

#[test]
fn test_planning_hash_different_intent_produces_different_hash() {
    let params = HashMap::new();
    let hash1 = crate::planning::application::pipeline_impl::compute_planning_hash(
        "template-id", &params, "read file",
    );
    let hash2 = crate::planning::application::pipeline_impl::compute_planning_hash(
        "template-id", &params, "write file",
    );
    assert_ne!(hash1, hash2, "Different intents must produce different hashes");
}

#[test]
fn test_planning_hash_format() {
    let params = HashMap::new();
    let hash = crate::planning::application::pipeline_impl::compute_planning_hash(
        "t", &params, "test",
    );
    assert_eq!(hash.as_str().len(), 64, "PlanningHash must be 64 hex characters");
    assert!(hash.as_str().chars().all(|c| c.is_ascii_hexdigit()), "Hash must be hex");
}

#[test]
fn test_planning_hash_different_templates_produce_different_hash() {
    let params = HashMap::new();
    let hash1 = crate::planning::application::pipeline_impl::compute_planning_hash(
        "template-a", &params, "test",
    );
    let hash2 = crate::planning::application::pipeline_impl::compute_planning_hash(
        "template-b", &params, "test",
    );
    assert_ne!(hash1, hash2, "Different template IDs must produce different hashes");
}

// ---------------------------------------------------------------------------
// MockClassifier Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_classifier_high_confidence_match() {
    let classifier = MockClassifier::new()
        .with_match("read file", "template-read", 0.95);

    let intent = UserIntent::new("please read file xyz".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50, max_tokens: 50000, used_calls: 0, used_tokens: 0, label: "test".to_string(),
    };

    let result = classifier
        .classify_with_alternatives(&intent, &budget, &["template-read".to_string()])
        .await
        .unwrap();

    assert!(!result.alternatives.is_empty());
    assert_eq!(result.alternatives[0].template_id, "template-read");
    assert!((result.alternatives[0].confidence - 0.95).abs() < 0.01);
    assert!(!result.requires_clarification, "High confidence should not require clarification");
    assert!(!result.needs_generator, "High confidence should not need generator");
}

#[tokio::test]
async fn test_classifier_low_confidence_triggers_generator() {
    let classifier = MockClassifier::new()
        .with_match("unknown thing", "template-generate", 0.15);

    let intent = UserIntent::new("unknown thing here".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50, max_tokens: 50000, used_calls: 0, used_tokens: 0, label: "test".to_string(),
    };

    let result = classifier
        .classify_with_alternatives(&intent, &budget, &[])
        .await
        .unwrap();

    assert!(result.needs_generator, "Confidence < 0.3 should need generator");
    assert!(!result.requires_clarification);
}

#[tokio::test]
async fn test_classifier_ambiguous_requires_clarification() {
    let classifier = MockClassifier::new()
        .with_match("ambiguous task", "template-a", 0.45);

    let intent = UserIntent::new("ambiguous task".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50, max_tokens: 50000, used_calls: 0, used_tokens: 0, label: "test".to_string(),
    };

    let result = classifier
        .classify_with_alternatives(&intent, &budget, &[])
        .await
        .unwrap();

    assert!(result.requires_clarification, "Confidence 0.3-0.7 should require clarification");
    assert!(!result.needs_generator);
}

#[tokio::test]
async fn test_classifier_no_match_triggers_generator() {
    let classifier = MockClassifier::new();
    let intent = UserIntent::new("something completely different".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50, max_tokens: 50000, used_calls: 0, used_tokens: 0, label: "test".to_string(),
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
        max_calls: 50, max_tokens: 50000, used_calls: 0, used_tokens: 0, label: "test".to_string(),
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
        max_calls: 50, max_tokens: 50000, used_calls: 0, used_tokens: 0, label: "test".to_string(),
    };

    let result = extractor
        .extract(&intent, &budget, "template-test", &["target".to_string(), "mode".to_string()])
        .await
        .unwrap();

    assert!(result.complete);
    assert_eq!(result.parameters.get("target").unwrap(), "/tmp/file.txt");
    assert_eq!(result.parameters.get("mode").unwrap(), "read");
}

#[tokio::test]
async fn test_extractor_auto_generates_mock_values() {
    let extractor = MockParameterExtractor::new();

    let intent = UserIntent::new("test".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50, max_tokens: 50000, used_calls: 0, used_tokens: 0, label: "test".to_string(),
    };

    let result = extractor
        .extract(&intent, &budget, "template-test", &["path".to_string(), "mode".to_string()])
        .await
        .unwrap();

    assert!(result.complete);
    assert_eq!(result.parameters.get("path").unwrap(), "mock_path");
    assert_eq!(result.parameters.get("mode").unwrap(), "mock_mode");
}

#[tokio::test]
async fn test_extractor_missing_parameter() {
    let extractor = MockParameterExtractor::new()
        .with_missing("template-test", "required_param");

    let intent = UserIntent::new("test".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50, max_tokens: 50000, used_calls: 0, used_tokens: 0, label: "test".to_string(),
    };

    let result = extractor
        .extract(&intent, &budget, "template-test", &["required_param".to_string()])
        .await
        .unwrap();

    assert!(!result.complete);
    assert!(result.missing_parameters.contains(&"required_param".to_string()));
}

#[tokio::test]
async fn test_extractor_simulates_error() {
    let extractor = MockParameterExtractor::new().with_error();

    let intent = UserIntent::new("test".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50, max_tokens: 50000, used_calls: 0, used_tokens: 0, label: "test".to_string(),
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
async fn test_extractor_template_specific_overrides() {
    let extractor = MockParameterExtractor::new()
        .with_default("target", "/tmp/default.txt")
        .with_override("specific-tpl", "target", "/tmp/override.txt");

    let intent = UserIntent::new("test".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50, max_tokens: 50000, used_calls: 0, used_tokens: 0, label: "test".to_string(),
    };

    // Test with overriding template
    let result = extractor
        .extract(&intent, &budget, "specific-tpl", &["target".to_string()])
        .await
        .unwrap();
    assert_eq!(result.parameters.get("target").unwrap(), "/tmp/override.txt");

    // Test without override (should use default)
    let result2 = extractor
        .extract(&intent, &budget, "other-tpl", &["target".to_string()])
        .await
        .unwrap();
    assert_eq!(result2.parameters.get("target").unwrap(), "/tmp/default.txt");
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
async fn test_pipeline_classify_ambiguous() {
    let pipeline = create_test_pipeline();
    let intent = UserIntent::new("ambiguous task".to_string(), None);

    let result = pipeline.classify_intent(intent).await.unwrap();
    assert!(result.requires_clarification);
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
    };

    let result = pipeline.plan(input).await.unwrap();
    assert!(result.total_llm_calls > 0, "Should track LLM calls");
    assert!(result.total_llm_tokens > 0, "Should track LLM tokens");
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
    };

    let result = pipeline.plan(input).await;

    match result {
        Err(PlanningError::NoMatchingTemplate { .. }) => {} // Expected: no match
        Err(PlanningError::ClassificationError { .. }) => {} // Also valid: low-confidence match
        other => panic!("Expected NoMatchingTemplate or ClassificationError, got: {:?}", other),
    }
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

#[tokio::test]
async fn test_request_clarification_generates_question() {
    let pipeline = create_test_pipeline();
    let intent = UserIntent::new("ambiguous task".to_string(), None);

    let classification = pipeline.classify_intent(intent.clone()).await.unwrap();

    let input = RequestClarificationInput {
        execution_id: Uuid::new_v4(),
        intent,
        classification: classification.clone(),
        custom_question: None,
    };

    let output = pipeline.request_clarification(input).await.unwrap();
    assert!(!output.question.is_empty(), "Should generate a question");
    assert!(output.ambiguous_templates.len() >= 1, "Should include ambiguous templates");
}

#[tokio::test]
async fn test_request_clarification_with_custom_question() {
    let pipeline = create_test_pipeline();
    let intent = UserIntent::new("ambiguous task".to_string(), None);
    let classification = pipeline.classify_intent(intent.clone()).await.unwrap();

    let input = RequestClarificationInput {
        execution_id: Uuid::new_v4(),
        intent,
        classification,
        custom_question: Some("What exactly do you want?".to_string()),
    };

    let output = pipeline.request_clarification(input).await.unwrap();
    assert_eq!(output.question, "What exactly do you want?");
}

// ---------------------------------------------------------------------------
// PlanningError Display Tests
// ---------------------------------------------------------------------------

#[test]
fn test_planning_error_display_budget_exhausted() {
    let err = PlanningError::BudgetExhausted {
        used_calls: 5, max_calls: 5, used_tokens: 10000, max_tokens: 10000,
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
fn test_user_intent_creation() {
    let intent = UserIntent::new("Hello world".to_string(), Some(Uuid::nil()));
    assert_eq!(intent.input, "Hello world");
    assert!(intent.clarifications.is_empty());
    assert_eq!(intent.execution_id, Some(Uuid::nil()));
}

#[test]
fn test_user_intent_generates_session_id() {
    let intent1 = UserIntent::new("test".to_string(), None);
    let intent2 = UserIntent::new("test".to_string(), None);
    assert_ne!(intent1.session_id, intent2.session_id, "Each UserIntent should get a unique session ID");
}

#[test]
fn test_user_intent_with_clarification() {
    let intent = UserIntent::new("Initial".to_string(), None)
        .with_clarification("Which file?".to_string(), "the config".to_string());

    assert_eq!(intent.clarification_count(), 1);
    assert_eq!(intent.clarifications[0].question, "Which file?");
    assert_eq!(intent.clarifications[0].answer, "the config");
    assert!(intent.has_clarifications());
}

#[test]
fn test_user_intent_multiple_clarifications() {
    let intent = UserIntent::new("Do thing".to_string(), None)
        .with_clarification("What file?".to_string(), "config.json".to_string())
        .with_clarification("What action?".to_string(), "read".to_string());

    assert_eq!(intent.clarification_count(), 2);
    assert!(intent.has_clarifications());
}

#[test]
fn test_user_intent_full_context() {
    let intent = UserIntent::new("Read config".to_string(), None)
        .with_clarification("Which file?".to_string(), "app.json".to_string());

    let context = intent.full_context();
    assert!(context.contains("Read config"));
    assert!(context.contains("Which file?"));
    assert!(context.contains("app.json"));
}

#[test]
fn test_user_intent_latest_clarification() {
    let intent = UserIntent::new("test".to_string(), None)
        .with_clarification("Q1".to_string(), "A1".to_string())
        .with_clarification("Q2".to_string(), "A2".to_string());

    let latest = intent.latest_clarification().unwrap();
    assert_eq!(latest.question, "Q2");
    assert_eq!(latest.answer, "A2");
}

// ---------------------------------------------------------------------------
// PlanningResult Tests
// ---------------------------------------------------------------------------

#[test]
fn test_planning_result_creation() {
    let mut params = HashMap::new();
    params.insert("target".to_string(), "/tmp/file".to_string());

    let hash = PlanningHash::new(
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
    );

    let result = PlanningResult::new(
        Uuid::new_v4(),
        "template-id".to_string(),
        0.95,
        params.clone(),
        hash,
        false,
        2,
        500,
    );

    assert_eq!(result.template_id, "template-id");
    assert_eq!(result.parameters, params);
    assert!((result.confidence - 0.95).abs() < 0.01);
    assert_eq!(result.llm_calls_used, 2);
    assert_eq!(result.llm_tokens_used, 500);
}

#[test]
#[should_panic(expected = "exactly 64 hex characters")]
fn test_planning_hash_invalid_length_panics() {
    PlanningHash::new("too-short".to_string());
}

#[test]
fn test_planning_result_tracks_timestamps() {
    let hash = PlanningHash::new(
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
    );

    let result = PlanningResult::new(
        Uuid::new_v4(),
        "tpl".to_string(),
        0.8,
        HashMap::new(),
        hash,
        false,
        1,
        100,
    );

    // planned_at should be set to now (within reasonable tolerance)
    let now = chrono::Utc::now();
    let diff = now - result.planned_at;
    assert!(diff.num_seconds() < 5, "planned_at should be recent");
}

// ---------------------------------------------------------------------------
// Pipeline Edge Cases
// ---------------------------------------------------------------------------

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
    assert_ne!(p1.execution_id(), p2.execution_id(), "Different pipelines must have different IDs");
}

// ---------------------------------------------------------------------------
// MockGenerator for testing
// ---------------------------------------------------------------------------

struct MockGenerator;

#[async_trait::async_trait]
impl TemplateGenerator for MockGenerator {
    async fn generate(
        &self,
        _intent: &UserIntent,
        _repo_context: &RepoContext,
        _budget: &crate::budget_tracking::domain::LlmBudget,
    ) -> Result<GeneratedTemplate, GeneratorError> {
        Ok(GeneratedTemplate {
            toml_content: "id = \"generated\"".to_string(),
            suggested_id: "generated".to_string(),
            suggested_name: "Generated Template".to_string(),
            description: "Auto-generated".to_string(),
            llm_calls_used: 1,
            llm_tokens_used: 200,
        })
    }

    fn estimate_cost(&self, _intent: &UserIntent) -> GeneratedTemplateCost {
        GeneratedTemplateCost { estimated_calls: 1, estimated_tokens: 200 }
    }
}

// ---------------------------------------------------------------------------
// TemplateGenerator Trait Tests
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
        invalid_references: vec![
            InvalidSymbolReference {
                symbol: "NonExistentType".to_string(),
                usage: "type".to_string(),
                reason: "Type not found in symbol graph".to_string(),
                is_any_type: false,
            },
        ],
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
fn test_repo_context_creation() {
    let ctx = RepoContext::new(
        std::path::PathBuf::from("/project"),
        "rust".to_string(),
    );
    assert_eq!(ctx.root_dir.to_str().unwrap(), "/project");
    assert_eq!(ctx.project_type, "rust");
    assert!(ctx.directory_tree.is_empty());
    assert!(ctx.dependencies.is_empty());
    assert!(ctx.public_api.is_empty());
    assert!(ctx.symbol_graph_snapshot.is_none());
}

#[test]
fn test_repo_context_has_files() {
    let mut ctx = RepoContext::new(
        std::path::PathBuf::from("."),
        "python".to_string(),
    );
    assert!(!ctx.has_files());
    ctx.directory_tree.push("src/main.py".to_string());
    assert!(ctx.has_files());
}

#[test]
fn test_repo_context_has_public_api() {
    let mut ctx = RepoContext::new(
        std::path::PathBuf::from("."),
        "typescript".to_string(),
    );
    assert!(!ctx.has_public_api());
    ctx.public_api.push("fetchUser".to_string());
    assert!(ctx.has_public_api());
}

#[test]
fn test_repo_context_serialization_roundtrip() {
    let mut ctx = RepoContext::new(
        std::path::PathBuf::from("/repo"),
        "rust".to_string(),
    );
    ctx.directory_tree.push("src/lib.rs".to_string());
    ctx.dependencies.push("serde".to_string());
    ctx.public_api.push("MyStruct".to_string());
    ctx.symbol_graph_snapshot = Some(serde_json::json!({"types": ["MyStruct"]}));

    let json = serde_json::to_string(&ctx).unwrap();
    let deserialized: RepoContext = serde_json::from_str(&json).unwrap();
    assert_eq!(ctx.root_dir, deserialized.root_dir);
    assert_eq!(ctx.project_type, deserialized.project_type);
    assert_eq!(ctx.directory_tree, deserialized.directory_tree);
    assert_eq!(ctx.dependencies, deserialized.dependencies);
    assert_eq!(ctx.symbol_graph_snapshot, deserialized.symbol_graph_snapshot);
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

#[tokio::test]
async fn test_mock_generator_returns_template() {
    let generator = MockGenerator;
    let intent = UserIntent::new("test".to_string(), None);
    let ctx = RepoContext::new(
        std::path::PathBuf::from("."),
        "rust".to_string(),
    );
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
fn test_template_generator_trait_is_object_safe() {
    // Verify the trait can be used as a trait object
    fn takes_generator(_gen: &dyn TemplateGenerator) {}
    let gen = MockGenerator;
    takes_generator(&gen);
}

// ---------------------------------------------------------------------------
// ClaudeClassifier Tests
// ---------------------------------------------------------------------------

#[test]
fn test_claude_classifier_parse_response_valid_json() {
    let api_key = "test-key".to_string();
    let classifier = crate::planning::domain::claude_classifier::ClaudeClassifier::new(api_key, None);

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
    let classifier = crate::planning::domain::claude_classifier::ClaudeClassifier::new(api_key, None);

    let response = r#"{"rankings":[{"template_id":"t1","confidence":1.5,"reasoning":"Too high"},{"template_id":"t2","confidence":-0.5,"reasoning":"Too low"}]}"#;

    let result = classifier.parse_response(response).unwrap();
    assert!((result[0].confidence - 1.0).abs() < 0.01, "Should clamp to 1.0");
    assert!((result[1].confidence - 0.0).abs() < 0.01, "Should clamp to 0.0");
}

#[test]
fn test_claude_classifier_parse_response_empty_rankings() {
    let api_key = "test-key".to_string();
    let classifier = crate::planning::domain::claude_classifier::ClaudeClassifier::new(api_key, None);

    let response = r#"{"rankings":[]}"#;
    let result = classifier.parse_response(response);
    assert!(result.is_err());
}

#[test]
fn test_claude_classifier_parse_response_invalid_json() {
    let api_key = "test-key".to_string();
    let classifier = crate::planning::domain::claude_classifier::ClaudeClassifier::new(api_key, None);

    let result = classifier.parse_response("not json at all");
    assert!(result.is_err());
}

#[test]
fn test_claude_classifier_parse_response_with_markdown_fence() {
    let api_key = "test-key".to_string();
    let classifier = crate::planning::domain::claude_classifier::ClaudeClassifier::new(api_key, None);

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
    let classifier = crate::planning::domain::claude_classifier::ClaudeClassifier::new(api_key, None);
    let intent = UserIntent::new("test".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50, max_tokens: 50000, used_calls: 0, used_tokens: 0, label: "test".to_string(),
    };

    let result = rt.block_on(classifier.classify_with_alternatives(&intent, &budget, &[])).unwrap();
    assert!(result.alternatives.is_empty());
    assert!(result.needs_generator);
}

#[test]
fn test_claude_config_defaults() {
    let config = crate::planning::domain::claude_classifier::ClaudeClassifierConfig::default();
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
    let classifier = crate::planning::domain::openai_classifier::OpenaiClassifier::new(api_key, None);

    let response = r#"{"rankings":[{"template_id":"t1","confidence":0.85,"reasoning":"Good match"}]}"#;

    let result = classifier.parse_response(response).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].template_id, "t1");
    assert!((result[0].confidence - 0.85).abs() < 0.01);
}

#[test]
fn test_openai_classifier_parse_response_invalid_json() {
    let api_key = "test-key".to_string();
    let classifier = crate::planning::domain::openai_classifier::OpenaiClassifier::new(api_key, None);

    let result = classifier.parse_response("invalid");
    assert!(result.is_err());
}

#[test]
fn test_openai_classifier_parse_response_empty() {
    let api_key = "test-key".to_string();
    let classifier = crate::planning::domain::openai_classifier::OpenaiClassifier::new(api_key, None);

    let result = classifier.parse_response(r#"{"rankings":[]}"#);
    assert!(result.is_err());
}

#[test]
fn test_openai_classifier_parse_response_with_markdown() {
    let api_key = "test-key".to_string();
    let classifier = crate::planning::domain::openai_classifier::OpenaiClassifier::new(api_key, None);

    let response = "```\n{\"rankings\":[{\"template_id\":\"t1\",\"confidence\":0.75,\"reasoning\":\"OK\"}]}\n```";
    let result = classifier.parse_response(response).unwrap();
    assert_eq!(result[0].template_id, "t1");
}

#[test]
fn test_openai_classifier_handles_empty_templates() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let api_key = "test-key".to_string();
    let classifier = crate::planning::domain::openai_classifier::OpenaiClassifier::new(api_key, None);
    let intent = UserIntent::new("test".to_string(), None);
    let budget = crate::budget_tracking::domain::LlmBudget {
        max_calls: 50, max_tokens: 50000, used_calls: 0, used_tokens: 0, label: "test".to_string(),
    };

    let result = rt.block_on(classifier.classify_with_alternatives(&intent, &budget, &[])).unwrap();
    assert!(result.alternatives.is_empty());
    assert!(result.needs_generator);
}

#[test]
fn test_openai_config_defaults() {
    let config = crate::planning::domain::openai_classifier::OpenaiClassifierConfig::default();
    assert_eq!(config.model, "gpt-4o");
    assert_eq!(config.max_tokens, 1024);
}

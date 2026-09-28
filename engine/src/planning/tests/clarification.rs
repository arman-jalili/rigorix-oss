//! planning tests (split by concern, #917).

use super::*;

#[tokio::test]
async fn test_classifier_ambiguous_requires_clarification() {
    let classifier = MockClassifier::new().with_match("ambiguous task", "template-a", 0.45);

    let intent = UserIntent::new("ambiguous task".to_string(), None);
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
        result.requires_clarification,
        "Confidence 0.3-0.7 should require clarification"
    );
    assert!(!result.needs_generator);
}

#[tokio::test]
async fn test_pipeline_classify_ambiguous() {
    let pipeline = create_test_pipeline();
    let intent = UserIntent::new("ambiguous task".to_string(), None);

    let result = pipeline.classify_intent(intent).await.unwrap();
    assert!(result.requires_clarification);
}

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
    assert!(
        !output.ambiguous_templates.is_empty(),
        "Should include ambiguous templates"
    );
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
fn test_user_intent_creation() {
    let intent = UserIntent::new("Hello world".to_string(), Some(Uuid::nil()));
    assert_eq!(intent.input, "Hello world");
    assert!(intent.clarifications.is_empty());
    assert_eq!(intent.execution_id, Some(Uuid::nil()));
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

//! planning tests (split by concern, #917).

use super::*;

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
        "template-id",
        &params1,
        &intent.input,
    );
    let hash2 = crate::planning::application::pipeline_impl::compute_planning_hash(
        "template-id",
        &params2,
        &intent.input,
    );

    // Different parameter order should produce the same hash
    assert_eq!(
        hash1, hash2,
        "Hash must be deterministic regardless of parameter order"
    );
}

#[test]
fn test_planning_hash_different_intent_produces_different_hash() {
    let params = HashMap::new();
    let hash1 = crate::planning::application::pipeline_impl::compute_planning_hash(
        "template-id",
        &params,
        "read file",
    );
    let hash2 = crate::planning::application::pipeline_impl::compute_planning_hash(
        "template-id",
        &params,
        "write file",
    );
    assert_ne!(
        hash1, hash2,
        "Different intents must produce different hashes"
    );
}

#[test]
fn test_planning_hash_format() {
    let params = HashMap::new();
    let hash =
        crate::planning::application::pipeline_impl::compute_planning_hash("t", &params, "test");
    assert_eq!(
        hash.as_str().len(),
        64,
        "PlanningHash must be 64 hex characters"
    );
    assert!(
        hash.as_str().chars().all(|c| c.is_ascii_hexdigit()),
        "Hash must be hex"
    );
}

#[test]
fn test_planning_hash_different_templates_produce_different_hash() {
    let params = HashMap::new();
    let hash1 = crate::planning::application::pipeline_impl::compute_planning_hash(
        "template-a",
        &params,
        "test",
    );
    let hash2 = crate::planning::application::pipeline_impl::compute_planning_hash(
        "template-b",
        &params,
        "test",
    );
    assert_ne!(
        hash1, hash2,
        "Different template IDs must produce different hashes"
    );
}

// ---------------------------------------------------------------------------
// MockClassifier Tests
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
        None,
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
        None,
    );

    // planned_at should be set to now (within reasonable tolerance)
    let now = chrono::Utc::now();
    let diff = now - result.planned_at;
    assert!(diff.num_seconds() < 5, "planned_at should be recent");
}

// ---------------------------------------------------------------------------
// Pipeline Edge Cases
// ---------------------------------------------------------------------------

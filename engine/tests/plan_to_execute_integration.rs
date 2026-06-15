//! Integration test: plan pipeline type construction.
//!
//! Verifies key domain types can be constructed and their basic
//! invariants hold. Full pipeline integration tests require
//! wiring real classifier and template engine components.

use rigorix::planning::domain::classification::ClassifiedTemplate;
use rigorix::planning::domain::intent::UserIntent;
use rigorix::planning::domain::result::PlanningHash;

#[test]
fn test_user_intent_construction() {
    let intent = UserIntent::new("read src/lib.rs".to_string(), None);
    assert_eq!(intent.input, "read src/lib.rs");
    assert!(intent.execution_id.is_none());
}

#[test]
fn test_classified_template_construction() {
    let template = ClassifiedTemplate {
        template_id: "read-file".to_string(),
        confidence: 0.95,
        reasoning: "Matched 'read' keyword".to_string(),
        from_override: false,
    };
    assert_eq!(template.template_id, "read-file");
    assert!(template.confidence > 0.9);
}

#[test]
fn test_planning_hash_construction() {
    let hash = PlanningHash("abc123".to_string());
    assert!(!hash.0.is_empty());
    assert_eq!(hash.0, "abc123");
}

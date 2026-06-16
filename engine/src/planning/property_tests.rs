//! Deterministic property-style tests for the Planning Pipeline.
//!
//! Tests planning_hash determinism: same inputs always produce same hash.

#![cfg(test)]

use std::collections::HashMap;
use crate::planning::application::pipeline_impl::compute_planning_hash;

fn hash(intent: &str, template_id: &str, params: &HashMap<String, String>) -> String {
    compute_planning_hash(template_id, params, intent).0
}

#[test]
fn test_planning_hash_is_deterministic() {
    let test_cases = vec![
        ("read src/lib.rs", "read-file", "file", "src/lib.rs"),
        ("write to output", "write-file", "content", "hello"),
        ("list functions", "list-fn", "module", "utils"),
    ];

    for (intent, template_id, param_key, param_val) in test_cases {
        let mut params = HashMap::new();
        params.insert(param_key.to_string(), param_val.to_string());

        let h1 = hash(intent, template_id, &params);
        let h2 = hash(intent, template_id, &params);

        assert_eq!(
            h1, h2,
            "Same inputs must produce same hash for '{}'",
            intent
        );
    }
}

#[test]
fn test_planning_hash_differs_with_different_intents() {
    let params: HashMap<String, String> = HashMap::new();
    let h1 = hash("read file", "template-a", &params);
    let h2 = hash("write file", "template-a", &params);
    assert_ne!(h1, h2, "Different intents should produce different hashes");
}

#[test]
fn test_planning_hash_differs_with_different_templates() {
    let params: HashMap<String, String> = HashMap::new();
    let h1 = hash("read file", "template-a", &params);
    let h2 = hash("read file", "template-b", &params);
    assert_ne!(h1, h2, "Different templates should produce different hashes");
}

#[test]
fn test_planning_hash_differs_with_different_params() {
    let mut params1 = HashMap::new();
    params1.insert("file".to_string(), "a.rs".to_string());

    let mut params2 = HashMap::new();
    params2.insert("file".to_string(), "b.rs".to_string());

    let h1 = hash("read", "template-a", &params1);
    let h2 = hash("read", "template-a", &params2);
    assert_ne!(h1, h2, "Different params should produce different hashes");
}

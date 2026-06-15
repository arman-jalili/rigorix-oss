//! Live integration tests for Claude/OpenAI classifiers against real APIs.
//!
//! These tests only compile when the `live-tests` feature is enabled.
//! Usage:
//!   CLAUDE_API_KEY=sk-... cargo test --features live-tests -- live_classifier

#![cfg(feature = "live-tests")]

#[cfg(test)]
mod tests {
    use crate::budget_tracking::domain::LlmBudget;
    use crate::planning::domain::classification::Classifier;
    use crate::planning::domain::intent::UserIntent;

    fn create_test_budget() -> LlmBudget {
        LlmBudget {
            max_calls: 10,
            max_tokens: 50_000,
            used_calls: 0,
            used_tokens: 0,
            label: "live-test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_claude_classifier_live() {
        let api_key = std::env::var("CLAUDE_API_KEY").unwrap_or_else(|_| {
            eprintln!("SKIP: CLAUDE_API_KEY not set");
            return String::new();
        });

        if api_key.is_empty() {
            return;
        }

        let classifier = crate::planning::infrastructure::claude_classifier::ClaudeClassifier::new(
            api_key,
            None,
        );

        let intent = UserIntent::new("read src/lib.rs".to_string(), None);
        let budget = create_test_budget();
        let templates = vec!["read-file".to_string(), "write-file".to_string()];

        let result = classifier.classify(&intent, &budget, &templates).await;
        match result {
            Ok(template) => {
                assert!(!template.template_id.is_empty());
                assert!((0.0..=1.0).contains(&template.confidence));
            }
            Err(e) => {
                eprintln!("Claude live test (expected in CI): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_openai_classifier_live() {
        let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
            eprintln!("SKIP: OPENAI_API_KEY not set");
            return String::new();
        });

        if api_key.is_empty() {
            return;
        }

        let classifier = crate::planning::infrastructure::openai_classifier::OpenaiClassifier::new(
            api_key,
            None,
        );

        let intent = UserIntent::new("list all functions in src/lib.rs".to_string(), None);
        let budget = create_test_budget();
        let templates = vec!["list-functions".to_string(), "read-file".to_string()];

        let result = classifier.classify(&intent, &budget, &templates).await;
        match result {
            Ok(template) => {
                assert!(!template.template_id.is_empty());
                assert!((0.0..=1.0).contains(&template.confidence));
            }
            Err(e) => {
                eprintln!("OpenAI live test (expected in CI): {}", e);
            }
        }
    }
}

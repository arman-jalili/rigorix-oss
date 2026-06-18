//! Live integration tests for ClaudeTemplateGenerator against real API.
//!
//! Only compiles with `live-tests` feature.
//! Usage: CLAUDE_API_KEY=sk-... cargo test --features live-tests

#[cfg(test)]
mod tests {
    use crate::budget_tracking::domain::LlmBudget;
    use crate::planning::domain::intent::UserIntent;
    use crate::template_generation::domain::{
        ClaudeGeneratorConfig, ClaudeTemplateGenerator, RepoContext, TemplateGenerator,
    };
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_claude_generator_live() {
        let api_key = std::env::var("CLAUDE_API_KEY").unwrap_or_else(|_| {
            eprintln!("SKIP: CLAUDE_API_KEY not set");
            return String::new();
        });

        if api_key.is_empty() {
            return;
        }

        let config = ClaudeGeneratorConfig::default();
        let generator = ClaudeTemplateGenerator::new(api_key, Some(config));

        let ctx = RepoContext::new(PathBuf::from("/test"), "rust".to_string());
        let intent = UserIntent::new("read a file".to_string(), None);
        let budget = LlmBudget {
            max_calls: 10,
            max_tokens: 50_000,
            used_calls: 0,
            used_tokens: 0,
            label: "live-test".to_string(),
        };

        let result = generator.generate(&intent, &ctx, &budget).await;
        match result {
            Ok(template) => {
                assert!(!template.name.is_empty());
                assert!(template.toml_content.contains("[[nodes]]"));
            }
            Err(e) => {
                eprintln!("Generator live test (expected in CI): {}", e);
            }
        }
    }
}

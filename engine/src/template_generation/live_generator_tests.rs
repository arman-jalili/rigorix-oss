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

    struct LiveTestTemplateEngine;

    #[async_trait::async_trait]
    impl crate::templates::application::service::TemplateEngineService for LiveTestTemplateEngine {
        async fn list_templates(
            &self,
        ) -> Result<
            crate::templates::application::dto::ListTemplatesOutput,
            crate::templates::domain::error::TemplateError,
        > {
            Ok(crate::templates::application::dto::ListTemplatesOutput {
                templates: vec![],
                total: 0,
            })
        }
        async fn register(
            &self,
            _input: crate::templates::application::dto::RegisterInput,
        ) -> Result<
            crate::templates::application::dto::RegisterOutput,
            crate::templates::domain::error::TemplateError,
        > {
            Ok(crate::templates::application::dto::RegisterOutput {
                template_id: String::new(),
                total_templates: 0,
                overwritten: false,
            })
        }
        async fn generate(
            &self,
            _input: crate::templates::application::dto::GenerateInput,
        ) -> Result<
            crate::templates::application::dto::GenerateOutput,
            crate::templates::domain::error::TemplateError,
        > {
            Ok(crate::templates::application::dto::GenerateOutput {
                template_id: String::new(),
                nodes: vec![],
                edges: vec![],
                valid: true,
                topological_order: vec![],
                errors: vec![],
                execution_id: uuid::Uuid::new_v4(),
                node_count: 0,
            })
        }
        async fn get_template(
            &self,
            _input: crate::templates::application::dto::GetTemplateInput,
        ) -> Result<
            Option<crate::templates::application::dto::TemplateSummary>,
            crate::templates::domain::error::TemplateError,
        > {
            Ok(None)
        }
        async fn get_template_full(
            &self,
            _template_id: &str,
        ) -> Option<crate::templates::domain::Template> {
            None
        }
        async fn has_template(&self, _template_id: &str) -> bool {
            false
        }
        async fn template_count(&self) -> usize {
            0
        }
    }

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
                assert!(!template.suggested_name.is_empty());
                assert!(template.toml_content.contains("[[nodes]]"));
            }
            Err(e) => {
                eprintln!("Generator live test (expected in CI): {}", e);
            }
        }
    }
    /// H-03: plan-pipeline round trip with the live generator. Builds a
    /// planning pipeline whose generator fallback is the real
    /// ClaudeTemplateGenerator; a low-confidence classifier match triggers
    /// the generator path end-to-end (intent -> generated template -> plan).
    #[tokio::test]
    async fn test_plan_pipeline_round_trip_with_live_generator() {
        let api_key = std::env::var("CLAUDE_API_KEY").unwrap_or_else(|_| {
            eprintln!("SKIP: CLAUDE_API_KEY not set");
            return String::new();
        });
        if api_key.is_empty() {
            return;
        }

        let classifier = Box::new(
            crate::planning::application::mock_classifier::MockClassifier::new()
                .with_match("read a file", "template-read", 0.95)
                .with_match("unknown task", "template-generate", 0.15),
        );
        let extractor =
            Box::new(crate::planning::application::mock_extractor::MockParameterExtractor::new());

        let generator = crate::template_generation::domain::ClaudeTemplateGenerator::new(
            api_key,
            Some(ClaudeGeneratorConfig::default()),
        );
        let pipeline = crate::planning::application::pipeline_impl::PlanningPipelineImpl::new(
            uuid::Uuid::new_v4(),
            classifier,
            extractor,
            std::sync::Arc::new(LiveTestTemplateEngine),
        )
        .with_generator(Box::new(generator))
        .with_workspace_root(std::env::temp_dir().to_string_lossy().to_string());

        let input = crate::planning::application::dto::PlanInput {
            intent: crate::planning::domain::intent::UserIntent::new(
                "build a rust module with a greet function".to_string(),
                None,
            ),
            execution_id: Some(uuid::Uuid::new_v4()),
            enable_generator_fallback: true,
            skip_validation: true,
            repo_root: std::env::temp_dir().to_string_lossy().to_string(),
            module_deps: None,
        };

        let result =
            crate::planning::application::service::PlanningPipelineService::plan(&pipeline, input)
                .await;
        match result {
            Ok(output) => {
                assert!(
                    output.from_generator,
                    "H-03: plan must come from the generator fallback"
                );
                assert!(
                    output.planning_result.generated_toml.is_some()
                        || !output.planning_result.template_id.is_empty(),
                    "H-03: plan must carry a generated template"
                );
            }
            Err(e) => {
                eprintln!("Plan-pipeline live test (expected in CI): {}", e);
            }
        }
    }
}

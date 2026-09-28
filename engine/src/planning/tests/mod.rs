//! Unit tests for the Planning Pipeline bounded context.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: PlanningPipeline — Unit tests
//! Issue: issue-planningpipeline
//!
//! Tests cover the PlanningPipelineImpl orchestration, MockClassifier,
//! MockParameterExtractor, planning hash computation, error handling,
//! and edge cases.

pub(crate) use std::collections::HashMap;

pub(crate) use uuid::Uuid;

pub(crate) use crate::planning::application::dto::{
    ExtractParametersInput, GenerateGraphInput, PlanInput, PlanWithGraphInput,
    RequestClarificationInput, ValidatePlanInput,
};

pub(crate) use crate::planning::application::mock_classifier::MockClassifier;

pub(crate) use crate::planning::application::mock_extractor::MockParameterExtractor;

pub(crate) use crate::planning::application::pipeline_impl::PlanningPipelineImpl;

pub(crate) use crate::planning::application::service::PlanningPipelineService;

pub(crate) use crate::planning::domain::classification::Classifier;

pub(crate) use crate::planning::domain::extractor::ParameterExtractor;

pub(crate) use crate::planning::domain::intent::UserIntent;

pub(crate) use crate::planning::domain::result::{PlanningHash, PlanningResult};

pub(crate) use crate::template_generation::domain::{
    ClaudeGeneratorConfig, ClaudeTemplateGenerator, GeneratedTemplate, GeneratedTemplateCost,
    GeneratorError, InvalidSymbolReference, RepoContext, TemplateGenerator,
};

pub(crate) use super::domain::PlanningError;

// ---------------------------------------------------------------------------
// Helper: MockTemplateEngine for controlled testing
// ---------------------------------------------------------------------------

pub(crate) use crate::planning::application::service::SymbolValidationService;

pub(crate) use crate::planning::application::symbol_validation_impl::SymbolValidationServiceImpl;

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
    ) -> Result<
        crate::templates::application::dto::RegisterOutput,
        crate::templates::domain::TemplateError,
    > {
        Ok(crate::templates::application::dto::RegisterOutput {
            template_id: "mock-registered".to_string(),
            total_templates: 1,
            overwritten: false,
        })
    }

    async fn generate(
        &self,
        input: crate::templates::application::dto::GenerateInput,
    ) -> Result<
        crate::templates::application::dto::GenerateOutput,
        crate::templates::domain::TemplateError,
    > {
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

    async fn get_template_full(
        &self,
        _template_id: &str,
    ) -> Option<crate::templates::domain::Template> {
        None
    }

    async fn get_template(
        &self,
        _input: crate::templates::application::dto::GetTemplateInput,
    ) -> Result<
        Option<crate::templates::application::dto::TemplateSummary>,
        crate::templates::domain::TemplateError,
    > {
        Ok(None)
    }

    async fn list_templates(
        &self,
    ) -> Result<
        crate::templates::application::dto::ListTemplatesOutput,
        crate::templates::domain::TemplateError,
    > {
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
    PlanningPipelineImpl::new(
        execution_id,
        classifier,
        extractor,
        std::sync::Arc::new(MockTemplateEngine::new()),
    )
}

// ---------------------------------------------------------------------------
// Planning Hash Tests (via public helper)
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
        GeneratedTemplateCost {
            estimated_calls: 1,
            estimated_tokens: 200,
        }
    }
}

// ---------------------------------------------------------------------------
// TemplateGenerator Trait Tests
// ---------------------------------------------------------------------------

/// Mock symbol graph that returns found=true for symbols we've registered
struct MockSymbolGraph {
    symbols: std::collections::HashSet<String>,
}

#[async_trait::async_trait]
impl crate::repo_engine::application::service::SymbolGraphService for MockSymbolGraph {
    async fn add_symbol(
        &self,
        _input: crate::repo_engine::application::dto::AddSymbolInput,
    ) -> Result<
        crate::repo_engine::application::dto::AddSymbolOutput,
        crate::repo_engine::domain::RepoEngineError,
    > {
        unimplemented!()
    }

    async fn lookup_symbol(
        &self,
        input: crate::repo_engine::application::dto::LookupSymbolInput,
    ) -> Result<
        crate::repo_engine::application::dto::LookupSymbolOutput,
        crate::repo_engine::domain::RepoEngineError,
    > {
        let found = self.symbols.contains(&input.name);
        Ok(crate::repo_engine::application::dto::LookupSymbolOutput {
            symbol: None,
            references_from: vec![],
            references_to: vec![],
            found,
        })
    }

    async fn search_symbols(
        &self,
        _input: crate::repo_engine::application::dto::SearchSymbolsInput,
    ) -> Result<
        crate::repo_engine::application::dto::SearchSymbolsOutput,
        crate::repo_engine::domain::RepoEngineError,
    > {
        unimplemented!()
    }

    async fn symbols_by_file(
        &self,
        _input: crate::repo_engine::application::dto::SymbolsByFileInput,
    ) -> Result<
        crate::repo_engine::application::dto::SymbolsByFileOutput,
        crate::repo_engine::domain::RepoEngineError,
    > {
        unimplemented!()
    }

    async fn remove_symbol(
        &self,
        _name: &str,
    ) -> Result<bool, crate::repo_engine::domain::RepoEngineError> {
        unimplemented!()
    }

    async fn clear_graph(&self) -> Result<(), crate::repo_engine::domain::RepoEngineError> {
        unimplemented!()
    }

    async fn graph_stats(
        &self,
        _input: crate::repo_engine::application::dto::GraphStatsInput,
    ) -> Result<
        crate::repo_engine::application::dto::GraphStatsOutput,
        crate::repo_engine::domain::RepoEngineError,
    > {
        Ok(crate::repo_engine::application::dto::GraphStatsOutput {
            total_symbols: self.symbols.len(),
            total_indexed: self.symbols.len(),
            by_kind: std::collections::HashMap::new(),
            by_language: std::collections::HashMap::new(),
            max_capacity: 0,
            reference_count: 0,
        })
    }

    async fn add_reference(
        &self,
        _from: &str,
        _to: &str,
    ) -> Result<bool, crate::repo_engine::domain::RepoEngineError> {
        unimplemented!()
    }

    fn graph(&self) -> crate::repo_engine::domain::SharedSymbolGraph {
        crate::repo_engine::domain::SharedSymbolGraph::new()
    }
}

mod clarification;
mod errors;
mod hash;
mod mocks;
mod pipeline;

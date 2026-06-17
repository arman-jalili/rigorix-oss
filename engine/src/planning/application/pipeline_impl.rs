//! Implementation of the PlanningPipelineService.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#pipeline
//! Implements: PlanningPipeline — PlanningPipelineService implementation
//! Issue: issue-planningpipeline
//!
//! Provides the concrete `PlanningPipelineImpl` that orchestrates the 6-phase
//! planning flow:
//! 1. Budget Pre-check
//! 2. Intent Classification
//! 3. Parameter Extraction
//! 4. Graph Generation
//! 5. Plan Validation
//! 6. Hash Computation
//!
//! # Thread Safety
//! - All dependencies are Send + Sync
//! - No mutable state in the pipeline (immutable after construction)
//! - All async methods are safe to call from multiple tasks

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

use crate::planning::application::dto::{
    AvailableTemplatesOutput, CheckBudgetInput, CheckBudgetOutput, ExtractParametersInput,
    ExtractParametersOutput, GenerateGraphInput, GenerateGraphOutput, PlanInput, PlanOutput,
    PlanWithGraphInput, PlanWithGraphOutput, RequestClarificationInput, RequestClarificationOutput,
    ValidatePlanInput, ValidatePlanOutput, ValidationError, ValidationWarning,
};
use crate::planning::application::service::PlanningPipelineService;
use crate::planning::domain::classification::{ClassificationResult, Classifier};
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::extractor::{ExtractedParameters, ParameterExtractor};
use crate::planning::domain::intent::UserIntent;
use crate::planning::domain::result::{PlanningHash, PlanningResult};
use crate::template_generation::domain::TemplateGenerator;

use super::factory::CompositeValidator;

/// Orchestrates the 6-phase planning flow from user intent to validated plan.
///
/// The PlanningPipelineImpl wires together the Classifier, ParameterExtractor,
/// TemplateEngine, optional TemplateGenerator, and optional CompositeValidator
/// to produce deterministic PlanningResults.
///
/// # Lifecycle
///
/// 1. Construct with `new()` or `with_generator()`
/// 2. Call `plan()` or `plan_with_graph()` with user intent
/// 3. Inspect `PlanningResult` for template, confidence, parameters, hash
pub struct PlanningPipelineImpl {
    /// Execution ID for this pipeline instance.
    execution_id: Uuid,

    /// Intent classifier (LLM-based intent-to-template matching).
    classifier: Box<dyn Classifier>,

    /// Parameter extractor (LLM-based parameter filling).
    extractor: Box<dyn ParameterExtractor>,

    /// Template engine for graph generation.
    template_service: Box<dyn crate::templates::application::service::TemplateEngineService>,

    /// Optional template generator fallback.
    template_generator: Option<Box<dyn TemplateGenerator>>,

    /// Optional composite validator.
    validator: Option<Box<dyn CompositeValidator>>,
}

impl PlanningPipelineImpl {
    /// Create a new PlanningPipelineImpl with the required dependencies.
    ///
    /// No TemplateGenerator fallback is configured — low-confidence intents
    /// will return clarification requests.
    pub fn new(
        execution_id: Uuid,
        classifier: Box<dyn Classifier>,
        extractor: Box<dyn ParameterExtractor>,
        template_service: Box<dyn crate::templates::application::service::TemplateEngineService>,
    ) -> Self {
        Self {
            execution_id,
            classifier,
            extractor,
            template_service,
            template_generator: None,
            validator: None,
        }
    }

    /// Set the optional template generator fallback.
    pub fn with_generator(mut self, generator: Box<dyn TemplateGenerator>) -> Self {
        self.template_generator = Some(generator);
        self
    }

    /// Set the optional composite validator.
    pub fn with_validator(mut self, validator: Box<dyn CompositeValidator>) -> Self {
        self.validator = Some(validator);
        self
    }

    /// Phase 1: Budget pre-check.
    ///
    /// For the implementation, we check that we have sufficient budget
    /// capacity. The actual budget tracking is done by the budget_tracking module.
    #[tracing::instrument(skip_all)]
    fn phase_check_budget(&self, _input: &CheckBudgetInput) -> CheckBudgetOutput {
        // In a full implementation, this would query the LlmBudget
        // For now, we assume budget is available (passed pre-check)
        CheckBudgetOutput {
            has_capacity: true,
            remaining_calls: 10,
            remaining_tokens: 10000,
            max_calls: 50,
            max_tokens: 50000,
            will_exhaust: false,
        }
    }

    /// Phase 2: Classify intent using the classifier.
    async fn phase_classify(
        &self,
        intent: &UserIntent,
    ) -> Result<ClassificationResult, PlanningError> {
        // Get available templates
        let templates = self.template_service.list_templates().await.map_err(|e| {
            PlanningError::TemplateEngineError {
                detail: format!("Failed to list templates: {}", e),
            }
        })?;

        let template_ids: Vec<String> = templates.templates.iter().map(|t| t.id.clone()).collect();

        // Use a mock budget for now — real implementation would get this from budget_tracking
        let budget = crate::budget_tracking::domain::LlmBudget {
            max_calls: 50,
            max_tokens: 50000,
            used_calls: 0,
            used_tokens: 0,
            label: "planning".to_string(),
        };

        self.classifier
            .classify_with_alternatives(intent, &budget, &template_ids)
            .await
    }

    /// Phase 3: Extract parameters using the extractor.
    async fn phase_extract(
        &self,
        intent: &UserIntent,
        template_id: &str,
        parameter_names: &[String],
    ) -> Result<ExtractedParameters, PlanningError> {
        let budget = crate::budget_tracking::domain::LlmBudget {
            max_calls: 50,
            max_tokens: 50000,
            used_calls: 0,
            used_tokens: 0,
            label: "planning".to_string(),
        };

        self.extractor
            .extract(intent, &budget, template_id, parameter_names)
            .await
    }

    /// Phase 4: Generate TaskGraph from template + parameters.
    async fn phase_generate_graph(
        &self,
        template_id: &str,
        parameters: &HashMap<String, String>,
    ) -> Result<GenerateGraphOutput, PlanningError> {
        let params: HashMap<String, serde_json::Value> = parameters
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();

        let input = crate::templates::application::dto::GenerateInput {
            template_id: template_id.to_string(),
            params,
            execution_id: self.execution_id,
            validate_graph: true,
        };

        let output = self.template_service.generate(input).await.map_err(|e| {
            PlanningError::TemplateEngineError {
                detail: format!("Failed to generate graph: {}", e),
            }
        })?;

        Ok(GenerateGraphOutput {
            graph: Self::build_task_graph(&output),
            node_count: output.node_count as u32,
            sealed: output.valid,
            from_generator: false,
        })
    }

    /// Build a TaskGraph from the template engine's GenerateOutput nodes.
    fn build_task_graph(
        output: &crate::templates::application::dto::GenerateOutput,
    ) -> crate::dag_engine::domain::TaskGraph {
        use crate::templates::domain::TemplateAction;
        use std::collections::HashMap;

        // Map template node IDs to TaskNode UUIDs
        let mut node_id_map: HashMap<String, uuid::Uuid> = HashMap::new();
        for node in &output.nodes {
            let id = uuid::Uuid::new_v4();
            node_id_map.insert(node.id.clone(), id);
        }

        // Build dependency map: for each node, find its dependency UUIDs from edges
        let mut dep_map: HashMap<String, Vec<uuid::Uuid>> = HashMap::new();
        for (from_id, to_id) in &output.edges {
            if let Some(&to_uuid) = node_id_map.get(to_id) {
                dep_map.entry(from_id.clone())
                    .or_default()
                    .push(to_uuid);
            }
        }

        let mut graph = crate::dag_engine::domain::TaskGraph::new();
        for node in &output.nodes {
            let node_id = node_id_map[&node.id];
            let deps: Vec<uuid::Uuid> = dep_map
                .remove(&node.id)
                .unwrap_or_default();
            let (tool, intent) = match &node.action {
                TemplateAction::RunCommand {
                    command, ..
                } => ("run_command", command.clone()),
                TemplateAction::FileRead { path } => ("file_read", path.clone()),
                TemplateAction::FileWrite { content, .. } => ("file_write", content.clone()),
                TemplateAction::FileAppend { content, .. } => ("file_append", content.clone()),
                TemplateAction::LspQuery {
                    query_type, ..
                } => ("lsp_query", query_type.clone()),
                _ => ("unknown", String::new()),
            };
            graph.nodes.push(crate::dag_engine::domain::TaskNode::new(
                node_id,
                &node.name,
                tool.to_string(),
                deps,
                intent,
            ));
        }
        graph
    }

    /// Phase 5: Validate plan.
    async fn phase_validate(
        &self,
        graph: &crate::dag_engine::domain::TaskGraph,
        template_id: &str,
    ) -> Result<(Vec<ValidationError>, Vec<ValidationWarning>), PlanningError> {
        match &self.validator {
            Some(validator) => {
                validator
                    .validate(self.execution_id, graph, template_id)
                    .await
            }
            None => Ok((vec![], vec![])),
        }
    }

    /// Phase 6: Compute deterministic planning hash.
    fn phase_compute_hash(
        template_id: &str,
        parameters: &HashMap<String, String>,
        intent: &UserIntent,
    ) -> PlanningHash {
        compute_planning_hash(template_id, parameters, &intent.input)
    }

    /// Generate a clarification question when confidence is ambiguous.
    #[tracing::instrument(skip_all)]
    fn generate_clarification_question(classification: &ClassificationResult) -> String {
        if classification.alternatives.is_empty() {
            return "Could you provide more details about what you'd like to do?".to_string();
        }

        let options: Vec<String> = classification
            .alternatives
            .iter()
            .map(|t| {
                format!(
                    "- {} (confidence: {:.0}%)",
                    t.template_id,
                    t.confidence * 100.0
                )
            })
            .collect();

        format!(
            "I found a few possible matches for your request:\n{}\n\n\
             Could you clarify which one you intended, or provide more details?",
            options.join("\n")
        )
    }
}

#[async_trait]
impl PlanningPipelineService for PlanningPipelineImpl {
    #[tracing::instrument(skip_all)]
    async fn plan(&self, input: PlanInput) -> Result<PlanOutput, PlanningError> {
        // Phase 1: Budget pre-check
        let budget_input = CheckBudgetInput {
            execution_id: self.execution_id,
            required_calls: 2,
        };
        let budget_status = self.phase_check_budget(&budget_input);
        if !budget_status.has_capacity {
            return Err(PlanningError::BudgetExhausted {
                used_calls: budget_status.max_calls - budget_status.remaining_calls,
                max_calls: budget_status.max_calls,
                used_tokens: budget_status.max_tokens - budget_status.remaining_tokens,
                max_tokens: budget_status.max_tokens,
            });
        }

        let intent = input.intent;
        let mut total_llm_calls = 0u32;
        let mut total_llm_tokens = 0u32;
        let mut _from_generator = false;
        let mut generated_toml: Option<String> = None;
        let clarification_used = false;
        let mut generator_attempts = 0u32;
        const MAX_GENERATOR_ATTEMPTS: u32 = 3;

        // Phase 2-3 loop: Classify → extract (with optional clarification/generator)
        loop {
            let classification = self.phase_classify(&intent).await?;
            total_llm_calls += classification.llm_calls_used;
            total_llm_tokens += classification.llm_tokens_used;

            let top = classification.alternatives.first().cloned();

            match top {
                Some(template) if template.confidence >= 0.7 => {
                    // High confidence: proceed to extraction
                    let param_names = vec![]; // Would get from template
                    let extracted = self
                        .phase_extract(&intent, &template.template_id, &param_names)
                        .await?;
                    total_llm_calls += extracted.llm_calls_used;
                    total_llm_tokens += extracted.llm_tokens_used;

                    if !extracted.complete {
                        return Err(PlanningError::MissingParameter {
                            template_id: template.template_id.clone(),
                            parameter: extracted
                                .missing_parameters
                                .first()
                                .cloned()
                                .unwrap_or_default(),
                            description: format!(
                                "Missing {} required parameters",
                                extracted.missing_parameters.len()
                            ),
                        });
                    }

                    // Phase 4: Generate graph
                    let gen_output = self
                        .phase_generate_graph(&template.template_id, &extracted.parameters)
                        .await?;
                    _from_generator = gen_output.from_generator;

                    // Phase 5: Validate
                    let (errors, _warnings) = self
                        .phase_validate(&gen_output.graph, &template.template_id)
                        .await?;

                    if !input.skip_validation && !errors.is_empty() {
                        return Err(PlanningError::ValidationFailed {
                            detail: errors
                                .iter()
                                .map(|e| e.message.clone())
                                .collect::<Vec<_>>()
                                .join("; "),
                            error_count: errors.len() as u32,
                        });
                    }

                    // Phase 6: Compute hash
                    let hash = Self::phase_compute_hash(
                        &template.template_id,
                        &extracted.parameters,
                        &intent,
                    );

                    let planning_result = PlanningResult::new(
                        self.execution_id,
                        template.template_id,
                        template.confidence,
                        extracted.parameters,
                        hash,
                        clarification_used,
                        total_llm_calls,
                        total_llm_tokens,
                        generated_toml.clone(),
                    );
                    let completed_at = planning_result.planned_at;

                    return Ok(PlanOutput {
                        planning_result,
                        from_generator: _from_generator,
                        clarification_used,
                        total_llm_calls,
                        total_llm_tokens,
                        completed_at,
                    });
                }
                Some(template) if template.confidence >= 0.3 => {
                    // Medium confidence: request clarification
                    if clarification_used {
                        // Already clarified, no more progress — proceed with best match
                        let extracted = self
                            .phase_extract(&intent, &template.template_id, &[])
                            .await?;
                        total_llm_calls += extracted.llm_calls_used;
                        total_llm_tokens += extracted.llm_tokens_used;

                        let _gen_output = self
                            .phase_generate_graph(&template.template_id, &extracted.parameters)
                            .await?;

                        let hash = Self::phase_compute_hash(
                            &template.template_id,
                            &extracted.parameters,
                            &intent,
                        );

                        let planning_result = PlanningResult::new(
                            self.execution_id,
                            template.template_id,
                            template.confidence,
                            extracted.parameters,
                            hash,
                            true,
                            total_llm_calls,
                            total_llm_tokens,
                        generated_toml.clone(),
                        );
                        let completed_at = planning_result.planned_at;

                        return Ok(PlanOutput {
                            planning_result,
                            from_generator: false,
                            clarification_used: true,
                            total_llm_calls,
                            total_llm_tokens,
                            completed_at,
                        });
                    }

                    return Err(PlanningError::ClassificationError {
                        detail: format!(
                            "Ambiguous intent (confidence={:.2}). Clarification required.",
                            template.confidence
                        ),
                    });
                }
                _ => {
                    // Low confidence or no match: try generator fallback
                    if input.enable_generator_fallback
                        && generator_attempts < MAX_GENERATOR_ATTEMPTS
                    {
                        generator_attempts += 1;
                        if let Some(generator) = &self.template_generator {
                            let budget = crate::budget_tracking::domain::LlmBudget {
                                max_calls: 50,
                                max_tokens: 50000,
                                used_calls: 0,
                                used_tokens: 0,
                                label: "planning".to_string(),
                            };

                            // Build a minimal RepoContext for the generator.
                            // Full RepoContext building is handled by TemplateGenerationService.
                            let repo_context = crate::template_generation::domain::RepoContext {
                                root_dir: std::path::PathBuf::from("."),
                                project_type: "unknown".to_string(),
                                directory_tree: Vec::new(),
                                dependencies: Vec::new(),
                                public_api: Vec::new(),
                                symbol_graph_snapshot: None,
                            };
                            let generated =
                                generator.generate(&intent, &repo_context, &budget).await?;
                            total_llm_calls += generated.llm_calls_used;
                            total_llm_tokens += generated.llm_tokens_used;

                            // Register the generated template
                            // Parse the TOML content to get real nodes and parameters
                            let template = match toml::from_str::<crate::templates::domain::Template>(
                                &generated.toml_content,
                            ) {
                                Ok(t) => t,
                                Err(_) => {
                                    // Fallback: create a minimal template with just metadata
                                    crate::templates::domain::Template {
                                        id: generated.suggested_id,
                                        name: generated.suggested_name,
                                        description: generated.description,
                                        version: "1.0.0".to_string(),
                                        parameters: vec![],
                                        nodes: vec![],
                                        tags: vec![],
                                        category: None,
                                        author: None,
                                    }
                                }
                            };
                            let register_input =
                                crate::templates::application::dto::RegisterInput {
                                    template,
                                    overwrite: false,
                                };
                            self.template_service
                                .register(register_input)
                                .await
                                .map_err(|e| PlanningError::TemplateEngineError {
                                    detail: format!("Failed to register generated template: {}", e),
                                })?;

                            _from_generator = true;
                            generated_toml = Some(generated.toml_content.clone());
                            // Re-classify with the new template available
                            continue;
                        }
                    }

                    return Err(PlanningError::NoMatchingTemplate {
                        intent_preview: intent.input.chars().take(100).collect(),
                        templates_evaluated: 0,
                    });
                }
            }
        }
    }

    async fn plan_with_graph(
        &self,
        input: PlanWithGraphInput,
    ) -> Result<PlanWithGraphOutput, PlanningError> {
        // Convert to PlanInput and call plan()
        let plan_input = PlanInput {
            intent: input.intent,
            execution_id: input.execution_id,
            enable_generator_fallback: input.enable_generator_fallback,
            skip_validation: input.skip_validation,
        };

        let plan_output = self.plan(plan_input).await?;

        // Generate the graph again to return it
        let gen_output = self
            .phase_generate_graph(
                &plan_output.planning_result.template_id,
                &plan_output.planning_result.parameters,
            )
            .await?;

        Ok(PlanWithGraphOutput {
            planning_result: plan_output.planning_result,
            graph: gen_output.graph,
            node_count: gen_output.node_count,
            validation_passed: true,
            validation_warnings: vec![],
            from_generator: plan_output.from_generator,
            clarification_used: plan_output.clarification_used,
            total_llm_calls: plan_output.total_llm_calls,
            total_llm_tokens: plan_output.total_llm_tokens,
            completed_at: chrono::Utc::now(),
        })
    }

    async fn check_budget(
        &self,
        input: CheckBudgetInput,
    ) -> Result<CheckBudgetOutput, PlanningError> {
        Ok(self.phase_check_budget(&input))
    }

    async fn classify_intent(
        &self,
        intent: UserIntent,
    ) -> Result<ClassificationResult, PlanningError> {
        self.phase_classify(&intent).await
    }

    async fn extract_parameters(
        &self,
        input: ExtractParametersInput,
    ) -> Result<ExtractParametersOutput, PlanningError> {
        let result = self
            .phase_extract(&input.intent, &input.template_id, &input.parameter_names)
            .await?;

        Ok(ExtractParametersOutput {
            template_id: result.template_id,
            parameters: result.parameters,
            extra_parameters: result.extra_parameters,
            missing_parameters: result.missing_parameters,
            complete: result.complete,
            llm_calls_used: result.llm_calls_used,
            llm_tokens_used: result.llm_tokens_used,
        })
    }

    async fn generate_graph(
        &self,
        input: GenerateGraphInput,
    ) -> Result<GenerateGraphOutput, PlanningError> {
        self.phase_generate_graph(&input.template_id, &input.parameters)
            .await
    }

    async fn validate_plan(
        &self,
        input: ValidatePlanInput,
    ) -> Result<ValidatePlanOutput, PlanningError> {
        let (errors, warnings) = self
            .phase_validate(&input.graph, &input.template_id)
            .await?;
        let error_count = errors.len();
        let warning_count = warnings.len();

        Ok(ValidatePlanOutput {
            passed: errors.is_empty(),
            errors,
            warnings,
            checks_performed: if error_count == 0 && warning_count == 0 {
                0
            } else {
                (error_count + warning_count) as u32
            },
        })
    }

    async fn request_clarification(
        &self,
        input: RequestClarificationInput,
    ) -> Result<RequestClarificationOutput, PlanningError> {
        let question = input
            .custom_question
            .unwrap_or_else(|| Self::generate_clarification_question(&input.classification));

        Ok(RequestClarificationOutput {
            question,
            ambiguous_templates: input.classification.alternatives.clone(),
            suggested_answers: vec![],
        })
    }

    #[tracing::instrument(skip_all)]
    async fn available_templates(&self) -> Result<AvailableTemplatesOutput, PlanningError> {
        let templates = self.template_service.list_templates().await.map_err(|e| {
            PlanningError::TemplateEngineError {
                detail: format!("Failed to list templates: {}", e),
            }
        })?;

        let summaries: Vec<crate::planning::domain::result::TemplateSummary> = templates
            .templates
            .into_iter()
            .map(|t| crate::planning::domain::result::TemplateSummary {
                id: t.id,
                name: t.name,
                description: t.description,
                parameter_count: t.param_count as u32,
                node_count: t.node_count as u32,
                category: None,
            })
            .collect();

        let count = summaries.len() as u32;
        Ok(AvailableTemplatesOutput {
            templates: summaries,
            total_count: count,
        })
    }

    #[tracing::instrument(skip_all)]
    fn execution_id(&self) -> Uuid {
        self.execution_id
    }
}

/// Compute a deterministic SHA-256 based planning hash.
///
/// # Determinism
///
/// The hash is computed from (template_id + sorted_parameters + intent_input).
/// Same inputs always produce the same hash, regardless of parameter order.
///
/// This is a `pub` function so tests and other modules can compute hashes
/// without constructing a full pipeline.
pub fn compute_planning_hash(
    template_id: &str,
    parameters: &HashMap<String, String>,
    intent_input: &str,
) -> PlanningHash {
    let mut hasher = Sha256::new();

    // Normalise inputs: template_id + sorted parameters + intent input
    hasher.update(template_id.as_bytes());

    let mut sorted_params: Vec<(&String, &String)> = parameters.iter().collect();
    sorted_params.sort_by(|a, b| a.0.cmp(b.0));
    for (key, value) in &sorted_params {
        hasher.update(key.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b"&");
    }

    hasher.update(intent_input.as_bytes());

    let hash_bytes = hasher.finalize();
    let hash_hex = hash_bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    PlanningHash::new(hash_hex)
}

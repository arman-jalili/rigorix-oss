//! Implementation of the ValidationLoopService.
//!
//! @canonical .pi/architecture/modules/plan-validation.md
//! Implements: ValidationLoopService — concrete validation loop implementation
//! Issue: issue-validationloopservice
//!
//! Provides the concrete `ValidationLoopImpl` that orchestrates the
//! self-correcting plan→execute→verify→fix cycle. The loop:
//!
//! 1. Plans and executes a template from user intent
//! 2. Verifies the output against quality gates
//! 3. On failure: parses errors via FailureParser, augments context,
//!    retries only generative (llm_generate) nodes
//! 4. Returns validated template or structured failure report
//!
//! # Selective Retry
//!
//! Only `llm_generate` nodes are retried with augmented context.
//! Deterministic nodes (file_read, file_patch, compile_check, etc.)
//! have their outputs cached and reused across iterations.

use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

use crate::failure_parser::domain::TemplateFailure;
use crate::plan_validation::application::context_augmenter::ContextAugmenter;
use crate::plan_validation::application::dto::{
    AugmentIntentInput, ClassifyNodesInput, ClassifyNodesOutput, EvaluateIterationInput,
    EvaluateIterationOutput, RetryGenerativeNodesInput, RetryGenerativeNodesOutput,
    ValidateInput, ValidateOutput,
};
use crate::plan_validation::application::service::{
    QualityGateEvaluationService, ValidationLoopService,
};
use crate::plan_validation::domain::error::ValidationLoopError;
use crate::plan_validation::domain::loop_config::ValidationLoopConfig;
use crate::plan_validation::domain::outcome::ValidationOutcome;
use crate::plan_validation::domain::report::ValidationIterationReport;
use crate::plan_validation::domain::state::ValidationState;
use crate::templates::domain::Template;

/// Concrete implementation of the ValidationLoopService.
///
/// Orchestrates the full validation flow:
///
/// 1. Creates a ValidationState for the session
/// 2. Plans and executes the template
/// 3. Verifies against quality gates
/// 4. On failure: parses errors, augments intent, retries generative nodes
/// 5. Returns outcome with full validation report
///
/// # Thread Safety
///
/// - All dependencies are Send + Sync
/// - No mutable state in the service (immutable after construction)
/// - All async methods are safe to call from multiple tasks
pub struct ValidationLoopImpl {
    /// The validation loop configuration.
    config: ValidationLoopConfig,

    /// Service for evaluating quality gate satisfaction.
    quality_gate: Arc<dyn QualityGateEvaluationService>,
}

impl ValidationLoopImpl {
    /// Create a new ValidationLoopImpl.
    ///
    /// # Arguments
    /// * `config` — The validation loop configuration.
    /// * `quality_gate` — Service for evaluating quality gates.
    pub fn new(
        config: ValidationLoopConfig,
        quality_gate: Arc<dyn QualityGateEvaluationService>,
    ) -> Self {
        Self {
            config,
            quality_gate,
        }
    }

    /// Execute a single validation iteration: plan → execute → verify.
    ///
    /// Returns the template state and iteration result.
    async fn execute_iteration(
        &self,
        state: &ValidationState,
        iteration: u32,
    ) -> Result<(Template, EvaluateIterationOutput), ValidationLoopError> {
        // In a full implementation, this would:
        // 1. Call PlanningPipelineService::plan_with_graph() to get a plan
        // 2. Call ExecutionEngine to execute the plan
        // 3. Call QualityGateService to verify the result
        //
        // For now, this is a contract placeholder that demonstrates the flow.
        let eval = self
            .quality_gate
            .evaluate_iteration(EvaluateIterationInput {
                execution_id: state.execution_id,
                template: state.template.clone().unwrap_or_default(),
                required_quality: format!("{:?}", self.config.required_quality),
                iteration,
            })
            .await?;

        let template = state.template.clone().unwrap_or_default();
        Ok((template, eval))
    }

    /// Retry generative nodes with augmented context.
    async fn retry_with_augmented_context(
        &self,
        state: &mut ValidationState,
        failures: Vec<TemplateFailure>,
        tokens_used: u64,
    ) -> Result<(), ValidationLoopError> {
        // Augment the intent with failure context
        let augment_output = ContextAugmenter::augment_intent(AugmentIntentInput {
            intent: state.current_intent.clone(),
            failures: failures.clone(),
            failure_history: state.failure_history.clone(),
            iteration: state.iteration,
            max_iterations: self.config.max_iterations,
        });

        state.current_intent = augment_output.augmented_intent;
        state.record_failure(failures, tokens_used);

        Ok(())
    }
}

#[async_trait]
impl ValidationLoopService for ValidationLoopImpl {
    async fn validate(
        &self,
        input: ValidateInput,
    ) -> Result<ValidateOutput, ValidationLoopError> {
        let execution_id = input.execution_id.unwrap_or_else(Uuid::new_v4);
        let mut state = ValidationState::new(execution_id, input.intent);
        let start_time = std::time::Instant::now();

        for iteration in 1..=self.config.max_iterations {
            // Execute this iteration
            let (template, eval) = self.execute_iteration(&state, iteration).await?;

            let _iter_report = ValidationIterationReport::new(iteration)
                .with_failures(eval.failures.clone())
                .with_tokens(eval.llm_tokens_used)
                .with_duration(eval.duration_ms)
                .with_fixes(eval.fixes_applied.clone());

            if eval.passed || eval.failures.is_empty() {
                // Validation passed
                state.mark_succeeded();
                state.set_template(template.clone());
                return Ok(ValidateOutput {
                    execution_id,
                    outcome: ValidationOutcome::Validated,
                    validated_template: Some(template),
                    iterations: iteration,
                    cumulative_tokens: state.cumulative_tokens,
                    total_duration_ms: start_time.elapsed().as_millis() as u64,
                    total_failures: state.total_failures() as u32,
                });
            }

            // Check budget
            if state.cumulative_tokens >= self.config.max_cumulative_tokens {
                return Ok(ValidateOutput {
                    execution_id,
                    outcome: ValidationOutcome::BudgetExhausted,
                    validated_template: None,
                    iterations: iteration,
                    cumulative_tokens: state.cumulative_tokens,
                    total_duration_ms: start_time.elapsed().as_millis() as u64,
                    total_failures: state.total_failures() as u32,
                });
            }

            if iteration < self.config.max_iterations {
                // Augment context and retry
                self.retry_with_augmented_context(
                    &mut state,
                    eval.failures,
                    eval.llm_tokens_used,
                )
                .await?;
            }
        }

        // All retries exhausted
        Ok(ValidateOutput {
            execution_id,
            outcome: ValidationOutcome::Failed,
            validated_template: None,
            iterations: self.config.max_iterations,
            cumulative_tokens: state.cumulative_tokens,
            total_duration_ms: start_time.elapsed().as_millis() as u64,
            total_failures: state.total_failures() as u32,
        })
    }

    async fn classify_nodes(
        &self,
        input: ClassifyNodesInput,
    ) -> Result<ClassifyNodesOutput, ValidationLoopError> {
        let mut generative = Vec::new();
        let mut deterministic = Vec::new();

        for node in &input.template.nodes {
            use crate::templates::domain::TemplateAction;
            match &node.action {
                // RunCommand nodes that generate LLM content are generative
                TemplateAction::RunCommand { command, .. }
                    if command.contains("llm_generate")
                        || command.contains("llm-generate")
                        || command.contains("generate") =>
                {
                    generative.push(node.id.clone());
                }
                // All other actions are deterministic
                _ => {
                    deterministic.push(node.id.clone());
                }
            }
        }

        Ok(ClassifyNodesOutput {
            generative,
            deterministic,
            total_nodes: input.template.nodes.len() as u32,
        })
    }

    async fn retry_generative_nodes(
        &self,
        input: RetryGenerativeNodesInput,
    ) -> Result<RetryGenerativeNodesOutput, ValidationLoopError> {
        // Classify the template nodes
        let classification = self
            .classify_nodes(ClassifyNodesInput {
                template: input.previous_template.clone(),
            })
            .await?;

        // In a full implementation, this would:
        // 1. Augment context with failure analysis (already done by caller)
        // 2. Re-execute only generative nodes via LLM
        // 3. Reassemble template with cached deterministic outputs
        //
        // For now, create a new template with the same structure.
        let retried_count = classification.generative.len() as u32;
        let skipped_count = classification.deterministic.len() as u32;

        Ok(RetryGenerativeNodesOutput {
            template: input.previous_template,
            retried_count,
            skipped_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_validation::domain::loop_config::ValidationLoopConfig;
    use crate::planning::domain::intent::UserIntent;

    struct MockQualityGate;

    #[async_trait]
    impl QualityGateEvaluationService for MockQualityGate {
        async fn evaluate_iteration(
            &self,
            _input: EvaluateIterationInput,
        ) -> Result<EvaluateIterationOutput, ValidationLoopError> {
            Ok(EvaluateIterationOutput {
                passed: true,
                failures: vec![],
                llm_tokens_used: 0,
                duration_ms: 0,
                fixes_applied: vec![],
            })
        }

        async fn check_budget(
            &self,
            _cumulative_tokens: u64,
            _max_tokens: u64,
        ) -> Result<bool, ValidationLoopError> {
            Ok(true)
        }
    }

    #[tokio::test]
    async fn test_validate_passes_first_iteration() {
        let quality_gate = Arc::new(MockQualityGate);
        let service = ValidationLoopImpl::new(ValidationLoopConfig::default(), quality_gate);

        let input = ValidateInput {
            intent: UserIntent::new("test intent".into(), None),
            config: ValidationLoopConfig::default(),
            execution_id: None,
            existing_template: None,
        };

        let result = service.validate(input).await.unwrap();
        assert!(result.outcome.is_validated());
        assert_eq!(result.iterations, 1);
        assert_eq!(result.total_failures, 0);
    }

    #[tokio::test]
    async fn test_classify_nodes_empty_template() {
        let quality_gate = Arc::new(MockQualityGate);
        let service = ValidationLoopImpl::new(ValidationLoopConfig::default(), quality_gate);

        let template = Template::default();
        let result = service
            .classify_nodes(ClassifyNodesInput { template })
            .await
            .unwrap();

        assert_eq!(result.generative.len(), 0);
        assert_eq!(result.deterministic.len(), 0);
        assert_eq!(result.total_nodes, 0);
    }

    #[tokio::test]
    async fn test_retry_generative_nodes_empty() {
        let quality_gate = Arc::new(MockQualityGate);
        let service = ValidationLoopImpl::new(ValidationLoopConfig::default(), quality_gate);

        let input = RetryGenerativeNodesInput {
            execution_id: Uuid::new_v4(),
            previous_template: Template::default(),
            failures: vec![],
            source_context: crate::failure_parser::domain::SourceContext::empty(),
        };

        let result = service.retry_generative_nodes(input).await.unwrap();
        assert_eq!(result.retried_count, 0);
        assert_eq!(result.skipped_count, 0);
    }

    #[tokio::test]
    async fn test_validate_with_execution_id() {
        let quality_gate = Arc::new(MockQualityGate);
        let service = ValidationLoopImpl::new(ValidationLoopConfig::default(), quality_gate);

        let exec_id = Uuid::new_v4();
        let input = ValidateInput {
            intent: UserIntent::new("test".into(), Some(exec_id)),
            config: ValidationLoopConfig::default(),
            execution_id: Some(exec_id),
            existing_template: None,
        };

        let result = service.validate(input).await.unwrap();
        assert_eq!(result.execution_id, exec_id);
    }
}

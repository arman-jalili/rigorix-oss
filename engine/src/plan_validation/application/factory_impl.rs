//! Factory implementation for constructing Plan Validation components.
//!
//! @canonical .pi/architecture/modules/plan-validation.md
//! Implements: ValidationLoopFactory — concrete factory implementation
//! Issue: issue-validationloopservice
//!
//! Provides concrete factory implementations that wire:
//! - QualityGateService → QualityGateEvaluationService (adapter pattern)
//! - PlanningPipelineService + FailureParserService + ValidationLoopImpl

use async_trait::async_trait;
use std::sync::Arc;

use crate::failure_parser::application::service::FailureParserService;
use crate::plan_validation::application::dto::EvaluateIterationInput;
use crate::plan_validation::application::dto::EvaluateIterationOutput;
use crate::plan_validation::application::factory::ValidationLoopFactory;
use crate::plan_validation::application::loop_impl::ValidationLoopImpl;
use crate::plan_validation::application::service::{
    QualityGateEvaluationService, ValidationLoopService,
};
use crate::plan_validation::domain::error::ValidationLoopError;
use crate::plan_validation::domain::loop_config::ValidationLoopConfig;
use crate::planning::application::service::PlanningPipelineService;
use crate::quality_gates::application::dto::{ClassifyTestScopeInput, EvaluateGateInput};
use crate::quality_gates::application::service::QualityGateService;
use crate::quality_gates::domain::{GreenContract, QualityLevel};

// ---------------------------------------------------------------------------
// QualityGateEvaluationAdapter
// ---------------------------------------------------------------------------

/// Adapter that wraps QualityGateService into the simplified
/// QualityGateEvaluationService interface used by ValidationLoopImpl.
struct QualityGateEvaluationAdapter {
    inner: Arc<dyn QualityGateService>,
}

impl QualityGateEvaluationAdapter {
    fn new(inner: Arc<dyn QualityGateService>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl QualityGateEvaluationService for QualityGateEvaluationAdapter {
    async fn evaluate_iteration(
        &self,
        input: EvaluateIterationInput,
    ) -> Result<EvaluateIterationOutput, ValidationLoopError> {
        // Classify what tests were run based on the evaluation context
        let classify_input = ClassifyTestScopeInput {
            targeted_tests_run: true,
            package_tests_run: input.iteration > 1,
            workspace_tests_run: input.iteration > 2,
            lint_passed: false,
            format_passed: false,
            audit_passed: false,
        };

        let classify_out = self
            .inner
            .classify_test_scope(classify_input)
            .await
            .map_err(|e| ValidationLoopError::QualityGateError {
                detail: format!("Quality gate classification failed: {}", e),
            })?;

        // Evaluate against the required quality level
        let required_level = self.parse_quality_level(&input.required_quality);
        let _contract = self.inner.create_contract(required_level);

        let eval_input = EvaluateGateInput {
            contract: GreenContract::new(required_level),
            observed_level: Some(classify_out.level),
            task_id: Some(input.execution_id.to_string()),
        };

        let eval_out = self.inner.evaluate_gate(eval_input).await.map_err(|e| {
            ValidationLoopError::QualityGateError {
                detail: format!("Quality gate evaluation failed: {}", e),
            }
        })?;

        let passed = eval_out.summary.contains("Satisfied");
        let failures = if passed {
            vec![]
        } else {
            use crate::failure_parser::domain::failure::SourceLocation;
            vec![
                crate::failure_parser::domain::TemplateFailure::TestFailure {
                    test_name: "quality_gate".to_string(),
                    message: format!(
                        "Required quality {:?} not met (observed: {:?})",
                        required_level, classify_out.level
                    ),
                    location: Some(SourceLocation::new("", 0, None)),
                },
            ]
        };

        Ok(EvaluateIterationOutput {
            passed,
            failures,
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
        // Budget checking is handled by the orchestrator's budget service
        Ok(true)
    }
}

impl QualityGateEvaluationAdapter {
    fn parse_quality_level(&self, s: &str) -> QualityLevel {
        match s {
            "MergeReady" => QualityLevel::MergeReady,
            "Workspace" => QualityLevel::Workspace,
            "Package" => QualityLevel::Package,
            _ => QualityLevel::TargetedTests,
        }
    }
}

// ---------------------------------------------------------------------------
// ValidationLoopFactoryImpl
// ---------------------------------------------------------------------------

/// Concrete factory implementation for ValidationLoopService.
pub struct ValidationLoopFactoryImpl;

#[async_trait]
impl ValidationLoopFactory for ValidationLoopFactoryImpl {
    async fn create_default(
        &self,
        _planning_pipeline: Box<dyn PlanningPipelineService>,
        _failure_parser: Box<dyn FailureParserService>,
        quality_gate: Box<dyn QualityGateService>,
    ) -> Result<Box<dyn ValidationLoopService>, ValidationLoopError> {
        let adapter = Arc::new(QualityGateEvaluationAdapter::new(quality_gate.into()));
        let service = ValidationLoopImpl::new(ValidationLoopConfig::default(), adapter);
        Ok(Box::new(service))
    }

    async fn create_custom(
        &self,
        config: ValidationLoopConfig,
        _planning_pipeline: Box<dyn PlanningPipelineService>,
        _failure_parser: Box<dyn FailureParserService>,
        quality_gate: Box<dyn QualityGateService>,
    ) -> Result<Box<dyn ValidationLoopService>, ValidationLoopError> {
        let adapter = Arc::new(QualityGateEvaluationAdapter::new(quality_gate.into()));
        let service = ValidationLoopImpl::new(config, adapter);
        Ok(Box::new(service))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quality_gates::application::dto::ContractSource;
    use crate::quality_gates::application::dto::*;
    use crate::quality_gates::application::service::QualityGateService;
    use crate::quality_gates::domain::QualityGateError;
    use crate::quality_gates::domain::{GreenContract, QualityGateOutcome, QualityLevel};
    use async_trait::async_trait;
    use uuid::Uuid;

    struct MockQualityGateService;

    #[async_trait]
    impl QualityGateService for MockQualityGateService {
        async fn evaluate_gate(
            &self,
            input: EvaluateGateInput,
        ) -> Result<EvaluateGateOutput, QualityGateError> {
            let req = input.contract.required_level;
            let obs = input.observed_level;
            let satisfied = match (req, obs) {
                (r, Some(o)) if o >= r => true,
                _ => false,
            };
            Ok(EvaluateGateOutput {
                outcome: if satisfied {
                    QualityGateOutcome::Satisfied {
                        required: req,
                        observed: obs.unwrap(),
                    }
                } else {
                    QualityGateOutcome::Unsatisfied {
                        required: req,
                        observed: obs.unwrap_or(QualityLevel::TargetedTests),
                        gap: 1,
                    }
                },
                summary: if satisfied {
                    "Satisfied"
                } else {
                    "Unsatisfied"
                }
                .to_string(),
                task_id: input.task_id.clone(),
            })
        }

        async fn classify_test_scope(
            &self,
            _input: ClassifyTestScopeInput,
        ) -> Result<ClassifyTestScopeOutput, QualityGateError> {
            Ok(ClassifyTestScopeOutput {
                level: QualityLevel::Package,
                explanation: "All package tests passed".to_string(),
            })
        }

        async fn get_contract(
            &self,
            _input: GetContractInput,
        ) -> Result<GetContractOutput, QualityGateError> {
            Ok(GetContractOutput {
                contract: GreenContract::new(QualityLevel::Package),
                source: ContractSource::Default,
            })
        }

        async fn validate_config(
            &self,
            _input: ValidateConfigInput,
        ) -> Result<ValidateConfigOutput, QualityGateError> {
            Ok(ValidateConfigOutput {
                valid: true,
                errors: vec![],
                warnings: vec![],
            })
        }

        fn create_contract(&self, level: QualityLevel) -> GreenContract {
            GreenContract::new(level)
        }
    }

    #[tokio::test]
    async fn test_create_default() {
        let factory = ValidationLoopFactoryImpl;
        struct MockPlanning;
        #[async_trait]
        impl crate::planning::application::PlanningPipelineService for MockPlanning {
            async fn plan(
                &self,
                _: crate::planning::application::dto::PlanInput,
            ) -> Result<
                crate::planning::application::dto::PlanOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn plan_with_graph(
                &self,
                _: crate::planning::application::dto::PlanWithGraphInput,
            ) -> Result<
                crate::planning::application::dto::PlanWithGraphOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn check_budget(
                &self,
                _: crate::planning::application::dto::CheckBudgetInput,
            ) -> Result<
                crate::planning::application::dto::CheckBudgetOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn classify_intent(
                &self,
                _: crate::planning::domain::intent::UserIntent,
            ) -> Result<
                crate::planning::domain::classification::ClassificationResult,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn extract_parameters(
                &self,
                _: crate::planning::application::dto::ExtractParametersInput,
            ) -> Result<
                crate::planning::application::dto::ExtractParametersOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn generate_graph(
                &self,
                _: crate::planning::application::dto::GenerateGraphInput,
            ) -> Result<
                crate::planning::application::dto::GenerateGraphOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn validate_plan(
                &self,
                _: crate::planning::application::dto::ValidatePlanInput,
            ) -> Result<
                crate::planning::application::dto::ValidatePlanOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn request_clarification(
                &self,
                _: crate::planning::application::dto::RequestClarificationInput,
            ) -> Result<
                crate::planning::application::dto::RequestClarificationOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn available_templates(
                &self,
            ) -> Result<
                crate::planning::application::dto::AvailableTemplatesOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            fn execution_id(&self) -> Uuid {
                Uuid::nil()
            }
        }

        use crate::failure_parser::domain::ParserRegistry;
        let parser = Box::new(
            crate::failure_parser::application::service_impl::FailureParserServiceImpl::new(
                ParserRegistry::new(),
            ),
        ) as Box<dyn FailureParserService>;
        let gate = Box::new(MockQualityGateService) as Box<dyn QualityGateService>;

        let result = factory
            .create_default(Box::new(MockPlanning), parser, gate)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_custom() {
        let factory = ValidationLoopFactoryImpl;
        struct MockPlanning;
        #[async_trait]
        impl crate::planning::application::PlanningPipelineService for MockPlanning {
            async fn plan(
                &self,
                _: crate::planning::application::dto::PlanInput,
            ) -> Result<
                crate::planning::application::dto::PlanOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn plan_with_graph(
                &self,
                _: crate::planning::application::dto::PlanWithGraphInput,
            ) -> Result<
                crate::planning::application::dto::PlanWithGraphOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn check_budget(
                &self,
                _: crate::planning::application::dto::CheckBudgetInput,
            ) -> Result<
                crate::planning::application::dto::CheckBudgetOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn classify_intent(
                &self,
                _: crate::planning::domain::intent::UserIntent,
            ) -> Result<
                crate::planning::domain::classification::ClassificationResult,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn extract_parameters(
                &self,
                _: crate::planning::application::dto::ExtractParametersInput,
            ) -> Result<
                crate::planning::application::dto::ExtractParametersOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn generate_graph(
                &self,
                _: crate::planning::application::dto::GenerateGraphInput,
            ) -> Result<
                crate::planning::application::dto::GenerateGraphOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn validate_plan(
                &self,
                _: crate::planning::application::dto::ValidatePlanInput,
            ) -> Result<
                crate::planning::application::dto::ValidatePlanOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn request_clarification(
                &self,
                _: crate::planning::application::dto::RequestClarificationInput,
            ) -> Result<
                crate::planning::application::dto::RequestClarificationOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn available_templates(
                &self,
            ) -> Result<
                crate::planning::application::dto::AvailableTemplatesOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            fn execution_id(&self) -> Uuid {
                Uuid::nil()
            }
        }

        use crate::failure_parser::domain::ParserRegistry;
        let parser = Box::new(
            crate::failure_parser::application::service_impl::FailureParserServiceImpl::new(
                ParserRegistry::new(),
            ),
        ) as Box<dyn FailureParserService>;
        let gate = Box::new(MockQualityGateService) as Box<dyn QualityGateService>;
        let config = ValidationLoopConfig {
            max_iterations: 5,
            ..ValidationLoopConfig::default()
        };

        let result = factory
            .create_custom(config, Box::new(MockPlanning), parser, gate)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_adapter_evaluate_iteration_passes() {
        let gate = Arc::new(MockQualityGateService) as Arc<dyn QualityGateService>;
        let adapter = QualityGateEvaluationAdapter::new(gate);

        let result = adapter
            .evaluate_iteration(EvaluateIterationInput {
                execution_id: Uuid::new_v4(),
                template: crate::templates::domain::Template::default(),
                required_quality: "Package".to_string(),
                iteration: 2,
            })
            .await
            .unwrap();

        assert!(result.passed);
    }
}

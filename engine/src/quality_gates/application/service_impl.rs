//! Concrete implementation of `QualityGateService`.
//!
//! @canonical .pi/architecture/modules/quality-gates.md#service
//! Implements: QualityGateService — quality gate evaluation and scope classification
//! Issue: #451, #452, #453
//!
//! Handles evaluating GreenContracts against observed quality levels,
//! classifying test scopes, and managing quality gate configurations.

use async_trait::async_trait;

use crate::quality_gates::domain::{
    GreenContract, QualityGateConfig, QualityGateError, QualityLevel,
};

use super::dto::{
    ClassifyTestScopeInput, ClassifyTestScopeOutput, ContractSource, EvaluateGateInput,
    EvaluateGateOutput, GetContractInput, GetContractOutput, ValidateConfigInput,
    ValidateConfigOutput,
};
use super::service::QualityGateService;

/// Concrete implementation of `QualityGateService`.
///
/// Uses a `QualityGateConfig` to determine contracts for tasks/templates.
/// Provides test scope classification logic that maps execution flags to
/// `QualityLevel` values.
pub struct QualityGateServiceImpl {
    /// The quality gate configuration (default level + template overrides).
    config: QualityGateConfig,
}

impl QualityGateServiceImpl {
    /// Create a new `QualityGateServiceImpl` with the given configuration.
    pub fn new(config: QualityGateConfig) -> Self {
        Self { config }
    }

    /// Create a new `QualityGateServiceImpl` with default configuration.
    pub fn new_default() -> Self {
        Self {
            config: QualityGateConfig::default(),
        }
    }
}

#[async_trait]
impl QualityGateService for QualityGateServiceImpl {
    async fn evaluate_gate(
        &self,
        input: EvaluateGateInput,
    ) -> Result<EvaluateGateOutput, QualityGateError> {
        let outcome = input.contract.evaluate(input.observed_level);
        let summary = outcome.summary();

        Ok(EvaluateGateOutput {
            outcome,
            summary,
            task_id: input.task_id,
        })
    }

    async fn classify_test_scope(
        &self,
        input: ClassifyTestScopeInput,
    ) -> Result<ClassifyTestScopeOutput, QualityGateError> {
        let (level, explanation) = if input.workspace_tests_run
            && input.lint_passed
            && input.format_passed
            && input.audit_passed
        {
            (
                QualityLevel::MergeReady,
                "Workspace tests passed with lint, format, and audit all green".to_string(),
            )
        } else if input.workspace_tests_run {
            (
                QualityLevel::Workspace,
                "Workspace-level tests passed".to_string(),
            )
        } else if input.package_tests_run {
            (
                QualityLevel::Package,
                "Package-level tests passed".to_string(),
            )
        } else if input.targeted_tests_run {
            (
                QualityLevel::TargetedTests,
                "Targeted tests passed".to_string(),
            )
        } else {
            (
                QualityLevel::TargetedTests,
                "No test results available — defaulting to targeted tests level".to_string(),
            )
        };

        Ok(ClassifyTestScopeOutput { level, explanation })
    }

    async fn get_contract(
        &self,
        input: GetContractInput,
    ) -> Result<GetContractOutput, QualityGateError> {
        // Check task-level override first
        if let Some(ref _task_id) = input.task_id {
            // In a full implementation, this would check per-task overrides.
            // For now, fall through to template/default.
        }

        // Check template-level override
        if let Some(ref template_name) = input.template_name {
            if let Some(level) = self.config.required_level_for_template(template_name) {
                return Ok(GetContractOutput {
                    contract: GreenContract::new(level),
                    source: ContractSource::TemplateOverride {
                        template_name: template_name.clone(),
                    },
                });
            }
        }

        // Fall back to default
        Ok(GetContractOutput {
            contract: GreenContract::new(self.config.default_required_level),
            source: ContractSource::Default,
        })
    }

    async fn validate_config(
        &self,
        input: ValidateConfigInput,
    ) -> Result<ValidateConfigOutput, QualityGateError> {
        let mut errors: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        let config = &input.config;

        // Validate default level is one of the known levels
        match config.default_required_level {
            QualityLevel::TargetedTests
            | QualityLevel::Package
            | QualityLevel::Workspace
            | QualityLevel::MergeReady => {}
        }

        // Validate template overrides
        for (template, level) in &config.template_overrides {
            if template.trim().is_empty() {
                errors.push("Template name must not be empty".to_string());
            }
            match level {
                QualityLevel::TargetedTests => {
                    warnings.push(format!(
                        "Template '{}' has TargetedTests level — this will never fail a gate",
                        template
                    ));
                }
                QualityLevel::MergeReady => {
                    warnings.push(format!(
                        "Template '{}' has MergeReady level — this is the strictest gate",
                        template
                    ));
                }
                _ => {}
            }
        }

        Ok(ValidateConfigOutput {
            valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    fn create_contract(&self, level: QualityLevel) -> GreenContract {
        GreenContract::new(level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_config() -> QualityGateConfig {
        let mut config = QualityGateConfig::new(QualityLevel::Package);
        let mut overrides = HashMap::new();
        overrides.insert("hotfix".to_string(), QualityLevel::MergeReady);
        overrides.insert("refactor".to_string(), QualityLevel::Workspace);
        config.template_overrides = overrides;
        config
    }

    fn test_service() -> QualityGateServiceImpl {
        QualityGateServiceImpl::new(test_config())
    }

    #[tokio::test]
    async fn test_evaluate_gate_satisfied() {
        let service = test_service();
        let contract = GreenContract::new(QualityLevel::Workspace);
        let output = service
            .evaluate_gate(EvaluateGateInput {
                contract,
                observed_level: Some(QualityLevel::Workspace),
                task_id: None,
            })
            .await
            .unwrap();

        assert!(output.outcome.is_satisfied());
        assert!(output.summary.contains("Satisfied"));
    }

    #[tokio::test]
    async fn test_evaluate_gate_unsatisfied() {
        let service = test_service();
        let contract = GreenContract::new(QualityLevel::Workspace);
        let output = service
            .evaluate_gate(EvaluateGateInput {
                contract,
                observed_level: Some(QualityLevel::TargetedTests),
                task_id: Some("task-1".to_string()),
            })
            .await
            .unwrap();

        assert!(output.outcome.is_unsatisfied());
        assert_eq!(output.task_id, Some("task-1".to_string()));
    }

    #[tokio::test]
    async fn test_classify_merge_ready() {
        let service = test_service();
        let output = service
            .classify_test_scope(ClassifyTestScopeInput {
                targeted_tests_run: true,
                package_tests_run: true,
                workspace_tests_run: true,
                lint_passed: true,
                format_passed: true,
                audit_passed: true,
            })
            .await
            .unwrap();

        assert_eq!(output.level, QualityLevel::MergeReady);
    }

    #[tokio::test]
    async fn test_classify_workspace() {
        let service = test_service();
        let output = service
            .classify_test_scope(ClassifyTestScopeInput {
                targeted_tests_run: true,
                package_tests_run: true,
                workspace_tests_run: true,
                lint_passed: false,
                format_passed: false,
                audit_passed: false,
            })
            .await
            .unwrap();

        assert_eq!(output.level, QualityLevel::Workspace);
    }

    #[tokio::test]
    async fn test_classify_package() {
        let service = test_service();
        let output = service
            .classify_test_scope(ClassifyTestScopeInput {
                targeted_tests_run: true,
                package_tests_run: true,
                workspace_tests_run: false,
                lint_passed: false,
                format_passed: false,
                audit_passed: false,
            })
            .await
            .unwrap();

        assert_eq!(output.level, QualityLevel::Package);
    }

    #[tokio::test]
    async fn test_classify_targeted() {
        let service = test_service();
        let output = service
            .classify_test_scope(ClassifyTestScopeInput {
                targeted_tests_run: true,
                package_tests_run: false,
                workspace_tests_run: false,
                lint_passed: false,
                format_passed: false,
                audit_passed: false,
            })
            .await
            .unwrap();

        assert_eq!(output.level, QualityLevel::TargetedTests);
    }

    #[tokio::test]
    async fn test_classify_no_tests() {
        let service = test_service();
        let output = service
            .classify_test_scope(ClassifyTestScopeInput {
                targeted_tests_run: false,
                package_tests_run: false,
                workspace_tests_run: false,
                lint_passed: false,
                format_passed: false,
                audit_passed: false,
            })
            .await
            .unwrap();

        assert_eq!(output.level, QualityLevel::TargetedTests);
    }

    #[tokio::test]
    async fn test_get_contract_default() {
        let service = QualityGateServiceImpl::new_default();
        let output = service
            .get_contract(GetContractInput {
                template_name: None,
                task_id: None,
            })
            .await
            .unwrap();

        assert_eq!(
            output.contract.required_level,
            QualityLevel::Package
        );
        assert!(matches!(output.source, ContractSource::Default));
    }

    #[tokio::test]
    async fn test_get_contract_template_override() {
        let service = test_service();
        let output = service
            .get_contract(GetContractInput {
                template_name: Some("hotfix".to_string()),
                task_id: None,
            })
            .await
            .unwrap();

        assert_eq!(
            output.contract.required_level,
            QualityLevel::MergeReady
        );
        assert!(
            matches!(output.source, ContractSource::TemplateOverride { .. })
        );
    }

    #[tokio::test]
    async fn test_get_contract_unknown_template_falls_back() {
        let service = test_service();
        let output = service
            .get_contract(GetContractInput {
                template_name: Some("unknown".to_string()),
                task_id: None,
            })
            .await
            .unwrap();

        assert_eq!(
            output.contract.required_level,
            QualityLevel::Package
        ); // default
    }

    #[tokio::test]
    async fn test_validate_config_valid() {
        let service = test_service();
        let config = QualityGateConfig::new(QualityLevel::Workspace);
        let output = service
            .validate_config(ValidateConfigInput { config })
            .await
            .unwrap();
        assert!(output.valid);
    }

    #[tokio::test]
    async fn test_validate_config_targeted_level_warning() {
        let service = test_service();
        let mut config = QualityGateConfig::new(QualityLevel::TargetedTests);
        config.add_override("feature", QualityLevel::TargetedTests);
        let output = service
            .validate_config(ValidateConfigInput { config })
            .await
            .unwrap();
        assert!(output.valid); // warnings don't make it invalid
        assert!(!output.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_create_contract() {
        let service = test_service();
        let contract = service.create_contract(QualityLevel::MergeReady);
        assert_eq!(contract.required_level, QualityLevel::MergeReady);
    }
}

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

/// Derive a level from what the steps recorded (#1585).
///
/// The tiers are the same four, but every one of them is now a statement about a step: a workspace
/// test that passed, a package test that passed, a targeted test that passed, and — for the top tier —
/// the three integration gates as well. Nothing here reads a counter, a status enum or a caller's
/// assertion, which is what makes `source: Engine` mean something when it travels.
///
/// The floor is the answer for a run whose steps recorded nothing about quality, and it says so. That
/// is the case the old code could not express: it had no way to say "I do not know" and defaulted to
/// a level with an explanation that read like an observation.
pub(crate) fn classify_from_outcomes(
    outcomes: &[crate::quality_gates::domain::RecordedOutcome],
) -> ClassifyTestScopeOutput {
    use crate::quality_gates::domain::{CheckKind, ClaimSource};

    let relevant: Vec<&crate::quality_gates::domain::RecordedOutcome> = outcomes
        .iter()
        .filter(|o| o.kind() != CheckKind::Other)
        .collect();

    if relevant.is_empty() {
        // No step recorded anything about quality. This is not a failure and not evidence of green:
        // it is the absence of a subject, and the claim is the caller's because the engine has none.
        return ClassifyTestScopeOutput {
            level: QualityLevel::TargetedTests,
            explanation: format!(
                "No step reported a {} result — the level is the floor, not an observation, and the claim is the caller's",
                relevant_kinds_sentence()
            ),
            source: ClaimSource::Caller,
            evidence: Vec::new(),
        };
    }

    let passed = |kind: CheckKind| -> Vec<&crate::quality_gates::domain::RecordedOutcome> {
        relevant
            .iter()
            .copied()
            .filter(|o| o.kind() == kind && o.passed)
            .collect()
    };
    let failed = |kind: CheckKind| -> Vec<&crate::quality_gates::domain::RecordedOutcome> {
        relevant
            .iter()
            .copied()
            .filter(|o| o.kind() == kind && !o.passed)
            .collect()
    };

    let workspace_passed = passed(CheckKind::WorkspaceTest);
    let package_passed = passed(CheckKind::PackageTest);
    let targeted_passed = passed(CheckKind::TargetedTest);
    let lint_passed = passed(CheckKind::Lint);
    let format_passed = passed(CheckKind::Format);
    let audit_passed = passed(CheckKind::Audit);

    let mut failures: Vec<&crate::quality_gates::domain::RecordedOutcome> = Vec::new();
    for kind in [
        CheckKind::WorkspaceTest,
        CheckKind::PackageTest,
        CheckKind::TargetedTest,
        CheckKind::Lint,
        CheckKind::Format,
        CheckKind::Audit,
    ] {
        failures.extend(failed(kind));
    }
    let failure_note = if failures.is_empty() {
        String::new()
    } else {
        format!(
            "; {} reported a failing result: {}",
            failures.len(),
            failures
                .iter()
                .map(|o| o.label())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    // A failing check is not evidence for the tier it belongs to: the tier is built from what passed,
    // and every failure is named so a reader is not told "targeted tests passed" while a workspace
    // run is red.
    let (level, deciding, what) = if !workspace_passed.is_empty()
        && !lint_passed.is_empty()
        && !format_passed.is_empty()
        && !audit_passed.is_empty()
    {
        let mut deciding = workspace_passed.clone();
        deciding.extend(lint_passed.iter().copied());
        deciding.extend(format_passed.iter().copied());
        deciding.extend(audit_passed.iter().copied());
        (
            QualityLevel::MergeReady,
            deciding,
            "workspace tests and lint, format and audit all reported success",
        )
    } else if !workspace_passed.is_empty() {
        (
            QualityLevel::Workspace,
            workspace_passed.clone(),
            "a workspace-wide test run reported success",
        )
    } else if !package_passed.is_empty() {
        (
            QualityLevel::Package,
            package_passed.clone(),
            "a package-scoped test run reported success",
        )
    } else if !targeted_passed.is_empty() {
        (
            QualityLevel::TargetedTests,
            targeted_passed.clone(),
            "a targeted test run reported success",
        )
    } else {
        // Nothing reported success. The evidence is empty on purpose: no step decided this tier —
        // the failures are named separately, because "nothing passed" is not a step's claim.
        (
            QualityLevel::TargetedTests,
            Vec::new(),
            "no test run reported success",
        )
    };

    let mut explanation = format!("{what} ({})", level.as_str());
    if !deciding.is_empty() {
        explanation.push_str(&format!(
            " — from {}",
            deciding
                .iter()
                .map(|o| o.label())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    explanation.push_str(&failure_note);

    ClassifyTestScopeOutput {
        level,
        explanation,
        source: ClaimSource::Engine,
        evidence: deciding.iter().map(|o| o.step_id.clone()).collect(),
    }
}

/// The three kinds that can make a run's quality claimable at all, spelled for an explanation.
fn relevant_kinds_sentence() -> &'static str {
    "test, lint, format or audit"
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
        Ok(classify_from_outcomes(&input.outcomes))
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
        if let Some(ref template_name) = input.template_name
            && let Some(level) = self.config.required_level_for_template(template_name)
        {
            return Ok(GetContractOutput {
                contract: GreenContract::new(level),
                source: ContractSource::TemplateOverride {
                    template_name: template_name.clone(),
                },
            });
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
    use crate::quality_gates::domain::{ClaimSource, RecordedOutcome};
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

    fn outcome(step: &str, tool: &str, subject: &str, passed: bool) -> RecordedOutcome {
        RecordedOutcome::new(step, tool, passed)
            .with_name(step)
            .with_subject(subject)
    }

    #[tokio::test]
    async fn a_workspace_test_and_the_three_gates_classify_merge_ready() {
        let service = test_service();
        let output = service
            .classify_test_scope(ClassifyTestScopeInput {
                outcomes: vec![
                    outcome("n1", "run_command", "cargo test --workspace", true),
                    outcome("n2", "run_command", "cargo clippy -- -D warnings", true),
                    outcome("n3", "run_command", "cargo fmt --check", true),
                    outcome("n4", "run_command", "cargo audit", true),
                ],
            })
            .await
            .unwrap();

        // The level is only meaningful if it names what produced it: four steps, not a counter.
        assert_eq!(output.level, QualityLevel::MergeReady);
        assert_eq!(output.source, ClaimSource::Engine);
        assert_eq!(output.evidence, vec!["n1", "n2", "n3", "n4"]);
        // The explanation has to be actionable: which step, and what kind of evidence it was.
        assert!(output.explanation.contains("n1"), "{}", output.explanation);
        assert!(
            output.explanation.contains("workspace_test"),
            "{}",
            output.explanation
        );
    }

    #[tokio::test]
    async fn a_workspace_test_alone_is_workspace() {
        let service = test_service();
        let output = service
            .classify_test_scope(ClassifyTestScopeInput {
                outcomes: vec![outcome("n1", "run_command", "cargo test --workspace", true)],
            })
            .await
            .unwrap();

        assert_eq!(output.level, QualityLevel::Workspace);
        assert_eq!(output.source, ClaimSource::Engine);
    }

    #[tokio::test]
    async fn a_package_test_is_package() {
        let service = test_service();
        let output = service
            .classify_test_scope(ClassifyTestScopeInput {
                outcomes: vec![outcome(
                    "n1",
                    "run_command",
                    "cargo test -p rigorix-engine",
                    true,
                )],
            })
            .await
            .unwrap();

        assert_eq!(output.level, QualityLevel::Package);
    }

    #[tokio::test]
    async fn a_targeted_test_is_targeted() {
        let service = test_service();
        let output = service
            .classify_test_scope(ClassifyTestScopeInput {
                outcomes: vec![outcome(
                    "n1",
                    "run_command",
                    "cargo test --test unit receipt",
                    true,
                )],
            })
            .await
            .unwrap();

        assert_eq!(output.level, QualityLevel::TargetedTests);
        assert_eq!(output.evidence, vec!["n1"]);
    }

    #[tokio::test]
    async fn no_recorded_outcome_is_the_floor_and_the_callers_claim() {
        let service = test_service();
        let output = service
            .classify_test_scope(ClassifyTestScopeInput::default())
            .await
            .unwrap();

        // The old code reached this same level by asserting `targeted_tests_run: true`. The level is
        // the same; the claim is not — and the explanation has to say so, or the caller cannot tell
        // a measurement from a default.
        assert_eq!(output.level, QualityLevel::TargetedTests);
        assert_eq!(output.source, ClaimSource::Caller);
        assert!(output.evidence.is_empty());
        assert!(
            output.explanation.contains("No step reported"),
            "{}",
            output.explanation
        );
    }

    #[tokio::test]
    async fn a_failed_workspace_test_does_not_raise_the_level() {
        let service = test_service();
        let output = service
            .classify_test_scope(ClassifyTestScopeInput {
                outcomes: vec![outcome(
                    "n1",
                    "run_command",
                    "cargo test --workspace",
                    false,
                )],
            })
            .await
            .unwrap();

        assert_eq!(
            output.level,
            QualityLevel::TargetedTests,
            "a failing run is not evidence for the tier it belongs to"
        );
        assert_eq!(output.source, ClaimSource::Engine);
        assert!(
            output.explanation.contains("failing result"),
            "{}",
            output.explanation
        );
        assert!(
            output.explanation.contains("workspace_test"),
            "{}",
            output.explanation
        );
    }

    #[tokio::test]
    async fn a_step_that_says_nothing_about_quality_is_not_evidence() {
        let service = test_service();
        let output = service
            .classify_test_scope(ClassifyTestScopeInput {
                outcomes: vec![outcome("n1", "file_read", "README.md", true)],
            })
            .await
            .unwrap();

        assert_eq!(output.level, QualityLevel::TargetedTests);
        assert_eq!(output.source, ClaimSource::Caller);
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

        assert_eq!(output.contract.required_level, QualityLevel::Package);
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

        assert_eq!(output.contract.required_level, QualityLevel::MergeReady);
        assert!(matches!(
            output.source,
            ContractSource::TemplateOverride { .. }
        ));
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

        assert_eq!(output.contract.required_level, QualityLevel::Package); // default
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

//! Service interfaces (use cases) for the Quality Gates bounded context.
//!
//! @canonical .pi/architecture/modules/quality-gates.md#service
//! Implements: Contract Freeze — QualityGateService trait
//! Issue: #449 (quality-gates epic)
//!
//! These traits define the application-level operations that can be performed
//! for quality gate evaluation. All methods are async and return domain error
//! types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::quality_gates::domain::{GreenContract, QualityGateError};

use super::dto::{
    ClassifyTestScopeInput, ClassifyTestScopeOutput, EvaluateGateInput, EvaluateGateOutput,
    GetContractInput, GetContractOutput, ValidateConfigInput, ValidateConfigOutput,
};

/// Application service for evaluating quality gates and classifying test scope.
///
/// The `QualityGateService` is the primary entry point for the quality-gates
/// module. It handles:
/// - Evaluating quality gates (comparing observed level against contract)
/// - Classifying test scope into quality level
/// - Retrieving contracts for tasks/templates
/// - Validating quality gate configurations
#[async_trait]
pub trait QualityGateService: Send + Sync {
    /// Evaluate a quality gate against an observed test scope.
    ///
    /// Compares the observed `QualityLevel` against the `GreenContract`'s
    /// required level. Returns the outcome (Satisfied or Unsatisfied).
    ///
    /// # Errors
    /// - `QualityGateError::MissingContract` if no contract is provided
    async fn evaluate_gate(
        &self,
        input: EvaluateGateInput,
    ) -> Result<EvaluateGateOutput, QualityGateError>;

    /// Classify a test scope into a `QualityLevel`.
    ///
    /// Determines the highest quality level achieved based on what
    /// tests and checks were run:
    /// - All checks passed → MergeReady
    /// - Workspace tests passed → Workspace
    /// - Package tests passed → Package
    /// - Only targeted tests → TargetedTests
    /// - No tests at all → TargetedTests (lowest level)
    async fn classify_test_scope(
        &self,
        input: ClassifyTestScopeInput,
    ) -> Result<ClassifyTestScopeOutput, QualityGateError>;

    /// Get the green contract for a given task or template.
    ///
    /// Checks task-level overrides first, then template overrides,
    /// then falls back to the default configuration.
    async fn get_contract(
        &self,
        input: GetContractInput,
    ) -> Result<GetContractOutput, QualityGateError>;

    /// Validate a quality gate configuration.
    ///
    /// Checks that the configuration is valid (default level is set,
    /// template overrides reference valid levels).
    async fn validate_config(
        &self,
        input: ValidateConfigInput,
    ) -> Result<ValidateConfigOutput, QualityGateError>;

    /// Create a default `GreenContract` at the given level.
    fn create_contract(&self, level: crate::quality_gates::domain::QualityLevel) -> GreenContract;
}

//! Data Transfer Objects for the Quality Gates module.
//!
//! @canonical .pi/architecture/modules/quality-gates.md
//! Implements: Contract Freeze — DTO schemas for quality gates
//! Issue: #449 (quality-gates epic)
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};

use crate::quality_gates::domain::{GreenContract, QualityGateOutcome, QualityLevel};

// ---------------------------------------------------------------------------
// Evaluate Gate DTOs
// ---------------------------------------------------------------------------

/// Input for evaluating a quality gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateGateInput {
    /// The green contract specifying the required quality level.
    pub contract: GreenContract,

    /// The observed quality level from test execution.
    /// `None` means no test scope was determined (treats as lowest level).
    pub observed_level: Option<QualityLevel>,

    /// Optional task ID or node name for traceability.
    pub task_id: Option<String>,
}

/// Output from evaluating a quality gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluateGateOutput {
    /// The evaluation outcome.
    pub outcome: QualityGateOutcome,

    /// Human-readable summary of the outcome.
    pub summary: String,

    /// Task ID for traceability (echoed from input).
    pub task_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Classify Test Scope DTOs
// ---------------------------------------------------------------------------

/// Input for classifying a test scope into a quality level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyTestScopeInput {
    /// Whether targeted tests were run.
    pub targeted_tests_run: bool,

    /// Whether package-level tests were run.
    pub package_tests_run: bool,

    /// Whether workspace-level tests were run.
    pub workspace_tests_run: bool,

    /// Whether lint (clippy) passed.
    pub lint_passed: bool,

    /// Whether format check (fmt --check) passed.
    pub format_passed: bool,

    /// Whether security audit passed.
    pub audit_passed: bool,
}

/// Output from classifying a test scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassifyTestScopeOutput {
    /// The classified quality level.
    pub level: QualityLevel,

    /// Human-readable explanation of the classification.
    pub explanation: String,
}

// ---------------------------------------------------------------------------
// Get Contract DTOs
// ---------------------------------------------------------------------------

/// Input for getting the contract for a task/template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetContractInput {
    /// Optional template name to check for overrides.
    pub template_name: Option<String>,

    /// Optional task ID for direct task-level overrides.
    pub task_id: Option<String>,
}

/// Output from getting a contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetContractOutput {
    /// The contract for the task/template.
    pub contract: GreenContract,

    /// How the contract was determined.
    pub source: ContractSource,
}

/// Source of a contract determination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContractSource {
    /// Default configuration.
    Default,
    /// Template-specific override.
    TemplateOverride { template_name: String },
    /// Task-specific override.
    TaskOverride { task_id: String },
}

// ---------------------------------------------------------------------------
// Validate Config DTOs
// ---------------------------------------------------------------------------

/// Input for validating a quality gate configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigInput {
    /// The quality gate config to validate.
    pub config: crate::quality_gates::domain::QualityGateConfig,
}

/// Output from validating a configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateConfigOutput {
    /// Whether the configuration is valid.
    pub valid: bool,
    /// List of validation errors (empty if valid).
    pub errors: Vec<String>,
    /// List of warnings (non-blocking issues).
    pub warnings: Vec<String>,
}

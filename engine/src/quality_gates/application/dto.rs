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

/// Input for classifying a test scope into a quality level (#1585).
///
/// The input is the recorded outcomes themselves, not six booleans about them. The booleans used to
/// be supplied by the caller, and both production callers invented them — `iteration > 2` and
/// `final_status != Failed` stood in for "workspace tests passed", and `lint_passed` was the literal
/// `false` at both sites, which made the top tier unreachable by construction.
///
/// An empty list is legal and meaningful: it is a run whose steps recorded nothing about quality, and
/// the classifier answers that with the floor and [`ClaimSource::Caller`] rather than a guess.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassifyTestScopeInput {
    /// What the steps reported, one entry per executed step that said anything.
    #[serde(default)]
    pub outcomes: Vec<crate::quality_gates::domain::RecordedOutcome>,
}

/// Output from classifying a test scope (#1585).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassifyTestScopeOutput {
    /// The classified quality level.
    pub level: QualityLevel,

    /// Human-readable explanation of the classification, naming the steps it came from.
    pub explanation: String,

    /// Whose claim the level is: derived from recorded outcomes, or defaulted in their absence.
    pub source: crate::quality_gates::domain::ClaimSource,

    /// The step ids the level was derived from. Empty exactly when nothing was recorded, which is
    /// the state `source: Caller` names.
    #[serde(default)]
    pub evidence: Vec<String>,
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

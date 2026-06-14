//! RiskClassifier trait — maps tool names to RiskLevel.
//!
//! @canonical .pi/architecture/modules/risk-gating.md#classifier
//! Implements: Contract Freeze — RiskClassifier trait
//! Issue: issue-contract-freeze
//!
//! The RiskClassifier is the core domain interface for determining the
//! risk level of a tool based on its name and parameters. Every tool
//! invocation passes through this classifier before the gating policy
//! is applied.
//!
//! # Classification Rules (Default Implementation)
//!
//! | Tool Pattern                          | Risk Level | Rationale                           |
//! |---------------------------------------|------------|-------------------------------------|
//! | `file_read`, `lsp_query`, `git_read`  | Low        | Read-only, no side effects          |
//! | `file_write`, `file_append`,          | Medium     | Modifies local state,               |
//! | `file_patch`, `git_stage`             |            | requires confirmation               |
//! | `run_command`, `git_commit`           | High       | External execution, irreversible    |
//!
//! # Contract (Frozen)
//! - The `classify` method is the single entry point
//! - Parameters are provided for potential context-aware classification
//! - Implementations must be deterministic (same input → same output)
//! - Overrides from `RiskConfig` take precedence over default rules

use serde::{Deserialize, Serialize};

use crate::risk_gating::domain::risk_level::RiskLevel;

/// Classifies tools/operations into risk levels for gating.
///
/// The classifier maps a tool name (and optionally its parameters) to a
/// `RiskLevel`. The result determines the gating policy applied before
/// execution: Low → auto-execute, Medium → confirm, High → dry-run.
///
/// # Determinism
///
/// Implementations MUST be deterministic — the same tool name and
/// parameters MUST always produce the same `RiskLevel`. This is
/// essential for auditability and reproducibility.
///
/// # Configuration Overrides
///
/// The `RiskConfig` (in `crate::risk_gating::domain::risk_config`)
/// provides per-tool risk overrides. Classifier implementations SHOULD
/// check for overrides before applying default classification rules.
pub trait RiskClassifier: Send + Sync {
    /// Classify a tool into a risk level.
    ///
    /// Takes the tool name and optional parameters and returns the
    /// classification result.
    ///
    /// # Arguments
    ///
    /// * `tool_name` — The name of the tool being classified
    ///   (e.g., "file_read", "run_command", "git_commit").
    /// * `parameters` — Optional JSON value containing the tool's
    ///   parameters/arguments for context-aware classification.
    ///
    /// # Returns
    ///
    /// A `ClassificationResult` containing the risk level and the
    /// reasoning/rule that produced it.
    fn classify(&self, tool_name: &str, parameters: Option<&serde_json::Value>) -> ClassificationResult;

    /// Get the risk level for a tool without detailed reasoning.
    ///
    /// Convenience wrapper around `classify` that returns only the
    /// `RiskLevel`. Useful when reasoning context is not needed.
    fn risk_level(&self, tool_name: &str, parameters: Option<&serde_json::Value>) -> RiskLevel {
        self.classify(tool_name, parameters).risk_level
    }
}

/// The result of a classification operation.
///
/// Carries both the risk level and the reasoning context (which rule
/// matched) for auditability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationResult {
    /// The risk level assigned to the tool.
    pub risk_level: RiskLevel,

    /// A human-readable description of why this level was assigned.
    /// For example: "Default rule: file_read → Low" or "Override from config: run_command → High".
    pub reason: String,

    /// Whether this classification came from a configured override
    /// as opposed to a built-in default rule.
    pub from_override: bool,
}

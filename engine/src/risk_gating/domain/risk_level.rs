//! RiskLevel domain enum.
//!
//! @canonical .pi/architecture/modules/risk-gating.md#level
//! Implements: Contract Freeze — RiskLevel enum
//! Issue: issue-contract-freeze
//!
//! Classifies tools/operations into one of three risk levels, each with a
//! corresponding gating policy. This is the core domain type for the
//! risk-gating module — every tool invocation is classified into one of
//! these levels before execution.
//!
//! # Gating Policies
//!
//! | Level  | Gate Policy               | Example Tools                     |
//! |--------|---------------------------|-----------------------------------|
//! | Low    | Auto-execute — no gate    | FileRead, LspQuery, GitRead       |
//! | Medium | User confirmation required| FileWrite, FileAppend, GitStage   |
//! | High   | Dry-run by default        | RunCommand, GitCommit             |
//!
//! # Contract (Frozen)
//! - The enum variants, their ordering, and serialization are frozen
//! - New variants require ADR approval
//! - `serde(rename_all = "lowercase")` ensures stable JSON representation

use serde::{Deserialize, Serialize};

/// Risk level assigned to a tool or operation.
///
/// Determines the gating policy applied before execution.
///
/// # Gating Semantics
/// - **Low:** Read-only or safe operations — auto-executed without user interaction.
/// - **Medium:** State-modifying operations — require explicit user confirmation.
/// - **High:** Irreversible or dangerous operations — dry-run by default,
///   requiring explicit opt-in to execute with side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    /// Read-only operations with no side effects.
    /// Gate: auto-execute without user interaction.
    Low,

    /// State-modifying operations that are reversible.
    /// Gate: require user confirmation before execution.
    Medium,

    /// Irreversible or dangerous operations.
    /// Gate: dry-run by default (preview, no side effects).
    High,
}

impl RiskLevel {
    /// Check if this level is Low (auto-execute).
    pub fn is_low(&self) -> bool {
        matches!(self, RiskLevel::Low)
    }

    /// Check if this level is Medium (requires confirmation).
    pub fn is_medium(&self) -> bool {
        matches!(self, RiskLevel::Medium)
    }

    /// Check if this level is High (requires dry-run).
    pub fn is_high(&self) -> bool {
        matches!(self, RiskLevel::High)
    }

    /// Get the gating action required for this risk level.
    pub fn gating_action(&self) -> GatingAction {
        match self {
            RiskLevel::Low => GatingAction::AutoExecute,
            RiskLevel::Medium => GatingAction::RequireConfirmation,
            RiskLevel::High => GatingAction::DryRun,
        }
    }
}

/// The gating action required for a given risk level.
///
/// This is derived from the `RiskLevel` and determines what action
/// the execution engine must take before allowing the tool to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatingAction {
    /// Execute immediately without any gate.
    AutoExecute,

    /// Request user confirmation before executing.
    RequireConfirmation,

    /// Execute in dry-run mode (preview only, no side effects).
    DryRun,
}

impl Default for RiskLevel {
    fn default() -> Self {
        RiskLevel::Low
    }
}

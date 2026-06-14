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

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // RiskLevel variant checks
    // -----------------------------------------------------------------------

    #[test]
    fn test_low_is_low() {
        assert!(RiskLevel::Low.is_low());
        assert!(!RiskLevel::Low.is_medium());
        assert!(!RiskLevel::Low.is_high());
    }

    #[test]
    fn test_medium_is_medium() {
        assert!(!RiskLevel::Medium.is_low());
        assert!(RiskLevel::Medium.is_medium());
        assert!(!RiskLevel::Medium.is_high());
    }

    #[test]
    fn test_high_is_high() {
        assert!(!RiskLevel::High.is_low());
        assert!(!RiskLevel::High.is_medium());
        assert!(RiskLevel::High.is_high());
    }

    // -----------------------------------------------------------------------
    // Gating action mapping
    // -----------------------------------------------------------------------

    #[test]
    fn test_low_gating_action() {
        assert_eq!(RiskLevel::Low.gating_action(), GatingAction::AutoExecute);
    }

    #[test]
    fn test_medium_gating_action() {
        assert_eq!(
            RiskLevel::Medium.gating_action(),
            GatingAction::RequireConfirmation
        );
    }

    #[test]
    fn test_high_gating_action() {
        assert_eq!(RiskLevel::High.gating_action(), GatingAction::DryRun);
    }

    // -----------------------------------------------------------------------
    // Default
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_is_low() {
        assert_eq!(RiskLevel::default(), RiskLevel::Low);
    }

    // -----------------------------------------------------------------------
    // Ordering
    // -----------------------------------------------------------------------

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::Low < RiskLevel::High);
        assert!(RiskLevel::High > RiskLevel::Low);
        assert!(RiskLevel::Medium > RiskLevel::Low);
    }

    #[test]
    fn test_risk_level_equality() {
        assert_eq!(RiskLevel::Low, RiskLevel::Low);
        assert_eq!(RiskLevel::Medium, RiskLevel::Medium);
        assert_eq!(RiskLevel::High, RiskLevel::High);
        assert_ne!(RiskLevel::Low, RiskLevel::Medium);
        assert_ne!(RiskLevel::Medium, RiskLevel::High);
    }

    // -----------------------------------------------------------------------
    // Serialization (JSON)
    // -----------------------------------------------------------------------

    #[test]
    fn test_risk_level_serialization_roundtrip() {
        for level in &[RiskLevel::Low, RiskLevel::Medium, RiskLevel::High] {
            let json = serde_json::to_string(level).unwrap();
            let deserialized: RiskLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(*level, deserialized);
        }
    }

    #[test]
    fn test_risk_level_serialization_lowercase() {
        assert_eq!(serde_json::to_string(&RiskLevel::Low).unwrap(), "\"low\"");
        assert_eq!(
            serde_json::to_string(&RiskLevel::Medium).unwrap(),
            "\"medium\""
        );
        assert_eq!(serde_json::to_string(&RiskLevel::High).unwrap(), "\"high\"");
    }

    #[test]
    fn test_risk_level_deserialization_case_insensitive() {
        assert_eq!(
            serde_json::from_str::<RiskLevel>("\"low\"").unwrap(),
            RiskLevel::Low
        );
        assert_eq!(
            serde_json::from_str::<RiskLevel>("\"medium\"").unwrap(),
            RiskLevel::Medium
        );
        assert_eq!(
            serde_json::from_str::<RiskLevel>("\"high\"").unwrap(),
            RiskLevel::High
        );
    }

    #[test]
    fn test_risk_level_deserialization_invalid() {
        let result = serde_json::from_str::<RiskLevel>("\"critical\"");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // GatingAction serialization
    // -----------------------------------------------------------------------

    #[test]
    fn test_gating_action_serialization_snake_case() {
        assert_eq!(
            serde_json::to_string(&GatingAction::AutoExecute).unwrap(),
            "\"auto_execute\""
        );
        assert_eq!(
            serde_json::to_string(&GatingAction::RequireConfirmation).unwrap(),
            "\"require_confirmation\""
        );
        assert_eq!(
            serde_json::to_string(&GatingAction::DryRun).unwrap(),
            "\"dry_run\""
        );
    }

    #[test]
    fn test_gating_action_deserialization() {
        assert_eq!(
            serde_json::from_str::<GatingAction>("\"auto_execute\"").unwrap(),
            GatingAction::AutoExecute
        );
        assert_eq!(
            serde_json::from_str::<GatingAction>("\"require_confirmation\"").unwrap(),
            GatingAction::RequireConfirmation
        );
        assert_eq!(
            serde_json::from_str::<GatingAction>("\"dry_run\"").unwrap(),
            GatingAction::DryRun
        );
    }

    #[test]
    fn test_gating_action_roundtrip() {
        for action in &[
            GatingAction::AutoExecute,
            GatingAction::RequireConfirmation,
            GatingAction::DryRun,
        ] {
            let json = serde_json::to_string(action).unwrap();
            let deserialized: GatingAction = serde_json::from_str(&json).unwrap();
            assert_eq!(*action, deserialized);
        }
    }

    // -----------------------------------------------------------------------
    // Clone and Debug
    // -----------------------------------------------------------------------

    #[test]
    fn test_risk_level_clone() {
        let level = RiskLevel::High;
        let cloned = level.clone();
        assert_eq!(level, cloned);
    }

    #[test]
    fn test_risk_level_debug() {
        assert_eq!(format!("{:?}", RiskLevel::Low), "Low");
        assert_eq!(format!("{:?}", RiskLevel::Medium), "Medium");
        assert_eq!(format!("{:?}", RiskLevel::High), "High");
    }

    #[test]
    fn test_gating_action_debug() {
        assert_eq!(format!("{:?}", GatingAction::AutoExecute), "AutoExecute");
        assert_eq!(
            format!("{:?}", GatingAction::RequireConfirmation),
            "RequireConfirmation"
        );
        assert_eq!(format!("{:?}", GatingAction::DryRun), "DryRun");
    }

    // -----------------------------------------------------------------------
    // Copy semantics
    // -----------------------------------------------------------------------

    #[test]
    fn test_risk_level_copy() {
        let level = RiskLevel::Medium;
        let copied = level; // Copy, not move
        assert_eq!(level, copied); // level is still valid
    }

    #[test]
    fn test_gating_action_copy() {
        let action = GatingAction::DryRun;
        let copied = action;
        assert_eq!(action, copied);
    }
}

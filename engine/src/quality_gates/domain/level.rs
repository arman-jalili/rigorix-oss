//! QualityLevel — four-tier escalation of test scope.
//!
//! @canonical .pi/architecture/modules/quality-gates.md#qualitylevel
//! Implements: Contract Freeze — QualityLevel enum
//! Issue: #449 (quality-gates epic)
//!
//! # Contract (Frozen)
//! - Four mutually exclusive quality levels with ascending scope
//! - Implements `PartialOrd` and `Ord` for comparison (levels are ordered)
//! - Implements `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq` for testability
//! - Serialization support for configuration and API responses
//! - `as_str()` returns snake_case names for serialization

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Four-tier escalation of test scope, from narrowest to broadest.
///
/// Each level represents a progressively broader scope of validation:
/// - `TargetedTests`: only tests directly relevant to the change
/// - `Package`: all tests in the affected crate/package
/// - `Workspace`: all tests across the entire workspace
/// - `MergeReady`: workspace + all integration gates (lint, format, audit)
///
/// The levels implement `PartialOrd`/`Ord` so contracts can check
/// `observed >= required` for satisfaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityLevel {
    /// Only tests directly relevant to the change passed.
    /// E.g., `cargo test -p crate -- specific::test`
    TargetedTests = 0,

    /// The crate/package tests passed.
    /// E.g., `cargo test -p crate`
    Package = 1,

    /// Full workspace tests passed.
    /// E.g., `cargo test --workspace`
    Workspace = 2,

    /// Workspace + all integration gates (lint, format, audit) passed.
    /// E.g., full CI pipeline.
    MergeReady = 3,
}

impl QualityLevel {
    /// Returns the canonical snake_case name of this quality level.
    pub fn as_str(&self) -> &'static str {
        match self {
            QualityLevel::TargetedTests => "targeted_tests",
            QualityLevel::Package => "package",
            QualityLevel::Workspace => "workspace",
            QualityLevel::MergeReady => "merge_ready",
        }
    }

    /// Returns a human-readable description of this level.
    pub fn description(&self) -> &'static str {
        match self {
            QualityLevel::TargetedTests => "Only tests directly relevant to the change passed",
            QualityLevel::Package => "All tests in the affected crate/package passed",
            QualityLevel::Workspace => "All tests across the entire workspace passed",
            QualityLevel::MergeReady => "Workspace tests + lint, format, and audit all passed",
        }
    }

    /// Returns the typical command or action associated with this level.
    pub fn typical_command(&self) -> &'static str {
        match self {
            QualityLevel::TargetedTests => "cargo test -p <crate> -- <specific_test>",
            QualityLevel::Package => "cargo test -p <crate>",
            QualityLevel::Workspace => "cargo test --workspace",
            QualityLevel::MergeReady => "full CI pipeline (build + test + lint + fmt + audit)",
        }
    }

    /// Returns the numeric value of this level (0-3).
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

impl PartialOrd for QualityLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QualityLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl std::fmt::Display for QualityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_str() {
        assert_eq!(QualityLevel::TargetedTests.as_str(), "targeted_tests");
        assert_eq!(QualityLevel::Package.as_str(), "package");
        assert_eq!(QualityLevel::Workspace.as_str(), "workspace");
        assert_eq!(QualityLevel::MergeReady.as_str(), "merge_ready");
    }

    #[test]
    fn test_description() {
        assert!(!QualityLevel::TargetedTests.description().is_empty());
        assert!(!QualityLevel::Package.description().is_empty());
        assert!(!QualityLevel::Workspace.description().is_empty());
        assert!(!QualityLevel::MergeReady.description().is_empty());
    }

    #[test]
    fn test_typical_command() {
        assert!(!QualityLevel::TargetedTests.typical_command().is_empty());
    }

    #[test]
    fn test_as_u8() {
        assert_eq!(QualityLevel::TargetedTests.as_u8(), 0);
        assert_eq!(QualityLevel::Package.as_u8(), 1);
        assert_eq!(QualityLevel::Workspace.as_u8(), 2);
        assert_eq!(QualityLevel::MergeReady.as_u8(), 3);
    }

    #[test]
    fn test_ordering() {
        assert!(QualityLevel::TargetedTests < QualityLevel::Package);
        assert!(QualityLevel::Package < QualityLevel::Workspace);
        assert!(QualityLevel::Workspace < QualityLevel::MergeReady);
        assert!(QualityLevel::TargetedTests < QualityLevel::MergeReady);
    }

    #[test]
    fn test_equality() {
        assert_eq!(QualityLevel::TargetedTests, QualityLevel::TargetedTests);
        assert_ne!(QualityLevel::TargetedTests, QualityLevel::Package);
    }

    #[test]
    fn test_display() {
        assert_eq!(QualityLevel::MergeReady.to_string(), "merge_ready");
    }

    #[test]
    fn test_serialization_roundtrip() {
        for variant in &[
            QualityLevel::TargetedTests,
            QualityLevel::Package,
            QualityLevel::Workspace,
            QualityLevel::MergeReady,
        ] {
            let json = serde_json::to_string(variant).unwrap();
            let deserialized: QualityLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(*variant, deserialized);
        }
    }

    #[test]
    fn test_serde_rename() {
        let json = serde_json::to_string(&QualityLevel::Workspace).unwrap();
        assert_eq!(json, "\"workspace\"");
    }

    #[test]
    fn test_ord_consistency() {
        // Verify that the PartialOrd implementation is consistent with Ord
        let levels = [
            QualityLevel::TargetedTests,
            QualityLevel::Package,
            QualityLevel::Workspace,
            QualityLevel::MergeReady,
        ];
        for i in 0..levels.len() {
            for j in 0..levels.len() {
                let cmp_ord = levels[i].cmp(&levels[j]);
                let cmp_partial = levels[i].partial_cmp(&levels[j]).unwrap();
                assert_eq!(cmp_ord, cmp_partial);
            }
        }
    }
}

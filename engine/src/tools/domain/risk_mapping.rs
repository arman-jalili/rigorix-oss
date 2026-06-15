//! Risk level mapping for all concrete tool implementations.
//!
//! @canonical .pi/architecture/modules/tool-system.md#risk-mapping
//! Implements: Contract Freeze — per-tool risk level classification
//! Issue: #124
//!
//! Each tool has an associated risk level that determines the gating
//! policy applied before execution. The risk level is classified by
//! the `RiskClassifier` using these default mappings, with optional
//! overrides from `RiskConfig`.
//!
//! # Risk Level Rules
//!
//! | Tool | Risk Level | Rationale |
//! |------|-----------|-----------|
//! | file-read | Low | Read-only |
//! | lsp-query | Low | Read-only |
//! | git-read | Low | Read-only |
//! | file-write | Medium | Modifies files |
//! | file-append | Medium | Modifies files |
//! | file-patch | Medium | Modifies files |
//! | git-stage | Medium | Modifies git index |
//! | run-command | High | Arbitrary execution |
//! | git-commit | High | Irreversible git action |
//!
//! # Contract (Frozen)
//! - Default risk levels are defined here and frozen
//! - Overrides through `RiskConfig` are allowed at runtime
//! - Risk level changes require ADR approval

use std::collections::HashMap;

use crate::risk_gating::domain::risk_level::RiskLevel;

/// Get the default risk level mapping for all concrete tools.
///
/// Returns an immutable map of tool name → default RiskLevel.
/// This mapping is used by the `DefaultClassifier` when no override
/// is configured in `RiskConfig`.
pub fn default_tool_risk_levels() -> HashMap<&'static str, RiskLevel> {
    let mut map = HashMap::new();

    // --- Low risk (read-only operations) ---
    map.insert("file-read", RiskLevel::Low);
    map.insert("lsp-query", RiskLevel::Low);
    map.insert("git-read", RiskLevel::Low);

    // --- Medium risk (state-modifying, reversible) ---
    map.insert("file-write", RiskLevel::Medium);
    map.insert("file-append", RiskLevel::Medium);
    map.insert("file-patch", RiskLevel::Medium);
    map.insert("git-stage", RiskLevel::Medium);

    // --- High risk (irreversible or dangerous) ---
    map.insert("run-command", RiskLevel::High);
    map.insert("git-commit", RiskLevel::High);

    map
}

/// Look up the default risk level for a tool by name.
///
/// Returns `None` if the tool name is unknown (classifier should default
/// to a conservative level for unknown tools).
pub fn default_risk_level_for(tool_name: &str) -> Option<RiskLevel> {
    let map = default_tool_risk_levels();
    map.get(tool_name).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tools_have_risk_level() {
        let map = default_tool_risk_levels();
        assert!(map.contains_key("file-read"));
        assert!(map.contains_key("file-write"));
        assert!(map.contains_key("file-append"));
        assert!(map.contains_key("file-patch"));
        assert!(map.contains_key("run-command"));
        assert!(map.contains_key("lsp-query"));
        assert!(map.contains_key("git-read"));
        assert!(map.contains_key("git-stage"));
        assert!(map.contains_key("git-commit"));
        assert_eq!(map.len(), 9);
    }

    #[test]
    fn test_risk_level_assignment() {
        let map = default_tool_risk_levels();

        // Low
        assert_eq!(map.get("file-read"), Some(&RiskLevel::Low));
        assert_eq!(map.get("lsp-query"), Some(&RiskLevel::Low));
        assert_eq!(map.get("git-read"), Some(&RiskLevel::Low));

        // Medium
        assert_eq!(map.get("file-write"), Some(&RiskLevel::Medium));
        assert_eq!(map.get("file-append"), Some(&RiskLevel::Medium));
        assert_eq!(map.get("file-patch"), Some(&RiskLevel::Medium));
        assert_eq!(map.get("git-stage"), Some(&RiskLevel::Medium));

        // High
        assert_eq!(map.get("run-command"), Some(&RiskLevel::High));
        assert_eq!(map.get("git-commit"), Some(&RiskLevel::High));
    }

    #[test]
    fn test_default_risk_level_lookup() {
        assert_eq!(default_risk_level_for("file-read"), Some(RiskLevel::Low));
        assert_eq!(default_risk_level_for("run-command"), Some(RiskLevel::High));
        assert_eq!(default_risk_level_for("git-commit"), Some(RiskLevel::High));
    }

    #[test]
    fn test_unknown_tool_returns_none() {
        assert_eq!(default_risk_level_for("unknown-tool"), None);
        assert_eq!(default_risk_level_for(""), None);
    }

    #[test]
    fn test_risk_level_values() {
        let map = default_tool_risk_levels();
        assert_eq!(*map.get("file-read").unwrap(), RiskLevel::Low);
        assert_eq!(*map.get("file-write").unwrap(), RiskLevel::Medium);
        assert_eq!(*map.get("run-command").unwrap(), RiskLevel::High);
    }
}

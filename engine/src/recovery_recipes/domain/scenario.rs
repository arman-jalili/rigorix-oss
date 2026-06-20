//! FailureScenario — typed enumeration of known recoverable failure scenarios.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md#failurescenario
//! Implements: Contract Freeze — FailureScenario enum
//! Issue: #438 (recovery-recipes epic)
//!
//! # Contract (Frozen)
//! - Each variant corresponds to a known recoverable failure
//! - Implements `Clone`, `Debug`, `PartialEq`, `Eq`, `Hash` for testability
//! - Serialization support for eventing and API responses
//! - `from_failure_type()` maps from `FailureType` (failure_classification) to
//!   `FailureScenario` — returns `None` for unrecognized scenarios
//! - `as_str()` returns snake_case name for serialization

use serde::{Deserialize, Serialize};

/// Typed enumeration of all known recoverable failure scenarios.
///
/// Each variant corresponds to a failure mode that has a configured
/// `RecoveryRecipe`. Unknown failure types have no recipe and are
/// escalated immediately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureScenario {
    /// Build failure (cargo build, npm build, etc.).
    CompileError,
    /// Test failure (non-compile test suite failure).
    TestFailure,
    /// Tool connection failure (LSP, MCP, external service).
    ToolConnectionError,
    /// LLM provider API failure (rate limit, timeout, 5xx).
    ProviderFailure,
    /// Partial initialization (some components started, some didn't).
    PartialInitialization,
    /// Authorization failure (trust prompt, API key, permission).
    AuthorizationError,
    /// Branch is stale relative to main.
    StaleBranch,
}

impl FailureScenario {
    /// Returns the canonical snake_case name of this scenario.
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureScenario::CompileError => "compile_error",
            FailureScenario::TestFailure => "test_failure",
            FailureScenario::ToolConnectionError => "tool_connection_error",
            FailureScenario::ProviderFailure => "provider_failure",
            FailureScenario::PartialInitialization => "partial_initialization",
            FailureScenario::AuthorizationError => "authorization_error",
            FailureScenario::StaleBranch => "stale_branch",
        }
    }

    /// Returns a human-readable description of this scenario.
    pub fn description(&self) -> &'static str {
        match self {
            FailureScenario::CompileError => "Build/compile failure (cargo build, npm build, etc.)",
            FailureScenario::TestFailure => "Test suite failure (non-compile)",
            FailureScenario::ToolConnectionError => {
                "Tool connection failure (LSP, MCP, external service)"
            }
            FailureScenario::ProviderFailure => {
                "LLM provider API failure (rate limit, timeout, 5xx)"
            }
            FailureScenario::PartialInitialization => {
                "Partial initialization (some components started, some didn't)"
            }
            FailureScenario::AuthorizationError => {
                "Authorization failure (trust prompt, API key, permission)"
            }
            FailureScenario::StaleBranch => "Branch is stale relative to main",
        }
    }

    /// Map from `FailureType` (failure_classification module) to `FailureScenario`.
    ///
    /// Returns `None` when the failure type does not correspond to a known
    /// recoverable scenario. The caller should escalate immediately in that case.
    ///
    /// # Current Mapping
    ///
    /// | FailureType | FailureScenario |
    /// |-------------|-----------------|
    /// | BuildFailure | CompileError |
    /// | TestFailure | TestFailure |
    /// | _ | None (escalate) |
    ///
    /// Additional mappings will be added when the failure_classification module
    /// introduces new `FailureType` variants (e.g., ToolConnectionError,
    /// LlmApiError, PartialInitialization, AuthorizationError, StaleBranch).
    pub fn from_failure_type(
        ft: &crate::failure_classification::domain::FailureType,
    ) -> Option<Self> {
        use crate::failure_classification::domain::FailureType;
        match ft {
            FailureType::BuildFailure => Some(Self::CompileError),
            FailureType::TestFailure => Some(Self::TestFailure),
            // Future FailureType variants to map when added:
            // FailureType::ToolConnectionError => Some(Self::ToolConnectionError),
            // FailureType::LlmApiError => Some(Self::ProviderFailure),
            // FailureType::PartialInitialization => Some(Self::PartialInitialization),
            // FailureType::AuthorizationError => Some(Self::AuthorizationError),
            // FailureType::StaleBranch => Some(Self::StaleBranch),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_str() {
        assert_eq!(FailureScenario::CompileError.as_str(), "compile_error");
        assert_eq!(FailureScenario::TestFailure.as_str(), "test_failure");
        assert_eq!(
            FailureScenario::ToolConnectionError.as_str(),
            "tool_connection_error"
        );
        assert_eq!(
            FailureScenario::ProviderFailure.as_str(),
            "provider_failure"
        );
        assert_eq!(
            FailureScenario::PartialInitialization.as_str(),
            "partial_initialization"
        );
        assert_eq!(
            FailureScenario::AuthorizationError.as_str(),
            "authorization_error"
        );
        assert_eq!(FailureScenario::StaleBranch.as_str(), "stale_branch");
    }

    #[test]
    fn test_description() {
        assert!(!FailureScenario::CompileError.description().is_empty());
        assert!(!FailureScenario::TestFailure.description().is_empty());
    }

    #[test]
    fn test_from_failure_type_build_failure() {
        let result = FailureScenario::from_failure_type(
            &crate::failure_classification::domain::FailureType::BuildFailure,
        );
        assert_eq!(result, Some(FailureScenario::CompileError));
    }

    #[test]
    fn test_from_failure_type_test_failure() {
        let result = FailureScenario::from_failure_type(
            &crate::failure_classification::domain::FailureType::TestFailure,
        );
        assert_eq!(result, Some(FailureScenario::TestFailure));
    }

    #[test]
    fn test_from_failure_type_unknown_returns_none() {
        let result = FailureScenario::from_failure_type(
            &crate::failure_classification::domain::FailureType::Transient,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn test_serialization_roundtrip() {
        for variant in &[
            FailureScenario::CompileError,
            FailureScenario::TestFailure,
            FailureScenario::ToolConnectionError,
            FailureScenario::ProviderFailure,
            FailureScenario::PartialInitialization,
            FailureScenario::AuthorizationError,
            FailureScenario::StaleBranch,
        ] {
            let json = serde_json::to_string(variant).unwrap();
            let deserialized: FailureScenario = serde_json::from_str(&json).unwrap();
            assert_eq!(*variant, deserialized);
        }
    }

    #[test]
    fn test_serde_rename() {
        let json = serde_json::to_string(&FailureScenario::CompileError).unwrap();
        assert_eq!(json, "\"compile_error\"");
    }
}

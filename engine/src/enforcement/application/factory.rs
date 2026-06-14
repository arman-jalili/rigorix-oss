//! Factory interfaces for constructing Enforcement domain objects.
//!
//! @canonical .pi/architecture/modules/enforcement.md
//! Implements: Contract Freeze — ExecutionEnforcerFactory trait
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of the ExecutionEnforcer with
//! appropriate budgets, limits, and tool policies loaded from configuration.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured ExecutionEnforcer
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::enforcement::domain::{EnforcementConfig, EnforcementError};

use super::service::ExecutionEnforcer;

/// Factory for constructing `ExecutionEnforcer` instances.
///
/// Handles creation of the enforcer with appropriate resource budgets,
/// execution limits, and tool policies loaded from the enforcement config.
/// Supports presets and custom overrides.
#[async_trait]
pub trait ExecutionEnforcerFactory: Send + Sync {
    /// Create an `ExecutionEnforcer` from an `EnforcementConfig`.
    ///
    /// Builds the full enforcer state: resource budgets (with zeroed current
    /// usage), execution limits, and per-tool policies.
    async fn create_from_config(
        &self,
        execution_id: &str,
        config: EnforcementConfig,
    ) -> Result<Box<dyn ExecutionEnforcer>, EnforcementError>;

    /// Create an `ExecutionEnforcer` using the default configuration profile.
    ///
    /// Uses `EnforcementPresetProfile::Standard` with all default values.
    async fn create_default(
        &self,
        execution_id: &str,
    ) -> Result<Box<dyn ExecutionEnforcer>, EnforcementError>;

    /// Create an `ExecutionEnforcer` with custom resource budgets.
    ///
    /// Merges the provided budgets with default execution limits and tool policies.
    /// If a budget already exists in the config, the provided value takes precedence.
    async fn create_with_custom_budgets(
        &self,
        execution_id: &str,
        config: EnforcementConfig,
        budget_overrides: std::collections::HashMap<
            String,
            crate::enforcement::domain::ResourceBudget,
        >,
    ) -> Result<Box<dyn ExecutionEnforcer>, EnforcementError>;

    /// Create an `ExecutionEnforcer` with custom tool policy overrides.
    ///
    /// Merges the provided tool policies with the config's default policies.
    /// If a tool already has a policy in the config, the override takes precedence.
    async fn create_with_tool_overrides(
        &self,
        execution_id: &str,
        config: EnforcementConfig,
        tool_overrides: std::collections::HashMap<String, crate::enforcement::domain::ToolPolicy>,
    ) -> Result<Box<dyn ExecutionEnforcer>, EnforcementError>;

    /// Create an `ExecutionEnforcer` that is a child of an existing enforcer.
    ///
    /// Useful for sub-executions or nested pipelines that share parent budgets.
    /// The child enforcer inherits the parent's budgets but has its own
    /// tool policies and execution limits.
    async fn create_child(
        &self,
        execution_id: &str,
        parent_enforcer: &dyn ExecutionEnforcer,
        config: EnforcementConfig,
    ) -> Result<Box<dyn ExecutionEnforcer>, EnforcementError>;
}

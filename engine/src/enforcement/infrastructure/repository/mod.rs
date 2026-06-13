//! Repository interfaces for the Enforcement bounded context.
//!
//! @canonical .pi/architecture/modules/enforcement.md
//! Implements: Contract Freeze — EnforcementPolicyRepository trait
//! Issue: issue-contract-freeze
//!
//! Enforcement policies and budgets are typically loaded from the global
//! configuration at startup. However, for advanced use cases — runtime
//! policy updates, per-execution policy overrides, or persistence of
//! enforcement state across restarts — repository interfaces are provided.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;

use crate::enforcement::domain::{EnforcementConfig, EnforcementError};

/// Repository for loading and persisting enforcement policies and budgets.
///
/// The default implementation loads from the global `Config` object.
/// Custom implementations may load from a database, remote API, or
/// file-based policy store.
#[async_trait]
pub trait EnforcementPolicyRepository: Send + Sync {
    /// Load enforcement configuration for an execution.
    ///
    /// Returns the full `EnforcementConfig` including budgets, execution
    /// limits, and tool policies. If no configuration is found, returns
    /// the default configuration.
    async fn load_config(&self, execution_id: &str) -> Result<EnforcementConfig, EnforcementError>;

    /// Save enforcement configuration for an execution.
    ///
    /// Persists any runtime modifications to budgets or policies.
    /// If persistence is not supported, returns `Ok(())` without error.
    async fn save_config(
        &self,
        execution_id: &str,
        config: &EnforcementConfig,
    ) -> Result<(), EnforcementError>;

    /// Load a specific tool policy override.
    ///
    /// Returns `None` if no override exists for this tool.
    async fn load_tool_policy(
        &self,
        tool: &str,
    ) -> Result<Option<crate::enforcement::domain::ToolPolicy>, EnforcementError>;

    /// Save a tool policy override.
    async fn save_tool_policy(
        &self,
        tool: &str,
        policy: &crate::enforcement::domain::ToolPolicy,
    ) -> Result<(), EnforcementError>;
}

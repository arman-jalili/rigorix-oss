//! Repository interfaces for the Risk Gating bounded context.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: Contract Freeze — RiskConfigRepository trait
//! Issue: issue-contract-freeze
//!
//! Risk configuration is typically loaded from the global configuration at
//! startup. However, for runtime policy updates, per-execution overrides,
//! or persistence across restarts, repository interfaces are provided.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;

use crate::risk_gating::domain::{RiskConfig, RiskGatingError, RiskLevel};

/// Repository for loading and persisting risk configuration and tool overrides.
///
/// The default implementation loads from the global `Config` object.
/// Custom implementations may load from a database, remote API, or
/// file-based configuration store.
#[async_trait]
pub trait RiskConfigRepository: Send + Sync {
    /// Load risk configuration for an execution.
    ///
    /// Returns the full `RiskConfig` including tool overrides and
    /// gating policy flags. If no configuration is found, returns
    /// the default `RiskConfig`.
    async fn load_config(&self, execution_id: &str) -> Result<RiskConfig, RiskGatingError>;

    /// Save risk configuration for an execution.
    ///
    /// Persists any runtime modifications to tool overrides or
    /// gating policy flags. If persistence is not supported,
    /// returns `Ok(())` without error.
    async fn save_config(
        &self,
        execution_id: &str,
        config: &RiskConfig,
    ) -> Result<(), RiskGatingError>;

    /// Load a specific tool risk level override.
    ///
    /// Returns `None` if no override exists for this tool.
    async fn load_tool_override(&self, tool: &str) -> Result<Option<RiskLevel>, RiskGatingError>;

    /// Save a tool risk level override.
    ///
    /// If the tool already has an override, it is updated.
    async fn save_tool_override(
        &self,
        tool: &str,
        risk_level: &RiskLevel,
    ) -> Result<(), RiskGatingError>;

    /// Remove a tool risk level override, restoring default classification.
    ///
    /// Returns `Ok(true)` if an override was removed, `Ok(false)` if
    /// no override existed.
    async fn remove_tool_override(&self, tool: &str) -> Result<bool, RiskGatingError>;
}

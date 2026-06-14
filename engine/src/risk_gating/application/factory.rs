//! Factory interfaces for constructing Risk Gating domain objects.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: Contract Freeze — RiskGateFactory trait
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of the RiskGateService with
//! appropriate classifier and configuration loaded from settings.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured RiskGateService
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::risk_gating::domain::{RiskConfig, RiskGatingError};

use super::service::RiskGateService;

/// Factory for constructing `RiskGateService` instances.
///
/// Handles creation of the risk gate service with appropriate
/// classifier rules and configuration. Supports presets and
/// custom overrides.
#[async_trait]
pub trait RiskGateFactory: Send + Sync {
    /// Create a `RiskGateService` from a `RiskConfig`.
    ///
    /// Builds the full risk gate state with:
    /// - A default RiskClassifier (built-in tool→risk mapping)
    /// - The provided RiskConfig (tool overrides + gating flags)
    ///
    /// If the config has tool_overrides, they are merged into the
    /// classifier's rule set at construction time.
    async fn create_from_config(
        &self,
        execution_id: &str,
        config: RiskConfig,
    ) -> Result<Box<dyn RiskGateService>, RiskGatingError>;

    /// Create a `RiskGateService` with default configuration.
    ///
    /// Uses `RiskConfig::default()` with empty overrides and
    /// standard gating flags (auto-confirm Low, review Medium,
    /// dry-run High).
    async fn create_default(
        &self,
        execution_id: &str,
    ) -> Result<Box<dyn RiskGateService>, RiskGatingError>;

    /// Create a `RiskGateService` with custom tool overrides.
    ///
    /// Merges the provided overrides with default gating flags.
    /// If an override already exists for a tool in the config,
    /// the provided value takes precedence.
    async fn create_with_overrides(
        &self,
        execution_id: &str,
        config: RiskConfig,
        additional_overrides: std::collections::HashMap<String, crate::risk_gating::domain::RiskLevel>,
    ) -> Result<Box<dyn RiskGateService>, RiskGatingError>;

    /// Create a `RiskGateService` with explicit gating policy flags.
    ///
    /// Allows specifying non-default gating behavior (e.g., disabling
    /// auto-confirm for Low-risk tools).
    async fn create_with_policy(
        &self,
        execution_id: &str,
        config: RiskConfig,
        auto_confirm_low: bool,
        require_review_medium: bool,
        dry_run_high: bool,
    ) -> Result<Box<dyn RiskGateService>, RiskGatingError>;
}

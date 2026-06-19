//! Factory interfaces for constructing PolicyEngine domain objects.
//!
//! @canonical .pi/architecture/modules/policy-engine.md
//! Implements: Contract Freeze — PolicyEngineFactory trait
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of the PolicyEngineService
//! with appropriate rules loaded from configuration or a repository.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured PolicyEngineService
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::policy_engine::domain::{PolicyConfig, PolicyEngineError};

use super::engine::PolicyEngineService;

/// Factory for constructing `PolicyEngineService` instances.
///
/// Handles creation of the policy engine with appropriate rules loaded
/// from configuration files, repositories, or inline definitions.
#[async_trait]
pub trait PolicyEngineFactory: Send + Sync {
    /// Create a `PolicyEngineService` from a `PolicyConfig`.
    ///
    /// Builds the full engine state with rules loaded from the config.
    /// Rules are validated during construction — duplicate names or
    /// invalid configurations return an error.
    async fn create_from_config(
        &self,
        config: PolicyConfig,
    ) -> Result<Box<dyn PolicyEngineService>, PolicyEngineError>;

    /// Create a `PolicyEngineService` using default rules.
    ///
    /// Uses a set of sensible default policy rules suitable for
    /// standard operation without user configuration.
    async fn create_default(&self) -> Result<Box<dyn PolicyEngineService>, PolicyEngineError>;

    /// Create a `PolicyEngineService` with inline rules.
    ///
    /// Useful for testing or programmatic rule definitions.
    /// Rules are validated before the engine is returned.
    async fn create_with_rules(
        &self,
        rule_definitions: Vec<crate::policy_engine::domain::config::RuleDefinition>,
    ) -> Result<Box<dyn PolicyEngineService>, PolicyEngineError>;

    /// Create a `PolicyEngineService` that loads rules from a repository on demand.
    ///
    /// The engine will defer rule loading until first evaluation or explicitly
    /// via `load_rules()`. The repository is queried each time rules are loaded.
    async fn create_with_repository(
        &self,
        repository: Box<dyn crate::policy_engine::infrastructure::repository::PolicyRepository>,
    ) -> Result<Box<dyn PolicyEngineService>, PolicyEngineError>;
}

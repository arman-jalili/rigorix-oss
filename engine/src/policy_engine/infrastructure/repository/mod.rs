//! Repository interfaces for the Policy Engine bounded context.
//!
//! @canonical .pi/architecture/modules/policy-engine.md
//! Implements: Contract Freeze — PolicyRepository trait
//! Issue: issue-contract-freeze
//!
//! Policy rules can be loaded from multiple sources: configuration files
//! (`.rigorix/policy.toml`), runtime overrides, or external policy stores.
//! The repository interface abstracts all persistence concerns.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;

use crate::policy_engine::domain::{PolicyConfig, PolicyEngineError, PolicyRule};

/// Repository for loading and persisting policy rules.
///
/// The default implementation loads from `.rigorix/policy.toml`.
/// Custom implementations may load from a database, remote API, or
/// file-based policy store.
#[async_trait]
pub trait PolicyRepository: Send + Sync {
    /// Load policy configuration from the configured source.
    ///
    /// Returns the full `PolicyConfig` including all rule definitions.
    /// If no configuration is found, returns the default configuration
    /// with sensible built-in rules.
    async fn load_config(&self) -> Result<PolicyConfig, PolicyEngineError>;

    /// Save policy configuration to the configured source.
    ///
    /// Persists any runtime modifications to rules.
    /// If persistence is not supported, returns `Ok(())` without error.
    async fn save_config(&self, config: &PolicyConfig) -> Result<(), PolicyEngineError>;

    /// Load a specific rule by name.
    ///
    /// Returns `None` if no rule with the given name exists.
    async fn load_rule(&self, name: &str) -> Result<Option<PolicyRule>, PolicyEngineError>;

    /// Save or update a specific rule.
    ///
    /// If a rule with the same name already exists, it is replaced.
    async fn save_rule(&self, rule: &PolicyRule) -> Result<(), PolicyEngineError>;

    /// Delete a specific rule by name.
    ///
    /// Returns `Ok(())` even if the rule didn't exist.
    async fn delete_rule(&self, name: &str) -> Result<(), PolicyEngineError>;

    /// List all available rule names in the repository.
    ///
    /// Returns an empty list if no rules are configured.
    async fn list_rules(&self) -> Result<Vec<String>, PolicyEngineError>;
}

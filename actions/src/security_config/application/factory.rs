//! Factory interfaces for constructing Security Configuration domain objects.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md
//! Implements: Contract Freeze — SecurityContextFactory, SecurityPolicyFactory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::security_config::domain::{
    SecurityContext, SecurityError, SecurityLevel, SecurityPolicy,
};

/// Factory for constructing `SecurityContext` from raw check results.
///
/// Handles the resolution of individual check results into a unified
/// security context with the correct security level.
#[async_trait]
pub trait SecurityContextFactory: Send + Sync {
    /// Build a `SecurityContext` from individual check results.
    ///
    /// Determines the `SecurityLevel` based on:
    /// - Fork PR + no API key → `Restricted`
    /// - Token invalid → `Blocked`
    /// - URL blocked → `Blocked`
    /// - All pass → `Full`
    async fn build(
        &self,
        is_fork_pr: bool,
        has_required_permissions: bool,
        api_key_masked: bool,
        backend_url_allowed: bool,
        policy_changed_from_base: bool,
        org_policy_loaded: bool,
    ) -> SecurityContext;

    /// Build a `SecurityContext` at the `Blocked` level.
    /// Used when critical checks fail.
    fn blocked(&self) -> SecurityContext;

    /// Build a `SecurityContext` at the `Full` level.
    /// Used when all checks pass.
    fn full(&self) -> SecurityContext;

    /// Determine the security level from individual check results.
    fn resolve_security_level(
        &self,
        is_fork_pr: bool,
        has_required_permissions: bool,
        api_key_masked: bool,
        backend_url_allowed: bool,
    ) -> SecurityLevel;
}

/// Factory for constructing `SecurityPolicy` from TOML configuration.
///
/// Handles parsing and validation of the `.rigorix/security.toml` file.
#[allow(clippy::wrong_self_convention)]
#[async_trait]
pub trait SecurityPolicyFactory: Send + Sync {
    /// Build a `SecurityPolicy` from raw TOML content.
    async fn from_toml(&self, toml_content: &str) -> Result<SecurityPolicy, SecurityError>;

    /// Create a default security policy with safe defaults.
    fn defaults(&self) -> SecurityPolicy;

    /// Merge a loaded policy with defaults (org policy overrides local).
    async fn merge(
        &self,
        local: SecurityPolicy,
        org: SecurityPolicy,
    ) -> Result<SecurityPolicy, SecurityError>;
}

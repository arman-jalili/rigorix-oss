//! Factory interface for constructing PermissionEnforcer instances.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md
//! Implements: Contract Freeze — PermissionEnforcerFactory trait
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of the PermissionEnforcer
//! with appropriate mode, rules, and configuration.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured PermissionEnforcer
//! - Configuration is validated during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use super::enforcer::PermissionEnforcer;
use crate::permission::domain::{PermissionConfig, PermissionError, PermissionMode};

/// Factory for constructing `PermissionEnforcer` instances.
///
/// Handles creation of the enforcer with the appropriate permission
/// mode, allow/deny/ask rules, and per-tool permission mappings.
#[async_trait]
pub trait PermissionEnforcerFactory: Send + Sync {
    /// Create a `PermissionEnforcer` from a `PermissionConfig`.
    async fn create_from_config(
        &self,
        config: PermissionConfig,
    ) -> Result<Box<dyn PermissionEnforcer>, PermissionError>;

    /// Create a `PermissionEnforcer` with the default configuration
    /// in the specified mode.
    async fn create_with_mode(
        &self,
        mode: PermissionMode,
    ) -> Result<Box<dyn PermissionEnforcer>, PermissionError>;

    /// Create a `PermissionEnforcer` using the default configuration.
    async fn create_default(&self) -> Result<Box<dyn PermissionEnforcer>, PermissionError>;

    /// Create an extremely permissive `PermissionEnforcer` (for testing).
    async fn create_permissive(&self) -> Result<Box<dyn PermissionEnforcer>, PermissionError>;

    /// Create a read-only `PermissionEnforcer`.
    async fn create_read_only(&self) -> Result<Box<dyn PermissionEnforcer>, PermissionError>;
}

//! Repository interfaces for the Quality Gates bounded context.
//!
//! @canonical .pi/architecture/modules/quality-gates.md
//! Implements: Contract Freeze — QualityGateConfigRepository trait
//! Issue: #449 (quality-gates epic)
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use filesystem, environment, or mock storage
//! without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::quality_gates::domain::{
    GreenContract, QualityGateConfig, QualityGateError, QualityLevel,
};

/// Repository for storing and retrieving quality gate configuration.
///
/// Implementations can load configuration from filesystem (TOML/JSON),
/// environment variables, or in-memory defaults.
#[async_trait]
pub trait QualityGateConfigRepository: Send + Sync {
    /// Load the quality gate configuration.
    ///
    /// Returns the default configuration if no custom config is found.
    async fn load_config(&self) -> Result<QualityGateConfig, QualityGateError>;

    /// Store a quality gate configuration.
    ///
    /// Implementations persist the configuration to their storage backend.
    async fn store_config(&self, config: &QualityGateConfig) -> Result<(), QualityGateError>;

    /// Get the green contract for a specific template or task.
    ///
    /// Checks template-level overrides, then falls back to the default level.
    async fn contract_for_template(
        &self,
        template_name: &str,
    ) -> Result<GreenContract, QualityGateError>;

    /// Get the default green contract.
    async fn default_contract(&self) -> Result<GreenContract, QualityGateError>;

    /// Update the default required quality level.
    async fn set_default_level(&self, level: QualityLevel) -> Result<(), QualityGateError>;

    /// Add or update a template-level quality override.
    async fn set_template_level(
        &self,
        template_name: &str,
        level: QualityLevel,
    ) -> Result<(), QualityGateError>;

    /// Remove a template-level quality override.
    async fn remove_template_override(
        &self,
        template_name: &str,
    ) -> Result<Option<QualityLevel>, QualityGateError>;
}

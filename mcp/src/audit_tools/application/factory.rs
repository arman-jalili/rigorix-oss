//! Factory interfaces for constructing Audit Tools domain objects.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#factories
//! Implements: Contract Freeze — AuditQueryServiceFactory, AuditFormatterFactory interfaces
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//!
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations
//! - Factory methods are async where construction involves I/O

use async_trait::async_trait;
use std::sync::Arc;

use crate::audit_tools::domain::entity::{AuditFormatter, AuditQueryService};
use crate::audit_tools::domain::error::AuditError;
use crate::execution_tools::domain::entity::SharedEngineFacade;

// ---------------------------------------------------------------------------
// AuditQueryServiceFactory
// ---------------------------------------------------------------------------

/// Factory for constructing AuditQueryService implementations.
///
/// Encapsulates the construction of the AuditQueryService aggregate root,
/// wiring it to the EngineFacade and optional configuration.
#[async_trait]
pub trait AuditQueryServiceFactory: Send + Sync {
    /// Create a new AuditQueryService instance.
    ///
    /// Wires the service to the given EngineFacade for querying
    /// rigorix-engine audit data.
    async fn create(
        &self,
        engine: SharedEngineFacade,
    ) -> Result<Arc<dyn AuditQueryService>, AuditError>;
}

// ---------------------------------------------------------------------------
// AuditFormatterFactory
// ---------------------------------------------------------------------------

/// Factory for constructing AuditFormatter implementations.
///
/// Encapsulates the construction of the AuditFormatter with
/// optional formatting configuration (e.g., date format, max list items).
#[async_trait]
pub trait AuditFormatterFactory: Send + Sync {
    /// Create a new AuditFormatter instance with default settings.
    async fn create_default(&self) -> Arc<dyn AuditFormatter>;

    /// Create a new AuditFormatter with custom configuration.
    async fn create_with_config(&self, config: FormatterConfig) -> Arc<dyn AuditFormatter>;
}

// ---------------------------------------------------------------------------
// FormatterConfig — configuration for AuditFormatter
// ---------------------------------------------------------------------------

/// Configuration for AuditFormatter construction.
///
/// Controls formatting behavior such as date format and list limits.
#[derive(Debug, Clone)]
pub struct FormatterConfig {
    /// Date/time format string (chrono format syntax).
    pub date_format: String,

    /// Maximum number of items to include in formatted lists.
    pub max_list_items: usize,

    /// Whether to include step details in text format.
    pub include_step_details: bool,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            date_format: "%Y-%m-%d %H:%M:%S UTC".into(),
            max_list_items: 50,
            include_step_details: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Handler constructor helper
// ---------------------------------------------------------------------------

/// Dependencies required by audit handler implementations.
///
/// Provides a shared dependency set that handler constructors can use
/// to wire up all three audit handlers consistently.
#[derive(Clone)]
pub struct AuditHandlerDependencies {
    /// The AuditQueryService to delegate all audit queries to.
    pub query_service: Arc<dyn AuditQueryService>,

    /// The AuditFormatter to format audit data for MCP output.
    pub formatter: Arc<dyn AuditFormatter>,
}

impl AuditHandlerDependencies {
    /// Create audit handler dependencies.
    pub fn new(
        query_service: Arc<dyn AuditQueryService>,
        formatter: Arc<dyn AuditFormatter>,
    ) -> Self {
        Self {
            query_service,
            formatter,
        }
    }
}

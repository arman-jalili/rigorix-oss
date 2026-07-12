//! Factory interfaces for constructing Execution Tools domain objects.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#factories
//! Implements: Contract Freeze — EngineFacadeFactory, PlanTemplateFactory interfaces
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
use std::time::Duration;

use crate::execution_tools::domain::entity::EngineFacade;
use crate::execution_tools::domain::error::EngineFacadeError;
use crate::execution_tools::domain::value::PlanTemplate;

use super::dto::ExecuteInput;

// ---------------------------------------------------------------------------
// EngineFacadeFactory
// ---------------------------------------------------------------------------

/// Factory for constructing EngineFacade implementations.
///
/// Encapsulates the construction of the EngineFacade aggregate root,
/// wiring it to rigorix-engine components and configuration.
#[async_trait]
pub trait EngineFacadeFactory: Send + Sync {
    /// Create a new EngineFacade instance.
    ///
    /// Accepts optional configuration overrides and returns a
    /// fully wired EngineFacade ready for execution/validation.
    async fn create(
        &self,
        config_overrides: Option<EngineFacadeConfig>,
    ) -> Result<Arc<dyn EngineFacade>, EngineFacadeError>;
}

// ---------------------------------------------------------------------------
// EngineFacadeConfig — configuration for EngineFacade construction
// ---------------------------------------------------------------------------

/// Configuration for EngineFacade construction.
///
/// Controls timeouts, enforcement settings, and engine connectivity.
#[derive(Debug, Clone)]
pub struct EngineFacadeConfig {
    /// Default timeout for execute operations in seconds.
    pub default_execute_timeout_secs: u64,

    /// Default timeout for validation operations in seconds.
    pub default_validate_timeout_secs: u64,

    /// Whether to enable enforcement checks.
    pub enable_enforcement: bool,

    /// Engine endpoint URL (for remote engine connections).
    pub engine_endpoint: Option<String>,

    /// Connection timeout in seconds.
    pub connection_timeout_secs: u64,
}

impl Default for EngineFacadeConfig {
    fn default() -> Self {
        Self {
            default_execute_timeout_secs: 300,
            default_validate_timeout_secs: 60,
            enable_enforcement: true,
            engine_endpoint: None,
            connection_timeout_secs: 10,
        }
    }
}

// ---------------------------------------------------------------------------
// PlanTemplateFactory
// ---------------------------------------------------------------------------

/// Factory for constructing PlanTemplate value objects.
///
/// Encapsulates plan construction logic, defaults, and validation.
#[async_trait]
pub trait PlanTemplateFactory: Send + Sync {
    /// Create a PlanTemplate from raw input.
    ///
    /// Parses and validates the plan structure, applying any defaults
    /// for optional fields.
    async fn create_from_input(
        &self,
        input: &ExecuteInput,
    ) -> Result<PlanTemplate, EngineFacadeError>;

    /// Create a PlanTemplate from JSON value.
    ///
    /// Deserializes and validates in one step, returning domain errors.
    #[allow(clippy::wrong_self_convention)]
    async fn from_json(&self, value: serde_json::Value) -> Result<PlanTemplate, EngineFacadeError>;

    /// Create a default PlanTemplate for testing.
    ///
    /// Returns a minimal valid plan with one step.
    fn default_test_plan(&self) -> PlanTemplate;
}

// ---------------------------------------------------------------------------
// Handler constructor helper
// ---------------------------------------------------------------------------

/// Dependencies required by handler implementations.
///
/// Provides a shared dependency set that handler constructors can use
/// to wire up all three handlers consistently.
#[derive(Clone)]
pub struct HandlerDependencies {
    /// The EngineFacade to delegate all engine operations to.
    pub engine: Arc<dyn EngineFacade>,

    /// Timeout for execute operations.
    pub execute_timeout: Duration,
}

impl HandlerDependencies {
    /// Create handler dependencies.
    pub fn new(engine: Arc<dyn EngineFacade>, execute_timeout: Duration) -> Self {
        Self {
            engine,
            execute_timeout,
        }
    }
}

//! Factory interfaces for constructing Execution Engine service instances.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: Contract Freeze — ParallelExecutionFactory and RetryEvaluationFactory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of ParallelExecutionService and
//! RetryEvaluationService instances with appropriate configuration, dependencies,
//! and integration points (event bus, cancellation, enforcement).
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured service instance
//! - Configuration is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;
use std::sync::Arc;

use crate::event_system::application::EventBusService;
use crate::execution_engine::domain::ExecutionError;

use super::service::{ParallelExecutionService, RetryEvaluationService};

/// Factory for constructing `ParallelExecutionService` instances.
///
/// Handles creation of the parallel executor with all necessary dependencies:
/// - Tool system for executing node tools
/// - Event bus for emitting execution events
/// - Cancellation token for graceful shutdown
/// - Enforcement service for limiting operations
/// - Retry evaluation service for retry decisions
#[async_trait]
pub trait ParallelExecutionFactory: Send + Sync {
    /// Create a `ParallelExecutionService` instance.
    ///
    /// Builds the executor with all configuration settings and integration
    /// points wired together.
    async fn create(
        &self,
        config: ParallelExecutionFactoryConfig,
    ) -> Result<Box<dyn ParallelExecutionService>, ExecutionError>;
}

/// Configuration for creating a `ParallelExecutionService` instance.
#[derive(Clone)]
pub struct ParallelExecutionFactoryConfig {
    /// The parallel executor configuration.
    pub executor_config: crate::execution_engine::domain::ParallelExecutorConfig,

    /// Whether to register event bus subscribers for execution events.
    pub register_event_handlers: bool,

    /// Whether to enable progress callbacks.
    pub enable_progress_callbacks: bool,

    /// Event bus channel capacity for execution events.
    pub event_channel_capacity: usize,

    /// The event bus service for publishing execution lifecycle events.
    pub event_bus: Option<Arc<dyn EventBusService>>,
}

impl Default for ParallelExecutionFactoryConfig {
    fn default() -> Self {
        Self {
            executor_config: crate::execution_engine::domain::ParallelExecutorConfig::default(),
            register_event_handlers: true,
            enable_progress_callbacks: true,
            event_channel_capacity: 1024,
            event_bus: None,
        }
    }
}

/// Factory for constructing `RetryEvaluationService` instances.
///
/// Handles creation of the retry evaluation service with retry policy defaults
/// and strategy mappings for different failure types.
#[async_trait]
pub trait RetryEvaluationFactory: Send + Sync {
    /// Create a `RetryEvaluationService` instance.
    ///
    /// Configures the service with default retry policies and strategy
    /// mappings for various failure classification types.
    async fn create(
        &self,
        config: RetryEvaluationFactoryConfig,
    ) -> Result<Box<dyn RetryEvaluationService>, ExecutionError>;
}

/// Configuration for creating a `RetryEvaluationService` instance.
#[derive(Debug, Clone)]
pub struct RetryEvaluationFactoryConfig {
    /// Default retry policy to use when no policy is specified.
    pub default_policy: crate::execution_engine::domain::RetryPolicy,

    /// Mapping of failure types to preferred RetryStrategy.
    /// Keys are failure type strings (e.g., "transient", "compile_error").
    pub failure_strategy_mapping: Vec<FailureStrategyOverride>,

    /// Whether to enable detailed logging of retry decisions.
    pub enable_decision_logging: bool,
}

impl Default for RetryEvaluationFactoryConfig {
    fn default() -> Self {
        Self {
            default_policy: crate::execution_engine::domain::RetryPolicy::default(),
            failure_strategy_mapping: Vec::new(),
            enable_decision_logging: true,
        }
    }
}

/// Override mapping between a failure type and its preferred retry strategy.
#[derive(Debug, Clone)]
pub struct FailureStrategyOverride {
    /// The failure type string (e.g., "transient", "compile_error").
    pub failure_type: String,
    /// The preferred retry strategy for this failure type.
    pub preferred_strategy: crate::execution_engine::domain::RetryStrategy,
}

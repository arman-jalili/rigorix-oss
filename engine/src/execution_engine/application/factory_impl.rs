//! Factory implementations for constructing Execution Engine service instances.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: ExecutionEngine — ParallelExecutionFactoryImpl, RetryEvaluationFactoryImpl
//! Issue: issue-retry-logic, issue-parallelexecutor
//!
//! Concrete factory implementations that wire up service instances with
//! configuration settings.

use async_trait::async_trait;

use crate::execution_engine::application::factory::{
    ParallelExecutionFactory, ParallelExecutionFactoryConfig, RetryEvaluationFactory,
    RetryEvaluationFactoryConfig,
};
use crate::execution_engine::application::service::{
    ParallelExecutionService, RetryEvaluationService,
};
use crate::execution_engine::application::service_impl::{
    ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
};
use crate::execution_engine::domain::ExecutionError;

/// Factory implementation for constructing `ParallelExecutionService` instances.
///
/// Creates ParallelExecutionServiceImpl instances with the given configuration,
/// wiring in a RetryEvaluationServiceImpl for retry decision-making.
pub struct ParallelExecutionFactoryImpl;

impl ParallelExecutionFactoryImpl {
    /// Create a new ParallelExecutionFactoryImpl.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ParallelExecutionFactoryImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ParallelExecutionFactory for ParallelExecutionFactoryImpl {
    async fn create(
        &self,
        config: ParallelExecutionFactoryConfig,
    ) -> Result<Box<dyn ParallelExecutionService>, ExecutionError> {
        let retry_service = Box::new(RetryEvaluationServiceImpl::new());
        // Use a default event bus if none was provided
        let event_bus = config
            .event_bus
            .unwrap_or_else(|| std::sync::Arc::new(crate::event_system::application::event_bus_service_impl::EventBusServiceImpl::default()));
        let executor =
            ParallelExecutionServiceImpl::new(config.executor_config, retry_service, event_bus);
        Ok(Box::new(executor))
    }
}

/// Factory implementation for constructing `RetryEvaluationService` instances.
///
/// Creates RetryEvaluationServiceImpl instances with the given configuration.
/// The service is stateless, so the config primarily controls validation
/// and logging settings.
pub struct RetryEvaluationFactoryImpl;

impl RetryEvaluationFactoryImpl {
    /// Create a new RetryEvaluationFactoryImpl.
    pub fn new() -> Self {
        Self
    }
}

impl Default for RetryEvaluationFactoryImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RetryEvaluationFactory for RetryEvaluationFactoryImpl {
    async fn create(
        &self,
        _config: RetryEvaluationFactoryConfig,
    ) -> Result<Box<dyn RetryEvaluationService>, ExecutionError> {
        Ok(Box::new(RetryEvaluationServiceImpl::new()))
    }
}

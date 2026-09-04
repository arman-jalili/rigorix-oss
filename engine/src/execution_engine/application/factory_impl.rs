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
use crate::failure_classification::application::failure_classifier_service_impl::FailureClassifierServiceImpl;

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
        // GAP-A-19: the production retry loop is driven by structured failure
        // classification (with policy fallback for unclassified failures).
        let retry_service = Box::new(RetryEvaluationServiceImpl::with_classifier(
            std::sync::Arc::new(FailureClassifierServiceImpl),
        ));
        // Use a default event bus if none was provided
        let event_bus = config
            .event_bus
            .unwrap_or_else(|| std::sync::Arc::new(crate::event_system::application::event_bus_service_impl::EventBusServiceImpl::default()));
        let mut executor =
            ParallelExecutionServiceImpl::new(config.executor_config, retry_service, event_bus);
        if let Some(enforcer) = config.permission_enforcer {
            executor = executor.with_permission_enforcer(enforcer);
        }
        if let Some(runner) = config.hook_runner {
            executor = executor.with_hook_runner(runner);
        }
        if let Some(binding) = config.approval_binding {
            // ADR-011: attach the approval binding with a live session-graph
            // intent resolver — approve/verify/consume now run at the runtime
            // choke point (records persist through the supplied repository).
            let sessions = executor.sessions_handle();
            let resolver = std::sync::Arc::new(
                crate::execution_engine::application::service_impl::SessionGraphResolver::new(
                    sessions,
                ),
            );
            let service = crate::approval::application::ApprovalServiceImpl::new(
                binding.repository,
                resolver,
                binding.run_key,
                std::time::Duration::from_secs(binding.ttl_seconds),
            );
            executor = executor.with_approval_service(std::sync::Arc::new(service));
        }
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

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

    /// Build the concrete executor with the full config applied (no boxing).
    ///
    /// Factored out of [`ParallelExecutionFactory::create`] so tests and
    /// callers can inspect the attached wiring without a trait-object downcast.
    fn build_executor(config: ParallelExecutionFactoryConfig) -> ParallelExecutionServiceImpl {
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
        if let Some(svc) = config.sequence_policy {
            executor = executor.with_sequence_policy(svc);
        }
        // ADR-017 R1/R2: attach the operator-authored precondition gate and
        // step-outcome gating mode when the composition root armed them.
        // `None` keeps the status-quo dispatch path (no gate).
        if let Some(setup) = config.precondition {
            executor = executor
                .with_gating_mode(setup.gating_mode)
                .with_precondition_gate(setup.gate);
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
        executor
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
        Ok(Box::new(Self::build_executor(config)))
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

#[cfg(test)]
mod precondition_wiring_tests {
    use super::*;

    /// ACT-1: a factory built with a `PreconditionSetup` produces an executor
    /// that actually holds the gate and the `[gating]` mode (the dormant-gate
    /// gap this epic closes).
    #[tokio::test]
    async fn factory_attaches_precondition_gate_and_gating_mode() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rigorix = dir.path().join(".rigorix");
        std::fs::create_dir_all(&rigorix).expect("create .rigorix");
        std::fs::write(
            rigorix.join("preconditions.toml"),
            r#"
[[preconditions]]
id = "guard"
match = { tool = "payment_execute" }
command = ["/bin/true"]

[gating]
release_dependents_on_failure = false
"#,
        )
        .expect("write fixture");

        let setup = crate::precondition::PreconditionSetup::from_env(dir.path())
            .expect("from_env")
            .expect("fixture present");
        let config = ParallelExecutionFactoryConfig {
            precondition: Some(setup),
            ..Default::default()
        };

        let executor = ParallelExecutionFactoryImpl::build_executor(config);
        assert!(
            executor.precondition_gate_present(),
            "the factory must attach the gate when a setup is configured"
        );
        assert!(
            !executor.gating_mode_value().release_dependents_on_failure,
            "the factory must apply the [gating] mode from the setup"
        );
    }

    #[test]
    fn factory_config_default_leaves_gate_unset() {
        assert!(
            ParallelExecutionFactoryConfig::default()
                .precondition
                .is_none()
        );
    }
}

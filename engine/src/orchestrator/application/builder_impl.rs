//! Implementation of `OrchestratorBuilder`.
//!
//! @canonical .pi/architecture/modules/orchestrator.md#builder
//! Implements: Issue #340 — OrchestratorBuilder concrete implementation
//! Issue: #340
//!
//! Builder pattern for constructing an OrchestratorService from config.
//! Accepts optional service overrides for testing/dependency injection.
//!
//! When no overrides are provided, `build()` returns an error indicating
//! that all sub-services must be explicitly wired. Use `with_*` methods
//! to inject real or mock implementations.

use async_trait::async_trait;
use std::sync::Arc;

use crate::code_graph::application::CodeGraphService;
use crate::orchestrator::domain::OrchestratorConfig;
use crate::orchestrator::domain::OrchestratorError;

use super::builder::OrchestratorBuilder;
use super::orchestrator_impl::OrchestratorServiceImpl;
use super::service::OrchestratorService;

/// Concrete builder for constructing `OrchestratorService`.
pub struct OrchestratorBuilderImpl {
    config: OrchestratorConfig,
    repo_root: Option<String>,
    enforcement_preset: Option<String>,
    max_llm_calls: Option<u32>,
    max_llm_tokens: Option<u64>,

    // Optional service overrides (for testing / DI)
    planning_pipeline: Option<Arc<dyn crate::planning::application::PlanningPipelineService>>,
    execution_service:
        Option<Arc<dyn crate::execution_engine::application::service::ParallelExecutionService>>,
    state_manager:
        Option<Arc<dyn crate::state_persistence::application::service::StateManagerService>>,
    cancellation_service: Option<Arc<dyn crate::cancellation::application::CancellationService>>,
    event_bus: Option<Arc<dyn crate::event_system::application::EventBusService>>,
    audit_service: Option<Arc<dyn crate::audit::application::AuditService>>,
    budget_service: Option<Arc<dyn crate::budget_tracking::application::LlmBudgetService>>,
    code_graph_service: Option<Arc<dyn CodeGraphService>>,
}

impl OrchestratorBuilderImpl {
    /// Override the planning pipeline service.
    pub fn with_planning_pipeline(
        mut self,
        svc: Arc<dyn crate::planning::application::PlanningPipelineService>,
    ) -> Self {
        self.planning_pipeline = Some(svc);
        self
    }

    /// Override the parallel execution service.
    pub fn with_execution_service(
        mut self,
        svc: Arc<dyn crate::execution_engine::application::service::ParallelExecutionService>,
    ) -> Self {
        self.execution_service = Some(svc);
        self
    }

    /// Override the state manager service.
    pub fn with_state_manager(
        mut self,
        svc: Arc<dyn crate::state_persistence::application::service::StateManagerService>,
    ) -> Self {
        self.state_manager = Some(svc);
        self
    }

    /// Override the cancellation service.
    pub fn with_cancellation_service(
        mut self,
        svc: Arc<dyn crate::cancellation::application::CancellationService>,
    ) -> Self {
        self.cancellation_service = Some(svc);
        self
    }

    /// Override the event bus service.
    pub fn with_event_bus(
        mut self,
        svc: Arc<dyn crate::event_system::application::EventBusService>,
    ) -> Self {
        self.event_bus = Some(svc);
        self
    }

    /// Override the audit service.
    pub fn with_audit_service(
        mut self,
        svc: Arc<dyn crate::audit::application::AuditService>,
    ) -> Self {
        self.audit_service = Some(svc);
        self
    }

    /// Override the budget service.
    pub fn with_budget_service(
        mut self,
        svc: Arc<dyn crate::budget_tracking::application::LlmBudgetService>,
    ) -> Self {
        self.budget_service = Some(svc);
        self
    }

    /// Override the code graph service.
    pub fn with_code_graph_service(mut self, svc: Arc<dyn CodeGraphService>) -> Self {
        self.code_graph_service = Some(svc);
        self
    }

    /// Check if all required services are provided.
    fn check_ready(&self) -> Result<(), OrchestratorError> {
        if self.planning_pipeline.is_none() {
            return Err(OrchestratorError::Internal {
                detail: "planning_pipeline is required — use with_planning_pipeline()".into(),
                source_module: "OrchestratorBuilder".into(),
            });
        }
        if self.execution_service.is_none() {
            return Err(OrchestratorError::Internal {
                detail: "execution_service is required — use with_execution_service()".into(),
                source_module: "OrchestratorBuilder".into(),
            });
        }
        if self.state_manager.is_none() {
            return Err(OrchestratorError::Internal {
                detail: "state_manager is required — use with_state_manager()".into(),
                source_module: "OrchestratorBuilder".into(),
            });
        }
        if self.cancellation_service.is_none() {
            return Err(OrchestratorError::Internal {
                detail: "cancellation_service is required — use with_cancellation_service()".into(),
                source_module: "OrchestratorBuilder".into(),
            });
        }
        if self.event_bus.is_none() {
            return Err(OrchestratorError::Internal {
                detail: "event_bus is required — use with_event_bus()".into(),
                source_module: "OrchestratorBuilder".into(),
            });
        }
        if self.budget_service.is_none() {
            return Err(OrchestratorError::Internal {
                detail: "budget_service is required — use with_budget_service()".into(),
                source_module: "OrchestratorBuilder".into(),
            });
        }
        Ok(())
    }
}

#[async_trait]
impl OrchestratorBuilder for OrchestratorBuilderImpl {
    fn new(config: OrchestratorConfig) -> Self {
        Self {
            config,
            repo_root: None,
            enforcement_preset: None,
            max_llm_calls: None,
            max_llm_tokens: None,
            planning_pipeline: None,
            execution_service: None,
            state_manager: None,
            cancellation_service: None,
            event_bus: None,
            audit_service: None,
            budget_service: None,
            code_graph_service: None,
        }
    }

    fn with_repo_root(mut self, repo_root: String) -> Self {
        self.repo_root = Some(repo_root);
        self
    }

    fn with_enforcement_preset(mut self, preset: String) -> Self {
        self.enforcement_preset = Some(preset);
        self
    }

    fn with_llm_budget(mut self, max_calls: u32, max_tokens: u64) -> Self {
        self.max_llm_calls = Some(max_calls);
        self.max_llm_tokens = Some(max_tokens);
        self
    }

    fn with_code_graph_service(mut self, svc: Arc<dyn CodeGraphService>) -> Self {
        self.code_graph_service = Some(svc);
        self
    }

    async fn build(self) -> Result<Box<dyn OrchestratorService>, OrchestratorError> {
        self.check_ready()?;

        let config = self.config;
        let planning_pipeline = self.planning_pipeline.unwrap();
        let execution_service = self.execution_service.unwrap();
        let state_manager = self.state_manager.unwrap();
        let cancellation_service = self.cancellation_service.unwrap();
        let event_bus = self.event_bus.unwrap();
        let audit_service = self.audit_service;
        let budget_service = self.budget_service.unwrap();
        let code_graph_service = self.code_graph_service;

        let orchestrator = OrchestratorServiceImpl::new(
            config,
            planning_pipeline,
            execution_service,
            state_manager,
            cancellation_service,
            event_bus,
            audit_service,
            budget_service,
            code_graph_service,
        );

        Ok(Box::new(orchestrator))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::application::orchestrator_impl::mocks;
    use crate::orchestrator::domain::OrchestratorConfig;

    #[tokio::test]
    async fn test_builder_with_all_services_creates_service() {
        let result = OrchestratorBuilderImpl::new(OrchestratorConfig::default())
            .with_repo_root("/tmp/test".into())
            .with_planning_pipeline(Arc::new(mocks::MockPlanningService::new()))
            .with_execution_service(Arc::new(mocks::MockExecutionService))
            .with_state_manager(Arc::new(mocks::MockStateService::new()))
            .with_cancellation_service(Arc::new(mocks::MockCancellationService))
            .with_event_bus(Arc::new(mocks::MockEventBusService::new()))
            .with_budget_service(Arc::new(mocks::MockBudgetService))
            .build()
            .await;
        assert!(
            result.is_ok(),
            "builder should create service: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_builder_service_can_run() {
        let orchestrator = OrchestratorBuilderImpl::new(OrchestratorConfig::default())
            .with_repo_root("/tmp/test".into())
            .with_planning_pipeline(Arc::new(mocks::MockPlanningService::new()))
            .with_execution_service(Arc::new(mocks::MockExecutionService))
            .with_state_manager(Arc::new(mocks::MockStateService::new()))
            .with_cancellation_service(Arc::new(mocks::MockCancellationService))
            .with_event_bus(Arc::new(mocks::MockEventBusService::new()))
            .with_budget_service(Arc::new(mocks::MockBudgetService))
            .build()
            .await
            .unwrap();

        use crate::orchestrator::application::dto::RunInput;
        let result = orchestrator
            .run(RunInput {
                intent: "test".into(),
                config: serde_json::json!({}),
                repo_root: "/tmp/test".into(),
                enforcement_preset: None,
            })
            .await;
        assert!(result.is_ok(), "run should succeed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_builder_missing_service_returns_error() {
        let result = OrchestratorBuilderImpl::new(OrchestratorConfig::default())
            .with_repo_root("/tmp/test".into())
            .build()
            .await;
        assert!(
            result.is_err(),
            "builder should fail without required services"
        );
        if let Err(OrchestratorError::Internal { ref detail, .. }) = result {
            assert!(
                detail.contains("planning_pipeline"),
                "error should mention missing service"
            );
        } else {
            panic!("expected Internal error, got error: {:?}", result.err());
        }
    }

    #[tokio::test]
    async fn test_builder_with_audit_service() {
        let result = OrchestratorBuilderImpl::new(OrchestratorConfig::default())
            .with_repo_root("/tmp/test".into())
            .with_planning_pipeline(Arc::new(mocks::MockPlanningService::new()))
            .with_execution_service(Arc::new(mocks::MockExecutionService))
            .with_state_manager(Arc::new(mocks::MockStateService::new()))
            .with_cancellation_service(Arc::new(mocks::MockCancellationService))
            .with_event_bus(Arc::new(mocks::MockEventBusService::new()))
            .with_budget_service(Arc::new(mocks::MockBudgetService))
            .with_audit_service(Arc::new(mocks::MockAuditService::new()))
            .build()
            .await;
        assert!(
            result.is_ok(),
            "builder with audit should succeed: {:?}",
            result.err()
        );
    }
}

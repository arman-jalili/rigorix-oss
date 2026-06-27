//! Mock service implementations for orchestrator tests.
//!
//! These mocks provide deterministic implementations of engine services
//! for unit testing the orchestrator. They live in a separate file to
//! keep `orchestrator_impl.rs` focused on production code.
//!
//! # Incomplete Stubs
//!
//! Some methods are annotated with `unimplemented!()` — these are secondary
//! endpoints not exercised by current orchestrator tests. If a new test
//! needs them, implement the mock method before writing the test.

use crate::budget_tracking::application as budget_app;
use crate::cancellation::application as cancel_app;
use crate::event_system::application as event_app;
use crate::execution_engine::application::{dto as exec_dto, service as exec_svc};
use crate::planning::application::dto as planning_dto;
use crate::state_persistence::application::{dto as state_dto, service as state_svc};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use uuid::Uuid;

fn mock_pr(eid: Uuid) -> crate::planning::domain::result::PlanningResult {
    crate::planning::domain::result::PlanningResult::new(
        eid,
        "mock-template".into(),
        0.95,
        HashMap::new(),
        crate::planning::domain::result::PlanningHash("a".repeat(64)),
        false,
        2,
        500,
        None,
    )
}

// ---------------------------------------------------------------------------
// MockPlanningService
// ---------------------------------------------------------------------------

pub(crate) struct MockPlanningService {
    eid: Uuid,
}
impl MockPlanningService {
    pub fn new() -> Self {
        Self {
            eid: Uuid::new_v4(),
        }
    }
}

#[async_trait]
impl crate::planning::application::PlanningPipelineService for MockPlanningService {
    async fn plan(
        &self,
        _: planning_dto::PlanInput,
    ) -> Result<planning_dto::PlanOutput, crate::planning::domain::PlanningError> {
        Ok(planning_dto::PlanOutput {
            planning_result: mock_pr(self.eid),
            from_generator: false,
            clarification_used: false,
            total_llm_calls: 2,
            total_llm_tokens: 500,
            completed_at: chrono::Utc::now(),
        })
    }
    async fn plan_with_graph(
        &self,
        _: planning_dto::PlanWithGraphInput,
    ) -> Result<planning_dto::PlanWithGraphOutput, crate::planning::domain::PlanningError> {
        Ok(planning_dto::PlanWithGraphOutput {
            planning_result: mock_pr(self.eid),
            graph: crate::dag_engine::domain::TaskGraph::new(),
            node_count: 0,
            validation_passed: true,
            validation_warnings: vec![],
            from_generator: false,
            clarification_used: false,
            total_llm_calls: 2,
            total_llm_tokens: 500,
            completed_at: chrono::Utc::now(),
        })
    }
    async fn check_budget(
        &self,
        _: planning_dto::CheckBudgetInput,
    ) -> Result<planning_dto::CheckBudgetOutput, crate::planning::domain::PlanningError> {
        Ok(planning_dto::CheckBudgetOutput {
            has_capacity: true,
            remaining_calls: 100,
            remaining_tokens: 10000,
            max_calls: 1000,
            max_tokens: 100000,
            will_exhaust: false,
        })
    }
    async fn classify_intent(
        &self,
        _: crate::planning::domain::intent::UserIntent,
    ) -> Result<
        crate::planning::domain::classification::ClassificationResult,
        crate::planning::domain::PlanningError,
    > {
        Ok(
            crate::planning::domain::classification::ClassificationResult {
                alternatives: vec![],
                requires_clarification: false,
                needs_generator: false,
                reasoning: "mock".into(),
                llm_calls_used: 0,
                llm_tokens_used: 0,
            },
        )
    }
    async fn extract_parameters(
        &self,
        _: planning_dto::ExtractParametersInput,
    ) -> Result<planning_dto::ExtractParametersOutput, crate::planning::domain::PlanningError> {
        Ok(planning_dto::ExtractParametersOutput {
            template_id: "mock".into(),
            parameters: HashMap::new(),
            extra_parameters: HashMap::new(),
            complete: true,
            missing_parameters: vec![],
            llm_calls_used: 0,
            llm_tokens_used: 0,
        })
    }
    async fn generate_graph(
        &self,
        _: planning_dto::GenerateGraphInput,
    ) -> Result<planning_dto::GenerateGraphOutput, crate::planning::domain::PlanningError> {
        Ok(planning_dto::GenerateGraphOutput {
            graph: crate::dag_engine::domain::TaskGraph::new(),
            node_count: 0,
            sealed: true,
            from_generator: false,
        })
    }
    async fn validate_plan(
        &self,
        _: planning_dto::ValidatePlanInput,
    ) -> Result<planning_dto::ValidatePlanOutput, crate::planning::domain::PlanningError> {
        Ok(planning_dto::ValidatePlanOutput {
            passed: true,
            errors: vec![],
            warnings: vec![],
            checks_performed: 0,
        })
    }
    async fn request_clarification(
        &self,
        _: planning_dto::RequestClarificationInput,
    ) -> Result<planning_dto::RequestClarificationOutput, crate::planning::domain::PlanningError>
    {
        Ok(planning_dto::RequestClarificationOutput {
            question: "?".into(),
            ambiguous_templates: vec![],
            suggested_answers: vec![],
        })
    }
    async fn available_templates(
        &self,
    ) -> Result<planning_dto::AvailableTemplatesOutput, crate::planning::domain::PlanningError>
    {
        Ok(planning_dto::AvailableTemplatesOutput {
            templates: vec![],
            total_count: 0,
        })
    }
    fn execution_id(&self) -> Uuid {
        self.eid
    }
}

// ---------------------------------------------------------------------------
// MockExecutionService
// ---------------------------------------------------------------------------

pub(crate) struct MockExecutionService;
#[async_trait]
impl exec_svc::ParallelExecutionService for MockExecutionService {
    async fn execute_graph(
        &self,
        _: exec_dto::ExecuteGraphInput,
    ) -> Result<exec_dto::ExecuteGraphOutput, crate::execution_engine::domain::ExecutionError> {
        Ok(exec_dto::ExecuteGraphOutput {
            result: crate::execution_engine::domain::ExecutionResult::new(Uuid::new_v4()),
            completed_at: chrono::Utc::now(),
        })
    }
    async fn execute_node(
        &self,
        _: exec_dto::ExecuteNodeInput,
    ) -> Result<exec_dto::ExecuteNodeOutput, crate::execution_engine::domain::ExecutionError> {
        unimplemented!("MockExecutionService::execute_node not needed by current tests")
    }
    async fn get_execution_state(
        &self,
        _: exec_dto::GetExecutionStateInput,
    ) -> Result<exec_dto::GetExecutionStateOutput, crate::execution_engine::domain::ExecutionError>
    {
        unimplemented!("MockExecutionService::get_execution_state not needed by current tests")
    }
    async fn pause_execution(
        &self,
        _: exec_dto::PauseExecutionInput,
    ) -> Result<exec_dto::PauseExecutionOutput, crate::execution_engine::domain::ExecutionError>
    {
        unimplemented!("MockExecutionService::pause_execution not needed by current tests")
    }
    async fn resume_execution(
        &self,
        _: exec_dto::ResumeExecutionInput,
    ) -> Result<exec_dto::ResumeExecutionOutput, crate::execution_engine::domain::ExecutionError>
    {
        unimplemented!("MockExecutionService::resume_execution not needed by current tests")
    }
    async fn abort_execution(
        &self,
        _: exec_dto::AbortExecutionInput,
    ) -> Result<exec_dto::AbortExecutionOutput, crate::execution_engine::domain::ExecutionError>
    {
        Ok(exec_dto::AbortExecutionOutput {
            dag_id: Uuid::new_v4(),
            completed_count: 0,
            skipped_count: 0,
            aborted_at: chrono::Utc::now(),
        })
    }
    fn on_progress(
        &self,
        _: Box<
            dyn Fn(crate::execution_engine::application::service::ExecutionProgress) + Send + Sync,
        >,
    ) {
    }
}

// ---------------------------------------------------------------------------
// MockStateService
// ---------------------------------------------------------------------------

pub(crate) struct MockStateService {
    _saved: AtomicBool,
}
impl MockStateService {
    pub fn new() -> Self {
        Self {
            _saved: AtomicBool::new(false),
        }
    }
}
#[async_trait]
impl state_svc::StateManagerService for MockStateService {
    async fn save_state(
        &self,
        _: state_dto::SaveStateInput,
    ) -> Result<state_dto::SaveStateOutput, crate::state_persistence::domain::StateError> {
        Ok(state_dto::SaveStateOutput {
            execution_id: Uuid::new_v4(),
            status: crate::state_persistence::domain::ExecutionStatus::Pending,
            node_count: 0,
            saved_at: chrono::Utc::now(),
        })
    }
    async fn load_state(
        &self,
        _: state_dto::LoadStateInput,
    ) -> Result<state_dto::LoadStateOutput, crate::state_persistence::domain::StateError> {
        unimplemented!("MockStateService::load_state not needed by current tests")
    }
    async fn update_node_state(
        &self,
        _: state_dto::NodeStateChangedInput,
    ) -> Result<state_dto::NodeStateChangedOutput, crate::state_persistence::domain::StateError>
    {
        unimplemented!("MockStateService::update_node_state not needed by current tests")
    }
    async fn list_executions(
        &self,
        _: state_dto::ListExecutionsInput,
    ) -> Result<state_dto::ListExecutionsOutput, crate::state_persistence::domain::StateError> {
        unimplemented!("MockStateService::list_executions not needed by current tests")
    }
    async fn delete_state(
        &self,
        _: Uuid,
    ) -> Result<(), crate::state_persistence::domain::StateError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MockCancellationService
// ---------------------------------------------------------------------------

pub(crate) struct MockCancellationService;
#[async_trait]
impl cancel_app::CancellationService for MockCancellationService {
    async fn request_graceful_shutdown(
        &self,
        _: cancel_app::CancelExecutionInput,
    ) -> Result<cancel_app::CancelExecutionOutput, crate::cancellation::domain::CancellationError>
    {
        Ok(cancel_app::CancelExecutionOutput {
            accepted: true,
            signal: crate::cancellation::domain::ShutdownSignal::Graceful,
            affected_tasks: 0,
            was_already_cancelling: false,
        })
    }
    async fn request_immediate_abort(
        &self,
        _: cancel_app::CancelExecutionInput,
    ) -> Result<cancel_app::CancelExecutionOutput, crate::cancellation::domain::CancellationError>
    {
        unimplemented!(
            "MockCancellationService::request_immediate_abort not needed by current tests"
        )
    }
    async fn await_shutdown(
        &self,
        _: cancel_app::ShutdownInput,
    ) -> Result<cancel_app::ShutdownOutput, crate::cancellation::domain::CancellationError> {
        unimplemented!("MockCancellationService::await_shutdown not needed by current tests")
    }
    fn is_cancelled(&self) -> bool {
        false
    }
    fn current_signal(&self) -> Option<crate::cancellation::domain::ShutdownSignal> {
        None
    }
    async fn status(&self) -> cancel_app::ShutdownStatusOutput {
        cancel_app::ShutdownStatusOutput {
            is_cancelled: false,
            current_signal: Some(crate::cancellation::domain::ShutdownSignal::Graceful),
            running_tasks: 0,
            completed_tasks: 0,
            cancelled_tasks: 0,
            shutdown_complete: false,
            elapsed_since_request_ms: Some(0),
        }
    }
    fn subscribe(
        &self,
    ) -> tokio::sync::watch::Receiver<crate::cancellation::domain::ShutdownSignal> {
        let (tx, rx) =
            tokio::sync::watch::channel(crate::cancellation::domain::ShutdownSignal::Graceful);
        let _ = tx;
        rx
    }
    fn cancellation_token(&self) -> tokio_util::sync::CancellationToken {
        tokio_util::sync::CancellationToken::new()
    }
}

// ---------------------------------------------------------------------------
// MockEventBusService
// ---------------------------------------------------------------------------

pub(crate) struct MockEventBusService {
    count: AtomicU32,
    sender: tokio::sync::broadcast::Sender<crate::event_system::domain::ExecutionEvent>,
}
impl MockEventBusService {
    pub fn new() -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(16);
        Self {
            count: AtomicU32::new(0),
            sender,
        }
    }
}
#[async_trait]
impl event_app::EventBusService for MockEventBusService {
    async fn publish(
        &self,
        _: event_app::PublishEventInput,
    ) -> Result<event_app::PublishEventOutput, crate::event_system::domain::EventSystemError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(event_app::PublishEventOutput {
            sequence: self.count.load(Ordering::SeqCst) as u64,
            subscriber_count: 0,
            had_laggers: false,
        })
    }
    async fn subscribe(
        &self,
        _: event_app::SubscribeInput,
    ) -> Result<event_app::SubscribeOutput, crate::event_system::domain::EventSystemError> {
        Ok(event_app::SubscribeOutput {
            success: true,
            subscriber_name: Uuid::new_v4().to_string(),
            active_subscriber_count: 0,
        })
    }
    async fn drain_persisted(
        &self,
        _: event_app::DrainPersistedInput,
    ) -> Result<event_app::DrainPersistedOutput, crate::event_system::domain::EventSystemError>
    {
        Ok(event_app::DrainPersistedOutput {
            events: vec![],
            count: 0,
            cleared: true,
        })
    }
    async fn query_events(
        &self,
        _: event_app::QueryEventsInput,
    ) -> Result<event_app::QueryEventsOutput, crate::event_system::domain::EventSystemError> {
        Ok(event_app::QueryEventsOutput {
            events: vec![],
            total: 0,
            has_more: false,
        })
    }
    async fn status(
        &self,
        _: event_app::EventBusStatusInput,
    ) -> Result<event_app::EventBusStatus, crate::event_system::domain::EventSystemError> {
        Ok(event_app::EventBusStatus {
            current_sequence: 0,
            active_subscriber_count: 0,
            channel_capacity: 10000,
            buffer_capacity: 10000,
            persisted_count: 0,
        })
    }
    async fn event_count(
        &self,
    ) -> Result<event_app::EventCountOutput, crate::event_system::domain::EventSystemError> {
        Ok(event_app::EventCountOutput {
            total: 0,
            persisted: 0,
            drained: 0,
        })
    }
    fn subscribe_receiver(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::event_system::domain::ExecutionEvent> {
        self.sender.subscribe()
    }
}

// ---------------------------------------------------------------------------
// MockAuditService
// ---------------------------------------------------------------------------

pub(crate) struct MockAuditService;
impl MockAuditService {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl crate::audit::application::AuditService for MockAuditService {
    async fn build_and_send(
        &self,
        _: crate::audit::application::BuildEnvelopeInput,
    ) -> Result<crate::audit::application::BuildEnvelopeOutput, crate::audit::domain::AuditError>
    {
        Ok(crate::audit::application::BuildEnvelopeOutput {
            envelope: crate::audit::domain::AuditEnvelope {
                execution_id: Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                template_id: "mock".into(),
                planning_hash: "hash".into(),
                events: vec![],
                signature: None,
            },
            signed: false,
            event_count: 0,
        })
    }
    async fn retry_pending(
        &self,
    ) -> Result<crate::audit::application::RetryPendingOutput, crate::audit::domain::AuditError>
    {
        Ok(crate::audit::application::RetryPendingOutput {
            delivered: 0,
            still_pending: 0,
            dropped: 0,
        })
    }
    async fn status(
        &self,
    ) -> Result<crate::audit::application::AuditStatusOutput, crate::audit::domain::AuditError>
    {
        Ok(crate::audit::application::AuditStatusOutput {
            pending_count: 0,
            circuit_breaker_state: crate::audit::domain::CircuitBreakerState::Closed,
            backend_available: false,
        })
    }
}

// ---------------------------------------------------------------------------
// MockBudgetService
// ---------------------------------------------------------------------------

pub(crate) struct MockBudgetService;
#[async_trait]
impl budget_app::LlmBudgetService for MockBudgetService {
    async fn reserve(
        &self,
        _: budget_app::ReserveBudgetInput,
    ) -> Result<budget_app::ReserveBudgetOutput, crate::budget_tracking::domain::LlmBudgetError>
    {
        Ok(budget_app::ReserveBudgetOutput {
            reservation: crate::budget_tracking::domain::LlmBudgetReservationState::new(0, 100),
            remaining_calls: 100,
            remaining_tokens: 10000,
            calls_used: 0,
            tokens_used_before_reservation: 0,
        })
    }
    async fn commit(
        &self,
        _: budget_app::CommitReservationInput,
    ) -> Result<budget_app::CommitReservationOutput, crate::budget_tracking::domain::LlmBudgetError>
    {
        Ok(budget_app::CommitReservationOutput {
            remaining_calls: 100,
            remaining_tokens: 10000,
            total_calls_used: 0,
            total_tokens_used: 0,
            reservation: crate::budget_tracking::domain::LlmBudgetReservationState::new(0, 100),
            warnings_triggered: vec![],
        })
    }
    async fn get_status(
        &self,
        _: budget_app::GetBudgetStatusInput,
    ) -> Result<budget_app::GetBudgetStatusOutput, crate::budget_tracking::domain::LlmBudgetError>
    {
        Ok(budget_app::GetBudgetStatusOutput {
            max_calls: 1000,
            max_tokens: 100000,
            calls_used: 0,
            tokens_used: 0,
            remaining_calls: 1000,
            remaining_tokens: 100000,
            call_usage_ratio: 0.0,
            token_usage_ratio: 0.0,
            active_warnings: vec![],
            label: "mock".into(),
        })
    }
    fn has_capacity(&self) -> bool {
        true
    }
    fn active_warnings(&self) -> Vec<budget_app::BudgetWarningInfo> {
        vec![]
    }
}

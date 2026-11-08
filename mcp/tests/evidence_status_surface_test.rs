//! #1491 (EVID-2): the engine's evidence verdict reaches both surfaces.
//!
//! The engine has computed `EvidenceStatus` since #1480, and both hosts dropped it:
//! a run whose envelope was rejected or lost serialised exactly like a complete one.
//! These tests assert the **response** — the serialised field and the rendered
//! sentence — not the struct, because the struct was already correct.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use rigorix_mcp::execution_tools::application::dto::ExecuteInput;
use rigorix_mcp::execution_tools::application::service::ExecuteHandler;
use rigorix_mcp::execution_tools::application::service_impl::ExecuteHandlerImpl;
use rigorix_mcp::execution_tools::domain::entity::EngineFacade;
use rigorix_mcp::execution_tools::domain::error::{EngineFacadeError, ToolCallResult};
use rigorix_mcp::execution_tools::domain::value::{
    CostBreakdown, EnforcementStatus, EvidenceStatus, ExecutionId, ExecutionResult,
    ExecutionStatus, PlanTemplate, StepDefinition, StepResult, ValidationResult,
};

/// A facade that returns one run with the evidence verdict under test.
struct FacadeWithEvidence(EvidenceStatus);

#[async_trait]
impl EngineFacade for FacadeWithEvidence {
    async fn execute(
        &self,
        _plan: PlanTemplate,
        _repository: Option<String>,
        _author: Option<String>,
        _identity: Option<rigorix_engine::identity::IdentityRef>,
        _verification_failure: Option<String>,
    ) -> Result<ExecutionResult, EngineFacadeError> {
        Ok(ExecutionResult::new(
            uuid::Uuid::nil(),
            ExecutionStatus::Completed,
            vec![StepResult::new(
                "s1".into(),
                true,
                None,
                serde_json::json!({}),
                50,
            )],
            100,
            Some(50),
            "rigorix://audit/test".into(),
        )
        .with_evidence(self.0.clone()))
    }

    async fn validate_plan(
        &self,
        _p: PlanTemplate,
        _i: Option<rigorix_engine::identity::IdentityRef>,
        _v: Option<String>,
    ) -> Result<ValidationResult, EngineFacadeError> {
        Ok(ValidationResult::new(true, vec![], vec![], None))
    }

    async fn check_enforcement(&self) -> Result<EnforcementStatus, EngineFacadeError> {
        Err(EngineFacadeError::EngineNotAvailable("mock".into()))
    }

    async fn get_execution_cost(
        &self,
        id: &ExecutionId,
    ) -> Result<CostBreakdown, EngineFacadeError> {
        Err(EngineFacadeError::ExecutionNotFound(*id.as_uuid()))
    }

    async fn run_template(
        &self,
        _template_name: &str,
        _repository: Option<String>,
        _author: Option<String>,
    ) -> Result<ExecutionResult, EngineFacadeError> {
        Err(EngineFacadeError::EngineNotAvailable("mock".into()))
    }

    async fn approve_execution(
        &self,
        _: &ExecutionId,
        _: Vec<String>,
        _: Option<rigorix_mcp::execution_tools::domain::value::ApprovalIdentity>,
    ) -> Result<rigorix_mcp::execution_tools::domain::value::ApprovalResult, EngineFacadeError>
    {
        Err(EngineFacadeError::EngineNotAvailable("mock".into()))
    }

    async fn execution_state(
        &self,
        _: &ExecutionId,
    ) -> Result<rigorix_mcp::execution_tools::domain::value::ExecutionStateInfo, EngineFacadeError>
    {
        Err(EngineFacadeError::EngineNotAvailable("mock".into()))
    }
}

fn plan() -> PlanTemplate {
    PlanTemplate::new(
        "t".into(),
        "d".into(),
        vec![StepDefinition::new(
            "s1".into(),
            "tool".into(),
            serde_json::json!({}),
            false,
            "d".into(),
            None,
        )],
        None,
        std::collections::HashMap::new(),
    )
    .expect("plan is valid")
}

/// Run the plan through `rigorix_execute`'s handler and return the parsed response.
fn execute_response(evidence: EvidenceStatus) -> serde_json::Value {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let facade: Arc<dyn EngineFacade> = Arc::new(FacadeWithEvidence(evidence));
    let handler = ExecuteHandlerImpl::new(facade, Duration::from_secs(60));

    let result: ToolCallResult = runtime
        .block_on(handler.handle(ExecuteInput {
            plan: Some(plan()),
            template_name: None,
            execution_id: None,
            repository: None,
            author: None,
            identity: None,
            identity_verification_failure: None,
        }))
        .expect("the handler must not fail for a completed run");

    assert!(!result.is_error);
    let text = result
        .content
        .first()
        .map(|item| item.text.clone())
        .expect("a response body");
    serde_json::from_str(&text).expect("the response body is JSON")
}

/// Criterion 2: a degraded run says so, with the reason and the count.
#[test]
fn a_degraded_run_carries_the_reason_and_the_undelivered_count() {
    let body = execute_response(EvidenceStatus::Degraded {
        reason: "the backend refused the envelope".to_string(),
        undelivered: 2,
    });

    assert_eq!(body["evidence"]["status"], "degraded", "{body}");
    assert_eq!(
        body["evidence"]["reason"], "the backend refused the envelope",
        "{body}"
    );
    assert_eq!(body["evidence"]["undelivered"], 2, "{body}");
}

/// Criterion 3: a complete run is quiet — and is not the same shape as a degraded one.
#[test]
fn a_complete_run_says_complete_and_carries_no_reason() {
    let body = execute_response(EvidenceStatus::Complete);

    assert_eq!(body["evidence"]["status"], "complete", "{body}");
    assert!(
        body["evidence"].get("reason").is_none(),
        "a complete run must not carry a reason: {body}"
    );
    assert!(
        body["evidence"].get("undelivered").is_none(),
        "a complete run must not carry an undelivered count: {body}"
    );
}

/// The seed for criterion 2: `not_requested` and `complete` must not serialise the
/// same, or an unwired audit path looks like a fully evidenced run (the #1480 defect
/// one level up).
#[test]
fn an_audit_disabled_run_does_not_look_complete() {
    let not_requested = execute_response(EvidenceStatus::NotRequested);
    assert_eq!(
        not_requested["evidence"]["status"], "not_requested",
        "{not_requested}"
    );
    assert_ne!(not_requested["evidence"]["status"], "complete");
}

/// The field is present on every response, so a client never has to interpret an
/// absent field as a value.
#[test]
fn every_run_response_carries_the_evidence_field() {
    for evidence in [
        EvidenceStatus::NotRequested,
        EvidenceStatus::Complete,
        EvidenceStatus::Degraded {
            reason: "lost".to_string(),
            undelivered: 1,
        },
    ] {
        let body = execute_response(evidence);
        assert!(
            body.get("evidence").is_some(),
            "the response dropped the evidence field: {body}"
        );
    }
}

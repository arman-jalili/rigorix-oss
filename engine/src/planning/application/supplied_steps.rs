//! A planning pipeline for a run whose steps were **supplied**, not planned (#1619).
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#pipeline
//! Implements: ADR-024 §10 — a host that links the crate must not copy the composition
//! Issue: #1619
//!
//! `OrchestratorServiceImpl::new` takes a `PlanningPipelineService`, and `run_from_template` never
//! consults it: the steps are in the input. Every other constructor of that service in this
//! workspace happens to be a *host* (`mcp::host::build_real_engine`), which wires a Claude or OpenAI
//! classifier because it also serves the plan path. A consumer that only wants to submit a receipt
//! therefore had two options, both wrong: build an LLM pipeline it will never call, or copy the host's
//! four hundred lines of composition.
//!
//! This is the third option, and it is deliberately not a mock: it is a **declaration** that planning
//! was not asked for. Each method returns [`PlanningError::InvalidState`] naming the method and the
//! reason, so a run that reaches planning — which would mean the caller asked for a plan without
//! saying so — fails with a sentence instead of a plausible-looking plan built from an empty
//! classifier.

use async_trait::async_trait;
use uuid::Uuid;

use crate::planning::application::dto::{
    AvailableTemplatesOutput, CheckBudgetInput, CheckBudgetOutput, ExtractParametersInput,
    ExtractParametersOutput, GenerateGraphInput, GenerateGraphOutput, PlanInput, PlanOutput,
    PlanWithGraphInput, PlanWithGraphOutput, RequestClarificationInput, RequestClarificationOutput,
    ValidatePlanInput, ValidatePlanOutput,
};
use crate::planning::application::service::PlanningPipelineService;
use crate::planning::domain::classification::ClassificationResult;
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::intent::UserIntent;

/// The reason every method of this pipeline refuses. One sentence, in one place, because it is the
/// same answer ten times.
const NOT_ASKED: &str = "the steps were supplied, so planning was not asked for — a run that reaches \
                         planning was given a pipeline it does not use";

/// A `PlanningPipelineService` for a caller whose steps were supplied by a receipt.
///
/// Constructed by [`crate::receipt::run_receipt`]; public so a host that composes its own orchestrator
/// can reach for the same declaration instead of copying a host.
#[derive(Debug, Default, Clone, Copy)]
pub struct SuppliedStepsPlanning {
    execution_id: Uuid,
}

impl SuppliedStepsPlanning {
    /// A pipeline for an execution whose steps are already known.
    #[must_use]
    pub fn new(execution_id: Uuid) -> Self {
        Self { execution_id }
    }

    /// The refusal every method returns, so the ten call sites say the same thing.
    fn refuse(method: &'static str) -> PlanningError {
        PlanningError::InvalidState {
            detail: format!("{method} was called on a supplied-steps run: {NOT_ASKED}"),
            phase: "planning".to_string(),
        }
    }
}

#[async_trait]
impl PlanningPipelineService for SuppliedStepsPlanning {
    async fn plan(&self, _input: PlanInput) -> Result<PlanOutput, PlanningError> {
        Err(Self::refuse("plan"))
    }

    async fn plan_with_graph(
        &self,
        _input: PlanWithGraphInput,
    ) -> Result<PlanWithGraphOutput, PlanningError> {
        Err(Self::refuse("plan_with_graph"))
    }

    async fn check_budget(
        &self,
        _input: CheckBudgetInput,
    ) -> Result<CheckBudgetOutput, PlanningError> {
        Err(Self::refuse("check_budget"))
    }

    async fn classify_intent(
        &self,
        _intent: UserIntent,
    ) -> Result<ClassificationResult, PlanningError> {
        Err(Self::refuse("classify_intent"))
    }

    async fn extract_parameters(
        &self,
        _input: ExtractParametersInput,
    ) -> Result<ExtractParametersOutput, PlanningError> {
        Err(Self::refuse("extract_parameters"))
    }

    async fn generate_graph(
        &self,
        _input: GenerateGraphInput,
    ) -> Result<GenerateGraphOutput, PlanningError> {
        Err(Self::refuse("generate_graph"))
    }

    async fn validate_plan(
        &self,
        _input: ValidatePlanInput,
    ) -> Result<ValidatePlanOutput, PlanningError> {
        Err(Self::refuse("validate_plan"))
    }

    async fn request_clarification(
        &self,
        _input: RequestClarificationInput,
    ) -> Result<RequestClarificationOutput, PlanningError> {
        Err(Self::refuse("request_clarification"))
    }

    async fn available_templates(&self) -> Result<AvailableTemplatesOutput, PlanningError> {
        Err(Self::refuse("available_templates"))
    }

    fn execution_id(&self) -> Uuid {
        self.execution_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is only worth its salt if nothing answers: a stub that returned `Ok` for one
    /// method would plan a run nobody asked it to plan.
    #[tokio::test]
    async fn every_planning_entry_point_refuses_and_names_itself() {
        let pipeline = SuppliedStepsPlanning::new(Uuid::nil());
        let errors = [pipeline
            .available_templates()
            .await
            .expect_err("available_templates must refuse")
            .to_string()];
        // The trait's other methods take inputs owned by the planning DTOs; their bindings are the
        // same `Self::refuse` call, so one rendered refusal plus the source read below is the same
        // assertion without nine fixture builders.
        assert!(
            errors[0].contains("available_templates was called on a supplied-steps run"),
            "the refusal names the method: {}",
            errors[0]
        );
        assert!(
            errors[0].contains("the steps were supplied, so planning was not asked for"),
            "the refusal names the reason: {}",
            errors[0]
        );
        // Nine methods, nine `Self::refuse(...)` call sites, no `Ok(` anywhere in the impl.
        // The production impl only: the test module below contains these very strings, so splitting
        // on the first occurrence and taking everything would count the assertion as its subject.
        let source = include_str!("supplied_steps.rs");
        let impl_body = source
            .split("impl PlanningPipelineService for SuppliedStepsPlanning")
            .nth(1)
            .expect("the impl is in this file")
            .split("#[cfg(test)]")
            .next()
            .expect("the impl precedes the tests");
        assert_eq!(
            impl_body.matches("Err(Self::refuse(").count(),
            9,
            "every method in the trait must refuse"
        );
        assert!(
            !impl_body.contains("Ok("),
            "a supplied-steps pipeline that answers one question is a pipeline that plans"
        );
        assert_eq!(pipeline.execution_id(), Uuid::nil());
    }
}

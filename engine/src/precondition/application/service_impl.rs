//! PreconditionServiceImpl — the concrete dispatch-time precondition service.
//!
//! @canonical .pi/architecture/modules/consequence-gating.md#preconditionservice
//! Implements: ISSUE-CONSEQUENCE-GATING-4 — match → require_params → run → verdict
//! Issue: #942; contract frozen in #938
//!
//! Evaluation contract (v1):
//!
//! ```text
//! 1. load the operator config (per dispatch — never cached across T₀→Tₙ)
//! 2. for each precondition whose StepPredicate matches the about-to-dispatch
//!    step, in config order:
//!    a. verify every require_params pointer is present and non-null
//!       (missing → Deny before the command runs)
//!    b. run the check through PreconditionRunner
//!    c. passed → continue; failed/error → Deny{precondition_id, outcome}
//! 3. no matching refusal → Dispatch
//! ```
//!
//! Degradation semantics (frozen): a missing config file (`Ok(None)`) or an
//! empty precondition set yields `Dispatch` (fail-open-absent — status quo). A
//! corrupt config (`Err`) is propagated (fail closed). A runner failure
//! (timeout / spawn / trust boundary) is an indeterminate `error` and refuses —
//! never a pass.

use async_trait::async_trait;
use serde_json::Value;

use crate::precondition::domain::{
    Precondition, PreconditionError, PreconditionOutcome, PreconditionVerdict,
};
use crate::precondition::infrastructure::PreconditionRunner;
use crate::precondition::infrastructure::repository::PreconditionRepository;

use super::dto::{DispatchStep, PreconditionCheckInput};
use super::service::PreconditionService;

/// Default `PreconditionService` implementation.
///
/// # Construction
/// - `new(repository, runner)` — inject the config repository (filesystem
///   `.rigorix/preconditions.toml`, or an in-memory test double) and the
///   deterministic argv runner. Both are always injected — never defaulted
///   inside the service.
pub struct PreconditionServiceImpl {
    repository: Box<dyn PreconditionRepository>,
    runner: Box<dyn PreconditionRunner>,
}

impl PreconditionServiceImpl {
    /// Create the service over the given repository and runner.
    pub fn new(
        repository: Box<dyn PreconditionRepository>,
        runner: Box<dyn PreconditionRunner>,
    ) -> Self {
        Self { repository, runner }
    }
}

/// Whether a JSON pointer resolves to a present, non-null value.
fn pointer_present(parameters: &Value, pointer: &str) -> bool {
    parameters
        .pointer(pointer)
        .is_some_and(|value| !value.is_null())
}

impl PreconditionServiceImpl {
    /// Evaluate the matching preconditions over the loaded config.
    async fn evaluate_with_config(
        &self,
        config: &crate::precondition::domain::PreconditionConfig,
        execution_id: uuid::Uuid,
        step: &DispatchStep,
    ) -> Result<PreconditionVerdict, PreconditionError> {
        for precondition in &config.preconditions {
            if !precondition.matches(&step.tool, &step.parameters)? {
                // AC #8: a non-matching step spawns nothing and is unaffected.
                continue;
            }
            if let Some(denial) = self
                .evaluate_match(precondition, execution_id, step)
                .await?
            {
                return Ok(denial);
            }
        }
        Ok(PreconditionVerdict::Dispatch)
    }

    /// Evaluate one matched precondition. `Ok(None)` = passed.
    async fn evaluate_match(
        &self,
        precondition: &Precondition,
        execution_id: uuid::Uuid,
        step: &DispatchStep,
    ) -> Result<Option<PreconditionVerdict>, PreconditionError> {
        // AC #7: presence-only obligation, enforced BEFORE the command runs.
        for pointer in &precondition.require_params {
            if !pointer_present(&step.parameters, pointer) {
                return Ok(Some(PreconditionVerdict::Deny {
                    precondition_id: precondition.id.clone(),
                    outcome: PreconditionOutcome::Failed,
                }));
            }
        }

        let input = PreconditionCheckInput {
            precondition_id: precondition.id.clone(),
            execution_id,
            step: step.name.clone(),
            tool: step.tool.clone(),
            parameters: step.parameters.clone(),
        };
        match self.runner.run(precondition, &input).await {
            Ok(run) if !run.outcome.refuses() => Ok(None),
            Ok(run) => Ok(Some(PreconditionVerdict::Deny {
                precondition_id: precondition.id.clone(),
                outcome: run.outcome,
            })),
            Err(error) => {
                // Timeout / spawn / trust boundary are indeterminate: refuse
                // (`error`), never pass.
                tracing::debug!(
                    precondition = %precondition.id,
                    %error,
                    "precondition check indeterminate — refusing"
                );
                Ok(Some(PreconditionVerdict::Deny {
                    precondition_id: precondition.id.clone(),
                    outcome: PreconditionOutcome::Error,
                }))
            }
        }
    }
}

#[async_trait]
impl PreconditionService for PreconditionServiceImpl {
    async fn evaluate(
        &self,
        execution_id: uuid::Uuid,
        step: &DispatchStep,
    ) -> Result<PreconditionVerdict, PreconditionError> {
        match self.repository.load_config().await? {
            Some(config) => self.evaluate_with_config(&config, execution_id, step).await,
            // Fail-open-absent: no config file → no gating (status quo).
            None => Ok(PreconditionVerdict::Dispatch),
        }
    }

    async fn is_configured(&self) -> Result<bool, PreconditionError> {
        Ok(self
            .repository
            .load_config()
            .await?
            .is_some_and(|config| !config.preconditions.is_empty()))
    }
}

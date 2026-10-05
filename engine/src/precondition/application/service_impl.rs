//! PreconditionServiceImpl — the concrete dispatch-time precondition service.
//!
//! @canonical .pi/architecture/modules/precondition.md#preconditionservice
//! Implements: ISSUE-CONSEQUENCE-GATING-4 — match → require_params → run → verdict
//! Issue: #942; evidence findings via #944; contract frozen in #938
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
//!
//! Every matching check also produces a redacted [`PreconditionFinding`]
//! (`outcome` / `exit_code` / `inputs_hash` / `checked_at`; no parameter values
//! or stdout) for the signed envelope.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::precondition::domain::{
    Precondition, PreconditionConfig, PreconditionError, PreconditionFinding, PreconditionOutcome,
    PreconditionVerdict,
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

/// One-way hash of the check inputs (never the inputs themselves).
fn inputs_hash(input: &PreconditionCheckInput) -> String {
    let bytes = serde_json::to_vec(input).unwrap_or_default();
    let digest = Sha256::digest(&bytes);
    format!("sha256:{}", hex::encode(digest))
}

impl PreconditionServiceImpl {
    /// Evaluate the matching preconditions over the loaded config, collecting
    /// one redacted finding per check that ran.
    async fn evaluate_with_config(
        &self,
        config: &PreconditionConfig,
        execution_id: uuid::Uuid,
        step: &DispatchStep,
    ) -> Result<(PreconditionVerdict, Vec<PreconditionFinding>), PreconditionError> {
        tracing::debug!(
            step = %step.name,
            tool = %step.tool,
            configured = config.preconditions.len(),
            "precondition: evaluating dispatch gate"
        );
        let mut findings = Vec::new();
        for precondition in &config.preconditions {
            if !precondition.matches(&step.tool, &step.parameters)? {
                // AC #8: a non-matching step spawns nothing and is unaffected.
                continue;
            }
            let (verdict, finding) = self
                .evaluate_match(precondition, execution_id, step)
                .await?;
            if let Some(finding) = finding {
                findings.push(finding);
            }
            if let Some(verdict) = verdict {
                return Ok((verdict, findings));
            }
        }
        Ok((PreconditionVerdict::Dispatch, findings))
    }

    /// Evaluate one matched precondition.
    ///
    /// Returns `(Some(verdict), finding)` when the check refused, and
    /// `(None, finding)` when it passed.
    async fn evaluate_match(
        &self,
        precondition: &Precondition,
        execution_id: uuid::Uuid,
        step: &DispatchStep,
    ) -> Result<(Option<PreconditionVerdict>, Option<PreconditionFinding>), PreconditionError> {
        let checked_at = Utc::now();
        let input = PreconditionCheckInput {
            precondition_id: precondition.id.clone(),
            execution_id,
            step: step.name.clone(),
            tool: step.tool.clone(),
            parameters: step.parameters.clone(),
        };
        let hash = inputs_hash(&input);

        // AC #7: presence-only obligation, enforced BEFORE the command runs.
        for pointer in &precondition.require_params {
            if !pointer_present(&step.parameters, pointer) {
                let finding = PreconditionFinding {
                    precondition_id: precondition.id.clone(),
                    step: step.name.clone(),
                    outcome: PreconditionOutcome::Failed,
                    exit_code: None,
                    inputs_hash: hash,
                    checked_at,
                    summary: Some(format!(
                        "precondition '{}' refused step '{}': required parameter '{}' absent",
                        precondition.id, step.name, pointer
                    )),
                };
                return Ok((
                    Some(PreconditionVerdict::Deny {
                        precondition_id: precondition.id.clone(),
                        outcome: PreconditionOutcome::Failed,
                    }),
                    Some(finding),
                ));
            }
        }

        match self.runner.run(precondition, &input).await {
            Ok(run) => {
                let finding = PreconditionFinding {
                    precondition_id: precondition.id.clone(),
                    step: step.name.clone(),
                    outcome: run.outcome,
                    exit_code: run.exit_code,
                    inputs_hash: hash,
                    checked_at,
                    summary: None,
                };
                if run.outcome.refuses() {
                    Ok((
                        Some(PreconditionVerdict::Deny {
                            precondition_id: precondition.id.clone(),
                            outcome: run.outcome,
                        }),
                        Some(finding),
                    ))
                } else {
                    Ok((None, Some(finding)))
                }
            }
            Err(error) => {
                // Timeout / spawn / trust boundary are indeterminate: refuse
                // (`error`), never pass.
                tracing::debug!(
                    precondition = %precondition.id,
                    %error,
                    "precondition check indeterminate — refusing"
                );
                let finding = PreconditionFinding {
                    precondition_id: precondition.id.clone(),
                    step: step.name.clone(),
                    outcome: PreconditionOutcome::Error,
                    exit_code: None,
                    inputs_hash: hash,
                    checked_at,
                    summary: Some(format!(
                        "precondition '{}' indeterminate for step '{}' ({})",
                        precondition.id, step.name, error
                    )),
                };
                Ok((
                    Some(PreconditionVerdict::Deny {
                        precondition_id: precondition.id.clone(),
                        outcome: PreconditionOutcome::Error,
                    }),
                    Some(finding),
                ))
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
        Ok(self.evaluate_with_findings(execution_id, step).await?.0)
    }

    async fn evaluate_with_findings(
        &self,
        execution_id: uuid::Uuid,
        step: &DispatchStep,
    ) -> Result<(PreconditionVerdict, Vec<PreconditionFinding>), PreconditionError> {
        match self.repository.load_config().await? {
            Some(config) => self.evaluate_with_config(&config, execution_id, step).await,
            // Fail-open-absent: no config file → no gating (status quo).
            None => Ok((PreconditionVerdict::Dispatch, Vec::new())),
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

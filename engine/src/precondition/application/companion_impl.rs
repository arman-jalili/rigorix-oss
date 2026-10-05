//! CompanionStepObligationServiceImpl — concrete R3 companion-step evaluator.
//!
//! @canonical .pi/architecture/modules/consequence-gating.md#r3
//! Implements: ISSUE-CONSEQUENCE-GATING-8 — `require_companion_step` evaluation
//! Issue: #946; contract frozen in #938
//!
//! Evaluates a set of operator-authored companion obligations over a
//! fully-materialized plan. For each obligation whose `match` predicate
//! selects a step, the plan MUST also contain a step matching
//! `require_companion_step`; when it does not, a redacted [`CompanionFinding`]
//! is returned carrying the configured action (`deny` default, or `promote`).
//!
//! Evaluation is deterministic: obligations in config order, then steps in
//! plan order. A malformed operator predicate is a fail-closed
//! [`PreconditionError::Match`].

use async_trait::async_trait;

use crate::precondition::domain::PreconditionError;

use super::companion::{CompanionFinding, CompanionStepObligation, CompanionStepObligationService};
use super::dto::DispatchStep;

/// Concrete companion-step obligation evaluator.
pub struct CompanionStepObligationServiceImpl {
    obligations: Vec<CompanionStepObligation>,
}

impl CompanionStepObligationServiceImpl {
    /// Build the evaluator over an ordered obligation set.
    pub fn new(obligations: Vec<CompanionStepObligation>) -> Self {
        Self { obligations }
    }

    /// The configured obligations.
    pub fn obligations(&self) -> &[CompanionStepObligation] {
        &self.obligations
    }
}

#[async_trait]
impl CompanionStepObligationService for CompanionStepObligationServiceImpl {
    async fn evaluate_plan(
        &self,
        steps: &[DispatchStep],
    ) -> Result<Vec<CompanionFinding>, PreconditionError> {
        let mut findings = Vec::new();
        for obligation in &self.obligations {
            for step in steps {
                let matches = obligation
                    .r#match
                    .matches(&step.tool, &step.parameters)
                    .map_err(|error| PreconditionError::Match {
                        precondition_id: obligation.id.clone(),
                        detail: error.to_string(),
                    })?;
                if !matches {
                    continue;
                }

                // A companion present anywhere in the same plan satisfies the
                // obligation (plan-time, pre-side-effect).
                let mut companion_present = false;
                for candidate in steps {
                    let companion = obligation
                        .require_companion_step
                        .matches(&candidate.tool, &candidate.parameters)
                        .map_err(|error| PreconditionError::Match {
                            precondition_id: obligation.id.clone(),
                            detail: error.to_string(),
                        })?;
                    if companion {
                        companion_present = true;
                        break;
                    }
                }
                if !companion_present {
                    findings.push(CompanionFinding {
                        obligation_id: obligation.id.clone(),
                        step: step.name.clone(),
                        required_companion: obligation.require_companion_step.tool.clone(),
                        action: obligation.action,
                    });
                }
            }
        }
        Ok(findings)
    }
}

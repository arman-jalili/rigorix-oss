//! `evaluate_gate` — the composition, and the only place the order lives.
//!
//! @canonical .pi/architecture/modules/gate.md
//! Issue: #1381 (Phase C.1)
//!
//! # The order, and why it is this order
//!
//! ```text
//! 1. preconditions   (ADR-017)  — may the step dispatch at all?
//! 2. requirements    (ADR-013 R9 / ADR-015) — which operator obligations does it leave unmet?
//! ```
//!
//! Preconditions run first because they answer a different question. A precondition
//! is about the step's *consequence* — a command that would run a destructive
//! operation without its declared guard — and its refusal is a "this must not happen",
//! not a "this is unmet". Requirements evaluate *whether the plan may proceed with
//! this step in it*: they can promote a step to human approval, and promoting a step
//! that will never dispatch is evidence about a run that cannot happen.
//!
//! So the composition is precondition → requirements, each stopping the walk on its
//! own refusal (fail closed), and the findings it returns are the ones produced before
//! the stop, in that order. Swapping the two is a behaviour change, and the test that
//! says so is `a_refusal_precondition_is_not_displaced_by_a_requirement_promotion`
//! plus the order-recording port in `the_order_is_preconditions_then_requirements`.

use sha2::{Digest, Sha256};

use crate::precondition::application::dto::DispatchStep;
use crate::sequence_policy::application::dto::PlannedStep;
use crate::sequence_policy::domain::RequirementAction;

use super::dto::{
    GateDecision, GateEnvelopeBinding, GateError, GateFinding, GateId, GateOutcome, GateRequest,
    GateVerdict,
};

/// Evaluate one run through every configured gate, in the fixed order.
///
/// Returns the verdict, the ordered findings, and the decision's digest. An error is
/// a refusal at the caller: the gate could not answer, which is never a pass.
///
/// # Errors
/// - [`GateError::UnpinnedExecution`] when the request carries no execution id
/// - [`GateError::Precondition`] when the precondition gate failed to evaluate
/// - [`GateError::Requirements`] when the requirements failed to evaluate
pub async fn evaluate_gate(request: GateRequest<'_>) -> Result<GateDecision, GateError> {
    let execution_id = request
        .pins
        .execution_id()
        .ok_or(GateError::UnpinnedExecution)?;

    let mut findings: Vec<GateFinding> = Vec::new();
    let mut verdict = GateVerdict::Dispatch;

    // ── 1. preconditions ─────────────────────────────────────────────────
    if let Some(service) = &request.ports.precondition {
        let step = DispatchStep {
            name: request.step.name.clone(),
            tool: request.step.tool.clone(),
            parameters: request.step.parameters.clone(),
        };
        let (precondition_verdict, precondition_findings) = service
            .evaluate_with_findings(execution_id, &step)
            .await
            .map_err(|error| GateError::Precondition {
                detail: error.to_string(),
            })?;

        let mut refusal: Option<(Option<String>, String)> = None;
        for finding in precondition_findings {
            let refused = finding.outcome.refuses();
            findings.push(GateFinding {
                gate_id: GateId::Preconditions,
                step: finding.step.clone(),
                outcome: if refused {
                    GateOutcome::Refused
                } else {
                    GateOutcome::Passed
                },
                rule_id: Some(finding.precondition_id.clone()),
                detail: finding.summary.clone(),
                evidence: serde_json::to_value(&finding).ok(),
            });
            if refused && refusal.is_none() {
                refusal = Some((
                    Some(finding.precondition_id),
                    finding
                        .summary
                        .clone()
                        .unwrap_or_else(|| "precondition refused".to_string()),
                ));
            }
        }

        if !precondition_verdict.allows_dispatch() {
            let rule_id = refusal.as_ref().and_then(|(id, _)| id.clone()).or_else(|| {
                precondition_verdict
                    .denying_precondition_id()
                    .map(str::to_string)
            });
            let reason = refusal
                .map(|(_, reason)| reason)
                .unwrap_or_else(|| verdict_reason(&precondition_verdict));
            // A refusal with no finding still names the gate: "refused" must never be
            // an empty verdict a caller has to interpret.
            if findings.is_empty() {
                findings.push(GateFinding {
                    gate_id: GateId::Preconditions,
                    step: request.step.name.clone(),
                    outcome: GateOutcome::Refused,
                    rule_id: rule_id.clone(),
                    detail: Some(reason.clone()),
                    evidence: None,
                });
            }
            verdict = GateVerdict::Refuse {
                gate: GateId::Preconditions,
                rule_id,
                reason,
            };
        }
    }

    // ── 2. requirements ──────────────────────────────────────────────────
    if verdict.permits_dispatch()
        && let Some(service) = &request.ports.sequence
    {
        let planned = vec![PlannedStep {
            name: request.step.name.clone(),
            tool: request.step.tool.clone(),
            parameters: request.step.parameters.clone(),
        }];
        let requirement_findings = service
            .evaluate_requirements(&planned, request.identity_attested)
            .await
            .map_err(|error| GateError::Requirements {
                detail: error.to_string(),
            })?;

        for finding in requirement_findings {
            let refused = matches!(finding.action, RequirementAction::Deny);
            findings.push(GateFinding {
                gate_id: GateId::Requirements,
                step: finding.step.clone(),
                outcome: if refused {
                    GateOutcome::Refused
                } else {
                    GateOutcome::Passed
                },
                rule_id: Some(finding.requirement_id.clone()),
                detail: Some(finding.decision_summary()),
                evidence: serde_json::to_value(&finding).ok(),
            });
            if refused {
                let summary = finding.decision_summary();
                if verdict.permits_dispatch() {
                    verdict = GateVerdict::Refuse {
                        gate: GateId::Requirements,
                        rule_id: Some(finding.requirement_id),
                        reason: summary,
                    };
                }
            }
        }
    }

    let envelope = GateEnvelopeBinding {
        execution_id,
        decision_digest: decision_digest(&verdict, &findings),
        pinned_timestamp: request.pins.timestamp(),
        pinned: !request.pins.is_empty(),
    };

    Ok(GateDecision {
        verdict,
        findings,
        envelope,
    })
}

/// A redacted refusal reason for a verdict that produced no finding.
fn verdict_reason(verdict: &crate::precondition::domain::PreconditionVerdict) -> String {
    match verdict {
        crate::precondition::domain::PreconditionVerdict::Deny { outcome, .. } => {
            format!("precondition refused: {outcome:?}")
        }
        crate::precondition::domain::PreconditionVerdict::Dispatch => {
            "precondition gate did not permit dispatch".to_string()
        }
    }
}

/// SHA-256 (hex) of the decision's canonical bytes.
///
/// The bytes are `[verdict, findings]` in serde declaration order. Nothing
/// ambient — no `Utc::now()`, no hash-map iteration — enters them, so two calls with
/// the same request produce the same digest. That is what `determinism_test` asserts,
/// and what makes the digest worth signing.
#[must_use]
pub fn decision_digest(verdict: &GateVerdict, findings: &[GateFinding]) -> String {
    let canonical = serde_json::to_vec(&(verdict, findings)).unwrap_or_else(|_| Vec::new());
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

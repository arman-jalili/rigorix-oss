//! Budget and limit evaluation, shared by both dispatch paths (#1108).
//!
//! @canonical .pi/architecture/modules/enforcement.md#executionenforcer
//! Implements: ISSUE-UNWIRED-BUDGETS-AND-RECOVERY — the enforcer was constructed
//!   by no entry point, and the two dispatch paths would otherwise disagree
//! Issue: #1108
//!
//! # Why this is a free function and not a method
//!
//! The engine has **two** tool-dispatch paths: the parallel loop
//! (`service_impl::spawn_concurrent_node`) and the single-node route
//! (`service_impl::tools::execute_tool`). They already duplicated the permission
//! and hook checks, with a comment apologising for the asymmetry. A budget check
//! added to one of them would be a control that is off on the other route — the
//! exact class (#1074) this series spent a round removing. So the evaluation lives
//! here, takes only what it needs, and both callers use it.
//!
//! # Fail-closed
//!
//! An enforcer error refuses the call ([`BudgetRefusal::Unavailable`]) rather than
//! letting it through: a budget that could not be evaluated is not a budget that
//! was satisfied.

use std::sync::Arc;

use uuid::Uuid;

use crate::enforcement::application::dto::{EvaluateToolCallInput, TrackResourceUsageInput};
use crate::enforcement::application::enforcer_impl::ExecutionEnforcerImpl;
use crate::enforcement::application::service::ExecutionEnforcer;

/// Why a call was refused before dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BudgetRefusal {
    /// The call exceeds a configured limit.
    Exceeded(String),
    /// The enforcer could not be evaluated (fail closed).
    Unavailable(String),
}

impl BudgetRefusal {
    /// The failure type recorded on the node result.
    pub(super) fn failure_type(&self) -> &'static str {
        match self {
            Self::Exceeded(_) => "budget_exceeded",
            Self::Unavailable(_) => "enforcement_error",
        }
    }

    /// The reason recorded on the node result.
    pub(super) fn reason(&self) -> &str {
        match self {
            Self::Exceeded(reason) | Self::Unavailable(reason) => reason,
        }
    }
}

/// Evaluate the per-execution budget for one tool call.
///
/// Returns `Ok(())` when the call may run — having recorded it, so the next
/// evaluation sees this one — or [`BudgetRefusal`] when it must not.
pub(super) async fn evaluate(
    enforcer: &Arc<ExecutionEnforcerImpl>,
    dag_id: Uuid,
    node_id: Uuid,
    tool: &str,
) -> Result<(), BudgetRefusal> {
    let evaluation = enforcer
        .evaluate_tool_call(EvaluateToolCallInput {
            execution_id: dag_id.to_string(),
            node_id: node_id.to_string(),
            tool: tool.to_string(),
            arguments: None,
            is_retry: false,
            attempt: 1,
        })
        .await;

    match evaluation {
        Ok(outcome) if outcome.allowed => {
            // The call is allowed; count it so the next evaluation sees this one.
            // A counting failure is logged rather than turned into a refusal: the
            // counter is cumulative and the next evaluation re-reads it, so the
            // budget may under-count but never over-permits silently.
            if let Err(error) = enforcer
                .track_resource_usage(TrackResourceUsageInput {
                    execution_id: dag_id.to_string(),
                    resource: "tool_calls".to_string(),
                    amount: 1,
                    tool: Some(tool.to_string()),
                    context: None,
                })
                .await
            {
                tracing::warn!(
                    %dag_id,
                    %node_id,
                    tool = %tool,
                    %error,
                    "enforcement: tool call could not be counted; the budget may under-count"
                );
            }
            Ok(())
        }
        Ok(outcome) => {
            let reason = outcome
                .reason
                .unwrap_or_else(|| format!("tool '{tool}' refused by enforcement policy"));
            tracing::warn!(
                %dag_id,
                %node_id,
                tool = %tool,
                %reason,
                "enforcement: tool call refused before dispatch"
            );
            Err(BudgetRefusal::Exceeded(reason))
        }
        Err(error) => {
            tracing::error!(
                %dag_id,
                %node_id,
                tool = %tool,
                %error,
                "enforcement: evaluation failed — refusing the call (fail closed)"
            );
            Err(BudgetRefusal::Unavailable(format!(
                "enforcement evaluation failed: {error}"
            )))
        }
    }
}

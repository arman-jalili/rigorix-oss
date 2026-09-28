//! Approval flow for `OrchestratorServiceImpl` (extracted from
//! `orchestrator_impl.rs`, #916): engine-session approval with cross-process
//! hydration. The coordinator's `approve_execution` delegates here.

use super::*;

impl OrchestratorServiceImpl {
    pub(super) async fn approve_execution_flow(
        &self,
        input: ApproveExecutionInput,
    ) -> Result<ApproveExecutionOutput, OrchestratorError> {
        // 0. Approve in the engine session. If the session is not in this
        // process's memory (GAP-3 cross-process resume), hydrate it from the
        // persisted ExecutionState first, then approve. Single approve call:
        // a second call would see the node already marked Ready and be
        // correctly denied by the GAP-H-07 gate.
        let approve_out = match self
            .execution_service
            .approve_node(exec_dto::ApproveNodeInput {
                dag_id: input.execution_id,
                step_names: input.step_names.clone(),
                approver_id: input.approver_id.clone(),
                authority: input.authority.clone(),
                decision_context: None,
                token_claims_ref: input.token_claims_ref.clone(),
            })
            .await
        {
            Ok(out) => out,
            Err(crate::execution_engine::domain::ExecutionError::NodeNotFound { node_id }) => {
                tracing::info!(
                    %node_id,
                    "approve_execution: session not in this process — hydrating from persisted state"
                );
                let loaded = self
                    .state_manager
                    .load_state(state_dto::LoadStateInput {
                        execution_id: input.execution_id,
                    })
                    .await
                    .map_err(|e| OrchestratorError::Internal {
                        detail: format!("Failed to load paused execution state: {e}"),
                        source_module: "orchestrator".into(),
                    })?;
                let Some(graph) = loaded.state.graph.clone() else {
                    return Err(OrchestratorError::Internal {
                        detail: format!(
                            "Execution {} has no resumable graph in state",
                            input.execution_id
                        ),
                        source_module: "orchestrator".into(),
                    });
                };
                let node_states = loaded.state.exec_node_states.clone().unwrap_or_default();
                let approved: std::collections::HashSet<uuid::Uuid> =
                    loaded.state.approved.iter().copied().collect();
                self.execution_service
                    .hydrate_execution(exec_dto::HydrateExecutionInput {
                        dag_id: input.execution_id,
                        graph,
                        node_states,
                        approved,
                        started_at: loaded.state.started_at,
                    })
                    .await
                    .map_err(|e| OrchestratorError::Internal {
                        detail: format!("Failed to hydrate execution session: {e}"),
                        source_module: "orchestrator".into(),
                    })?;
                self.execution_service
                    .approve_node(exec_dto::ApproveNodeInput {
                        dag_id: input.execution_id,
                        step_names: input.step_names,
                        approver_id: None,
                        authority: None,
                        decision_context: None,
                        token_claims_ref: None,
                    })
                    .await
                    .map_err(|e| OrchestratorError::Internal {
                        detail: format!("Failed to approve execution steps: {e}"),
                        source_module: "orchestrator".into(),
                    })?
            }
            Err(e) => {
                return Err(OrchestratorError::Internal {
                    detail: format!("Failed to approve execution steps: {e}"),
                    source_module: "orchestrator".into(),
                });
            }
        };

        // 1. Resume the paused execution if no steps remain pending.
        let mut resumed = false;
        if approve_out.still_pending.is_empty() {
            resumed = self
                .execution_service
                .resume_execution(exec_dto::ResumeExecutionInput {
                    dag_id: input.execution_id,
                })
                .await
                .is_ok();
        }

        // 3. Sync the orchestrator's current execution status.
        if let Some(s) = self.current_execution.write().await.as_mut() {
            s.status = if resumed {
                match self
                    .execution_service
                    .get_execution_state(exec_dto::GetExecutionStateInput {
                        dag_id: input.execution_id,
                    })
                    .await
                {
                    Ok(state) => {
                        let failed = state
                            .node_states
                            .values()
                            .filter(|st| {
                                st.status == crate::execution_engine::domain::NodeStatus::Failed
                            })
                            .count();
                        if failed > 0 {
                            ExecutionStatus::PartialFailure
                        } else {
                            ExecutionStatus::Completed
                        }
                    }
                    Err(_) => ExecutionStatus::Completed,
                }
            } else {
                ExecutionStatus::PendingApproval
            };
        }

        Ok(ApproveExecutionOutput {
            execution_id: input.execution_id,
            approved: approve_out.approved,
            not_found: approve_out.not_found,
            still_pending: approve_out.still_pending,
            resumed,
        })
    }
}

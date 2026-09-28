//! `execute_node` flow for `ParallelExecutionServiceImpl` (extracted from
//! `service_impl.rs`, #914): per-node tool execution + inline retry loop.

use super::*;

impl ParallelExecutionServiceImpl {
    pub(super) async fn execute_node_flow(
        &self,
        input: ExecuteNodeInput,
    ) -> Result<ExecuteNodeOutput, ExecutionError> {
        // Execute a single node with an inline retry loop.
        //
        // The retry loop follows this lifecycle:
        // 1. Attempt to execute the node's action
        // 2. If successful → return TaskResult with success
        // 3. If failed → build FailureContext, evaluate retry
        // 4. If Retry → apply backoff, loop
        // 5. If Fallback/Skip/Abort → terminal, return result
        //
        // This is the **inline retry loop** — not a separate retry wrapper.
        // Each retry can escalate the strategy per the RetryPolicy.

        let policy = input
            .retry_policy
            .clone()
            .unwrap_or_else(|| self.config.default_retry_policy.clone());
        let max_attempts = policy.max_attempts;
        let node_id = input.node_id;

        let mut last_retry_decision: Option<RetryDecision> = None;

        // Inline retry loop per node
        for attempt in 0..max_attempts {
            let start = std::time::Instant::now();

            // --- Phase 1: Check skip conditions before execution ---
            if policy.has_skip_conditions()
                && let Some(conditions) = &policy.skip_conditions
            {
                for condition in conditions {
                    if condition == "always_skip" {
                        let result = TaskResult::failure(
                            node_id,
                            format!("node-{}", node_id),
                            format!("Skipped by condition: {}", condition),
                            "skipped".to_string(),
                            start.elapsed().as_millis() as u64,
                            attempt,
                        );
                        return Ok(ExecuteNodeOutput {
                            result,
                            retry_decision: Some(RetryDecision::Skip {
                                reason: format!("Skip condition '{}' matched", condition),
                            }),
                        });
                    }
                }
            }

            // --- Phase 2: Check cancellation (placeholder) ---
            // In production, checks CancellationToken here

            // --- Phase 3: Execute the node ---
            // Look up node from the session graph to dispatch the tool.

            // Emit NodeStarted
            if let Err(e) = self
                .event_bus
                .publish(crate::event_system::application::dto::PublishEventInput {
                    event: ExecutionEvent::NodeStarted {
                        execution_id: input.dag_id,
                        node_id: node_id.to_string(),
                        node_name: format!("node-{}", node_id),
                        timestamp: chrono::Utc::now(),
                    },
                })
                .await
            {
                tracing::warn!(error = %e, "event publish failed — evidence may be incomplete");
            }

            // Extract node info from sessions WITHOUT holding the lock across .await
            let node_info = {
                let sessions = self
                    .sessions
                    .lock()
                    .map_err(|e| ExecutionError::InternalError {
                        detail: format!("Lock error: {}", e),
                    })?;
                sessions
                    .get(&input.dag_id)
                    .and_then(|s| s.graph.as_ref())
                    .and_then(|g| g.get_node(node_id).cloned())
            };

            let (execution_successful, output_text, failure_type, error_message, exec_duration_ms) =
                if let Some(node) = node_info {
                    let task_result = self.execute_tool(&node, node_id, start).await;
                    let dur = task_result.duration_ms;
                    if task_result.success {
                        (
                            true,
                            task_result.output.unwrap_or_default(),
                            String::new(),
                            String::new(),
                            dur,
                        )
                    } else {
                        (
                            false,
                            String::new(),
                            task_result
                                .failure_type
                                .unwrap_or_else(|| "unknown".to_string()),
                            task_result.error.unwrap_or_default(),
                            dur,
                        )
                    }
                } else {
                    // No graph or node not found: fail the node instead of placeholder success
                    (
                        false,
                        String::new(),
                        "node_not_found".to_string(),
                        format!(
                            "Node {} not found in session graph for dag {}",
                            node_id, input.dag_id
                        ),
                        0,
                    )
                };

            // Emit NodeCompleted
            if let Err(e) = self
                .event_bus
                .publish(crate::event_system::application::dto::PublishEventInput {
                    event: ExecutionEvent::NodeCompleted {
                        execution_id: input.dag_id,
                        node_id: node_id.to_string(),
                        node_name: format!("node-{}", node_id),
                        duration_ms: exec_duration_ms,
                        output: serde_json::json!(output_text.clone()),
                        timestamp: chrono::Utc::now(),
                    },
                })
                .await
            {
                tracing::warn!(error = %e, "event publish failed — evidence may be incomplete");
            }

            let duration_ms = start.elapsed().as_millis() as u64;

            if execution_successful {
                let result = TaskResult::success(
                    node_id,
                    format!("node-{}", node_id),
                    Some(output_text),
                    duration_ms,
                    attempt,
                );
                return Ok(ExecuteNodeOutput {
                    result,
                    retry_decision: last_retry_decision,
                });
            }

            // Fall through to Phase 4 with actual error info
            let _ = output_text;

            // --- Phase 3b: Attempt recovery before retry evaluation ---
            if let Some(ref recovery_svc) = self.recovery_service {
                let scenario_opt = if failure_type.contains("command_failed")
                    || failure_type.contains("edit_file_read_error")
                    || failure_type.contains("file_write_error")
                {
                    Some(FailureScenario::CompileError)
                } else if failure_type.contains("file_read_error") {
                    Some(FailureScenario::TestFailure)
                } else if failure_type.contains("exec_error") {
                    Some(FailureScenario::ToolConnectionError)
                } else if failure_type.contains("parse_error") {
                    Some(FailureScenario::ProviderFailure)
                } else {
                    None
                };

                if let Some(scenario) = scenario_opt {
                    let recipe_input = RecipeForInput {
                        scenario,
                        custom_recipes: None,
                    };
                    if let Ok(recipe_out) = recovery_svc.recipe_for(recipe_input).await {
                        let (can_attempt, attempt_count) = {
                            let mut ctx_guard = self.recovery_contexts.lock().map_err(|e| {
                                ExecutionError::InternalError {
                                    detail: format!("Recovery context lock error: {}", e),
                                }
                            })?;
                            let ctx = ctx_guard
                                .entry(input.dag_id)
                                .or_insert_with(RecoveryContext::new);
                            let recipe_check = recipe_out
                                .recipe
                                .as_ref()
                                .map(|r| ctx.can_attempt(scenario, r))
                                .unwrap_or(false);
                            let count = ctx.attempt_count(scenario);
                            (recipe_check, count)
                        };

                        if let Some(ref recipe) = recipe_out.recipe
                            && can_attempt
                        {
                            let attempt = attempt_count + 1;
                            let recovery_input = AttemptRecoveryInput {
                                scenario,
                                recipe: recipe.clone(),
                                attempt_number: attempt,
                                original_error: Some(error_message.clone()),
                                execution_id: Some(input.dag_id.to_string()),
                            };
                            if let Ok(recovery_out) =
                                recovery_svc.attempt_recovery(recovery_input).await
                            {
                                {
                                    let mut ctx_guard =
                                        self.recovery_contexts.lock().map_err(|e| {
                                            ExecutionError::InternalError {
                                                detail: format!(
                                                    "Recovery context lock error: {}",
                                                    e
                                                ),
                                            }
                                        })?;
                                    let ctx = ctx_guard
                                        .entry(input.dag_id)
                                        .or_insert_with(RecoveryContext::new);
                                    ctx.record_attempt(scenario);
                                }
                                tracing::info!(
                                    dag_id = %input.dag_id,
                                    node_id = %node_id,
                                    scenario = %scenario.as_str(),
                                    result = %recovery_out.result.summary(),
                                    "Recovery attempted"
                                );

                                if recovery_out.result.is_recovered() {
                                    // Recovery succeeded — re-add node to ready queue
                                    let mut sessions = self.sessions.lock().map_err(|e| {
                                        ExecutionError::InternalError {
                                            detail: format!("Session lock error: {}", e),
                                        }
                                    })?;
                                    if let Some(session) = sessions.get_mut(&input.dag_id)
                                        && let Some(state) = session.node_states.get_mut(&node_id)
                                    {
                                        state.status = NodeStatus::Ready;
                                    }
                                    continue; // Re-enter the retry loop
                                }
                            }
                        }
                    }
                }
            }

            // --- Phase 4: Handle failure with retry evaluation ---

            let failure_context = FailureContext::new(
                node_id,
                format!("node-{}", node_id),
                "tool",
                "node intent",
                &failure_type,
                &error_message,
                attempt,
                max_attempts,
                duration_ms,
                duration_ms,
            );

            let retry_input = EvaluateRetryInput {
                failure_context,
                policy: policy.clone(),
                fallback_node_id: None,
            };

            let retry_output = self
                .retry_service
                .evaluate_retry(retry_input)
                .await
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Retry evaluation failed: {}", e),
                })?;

            match retry_output.decision {
                RetryDecision::Retry {
                    strategy,
                    attempt: next,
                    backoff_ms,
                    ..
                } => {
                    // GAP-A-11: session-wide retry budget — when
                    // max_total_retries_per_session is crossed, stop retrying
                    // and fail the node instead of looping forever.
                    let budget_exhausted = {
                        let mut sessions =
                            self.sessions
                                .lock()
                                .map_err(|e| ExecutionError::InternalError {
                                    detail: format!("Lock error: {e}"),
                                })?;
                        match sessions.get_mut(&input.dag_id) {
                            Some(s) => {
                                if self.config.max_total_retries_per_session > 0
                                    && s.total_retries >= self.config.max_total_retries_per_session
                                {
                                    true
                                } else {
                                    s.total_retries += 1;
                                    false
                                }
                            }
                            None => false,
                        }
                    };
                    if budget_exhausted {
                        let result = TaskResult::failure(
                            node_id,
                            format!("node-{}", node_id),
                            format!(
                                "Session retry budget exhausted ({})",
                                self.config.max_total_retries_per_session
                            ),
                            "retry_budget_exhausted".to_string(),
                            duration_ms,
                            attempt,
                        );
                        return Ok(ExecuteNodeOutput {
                            result,
                            retry_decision: Some(RetryDecision::Abort {
                                reason: "Session retry budget exhausted".to_string(),
                            }),
                        });
                    }
                    if backoff_ms > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                    }
                    last_retry_decision = Some(RetryDecision::Retry {
                        strategy,
                        attempt: next,
                        backoff_ms,
                        reason: format!("Retry attempt {}/{}", attempt + 1, max_attempts),
                    });
                    // Loop continues to next attempt
                }
                RetryDecision::Fallback {
                    fallback_node_id, ..
                } => {
                    let result = TaskResult::failure(
                        node_id,
                        format!("node-{}", node_id),
                        format!("Fallback to node {}", fallback_node_id),
                        "fallback".to_string(),
                        duration_ms,
                        attempt,
                    );
                    return Ok(ExecuteNodeOutput {
                        result,
                        retry_decision: Some(RetryDecision::Fallback {
                            fallback_node_id,
                            reason: format!("Retries exhausted at attempt {}", attempt + 1),
                        }),
                    });
                }
                RetryDecision::Skip { reason } => {
                    let result = TaskResult::failure(
                        node_id,
                        format!("node-{}", node_id),
                        reason.clone(),
                        "skipped".to_string(),
                        duration_ms,
                        attempt,
                    );
                    return Ok(ExecuteNodeOutput {
                        result,
                        retry_decision: Some(RetryDecision::Skip { reason }),
                    });
                }
                RetryDecision::Abort { reason } => {
                    let result = TaskResult::failure(
                        node_id,
                        format!("node-{}", node_id),
                        reason.clone(),
                        "aborted".to_string(),
                        duration_ms,
                        attempt,
                    );
                    return Ok(ExecuteNodeOutput {
                        result,
                        retry_decision: Some(RetryDecision::Abort { reason }),
                    });
                }
            }
        }

        // All attempts exhausted without success
        let result = TaskResult::failure(
            node_id,
            format!("node-{}", node_id),
            format!("All {} attempts exhausted", max_attempts),
            "exhausted".to_string(),
            0,
            max_attempts.saturating_sub(1),
        );

        Ok(ExecuteNodeOutput {
            result,
            retry_decision: None,
        })
    }
}

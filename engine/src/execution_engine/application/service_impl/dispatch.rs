//! Dispatch loop + sequence-policy dispatch helpers for
//! `ParallelExecutionServiceImpl` (extracted from `service_impl.rs`, #914).
//!
//! @canonical .pi/architecture/modules/execution-engine.md#dispatch
//! Implements: ADR-019 effect-commitment write-ahead/settle in `run_dispatch_loop`
//! Issue: #1129

use sha2::{Digest, Sha256};

use crate::effect_commitment::application::key::IntentKeyDerivation;
use crate::effect_commitment::application::key_impl::Sha256IntentKeyDerivation;
use crate::effect_commitment::application::{DefaultRetryFence, RetryFence};

use super::gate::GateOutcome;
use super::*;

/// The outcome of the ADR-018 capacity seam's pre-dispatch check.
///
/// Declared at module scope (an `enum` cannot live inside an `impl` block).
pub(super) enum CapacityVerdict {
    /// No rule governs this step, or a grant was taken. Dispatch.
    Dispatch,
    /// The step was refused; the tool must never be called.
    Deny {
        /// The contended resource label.
        resource: String,
        /// Units the rule asked for.
        amount: u64,
        /// Human-readable cause (already redacted by the ledger error).
        detail: String,
        /// The reason, for evidence and operator differentiation.
        reason: &'static str,
    },
}

impl ParallelExecutionServiceImpl {
    /// Pop the next dispatchable ready node from the graph.
    ///
    /// Returns `(node_id, paused)`:
    /// - `(Some(id), false)` — `id` is ready to dispatch.
    /// - `(None, false)` — the ready queue is empty.
    /// - `(None, true)` — the front node requires un-granted human approval;
    ///   it was requeued and dispatch must pause until it is approved.
    pub(super) fn pop_dispatchable(
        graph: &mut crate::dag_engine::domain::TaskGraph,
        approved: &HashSet<Uuid>,
    ) -> (Option<Uuid>, bool) {
        let Some(node_id) = graph.pop_ready_node() else {
            return (None, false);
        };
        let needs_approval = graph
            .get_node(node_id)
            .map(|n| n.requires_approval && !approved.contains(&node_id))
            .unwrap_or(false);
        if needs_approval {
            graph.requeue_ready_node(node_id);
            (None, true)
        } else {
            (Some(node_id), false)
        }
    }

    /// Run the producer-consumer dispatch loop for a sealed TaskGraph.
    ///
    /// Dispatches ready nodes up to `max_concurrent`, gating dispatch on
    /// human approval: a ready node with `requires_approval: true` that is
    /// not in `approved` is requeued and marked `AwaitingApproval`, and
    /// dispatch pauses at the first such boundary. In-flight nodes always
    /// drain before the loop returns, so a paused execution has no dangling
    /// tasks and can be continued by `approve_node` + `resume_execution`.
    ///
    /// Returns `(completed_count, failed_count, node_results, approval_blocked)`.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn run_dispatch_loop(
        &self,
        graph: &mut crate::dag_engine::domain::TaskGraph,
        dag_id: Uuid,
        started_at: chrono::DateTime<chrono::Utc>,
        total_nodes: u32,
        approved: &HashSet<Uuid>,
        config: &crate::execution_engine::domain::ParallelExecutorConfig,
    ) -> Result<(u32, u32, HashMap<Uuid, TaskResult>, bool), ExecutionError> {
        let mut join_set: tokio::task::JoinSet<(Uuid, TaskResult)> = tokio::task::JoinSet::new();
        let mut completed_count: u32 = 0;
        let mut failed_count: u32 = 0;
        let mut node_results: HashMap<Uuid, TaskResult> = HashMap::new();
        let max_concurrent = config.max_concurrent_executions.max(1) as usize;
        let event_bus = &self.event_bus;
        let mut approval_blocked = false;
        // ADR-011 R5: pre-dispatch effect baseline (git oracle).
        //
        // Only when the graph has an approval-gated node: the baseline can only
        // be consumed by `effect_scope_check`, which runs for such nodes. Every
        // entry point now sets `approval_repo_path` (#1099), so taking two git
        // snapshots per run unconditionally would be pure overhead for the runs
        // that have nothing to check.
        let gated_nodes = graph.nodes.iter().any(|node| node.requires_approval);
        let scope_baseline: Option<ChangeSnapshot> = match &config.approval_repo_path {
            Some(repo) if gated_nodes => GitDiffEffectOracle
                .snapshot(std::path::Path::new(repo))
                .ok(),
            _ => None,
        };

        // Phase 1: Initial dispatch — fill the pipeline up to max_concurrent
        while join_set.len() < max_concurrent {
            let (node_id, paused) = Self::pop_dispatchable(graph, approved);
            if paused {
                approval_blocked = true;
                self.mark_awaiting_approval(dag_id, graph, total_nodes)
                    .await?;
                break;
            }
            let Some(node_id) = node_id else {
                break;
            };

            // Single consequence-gate sequence (#1074): approval binding →
            // precondition → sequence policy → capacity. Shared with `execute_node`
            // so the two routes cannot answer differently.
            match self
                .apply_gate(
                    dag_id,
                    graph,
                    node_id,
                    approved,
                    total_nodes,
                    &mut node_results,
                    &mut failed_count,
                )
                .await?
            {
                GateOutcome::Dispatch => {}
                GateOutcome::Paused => {
                    approval_blocked = true;
                    break;
                }
                GateOutcome::Refused => continue,
            }

            // ADR-019: write the intent durably before the tool is sent. A
            // failed begin refuses the dispatch.
            self.begin_effect(dag_id, graph, node_id).await?;

            spawn_concurrent_node(
                &mut join_set,
                graph,
                event_bus,
                &self.sessions,
                dag_id,
                node_id,
                &self.permission_enforcer,
                &self.budget_enforcer_for(dag_id),
                &self.hook_runner,
                config.enable_enforcement,
            )
            .await?;
        }

        // Phase 2: Consume completions, dispatch new nodes as slots open
        while let Some(joined) = join_set.join_next().await {
            let (node_id, task_result) = joined.map_err(|e| ExecutionError::InternalError {
                detail: format!("Task panicked: {e}"),
            })?;

            let success = task_result.success;
            if success {
                completed_count += 1;
            } else {
                failed_count += 1;
            }
            node_results.insert(node_id, task_result.clone());

            // ADR-019: settle the write-ahead record from what the dispatch
            // observed. A lost/timeout/cancelled outcome becomes Ambiguous.
            self.settle_effect(node_id, &task_result).await;

            // GAP-A-11: abort dispatch when the failure threshold is crossed
            // (0 = unlimited) or the session was cancelled.
            if config.max_failures_before_abort > 0
                && failed_count >= config.max_failures_before_abort
            {
                tracing::warn!(
                    %dag_id,
                    failed_count,
                    threshold = config.max_failures_before_abort,
                    "aborting dispatch: failure threshold reached"
                );
                break;
            }
            if config.enable_cancellation {
                let cancelled = {
                    let sessions =
                        self.sessions
                            .lock()
                            .map_err(|e| ExecutionError::InternalError {
                                detail: format!("Lock error: {e}"),
                            })?;
                    sessions.get(&dag_id).map(|s| s.aborted).unwrap_or(false)
                };
                if cancelled {
                    tracing::warn!(%dag_id, "aborting dispatch: execution cancelled");
                    break;
                }
            }

            // ADR-018: settle the capacity hold for this node's terminal outcome.
            // Success commits; failure releases immediately (RAII) instead of
            // letting the hold lapse with its lease.
            self.settle_capacity(node_id, success).await;

            // Update session node state (lock scope ends before notify_progress:
            // it re-locks `sessions` internally — a self-deadlock otherwise).
            let updated_state = {
                let mut sessions =
                    self.sessions
                        .lock()
                        .map_err(|e| ExecutionError::InternalError {
                            detail: format!("Lock error: {e}"),
                        })?;
                if let Some(session) = sessions.get_mut(&dag_id)
                    && let Some(state) = session.node_states.get_mut(&node_id)
                {
                    if success {
                        state.mark_completed(task_result.duration_ms);
                    } else {
                        state.mark_failed(
                            task_result
                                .failure_type
                                .clone()
                                .unwrap_or_else(|| "unknown".to_string()),
                            task_result.error.clone().unwrap_or_default(),
                        );
                    }
                    Some(state.clone())
                } else {
                    None
                }
            };
            if let Some(state) = updated_state {
                self.notify_progress(dag_id, node_id, state, total_nodes);
            }

            // ADR-011 R3: consume the single-use approval on terminal outcome
            // (success, skipped, or exhausted failure after ≥1 dispatch).
            if graph
                .get_node(node_id)
                .map(|n| n.requires_approval)
                .unwrap_or(false)
            {
                self.consume_on_terminal(node_id).await;
                if let Some(baseline) = &scope_baseline
                    && let Some(repo) = &config.approval_repo_path
                {
                    self.effect_scope_check(dag_id, node_id, repo, baseline)
                        .await;
                }
            }

            // ADR-017 R2: release dependents, or stop the failure so its
            // transitive dependents are Skipped and never dispatched.
            self.release_or_skip_dependents(graph, dag_id, node_id, total_nodes, !success)
                .await;

            // Phase 3: Dispatch newly ready nodes (a slot just opened)
            while join_set.len() < max_concurrent {
                let (next_id, paused) = Self::pop_dispatchable(graph, approved);
                if paused {
                    approval_blocked = true;
                    self.mark_awaiting_approval(dag_id, graph, total_nodes)
                        .await?;
                    break;
                }
                let Some(next_id) = next_id else {
                    break;
                };

                // Single consequence-gate sequence (#1074): approval binding →
                // precondition → sequence policy → capacity. Shared with `execute_node`
                // so the two routes cannot answer differently.
                match self
                    .apply_gate(
                        dag_id,
                        graph,
                        next_id,
                        approved,
                        total_nodes,
                        &mut node_results,
                        &mut failed_count,
                    )
                    .await?
                {
                    GateOutcome::Dispatch => {}
                    GateOutcome::Paused => {
                        approval_blocked = true;
                        break;
                    }
                    GateOutcome::Refused => continue,
                }

                // ADR-019: write the intent durably before the tool is sent.
                self.begin_effect(dag_id, graph, next_id).await?;

                spawn_concurrent_node(
                    &mut join_set,
                    graph,
                    event_bus,
                    &self.sessions,
                    dag_id,
                    next_id,
                    &self.permission_enforcer,
                    &self.budget_enforcer_for(dag_id),
                    &self.hook_runner,
                    config.enable_enforcement,
                )
                .await?;
            }

            // Update aggregate result in session
            {
                let mut sessions =
                    self.sessions
                        .lock()
                        .map_err(|e| ExecutionError::InternalError {
                            detail: format!("Lock error: {e}"),
                        })?;
                if let Some(session) = sessions.get_mut(&dag_id) {
                    session.result = ExecutionResult {
                        dag_id,
                        node_results: node_results.clone(),
                        execution_states: session.node_states.clone(),
                        completed_count,
                        failed_count,
                        skipped_count: 0,
                        total_nodes,
                        total_duration_ms: Utc::now()
                            .signed_duration_since(started_at)
                            .num_milliseconds()
                            .max(0) as u64,
                        total_retries: node_results.values().map(|r| r.retry_attempts as u32).sum(),
                        started_at,
                        completed_at: Utc::now(),
                        cancelled: false,
                        cancellation_reason: None,
                    };
                }
            }
        }

        Ok((
            completed_count,
            failed_count,
            node_results,
            approval_blocked,
        ))
    }

    /// ADR-011 choke point — verify a gated node's intent before dispatch.
    ///
    /// Returns `Ok(true)` to dispatch (`Matched`), `Ok(false)` to HALT
    /// (`Mismatched` / `Invalid` / missing record — the tool is never called),
    /// or an internal error when the approval service itself fails (fail-closed).
    /// ADR-011 choke point — verify a gated node's intent before dispatch.
    ///
    /// Returns `(dispatch, mismatch_digests)`:
    /// - `(true, _)` — verified (`Matched`) — dispatch
    /// - `(false, None)` — invalid / missing record — HALT
    /// - `(false, Some((expected, actual)))` — intent mismatch — HALT with the
    ///   digests for the audit event. The tool is never called on `false`.
    pub(super) async fn verify_before_dispatch(
        &self,
        node_id: Uuid,
    ) -> Result<(bool, Option<(String, String)>), ExecutionError> {
        let Some(binding) = &self.approval_binding else {
            return Ok((true, None)); // legacy gate — no binding
        };
        match binding.service.verify_intent(node_id).await {
            Ok(IntentVerification::Matched) => {
                tracing::info!(node_id = %node_id, "dispatch: intent MATCHED — verified against approved record");
                Ok((true, None))
            }
            Ok(IntentVerification::Mismatched { expected, actual }) => {
                tracing::error!(
                    node_id = %node_id,
                    expected = %expected.0,
                    actual = %actual.0,
                    "dispatch HALT: INTENT MISMATCH — re-approval required"
                );
                Ok((false, Some((expected.0, actual.0))))
            }
            Ok(verdict) => {
                tracing::error!(
                    node_id = %node_id,
                    verdict = ?verdict,
                    "dispatch HALT: approval invalid — re-approval required"
                );
                Ok((false, None))
            }
            Err(ApprovalServiceError::NotFound(_)) => {
                tracing::warn!(
                    node_id = %node_id,
                    "dispatch HALT: no approval record (legacy or invalidated) — re-approval required"
                );
                Ok((false, None))
            }
            Err(e) => Err(ExecutionError::InternalError {
                detail: format!("Approval verification failed: {e}"),
            }),
        }
    }

    /// Halt a node on intent mismatch (R2): never dispatches; the node moves
    /// to `IntentMismatch`, is requeued (so a later re-approval + resume can
    /// pick it up), and is dropped from the session's approved set.
    pub(super) async fn halt_intent_mismatch(
        &self,
        dag_id: Uuid,
        graph: &mut crate::dag_engine::domain::TaskGraph,
        node_id: Uuid,
        total_nodes: u32,
    ) -> Result<(), ExecutionError> {
        graph.requeue_ready_node(node_id);
        let updated_state = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Lock error: {e}"),
                })?;
            if let Some(session) = sessions.get_mut(&dag_id) {
                session.approved.remove(&node_id);
                if let Some(state) = session.node_states.get_mut(&node_id) {
                    state.mark_intent_mismatch();
                    Some(state.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some(state) = updated_state {
            self.notify_progress(dag_id, node_id, state, total_nodes);
        }
        Ok(())
    }

    /// ADR-011 R5: post-execution effect-scope verification (evidence only).
    ///
    /// Compares the run's net git changes against the approved record's
    /// declared scope and forwards any violation to the binding (non-blocking;
    /// the blocking check is R2 verification at the choke point).
    pub(super) async fn effect_scope_check(
        &self,
        dag_id: Uuid,
        node_id: Uuid,
        repo_path: &str,
        baseline: &ChangeSnapshot,
    ) {
        let Some(binding) = &self.approval_binding else {
            return;
        };
        let Ok(Some(record)) = binding.service.get_approval(node_id).await else {
            return;
        };
        // Declared scope lives in the canonical intent payload captured at
        // approval time. No declared scope ⇒ nothing to verify against.
        let declared: Vec<String> = record
            .intent_payload
            .get("declared_scope")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(|x| x.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        if declared.is_empty() {
            return;
        }
        let oracle = GitDiffEffectOracle;
        let Ok(post) = oracle.snapshot(std::path::Path::new(repo_path)) else {
            return;
        };
        let effects = oracle.diff(baseline, &post);
        if effects.is_empty() {
            return;
        }
        if let Some(violation) = crate::approval::domain::ScopeViolation::detect(
            record.node_id,
            record.step_name.clone(),
            &declared,
            &effects,
            chrono::Utc::now(),
        ) {
            tracing::warn!(
                node_id = %node_id,
                out_of_scope = ?violation.out_of_scope,
                "effect-scope violation recorded (non-blocking evidence)"
            );
            let _ = binding
                .service
                .record_scope_violation(violation.clone())
                .await;
            // ADR-011 R5: publish ScopeViolationRecorded — the drained events
            // are the source the audit envelope derives `scope_violations[]`
            // from.
            self.publish_event(ExecutionEvent::ScopeViolationRecorded {
                execution_id: dag_id,
                node_id: violation.node_id.to_string(),
                step_name: violation.step_name.clone(),
                out_of_scope: violation.out_of_scope.clone(),
                timestamp: chrono::Utc::now(),
            })
            .await;
        }
    }

    /// Single-use settlement: consume the approval once the node reaches a
    /// terminal outcome. Non-terminal failures never reach this point.
    pub(super) async fn consume_on_terminal(&self, node_id: Uuid) {
        let Some(binding) = &self.approval_binding else {
            return;
        };
        match binding.service.consume(node_id).await {
            Ok(()) => tracing::info!(node_id = %node_id, "approval consumed (terminal outcome)"),
            Err(ApprovalServiceError::NotFound(_) | ApprovalServiceError::AlreadyConsumed(_)) => {}
            Err(e) => {
                tracing::warn!(node_id = %node_id, error = %e, "consume failed (non-fatal)")
            }
        }
    }

    /// Publish an execution event (best-effort — evidence must never block the run).
    pub(super) async fn publish_event(&self, event: ExecutionEvent) {
        if let Err(e) = self
            .event_bus
            .publish(crate::event_system::application::dto::PublishEventInput { event })
            .await
        {
            tracing::warn!(error = %e, "event publish failed — evidence may be incomplete");
        }
    }

    /// Mark the front ready node as awaiting human approval in the session.
    ///
    /// Called when dispatch pauses at an approval boundary. The blocked node
    /// was requeued to the front of the ready queue by `pop_dispatchable`.
    pub(super) async fn mark_awaiting_approval(
        &self,
        dag_id: Uuid,
        graph: &crate::dag_engine::domain::TaskGraph,
        total_nodes: u32,
    ) -> Result<(), ExecutionError> {
        let Some(blocked_id) = graph.ready_nodes().first().copied() else {
            return Ok(());
        };
        let updated_state = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Lock error: {}", e),
                })?;
            if let Some(session) = sessions.get_mut(&dag_id)
                && let Some(state) = session.node_states.get_mut(&blocked_id)
            {
                state.mark_awaiting_approval();
                Some(state.clone())
            } else {
                None
            }
        };
        // Drop the sessions lock before notify (it re-locks internally).
        if let Some(state) = updated_state {
            self.notify_progress(dag_id, blocked_id, state, total_nodes);
        }
        Ok(())
    }

    /// R3 — run-time prefix gate (module doc §R3; AC#9).
    ///
    /// Builds the session's completed dispatch prefix (successful
    /// completions in graph declaration order — deterministic for an
    /// identical DAG + identical completed prefix) plus `node_id` as the
    /// proposed next step and asks the policy service `evaluate_prefix`:
    ///
    /// - any match with action `deny` → `Deny` (outranks promote; a deny is
    ///   absolute — human approval cannot override a forbidden sequence)
    /// - any match with action `promote`, node not already human-approved →
    ///   `Promote` (a previously promoted-and-approved node dispatches: its
    ///   pause was already the human decision)
    /// - otherwise → `Dispatch`
    ///
    /// An evaluation error fails CLOSED — the run halts before the node
    /// dispatches (corrupt/over-cap rule config must never execute silently).
    pub(super) async fn sequence_policy_verdict(
        &self,
        dag_id: Uuid,
        graph: &crate::dag_engine::domain::TaskGraph,
        node_id: Uuid,
        approved: &HashSet<Uuid>,
    ) -> Result<SequencePolicyVerdict, ExecutionError> {
        let Some(svc) = &self.sequence_policy else {
            return Ok(SequencePolicyVerdict::Dispatch);
        };

        // Snapshot the completed prefix + next step under the sessions lock
        // (no awaits held). Missing session / node ⇒ nothing to gate.
        let (prefix, next) = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|e| ExecutionError::InternalError {
                    detail: format!("Lock error: {e}"),
                })?;
            let Some(session) = sessions.get(&dag_id) else {
                return Ok(SequencePolicyVerdict::Dispatch);
            };
            let prefix = graph
                .nodes()
                .filter(|n| {
                    session
                        .node_states
                        .get(&n.id)
                        .map(|s| s.status == NodeStatus::Completed)
                        .unwrap_or(false)
                })
                .map(|n| DispatchedStep {
                    name: n.name.clone(),
                    tool: n.tool.clone(),
                    parameters: serde_json::from_str(&n.intent).unwrap_or_default(),
                })
                .collect::<Vec<_>>();
            let node = match graph.get_node(node_id) {
                Some(n) => n,
                None => return Ok(SequencePolicyVerdict::Dispatch),
            };
            let next = PlannedStep {
                name: node.name.clone(),
                tool: node.tool.clone(),
                parameters: serde_json::from_str(&node.intent).unwrap_or_default(),
            };
            (prefix, next)
        };

        let matches = match svc.evaluate_prefix(&prefix, &next, None).await {
            Ok(m) => m,
            Err(e) => {
                // R6: record the fail-closed halt (GAP-M-14 — never silent).
                self.publish_event(ExecutionEvent::SequencePolicyConfigError {
                    execution_id: dag_id,
                    detail: e.to_string(),
                    timestamp: chrono::Utc::now(),
                })
                .await;
                return Err(ExecutionError::InternalError {
                    detail: format!(
                        "Sequence policy evaluation failed — run halted before dispatch (fail closed): {e}"
                    ),
                });
            }
        };

        // R6 evidence: record the runtime gate decision as first-class
        // events BEFORE acting on it (envelope `sequence_policy_findings[]`
        // derives from these). Summaries are pre-redacted (SpanPrivacy).
        for m in &matches {
            if m.action == RuleAction::Deny {
                self.publish_event(ExecutionEvent::SequencePolicyDenied {
                    execution_id: dag_id,
                    rule_id: m.rule_id.clone(),
                    later_step: m.later_step.clone(),
                    reason: format!(
                        "rule '{}' denies step '{}' before dispatch",
                        m.rule_id, m.later_step
                    ),
                    timestamp: chrono::Utc::now(),
                })
                .await;
                return Ok(SequencePolicyVerdict::Deny {
                    rule_id: m.rule_id.clone(),
                    later_step: m.later_step.clone(),
                });
            }
        }
        // A previously promoted-and-approved node dispatches on resume — its
        // promotion was already recorded when it was first gated.
        if matches.iter().any(|m| m.action == RuleAction::Promote) && !approved.contains(&node_id) {
            for m in matches.iter().filter(|m| m.action == RuleAction::Promote) {
                self.publish_event(ExecutionEvent::SequenceRuleMatched {
                    execution_id: dag_id,
                    rule_id: m.rule_id.clone(),
                    action: "promote".to_string(),
                    later_step: m.later_step.clone(),
                    matched_indices: m.matched_indices.clone(),
                    summary: m.decision_summary(),
                    timestamp: chrono::Utc::now(),
                })
                .await;
            }
            return Ok(SequencePolicyVerdict::Promote);
        }

        // #1069: the plan-time companion check is a *presence* lint. Here, at
        // the dispatch seam, the companion must have already **completed
        // successfully**: `prefix` is built from `NodeStatus::Completed` nodes,
        // so it is the outcome set, not the plan. A companion listed after the
        // matched step, or one that failed, does not satisfy this.
        let completed_steps: Vec<PlannedStep> = prefix
            .iter()
            .map(|step| PlannedStep {
                name: step.name.clone(),
                tool: step.tool.clone(),
                parameters: step.parameters.clone(),
            })
            .collect();
        match svc
            .evaluate_companion_prerequisite(&completed_steps, &next)
            .await
        {
            Ok(findings) => {
                if let Some(finding) = findings.into_iter().next() {
                    tracing::warn!(
                        %dag_id,
                        %node_id,
                        obligation = %finding.obligation_id,
                        required = %finding.required_companion,
                        "companion prerequisite unmet — step refused before dispatch (fail closed)"
                    );
                    return Ok(SequencePolicyVerdict::CompanionUnmet {
                        obligation_id: finding.obligation_id,
                        required_companion: finding.required_companion,
                    });
                }
            }
            Err(e) => {
                self.publish_event(ExecutionEvent::SequencePolicyConfigError {
                    execution_id: dag_id,
                    detail: format!("companion prerequisite: {e}"),
                    timestamp: chrono::Utc::now(),
                })
                .await;
                return Err(ExecutionError::InternalError {
                    detail: format!(
                        "Companion prerequisite evaluation failed — run halted before dispatch (fail closed): {e}"
                    ),
                });
            }
        }
        Ok(SequencePolicyVerdict::Dispatch)
    }

    /// Record an R3 sequence-policy denial as a deterministic pre-dispatch
    /// node failure (no tool call, no `NodeStarted`). Shared by the two
    /// dispatch fill loops.
    ///
    /// Returns the updated node state so the caller can notify progress.
    pub(super) fn record_sequence_denial(
        &self,
        dag_id: Uuid,
        node_id: Uuid,
        rule_id: &str,
        later_step: &str,
    ) -> Option<(Uuid, NodeExecutionState)> {
        let mut sessions = match self.sessions.lock() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "sequence-policy denial: sessions lock poisoned");
                return None;
            }
        };
        let session = sessions.get_mut(&dag_id)?;
        let state = session.node_states.get_mut(&node_id)?;
        let error = format!(
            "Sequence policy denied by rule '{rule_id}' — step '{later_step}' must not dispatch"
        );
        state.mark_failed("sequence_policy_denied".to_string(), error);
        let cloned = state.clone();
        Some((node_id, cloned))
    }

    /// Build the intent identity shared by the effect record and the
    /// capacity hold (#1163).
    ///
    /// The gate's digests are not yet exposed on the node, so the intent
    /// identity is derived from the node and execution identity for now. The
    /// placeholder key is replaced by the service's intent derivation; the
    /// digest fields are what make the derived key stable across retries.
    fn effect_intent(
        dag_id: Uuid,
        node_id: Uuid,
        node: &crate::dag_engine::domain::TaskNode,
    ) -> EffectIntent {
        EffectIntent::new(
            dag_id,
            node_id,
            0,
            IdempotencyKey::new(format!("{dag_id}:{node_id}:pending")),
            node.tool.clone(),
            Self::effect_digest(&node.intent),
            Self::effect_digest(&format!("{dag_id}:{node_id}:precondition")),
            Self::effect_digest(&format!("{dag_id}:{node_id}:authority")),
        )
    }

    /// R1 dispatch-time precondition verdict for one ready node (ADR-017).
    ///
    /// Returns `Dispatch` when no gate is wired or the gate allows dispatch.
    /// A `Deny` verdict means the tool must never be called. An unarmed gate
    /// (`NotArmed`) is a fail-closed refusal mapped to `Deny { error }`; any
    /// other gate error halts the run fail-closed (mirrors sequence policy).
    /// ADR-019: write the intent durably before `spawn_concurrent_node`.
    ///
    /// A missing node or a failed `begin` refuses the dispatch; the tool is
    /// never sent. The returned record id is remembered by node id so the
    /// terminal outcome can settle it.
    pub(super) async fn begin_effect(
        &self,
        dag_id: Uuid,
        graph: &crate::dag_engine::domain::TaskGraph,
        node_id: Uuid,
    ) -> Result<(), ExecutionError> {
        let Some(seam) = &self.effect_commitment else {
            return Ok(());
        };

        let node = graph
            .get_node(node_id)
            .ok_or_else(|| ExecutionError::InternalError {
                detail: format!("effect commitment: node {node_id} is not in the graph"),
            })?;

        let intent = Self::effect_intent(dag_id, node_id, node);

        let record = seam.before_dispatch(intent).await.map_err(|error| {
            ExecutionError::InternalError {
                detail: format!(
                    "effect commitment begin failed; refusing to dispatch node {node_id}: {error}"
                ),
            }
        })?;

        // ADR-019 F2: a write-ahead read that came back unsettled means this
        // intent was already dispatched and its outcome is unknown. Re-sending
        // it is the double-spend path.
        if !DefaultRetryFence.may_retry(&record) {
            return Err(ExecutionError::InternalError {
                detail: format!(
                    "effect {} is Ambiguous; retry refused (ADR-019 F2)",
                    record.id()
                ),
            });
        }
        if record.is_terminal() {
            return Err(ExecutionError::InternalError {
                detail: format!(
                    "effect {} is already settled ({}); refusing to re-dispatch a terminal record",
                    record.id(),
                    record.state()
                ),
            });
        }

        let mut records =
            self.effect_records
                .lock()
                .map_err(|_| ExecutionError::InternalError {
                    detail: "effect record registry poisoned; refusing to dispatch".to_string(),
                })?;
        records.insert(node_id, record.id());
        Ok(())
    }

    /// ADR-019: settle the write-ahead record from the dispatch outcome.
    ///
    /// A definite result settles `Committed`/`NotCommitted`; a timeout,
    /// cancellation, network failure, or lost response marks the record
    /// `Ambiguous` so dependents and retry are fenced.
    pub(super) async fn settle_effect(&self, node_id: Uuid, result: &TaskResult) {
        let Some(seam) = &self.effect_commitment else {
            return;
        };

        let record_id = self
            .effect_records
            .lock()
            .ok()
            .and_then(|mut records| records.remove(&node_id));
        let Some(record_id) = record_id else {
            return;
        };

        let failure = result
            .failure_type
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();

        let observation = if Self::is_lost_outcome(result) {
            DispatchObservation::Lost {
                reason: format!("dispatch outcome unknown ({failure})"),
            }
        } else {
            let digest = Self::effect_digest(&format!(
                "{node_id}:{}",
                result.output.as_deref().unwrap_or_default()
            ));
            let receipt = format!("node:{node_id}");
            let observed_at = Utc::now();
            let source = "execution-engine";
            let outcome = if result.success {
                ObservedOutcome::committed(receipt, digest, observed_at, source)
            } else {
                ObservedOutcome::not_committed(receipt, digest, observed_at, source)
            };
            DispatchObservation::Observed(outcome)
        };

        match seam.after_dispatch(record_id, observation).await {
            Ok(record) => self.record_effect_fence(node_id, record.blocks_dependents()),
            Err(error) => {
                tracing::warn!(
                    %node_id,
                    %error,
                    "effect commitment settle failed; attempting to mark ambiguous"
                );
                match seam
                    .after_dispatch(
                        record_id,
                        DispatchObservation::Lost {
                            reason: format!("settle failed: {error}"),
                        },
                    )
                    .await
                {
                    Ok(record) => {
                        self.record_effect_fence(node_id, record.blocks_dependents());
                    }
                    Err(second) => {
                        // We could not prove the record terminal. Fence.
                        tracing::warn!(
                            %node_id,
                            %second,
                            "effect commitment could not record the unknown outcome; fencing"
                        );
                        self.record_effect_fence(node_id, true);
                    }
                }
            }
        }
    }

    /// Remember whether a node's effect blocks its dependents (F1).
    fn record_effect_fence(&self, node_id: Uuid, ambiguous: bool) {
        if let Ok(mut fenced) = self.effect_ambiguous.lock() {
            if ambiguous {
                fenced.insert(node_id);
            } else {
                fenced.remove(&node_id);
            }
        }
    }

    /// Whether a node's effect is still `Ambiguous`.
    ///
    /// A poisoned lock is treated as fenced: releasing dependents on unknown
    /// state would be the double-spend path.
    pub(super) fn effect_blocks_dependents(&self, node_id: Uuid) -> bool {
        self.effect_ambiguous
            .lock()
            .map(|fenced| fenced.contains(&node_id))
            .unwrap_or(true)
    }

    /// Whether a task result is an unknown outcome rather than a definite one.
    ///
    /// A lapsed deadline, an in-flight cancellation, or a lost transport is
    /// absence of evidence, not evidence of absence: the record must become
    /// `Ambiguous`, never `NotCommitted`.
    pub(super) fn is_lost_outcome(result: &TaskResult) -> bool {
        let failure = result
            .failure_type
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let error_text = result
            .error
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();

        matches!(
            failure.as_str(),
            "timeout"
                | "timed_out"
                | "command_timed_out"
                | "cancelled"
                | "canceled"
                | "cancellation"
                | "network"
                | "lost_response"
        ) || error_text.contains("timed out")
            || error_text.contains("timeout")
            || error_text.contains("cancelled")
            || error_text.contains("canceled")
    }

    fn effect_digest(seed: &str) -> String {
        hex::encode(Sha256::digest(seed.as_bytes()))
    }

    /// ADR-018 capacity-ledger pre-dispatch check.
    ///
    /// Deliberately placed **after** the precondition and sequence gates so
    /// capacity is never reserved for a step that will be denied, and
    /// immediately before `spawn_concurrent_node` so a lease is held for the
    /// shortest possible time.
    ///
    /// * no seam, or no rule for this step -> `Dispatch` (status quo)
    /// * a grant was taken -> `Dispatch` (guard stored for settlement)
    /// * `Insufficient`, `Unavailable`, or any other ledger failure -> `Deny`
    ///   (fail closed; the tool is never called)
    pub(super) async fn capacity_verdict(
        &self,
        dag_id: Uuid,
        graph: &crate::dag_engine::domain::TaskGraph,
        node_id: Uuid,
    ) -> Result<CapacityVerdict, ExecutionError> {
        let Some(setup) = &self.capacity else {
            return Ok(CapacityVerdict::Dispatch);
        };
        let Some(node) = graph.get_node(node_id) else {
            return Ok(CapacityVerdict::Dispatch);
        };
        let Some(rule) = setup.rule_for(&node.name, &node.tool) else {
            return Ok(CapacityVerdict::Dispatch);
        };

        // The hold and the effect record must share one identity (#1163): a
        // deterministic key from (dag_id, node_id, intent_digest). A same-key,
        // same-amount retry collapses onto the existing reservation (#1052), so
        // a resumed dispatch finds the hold it already owns instead of holding
        // the resource twice. A different amount is refused as `Contended`
        // rather than silently under-reserved.
        let intent = Self::effect_intent(dag_id, node_id, node);
        let idempotency_key = Sha256IntentKeyDerivation::new().derive_key(&intent);
        let input = ReserveCapacityInput {
            resource: ResourceId::new(rule.resource.clone()),
            amount: rule.amount,
            idempotency_key,
            requested_by: Some(format!("{}:{}", node.name, node.tool)),
        };

        match setup.service.reserve(input).await {
            Ok(output) => {
                let mut holds =
                    self.capacity_holds
                        .lock()
                        .map_err(|e| ExecutionError::InternalError {
                            detail: format!("Lock error: {e}"),
                        })?;
                holds.insert(node_id, output.reservation.id);
                Ok(CapacityVerdict::Dispatch)
            }
            // Fail closed on EVERY ledger failure: a cap that fails open is not a
            // cap, and an outage must never read as "room available".
            Err(error) => {
                let reason = match error {
                    LedgerError::Insufficient { .. } => "capacity_exhausted",
                    _ => "capacity_unavailable",
                };
                Ok(CapacityVerdict::Deny {
                    resource: rule.resource.clone(),
                    amount: rule.amount,
                    detail: error.to_string(),
                    reason,
                })
            }
        }
    }

    /// Settle a node's capacity hold at its terminal outcome.
    ///
    /// Success commits (the capacity is spent); anything else releases the hold
    /// immediately rather than letting it lapse with the lease. A missing hold
    /// means the step was never governed, which is not an error.
    pub(super) async fn settle_capacity(&self, node_id: Uuid, success: bool) {
        let hold = {
            let mut holds = match self.capacity_holds.lock() {
                Ok(holds) => holds,
                Err(error) => {
                    tracing::warn!(
                        %node_id,
                        %error,
                        "capacity: hold registry poisoned; the hold will lapse with its lease"
                    );
                    return;
                }
            };
            holds.remove(&node_id)
        };
        let Some(reservation_id) = hold else { return };
        let Some(setup) = &self.capacity else { return };

        if success {
            match setup
                .service
                .commit(CommitCapacityInput { reservation_id })
                .await
            {
                Ok(_) => {
                    tracing::debug!(%node_id, "capacity: hold committed (step succeeded)")
                }
                Err(error) => tracing::warn!(
                    %node_id,
                    %error,
                    "capacity: commit failed; the lease will expire and return the capacity"
                ),
            }
        } else {
            match setup
                .service
                .release(ReleaseCapacityInput {
                    reservation_id,
                    reason: Some("step did not succeed".to_string()),
                })
                .await
            {
                Ok(_) => {
                    tracing::debug!(%node_id, "capacity: hold released (step did not succeed)")
                }
                Err(error) => tracing::warn!(
                    %node_id,
                    %error,
                    "capacity: release failed; the lease will expire and return the capacity"
                ),
            }
        }
    }

    pub(super) async fn precondition_verdict(
        &self,
        dag_id: Uuid,
        graph: &crate::dag_engine::domain::TaskGraph,
        node_id: Uuid,
    ) -> Result<PreconditionVerdict, ExecutionError> {
        let Some(gate) = &self.precondition_gate else {
            return Ok(PreconditionVerdict::Dispatch);
        };
        let Some(node) = graph.get_node(node_id) else {
            return Ok(PreconditionVerdict::Dispatch);
        };
        let step = DispatchStep {
            name: node.name.clone(),
            tool: node.tool.clone(),
            parameters: serde_json::from_str(&node.intent).unwrap_or_default(),
        };
        match gate.assess_with_findings(dag_id, &step).await {
            Ok((verdict, findings)) => {
                // Evidence (ADR-017): record every check that ran as a
                // first-class event; the envelope `precondition_findings[]`
                // derives from these. Summaries are pre-redacted (SpanPrivacy).
                for finding in &findings {
                    self.publish_event(ExecutionEvent::PreconditionChecked {
                        execution_id: dag_id,
                        precondition_id: finding.precondition_id.clone(),
                        step: finding.step.clone(),
                        outcome: finding.outcome.as_str().to_string(),
                        exit_code: finding.exit_code,
                        inputs_hash: finding.inputs_hash.clone(),
                        summary: finding.decision_summary(),
                        check_digest: finding.check_digest.clone(),
                        authority_digest: finding.authority_digest.clone(),
                        check_writable: finding.check_writable,
                        attribution: finding
                            .attribution
                            .map(|reason| reason.as_str().to_string()),
                        timestamp: finding.checked_at,
                    })
                    .await;
                }
                Ok(verdict)
            }
            Err(PreconditionError::NotArmed {
                precondition_id,
                detail,
            }) => {
                tracing::warn!(
                    precondition = %precondition_id,
                    %detail,
                    "precondition gate unarmed — refusing step (fail closed)"
                );
                Ok(PreconditionVerdict::Deny {
                    precondition_id,
                    outcome: PreconditionOutcome::Error,
                })
            }
            Err(error) => Err(ExecutionError::InternalError {
                detail: format!(
                    "Precondition evaluation failed — run halted before dispatch (fail closed): {error}"
                ),
            }),
        }
    }

    /// Record an R1 precondition denial as a deterministic pre-dispatch node
    /// failure (no tool call, no `NodeStarted`). Shared by the two dispatch
    /// fill loops.
    ///
    /// Returns the updated node state so the caller can notify progress.
    pub(super) fn record_precondition_denial(
        &self,
        dag_id: Uuid,
        node_id: Uuid,
        precondition_id: &str,
        outcome: PreconditionOutcome,
    ) -> Option<(Uuid, NodeExecutionState)> {
        let mut sessions = match self.sessions.lock() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "precondition denial: sessions lock poisoned");
                return None;
            }
        };
        let session = sessions.get_mut(&dag_id)?;
        let state = session.node_states.get_mut(&node_id)?;
        let error = format!(
            "Precondition '{precondition_id}' refused step before dispatch ({})",
            outcome.as_str()
        );
        state.mark_failed("precondition_denied".to_string(), error);
        let cloned = state.clone();
        Some((node_id, cloned))
    }

    /// ADR-017 R2: release a terminal node's dependents, or — when
    /// `release_dependents_on_failure = false` and the node failed — mark the
    /// failed node's transitive dependents `Skipped` and never dispatch them.
    ///
    /// The default gating mode (`release = true`) preserves today's behavior:
    /// `mark_completed` releases dependents. With `release = false`, the failed
    /// node and its transitive dependents are recorded as completed without
    /// decrementing any dependent's in-degree, so none enter the ready queue.
    pub(super) async fn release_or_skip_dependents(
        &self,
        graph: &mut crate::dag_engine::domain::TaskGraph,
        dag_id: Uuid,
        node_id: Uuid,
        total_nodes: u32,
        is_failure: bool,
    ) {
        // ADR-019 F1: an unsettled effect blocks its dependents. The node is
        // marked completed in the graph without decrementing any dependent's
        // in-degree, so nothing downstream can become ready. The run stays open
        // until the operator resolves the record.
        if self.effect_blocks_dependents(node_id) {
            tracing::warn!(
                %dag_id,
                %node_id,
                "effect is Ambiguous; dependents stay fenced (ADR-019 F1)"
            );
            let _ = graph.mark_completed_without_release(node_id);
            return;
        }

        if !is_failure || self.gating_mode.release_dependents_on_failure {
            let _ = graph.mark_completed(node_id);
            return;
        }

        let skipped = graph.transitive_dependents(node_id);
        let _ = graph.mark_completed_without_release(node_id);
        for dep in &skipped {
            let _ = graph.mark_completed_without_release(*dep);
        }

        let mut updates: Vec<(Uuid, NodeExecutionState)> = Vec::new();
        match self.sessions.lock() {
            Ok(mut sessions) => {
                if let Some(session) = sessions.get_mut(&dag_id) {
                    for dep in &skipped {
                        if let Some(state) = session.node_states.get_mut(dep) {
                            state.mark_skipped("dependency_failed".to_string());
                            updates.push((*dep, state.clone()));
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "gating: sessions lock poisoned while skipping dependents"
                );
            }
        }
        for (dep, state) in updates {
            self.notify_progress(dag_id, dep, state, total_nodes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(success: bool, failure_type: Option<&str>, error: Option<&str>) -> TaskResult {
        TaskResult {
            node_id: Uuid::nil(),
            node_name: "n".to_string(),
            success,
            output: None,
            duration_ms: 0,
            retry_attempts: 0,
            error: error.map(str::to_string),
            failure_type: failure_type.map(str::to_string),
            started_at: Utc::now(),
            completed_at: Utc::now(),
        }
    }

    fn node(id: Uuid, tool: &str, intent: &str) -> crate::dag_engine::domain::TaskNode {
        crate::dag_engine::domain::TaskNode::new(id, "n", tool, vec![], intent)
    }

    #[test]
    fn capacity_and_effect_share_an_identity_across_retries() {
        // #1163: the hold must use the same deterministic key as the record.
        let dag = Uuid::now_v7();
        let id = Uuid::now_v7();
        let n = node(id, "run_command", "echo hello");
        let a = ParallelExecutionServiceImpl::effect_intent(dag, id, &n);
        let b = ParallelExecutionServiceImpl::effect_intent(dag, id, &n);
        let derive = Sha256IntentKeyDerivation::new();
        assert_eq!(derive.derive_key(&a), derive.derive_key(&b));

        // A different payload is a different intent (and must not reuse a hold).
        let changed = node(id, "run_command", "echo goodbye");
        let c = ParallelExecutionServiceImpl::effect_intent(dag, id, &changed);
        assert_ne!(derive.derive_key(&a), derive.derive_key(&c));
    }

    #[test]
    fn lost_outcomes_are_the_ones_that_must_stay_ambiguous() {
        // Definite results.
        assert!(!ParallelExecutionServiceImpl::is_lost_outcome(&result(
            true, None, None
        )));
        assert!(!ParallelExecutionServiceImpl::is_lost_outcome(&result(
            false,
            Some("command_failed"),
            Some("exit status 1")
        )));
        assert!(!ParallelExecutionServiceImpl::is_lost_outcome(&result(
            false,
            Some("unknown_tool"),
            Some("unknown tool")
        )));

        // Unknown outcomes: deadline, cancellation, lost transport.
        for (failure_type, error) in [
            (Some("timeout"), None),
            (
                Some("command_timed_out"),
                Some("command timed out after 120s"),
            ),
            (Some("cancelled"), None),
            (Some("network"), None),
            (None, Some("connection timed out")),
            (None, Some("request was cancelled")),
        ] {
            assert!(
                ParallelExecutionServiceImpl::is_lost_outcome(&result(false, failure_type, error)),
                "{failure_type:?}/{error:?} must be ambiguous"
            );
        }
    }
}

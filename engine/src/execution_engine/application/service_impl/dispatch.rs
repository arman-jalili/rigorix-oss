//! Dispatch loop + sequence-policy dispatch helpers for
//! `ParallelExecutionServiceImpl` (extracted from `service_impl.rs`, #914).

use super::*;

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
        let scope_baseline: Option<ChangeSnapshot> = match &config.approval_repo_path {
            Some(repo) => GitDiffEffectOracle
                .snapshot(std::path::Path::new(repo))
                .ok(),
            None => None,
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

            // ADR-011 choke point: verify the approved intent before dispatch.
            if graph
                .get_node(node_id)
                .map(|n| n.requires_approval)
                .unwrap_or(false)
            {
                match self.verify_before_dispatch(node_id).await {
                    Ok((true, _)) => {}
                    Ok((false, digests)) => {
                        if let Some((expected, actual)) = digests {
                            let step_name = graph
                                .get_node(node_id)
                                .map(|n| n.name.clone())
                                .unwrap_or_default();
                            self.publish_event(ExecutionEvent::IntentMismatchDetected {
                                execution_id: dag_id,
                                node_id: node_id.to_string(),
                                step_name,
                                expected,
                                actual,
                                timestamp: chrono::Utc::now(),
                            })
                            .await;
                        }
                        self.halt_intent_mismatch(dag_id, graph, node_id, total_nodes)
                            .await?;
                        approval_blocked = true;
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }

            // R1 dispatch-time precondition gate (ADR-017): after ADR-011
            // approval verification, before the R3 sequence-policy gate.
            match self.precondition_verdict(dag_id, graph, node_id).await? {
                PreconditionVerdict::Dispatch => {}
                PreconditionVerdict::Deny {
                    precondition_id,
                    outcome,
                } => {
                    let node_name = graph
                        .get_node(node_id)
                        .map(|n| n.name.clone())
                        .unwrap_or_default();
                    let task_result = TaskResult::failure(
                        node_id,
                        &node_name,
                        format!(
                            "Precondition '{precondition_id}' refused step before dispatch ({})",
                            outcome.as_str()
                        ),
                        "precondition_denied".to_string(),
                        0,
                        0,
                    );
                    node_results.insert(node_id, task_result);
                    failed_count += 1;
                    if let Some((denied_id, state)) =
                        self.record_precondition_denial(dag_id, node_id, &precondition_id, outcome)
                    {
                        self.notify_progress(dag_id, denied_id, state, total_nodes);
                    }
                    // Release dependents exactly like a dispatched failure.
                    let _ = graph.mark_completed(node_id);
                    // NEVER fall through to dispatch: a denied node's tool is
                    // never called. Continue the fill loop.
                    continue;
                }
            }

            // R3 sequence-policy prefix gate (opt-in; module doc §R3).
            // Promotion flips the live node's requires_approval flag so the
            // SAME approval pause/approve/resume chain as a pre-declared
            // gated step applies; denial records a deterministic failure
            // before the node's tool is ever called.
            match self
                .sequence_policy_verdict(dag_id, graph, node_id, approved)
                .await?
            {
                SequencePolicyVerdict::Dispatch => {}
                SequencePolicyVerdict::Promote => {
                    if let Some(node) = graph.get_node_mut(node_id) {
                        node.requires_approval = true;
                    }
                    graph.requeue_ready_node(node_id);
                    approval_blocked = true;
                    self.mark_awaiting_approval(dag_id, graph, total_nodes)
                        .await?;
                    break;
                }
                SequencePolicyVerdict::Deny {
                    rule_id,
                    later_step,
                } => {
                    let node_name = graph
                        .get_node(node_id)
                        .map(|n| n.name.clone())
                        .unwrap_or_default();
                    let task_result = TaskResult::failure(
                        node_id,
                        &node_name,
                        format!(
                            "Sequence policy denied by rule '{rule_id}' — step '{later_step}' must not dispatch"
                        ),
                        "sequence_policy_denied".to_string(),
                        0,
                        0,
                    );
                    node_results.insert(node_id, task_result);
                    failed_count += 1;
                    // Mirror the consumption-loop state update so observers
                    // see the denial like any deterministic node failure.
                    if let Some((denied_id, state)) =
                        self.record_sequence_denial(dag_id, node_id, &rule_id, &later_step)
                    {
                        self.notify_progress(dag_id, denied_id, state, total_nodes);
                    }
                    // Release dependents exactly like a dispatched failure.
                    let _ = graph.mark_completed(node_id);
                    // NEVER fall through to dispatch: a denied node's tool is
                    // never called. Continue the fill loop.
                    continue;
                }
            }

            spawn_concurrent_node(
                &mut join_set,
                graph,
                event_bus,
                &self.sessions,
                dag_id,
                node_id,
                &self.permission_enforcer,
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

            // Mark completed in graph to release dependents
            let _ = graph.mark_completed(node_id);

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

                // ADR-011 choke point: verify the approved intent before dispatch.
                if graph
                    .get_node(next_id)
                    .map(|n| n.requires_approval)
                    .unwrap_or(false)
                {
                    match self.verify_before_dispatch(next_id).await {
                        Ok((true, _)) => {}
                        Ok((false, digests)) => {
                            if let Some((expected, actual)) = digests {
                                let step_name = graph
                                    .get_node(next_id)
                                    .map(|n| n.name.clone())
                                    .unwrap_or_default();
                                self.publish_event(ExecutionEvent::IntentMismatchDetected {
                                    execution_id: dag_id,
                                    node_id: next_id.to_string(),
                                    step_name,
                                    expected,
                                    actual,
                                    timestamp: chrono::Utc::now(),
                                })
                                .await;
                            }
                            self.halt_intent_mismatch(dag_id, graph, next_id, total_nodes)
                                .await?;
                            approval_blocked = true;
                            break;
                        }
                        Err(e) => return Err(e),
                    }
                }

                // R1 dispatch-time precondition gate (ADR-017).
                match self.precondition_verdict(dag_id, graph, next_id).await? {
                    PreconditionVerdict::Dispatch => {}
                    PreconditionVerdict::Deny {
                        precondition_id,
                        outcome,
                    } => {
                        let node_name = graph
                            .get_node(next_id)
                            .map(|n| n.name.clone())
                            .unwrap_or_default();
                        let task_result = TaskResult::failure(
                            next_id,
                            &node_name,
                            format!(
                                "Precondition '{precondition_id}' refused step before dispatch ({})",
                                outcome.as_str()
                            ),
                            "precondition_denied".to_string(),
                            0,
                            0,
                        );
                        node_results.insert(next_id, task_result);
                        failed_count += 1;
                        if let Some((denied_id, state)) = self.record_precondition_denial(
                            dag_id,
                            next_id,
                            &precondition_id,
                            outcome,
                        ) {
                            self.notify_progress(dag_id, denied_id, state, total_nodes);
                        }
                        let _ = graph.mark_completed(next_id);
                        // NEVER fall through to dispatch (tool never called).
                        continue;
                    }
                }

                // R3 sequence-policy prefix gate (opt-in; module doc §R3).
                // Same semantics as the Phase-1 gate above.
                match self
                    .sequence_policy_verdict(dag_id, graph, next_id, approved)
                    .await?
                {
                    SequencePolicyVerdict::Dispatch => {}
                    SequencePolicyVerdict::Promote => {
                        if let Some(node) = graph.get_node_mut(next_id) {
                            node.requires_approval = true;
                        }
                        graph.requeue_ready_node(next_id);
                        approval_blocked = true;
                        self.mark_awaiting_approval(dag_id, graph, total_nodes)
                            .await?;
                        break;
                    }
                    SequencePolicyVerdict::Deny {
                        rule_id,
                        later_step,
                    } => {
                        let node_name = graph
                            .get_node(next_id)
                            .map(|n| n.name.clone())
                            .unwrap_or_default();
                        let task_result = TaskResult::failure(
                            next_id,
                            &node_name,
                            format!(
                                "Sequence policy denied by rule '{rule_id}' — step '{later_step}' must not dispatch"
                            ),
                            "sequence_policy_denied".to_string(),
                            0,
                            0,
                        );
                        node_results.insert(next_id, task_result);
                        failed_count += 1;
                        if let Some((denied_id, state)) =
                            self.record_sequence_denial(dag_id, next_id, &rule_id, &later_step)
                        {
                            self.notify_progress(dag_id, denied_id, state, total_nodes);
                        }
                        let _ = graph.mark_completed(next_id);
                        // NEVER fall through to dispatch (tool never called).
                        continue;
                    }
                }

                spawn_concurrent_node(
                    &mut join_set,
                    graph,
                    event_bus,
                    &self.sessions,
                    dag_id,
                    next_id,
                    &self.permission_enforcer,
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

    /// R1 dispatch-time precondition verdict for one ready node (ADR-017).
    ///
    /// Returns `Dispatch` when no gate is wired or the gate allows dispatch.
    /// A `Deny` verdict means the tool must never be called. An unarmed gate
    /// (`NotArmed`) is a fail-closed refusal mapped to `Deny { error }`; any
    /// other gate error halts the run fail-closed (mirrors sequence policy).
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
        match gate.assess(dag_id, &step).await {
            Ok(verdict) => Ok(verdict),
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
}

//! The **single** consequence-gate sequence for a node (#1074).
//!
//! @canonical .pi/architecture/modules/execution-engine.md#consequence-gates-one-sequence-two-routes-1074
//! `execute_graph` (via `run_dispatch_loop`) and the public `execute_node`
//! (via `execute_node_flow`) used to answer differently for the same tool: the
//! graph route ran approval binding, precondition, sequence and capacity gates;
//! the single-node route ran none of them. The difference was invisible in the
//! signature.
//!
//! This module owns the gate order once:
//!
//! 1. ADR-011 approval binding (only for `requires_approval` nodes)
//! 2. ADR-017 precondition (authoritative state re-read at the moment of
//!    consequence)
//! 3. ADR-015/ADR-013 sequence policy (prefix, companion prerequisite)
//! 4. ADR-018 capacity reservation
//!
//! [`ParallelExecutionServiceImpl::gate_node`] computes the verdict;
//! [`ParallelExecutionServiceImpl::apply_gate`] performs the dispatch-loop
//! bookkeeping (record the deterministic failure, release dependents, or pause).
//! [`ParallelExecutionServiceImpl::execute_node_flow`] calls `gate_node` and
//! turns a refusal into a failed node result — it has no session to pause, so a
//! `Promote`/`HaltIntent` is reported as `approval_required`, never dispatched.
//!
//! Duplicating this sequence in either caller is the drift the gate exists to
//! prevent.

use super::dispatch::CapacityVerdict;
use super::*;

/// What the gate sequence decided for one node.
pub(super) enum NodeGate {
    /// Every gate passed — the node may dispatch.
    Dispatch,
    /// A sequence rule with `promote` semantics matched and the node has not
    /// been approved: route into the approval pause.
    Promote,
    /// The approved intent no longer matches (or there is no record): halt.
    HaltIntent {
        /// `(expected, actual)` digests when the verdict was a mismatch.
        digests: Option<(String, String)>,
    },
    /// A gate refused the node. Its tool must never be called.
    Refuse {
        /// Deterministic failure type recorded on the node
        /// (e.g. `precondition_denied`).
        failure_type: String,
        /// Redacted operator-facing message.
        message: String,
        /// Bookkeeping the dispatch loop performs for this refusal.
        record: DenialRecord,
    },
}

/// Which denial bookkeeping a refusal needs.
pub(super) enum DenialRecord {
    /// No state update beyond the failure (capacity refusals log only).
    None,
    /// `record_precondition_denial` (ADR-017 evidence).
    Precondition {
        precondition_id: String,
        outcome: crate::precondition::domain::PreconditionOutcome,
    },
    /// `record_sequence_denial` (ADR-013 R6 evidence).
    Sequence { rule_id: String, later_step: String },
}

/// Outcome of applying a gate inside a dispatch loop.
pub(super) enum GateOutcome {
    /// Dispatch unchanged.
    Dispatch,
    /// The loop must pause (approval) and stop filling.
    Paused,
    /// The node was refused and recorded; release its dependents and continue.
    Refused,
}

impl ParallelExecutionServiceImpl {
    /// Run the full consequence-gate sequence for `node_id`.
    ///
    /// Order is load-bearing: approval → precondition → sequence → capacity.
    /// Capacity is last because a reservation must be taken at the last possible
    /// moment, and a refused node must never hold capacity.
    ///
    /// # Errors
    /// Propagates fail-closed evaluation errors (a corrupt gate config halts the
    /// run rather than dispatching ungated).
    pub(super) async fn gate_node(
        &self,
        dag_id: Uuid,
        graph: &crate::dag_engine::domain::TaskGraph,
        node_id: Uuid,
        approved: &HashSet<Uuid>,
    ) -> Result<NodeGate, ExecutionError> {
        // ── 1. ADR-011 approval binding ──
        if graph
            .get_node(node_id)
            .map(|n| n.requires_approval)
            .unwrap_or(false)
        {
            match self.verify_before_dispatch(node_id).await {
                Ok((true, _)) => {}
                Ok((false, digests)) => {
                    if let Some((expected, actual)) = &digests {
                        let step_name = graph
                            .get_node(node_id)
                            .map(|n| n.name.clone())
                            .unwrap_or_default();
                        self.publish_event(ExecutionEvent::IntentMismatchDetected {
                            execution_id: dag_id,
                            node_id: node_id.to_string(),
                            step_name,
                            expected: expected.clone(),
                            actual: actual.clone(),
                            timestamp: chrono::Utc::now(),
                        })
                        .await;
                    }
                    return Ok(NodeGate::HaltIntent { digests });
                }
                Err(e) => return Err(e),
            }
        }

        // ── 2. ADR-017 precondition ──
        match self.precondition_verdict(dag_id, graph, node_id).await? {
            PreconditionVerdict::Dispatch => {}
            PreconditionVerdict::Deny {
                precondition_id,
                outcome,
            } => {
                return Ok(NodeGate::Refuse {
                    failure_type: "precondition_denied".to_string(),
                    message: format!(
                        "Precondition '{precondition_id}' refused step before dispatch ({})",
                        outcome.as_str()
                    ),
                    record: DenialRecord::Precondition {
                        precondition_id,
                        outcome,
                    },
                });
            }
        }

        // ── 3. ADR-013/ADR-015 sequence policy ──
        match self
            .sequence_policy_verdict(dag_id, graph, node_id, approved)
            .await?
        {
            SequencePolicyVerdict::Dispatch => {}
            SequencePolicyVerdict::Promote => return Ok(NodeGate::Promote),
            SequencePolicyVerdict::Deny {
                rule_id,
                later_step,
            } => {
                return Ok(NodeGate::Refuse {
                    failure_type: "sequence_policy_denied".to_string(),
                    message: format!(
                        "Sequence policy denied by rule '{rule_id}' — step '{later_step}' must not dispatch"
                    ),
                    record: DenialRecord::Sequence {
                        rule_id,
                        later_step,
                    },
                });
            }
            SequencePolicyVerdict::CompanionUnmet {
                obligation_id,
                required_companion,
            } => {
                return Ok(NodeGate::Refuse {
                    failure_type: "companion_prerequisite_unmet".to_string(),
                    message: format!(
                        "Companion prerequisite '{obligation_id}' unmet — '{required_companion}' must complete successfully before this step dispatches"
                    ),
                    record: DenialRecord::None,
                });
            }
        }

        // ── 4. ADR-018 capacity ──
        match self.capacity_verdict(dag_id, graph, node_id).await? {
            CapacityVerdict::Dispatch => {}
            CapacityVerdict::Deny {
                resource,
                amount,
                detail,
                reason,
            } => {
                tracing::warn!(
                    %dag_id,
                    %node_id,
                    %resource,
                    amount,
                    reason,
                    "capacity: step refused before dispatch (fail closed)"
                );
                return Ok(NodeGate::Refuse {
                    failure_type: "capacity_denied".to_string(),
                    message: format!("Capacity refused step before dispatch: {detail}"),
                    record: DenialRecord::None,
                });
            }
        }

        // ── 5. ADR-019 effect write-ahead ──
        //
        // The fifth consequence gate, and the one #1185 found outside this
        // function: it used to run only in the graph loops, so `execute_node`
        // dispatched the same tool with no record and no fence. It runs last,
        // after capacity, so a refused node holds neither a reservation nor an
        // effect record. Both routes call this function.
        self.begin_effect(dag_id, graph, node_id).await?;

        Ok(NodeGate::Dispatch)
    }

    /// Apply a gate verdict inside a dispatch loop: record refusals, release
    /// dependents, or pause for approval. Returns whether to dispatch, pause or
    /// continue.
    ///
    /// Both fill loops (`Phase 1` and `Phase 3`) share this so their bookkeeping
    /// cannot diverge.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn apply_gate(
        &self,
        dag_id: Uuid,
        graph: &mut crate::dag_engine::domain::TaskGraph,
        node_id: Uuid,
        approved: &HashSet<Uuid>,
        total_nodes: u32,
        node_results: &mut HashMap<Uuid, TaskResult>,
        failed_count: &mut u32,
    ) -> Result<GateOutcome, ExecutionError> {
        match self.gate_node(dag_id, graph, node_id, approved).await? {
            NodeGate::Dispatch => Ok(GateOutcome::Dispatch),
            NodeGate::Promote => {
                if let Some(node) = graph.get_node_mut(node_id) {
                    node.requires_approval = true;
                }
                graph.requeue_ready_node(node_id);
                self.mark_awaiting_approval(dag_id, graph, total_nodes)
                    .await?;
                Ok(GateOutcome::Paused)
            }
            NodeGate::HaltIntent { digests } => {
                let _ = digests; // already published by `gate_node`
                self.halt_intent_mismatch(dag_id, graph, node_id, total_nodes)
                    .await?;
                Ok(GateOutcome::Paused)
            }
            NodeGate::Refuse {
                failure_type,
                message,
                record,
            } => {
                let node_name = graph
                    .get_node(node_id)
                    .map(|n| n.name.clone())
                    .unwrap_or_default();
                node_results.insert(
                    node_id,
                    TaskResult::failure(node_id, &node_name, message, failure_type, 0, 0),
                );
                *failed_count += 1;
                match record {
                    DenialRecord::Precondition {
                        precondition_id,
                        outcome,
                    } => {
                        if let Some((denied_id, state)) = self.record_precondition_denial(
                            dag_id,
                            node_id,
                            &precondition_id,
                            outcome,
                        ) {
                            self.notify_progress(dag_id, denied_id, state, total_nodes);
                        }
                    }
                    DenialRecord::Sequence {
                        rule_id,
                        later_step,
                    } => {
                        if let Some((denied_id, state)) =
                            self.record_sequence_denial(dag_id, node_id, &rule_id, &later_step)
                        {
                            self.notify_progress(dag_id, denied_id, state, total_nodes);
                        }
                    }
                    DenialRecord::None => {}
                }
                self.release_or_skip_dependents(graph, dag_id, node_id, total_nodes, true)
                    .await;
                Ok(GateOutcome::Refused)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag_engine::domain::{TaskGraph, TaskNode};

    /// #1074: the gate sequence is one function, so a route that calls it
    /// cannot skip a gate. This pins the capacity arm — the arm that
    /// `execute_node` previously never reached — through the shared entry point.
    /// #1185: the write-ahead is the fifth gate and lives in the shared path,
    /// so `execute_node` cannot dispatch a tool without a record. The settle is
    /// the same call both routes make from their terminal outcome.
    #[tokio::test]
    async fn the_shared_gate_writes_and_settles_the_effect_record() {
        use crate::effect_commitment::application::{
            EffectCommitmentSeam, EffectCommitmentServiceImpl, Sha256IntentKeyDerivation,
        };
        use crate::effect_commitment::domain::{EffectQuery, EffectState};
        use crate::effect_commitment::infrastructure::InMemoryEffectLedger;
        use crate::effect_commitment::interfaces::EffectCommitmentLedger;

        let ledger = Arc::new(InMemoryEffectLedger::new());
        let service = Arc::new(EffectCommitmentServiceImpl::new(
            ledger.clone(),
            Arc::new(Sha256IntentKeyDerivation::new()),
        ));
        let svc = ParallelExecutionServiceImpl::new(
            crate::execution_engine::domain::ParallelExecutorConfig::default(),
            Box::new(super::super::RetryEvaluationServiceImpl::default()),
            std::sync::Arc::new(
                crate::event_system::application::event_bus_service_impl::EventBusServiceImpl::default(),
            ),
        )
        .with_effect_commitment_seam(Arc::new(EffectCommitmentSeam::new(service)));

        let dag_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();
        let mut graph = TaskGraph::new();
        graph
            .add_unchecked(TaskNode::new(
                node_id,
                "step",
                "run_command",
                vec![],
                r#"{"command":"true"}"#.to_string(),
            ))
            .expect("add node");
        graph.seal().expect("seal");

        let verdict = svc
            .gate_node(dag_id, &graph, node_id, &HashSet::new())
            .await
            .expect("gate evaluation");
        assert!(matches!(verdict, NodeGate::Dispatch));
        let records = ledger.query(EffectQuery::by_dag(dag_id)).await.unwrap();
        assert_eq!(
            records.len(),
            1,
            "the gate writes one record before dispatch"
        );
        assert_eq!(records[0].state(), EffectState::Dispatched);

        // The single-node route's settle call, after its attempts.
        svc.settle_effect(
            node_id,
            &TaskResult::success(node_id, "step", Some("ok".to_string()), 0, 0),
        )
        .await;
        let records = ledger.query(EffectQuery::by_dag(dag_id)).await.unwrap();
        assert_eq!(records[0].state(), EffectState::Committed);
    }

    #[tokio::test]
    async fn the_shared_gate_sequence_refuses_a_step_over_the_capacity_cap() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let rigorix = dir.path().join(".rigorix");
        std::fs::create_dir_all(&rigorix).expect("create .rigorix");
        std::fs::write(
            rigorix.join("capacity.toml"),
            r#"
[[resources]]
id = "gate-budget"
hard_limit = 1
window_seconds = 3600

[[grants]]
step = "pay"
tool = "run_command"
resource = "gate-budget"
amount = 2
lease_seconds = 60
"#,
        )
        .expect("write capacity.toml");

        let setup = crate::capacity_ledger::CapacitySetup::from_env(dir.path())
            .await
            .expect("from_env")
            .expect("fixture present");
        assert!(setup.is_armed(), "the fixture must arm the capacity seam");

        let svc = ParallelExecutionServiceImpl::new(
            crate::execution_engine::domain::ParallelExecutorConfig::default(),
            Box::new(super::super::RetryEvaluationServiceImpl::default()),
            std::sync::Arc::new(
                crate::event_system::application::event_bus_service_impl::EventBusServiceImpl::default(),
            ),
        )
        .with_capacity_ledger(setup);

        let node_id = Uuid::new_v4();
        let mut graph = TaskGraph::new();
        graph
            .add_unchecked(TaskNode::new(
                node_id,
                "pay",
                "run_command",
                vec![],
                r#"{"tool":"run_command","command":"true"}"#.to_string(),
            ))
            .expect("add node");
        graph.seal().expect("seal");

        let verdict = svc
            .gate_node(Uuid::new_v4(), &graph, node_id, &HashSet::new())
            .await
            .expect("gate evaluation");

        match verdict {
            NodeGate::Refuse { failure_type, .. } => {
                assert_eq!(
                    failure_type, "capacity_denied",
                    "the shared gate must apply the capacity arm"
                );
            }
            _ => panic!(
                "a step over the cap must be refused by the shared gate; \
                 otherwise execute_node dispatches what execute_graph refuses"
            ),
        }
    }
}

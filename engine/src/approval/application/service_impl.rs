//! ApprovalService implementation.
//!
//! @canonical .pi/architecture/modules/approval.md#approvalservice
//! Implements: ISSUE-APPROVALSERVICE (#792) — R1 capture, R2 verify, R3
//!   consume, R5 scope-violation reporting
//!
//! Wires the frozen `ApprovalService` contract to an `ApprovalRepository`, a
//! `NodeIntentResolver` (the engine's sealed graph), and a
//! `ScopeViolationSink` (audit envelope wiring).
//!
//! # Lifecycle semantics implemented here
//! - `approve` (R1+R3): resolve each step name → canonical `ExecutionIntent`,
//!   compute `intent_hash` with the run key, persist a single-use
//!   `ApprovalRecord` (TTL = `now + ttl`)
//! - `verify_intent` (R2): re-derive the current intent and compare digests —
//!   `Matched` → dispatch; `Mismatched` → HALT, re-approval required;
//!   `Invalid` → expired/consumed/superseded never dispatch. A record that
//!   does not exist (never approved, or legacy pre-binding state) returns
//!   `NotFound` — the run must re-approve (migration rule)
//! - `consume` (R3): single-use on terminal outcome. Failed attempts do NOT
//!   consume (callers only consume on success/skipped/exhausted-failure)
//! - `record_scope_violation` (R5): forwards non-blocking evidence to the sink

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;

use crate::approval::domain::{
    ApprovalError, ApprovalRecord, ApprovalStatus, DecisionContext, ExecutionIntent, IntentHash,
    ScopeViolation,
};
use crate::approval::infrastructure::repository::ApprovalRepository;

use super::dto::{ApproveInput, ApproveOutput};
use super::service::{ApprovalService, IntentVerification, NodeIntentResolver, ScopeViolationSink};

/// Concrete `ApprovalService` over a repository + intent resolver.
pub struct ApprovalServiceImpl {
    repo: Arc<dyn ApprovalRepository>,
    resolver: Arc<dyn NodeIntentResolver>,
    sink: Arc<dyn ScopeViolationSink>,
    run_key: Vec<u8>,
    /// Approval lifetime — `expires_at = decided_at + ttl`.
    ttl: Duration,
}

impl ApprovalServiceImpl {
    /// Build the service.
    pub fn new(
        repo: Arc<dyn ApprovalRepository>,
        resolver: Arc<dyn NodeIntentResolver>,
        run_key: Vec<u8>,
        ttl: Duration,
    ) -> Self {
        Self {
            repo,
            resolver,
            sink: Arc::new(()),
            run_key,
            ttl,
        }
    }

    /// Build the service with a scope-violation sink (audit envelope wiring).
    pub fn with_scope_violation_sink(mut self, sink: Arc<dyn ScopeViolationSink>) -> Self {
        self.sink = sink;
        self
    }

    fn digest(&self, intent: &ExecutionIntent) -> IntentHash {
        IntentHash::compute(intent, &self.run_key)
    }

    fn build_record(
        &self,
        resolved: &crate::approval::application::service::ResolvedNode,
        input: &ApproveInput,
        now: chrono::DateTime<Utc>,
    ) -> Result<ApprovalRecord, ApprovalError> {
        // #1190: an approver may add scope, but never remove what the planner
        // declared. Merge before hashing: the signature covers the value the
        // oracle will compare against.
        let mut intent = resolved.intent.clone();
        if let Some(scope) = &input.declared_scope {
            intent.declared_scope.extend(scope.iter().cloned());
        }
        intent.declared_scope.sort();
        intent.declared_scope.dedup();
        if input.strict_scope == Some(true) && intent.declared_scope.is_empty() {
            return Err(ApprovalError::InvalidScope(
                "strict scope mode requires a declared scope, and neither the plan nor the approver supplied one"
                    .into(),
            ));
        }
        let decision_context = match input.decision_context.clone() {
            Some(mut ctx) => {
                // R4: derive the envelope-safe summary when the caller did not
                // pre-compute one — redacted, full_payload excluded by default.
                if ctx.summary.trim().is_empty() {
                    ctx.summary = ctx.summarize();
                }
                ctx
            }
            None => {
                // Degraded context (R4): summary only — evidence unavailable.
                DecisionContext {
                    rendered_step: "(degraded)".into(),
                    upstream_evidence: None,
                    state_snapshot: None,
                    summary: resolved.step_name.clone(),
                    full_payload: None,
                }
            }
        };
        Ok(ApprovalRecord {
            step_name: resolved.step_name.clone(),
            node_id: resolved.node_id,
            intent_hash: self.digest(&intent),
            intent_payload: serde_json::to_value(&intent).unwrap_or(serde_json::Value::Null),
            approver_id: input.approver_id.clone(),
            authority: input.authority.clone(),
            decided_at: now,
            expires_at: now + chrono::Duration::from_std(self.ttl).unwrap_or_default(),
            nonce: Uuid::new_v4(),
            token_claims_ref: input.token_claims_ref.clone(),
            status: ApprovalStatus::Pending,
            decision_context,
            approver_scope: input.declared_scope.clone().unwrap_or_default(),
            mac: None,
        })
    }
}

#[async_trait::async_trait]
impl ApprovalService for ApprovalServiceImpl {
    #[tracing::instrument(skip_all, name = "approval.approve")]
    async fn approve(&self, input: ApproveInput) -> Result<ApproveOutput, ApprovalError> {
        let now = Utc::now();
        let mut approved = Vec::new();
        let mut not_found = Vec::new();
        let mut approval_records = Vec::new();

        for step_name in &input.step_names {
            let Some(resolved) = self.resolver.resolve_by_step_name(step_name).await else {
                not_found.push(step_name.clone());
                continue;
            };
            let mut record = self.build_record(&resolved, &input, now)?;
            record.seal(&self.run_key)?;
            self.repo.save(&record).await?;
            tracing::info!(
                step_name = %record.step_name,
                node_id = %record.node_id,
                approver_id = %record.approver_id,
                expires_at = %record.expires_at,
                "approval recorded (intent hash bound, single-use, TTL set)"
            );
            approved.push(step_name.clone());
            approval_records.push(record);
        }
        tracing::info!(
            requested = input.step_names.len(),
            approved = approved.len(),
            not_found = not_found.len(),
            "approve complete"
        );

        Ok(ApproveOutput {
            dag_id: input.dag_id,
            approved,
            not_found,
            // The resolver supplies only approvable nodes; nodes still in an
            // engine-side `AwaitingApproval` queue are resolved by the caller.
            still_pending: Vec::new(),
            approval_records,
        })
    }

    #[tracing::instrument(skip_all, name = "approval.verify_intent", fields(node_id = %node_id))]
    async fn verify_intent(&self, node_id: Uuid) -> Result<IntentVerification, ApprovalError> {
        let now = Utc::now();
        let loaded = match self.repo.load(node_id).await {
            Ok(loaded) => loaded,
            Err(ApprovalError::RecordIntegrity { node_id, step_name }) => {
                tracing::error!(
                    node_id = %node_id,
                    step_name = %step_name,
                    "verify: approval record failed integrity verification — refusing dispatch"
                );
                return Ok(IntentVerification::IntegrityFailure { node_id, step_name });
            }
            Err(ApprovalError::RecordRollback { node_id, step_name }) => {
                tracing::error!(
                    node_id = %node_id,
                    step_name = %step_name,
                    "verify: approval store is older than recorded consumption — refusing dispatch"
                );
                return Ok(IntentVerification::IntegrityFailure { node_id, step_name });
            }
            Err(ApprovalError::UncoveredTerminalRecord {
                node_id, step_name, ..
            }) => {
                tracing::error!(
                    node_id = %node_id,
                    step_name = %step_name,
                    "verify: terminal approval has no consumption marker — refusing dispatch"
                );
                return Ok(IntentVerification::IntegrityFailure { node_id, step_name });
            }
            Err(error) => return Err(error),
        };
        let Some(mut record) = loaded else {
            // No record: never approved (or legacy pre-binding state that was
            // invalidated on hydrate) → re-approval required.
            tracing::warn!(node_id = %node_id, "verify: no approval record — re-approval required");
            return Err(ApprovalError::NotFound(node_id));
        };
        // The repository verifies a keyed store; the service verifies too, so
        // an unkeyed repository cannot turn a tampered record into `Matched`.
        if !record.verify_integrity(&self.run_key) {
            tracing::error!(
                node_id = %node_id,
                step_name = %record.step_name,
                "verify: approval record MAC invalid — refusing dispatch"
            );
            return Ok(IntentVerification::IntegrityFailure {
                node_id,
                step_name: record.step_name,
            });
        }

        // R3 TTL — enforced at verification time; expired never dispatches.
        if record.is_expired_at(now) {
            record.status = ApprovalStatus::Expired;
            record.seal(&self.run_key)?;
            self.repo.save(&record).await?;
            tracing::warn!(node_id = %node_id, "verify: approval expired (TTL) — no dispatch");
            return Ok(IntentVerification::Invalid(ApprovalStatus::Expired));
        }

        match record.status {
            ApprovalStatus::Pending => {
                let Some(resolved) = self.resolver.resolve_by_node_id(node_id).await else {
                    return Err(ApprovalError::NotFound(node_id));
                };
                // #1190: the approved scope may include the approver's
                // addition. Re-merge it into the current intent before hashing
                // so the comparison covers the same bytes the record signed;
                // a changed planner scope still mismatches.
                let mut current_intent = resolved.intent.clone();
                current_intent
                    .declared_scope
                    .extend(record.approver_scope.iter().cloned());
                current_intent.declared_scope.sort();
                current_intent.declared_scope.dedup();
                let current = self.digest(&current_intent);
                if current == record.intent_hash {
                    tracing::info!(node_id = %node_id, step_name = %resolved.step_name, "verify: MATCHED — dispatch");
                    Ok(IntentVerification::Matched)
                } else {
                    // R2 — HALT: the executing call no longer matches what was
                    // approved. Re-approval is the only recovery.
                    tracing::error!(
                        node_id = %node_id,
                        expected = %record.intent_hash.0,
                        actual = %current.0,
                        "verify: INTENT MISMATCH — HALT, re-approval required"
                    );
                    Ok(IntentVerification::Mismatched {
                        expected: record.intent_hash,
                        actual: current,
                    })
                }
            }
            // Consumed / superseded approvals never dispatch (single-use).
            other => {
                tracing::warn!(node_id = %node_id, status = ?other, "verify: INVALID — approval not dispatchable");
                Ok(IntentVerification::Invalid(other))
            }
        }
    }

    #[tracing::instrument(skip_all, name = "approval.consume", fields(node_id = %node_id))]
    async fn consume(&self, node_id: Uuid) -> Result<(), ApprovalError> {
        let mut record = self
            .repo
            .load(node_id)
            .await?
            .ok_or(ApprovalError::NotFound(node_id))?;
        if !record.verify_integrity(&self.run_key) {
            return Err(ApprovalError::RecordIntegrity {
                node_id: record.node_id,
                step_name: record.step_name,
            });
        }
        record.consume()?;
        record.seal(&self.run_key)?;
        self.repo.save(&record).await?;
        tracing::info!(node_id = %node_id, "approval consumed (single-use, terminal outcome)");
        Ok(())
    }

    #[tracing::instrument(skip_all, name = "approval.record_scope_violation", fields(node_id = %violation.node_id, out_of_scope_count = violation.out_of_scope.len()))]
    async fn record_scope_violation(&self, violation: ScopeViolation) -> Result<(), ApprovalError> {
        tracing::warn!(
            node_id = %violation.node_id,
            step_name = %violation.step_name,
            out_of_scope = ?violation.out_of_scope,
            "scope_violation recorded (non-blocking evidence)"
        );
        self.sink.record(&violation).await;
        Ok(())
    }

    async fn get_approval(&self, node_id: Uuid) -> Result<Option<ApprovalRecord>, ApprovalError> {
        let record = self.repo.load(node_id).await?;
        if let Some(record) = record.as_ref()
            && !record.verify_integrity(&self.run_key)
        {
            return Err(ApprovalError::RecordIntegrity {
                node_id: record.node_id,
                step_name: record.step_name.clone(),
            });
        }
        Ok(record)
    }
}

/// R3+AC #10 migration support — invalidate legacy approvals on hydrate.
///
/// Persisted pre-binding runs carry an `approved: Vec<Uuid>` set with **no
/// records** (no intent hash, no decision record). On hydrate those approvals
/// cannot be verified and must not authorize dispatch. Returns the subset of
/// `legacy_approved` node ids that have no stored record — those require
/// re-approval. Node ids that already have records are unaffected.
pub async fn legacy_approvals_requiring_reapproval(
    repo: &dyn ApprovalRepository,
    legacy_approved: &[Uuid],
) -> Result<Vec<Uuid>, ApprovalError> {
    let mut invalidated = Vec::new();
    for node_id in legacy_approved {
        if repo.load(*node_id).await?.is_none() {
            invalidated.push(*node_id);
        }
    }
    Ok(invalidated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_record_assigns_pending_and_ttl() {
        // Lightweight construction sanity (no async).
        let svc = ApprovalServiceImpl::new(
            Arc::new(
                crate::approval::infrastructure::repository::InMemoryApprovalRepository::new(),
            ),
            Arc::new(crate::approval::application::service_impl::tests::DummyResolver),
            b"key".to_vec(),
            Duration::from_secs(600),
        );
        let now = Utc::now();
        let record = svc
            .build_record(
                &crate::approval::application::service::ResolvedNode {
                    node_id: Uuid::new_v4(),
                    step_name: "s".into(),
                    intent: ExecutionIntent {
                        tool: "run_command".into(),
                        intent: serde_json::json!({"command": "echo hi"}),
                        declared_scope: vec![],
                    },
                },
                &ApproveInput {
                    dag_id: Uuid::new_v4(),
                    step_names: vec!["s".into()],
                    approver_id: "user".into(),
                    authority: None,
                    decision_context: None,
                    token_claims_ref: None,
                    declared_scope: None,
                    strict_scope: None,
                },
                now,
            )
            .expect("record");
        assert_eq!(record.status, ApprovalStatus::Pending);
        assert!(record.expires_at > record.decided_at);
        assert_eq!(record.approver_id, "user");
    }

    fn record_with(
        svc: &ApprovalServiceImpl,
        planner_scope: Vec<String>,
        input_scope: Option<Vec<String>>,
        strict: Option<bool>,
    ) -> Result<ApprovalRecord, ApprovalError> {
        svc.build_record(
            &crate::approval::application::service::ResolvedNode {
                node_id: Uuid::new_v4(),
                step_name: "s".into(),
                intent: ExecutionIntent {
                    tool: "run_command".into(),
                    intent: serde_json::json!({"command": "echo hi"}),
                    declared_scope: planner_scope,
                },
            },
            &ApproveInput {
                dag_id: Uuid::new_v4(),
                step_names: vec!["s".into()],
                approver_id: "user".into(),
                authority: None,
                decision_context: None,
                token_claims_ref: None,
                declared_scope: input_scope,
                strict_scope: strict,
            },
            Utc::now(),
        )
    }

    /// #1190: the approver's scope is merged, not substituted, and the signature
    /// covers the merged value the oracle will use.
    #[test]
    fn approver_scope_is_merged_into_the_hashed_intent() {
        let svc = ApprovalServiceImpl::new(
            Arc::new(
                crate::approval::infrastructure::repository::InMemoryApprovalRepository::new(),
            ),
            Arc::new(crate::approval::application::service_impl::tests::DummyResolver),
            b"key".to_vec(),
            Duration::from_secs(600),
        );
        let with_scope = record_with(&svc, vec!["docs/".into()], Some(vec!["src/".into()]), None)
            .expect("record");
        assert_eq!(
            with_scope.intent_payload["declared_scope"],
            serde_json::json!(["docs/", "src/"]),
            "planner scope must be kept and the approver's added"
        );
        assert_eq!(with_scope.approver_scope, vec!["src/".to_string()]);

        let without = record_with(&svc, vec!["docs/".into()], None, None).expect("record");
        assert_ne!(
            with_scope.intent_hash, without.intent_hash,
            "the hash must cover the approver-supplied scope"
        );
    }

    /// #1190: strict mode is opt-in and refuses, rather than warning, when
    /// neither producer supplied a scope.
    #[test]
    fn strict_scope_refuses_when_no_scope_is_declared() {
        let svc = ApprovalServiceImpl::new(
            Arc::new(
                crate::approval::infrastructure::repository::InMemoryApprovalRepository::new(),
            ),
            Arc::new(crate::approval::application::service_impl::tests::DummyResolver),
            b"key".to_vec(),
            Duration::from_secs(600),
        );
        let error = record_with(&svc, Vec::new(), None, Some(true)).expect_err("strict refusal");
        assert!(matches!(error, ApprovalError::InvalidScope(_)), "{error:?}");
    }

    #[test]
    fn a_traversal_entry_is_refused_before_hashing() {
        let mut input = ApproveInput {
            dag_id: Uuid::new_v4(),
            step_names: vec!["s".into()],
            approver_id: "user".into(),
            authority: None,
            decision_context: None,
            token_claims_ref: None,
            declared_scope: Some(vec!["../etc/passwd".into()]),
            strict_scope: None,
        };
        assert!(matches!(
            input.validate(),
            Err(ApprovalError::InvalidScope(_))
        ));
        input.declared_scope = Some(vec!["/etc/passwd".into()]);
        assert!(matches!(
            input.validate(),
            Err(ApprovalError::InvalidScope(_))
        ));
        input.declared_scope = Some(vec!["src/".into()]);
        assert!(input.validate().is_ok());
    }

    /// Dummy resolver used by the sanity test above (never resolves).
    pub struct DummyResolver;
    #[async_trait::async_trait]
    impl NodeIntentResolver for DummyResolver {
        async fn resolve_by_step_name(
            &self,
            _s: &str,
        ) -> Option<crate::approval::application::service::ResolvedNode> {
            None
        }
        async fn resolve_by_node_id(
            &self,
            _id: Uuid,
        ) -> Option<crate::approval::application::service::ResolvedNode> {
            None
        }
    }
}

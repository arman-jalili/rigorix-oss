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
    ) -> ApprovalRecord {
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
        ApprovalRecord {
            step_name: resolved.step_name.clone(),
            node_id: resolved.node_id,
            intent_hash: self.digest(&resolved.intent),
            intent_payload: serde_json::to_value(&resolved.intent)
                .unwrap_or(serde_json::Value::Null),
            approver_id: input.approver_id.clone(),
            authority: input.authority.clone(),
            decided_at: now,
            expires_at: now + chrono::Duration::from_std(self.ttl).unwrap_or_default(),
            nonce: Uuid::new_v4(),
            token_claims_ref: input.token_claims_ref.clone(),
            status: ApprovalStatus::Pending,
            decision_context,
        }
    }
}

#[async_trait::async_trait]
impl ApprovalService for ApprovalServiceImpl {
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
            let record = self.build_record(&resolved, &input, now);
            self.repo.save(&record).await?;
            approved.push(step_name.clone());
            approval_records.push(record);
        }

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

    async fn verify_intent(&self, node_id: Uuid) -> Result<IntentVerification, ApprovalError> {
        let now = Utc::now();
        let Some(mut record) = self.repo.load(node_id).await? else {
            // No record: never approved (or legacy pre-binding state that was
            // invalidated on hydrate) → re-approval required.
            return Err(ApprovalError::NotFound(node_id));
        };

        // R3 TTL — enforced at verification time; expired never dispatches.
        if record.is_expired_at(now) {
            record.status = ApprovalStatus::Expired;
            self.repo.save(&record).await?;
            return Ok(IntentVerification::Invalid(ApprovalStatus::Expired));
        }

        match record.status {
            ApprovalStatus::Pending => {
                let Some(resolved) = self.resolver.resolve_by_node_id(node_id).await else {
                    return Err(ApprovalError::NotFound(node_id));
                };
                let current = self.digest(&resolved.intent);
                if current == record.intent_hash {
                    Ok(IntentVerification::Matched)
                } else {
                    // R2 — HALT: the executing call no longer matches what was
                    // approved. Re-approval is the only recovery.
                    Ok(IntentVerification::Mismatched {
                        expected: record.intent_hash,
                        actual: current,
                    })
                }
            }
            // Consumed / superseded approvals never dispatch (single-use).
            other => Ok(IntentVerification::Invalid(other)),
        }
    }

    async fn consume(&self, node_id: Uuid) -> Result<(), ApprovalError> {
        let mut record = self
            .repo
            .load(node_id)
            .await?
            .ok_or(ApprovalError::NotFound(node_id))?;
        record.consume()?;
        self.repo.save(&record).await
    }

    async fn record_scope_violation(&self, violation: ScopeViolation) -> Result<(), ApprovalError> {
        self.sink.record(&violation).await;
        Ok(())
    }

    async fn get_approval(&self, node_id: Uuid) -> Result<Option<ApprovalRecord>, ApprovalError> {
        self.repo.load(node_id).await
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
        let record = svc.build_record(
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
            },
            now,
        );
        assert_eq!(record.status, ApprovalStatus::Pending);
        assert!(record.expires_at > record.decided_at);
        assert_eq!(record.approver_id, "user");
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

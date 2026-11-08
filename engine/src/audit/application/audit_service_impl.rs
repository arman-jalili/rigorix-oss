//! Implementation of `AuditService`.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: AuditService trait — orchestrates envelope build and delivery
//! Issue: #14
//!
//! Orchestrates the full audit workflow: building envelopes from execution
//! events, delivering them via `AuditSender`, and managing failed deliveries
//! via `AuditQueue`.

use async_trait::async_trait;
use std::sync::Arc;

use crate::audit::domain::{AuditError, HistoryPolicy};

#[cfg(not(feature = "portable"))]
use super::audit_queue_impl::AuditQueueImpl;
#[cfg(not(feature = "portable"))]
use super::audit_sender_impl::AuditSenderImpl;
use super::dto::{
    BuildEnvelopeInput, BuildEnvelopeOutput, DegradedEvidence, DeliverEnvelopeInput, EnqueueInput,
    EnqueueOutput,
};
#[cfg(not(feature = "portable"))]
use super::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use super::factory::AuditEnvelopeFactory;
use super::service::RetryPendingOutput;
use super::service::{AuditQueue, AuditSender, AuditService, AuditStatusOutput};

/// ADR-016: environment variable naming the local chain producer
/// (default `local`).
pub const AUDIT_PRODUCER_ID_ENV: &str = "RIGORIX_AUDIT_PRODUCER_ID";

/// ADR-016: environment variable recording the history-policy opt-in.
pub const HISTORY_POLICY_ENV: &str = "RIGORIX_HISTORY_POLICY";

/// The only recorded opt-in value — `allow_unanchored`.
pub const HISTORY_POLICY_ALLOW_UNANCHORED: &str = "allow_unanchored";

/// Resolve the local chain producer id (`RIGORIX_AUDIT_PRODUCER_ID`,
/// default `local`). Shared with the sequence-policy composition root.
pub fn resolve_producer_id() -> String {
    std::env::var(AUDIT_PRODUCER_ID_ENV)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "local".to_string())
}

/// Resolve the recorded history-policy opt-in
/// (`RIGORIX_HISTORY_POLICY=allow_unanchored`). Any other value is ignored
/// (the fail-closed default stands). Shared with the sequence-policy
/// composition root so the guard's behavior and the recorded envelope field
/// cannot diverge.
pub fn resolve_history_policy() -> Option<HistoryPolicy> {
    match std::env::var(HISTORY_POLICY_ENV) {
        Ok(v)
            if v.trim()
                .eq_ignore_ascii_case(HISTORY_POLICY_ALLOW_UNANCHORED) =>
        {
            Some(HistoryPolicy::AllowUnanchored)
        }
        _ => None,
    }
}

/// Implementation of `AuditService`.
///
/// Coordinates envelope building, sending, and failed delivery retries.
pub struct AuditServiceImpl {
    /// Factory for building envelopes.
    envelope_factory: Box<dyn AuditEnvelopeFactory>,
    /// Sender for delivering envelopes.
    sender: Arc<dyn AuditSender>,
    /// Queue for failed deliveries.
    queue: Box<dyn AuditQueue>,
    /// Whether audit is enabled.
    enabled: bool,
    /// R7 / local read-back: optional durable envelope store (e.g.
    /// `.rigorix/audit`). When set, every built envelope is ALSO persisted
    /// locally — the signed trail that cross-run policy (sequence-policy R7)
    /// and offline read-back consume. `None` = remote-send only (status quo).
    local_repository: Option<
        std::sync::Arc<dyn crate::audit::infrastructure::repository::AuditEnvelopeRepository>,
    >,
    /// Default retry configuration.
    max_retries: u32,
    backoff_base_secs: u64,
    backoff_max_secs: u64,
    /// ADR-016: producer id this local store chains under
    /// (`RIGORIX_AUDIT_PRODUCER_ID`, default `local`).
    producer_id: String,
    /// ADR-016: the recorded `allow_unanchored` opt-in, if configured
    /// (`RIGORIX_HISTORY_POLICY=allow_unanchored`). Signed into every envelope.
    history_policy: Option<HistoryPolicy>,
    /// ADR-016: serializes chain-link + save within this process so two
    /// concurrent runs cannot claim the same sequence number. (A cross-process
    /// lock is a Phase B anchor concern.)
    chain_lock: tokio::sync::Mutex<()>,
}

impl AuditServiceImpl {
    /// Create a new audit service.
    pub fn new(
        envelope_factory: Box<dyn AuditEnvelopeFactory>,
        sender: Arc<dyn AuditSender>,
        queue: Box<dyn AuditQueue>,
        enabled: bool,
    ) -> Self {
        Self {
            envelope_factory,
            sender,
            queue,
            enabled,
            max_retries: 3,
            backoff_base_secs: 1,
            backoff_max_secs: 60,
            local_repository: None,
            producer_id: resolve_producer_id(),
            history_policy: resolve_history_policy(),
            chain_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Attach a durable local envelope store (R7 cross-run policy input +
    /// offline read-back). Save failures are logged, never fatal.
    pub fn with_local_repository(
        mut self,
        repo: std::sync::Arc<dyn crate::audit::infrastructure::repository::AuditEnvelopeRepository>,
    ) -> Self {
        self.local_repository = Some(repo);
        self
    }

    /// ADR-016: override the local chain producer id (defaults to
    /// `RIGORIX_AUDIT_PRODUCER_ID` or `local`).
    pub fn with_producer_id(mut self, producer_id: impl Into<String>) -> Self {
        self.producer_id = producer_id.into();
        self
    }

    /// Create a new audit service with custom retry config.
    pub fn with_retry_config(
        envelope_factory: Box<dyn AuditEnvelopeFactory>,
        sender: Arc<dyn AuditSender>,
        queue: Box<dyn AuditQueue>,
        enabled: bool,
        max_retries: u32,
        backoff_base_secs: u64,
        backoff_max_secs: u64,
    ) -> Self {
        Self {
            envelope_factory,
            sender,
            queue,
            enabled,
            max_retries,
            backoff_base_secs,
            backoff_max_secs,
            local_repository: None,
            producer_id: resolve_producer_id(),
            history_policy: resolve_history_policy(),
            chain_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Create a default audit service with in-memory queue and no sender.
    ///
    /// Useful for testing.
    #[cfg(not(feature = "portable"))]
    pub fn default_test() -> Self {
        Self::new(
            Box::new(AuditEnvelopeFactoryImpl::default()),
            Arc::new(AuditSenderImpl::new(None, None)),
            Box::new(AuditQueueImpl::default()),
            true,
        )
    }
}

/// The fact a failed delivery leaves behind (#1480).
///
/// Three outcomes, three different sentences, because "enqueued for retry" was
/// true while the durability an operator would infer from it was not:
///
/// - rejected: the envelope is gone, and the reason says so with the count;
/// - in-memory queue: it survives the process only until it does not;
/// - durable queue: it is recoverable, so `undelivered` is zero — the remaining
///   fact is that the backend did not receive it during the run.
fn degraded_or_queue_fact(
    prior: Option<DegradedEvidence>,
    queue: &dyn AuditQueue,
    reason: &str,
    enqueued: Result<EnqueueOutput, AuditError>,
) -> DegradedEvidence {
    let (queue_fact, undelivered) = match enqueued {
        Err(error) => (
            format!(
                "the retry queue rejected the envelope ({error}); it is lost when the process \
                 exits"
            ),
            1,
        ),
        Ok(_) if queue.durable() => (
            "the envelope is queued durably and will be retried".to_string(),
            0,
        ),
        Ok(_) => (
            "the envelope is queued in memory and is lost on restart".to_string(),
            1,
        ),
    };
    match prior {
        Some(prior) => DegradedEvidence {
            reason: format!("{}; also: {reason}; {queue_fact}", prior.reason),
            undelivered: prior.undelivered.max(undelivered),
        },
        None => DegradedEvidence {
            reason: format!("{reason}; {queue_fact}"),
            undelivered,
        },
    }
}

#[async_trait]
impl AuditService for AuditServiceImpl {
    async fn build_and_send(
        &self,
        mut input: BuildEnvelopeInput,
    ) -> Result<BuildEnvelopeOutput, AuditError> {
        // ADR-016 Phase A: resolve the local chain link BEFORE signing, so the
        // chain fields are covered by the HMAC (the chain and the signature
        // agree). The in-process lock serializes link+save so two concurrent
        // runs cannot claim the same sequence number.
        let mut chain_guard = None;
        if let Some(repo) = &self.local_repository {
            chain_guard = Some(self.chain_lock.lock().await);
            let existing = repo.list(None, None, Some(u32::MAX)).await?;
            // ADR-016: one execution may emit more than one envelope — a
            // pause-point snapshot, then a re-dispatched FINAL envelope after
            // approval (MCP host, F-20260907-05). The local store keeps ONE
            // envelope per `execution_id` (`{execution_id}.json`), so the
            // re-emission REPLACES that chain link and MUST reuse the sequence
            // and `prev_hash` of the link it replaces: a fresh sequence would
            // leave a hole where the replaced link was, and `verify_chain`
            // would read the store as an interior deletion. Only a prior
            // envelope already chained for THIS producer is reused; a legacy
            // (unchained) prior falls through to a fresh link and is
            // overwritten harmlessly (it never held a sequence).
            input.producer_id = Some(self.producer_id.clone());
            let prior = existing.iter().find(|e| {
                e.execution_id == input.execution_id
                    && e.producer_id.as_deref() == Some(self.producer_id.as_str())
            });
            match prior {
                Some(prior) => {
                    input.sequence = prior.sequence;
                    input.prev_hash = prior.prev_hash.clone();
                }
                None => {
                    let (sequence, prev_hash) = crate::audit::application::chain::next_link_from(
                        &existing,
                        &self.producer_id,
                    )?;
                    input.sequence = Some(sequence);
                    input.prev_hash = prev_hash;
                }
            }
        }
        // ADR-016: the run-wide history-policy opt-in is a SIGNED field, so the
        // regime under which the evidence was produced is auditable.
        input.history_policy = self.history_policy;

        // Build the envelope (always — the caller needs it back for the
        // run response/read-back even when remote delivery is disabled).
        let envelope = self.envelope_factory.build_envelope(input).await?;
        let event_count = envelope.events.len();
        let signed = envelope.signature.is_some();

        // R7 / offline read-back: persist to the local store when wired —
        // INDEPENDENT of remote-backend enablement. The signed local trail
        // is the cross-run policy history (`EnvelopeHistoryAdapter` over
        // `<repo_root>/.rigorix/audit`); coupling it to a configured backend
        // URL silently disabled R7 (envelope never saved → empty history →
        // no-cross-run rules never fire).
        //
        // #1480: this save used to be best-effort (warn and continue), so the
        // artifact the design treats as the evidence of record could be missing
        // while the run reported success. The failure is now a fact on the output.
        let mut degraded = None;
        if let Some(repo) = &self.local_repository
            && let Err(e) = repo.save(&envelope).await
        {
            tracing::warn!(
                execution_id = %envelope.execution_id,
                "audit: local envelope persistence failed ({e}) — cross-run policy history may be incomplete"
            );
            degraded = Some(DegradedEvidence {
                reason: format!(
                    "the local signed trail rejected the save ({e}); cross-run policy history \
                     will not include this run"
                ),
                undelivered: 0,
            });
        }
        // Release the chain lock before (potentially slow) remote delivery;
        // the envelope is already persisted and chained.
        drop(chain_guard);

        // Remote delivery is gated on `enabled` (a backend URL configured);
        // the local signed trail is NOT.
        if !self.enabled {
            return Ok(BuildEnvelopeOutput {
                envelope,
                signed,
                event_count,
                degraded,
            });
        }

        // Try to send
        let deliver_input = DeliverEnvelopeInput {
            envelope: envelope.clone(),
            max_retries: self.max_retries,
            backoff_base_secs: self.backoff_base_secs,
            backoff_max_secs: self.backoff_max_secs,
        };

        let delivery = self.sender.deliver_with_retry(deliver_input).await;

        // #1480: every arm below computes a fact instead of discarding one. The
        // enqueue result used to be `let _ =`, so a queue that rejected (full, or
        // in-memory and lossy) looked identical to one that accepted.
        match delivery {
            Ok(output) if output.success => {
                // Delivered successfully: the evidence is complete unless the
                // local save already failed above.
            }
            Ok(output) => {
                let reason = output
                    .last_error
                    .unwrap_or_else(|| "delivery failed after retries".to_string());
                let enqueued = self
                    .queue
                    .enqueue(EnqueueInput {
                        envelope: envelope.clone(),
                        failure_reason: reason.clone(),
                        retry_count: output.attempts,
                        max_retries: self.max_retries,
                    })
                    .await;
                degraded = Some(degraded_or_queue_fact(
                    degraded,
                    self.queue.as_ref(),
                    &reason,
                    enqueued,
                ));
            }
            Err(error) => {
                // Unexpected error from sender — enqueue
                let reason = format!("the sender failed unexpectedly ({error})");
                let enqueued = self
                    .queue
                    .enqueue(EnqueueInput {
                        envelope: envelope.clone(),
                        failure_reason: reason.clone(),
                        retry_count: 0,
                        max_retries: self.max_retries,
                    })
                    .await;
                degraded = Some(degraded_or_queue_fact(
                    degraded,
                    self.queue.as_ref(),
                    &reason,
                    enqueued,
                ));
            }
        }

        Ok(BuildEnvelopeOutput {
            envelope,
            signed,
            event_count,
            degraded,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn retry_pending(&self) -> Result<RetryPendingOutput, AuditError> {
        let mut delivered = 0u32;
        let mut still_pending = 0u32;
        let mut dropped = 0u32;

        while let Some(output) = self.queue.dequeue().await? {
            if let Some(envelope) = output.envelope {
                let deliver_input = DeliverEnvelopeInput {
                    envelope,
                    max_retries: self.max_retries,
                    backoff_base_secs: self.backoff_base_secs,
                    backoff_max_secs: self.backoff_max_secs,
                };

                match self.sender.deliver_with_retry(deliver_input).await {
                    Ok(delivery) if delivery.success => {
                        delivered += 1;
                    }
                    Ok(delivery) if delivery.attempts >= self.max_retries => {
                        dropped += 1;
                    }
                    _ => {
                        still_pending += 1;
                    }
                }
            }
        }

        Ok(RetryPendingOutput {
            delivered,
            still_pending,
            dropped,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn status(&self) -> Result<AuditStatusOutput, AuditError> {
        let pending_count = self.queue.len().await?;
        Ok(AuditStatusOutput {
            pending_count,
            circuit_breaker_state: crate::audit::domain::CircuitBreakerState::Closed,
            backend_available: self.enabled,
        })
    }
}

#[cfg(all(test, not(feature = "portable")))]
mod tests {
    use super::*;
    use crate::audit::domain::{EventStatus, ExecutionEventRef};

    #[tracing::instrument(skip_all)]
    fn sample_input() -> BuildEnvelopeInput {
        BuildEnvelopeInput {
            execution_id: uuid::Uuid::new_v4(),
            template_id: "test-template".to_string(),
            planning_prompt: "plan the execution".to_string(),
            events: vec![ExecutionEventRef {
                event_type: "task_completed".to_string(),
                summary: "Test task completed".to_string(),
                occurred_at: chrono::Utc::now(),
                correlation_id: None,
                status: EventStatus::Success,
                payload: None,
            }],
            source: None,
            total_tokens: 0,
            duration_ms: 0,
            git_commit: None,
            git_branch: None,
            model_version: None,
            planning_prompt_content: None,
            file_paths: vec![],
            metadata: None,
            scoring_results: std::collections::HashMap::new(),
            sign: false,
            repository: None,
            author: None,
            identity: None,
            effect_key: None,
            producer_id: None,
            sequence: None,
            prev_hash: None,
            history_policy: None,
        }
    }

    // -----------------------------------------------------------------------
    // #1480: the evidence path states facts instead of dropping them
    // -----------------------------------------------------------------------

    /// A sender whose backend is unreachable for every attempt.
    struct UnreachableSender;

    #[async_trait]
    impl super::AuditSender for UnreachableSender {
        async fn send(
            &self,
            _: crate::audit::application::SendEnvelopeInput,
        ) -> Result<crate::audit::application::SendEnvelopeOutput, AuditError> {
            Err(AuditError::SendFailed {
                detail: "backend unreachable".to_string(),
                attempt: 1,
                max_retries: 3,
                http_status: None,
            })
        }

        async fn deliver_with_retry(
            &self,
            _: crate::audit::application::DeliverEnvelopeInput,
        ) -> Result<crate::audit::application::DeliverEnvelopeOutput, AuditError> {
            Ok(crate::audit::application::DeliverEnvelopeOutput {
                success: false,
                attempts: 3,
                total_duration_ms: 5,
                last_http_status: None,
                last_error: Some("backend unreachable after 3 attempts".to_string()),
            })
        }
    }

    /// A local trail that rejects every save (a full disk, an unwritable dir).
    struct RejectingRepository;

    #[async_trait]
    impl crate::audit::infrastructure::repository::AuditEnvelopeRepository for RejectingRepository {
        async fn save(&self, _: &crate::audit::domain::AuditEnvelope) -> Result<(), AuditError> {
            Err(AuditError::Internal {
                detail: "read-only file system".to_string(),
            })
        }

        async fn find_by_execution_id(
            &self,
            _: &uuid::Uuid,
        ) -> Result<Option<crate::audit::domain::AuditEnvelope>, AuditError> {
            Ok(None)
        }

        async fn list(
            &self,
            _: Option<chrono::DateTime<chrono::Utc>>,
            _: Option<chrono::DateTime<chrono::Utc>>,
            _: Option<u32>,
        ) -> Result<Vec<crate::audit::domain::AuditEnvelope>, AuditError> {
            Ok(vec![])
        }

        async fn delete(&self, _: &uuid::Uuid) -> Result<(), AuditError> {
            Ok(())
        }

        async fn record_delivery(
            &self,
            _: &crate::audit::application::RecordDeliveryInput,
        ) -> Result<(), AuditError> {
            Ok(())
        }

        async fn count(
            &self,
            _: Option<chrono::DateTime<chrono::Utc>>,
            _: Option<chrono::DateTime<chrono::Utc>>,
        ) -> Result<u64, AuditError> {
            Ok(0)
        }

        async fn prune(&self, _: chrono::DateTime<chrono::Utc>) -> Result<u64, AuditError> {
            Ok(0)
        }
    }

    fn failing_delivery_service(queue: Box<dyn super::AuditQueue>) -> AuditServiceImpl {
        let mut service = AuditServiceImpl::new(
            Box::new(crate::audit::application::AuditEnvelopeFactoryImpl::new(
                None,
            )),
            std::sync::Arc::new(UnreachableSender),
            queue,
            true,
        );
        service.max_retries = 3;
        service
    }

    /// A failed delivery is a stated fact: the envelope is undelivered, and the
    /// reason says which queue it went to and what that queue costs.
    #[tokio::test]
    async fn a_failed_delivery_reports_degraded_with_the_memory_loss_window() {
        let service =
            failing_delivery_service(Box::new(crate::audit::application::AuditQueueImpl::new(10)));

        let output = service
            .build_and_send(sample_input())
            .await
            .expect("build succeeds even when delivery does not");

        let degraded = output.degraded.expect("a failed delivery is not Complete");
        assert_eq!(
            degraded.undelivered, 1,
            "the envelope did not reach the backend"
        );
        assert!(
            degraded.reason.contains("in memory") && degraded.reason.contains("lost on restart"),
            "the in-memory loss window must be named: {}",
            degraded.reason
        );
    }

    /// The same failure with a durable queue is recoverable — and says so,
    /// instead of claiming a durability the configuration does not have.
    #[tokio::test]
    async fn a_failed_delivery_into_a_durable_queue_is_recoverable_and_says_so() {
        let dir = tempfile::tempdir().expect("temp dir");
        let queue = crate::audit::application::AuditQueueImpl::durable(dir.path(), 10)
            .expect("durable queue");
        let service = failing_delivery_service(Box::new(queue));

        let output = service
            .build_and_send(sample_input())
            .await
            .expect("build succeeds");

        let degraded = output.degraded.expect("delivery did not happen");
        assert_eq!(
            degraded.undelivered, 0,
            "a durably queued envelope is not lost"
        );
        assert!(
            degraded.reason.contains("durably"),
            "the reason must state the durability: {}",
            degraded.reason
        );

        // And it really is recoverable: a fresh queue over the same directory
        // finds it pending.
        let restarted =
            crate::audit::application::AuditQueueImpl::durable(dir.path(), 10).expect("reopen");
        assert_eq!(
            super::AuditQueue::len(&restarted).await.unwrap(),
            1,
            "the queued envelope survives the process"
        );
    }

    /// #1480 acceptance 4: the local save is not best-effort. An unwritable
    /// evidence store makes the run report `Degraded`, not success.
    #[tokio::test]
    async fn a_rejected_local_save_is_degraded_not_success() {
        let mut service = AuditServiceImpl {
            enabled: false,
            ..AuditServiceImpl::default_test()
        };
        service.local_repository = Some(std::sync::Arc::new(RejectingRepository));

        let output = service
            .build_and_send(sample_input())
            .await
            .expect("the envelope is still built");

        let degraded = output
            .degraded
            .expect("a lost evidence-of-record is not Complete");
        assert!(
            degraded.reason.contains("local signed trail"),
            "the reason must name the store that rejected it: {}",
            degraded.reason
        );
    }

    /// #1480 acceptance 2 at the queue's own level: enqueue, drop the process
    /// (drop the queue), re-read.
    #[tokio::test]
    async fn a_durable_queue_survives_a_restart() {
        let dir = tempfile::tempdir().expect("temp dir");
        let execution_id = {
            let queue = crate::audit::application::AuditQueueImpl::durable(dir.path(), 10)
                .expect("durable queue");
            let envelope = crate::audit::application::AuditEnvelopeFactoryImpl::new(None)
                .build_envelope(sample_input())
                .await
                .expect("build the envelope");
            let id = envelope.execution_id;
            let input = crate::audit::application::EnqueueInput {
                envelope,
                failure_reason: "connection timeout".to_string(),
                retry_count: 1,
                max_retries: 3,
            };
            super::AuditQueue::enqueue(&queue, input)
                .await
                .expect("enqueue");
            id
        };

        let restarted = crate::audit::application::AuditQueueImpl::durable(dir.path(), 10)
            .expect("reopen the queue");
        assert!(super::AuditQueue::durable(&restarted));
        let pending = super::AuditQueue::dequeue(&restarted)
            .await
            .expect("dequeue")
            .expect("the envelope is still pending after the restart");
        assert_eq!(
            pending.envelope.expect("envelope").execution_id,
            execution_id
        );
    }

    #[tokio::test]
    async fn test_build_and_send_disabled() {
        let service = AuditServiceImpl {
            enabled: false,
            ..AuditServiceImpl::default_test()
        };
        // Disabled audit still BUILDS the envelope (needed for the run
        // response/read-back); only remote delivery is skipped.
        let mut input = sample_input();
        input.sign = false;
        let exec_id = input.execution_id;
        let result = service.build_and_send(input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.signed);
        assert_eq!(
            output.envelope.execution_id, exec_id,
            "envelope must still be built when delivery is disabled"
        );
    }

    /// R7 regression (conference-demo live session, 2026-09-08): the signed
    /// local trail must persist even when NO remote backend is configured —
    /// cross-run policy reads it. Before the fix, `!enabled` returned before
    /// the local save and R7 history was always empty.
    #[tokio::test]
    async fn test_local_trail_persists_when_backend_disabled() {
        let dir = tempfile::tempdir().expect("temp dir");
        let repo = std::sync::Arc::new(
            crate::audit::infrastructure::LocalAuditEnvelopeRepository::new(
                dir.path().to_path_buf(),
            ),
        );
        let mut service = AuditServiceImpl::default_test();
        service.enabled = false; // no remote backend configured
        service.local_repository = Some(repo);
        let input = sample_input(); // signed (default sample signs)
        let output = service
            .build_and_send(input.clone())
            .await
            .expect("build_and_send succeeds");
        assert!(output.degraded.is_none(), "{:?}", output.degraded);
        let files: Vec<_> = std::fs::read_dir(dir.path())
            .expect("read audit dir")
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 1, "envelope must be persisted locally");
        let saved = std::fs::read_to_string(files[0].path()).expect("read envelope");
        assert!(
            saved.contains("\"signature\":"),
            "saved envelope must carry the HMAC signature"
        );
    }

    #[tokio::test]
    async fn test_build_and_send_enabled_no_backend() {
        let service = AuditServiceImpl::default_test();
        let mut input = sample_input();
        input.sign = false;
        let result = service.build_and_send(input).await;
        // Should still succeed — building works even if send fails
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_status() {
        let service = AuditServiceImpl::default_test();
        let status = service.status().await.unwrap();
        assert_eq!(status.pending_count, 0);
    }
}

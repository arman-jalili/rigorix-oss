//! Implementation of `AuditQueue`.
//!
//! @canonical .pi/architecture/modules/audit.md#queue
//! Implements: AuditQueue trait — bounded retry queue, in memory or file-backed
//! Issue: #14, #1480
//!
//! Two modes, one API:
//!
//! - [`AuditQueueImpl::new`] — bounded in-memory FIFO. An undelivered envelope is
//!   lost on restart and on overflow. That is a loss window, and it is stated
//!   rather than implied (`AuditQueue::durable` returns `false`, and the audit
//!   service records "queued in memory and is lost on restart" on the run's
//!   evidence status).
//! - [`AuditQueueImpl::durable`] — the same queue with the pending set on disk:
//!   `enqueue` writes the envelope to `<dir>` (temp file + rename + fsync) before
//!   returning, and a new instance reads the directory back on construction, so
//!   a restart finds the envelope still pending (#1480 acceptance 2).
//!
//! # What it does not do
//!
//! It does not survive a full disk: a write failure means the envelope was never
//! queued, and `enqueue` says so instead of reporting success. It does not
//! deduplicate: an envelope enqueued twice is two files.

use async_trait::async_trait;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

use crate::audit::domain::{AuditEnvelope, AuditError};

use super::dto::{EnqueueInput, EnqueueOutput};
use super::service::AuditQueue;

/// The on-disk shape of a pending envelope.
#[derive(serde::Serialize, serde::Deserialize)]
struct PersistedEnvelope {
    envelope: AuditEnvelope,
    failure_reason: String,
    retry_count: u32,
    max_retries: u32,
}

/// Bounded implementation of `AuditQueue`, in memory or file-backed.
pub struct AuditQueueImpl {
    /// Maximum queue capacity.
    capacity: u32,
    /// Internal FIFO queue (front = oldest, back = newest).
    queue: Mutex<VecDeque<QueuedEnvelope>>,
    /// When set, every pending envelope is also a file in this directory.
    durable_dir: Option<PathBuf>,
}

/// A failed delivery envelope waiting for retry.
#[derive(Debug, Clone)]
struct QueuedEnvelope {
    envelope: AuditEnvelope,
    failure_reason: String,
    retry_count: u32,
    max_retries: u32,
    /// The file backing this entry, when the queue is durable.
    file: Option<PathBuf>,
}

impl AuditQueueImpl {
    /// Create a new bounded in-memory audit queue.
    ///
    /// The loss window is part of the contract, not a defect: a queued envelope
    /// is lost on restart and rejected at capacity.
    pub fn new(capacity: u32) -> Self {
        Self {
            capacity,
            queue: Mutex::new(VecDeque::with_capacity(capacity as usize)),
            durable_dir: None,
        }
    }

    /// Create a bounded queue whose pending set lives in `dir` (#1480).
    ///
    /// Files already in `dir` are loaded, oldest first, so a queue constructed
    /// after a restart resumes with the envelopes the previous process queued.
    /// A file that cannot be parsed is left in place and logged: deleting it
    /// would destroy the only copy of evidence we failed to read.
    pub fn durable(dir: impl Into<PathBuf>, capacity: u32) -> Result<Self, AuditError> {
        let dir = dir.into();
        // sink: audit:retry-queue
        std::fs::create_dir_all(&dir).map_err(|e| AuditError::Internal {
            detail: format!(
                "cannot create audit queue directory '{}': {e}",
                dir.display()
            ),
        })?;

        let mut pending = Vec::new();
        let entries = std::fs::read_dir(&dir).map_err(|e| AuditError::Internal {
            detail: format!("cannot read audit queue directory '{}': {e}", dir.display()),
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<PersistedEnvelope>(&content) {
                    Ok(persisted) => pending.push(QueuedEnvelope {
                        envelope: persisted.envelope,
                        failure_reason: persisted.failure_reason,
                        retry_count: persisted.retry_count,
                        max_retries: persisted.max_retries,
                        file: Some(path),
                    }),
                    Err(error) => tracing::warn!(
                        path = %path.display(),
                        "audit: queued envelope is unreadable ({error}); leaving the file in place"
                    ),
                },
                Err(error) => tracing::warn!(
                    path = %path.display(),
                    "audit: queued envelope could not be read ({error}); leaving the file in place"
                ),
            }
        }
        // The filename prefix is the enqueue time in milliseconds, so a plain
        // sort restores FIFO order across a restart.
        pending.sort_by(|a, b| a.file.cmp(&b.file));

        let mut queue = VecDeque::with_capacity(capacity as usize);
        for item in pending {
            if queue.len() >= capacity as usize {
                tracing::warn!(
                    capacity,
                    "audit: durable queue holds more envelopes than its capacity; the excess stays on disk"
                );
                break;
            }
            queue.push_back(item);
        }

        Ok(Self {
            capacity,
            queue: Mutex::new(queue),
            durable_dir: Some(dir),
        })
    }

    /// Write one pending envelope to the durable directory.
    ///
    /// Temp file, fsync, rename: an `enqueue` that returns `Ok` has the envelope
    /// on disk, which is the only reason the caller may call it durable.
    fn persist(&self, item: &QueuedEnvelope) -> Result<PathBuf, AuditError> {
        use std::io::Write;
        let Some(dir) = self.durable_dir.as_ref() else {
            return Err(AuditError::Internal {
                detail: "the durable queue has no directory; the envelope was not persisted"
                    .to_string(),
            });
        };
        let name = format!(
            "{:013}-{}.json",
            chrono::Utc::now().timestamp_millis(),
            uuid::Uuid::new_v4()
        );
        let path = dir.join(&name);
        let temp = dir.join(format!(".{name}.tmp"));
        let body = serde_json::to_vec(&PersistedEnvelope {
            envelope: item.envelope.clone(),
            failure_reason: item.failure_reason.clone(),
            retry_count: item.retry_count,
            max_retries: item.max_retries,
        })
        .map_err(|e| AuditError::Internal {
            detail: format!("cannot serialize the envelope for the durable queue: {e}"),
        })?;

        let write = || -> std::io::Result<()> {
            let mut file = std::fs::File::create(&temp)?;
            file.write_all(&body)?;
            file.sync_all()?;
            drop(file);
            // sink: audit:retry-queue
            std::fs::rename(&temp, &path)
        };
        write().map_err(|e| {
            // sink: audit:retry-queue
            if let Err(cleanup) = std::fs::remove_file(&temp) {
                tracing::debug!(
                    path = %temp.display(),
                    "audit: could not remove the temporary queue file ({cleanup})"
                );
            }
            AuditError::Internal {
                detail: format!(
                    "cannot persist the envelope to '{}': {e} — it was not queued",
                    path.display()
                ),
            }
        })?;
        Ok(path)
    }

    /// Remove a dequeued/cleared envelope's backing file.
    ///
    /// A failure here means the file survives its in-memory entry, so the
    /// envelope would be re-queued on the next restart (a duplicate delivery,
    /// not a lost one) — worth a line, not a failure.
    fn remove_file(path: Option<&Path>) {
        if let Some(path) = path
            // sink: audit:retry-queue
            && let Err(error) = std::fs::remove_file(path)
        {
            tracing::warn!(
                path = %path.display(),
                "audit: queued envelope file could not be removed ({error}); it will be re-read on the next restart"
            );
        }
    }
}

impl Default for AuditQueueImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new(100) // Default capacity of 100
    }
}

#[async_trait]
impl AuditQueue for AuditQueueImpl {
    fn durable(&self) -> bool {
        self.durable_dir.is_some()
    }

    #[tracing::instrument(skip_all)]
    async fn enqueue(&self, input: EnqueueInput) -> Result<EnqueueOutput, AuditError> {
        let mut queue = self.queue.lock().await;

        if queue.len() >= self.capacity as usize {
            return Err(AuditError::QueueFull {
                capacity: self.capacity,
                pending: queue.len() as u32,
            });
        }

        let mut item = QueuedEnvelope {
            envelope: input.envelope,
            failure_reason: input.failure_reason,
            retry_count: input.retry_count,
            max_retries: input.max_retries,
            file: None,
        };
        // Persist before the in-memory push: if the write fails the caller is
        // told the envelope was not queued, instead of holding an entry that
        // disappears at the next restart.
        if self.durable_dir.is_some() {
            item.file = Some(self.persist(&item)?);
        }
        queue.push_back(item);

        Ok(EnqueueOutput {
            envelope: None,
            success: true,
            reason: None,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn dequeue(&self) -> Result<Option<EnqueueOutput>, AuditError> {
        let mut queue = self.queue.lock().await;

        match queue.pop_front() {
            Some(queued) => {
                Self::remove_file(queued.file.as_deref());
                Ok(Some(EnqueueOutput {
                    envelope: Some(queued.envelope),
                    success: true,
                    reason: Some(queued.failure_reason),
                }))
            }
            None => Ok(None),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn peek(&self) -> Result<Option<EnqueueOutput>, AuditError> {
        let queue = self.queue.lock().await;

        match queue.front() {
            Some(queued) => Ok(Some(EnqueueOutput {
                envelope: Some(queued.envelope.clone()),
                success: true,
                reason: Some(queued.failure_reason.clone()),
            })),
            None => Ok(None),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn len(&self) -> Result<u32, AuditError> {
        let queue = self.queue.lock().await;
        Ok(queue.len() as u32)
    }

    #[tracing::instrument(skip_all)]
    async fn is_empty(&self) -> Result<bool, AuditError> {
        let queue = self.queue.lock().await;
        Ok(queue.is_empty())
    }

    #[tracing::instrument(skip_all)]
    async fn clear(&self) -> Result<u32, AuditError> {
        let mut queue = self.queue.lock().await;
        let count = queue.len() as u32;
        for item in queue.iter() {
            Self::remove_file(item.file.as_deref());
        }
        queue.clear();
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::domain::{EventStatus, ExecutionEventRef};

    #[tracing::instrument(skip_all)]
    fn sample_envelope() -> AuditEnvelope {
        AuditEnvelope {
            execution_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            template_id: "test".to_string(),
            planning_hash: "abc123".to_string(),
            events: vec![ExecutionEventRef {
                event_type: "test".to_string(),
                summary: "test event".to_string(),
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
            planning_prompt: None,
            file_paths: vec![],
            scoring_results: std::collections::HashMap::new(),
            approval_events: Vec::new(),
            sequence_policy_findings: Vec::new(),
            requirement_findings: Vec::new(),
            precondition_findings: Vec::new(),
            scope_violations: Vec::new(),
            scope_check: None,
            decision_context_ref: None,
            signature: None,
            evidence_degraded: false,
            repository: None,
            author: None,
            identity: None,
            effect_key: None,
            producer_id: None,
            sequence: None,
            prev_hash: None,
            history_integrity: None,
            anchor_head: None,
            history_policy: None,
        }
    }

    #[tracing::instrument(skip_all)]
    fn sample_input() -> EnqueueInput {
        EnqueueInput {
            envelope: sample_envelope(),
            failure_reason: "connection timeout".to_string(),
            retry_count: 0,
            max_retries: 3,
        }
    }

    #[tokio::test]
    async fn test_enqueue_dequeue() {
        let queue = AuditQueueImpl::new(10);
        assert!(queue.is_empty().await.unwrap());

        queue.enqueue(sample_input()).await.unwrap();
        assert_eq!(queue.len().await.unwrap(), 1);

        let output = queue.dequeue().await.unwrap();
        assert!(output.is_some());
        assert!(queue.is_empty().await.unwrap());
    }

    #[tokio::test]
    async fn test_peek_does_not_remove() {
        let queue = AuditQueueImpl::new(10);
        queue.enqueue(sample_input()).await.unwrap();

        let peeked = queue.peek().await.unwrap();
        assert!(peeked.is_some());
        assert_eq!(queue.len().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_queue_full() {
        let queue = AuditQueueImpl::new(2);
        queue.enqueue(sample_input()).await.unwrap();
        queue.enqueue(sample_input()).await.unwrap();

        let result = queue.enqueue(sample_input()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditError::QueueFull { capacity, pending } => {
                assert_eq!(capacity, 2);
                assert_eq!(pending, 2);
            }
            other => panic!("Expected QueueFull, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_clear() {
        let queue = AuditQueueImpl::new(10);
        queue.enqueue(sample_input()).await.unwrap();
        queue.enqueue(sample_input()).await.unwrap();

        let cleared = queue.clear().await.unwrap();
        assert_eq!(cleared, 2);
        assert!(queue.is_empty().await.unwrap());
    }

    #[tokio::test]
    async fn test_dequeue_empty() {
        let queue = AuditQueueImpl::new(10);
        let output = queue.dequeue().await.unwrap();
        assert!(output.is_none());
    }
}

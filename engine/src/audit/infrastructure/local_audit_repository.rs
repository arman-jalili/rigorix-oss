//! Local filesystem-based implementation of `AuditEnvelopeRepository`.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: AuditEnvelopeRepository trait — persists envelopes as JSON files
//! Issue: #14
//!
//! Persists audit envelopes as individual JSON files on the local filesystem.
//! Uses atomic write-rename pattern for crash safety. Envelopes are stored
//! under a configurable base path with execution ID as the filename.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::audit::domain::{AuditEnvelope, AuditError};

use crate::audit::application::dto::RecordDeliveryInput;
use crate::audit::infrastructure::repository::AuditEnvelopeRepository;

/// Filesystem-based implementation of `AuditEnvelopeRepository`.
///
/// Stores envelopes as `{base_path}/{execution_id}.json` files.
/// Uses atomic write-rename pattern for crash-safe persistence.
pub struct LocalAuditEnvelopeRepository {
    /// Base directory for envelope storage.
    base_path: PathBuf,
    /// An explicit retention bound, when the composition resolved one (#1658).
    max_envelopes: Option<usize>,
}

/// How many envelopes `.rigorix/audit/` keeps before the oldest are removed (#1658).
///
/// Overridable with `RIGORIX_AUDIT_MAX_ENVELOPES` (a deployment with a long review window raises it);
/// the default bounds a host that runs a request loop.
pub const DEFAULT_MAX_LOCAL_AUDIT_ENVELOPES: usize = 5_000;

/// The retention bound a configured value resolves to (`None` → the default) (#1658).
///
/// Pure, so the rule is testable without touching the process environment.
pub fn resolve_max_envelopes(configured: Option<&str>) -> usize {
    configured
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|bound| *bound > 0)
        .unwrap_or(DEFAULT_MAX_LOCAL_AUDIT_ENVELOPES)
}

impl LocalAuditEnvelopeRepository {
    /// The configured bound: `RIGORIX_AUDIT_MAX_ENVELOPES`, or the default.
    pub fn max_envelopes(&self) -> usize {
        if let Some(bound) = self.max_envelopes {
            return bound;
        }
        // sink: audit:retention-bound
        resolve_max_envelopes(std::env::var("RIGORIX_AUDIT_MAX_ENVELOPES").ok().as_deref())
    }

    /// Remove the oldest envelopes beyond [`Self::max_envelopes`], and say so.
    ///
    /// Oldest is decided by modification time, then by name, so the order is total even on a
    /// filesystem with coarse timestamps.
    async fn enforce_retention(&self) -> Result<u64, AuditError> {
        let bound = self.max_envelopes();
        let mut entries: Vec<(std::time::SystemTime, std::path::PathBuf)> = Vec::new();
        let mut dir =
            tokio::fs::read_dir(&self.base_path)
                .await
                .map_err(|e| AuditError::Internal {
                    detail: format!("retention: failed to read directory: {e}"),
                })?;
        while let Some(entry) = dir.next_entry().await.map_err(|e| AuditError::Internal {
            detail: format!("retention: failed to read an entry: {e}"),
        })? {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let modified = entry
                .metadata()
                .await
                .ok()
                .and_then(|meta| meta.modified().ok())
                .unwrap_or(std::time::UNIX_EPOCH);
            entries.push((modified, path));
        }
        if entries.len() <= bound {
            return Ok(0);
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let excess = entries.len() - bound;
        let mut deleted = 0u64;
        for (_, path) in entries.into_iter().take(excess) {
            // sink: audit:local-chain
            match tokio::fs::remove_file(&path).await {
                Ok(()) => deleted += 1,
                Err(error) => tracing::warn!(
                    path = %path.display(),
                    %error,
                    "audit retention: could not remove an envelope past the bound"
                ),
            }
        }
        if deleted > 0 {
            tracing::warn!(
                deleted,
                kept = bound,
                bound_source = "RIGORIX_AUDIT_MAX_ENVELOPES or DEFAULT_MAX_LOCAL_AUDIT_ENVELOPES",
                "audit retention: removed the oldest envelopes past the bound — the trail is bounded \
                 by a count, and this line is the record that it was (the bound, not an age, so a \
                 quiet month cannot empty the trail)"
            );
        }
        Ok(deleted)
    }
    /// Create a new repository with the given base path.
    ///
    /// Creates the base directory if it doesn't exist.
    pub fn new(base_path: PathBuf) -> Self {
        // sink: audit:local-chain
        crate::common::private_fs::create_dir_all(&base_path).ok();
        Self {
            base_path,
            max_envelopes: None,
        }
    }

    /// The same store with an explicit retention bound, for a test or a composition that has already
    /// resolved one (#1658).
    ///
    /// An explicit bound rather than an environment variable in every case: a test that mutates the
    /// process environment races every other test that reads it, and a race here reads as a retention
    /// bug in a completely different test.
    pub fn with_max_envelopes(base_path: PathBuf, max_envelopes: usize) -> Self {
        // sink: audit:local-chain
        crate::common::private_fs::create_dir_all(&base_path).ok();
        Self {
            base_path,
            max_envelopes: Some(max_envelopes),
        }
    }

    /// Build the file path for an execution ID.
    fn envelope_path(&self, execution_id: &uuid::Uuid) -> PathBuf {
        self.base_path.join(format!("{}.json", execution_id))
    }

    /// Atomically write content to a file using write-rename pattern.
    /// One atomic, owner-only write (#1658 item 2).
    ///
    /// Measured before the change: `std::fs::write` under the usual `umask 022` produces **0644**, so an
    /// envelope — which records the planning prompt, the file paths and the findings — was readable by
    /// every account on the host. The temp file carries mode 0600 and *is* what the rename makes the
    /// destination, so a file written before this rule is tightened the next time it is replaced.
    fn atomic_write(path: &Path, content: &str) -> Result<(), AuditError> {
        // sink: audit:local-chain
        crate::common::private_fs::write_atomic(path, content.as_bytes()).map_err(|e| {
            AuditError::Internal {
                detail: format!("Failed to write '{}': {e}", path.display()),
            }
        })
    }
}

#[async_trait]
impl AuditEnvelopeRepository for LocalAuditEnvelopeRepository {
    async fn save(&self, envelope: &AuditEnvelope) -> Result<(), AuditError> {
        let json = serde_json::to_string_pretty(envelope).map_err(|e| {
            AuditError::SerializationFailed {
                detail: e.to_string(),
            }
        })?;

        let path = self.envelope_path(&envelope.execution_id);
        Self::atomic_write(&path, &json)?;

        // #1658 (audit retention): `.rigorix/audit/` grew one file per run, forever. `prune` existed
        // and was called only from the test module, so with an open `/rpc` a loop of `run_command`
        // steps grew the server host's disk without bound.
        //
        // The bound is a **count**, not an age: retention by age loses a trail after a quiet month,
        // and the failure the audit measured is disk growth, which a count bounds exactly and a clock
        // does not. Reaching it is a **recorded** event — the log line below names how many were
        // deleted and what the bound is — never a silent deletion, because an audit trail that
        // disappears without a word is the one thing this store must not do.
        if let Err(error) = self.enforce_retention().await {
            tracing::warn!(%error, "audit retention: could not enforce the bound; the trail keeps growing");
        }

        Ok(())
    }

    async fn find_by_execution_id(
        &self,
        execution_id: &uuid::Uuid,
    ) -> Result<Option<AuditEnvelope>, AuditError> {
        let path = self.envelope_path(execution_id);
        if !path.exists() {
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| AuditError::Internal {
                detail: format!("Failed to read envelope: {e}"),
            })?;

        let envelope: AuditEnvelope =
            serde_json::from_str(&content).map_err(|e| AuditError::SerializationFailed {
                detail: format!("Failed to deserialize envelope: {e}"),
            })?;

        Ok(Some(envelope))
    }

    async fn list(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
        limit: Option<u32>,
    ) -> Result<Vec<AuditEnvelope>, AuditError> {
        let mut envelopes = Vec::new();
        let max = limit.unwrap_or(100) as usize;

        let mut dir =
            tokio::fs::read_dir(&self.base_path)
                .await
                .map_err(|e| AuditError::Internal {
                    detail: format!("Failed to read directory: {e}"),
                })?;

        let mut sorted_entries: Vec<std::path::PathBuf> = Vec::new();
        while let Some(entry) = dir.next_entry().await.map_err(|e| AuditError::Internal {
            detail: format!("Failed to read directory entry: {e}"),
        })? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                sorted_entries.push(path);
            }
        }

        // Sort by modified time (newest first)
        sorted_entries.sort_by(|a, b| {
            let a_time = a.metadata().ok().and_then(|m| m.modified().ok());
            let b_time = b.metadata().ok().and_then(|m| m.modified().ok());
            b_time.cmp(&a_time)
        });

        for path in sorted_entries {
            if envelopes.len() >= max {
                break;
            }

            let content =
                tokio::fs::read_to_string(&path)
                    .await
                    .map_err(|e| AuditError::Internal {
                        detail: format!("Failed to read envelope: {e}"),
                    })?;

            if let Ok(envelope) = serde_json::from_str::<AuditEnvelope>(&content) {
                // Apply date filters
                if let Some(since) = since
                    && envelope.timestamp < since
                {
                    continue;
                }
                if let Some(until) = until
                    && envelope.timestamp > until
                {
                    continue;
                }
                envelopes.push(envelope);
            }
        }

        Ok(envelopes)
    }

    async fn delete(&self, execution_id: &uuid::Uuid) -> Result<(), AuditError> {
        let path = self.envelope_path(execution_id);
        if path.exists() {
            // sink: audit:local-chain
            tokio::fs::remove_file(&path)
                .await
                .map_err(|e| AuditError::Internal {
                    detail: format!("Failed to delete envelope: {e}"),
                })?;
        }
        Ok(())
    }

    async fn record_delivery(&self, _input: &RecordDeliveryInput) -> Result<(), AuditError> {
        // Delivery status is tracked externally (queue, circuit breaker).
        // For filesystem storage, the envelope is the source of truth.
        Ok(())
    }

    async fn count(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<u64, AuditError> {
        let envelopes = self.list(since, until, Some(u32::MAX)).await?;
        Ok(envelopes.len() as u64)
    }

    async fn prune(&self, older_than: chrono::DateTime<chrono::Utc>) -> Result<u64, AuditError> {
        let mut deleted = 0u64;

        let mut dir =
            tokio::fs::read_dir(&self.base_path)
                .await
                .map_err(|e| AuditError::Internal {
                    detail: format!("Failed to read directory: {e}"),
                })?;

        while let Some(entry) = dir.next_entry().await.map_err(|e| AuditError::Internal {
            detail: format!("Failed to read directory entry: {e}"),
        })? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
                if let Ok(envelope) = serde_json::from_str::<AuditEnvelope>(&content)
                    && envelope.timestamp < older_than
                {
                    // sink: audit:local-chain
                    tokio::fs::remove_file(&path).await.unwrap_or_default();
                    deleted += 1;
                }
            }
        }

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::domain::{EventStatus, ExecutionEventRef};
    use tempfile::TempDir;

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
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_save_and_find() {
        let dir = TempDir::new().unwrap();
        let repo = LocalAuditEnvelopeRepository::new(dir.path().to_path_buf());

        let envelope = sample_envelope();
        repo.save(&envelope).await.unwrap();

        let found = repo
            .find_by_execution_id(&envelope.execution_id)
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().execution_id, envelope.execution_id);
    }

    #[tokio::test]
    async fn test_find_not_found() {
        let dir = TempDir::new().unwrap();
        let repo = LocalAuditEnvelopeRepository::new(dir.path().to_path_buf());

        let result = repo
            .find_by_execution_id(&uuid::Uuid::new_v4())
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete() {
        let dir = TempDir::new().unwrap();
        let repo = LocalAuditEnvelopeRepository::new(dir.path().to_path_buf());

        let envelope = sample_envelope();
        repo.save(&envelope).await.unwrap();
        repo.delete(&envelope.execution_id).await.unwrap();

        let result = repo
            .find_by_execution_id(&envelope.execution_id)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list() {
        let dir = TempDir::new().unwrap();
        let repo = LocalAuditEnvelopeRepository::new(dir.path().to_path_buf());

        let e1 = sample_envelope();
        let e2 = sample_envelope();
        repo.save(&e1).await.unwrap();
        repo.save(&e2).await.unwrap();

        let envelopes = repo.list(None, None, None).await.unwrap();
        assert_eq!(envelopes.len(), 2);
    }

    #[tokio::test]
    async fn test_count() {
        let dir = TempDir::new().unwrap();
        let repo = LocalAuditEnvelopeRepository::new(dir.path().to_path_buf());

        repo.save(&sample_envelope()).await.unwrap();
        repo.save(&sample_envelope()).await.unwrap();

        let count = repo.count(None, None).await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_prune() {
        let dir = TempDir::new().unwrap();
        let repo = LocalAuditEnvelopeRepository::new(dir.path().to_path_buf());

        // Save two envelopes
        repo.save(&sample_envelope()).await.unwrap();
        repo.save(&sample_envelope()).await.unwrap();

        // Prune everything older than now + 1s (should prune nothing)
        let pruned = repo
            .prune(chrono::Utc::now() + chrono::Duration::seconds(1))
            .await
            .unwrap();
        assert_eq!(pruned, 2); // Both are older than now+1s

        // Prune everything older than now - 1s (should prune nothing)
        let pruned = repo
            .prune(chrono::Utc::now() - chrono::Duration::seconds(1))
            .await
            .unwrap();
        assert_eq!(pruned, 0);
    }
}

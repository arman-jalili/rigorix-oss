//! In-memory implementation of AuditQueryService.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#auditqueryservice
//! Implements: AuditQueryService — in-memory audit storage for testing and standalone use

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

use crate::audit_tools::domain::entity::AuditQueryService;
use crate::audit_tools::domain::error::AuditError;
use crate::audit_tools::domain::value::{
    AuditEnvelope, AuditFilter, AuditSummary, ExecutionStep, TopFailure, TopTemplate,
};
use crate::execution_tools::domain::value::{ExecutionId, ExecutionStatus};

/// Thread-safe in-memory audit query service.
///
/// Stores audit envelopes in memory and provides query/summary operations.
/// Useful for testing and as a standalone implementation before engine integration.
pub struct InMemoryAuditQueryService {
    audits: RwLock<HashMap<Uuid, AuditEnvelope>>,
}

impl InMemoryAuditQueryService {
    /// Create a new empty audit query service.
    pub fn new() -> Self {
        Self {
            audits: RwLock::new(HashMap::new()),
        }
    }

    /// Store an audit envelope.
    pub fn store(&self, envelope: AuditEnvelope) -> Result<(), AuditError> {
        let id = envelope.execution_id();
        let mut map = self
            .audits
            .write()
            .map_err(|e| AuditError::Internal(format!("Lock poisoned: {}", e)))?;
        map.insert(id, envelope);
        Ok(())
    }

    /// Store multiple audit envelopes.
    pub fn store_batch(&self, envelopes: Vec<AuditEnvelope>) -> Result<(), AuditError> {
        for envelope in envelopes {
            self.store(envelope)?;
        }
        Ok(())
    }

    /// Create a sample audit envelope for testing.
    pub fn create_sample(
        execution_id: Uuid,
        status: ExecutionStatus,
        template_name: Option<String>,
        started_at: DateTime<Utc>,
        duration_ms: u64,
    ) -> AuditEnvelope {
        AuditEnvelope::new(
            execution_id,
            status,
            template_name,
            started_at,
            started_at + chrono::Duration::milliseconds(duration_ms as i64),
            duration_ms,
            vec![],
            Some(100),
            "sample-hmac".into(),
            vec![],
        )
    }

    /// Build a REAL audit envelope from an actual run result and sign it.
    ///
    /// Replaces the fabricated `create_sample` path: steps come from the run
    /// output (real names, success flags, errors, durations), the HMAC is
    /// computed over the same canonical fields the engine signs
    /// (envelope_factory_impl::compute_signature), so `rigorix_read_audit`
    /// returns evidence that is honest — every step actually ran, and the
    /// signature is verifiable with the configured key.
    pub fn build_from_run(
        execution_id: Uuid,
        status: ExecutionStatus,
        template_name: Option<String>,
        duration_ms: u64,
        steps: Vec<ExecutionStep>,
        hmac_key: Option<&str>,
    ) -> AuditEnvelope {
        Self::build_from_run_at(
            execution_id,
            status,
            template_name,
            duration_ms,
            steps,
            hmac_key,
            chrono::Utc::now(),
        )
    }

    /// Build a REAL audit envelope from an actual run result and sign it,
    /// with an explicit run start time (from the engine session) so the
    /// envelope's Started/Completed timestamps reflect the REAL run, not
    /// envelope-construction time.
    pub fn build_from_run_at(
        execution_id: Uuid,
        status: ExecutionStatus,
        template_name: Option<String>,
        duration_ms: u64,
        steps: Vec<ExecutionStep>,
        hmac_key: Option<&str>,
        started_at: chrono::DateTime<chrono::Utc>,
    ) -> AuditEnvelope {
        let completed_at = started_at + chrono::Duration::milliseconds(duration_ms as i64);
        let envelope = AuditEnvelope::new(
            execution_id,
            status.clone(),
            template_name.clone(),
            started_at,
            completed_at,
            duration_ms,
            steps,
            None,
            String::new(),
            vec![],
        );

        let hmac = hmac_key.map(|key| compute_hmac(&envelope, key));
        match hmac {
            Some(sig) => AuditEnvelope::new(
                execution_id,
                status,
                template_name,
                started_at,
                completed_at,
                duration_ms,
                envelope.steps().to_vec(),
                None,
                sig,
                vec![],
            ),
            None => envelope,
        }
    }
}

impl Default for InMemoryAuditQueryService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuditQueryService for InMemoryAuditQueryService {
    async fn read_audit(&self, execution_id: &ExecutionId) -> Result<AuditEnvelope, AuditError> {
        let map = self
            .audits
            .read()
            .map_err(|e| AuditError::Internal(format!("Lock poisoned: {}", e)))?;
        map.get(execution_id.as_uuid())
            .cloned()
            .ok_or(AuditError::NotFound(*execution_id.as_uuid()))
    }

    async fn list_audits(&self, filter: AuditFilter) -> Result<Vec<AuditEnvelope>, AuditError> {
        let map = self
            .audits
            .read()
            .map_err(|e| AuditError::Internal(format!("Lock poisoned: {}", e)))?;

        let mut results: Vec<AuditEnvelope> = map
            .values()
            .filter(|a| {
                if let Some(status) = filter.status()
                    && a.status() != status
                {
                    return false;
                }
                if let Some(since) = filter.since()
                    && a.completed_at() < since
                {
                    return false;
                }
                if let Some(until) = filter.until()
                    && a.completed_at() > until
                {
                    return false;
                }
                if let Some(tn) = filter.template_name()
                    && a.template_name() != Some(tn)
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        // Sort by completion time, newest first
        results.sort_by(|a, b| b.completed_at().cmp(a.completed_at()));

        // Apply pagination
        let offset = filter.offset().unwrap_or(0);
        let results = results
            .into_iter()
            .skip(offset)
            .take(filter.limit())
            .collect();

        Ok(results)
    }

    async fn audit_summary(
        &self,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<AuditSummary, AuditError> {
        let map = self
            .audits
            .read()
            .map_err(|e| AuditError::Internal(format!("Lock poisoned: {}", e)))?;

        let filtered: Vec<&AuditEnvelope> = map
            .values()
            .filter(|a| a.completed_at() >= &since && a.completed_at() <= &until)
            .collect();

        let total_executions = filtered.len() as u64;
        let success_count = filtered
            .iter()
            .filter(|a| matches!(a.status(), ExecutionStatus::Completed))
            .count() as u64;
        let failure_count = filtered
            .iter()
            .filter(|a| matches!(a.status(), ExecutionStatus::Failed))
            .count() as u64;
        let success_rate = if total_executions > 0 {
            success_count as f64 / total_executions as f64
        } else {
            0.0
        };
        let total_duration_ms: u64 = filtered.iter().map(|a| a.duration_ms()).sum();
        let total_tokens: Option<u64> = {
            let sum: u64 = filtered.iter().filter_map(|a| a.tokens_used()).sum();
            if sum > 0 { Some(sum) } else { None }
        };

        // Compute top failures - group by template_name + status
        let mut failure_map: HashMap<String, u64> = HashMap::new();
        for a in &filtered {
            if matches!(a.status(), ExecutionStatus::Failed) {
                let key = a.template_name().unwrap_or("unknown").to_string();
                *failure_map.entry(key.clone()).or_insert(0) += 1;
            }
        }
        let mut top_failures: Vec<TopFailure> = failure_map
            .into_iter()
            .map(|(k, v)| TopFailure::new(format!("Failures for template '{}'", k), v, Some(k)))
            .collect();
        top_failures.sort_by_key(|b| std::cmp::Reverse(b.count()));
        top_failures.truncate(5);

        // Compute top templates
        let mut template_map: HashMap<String, (u64, u64)> = HashMap::new();
        for a in &filtered {
            let tn = a.template_name().unwrap_or("unknown").to_string();
            let entry = template_map.entry(tn).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += a.duration_ms();
        }
        let mut top_templates: Vec<TopTemplate> = template_map
            .into_iter()
            .map(|(name, (count, total_dur))| {
                TopTemplate::new(name, count, total_dur.checked_div(count).unwrap_or(0))
            })
            .collect();
        top_templates.sort_by_key(|b| std::cmp::Reverse(b.count()));
        top_templates.truncate(5);

        Ok(AuditSummary::new(
            since,
            until,
            total_executions,
            success_count,
            failure_count,
            success_rate,
            total_duration_ms,
            total_tokens,
            top_failures,
            top_templates,
        ))
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use crate::audit_tools::domain::entity::AuditQueryService;
    use crate::audit_tools::domain::value::AuditFilter;
    use crate::audit_tools::infrastructure::InMemoryAuditQueryService;
    use crate::execution_tools::domain::value::{ExecutionId, ExecutionStatus};

    #[tokio::test]
    async fn test_read_audit_found() {
        let svc = InMemoryAuditQueryService::new();
        let id = Uuid::new_v4();
        svc.store(InMemoryAuditQueryService::create_sample(
            id,
            ExecutionStatus::Completed,
            None,
            Utc::now(),
            100,
        ))
        .unwrap();
        let result = svc.read_audit(&ExecutionId::from_uuid(id)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().execution_id(), id);
    }

    #[tokio::test]
    async fn test_read_audit_not_found() {
        let svc = InMemoryAuditQueryService::new();
        let result = svc
            .read_audit(&ExecutionId::from_uuid(Uuid::new_v4()))
            .await;
        assert!(matches!(
            result.unwrap_err(),
            crate::audit_tools::domain::error::AuditError::NotFound(_)
        ));
    }

    #[tokio::test]
    async fn test_list_audits_filter_by_status() {
        let svc = InMemoryAuditQueryService::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        svc.store(InMemoryAuditQueryService::create_sample(
            id1,
            ExecutionStatus::Completed,
            None,
            Utc::now(),
            100,
        ))
        .unwrap();
        svc.store(InMemoryAuditQueryService::create_sample(
            id2,
            ExecutionStatus::Failed,
            None,
            Utc::now(),
            100,
        ))
        .unwrap();
        let filter =
            AuditFilter::with_all(Some(ExecutionStatus::Completed), None, None, None, 50, None);
        let results = svc.list_audits(filter).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].execution_id(), id1);
    }

    #[tokio::test]
    async fn test_audit_summary_counts() {
        let svc = InMemoryAuditQueryService::new();
        let now = Utc::now();
        for i in 0..5 {
            let status = if i < 3 {
                ExecutionStatus::Completed
            } else {
                ExecutionStatus::Failed
            };
            svc.store(InMemoryAuditQueryService::create_sample(
                Uuid::new_v4(),
                status,
                Some("t".into()),
                now - Duration::hours(i as i64),
                100,
            ))
            .unwrap();
        }
        let summary = svc
            .audit_summary(now - Duration::days(1), now + Duration::hours(1))
            .await
            .unwrap();
        assert_eq!(summary.total_executions(), 5);
        assert_eq!(summary.success_count(), 3);
        assert_eq!(summary.failure_count(), 2);
    }

    #[tokio::test]
    async fn test_audit_summary_empty_range() {
        let svc = InMemoryAuditQueryService::new();
        let now = Utc::now();
        let summary = svc
            .audit_summary(now - Duration::days(2), now - Duration::days(1))
            .await
            .unwrap();
        assert_eq!(summary.total_executions(), 0);
    }

    #[test]
    fn test_build_from_run_signs_real_steps() {
        // Honest-evidence regression: the envelope stored for read_audit must
        // carry the REAL steps that ran and a signature derived from them —
        // not the fabricated "sample-hmac" placeholder.
        use crate::audit_tools::domain::value::ExecutionStep;

        let id = Uuid::new_v4();
        let steps = vec![
            ExecutionStep::new("validate".into(), true, None, serde_json::json!({}), 5),
            ExecutionStep::new(
                "migrate".into(),
                false,
                Some("SQL error".into()),
                serde_json::json!({}),
                12,
            ),
        ];
        let env = InMemoryAuditQueryService::build_from_run(
            id,
            ExecutionStatus::Failed,
            Some("db-migration".into()),
            17,
            steps,
            Some("test-key"),
        );

        assert_ne!(env.hmac(), "sample-hmac", "must not be the placeholder");
        assert!(!env.hmac().is_empty(), "must carry a signature");
        assert_eq!(env.hmac().len(), 64, "SHA-256 hex digest is 64 chars");
        assert_eq!(env.steps().len(), 2);
        assert_eq!(env.steps()[0].step_name(), "validate");
        assert!(env.steps()[0].is_success());
        assert_eq!(env.steps()[1].error(), Some("SQL error"));

        // Same steps but different key → different signature.
        let other_key = InMemoryAuditQueryService::build_from_run(
            id,
            ExecutionStatus::Failed,
            Some("db-migration".into()),
            17,
            vec![
                ExecutionStep::new("validate".into(), true, None, serde_json::json!({}), 5),
                ExecutionStep::new(
                    "migrate".into(),
                    false,
                    Some("SQL error".into()),
                    serde_json::json!({}),
                    12,
                ),
            ],
            Some("other-key"),
        );
        assert_ne!(env.hmac(), other_key.hmac(), "different key must differ");
    }
}

/// Compute an HMAC-SHA256 signature over the envelope's canonical form,
/// mirroring the engine's `envelope_factory_impl::compute_signature` so the
/// MCP-facing envelope is verified with the same key used by the engine.
///
/// GAP-A-06: the signature covers the FULL serialized envelope (the hmac
/// field is empty at signing time), so tampering with any step/status/duration
/// field breaks the signature — previously only a scalar subset was signed.
fn compute_hmac(envelope: &AuditEnvelope, key: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let mut mac =
        Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC key from config is valid");
    // Canonical: full sorted-key serialization (serde_json::Map is
    // BTreeMap-backed without preserve_order). The hmac field is empty at
    // signing time, matching the envelope the caller signs before rebuilding.
    let canonical = serde_json::to_string(envelope).unwrap_or_default();
    mac.update(canonical.as_bytes());
    let result = mac.finalize().into_bytes();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

//! ExecutionHistory — outbound port over the signed audit trail (R7).
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#r7
//! Issue: #871 (R7 cross-run conflicting-action rules)
//!
//! R7 rules consult actions that COMPLETED in PRIOR runs. The port lives in
//! the module's infrastructure layer (same convention as the rule-config
//! `SequencePolicyRepository`); the default implementation reads the signed
//! envelope store (`.rigorix/audit`, `LocalAuditEnvelopeRepository`), so
//! policy input == signed evidence — tampering with the trail to evade a
//! rule breaks the envelope HMAC.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::audit::domain::envelope::AuditEnvelope;
use crate::sequence_policy::domain::{HistoryAction, SequencePolicyError};

/// Durable record of executed actions from PRIOR runs.
#[async_trait]
pub trait ExecutionHistory: Send + Sync {
    /// Return the actions completed since `since` (newest first).
    ///
    /// # Errors
    /// - A history read failure is fail-closed (`SequencePolicyError`) —
    ///   the run is refused rather than evaluated against partial history.
    /// - A missing history store is NOT an error: it yields an empty list
    ///   (no prior actions — status quo).
    async fn prior_actions(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<HistoryAction>, SequencePolicyError>;
}

/// Envelope-backed history: extracts per-node completions from the signed
/// envelopes persisted by the composition roots (`LocalAuditEnvelopeRepository`
/// under `<repo_root>/.rigorix/audit`).
///
/// - Node identity = the envelope's execution-event `step_name` payload
///   (stable template identifiers; parameter values stay redacted —
///   SpanPrivacy, never recovered into policy input).
/// - Principal = the envelope `author`, falling back to the attested
///   `identity.subject` when present.
pub struct EnvelopeHistoryAdapter {
    repo: std::sync::Arc<dyn crate::audit::infrastructure::repository::AuditEnvelopeRepository>,
}

impl EnvelopeHistoryAdapter {
    /// Create the adapter over the given envelope repository.
    pub fn new(
        repo: std::sync::Arc<dyn crate::audit::infrastructure::repository::AuditEnvelopeRepository>,
    ) -> Self {
        Self { repo }
    }
}

/// Extract the executed node names from one envelope's event refs.
///
/// Node-completion events carry `payload.step_name`; duplicate names within
/// one envelope collapse to the last occurrence (per-node completion).
fn actions_from_envelope(envelope: &AuditEnvelope) -> Vec<HistoryAction> {
    let principal = envelope
        .author
        .clone()
        .or_else(|| envelope.identity.as_ref().map(|i| i.subject.clone()));
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for ev in &envelope.events {
        let Some(payload) = &ev.payload else { continue };
        let Some(step_name) = payload.get("step_name").and_then(|v| v.as_str()) else {
            continue;
        };
        if seen.contains(&step_name.to_string()) {
            continue;
        }
        seen.push(step_name.to_string());
        out.push(HistoryAction {
            node: step_name.to_string(),
            principal: principal.clone(),
            at: ev.occurred_at,
        });
    }
    out
}

#[async_trait]
impl ExecutionHistory for EnvelopeHistoryAdapter {
    async fn prior_actions(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<HistoryAction>, SequencePolicyError> {
        match self.repo.list(Some(since), None, Some(500)).await {
            Ok(envelopes) => {
                let mut actions: Vec<HistoryAction> = Vec::new();
                for envelope in envelopes {
                    actions.extend(actions_from_envelope(&envelope));
                }
                Ok(actions)
            }
            Err(e) => {
                // A missing audit directory is the no-history case — status
                // quo, not an error. Anything else fails closed.
                if e.to_string().contains("No such file or directory")
                    || e.to_string().contains("failed to read directory")
                {
                    return Ok(Vec::new());
                }
                Err(SequencePolicyError::Internal(format!(
                    "history read failed: {e}"
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::application::dto::BuildEnvelopeInput;
    use crate::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
    use crate::audit::domain::envelope::{EventStatus, ExecutionEventRef};
    use crate::audit::infrastructure::local_audit_repository::LocalAuditEnvelopeRepository;
    use serde_json::json;

    fn completed_envelope_input(node: &str, author: &str) -> BuildEnvelopeInput {
        BuildEnvelopeInput {
            execution_id: uuid::Uuid::new_v4(),
            template_id: "attendance".to_string(),
            planning_prompt: "run".to_string(),
            events: vec![ExecutionEventRef {
                event_type: "node_completed".to_string(),
                summary: format!("{node} completed"),
                occurred_at: chrono::Utc::now(),
                correlation_id: None,
                status: EventStatus::Success,
                payload: Some(json!({ "step_name": node })),
            }],
            source: Some("rigorix_mcp".to_string()),
            total_tokens: 0,
            duration_ms: 10,
            git_commit: None,
            git_branch: None,
            model_version: None,
            planning_prompt_content: None,
            file_paths: vec![],
            metadata: None,
            scoring_results: std::collections::HashMap::new(),
            sign: false,
            repository: None,
            author: Some(author.to_string()),
            identity: None,
        }
    }

    /// R7 adapter over the REAL signed envelope store: an envelope written by
    /// the local repo surfaces as a prior HistoryAction with node + principal.
    #[tokio::test]
    async fn adapter_recovers_prior_actions_from_saved_envelopes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo: std::sync::Arc<
            dyn crate::audit::infrastructure::repository::AuditEnvelopeRepository,
        > = std::sync::Arc::new(LocalAuditEnvelopeRepository::new(dir.path().to_path_buf()));

        let factory = AuditEnvelopeFactoryImpl::default();
        // Trait method — bring the factory trait into scope.
        use crate::audit::application::factory::AuditEnvelopeFactory;
        let envelope = factory
            .build_envelope(completed_envelope_input("remove_attendance", "jeff@corp"))
            .await
            .expect("build");
        repo.save(&envelope).await.expect("save");

        let adapter = EnvelopeHistoryAdapter::new(repo.clone());
        let since = chrono::Utc::now() - chrono::Duration::hours(1);
        let actions = adapter.prior_actions(since).await.expect("history read");

        assert!(
            actions.iter().any(|a| {
                a.node == "remove_attendance" && a.principal.as_deref() == Some("jeff@corp")
            }),
            "prior actions must include the saved node + principal: {actions:?}"
        );
    }

    /// Missing audit directory = empty history (status quo), never an error.
    #[tokio::test]
    async fn adapter_missing_dir_is_empty_history() {
        let dir =
            std::env::temp_dir().join(format!("rigorix-r7-missing-{}/audit", uuid::Uuid::new_v4()));
        let repo: std::sync::Arc<
            dyn crate::audit::infrastructure::repository::AuditEnvelopeRepository,
        > = std::sync::Arc::new(LocalAuditEnvelopeRepository::new(dir));
        let adapter = EnvelopeHistoryAdapter::new(repo);
        let actions = adapter
            .prior_actions(chrono::Utc::now() - chrono::Duration::hours(1))
            .await
            .expect("missing dir must yield empty history");
        assert!(actions.is_empty());
    }
}

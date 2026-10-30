//! Concrete implementations of Audit Tools service traits.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#services
//! Implements: ReadAuditHandler, ListAuditsHandler, AuditSummaryHandler
//!
//! These are the concrete implementations that wire AuditQueryService with
//! AuditFormatter for the three audit use cases.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use std::str::FromStr;
use std::sync::Arc;

use crate::audit_tools::application::dto::{
    AuditSummaryInput, AuthorityFactsInput, ListAuditsInput, ReadAuditInput,
};
use crate::audit_tools::application::service::{
    AuditAuthorityFactsHandler, AuditSummaryHandler, ListAuditsHandler, ReadAuditHandler,
};
use crate::audit_tools::domain::entity::{AuditFormatter, SharedAuditQueryService};
use crate::audit_tools::domain::error::{AuditError, AuditHandlerError};
use crate::audit_tools::domain::value::{
    AuditEnvelope, AuditFilter, ExecutionEvent, ExecutionStep, PreconditionFinding,
    RequirementFinding, SequencePolicyFinding,
};
use crate::audit_tools::infrastructure::in_memory_audit_service::InMemoryAuditQueryService;
use crate::execution_tools::domain::error::ToolCallResult;
use crate::execution_tools::domain::value::ExecutionId;
use crate::execution_tools::domain::value::ExecutionStatus;

use rigorix_engine::audit::application::projection::ProjectAuthorityFacts;
use rigorix_engine::audit::domain::AuditEnvelope as EngineAuditEnvelope;
use rigorix_engine::audit::domain::authority_fact::AuthorityFactBundle;
use rigorix_engine::audit::infrastructure::repository::AuditEnvelopeRepository;

use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// ReadAuditHandlerImpl
// ---------------------------------------------------------------------------

/// Implementation of ReadAuditHandler.
pub struct ReadAuditHandlerImpl {
    query_service: SharedAuditQueryService,
    formatter: Arc<dyn AuditFormatter>,
    /// Persisted signed-envelope history (`<repo_root>/.rigorix/audit`). When
    /// the session index misses, this is the record a recipient receives.
    repository: Option<Arc<dyn AuditEnvelopeRepository>>,
}

impl ReadAuditHandlerImpl {
    /// Create a new ReadAuditHandlerImpl.
    pub fn new(query_service: SharedAuditQueryService, formatter: Arc<dyn AuditFormatter>) -> Self {
        Self {
            query_service,
            formatter,
            repository: None,
        }
    }

    /// Fall back to the on-disk signed trail when the session index misses.
    pub fn with_repository(mut self, repository: Arc<dyn AuditEnvelopeRepository>) -> Self {
        self.repository = Some(repository);
        self
    }
}

#[async_trait]
impl ReadAuditHandler for ReadAuditHandlerImpl {
    async fn handle(&self, input: ReadAuditInput) -> Result<ToolCallResult, AuditHandlerError> {
        // Parse execution_id from input string
        let uuid = uuid::Uuid::from_str(&input.execution_id)
            .map_err(|_| AuditHandlerError::InvalidArguments("execution_id".into()))?;
        let execution_id = ExecutionId::from_uuid(uuid);

        // The signed file is the artifact a recipient receives, so it is the
        // source of truth when present. The session index is a projection of
        // the same run and can be empty or stale (#1227, #1223). If the file is
        // absent (in-memory tests), fall back to the session index.
        let envelope = match &self.repository {
            Some(repository) => match repository.find_by_execution_id(&uuid).await.map_err(|e| {
                AuditHandlerError::AuditError(AuditError::EngineError(e.to_string()))
            })? {
                Some(envelope) => engine_envelope_to_mcp(&envelope),
                None => self
                    .query_service
                    .read_audit(&execution_id)
                    .await
                    .map_err(AuditHandlerError::AuditError)?,
            },
            None => self
                .query_service
                .read_audit(&execution_id)
                .await
                .map_err(AuditHandlerError::AuditError)?,
        };

        // Format the result
        let formatted = match input.format.as_deref() {
            Some("json") => {
                let value = self.formatter.format_audit_json(&envelope);
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
            }
            _ => self.formatter.format_audit_text(&envelope),
        };

        Ok(ToolCallResult {
            content: vec![crate::execution_tools::domain::error::ToolContentItem {
                r#type: "text".into(),
                text: formatted,
            }],
            is_error: false,
        })
    }
}

// ---------------------------------------------------------------------------
// ListAuditsHandlerImpl
// ---------------------------------------------------------------------------

/// Implementation of ListAuditsHandler.
pub struct ListAuditsHandlerImpl {
    query_service: SharedAuditQueryService,
    formatter: Arc<dyn AuditFormatter>,
}

impl ListAuditsHandlerImpl {
    /// Create a new ListAuditsHandlerImpl.
    pub fn new(query_service: SharedAuditQueryService, formatter: Arc<dyn AuditFormatter>) -> Self {
        Self {
            query_service,
            formatter,
        }
    }

    /// Parse optional status string into ExecutionStatus.
    fn parse_status(
        s: Option<&str>,
    ) -> Option<crate::execution_tools::domain::value::ExecutionStatus> {
        match s {
            Some("Completed") => {
                Some(crate::execution_tools::domain::value::ExecutionStatus::Completed)
            }
            Some("Failed") => Some(crate::execution_tools::domain::value::ExecutionStatus::Failed),
            Some("PartialFailed") => {
                Some(crate::execution_tools::domain::value::ExecutionStatus::PartialFailed)
            }
            Some("Cancelled") => {
                Some(crate::execution_tools::domain::value::ExecutionStatus::Cancelled)
            }
            Some("EnforcementBlocked") => {
                Some(crate::execution_tools::domain::value::ExecutionStatus::EnforcementBlocked)
            }
            _ => None,
        }
    }

    /// Parse optional datetime string.
    fn parse_datetime(s: Option<&str>) -> Result<Option<DateTime<Utc>>, AuditHandlerError> {
        match s {
            Some(s) => {
                let dt = DateTime::parse_from_rfc3339(s).map_err(|_| {
                    AuditHandlerError::InvalidArguments(format!("Invalid datetime: {}", s))
                })?;
                Ok(Some(dt.with_timezone(&Utc)))
            }
            None => Ok(None),
        }
    }
}

#[async_trait]
impl ListAuditsHandler for ListAuditsHandlerImpl {
    async fn handle(&self, input: ListAuditsInput) -> Result<ToolCallResult, AuditHandlerError> {
        let limit = input.limit.unwrap_or(50) as usize;
        if limit == 0 || limit > 200 {
            return Err(AuditHandlerError::InvalidArguments(
                "limit must be between 1 and 200".into(),
            ));
        }

        let filter = AuditFilter::with_all(
            Self::parse_status(input.status.as_deref()),
            Self::parse_datetime(input.since.as_deref())?,
            Self::parse_datetime(input.until.as_deref())?,
            input.template.clone(),
            limit,
            None,
        );

        let envelopes = self
            .query_service
            .list_audits(filter)
            .await
            .map_err(AuditHandlerError::AuditError)?;

        let formatted = self.formatter.format_list_text(&envelopes);

        Ok(ToolCallResult {
            content: vec![crate::execution_tools::domain::error::ToolContentItem {
                r#type: "text".into(),
                text: formatted,
            }],
            is_error: false,
        })
    }
}

/// Project an engine envelope into the MCP read model.
///
/// The engine envelope is the signed artifact; the MCP type is a presentation
/// projection (steps + normalized status). Keeping the mapping here means the
/// disk fallback returns the same shape as an in-session read.
fn engine_envelope_to_mcp(envelope: &EngineAuditEnvelope) -> AuditEnvelope {
    let completed_at = envelope.timestamp;
    let started_at = completed_at - Duration::milliseconds(envelope.duration_ms as i64);

    let status = engine_status(envelope);
    let steps = envelope
        .events
        .iter()
        .filter_map(engine_step)
        .collect::<Vec<_>>();
    let events = envelope
        .events
        .iter()
        .map(|e| {
            let status = match e.status {
                rigorix_engine::audit::domain::EventStatus::Success => {
                    crate::audit_tools::domain::value::EventStatus::Success
                }
                rigorix_engine::audit::domain::EventStatus::Failure => {
                    crate::audit_tools::domain::value::EventStatus::Failure
                }
                rigorix_engine::audit::domain::EventStatus::Skipped => {
                    crate::audit_tools::domain::value::EventStatus::Skipped
                }
                rigorix_engine::audit::domain::EventStatus::Cancelled => {
                    crate::audit_tools::domain::value::EventStatus::Cancelled
                }
            };
            ExecutionEvent::new(
                e.event_type.clone(),
                e.summary.clone(),
                e.occurred_at,
                e.correlation_id,
                status,
            )
        })
        .collect();

    AuditEnvelope::new_with_findings(
        envelope.execution_id,
        status,
        Some(envelope.template_id.clone()),
        started_at,
        completed_at,
        envelope.duration_ms,
        steps,
        (envelope.total_tokens > 0).then_some(envelope.total_tokens as u64),
        envelope.signature.clone().unwrap_or_default(),
        events,
        envelope
            .precondition_findings
            .iter()
            .map(|f| {
                PreconditionFinding::new(
                    f.precondition_id.clone(),
                    f.step.clone(),
                    f.outcome.clone(),
                    f.exit_code,
                    f.inputs_hash.clone(),
                    f.checked_at,
                    f.summary.clone(),
                    f.check_digest.clone(),
                    f.authority_digest.clone(),
                    f.check_writable,
                    f.attribution.clone(),
                )
            })
            .collect(),
        envelope
            .sequence_policy_findings
            .iter()
            .map(|f| {
                SequencePolicyFinding::new(
                    f.rule_id.clone(),
                    f.action.clone(),
                    f.later_step.clone(),
                    f.matched_indices.clone(),
                    f.summary.clone(),
                )
            })
            .collect(),
        envelope
            .requirement_findings
            .iter()
            .map(|f| {
                RequirementFinding::new(
                    f.requirement_id.clone(),
                    f.step.clone(),
                    f.action.clone(),
                    f.unmet.clone(),
                    f.summary.clone(),
                )
            })
            .collect(),
    )
}

fn engine_status(envelope: &EngineAuditEnvelope) -> ExecutionStatus {
    let has = |name: &str| envelope.events.iter().any(|e| e.event_type == name);
    if has("execution_failed") {
        ExecutionStatus::Failed
    } else if has("execution_cancelled") {
        ExecutionStatus::Cancelled
    } else if has("execution_completed") {
        ExecutionStatus::Completed
    } else if envelope
        .events
        .iter()
        .any(|e| e.event_type == "node_failed")
    {
        ExecutionStatus::Failed
    } else {
        ExecutionStatus::Completed
    }
}

fn engine_step(event: &rigorix_engine::audit::domain::ExecutionEventRef) -> Option<ExecutionStep> {
    if event.event_type != "node_completed" && event.event_type != "node_failed" {
        return None;
    }
    let payload = event.payload.as_ref();
    let name = payload
        .and_then(|p| p.get("step_name").or_else(|| p.get("node_name")))
        .and_then(|v| v.as_str())
        .unwrap_or(&event.summary)
        .to_string();
    let success = event.event_type == "node_completed";
    let error = (!success)
        .then(|| {
            payload
                .and_then(|p| p.get("error"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .flatten();
    let output = payload
        .and_then(|p| p.get("output"))
        .cloned()
        .unwrap_or_default();
    let duration_ms = payload
        .and_then(|p| p.get("duration_ms"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Some(ExecutionStep::new(
        name,
        success,
        error,
        output,
        duration_ms,
    ))
}

// ---------------------------------------------------------------------------
// AuditSummaryHandlerImpl
// ---------------------------------------------------------------------------

/// Implementation of AuditSummaryHandler.
pub struct AuditSummaryHandlerImpl {
    query_service: SharedAuditQueryService,
    formatter: Arc<dyn AuditFormatter>,
    /// Persisted signed trail. When present it is the source of truth, so a
    /// fresh process summarizes the 14 records on disk, not its empty session.
    repository: Option<Arc<dyn AuditEnvelopeRepository>>,
}

impl AuditSummaryHandlerImpl {
    /// Create a new AuditSummaryHandlerImpl.
    pub fn new(query_service: SharedAuditQueryService, formatter: Arc<dyn AuditFormatter>) -> Self {
        Self {
            query_service,
            formatter,
            repository: None,
        }
    }

    /// Count the persisted trail rather than the session index.
    pub fn with_repository(mut self, repository: Arc<dyn AuditEnvelopeRepository>) -> Self {
        self.repository = Some(repository);
        self
    }
}

#[async_trait]
impl AuditSummaryHandler for AuditSummaryHandlerImpl {
    async fn handle(&self, input: AuditSummaryInput) -> Result<ToolCallResult, AuditHandlerError> {
        let now = Utc::now();

        let since = match input.since {
            Some(ref s) => DateTime::parse_from_rfc3339(s)
                .map_err(|_| AuditHandlerError::InvalidArguments(format!("Invalid since: {}", s)))?
                .with_timezone(&Utc),
            None => now - Duration::days(7),
        };

        let until = match input.until {
            Some(ref s) => DateTime::parse_from_rfc3339(s)
                .map_err(|_| AuditHandlerError::InvalidArguments(format!("Invalid until: {}", s)))?
                .with_timezone(&Utc),
            None => now,
        };

        if since >= until {
            return Err(AuditHandlerError::InvalidArguments(
                "since must be before until".into(),
            ));
        }

        let summary = match &self.repository {
            Some(repository) => {
                let envelopes = repository
                    .list(Some(since), Some(until), None)
                    .await
                    .map_err(|e| {
                        AuditHandlerError::AuditError(AuditError::EngineError(e.to_string()))
                    })?;
                let projected: Vec<AuditEnvelope> =
                    envelopes.iter().map(engine_envelope_to_mcp).collect();
                let refs: Vec<&AuditEnvelope> = projected.iter().collect();
                InMemoryAuditQueryService::summarize(&refs, since, until)
            }
            None => self
                .query_service
                .audit_summary(since, until)
                .await
                .map_err(AuditHandlerError::AuditError)?,
        };

        // `rigorix_audit_summary` is a structured tool: the host parses this
        // text as JSON. Returning the markdown summary made that parse fail and
        // `unwrap_or_default()` turn it into `null` (#1224).
        let formatted = serde_json::to_string(&self.formatter.format_summary_json(&summary))
            .unwrap_or_else(|_| "{}".to_string());

        Ok(ToolCallResult {
            content: vec![crate::execution_tools::domain::error::ToolContentItem {
                r#type: "text".into(),
                text: formatted,
            }],
            is_error: false,
        })
    }
}

// ---------------------------------------------------------------------------
// AuditAuthorityFactsHandlerImpl
// ---------------------------------------------------------------------------

/// Projects signed audit history into a bundle a third party can verify offline.
///
/// Exposes the ENGINE's derivation (`ProjectAuthorityFacts`) verbatim. A second
/// derivation here would be a second dialect, and the two would drift.
pub struct AuditAuthorityFactsHandlerImpl {
    repository: Arc<dyn AuditEnvelopeRepository>,
}

impl AuditAuthorityFactsHandlerImpl {
    /// Create a handler over the signed-envelope history.
    pub fn new(repository: Arc<dyn AuditEnvelopeRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl AuditAuthorityFactsHandler for AuditAuthorityFactsHandlerImpl {
    async fn handle(
        &self,
        input: AuthorityFactsInput,
    ) -> Result<ToolCallResult, AuditHandlerError> {
        if input.subjects.is_empty() {
            return Err(AuditHandlerError::InvalidArguments(
                "subjects must not be empty: a projection with no subjects carries no attestation"
                    .into(),
            ));
        }

        // A bundle must carry a COMPLETE, verifiable chain, so ask for the whole
        // store rather than the repository's default window. A truncated history
        // would re-derive to different facts (and a broken chain).
        let mut envelopes = self
            .repository
            .list(None, None, Some(u32::MAX))
            .await
            .map_err(|e| {
                AuditHandlerError::Internal(format!("failed to read audit history: {e}"))
            })?;

        // `producer_id` is a scope filter on the transport contract, applied here
        // so the engine's derivation signature stays free of a transport concern.
        if let Some(producer) = input.producer_id.as_deref() {
            envelopes.retain(|envelope| envelope.producer_id.as_deref() == Some(producer));
        }

        // Which of the requested subjects actually have a record? Project once
        // with no absences and read the answered subjects off the result, rather
        // than re-implementing the subject formatting here (a second dialect).
        let answered = ProjectAuthorityFacts::project(&envelopes, &[]);
        let recorded: BTreeSet<&str> = answered
            .facts
            .iter()
            .map(|fact| fact.subject.as_str())
            .collect();

        let subjects_without_record: Vec<String> = input
            .subjects
            .iter()
            .filter(|subject| !recorded.contains(subject.as_str()))
            .cloned()
            .collect();

        let bundle = AuthorityFactBundle {
            projection: ProjectAuthorityFacts::project(&envelopes, &subjects_without_record),
            envelopes,
            subjects_without_record,
        };

        let text = serde_json::to_string(&bundle).map_err(|e| {
            AuditHandlerError::Internal(format!("failed to serialize authority-fact bundle: {e}"))
        })?;

        Ok(ToolCallResult {
            content: vec![crate::execution_tools::domain::error::ToolContentItem {
                r#type: "text".into(),
                text,
            }],
            is_error: false,
        })
    }
}

// ---------------------------------------------------------------------------
// Factory function
// ---------------------------------------------------------------------------

/// Create all three audit handler instances from shared dependencies.
pub fn create_audit_handler_instances(
    query_service: SharedAuditQueryService,
    formatter: Arc<dyn AuditFormatter>,
) -> AuditHandlerInstanceSet {
    AuditHandlerInstanceSet {
        read_audit: Arc::new(ReadAuditHandlerImpl::new(
            query_service.clone(),
            formatter.clone(),
        )),
        list_audits: Arc::new(ListAuditsHandlerImpl::new(
            query_service.clone(),
            formatter.clone(),
        )),
        audit_summary: Arc::new(AuditSummaryHandlerImpl::new(query_service, formatter)),
    }
}

/// Set of all audit handler instances.
pub struct AuditHandlerInstanceSet {
    /// Read audit handler.
    pub read_audit: Arc<dyn ReadAuditHandler>,
    /// List audits handler.
    pub list_audits: Arc<dyn ListAuditsHandler>,
    /// Audit summary handler.
    pub audit_summary: Arc<dyn AuditSummaryHandler>,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::audit_tools::application::dto::{
        AuditSummaryInput, ListAuditsInput, ReadAuditInput,
    };
    use crate::audit_tools::application::service::{
        AuditSummaryHandler, ListAuditsHandler, ReadAuditHandler,
    };
    use crate::audit_tools::application::service_impl::{
        AuditSummaryHandlerImpl, ListAuditsHandlerImpl, ReadAuditHandlerImpl,
    };
    use crate::audit_tools::domain::formatter_impl::AuditFormatterImpl;
    use crate::audit_tools::infrastructure::InMemoryAuditQueryService;
    use crate::execution_tools::domain::value::ExecutionStatus;

    #[tokio::test]
    async fn test_read_audit_handler_returns_text() {
        let svc = Arc::new(InMemoryAuditQueryService::new());
        let id = Uuid::new_v4();
        svc.store(InMemoryAuditQueryService::create_sample(
            id,
            ExecutionStatus::Completed,
            Some("t".into()),
            chrono::Utc::now(),
            100,
        ))
        .unwrap();
        let formatter = Arc::new(AuditFormatterImpl::new());
        let handler = ReadAuditHandlerImpl::new(svc, formatter);
        let result = handler
            .handle(ReadAuditInput {
                execution_id: id.to_string(),
                format: Some("text".into()),
            })
            .await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_error);
    }

    #[tokio::test]
    async fn test_read_audit_handler_invalid_id() {
        let svc = Arc::new(InMemoryAuditQueryService::new());
        let formatter = Arc::new(AuditFormatterImpl::new());
        let handler = ReadAuditHandlerImpl::new(svc, formatter);
        let result = handler
            .handle(ReadAuditInput {
                execution_id: "bad-id".into(),
                format: None,
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_audits_handler_empty() {
        let svc = Arc::new(InMemoryAuditQueryService::new());
        let formatter = Arc::new(AuditFormatterImpl::new());
        let handler = ListAuditsHandlerImpl::new(svc, formatter);
        let result = handler
            .handle(ListAuditsInput {
                status: None,
                since: None,
                until: None,
                template: None,
                limit: Some(50),
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_audits_handler_invalid_limit() {
        let svc = Arc::new(InMemoryAuditQueryService::new());
        let formatter = Arc::new(AuditFormatterImpl::new());
        let handler = ListAuditsHandlerImpl::new(svc, formatter);
        let result = handler
            .handle(ListAuditsInput {
                status: None,
                since: None,
                until: None,
                template: None,
                limit: Some(0),
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_audit_summary_handler_default_range() {
        let svc = Arc::new(InMemoryAuditQueryService::new());
        let formatter = Arc::new(AuditFormatterImpl::new());
        let handler = AuditSummaryHandlerImpl::new(svc, formatter);
        let result = handler
            .handle(AuditSummaryInput {
                since: None,
                until: None,
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_audit_summary_handler_invalid_date() {
        let svc = Arc::new(InMemoryAuditQueryService::new());
        let formatter = Arc::new(AuditFormatterImpl::new());
        let handler = AuditSummaryHandlerImpl::new(svc, formatter);
        let result = handler
            .handle(AuditSummaryInput {
                since: Some("bad-date".into()),
                until: None,
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_audit_summary_handler_with_data() {
        let svc = Arc::new(InMemoryAuditQueryService::new());
        let now = chrono::Utc::now();
        for i in 0..3 {
            svc.store(InMemoryAuditQueryService::create_sample(
                Uuid::new_v4(),
                ExecutionStatus::Completed,
                Some("t".into()),
                now - chrono::Duration::hours(i as i64),
                100,
            ))
            .unwrap();
        }
        let formatter = Arc::new(AuditFormatterImpl::new());
        let handler = AuditSummaryHandlerImpl::new(svc, formatter);
        let since = (now - chrono::Duration::days(7))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        let until = (now + chrono::Duration::hours(1))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        let result = handler
            .handle(AuditSummaryInput {
                since: Some(since),
                until: Some(until),
            })
            .await;
        let result = result.unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("summary must be JSON");
        assert_eq!(json["total_executions"], 3);
        assert_eq!(json["success_count"], 3);
        assert_eq!(json["failure_count"], 0);
    }

    #[tokio::test]
    async fn test_audit_summary_handler_empty_trail_is_zeroes_not_null() {
        let svc = Arc::new(InMemoryAuditQueryService::new());
        let formatter = Arc::new(AuditFormatterImpl::new());
        let handler = AuditSummaryHandlerImpl::new(svc, formatter);
        let result = handler
            .handle(AuditSummaryInput {
                since: None,
                until: None,
            })
            .await
            .expect("an empty trail is a valid summary");
        let json: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("summary must be JSON");
        assert_eq!(json["total_executions"], 0);
        assert_eq!(json["success_count"], 0);
        assert_eq!(json["failure_count"], 0);
        assert_eq!(json["success_rate"], 0.0);
    }
}

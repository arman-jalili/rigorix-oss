//! The envelope step — where a decision becomes a record.
//!
//! @canonical .pi/architecture/modules/gate.md
//! Issue: #1389 (Phase C.1b)
//!
//! # Why the engine assembles it
//!
//! `GateDecision::envelope` carries a digest over `{verdict, findings}` and nothing else,
//! because a gate cannot know a run's template, prompt, authors or token totals. The first
//! cut left the rest to the caller, which was right about the *facts* and wrong about the
//! *assembly*: two runtimes that each build their own `BuildEnvelopeInput` produce the same
//! digest over different signed bytes, and the divergence shows up in a signature rather
//! than in a test.
//!
//! So the caller supplies [`RunContext`] — facts it has and the gate does not — and this
//! module assembles and signs with the same `AuditEnvelopeFactoryImpl` the rest of the
//! engine uses. The bytes come from `canonical_envelope_bytes`, so the recorded bytes are
//! the ones a signature covers, not a re-encoding (ADR-010).

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::audit::application::dto::{BuildEnvelopeInput, PinnedInputs};
use crate::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use crate::audit::application::factory::AuditEnvelopeFactory;
use crate::audit::domain::{EventStatus, ExecutionEventRef};

use super::request::RunContext;
use crate::gate::domain::{GateEnvelopeEvidence, GateError, GateFinding, GateVerdict, GatedStep};

/// Assemble and sign the decision's record.
///
/// # Errors
/// - [`GateError::Envelope`] when the factory refuses the input or signing fails.
pub(super) async fn decision_evidence(
    execution_id: Uuid,
    verdict: &GateVerdict,
    findings: &[GateFinding],
    step: &GatedStep,
    action: &str,
    pins: &PinnedInputs,
    context: RunContext,
) -> Result<GateEnvelopeEvidence, GateError> {
    let envelope_input = BuildEnvelopeInput {
        execution_id,
        template_id: context.template_id,
        planning_prompt: context.planning_prompt,
        events: vec![decision_event(verdict, findings, step, action, pins)],
        source: context.source,
        repository: context.repository,
        author: context.author,
        identity: context.identity,
        effect_key: None,
        producer_id: None,
        sequence: None,
        prev_hash: None,
        history_policy: None,
        total_tokens: context.total_tokens,
        duration_ms: context.duration_ms,
        git_commit: context.git_commit,
        git_branch: context.git_branch,
        model_version: None,
        planning_prompt_content: None,
        file_paths: Vec::new(),
        metadata: None,
        // An unsigned envelope is a record without an attestation, not a forged one.
        sign: context.signing_key.is_some(),
        scoring_results: std::collections::HashMap::new(),
    };

    let factory = AuditEnvelopeFactoryImpl::new(
        context
            .signing_key
            .as_ref()
            .map(|key| key.expose().to_string()),
    )
    .with_pinned_inputs(pins.clone());

    let envelope = factory
        .build_envelope(envelope_input)
        .await
        .map_err(|error| GateError::Envelope {
            detail: error.to_string(),
        })?;
    let canonical_bytes =
        AuditEnvelopeFactoryImpl::canonical_envelope_bytes(&envelope).map_err(|error| {
            GateError::Envelope {
                detail: error.to_string(),
            }
        })?;
    let envelope_hash = AuditEnvelopeFactoryImpl::envelope_hash(&envelope).map_err(|error| {
        GateError::Envelope {
            detail: error.to_string(),
        }
    })?;

    Ok(GateEnvelopeEvidence {
        envelope,
        canonical_bytes,
        envelope_hash,
    })
}

/// The decision as one event: what was asked, and what the gate said.
///
/// The instant is the pinned one, so a replay at a different instant produces the same
/// record — the property `PinnedInputs` exists for.
fn decision_event(
    verdict: &GateVerdict,
    findings: &[GateFinding],
    step: &GatedStep,
    action: &str,
    pins: &PinnedInputs,
) -> ExecutionEventRef {
    let (event_type, status, summary) = match verdict {
        GateVerdict::Dispatch => (
            "gate_dispatched",
            EventStatus::Success,
            format!(
                "the gate permitted '{}' ({}: {action}) after {} finding(s)",
                step.name,
                step.tool,
                findings.len()
            ),
        ),
        // A promotion is a decision that has not been made yet: `Skipped` is the closest
        // status that is neither "it happened" nor "it failed" — the event records that the
        // gate stopped here and a human must continue.
        GateVerdict::Promote {
            gate,
            rule_id,
            reason,
        } => (
            "gate_promoted",
            EventStatus::Skipped,
            format!(
                "the gate promoted '{}' ({}: {action}) at {}: {reason} [{rule_id}] — awaiting approval",
                step.name,
                step.tool,
                gate.as_str()
            ),
        ),
        GateVerdict::Refuse {
            gate,
            rule_id,
            reason,
        } => (
            "gate_refused",
            EventStatus::Failure,
            format!(
                "the gate refused '{}' ({}: {action}) at {}: {reason}{}",
                step.name,
                step.tool,
                gate.as_str(),
                rule_id
                    .as_ref()
                    .map(|id| format!(" [{id}]"))
                    .unwrap_or_default()
            ),
        ),
    };

    ExecutionEventRef {
        event_type: event_type.to_string(),
        summary,
        occurred_at: instant(pins),
        correlation_id: None,
        status,
        payload: None,
    }
}

/// The instant the decision is recorded at: the pinned one, or the fixed fallback.
///
/// A wall-clock read here would make two replays of one decision differ, which is the
/// defect `PinnedInputs` was introduced to remove (ADR-014 §2).
fn instant(pins: &PinnedInputs) -> DateTime<Utc> {
    pins.timestamp().unwrap_or(DateTime::<Utc>::MIN_UTC)
}

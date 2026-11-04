//! The M2 operation: one precondition refusal, reduced to the envelope a host signs.
//!
//! @canonical .pi/architecture/decisions/ADR-010-canonical-serialization.md
//! Implements: ISSUE-M2-CLI-HALF-PINNED-INPUTS (#1355) — "one refusal, one envelope, two runtimes"
//!
//! # Why this function exists at all
//!
//! The measurement is "the CLI and the crate produce the same bytes for the same refusal".
//! That claim is only about the *engine* if both sides build the input in exactly one place.
//! Two hand-written fixtures would compare two fixtures, and a drift between them would be
//! reported as an engine difference. So the operation lives here, the CLI calls it, the
//! in-crate example calls it, and the only thing that can differ between the two digests is
//! the runtime.
//!
//! It performs no I/O, reads no clock and mints no id: every value comes from the argument
//! or from [`PinnedInputs`]. A caller that supplies nothing gets a refusal envelope stamped
//! with the wall clock — today's behaviour — which is what the unpinned control measures.
//!
//! # Scope
//!
//! One refusal, not a run. It does not reach the orchestrator, the planner or the LLM; the
//! refusal is a fact of this fixture, recorded the way a real precondition refusal would be.

use chrono::{DateTime, NaiveDate, Utc};

use crate::audit::application::dto::{BuildEnvelopeInput, PinnedInputs};
use crate::audit::domain::{EventStatus, ExecutionEventRef};

/// The instant the fixture's *event* happened, when nothing is pinned.
///
/// A fixed value, deliberately: the event is part of the record's content, not of the
/// runtime's state, so it is pinned by construction rather than by the caller.
///
/// 2026-11-03T12:39:59Z — the value that shipped in 24e6dd6f and therefore the one the golden
/// digest in `tests/unit/pinnedinputs` pins. It carries no meaning beyond being fixed; what
/// matters is that this construction and the digest agree, because a fixture whose bytes move
/// silently moves every documented witness with it.
#[must_use]
fn event_instant() -> DateTime<Utc> {
    // Written as the instant it *is*, not as a hand-computed epoch. #1357 surfaced the
    // difference: this value shipped as `DateTime::from_timestamp(1_793_709_599, 0)` under a
    // comment saying 11:59:59Z, and that epoch is 12:39:59Z — forty minutes later than its own
    // documentation, which moved the signed bytes without changing their length. The instant is
    // arbitrary fixture content, so the value that shipped is kept and the construction now
    // names it; the date/time parts are checked at runtime and the fallback is unreachable,
    // because a fixture that panics is a fixture the panic-surface gate rightly rejects.
    NaiveDate::from_ymd_opt(2026, 11, 3)
        .and_then(|date| date.and_hms_opt(12, 39, 59))
        .map(|naive| naive.and_utc())
        .unwrap_or(DateTime::<Utc>::MIN_UTC)
}

/// The execution id the fixture uses when the caller does not pin one.
#[must_use]
fn default_execution_id() -> uuid::Uuid {
    // Compile-time checked: a bad literal is a build error, not a runtime panic.
    uuid::uuid!("018f5b8e-0000-7000-8000-000000000001")
}

/// Build the inputs for one precondition refusal.
///
/// The pins are applied here rather than in the factory so that an unpinned caller gets a
/// stamped-wall-clock envelope and a pinned one gets the pinned instant — and so that the
/// two runtimes cannot disagree about which field is pinned.
#[must_use]
pub fn refusal_envelope_input(pinned: &PinnedInputs) -> BuildEnvelopeInput {
    BuildEnvelopeInput {
        execution_id: pinned.execution_id().unwrap_or_else(default_execution_id),
        template_id: "m2-precondition-refusal".to_string(),
        planning_prompt: "refuse the step: precondition beneficiary-eligible failed".to_string(),
        events: vec![ExecutionEventRef {
            event_type: "precondition_refused".to_string(),
            summary: "precondition 'beneficiary-eligible' refused the step".to_string(),
            occurred_at: event_instant(),
            correlation_id: None,
            status: EventStatus::Failure,
            payload: None,
        }],
        source: Some("m2-pinned".to_string()),
        repository: Some("arman-jalili/rigorix-oss".to_string()),
        author: Some("m2".to_string()),
        identity: None,
        effect_key: None,
        producer_id: Some("m2-producer".to_string()),
        sequence: Some(1),
        prev_hash: None,
        history_policy: None,
        total_tokens: 0,
        duration_ms: 7,
        git_commit: Some("0000000000000000000000000000000000000000".to_string()),
        git_branch: Some("feature/portable-slice-followups".to_string()),
        model_version: None,
        planning_prompt_content: None,
        file_paths: vec!["engine/src/precondition/application/service_impl.rs".to_string()],
        metadata: None,
        sign: true,
        scoring_results: std::collections::HashMap::new(),
    }
}

/// Parse the `--pinned-clock` form: an RFC 3339 instant.
///
/// Rejected loudly rather than defaulted: a measurement whose pin silently did not apply is
/// an unpinned measurement wearing a pinned label.
///
/// # Errors
/// Returns the parse error's message when the value is not RFC 3339.
pub fn parse_pinned_clock(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.with_timezone(&Utc))
        .map_err(|error| format!("`{value}` is not an RFC 3339 instant: {error}"))
}

/// Parse the `--pinned-ids` form: a single execution id.
///
/// # Errors
/// Returns the parse error's message when the value is not a UUID.
pub fn parse_pinned_id(value: &str) -> Result<uuid::Uuid, String> {
    uuid::Uuid::parse_str(value).map_err(|error| format!("`{value}` is not a UUID: {error}"))
}

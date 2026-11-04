//! Pinned inputs — the values an envelope cannot derive and a caller must supply.
//!
//! @canonical .pi/architecture/decisions/ADR-010-canonical-serialization.md
//! Implements: ISSUE-M2-CLI-HALF-PINNED-INPUTS (#1355)
//!
//! # Why this is a type and not a field
//!
//! The spike added `BuildEnvelopeInput::pinned_timestamp` — a public field on a public,
//! literally constructible DTO. Two other crates in this workspace construct that DTO
//! literally (`server/tests/audit_scope_check_verify.rs`,
//! `mcp/src/host/mod.rs`), so a new field is a **breaking change** under Rust's semver
//! rules: it compiles here and breaks a downstream user who never appears in this
//! workspace's test run. Making the struct `#[non_exhaustive]` would break those
//! literals harder.
//!
//! So the pins travel in their own additive type, through an additive method
//! ([`AuditEnvelopeFactoryImpl::with_pinned_inputs`]): no existing public item gains a
//! field, no existing signature changes, and nothing that compiled before needs an edit.
//!
//! # What it pins, and why only this
//!
//! The measured operation — a precondition refusal reduced to a signed envelope —
//! traverses exactly **one** value it cannot derive: the envelope's own `timestamp`.
//! The execution id is already an input of the envelope. Everything else (event
//! timestamps, source, repository, author) the caller supplies as it always has.
//!
//! That count is the spike's most useful finding for planning: the envelope builder is
//! nearly deterministic already, and the 94 `Utc::now()` / 57 `Uuid::new_v4()` sites in
//! the gate modules belong to its *callers*. A `Clock`/`IdSource` trait pair across the
//! engine would be an abstraction the evidence does not support: one operation, one site.
//!
//! [`AuditEnvelopeFactoryImpl::with_pinned_inputs`]: super::envelope_factory_impl::AuditEnvelopeFactoryImpl::with_pinned_inputs

use chrono::{DateTime, Utc};

/// Values a caller pins so the same operation produces the same bytes.
///
/// Every field is optional and `None` means "derive it the way this build always has" —
/// so an unpinned build behaves exactly as before, and a pinned one differs only where
/// it was told to.
///
/// # Contract
/// - Adding a field is additive: this type is `#[non_exhaustive]` from day one, so the
///   next pin does not repeat the mistake that produced it
/// - Nothing here is a secret; a signing key travels separately
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PinnedInputs {
    timestamp: Option<DateTime<Utc>>,
    execution_id: Option<uuid::Uuid>,
}

impl PinnedInputs {
    /// No pins: every derived value is derived, i.e. today's behaviour.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Pin the instant stamped into the envelope.
    ///
    /// The envelope's `timestamp` is the one value in the operation a caller cannot
    /// otherwise supply, and it is signed — so a wall-clock read makes two identical
    /// decisions produce different bytes.
    #[must_use]
    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Pin the execution id, for a caller that would otherwise mint one.
    #[must_use]
    pub fn with_execution_id(mut self, execution_id: uuid::Uuid) -> Self {
        self.execution_id = Some(execution_id);
        self
    }

    /// The pinned instant, if any.
    #[must_use]
    pub fn timestamp(&self) -> Option<DateTime<Utc>> {
        self.timestamp
    }

    /// The pinned execution id, if any.
    #[must_use]
    pub fn execution_id(&self) -> Option<uuid::Uuid> {
        self.execution_id
    }

    /// Whether anything is pinned at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.timestamp.is_none() && self.execution_id.is_none()
    }
}

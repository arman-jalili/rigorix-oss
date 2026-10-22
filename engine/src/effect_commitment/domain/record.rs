//! EffectRecord — the aggregate for one effect's commitment.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectrecord
//! Implements: ISSUE-CONTRACT-FREEZE — EffectRecord aggregate + transitions
//! Issue: #1113
//! ADR-019: normative state machine and fencing rules
//!
//! The aggregate holds the intent, the current state, the observed outcome when
//! one exists, and the resolver identity that settled it.
//!
//! # Contract (Frozen)
//! - A record is created `Dispatched` by [`EffectRecord::begin`]
//! - `settle_definite` only applies to `Dispatched`
//! - `mark_ambiguous` only applies to `Dispatched`
//! - `resolve` only applies to `Ambiguous`, and requires both evidence and a
//!   resolver
//! - A terminal record **always** carries an `ObservedOutcome`; terminal state
//!   without evidence is unrepresentable through the public constructors
//! - Fields are private so the invariant cannot be bypassed by struct literals;
//!   adapters hydrate through [`EffectRecord::hydrate`]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    CommitmentError, EffectIntent, EffectQuery, EffectState, ObservedOutcome, ResolverIdentity,
};

/// Stable identity of an effect record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EffectRecordId(Uuid);

impl EffectRecordId {
    /// Generate a fresh, time-ordered record identity.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wrap an existing UUID (adapter hydration).
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// The underlying UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for EffectRecordId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EffectRecordId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The aggregate: one proposed effect and what is known about its commitment.
///
/// # Fields (Frozen, read via accessors)
/// - `id: EffectRecordId` — stable identity
/// - `intent: EffectIntent` — the write-ahead intent
/// - `state: EffectState` — current lifecycle state
/// - `outcome: Option<ObservedOutcome>` — provider evidence, present iff terminal
/// - `resolver: Option<ResolverIdentity>` — who resolved an `Ambiguous` effect
/// - `ambiguous_reason: Option<String>` — why the outcome became unknown
/// - `updated_at: DateTime<Utc>` — last transition timestamp
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectRecord {
    id: EffectRecordId,
    intent: EffectIntent,
    state: EffectState,
    outcome: Option<ObservedOutcome>,
    resolver: Option<ResolverIdentity>,
    ambiguous_reason: Option<String>,
    updated_at: DateTime<Utc>,
}

impl EffectRecord {
    /// Record a durable intent immediately before dispatch.
    ///
    /// This is the write-ahead entry point. If the ledger cannot persist it,
    /// the caller must not send the effect.
    pub fn begin(intent: EffectIntent) -> Self {
        Self {
            id: EffectRecordId::new(),
            intent,
            state: EffectState::Dispatched,
            outcome: None,
            resolver: None,
            ambiguous_reason: None,
            updated_at: Utc::now(),
        }
    }

    /// Reconstitute a record from persisted parts, enforcing the frozen
    /// invariants.
    ///
    /// # Errors
    /// - [`CommitmentError::EvidenceMissing`] when a terminal state has no
    ///   observed outcome
    /// - [`CommitmentError::Contended`] when the parts are internally
    ///   inconsistent (e.g. `Ambiguous` carrying evidence, or a resolver without
    ///   a terminal outcome)
    pub fn hydrate(
        id: EffectRecordId,
        intent: EffectIntent,
        state: EffectState,
        outcome: Option<ObservedOutcome>,
        resolver: Option<ResolverIdentity>,
        ambiguous_reason: Option<String>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self, CommitmentError> {
        if state.is_terminal() {
            if outcome.is_none() {
                return Err(CommitmentError::EvidenceMissing { record_id: id });
            }
        } else {
            if outcome.is_some() {
                return Err(CommitmentError::Contended {
                    record_id: Some(id),
                    detail: format!(
                        "non-terminal state '{state}' must not carry an observed outcome"
                    ),
                });
            }
            if resolver.is_some() {
                return Err(CommitmentError::Contended {
                    record_id: Some(id),
                    detail: format!("non-terminal state '{state}' must not carry a resolver"),
                });
            }
        }
        Ok(Self {
            id,
            intent,
            state,
            outcome,
            resolver,
            ambiguous_reason,
            updated_at,
        })
    }

    /// Stable identity.
    pub fn id(&self) -> EffectRecordId {
        self.id
    }

    /// The write-ahead intent.
    pub fn intent(&self) -> &EffectIntent {
        &self.intent
    }

    /// Current lifecycle state.
    pub fn state(&self) -> EffectState {
        self.state
    }

    /// Provider evidence, present iff [`EffectRecord::is_terminal`].
    pub fn outcome(&self) -> Option<&ObservedOutcome> {
        self.outcome.as_ref()
    }

    /// The resolver that settled an `Ambiguous` effect, if any.
    pub fn resolver(&self) -> Option<&ResolverIdentity> {
        self.resolver.as_ref()
    }

    /// Why the outcome became unknown, if it did.
    pub fn ambiguous_reason(&self) -> Option<&str> {
        self.ambiguous_reason.as_deref()
    }

    /// When the record last transitioned.
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Whether the effect has been settled.
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    /// Whether this record blocks its dependents (F1).
    pub fn blocks_dependents(&self) -> bool {
        self.state.blocks_dependents()
    }

    /// Whether this record blocks a retry (F2).
    pub fn blocks_retry(&self) -> bool {
        self.state.blocks_retry()
    }

    /// Whether provider evidence has been recorded.
    pub fn has_evidence(&self) -> bool {
        self.outcome.is_some()
    }

    /// Whether this record is currently unsettled.
    pub fn is_ambiguous(&self) -> bool {
        self.state.is_ambiguous()
    }

    /// Whether this record matches a ledger query.
    ///
    /// Adapters use this to implement the read side without duplicating the
    /// query semantics; `Ambiguous` is the operator surface that lists what is
    /// unsettled.
    pub fn matches_query(&self, query: &EffectQuery) -> bool {
        match query {
            EffectQuery::ById { record_id } => self.id == *record_id,
            EffectQuery::ByDag { dag_id } => self.intent.dag_id == *dag_id,
            EffectQuery::Ambiguous { dag_id } => {
                self.is_ambiguous() && dag_id.is_none_or(|dag| self.intent.dag_id == dag)
            }
            EffectQuery::Dispatched { dag_id } => {
                self.state == EffectState::Dispatched
                    && dag_id.is_none_or(|dag| self.intent.dag_id == dag)
            }
        }
    }

    /// The instant after which a `Dispatched` record has no live owner.
    ///
    /// The deadline is derived from the intent's `created_at` plus the dispatch
    /// lease, so the record shape does not carry a second timestamp. A record
    /// that is still `Dispatched` past this instant was written before a crash
    /// (or its owner stopped reporting) and must be swept to `Ambiguous`.
    pub fn dispatch_deadline(&self, lease: std::time::Duration) -> DateTime<Utc> {
        let lease = chrono::Duration::from_std(lease)
            .unwrap_or_else(|_| chrono::Duration::seconds(i64::MAX / 2));
        self.intent.created_at + lease
    }

    /// Whether this record is `Dispatched` and older than `lease`.
    ///
    /// Only `Dispatched` can expire: a terminal record is settled, and an
    /// `Ambiguous` record is already awaiting a resolver.
    pub fn is_dispatch_expired(&self, now: DateTime<Utc>, lease: std::time::Duration) -> bool {
        self.state == EffectState::Dispatched && self.dispatch_deadline(lease) < now
    }

    /// Settle a `Dispatched` effect with definite provider evidence.
    ///
    /// # Errors
    /// - [`CommitmentError::AlreadySettled`] if the record is terminal
    /// - [`CommitmentError::NotAmbiguous`] if the record is `Ambiguous` — that
    ///   state can only be left by [`EffectRecord::resolve`]
    pub fn settle_definite(
        mut self,
        outcome: ObservedOutcome,
        now: DateTime<Utc>,
    ) -> Result<Self, CommitmentError> {
        if self.state.is_terminal() {
            return Err(CommitmentError::AlreadySettled {
                record_id: self.id,
                state: self.state,
            });
        }
        if self.state == EffectState::Ambiguous {
            return Err(CommitmentError::NotAmbiguous {
                record_id: self.id,
                state: self.state,
            });
        }
        self.state = outcome.terminal_state();
        self.outcome = Some(outcome);
        self.ambiguous_reason = None;
        self.updated_at = now;
        Ok(self)
    }

    /// Mark a `Dispatched` effect whose outcome is unknown.
    ///
    /// A lost response, timeout, cancellation in flight, or process death is
    /// **not** proof of absence. The record keeps blocking dependents and retry
    /// until a resolver presents provider evidence.
    ///
    /// # Errors
    /// - [`CommitmentError::AlreadySettled`] if the record is terminal
    /// - [`CommitmentError::Contended`] if it is already `Ambiguous`
    pub fn mark_ambiguous(
        mut self,
        reason: impl Into<String>,
        now: DateTime<Utc>,
    ) -> Result<Self, CommitmentError> {
        if self.state.is_terminal() {
            return Err(CommitmentError::AlreadySettled {
                record_id: self.id,
                state: self.state,
            });
        }
        if self.state == EffectState::Ambiguous {
            return Err(CommitmentError::Contended {
                record_id: Some(self.id),
                detail: "effect is already ambiguous".to_string(),
            });
        }
        self.state = EffectState::Ambiguous;
        self.ambiguous_reason = Some(reason.into());
        self.updated_at = now;
        Ok(self)
    }

    /// Leave `Ambiguous` with provider evidence and a recorded resolver (F3/F4).
    ///
    /// # Errors
    /// - [`CommitmentError::NotAmbiguous`] unless the record is `Ambiguous`
    pub fn resolve(
        mut self,
        outcome: ObservedOutcome,
        resolver: ResolverIdentity,
        now: DateTime<Utc>,
    ) -> Result<Self, CommitmentError> {
        if self.state != EffectState::Ambiguous {
            return Err(CommitmentError::NotAmbiguous {
                record_id: self.id,
                state: self.state,
            });
        }
        self.state = outcome.terminal_state();
        self.outcome = Some(outcome);
        self.resolver = Some(resolver);
        self.ambiguous_reason = None;
        self.updated_at = now;
        Ok(self)
    }
}

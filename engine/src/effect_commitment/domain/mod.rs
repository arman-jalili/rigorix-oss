//! Domain types, invariants, and errors for Effect Commitment.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#ddd-layers
//! Implements: ISSUE-CONTRACT-FREEZE — EffectIntent, EffectState, EffectRecord,
//!              ObservedOutcome, ResolverIdentity, CommitmentError, EffectEvent
//! Issue: #1113
//!
//! Pure types only: **zero framework imports**. This module is the frozen model
//! that every adapter and use case depends on. Storage types (rows, tables,
//! connection handles) are forbidden here and belong behind the
//! `EffectCommitmentLedger` port in `interfaces/`.
//!
//! # Contract (Frozen)
//! - `Ambiguous` is a non-terminal state; only provider evidence may leave it
//! - A terminal record always carries an `ObservedOutcome`
//! - The dispatching process and the agent are representable resolvers but are
//!   rejected by [`crate::effect_commitment::application::ReconciliationPolicy`]
//! - No catch-all error variant

pub mod error;
pub mod event;
pub mod intent;
pub mod outcome;
pub mod query;
pub mod record;
pub mod resolver;
pub mod state;

pub use error::CommitmentError;
pub use event::EffectEvent;
pub use intent::EffectIntent;
pub use outcome::{ObservedOutcome, OutcomeVerdict};
pub use query::EffectQuery;
pub use record::{EffectRecord, EffectRecordId};
pub use resolver::{ResolverIdentity, ResolverKind};
pub use state::EffectState;

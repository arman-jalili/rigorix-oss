//! Domain entities and interfaces for the Approval Binding bounded context.
//!
//! @canonical .pi/architecture/modules/approval.md#domain
//! Implements: Contract Freeze — domain entities ExecutionIntent, IntentHash,
//!   ApprovalRecord, DecisionContext, ApprovalStatus, ScopeViolation,
//!   ApprovalError
//! Issue: #786 (approval epic — contract freeze)
//!
//! Pure business logic with zero framework imports (`thiserror` and serde
//! derives only). The domain owns the binding chain: the canonical intent,
//! the digest that ties an approval to the exact dispatch payload, the
//! durable record of a human decision, and the typed failure modes.
//!
//! # Contract (Frozen)
//! - `ExecutionIntent` is the single canonical step payload — what is shown
//!   is what is hashed (one renderer, one hash)
//! - `IntentHash` binds the approval to the exact dispatch bytes; identical
//!   payloads hash identically, any byte change produces a different digest
//! - `ApprovalRecord` status transitions: `Pending → Consumed | Expired |
//!   Superseded` (single-use; consume happens once on terminal outcome)
//! - `DecisionContext` records what the human was shown; `summary` is always
//!   envelope-safe, `full_payload` is opt-in (privacy pattern)
//! - `ScopeViolation` is first-class non-blocking evidence (R5)
//! - `ApprovalError` `IntentMismatch` is **non-retriable** — re-approval is
//!   the only recovery

pub mod error;
pub mod hash;
pub mod intent;
pub mod record;
pub mod violation;

pub use error::ApprovalError;
pub use hash::IntentHash;
pub use intent::ExecutionIntent;
pub use record::{ApprovalRecord, ApprovalStatus, DecisionContext};
pub use violation::ScopeViolation;

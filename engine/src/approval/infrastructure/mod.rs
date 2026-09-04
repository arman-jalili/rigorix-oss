//! Infrastructure layer interfaces for the Approval Binding bounded context.
//!
//! @canonical .pi/architecture/modules/approval.md#infrastructure
//! Implements: Contract Freeze — ApprovalRepository interface
//! Issue: #786 (approval epic — contract freeze)
//!
//! Durable approval-record persistence, backed by state persistence
//! (`ExecutionState`) — approval records are appended to the persisted state
//! file so they survive cross-process resume (GAP-3) and stay queryable via
//! `ApprovalService::get_approval`.
//!
//! # Contract (Frozen)
//! - Records are node-scoped: `load(node_id)` returns the single approval for
//!   that node (single-use semantics)
//! - Persistence is hidden behind this interface — no caller touches storage
//!   format or columns directly
//! - The migration rule (legacy `approved` sets without records are invalidated
//!   on hydrate) is enforced at hydration time by the implementation
//! - Degradation: when state persistence is unavailable, the implementation
//!   holds records in memory and logs a warning (same pattern as the
//!   evaluation repository)

pub mod effect_scope;
pub mod repository;

pub use effect_scope::{ChangeSnapshot, GitDiffEffectOracle};
pub use repository::ApprovalRepository;

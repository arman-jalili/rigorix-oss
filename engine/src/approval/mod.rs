//! Approval Binding bounded context.
//!
//! @canonical .pi/architecture/modules/approval.md
//! Implements: Contract Freeze — approval module root
//! Issue: #786 (approval epic — contract freeze)
//!
//! Binds human approval to the exact resolved execution intent — replay
//! protection at the step level, first-class signed evidence, and
//! effect-scope verification. Today `approve_node` binds an approval to a
//! *node*, not to a *consequence*; this module binds it to the byte-level
//! `ExecutionIntent` that will dispatch (R1), re-verifies pre-dispatch
//! (R2), records a signed single-use `ApprovalRecord` (R3), captures what
//! the human was shown (R4), and flags post-execution effects outside the
//! declared scope (R5).
//!
//! # The Binding Chain
//!
//! > **Shown = Dispatched = Executed (at dispatch level).**
//! > What the human was shown (`decision_context`) is derived from the same
//! > canonical `ExecutionIntent` that is hashed at approval time
//! > (`intent_hash`) and re-derived at dispatch time. The signed envelope
//! > proves the chain.
//!
//! # Architecture
//!
//! ```text
//! approval/
//! ├── domain/             # Domain entities: ExecutionIntent, IntentHash,
//! │   │                     ApprovalRecord + DecisionContext + ApprovalStatus,
//! │   │                     ScopeViolation, ApprovalError
//! │   ├── intent.rs       # ExecutionIntent value object (R1)
//! │   ├── hash.rs         # IntentHash (HMAC-SHA256 digest, R1)
//! │   ├── record.rs       # ApprovalRecord + DecisionContext + ApprovalStatus
//! │   ├── violation.rs    # ScopeViolation (R5)
//! │   └── error.rs        # ApprovalError enum (thiserror)
//! ├── application/        # Service trait and DTOs
//! │   ├── service.rs      # ApprovalService trait + IntentVerification
//! │   └── dto/            # ApproveInput / ApproveOutput DTO schemas
//! └── infrastructure/     # Durable record repository interface
//!     └── repository/     # ApprovalRepository trait (state persistence)
//! ```
//!
//! **Note:** This module has **no `interfaces/` layer** — its API is exposed
//! through the `ApprovalService` trait and consumed by `execution_engine` at
//! the dispatch choke point (`run_dispatch_loop`). MCP/HTTP surfaces live in
//! the MCP crate (execution-tools). The 3-layer shape matches
//! `quality_gates` and `scored_evaluation`.
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract
//! - Domain/application method bodies are `todo!()` stubs — behavior lands in
//!   the implementation issues (ISSUE-EXECUTIONINTENT … ISSUE-APPROVALERROR)
//!
//! # Related Components
//!
//! - `dag_engine` — `TaskNode` is the intent source (`tool` + `intent` +
//!   `requires_approval`)
//! - `execution_engine` — verification at the single dispatch choke point,
//!   `NodeStatus::IntentMismatch`
//! - `identity` — `approver_id` / `authority` / `token_claims_ref` attribution
//! - `audit` — `ApprovalRecorded` event, envelope `approval_events[]`,
//!   `scope_violations[]`, `decision_context_ref`
//! - `state_persistence` — durable `ApprovalRecord` storage (+ legacy migration)
//! - `failure_classification` — `FailureType::IntentMismatch` (non-retriable)

pub mod application;
pub mod domain;
pub mod infrastructure;

pub use application::*;
pub use domain::*;
pub use infrastructure::*;

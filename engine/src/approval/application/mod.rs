//! Application layer interfaces for the Approval Binding bounded context.
//!
//! @canonical .pi/architecture/modules/approval.md#application
//! Implements: Contract Freeze — ApprovalService trait, IntentVerification,
//!   ApproveInput / ApproveOutput DTOs
//! Issue: #786 (approval epic — contract freeze)
//!
//! This module defines:
//! - The `ApprovalService` use-case trait (R1+R3 capture, R2 verify,
//!   single-use consume, R5 scope-violation reporting)
//! - The `IntentVerification` outcome enum (the R2 verdict at the dispatch
//!   choke point)
//! - Input/Output DTOs (`ApproveInput`, `ApproveOutput`)
//!
//! # Contract (Frozen)
//! - All service methods are async and trait-object safe (`Send + Sync`,
//!   via `async-trait`)
//! - All public methods return domain types (`ApprovalRecord`,
//!   `ScopeViolation`) or `ApprovalError`
//! - Verification is fail-closed: an approval-service error halts the node —
//!   never a silent pass-through to dispatch
//! - The engine API surface retains `approve_node` for compatibility,
//!   delegating internally to this contract
//! - No implementation logic — only contract signatures (behavior lands in
//!   ISSUE-APPROVALSERVICE)

pub mod dto;
pub mod service;

pub use dto::{ApproveInput, ApproveOutput};
pub use service::{ApprovalService, IntentVerification};

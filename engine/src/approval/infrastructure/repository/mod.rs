//! Repository interfaces for the Approval Binding bounded context.
//!
//! @canonical .pi/architecture/modules/approval.md#infrastructure
//! Implements: Contract Freeze — ApprovalRepository trait
//! Issue: #786 (approval epic — contract freeze); implementations in
//!   ISSUE-APPROVALSERVICE (#792)
//!
//! # Contract (Frozen)
//! - `save` persists (or replaces) the record for a node — replaces model
//!   supersedes older approvals
//! - `load` reads the current record for a node; `None` means never approved
//!   or superseded-and-purged
//! - `delete` removes a record (consumed-record compaction / test cleanup)

pub mod approval_repository;
pub mod approval_repository_impl;

pub use approval_repository::ApprovalRepository;
pub use approval_repository_impl::{FileBackedApprovalRepository, InMemoryApprovalRepository};

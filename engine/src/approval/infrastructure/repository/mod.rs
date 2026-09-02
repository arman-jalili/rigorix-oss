//! Repository interfaces for the Approval Binding bounded context.
//!
//! @canonical .pi/architecture/modules/approval.md#infrastructure
//! Implements: Contract Freeze — ApprovalRepository trait
//! Issue: #786 (approval epic — contract freeze)
//!
//! # Contract (Frozen)
//! - `save` persists (or replaces) the record for a node — replaces model
//!   supersedes older approvals
//! - `load` reads the current record for a node; `None` means never approved
//!   or superseded-and-purged
//! - `delete` removes a record (consumed-record compaction / test cleanup)

pub mod approval_repository;

pub use approval_repository::ApprovalRepository;

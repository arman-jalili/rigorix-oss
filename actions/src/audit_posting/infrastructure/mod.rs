//! Infrastructure layer for the Audit Posting bounded context.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md#infrastructure
//! Implements: Contract Freeze — repository interfaces for audit backend and filesystem storage
//! Issue: issue-contract-freeze
//!
//! This module defines the repository interfaces that abstract data access
//! for the Audit Posting module. Implementations will post to remote HTTP
//! backends, read/write to the local filesystem, and manage HMAC signing keys.

pub mod filesystem_audit_backend;
pub mod http_audit_backend;
pub mod repository;

pub use filesystem_audit_backend::*;
pub use http_audit_backend::*;
pub use repository::*;

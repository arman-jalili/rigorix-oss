//! Infrastructure layer for the Audit Tools bounded context.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#infrastructure
//! Implements: Contract Freeze — repository interfaces for audit persistence
//!
//! This module provides:
//! - Repository interface definitions (traits) for audit state
//! - Engine-facing client abstraction
//!
//! # Contract (Frozen)
//!
//! - Repository traits only define contracts — no implementation in interfaces
//! - All methods are async
//! - All methods return domain error types

pub mod in_memory_audit_service;
pub mod repository;

pub use in_memory_audit_service::InMemoryAuditQueryService;
pub use repository::AuditRepository;

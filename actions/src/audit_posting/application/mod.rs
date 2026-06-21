//! Application layer for the Audit Posting bounded context.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md#application
//! Implements: Contract Freeze — service traits, DTO schemas, factory interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines the application-level contracts:
//! - Service interfaces (use cases) in `service.rs`
//! - Input/output DTO schemas in `dto/`
//! - Factory interfaces in `factory.rs`
//!
//! # Contract (Frozen)
//! - All service traits are async (use `async-trait`)
//! - All public methods return `Result<_, AuditPostingError>`
//! - DTOs carry full documentation for each field
//! - No implementation — only contract signatures

pub mod audit_backend_factory_impl;
pub mod dto;
pub mod factory;
pub mod service;

pub use audit_backend_factory_impl::*;
pub use dto::*;
pub use factory::*;
pub use service::*;

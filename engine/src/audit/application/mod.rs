//! Application layer interfaces for the Audit bounded context.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: #13
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing domain objects
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, AuditError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod audit_queue_impl;
pub mod audit_sender_impl;
pub mod audit_service_impl;
pub mod circuit_breaker_factory_impl;
pub mod circuit_breaker_impl;
pub mod dto;
pub mod envelope_factory_impl;
pub mod factory;
pub mod service;

pub use audit_queue_impl::*;
pub use audit_sender_impl::*;
pub use audit_service_impl::*;
pub use circuit_breaker_factory_impl::*;
pub use circuit_breaker_impl::*;
pub use dto::*;
pub use envelope_factory_impl::*;
pub use factory::*;
pub use service::*;

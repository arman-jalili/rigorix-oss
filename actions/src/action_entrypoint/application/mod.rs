//! Application layer for the Action Entrypoint bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md#application
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
//! - All public methods return `Result<_, ActionError>`
//! - DTOs carry full documentation for each field
//! - No implementation — only contract signatures

pub mod context_builder_impl;
pub mod dto;
pub mod factory;
pub mod mode_resolver_impl;
pub mod router_impl;
pub mod service;

pub use context_builder_impl::*;
pub use dto::*;
pub use factory::*;
pub use mode_resolver_impl::*;
pub use router_impl::*;
pub use service::*;
